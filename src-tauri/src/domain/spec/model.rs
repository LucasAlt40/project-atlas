//! What a specification is, as data. Nothing here runs an agent, reads a file or knows a format:
//! the framework (`OpenSpec`) is behind `application::specs::SpecFramework`, and the rules that
//! move these records from one state to another are in `rules.rs`.
//!
//! Three things are kept apart on purpose, because mixing them is how a half-finished document
//! gets called ready: what the *run* of an agent is doing (`Generation`), what the *person*
//! decided about the current revision (`Review`), and what the *checks* found in it
//! (`ArtifactValidation`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::domain::interaction::InteractionOption;

pub const MAX_TITLE: usize = 120;
pub const MAX_REQUEST: usize = 8_000;
pub const MAX_CONTEXT: usize = 8_000;
pub const MAX_NOTE: usize = 2_000;
/// Longest document kept in one file of a revision.
pub const MAX_FILE: usize = 120_000;
/// Files in one revision (an `OpenSpec` change has one spec file per capability).
pub const MAX_FILES: usize = 24;
/// Runs kept in the record; the oldest go first.
pub const MAX_RUNS_KEPT: usize = 80;
/// Revisions kept per artifact. Approved revisions are never dropped.
pub const MAX_REVISIONS_KEPT: usize = 30;

/// The formats Atlas can write a specification in. Only `OpenSpec` exists; the field is there so a
/// record says which one it is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameworkKind {
    Openspec,
}

/// The documents of the `OpenSpec` change. One artifact each; `Specs` holds a file per capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Proposal,
    Specs,
    Design,
    Tasks,
}

impl ArtifactKind {
    pub const ALL: [Self; 4] = [Self::Proposal, Self::Specs, Self::Design, Self::Tasks];

    /// The artifact's id inside a spec, which is also its name on the wire.
    pub fn id(self) -> &'static str {
        match self {
            Self::Proposal => "proposal",
            Self::Specs => "specs",
            Self::Design => "design",
            Self::Tasks => "tasks",
        }
    }
}

/// A file of a revision. `path` is relative to the change's folder (`proposal.md`,
/// `specs/user-auth/spec.md`), never absolute and never leaving it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevisionOrigin {
    /// The first document an agent wrote for the artifact.
    Generated,
    /// The agent revised the existing document after the person's feedback.
    Revised,
    /// The person had the document thrown away and written again.
    Regenerated,
    /// Read from an `OpenSpec` change already in the project.
    Imported,
}

/// One version of an artifact. Never edited and never dropped once approved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Revision {
    /// 1, 2, 3… in order of creation.
    pub number: u32,
    pub files: Vec<SpecFile>,
    pub origin: RevisionOrigin,
    /// The execution that wrote it, when an agent did.
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub agent_id: Option<String>,
    /// The revision of each artifact this one depends on that was approved when it was written.
    /// When a dependency moves past it, this artifact is outdated.
    #[serde(default)]
    pub based_on: BTreeMap<String, u32>,
    pub created_at: u64,
}

/// What an agent's run for an artifact is doing. Says nothing about whether the document is
/// good or approved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum Generation {
    /// No run is going: not started yet, or the last one ended and left its revision.
    Idle,
    Running {
        run_id: String,
        started_at: u64,
        mode: GenerationMode,
    },
    /// The agent stopped to ask the person something. The run ended; answering starts another.
    WaitingForInput { question_id: String },
    /// The last run produced nothing usable. The previous revision, if any, is untouched.
    Failed {
        message: String,
        at: u64,
        /// The app ended while the run was going.
        interrupted: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationMode {
    /// There was no document yet.
    Initial,
    /// Change the existing document as the feedback says.
    Revise,
    /// Write it again; the old one is kept in the history.
    Regenerate,
}

/// What the person decided about the artifact's latest revision.
/// `AwaitingReview` is the word the screens and the stored records use: it keeps its name.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Review {
    /// Nothing to review: no revision yet.
    None,
    AwaitingReview,
    ChangesRequested,
    Approved,
}

/// The three answers to a document: the vocabulary of ADR 0031 (`approve`, `revise`, `redo`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionKind {
    Approve,
    Revise,
    Redo,
}

impl DecisionKind {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            crate::domain::interaction::PLAN_APPROVE => Some(Self::Approve),
            crate::domain::interaction::PLAN_REVISE => Some(Self::Revise),
            crate::domain::interaction::PLAN_REDO => Some(Self::Redo),
            _ => None,
        }
    }
}

/// What the person said about one revision, kept for the history and for the agent's next pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Feedback {
    pub id: String,
    pub revision: u32,
    pub kind: DecisionKind,
    #[serde(default)]
    pub note: Option<String>,
    pub at: u64,
}

/// Why an artifact may no longer match what it was written from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Outdated {
    /// The artifact that changed.
    pub because: String,
    /// The revision of it that made this one outdated.
    pub revision: u32,
    pub at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueSeverity {
    /// Keeps the spec from being ready.
    Error,
    /// Worth reading; does not block.
    Warning,
    /// A way to make it better; does not block.
    Suggestion,
}

/// One thing the deterministic checks found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    /// Stable code the interface translates (`requirement_no_scenario`…).
    pub code: String,
    pub severity: IssueSeverity,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub line: Option<u32>,
    /// English text, for logs; the interface uses the code and `params`.
    pub message: String,
    #[serde(default)]
    pub params: BTreeMap<String, String>,
}

/// What the checks found in one revision. Tied to that revision: a later one has no result until
/// the checks run on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactValidation {
    pub revision: u32,
    /// Which rules produced this (`atlas-openspec/1`): a result from older rules is rerun.
    pub validator: String,
    pub issues: Vec<ValidationIssue>,
    pub at: u64,
}

impl ArtifactValidation {
    pub fn errors(&self) -> usize {
        self.count(IssueSeverity::Error)
    }

    pub fn count(&self, severity: IssueSeverity) -> usize {
        self.issues
            .iter()
            .filter(|issue| issue.severity == severity)
            .count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecArtifact {
    /// `proposal`, `specs`, `design` or `tasks`.
    pub id: String,
    pub kind: ArtifactKind,
    /// The artifacts this one is written from, by id.
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub revisions: Vec<Revision>,
    pub generation: Generation,
    /// About the latest revision.
    pub review: Review,
    /// The latest revision the person approved. Equal to the latest one only while `review` is
    /// `Approved`; kept after a new revision so what was approved before is not forgotten.
    #[serde(default)]
    pub approved_revision: Option<u32>,
    #[serde(default)]
    pub outdated: Option<Outdated>,
    #[serde(default)]
    pub validation: Option<ArtifactValidation>,
    #[serde(default)]
    pub feedback: Vec<Feedback>,
    #[serde(default)]
    pub agent_id: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl SpecArtifact {
    pub fn latest(&self) -> Option<&Revision> {
        self.revisions.last()
    }

    pub fn latest_number(&self) -> Option<u32> {
        self.latest().map(|r| r.number)
    }

    pub fn revision(&self, number: u32) -> Option<&Revision> {
        self.revisions.iter().find(|r| r.number == number)
    }

    /// Approved as it stands now: the latest revision is the approved one and nothing it was
    /// written from has moved since.
    pub fn is_approved(&self) -> bool {
        self.review == Review::Approved
            && self.outdated.is_none()
            && self.latest_number().is_some()
            && self.latest_number() == self.approved_revision
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestionSource {
    /// An agent writing an artifact needed something only the person knows.
    Artifact,
    /// The reviewing agent found something that is the person's to decide.
    Review,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestionStatus {
    Pending,
    Answered,
}

/// What an agent asked (`atlas-interaction`, kind clarification). Kept with the spec so it
/// survives a restart.
/// What the person is asked is `question`: the name the screens and stored records use.
#[allow(clippy::struct_field_names)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    pub id: String,
    pub source: QuestionSource,
    #[serde(default)]
    pub artifact_id: Option<String>,
    #[serde(default)]
    pub finding_id: Option<String>,
    pub question: String,
    #[serde(default)]
    pub context: String,
    #[serde(default)]
    pub options: Vec<InteractionOption>,
    pub status: QuestionStatus,
    #[serde(default)]
    pub choice: Option<String>,
    #[serde(default)]
    pub answer: Option<String>,
    /// The execution that asked.
    pub run_id: String,
    pub asked_at: u64,
    #[serde(default)]
    pub answered_at: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingStatus {
    Open,
    /// The document was changed (or the person answered) so it no longer applies.
    Resolved,
    /// The person decided to go on with it, in writing.
    AcceptedRisk,
    /// The person decided it is not a problem.
    Dismissed,
}

/// What the reviewing agent found, as it said it: an opinion with evidence, never a check that
/// failed. Whether it keeps the spec from being ready is the person's call, not the agent's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// Stable across reviews: the same problem found twice is one finding.
    pub id: String,
    /// `ambiguity`, `contradiction`, `missing_scenario`, `edge_case`, `risk`, `coverage`…
    pub category: String,
    pub severity: FindingSeverity,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub artifact_id: Option<String>,
    /// Where in the artifact (a requirement name, a section, a task id).
    #[serde(default)]
    pub reference: String,
    #[serde(default)]
    pub evidence: String,
    #[serde(default)]
    pub rationale: String,
    /// What the person would have to decide, when it is theirs to decide.
    #[serde(default)]
    pub question: Option<String>,
    #[serde(default)]
    pub suggestion: Option<String>,
    /// The revision of `artifact_id` that was read when it was found: a later review that no
    /// longer reports it, of a later revision, is what lets Atlas call it resolved.
    #[serde(default)]
    pub artifact_revision: Option<u32>,
    pub status: FindingStatus,
    #[serde(default)]
    pub resolution_note: Option<String>,
    /// The review that found it.
    pub review_id: String,
    pub created_at: u64,
    #[serde(default)]
    pub resolved_at: Option<u64>,
}

impl Finding {
    /// Whether it keeps the spec from being ready while it is open: serious, or a question only
    /// the person can answer.
    pub fn blocks(&self) -> bool {
        self.status == FindingStatus::Open
            && (self.severity == FindingSeverity::High || self.question.is_some())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ReviewRun {
    Running {
        run_id: String,
        started_at: u64,
    },
    WaitingForInput {
        question_id: String,
    },
    Failed {
        message: String,
        at: u64,
        interrupted: bool,
    },
    Completed {
        at: u64,
        summary: String,
    },
    /// The person decided to go without it, in writing. Counts as done for readiness and says so.
    Waived {
        note: String,
        at: u64,
    },
}

/// The reviewing agent's pass over the spec, tied to the revisions it read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiReview {
    pub id: String,
    pub agent_id: String,
    /// The revision of each artifact it read. A review of older revisions is outdated.
    pub revisions: BTreeMap<String, u32>,
    pub run: ReviewRun,
    pub started_at: u64,
}

/// What the checks across the artifacts found (requirements without tasks, links to nothing…).
/// Tied to the revisions it read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyValidation {
    pub revisions: BTreeMap<String, u32>,
    pub validator: String,
    pub issues: Vec<ValidationIssue>,
    pub at: u64,
}

/// The person's last word, tied to the revisions it was given on. A change to any of them voids it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalApproval {
    pub revisions: BTreeMap<String, u32>,
    /// Warnings that were showing when the person approved, so the decision is on record.
    #[serde(default)]
    pub acknowledged_warnings: Vec<String>,
    pub at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum Publication {
    NotPublished,
    /// Writing the files. A crash here leaves this state: the next publish picks it up.
    InProgress {
        started_at: u64,
        change_id: String,
        /// The files already written, so a repeat does not trip over them.
        written: Vec<String>,
        revisions: BTreeMap<String, u32>,
    },
    Published {
        change_id: String,
        /// Where it is, as the project's own relative path (`openspec/changes/<id>`).
        root: String,
        files: Vec<String>,
        revisions: BTreeMap<String, u32>,
        at: u64,
    },
    Failed {
        message: String,
        at: u64,
        change_id: String,
        written: Vec<String>,
    },
}

/// One line of what the `OpenSpec` command found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliIssue {
    pub level: String,
    pub path: String,
    pub message: String,
}

/// What the `OpenSpec` command said of the published change, when the person asked it to look.
/// A second opinion beside Atlas's own checks: it never decides whether the spec is ready.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliCheck {
    /// The revisions that were published when it looked.
    pub revisions: BTreeMap<String, u32>,
    pub cli_version: String,
    pub valid: bool,
    pub issues: Vec<CliIssue>,
    pub at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Archived {
    pub at: u64,
    /// Where the change went (`openspec/changes/archive/2026-10-12-<id>`), when the project's own
    /// files were moved; `None` when only Atlas's record was closed.
    #[serde(default)]
    pub path: Option<String>,
    /// `cli` when the `OpenSpec` CLI did it, `atlas` when Atlas only closed its own record.
    pub method: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum SpecOrigin {
    Created,
    /// Read from a change already in the project.
    Imported {
        path: String,
        at: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunKind {
    Generate,
    Review,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunOutcome {
    Completed,
    WaitingForInput,
    Failed,
    Cancelled,
    Interrupted,
    /// It finished after something newer took its place, so its result was not used.
    Superseded,
}

/// One agent run made for the spec: the execution is the Atlas execution, this is the link to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecRun {
    /// The execution id.
    pub id: String,
    pub kind: RunKind,
    #[serde(default)]
    pub artifact_id: Option<String>,
    pub agent_id: String,
    pub started_at: u64,
    #[serde(default)]
    pub ended_at: Option<u64>,
    #[serde(default)]
    pub outcome: Option<RunOutcome>,
}

/// A run of a workflow that implements the spec, and the revisions it was given.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImplementationLink {
    pub workflow_execution_id: String,
    pub workflow_id: String,
    pub workflow_name: String,
    pub revisions: BTreeMap<String, u32>,
    pub started_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spec {
    pub id: String,
    pub title: String,
    /// What the person asked for, as they wrote it.
    pub request: String,
    #[serde(default)]
    pub context: String,
    pub workspace_id: String,
    pub project_id: String,
    pub framework: FrameworkKind,
    /// The framework's own schema and version (`spec-driven/1`).
    pub framework_schema: String,
    /// The name of the change folder in the project (`add-dark-mode`): kebab-case, stable.
    pub change_id: String,
    /// The agent that writes the documents, and the one that reviews them.
    pub author_agent_id: String,
    pub reviewer_agent_id: String,
    /// The person pressed start. Until then nothing has run and nothing will.
    pub started: bool,
    pub artifacts: Vec<SpecArtifact>,
    #[serde(default)]
    pub questions: Vec<Question>,
    #[serde(default)]
    pub findings: Vec<Finding>,
    #[serde(default)]
    pub ai_review: Option<AiReview>,
    #[serde(default)]
    pub consistency: Option<ConsistencyValidation>,
    #[serde(default)]
    pub final_approval: Option<FinalApproval>,
    pub publication: Publication,
    #[serde(default)]
    pub archived: Option<Archived>,
    #[serde(default)]
    pub cli_check: Option<CliCheck>,
    #[serde(default)]
    pub implementations: Vec<ImplementationLink>,
    #[serde(default)]
    pub runs: Vec<SpecRun>,
    /// How many times the person said to go on after the token budget was reached; each one
    /// grants the budget again.
    #[serde(default)]
    pub budget_steps: u32,
    pub origin: SpecOrigin,
    /// Bumped on every change, so a screen that read an older state can be told it did.
    pub version: u64,
    pub created_at: u64,
    pub updated_at: u64,
}

impl Spec {
    pub fn artifact(&self, id: &str) -> Option<&SpecArtifact> {
        self.artifacts.iter().find(|a| a.id == id)
    }

    pub fn artifact_mut(&mut self, id: &str) -> Option<&mut SpecArtifact> {
        self.artifacts.iter_mut().find(|a| a.id == id)
    }

    /// The latest revision of each artifact that has one.
    pub fn current_revisions(&self) -> BTreeMap<String, u32> {
        self.artifacts
            .iter()
            .filter_map(|a| a.latest_number().map(|n| (a.id.clone(), n)))
            .collect()
    }

    pub fn question(&self, id: &str) -> Option<&Question> {
        self.questions.iter().find(|q| q.id == id)
    }
}

/// How a spec is doing, for a list. Derived from the record, never stored: nothing can set a spec
/// to a status its documents do not support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecStatus {
    /// Created; the person has not started it.
    Draft,
    /// Started; the next document is waiting for someone to ask for it.
    AwaitingGeneration,
    Generating,
    WaitingForInput,
    AwaitingReview,
    ChangesRequested,
    /// Every document is approved; the checks or the review have not run (or ran on older text).
    AwaitingValidation,
    /// Something stands in the way and the person has to act (outdated documents, errors,
    /// findings).
    Blocked,
    /// Every condition but the person's last word is met.
    Approved,
    ReadyForImplementation,
    Published,
    Archived,
    Failed,
}
