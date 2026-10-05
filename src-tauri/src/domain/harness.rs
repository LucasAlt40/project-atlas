//! The Project Harness: what Atlas knows about a project, kept in the project's own `.atlas/`
//! folder. These are plain data types; nothing here reads the disk or knows a runtime.
//!
//! Knowledge has three kinds of source, and they are never equally trusted:
//! *facts* read from the repository, *inferences* concluded from it, and what the *user* said.
//! Every statement that came from analysis points at its [`Evidence`].

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The manifest version this build reads and writes. New manifest fields are optional, so
/// manifests written by V0.7 still load.
pub const HARNESS_VERSION: u32 = 1;
/// Version of `knowledge/findings.yaml`.
pub const KNOWLEDGE_VERSION: u32 = 1;

/// How sure the analysis is.
///
/// - `High`: direct, unambiguous evidence (`package.json` lists `@angular/core`).
/// - `Medium`: several pieces of evidence, but some interpretation (`domain/` + `application/` +
///   `infrastructure/` side by side).
/// - `Low`: weak signals. A `Low` finding is never presented to an agent as established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

/// Where a piece of knowledge comes from. Ranked for precedence: the user's word beats a fact,
/// a fact beats an inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// Read from the repository.
    Fact,
    /// Concluded from the repository's shape (by rules or by a model).
    Inference,
    /// An inference the user explicitly confirmed.
    UserConfirmed,
    /// A value the user replaced.
    UserCorrected,
    /// Produced by Atlas itself, not from evidence (for example "architecture: unknown").
    Generated,
}

impl Origin {
    /// The user has spoken about this: it outranks facts and inferences.
    pub fn is_user(self) -> bool {
        matches!(self, Self::UserConfirmed | Self::UserCorrected)
    }
}

/// Whether a statement was *checked*, as opposed to merely read or concluded. Orthogonal to
/// [`Origin`]: an inference is `Unverified` until something confirms it; a fact read from a
/// manifest is `Verified` by that very file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    /// Nobody and nothing checked it. The default: a statement is not verified until proven.
    #[default]
    Unverified,
    /// Checked by `method` at `verified_at`.
    Verified,
    /// Was verified, but the project changed where its evidence lives, so it may no longer hold.
    Stale,
}

/// How a statement was checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationMethod {
    /// The statement is what a repository file says (`package.json` lists the dependency).
    RepositoryFile,
    /// Atlas ran the command and it succeeded. Reserved: V0.7.2 never runs commands.
    CommandExecution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Verification {
    pub status: VerificationStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<VerificationMethod>,
}

impl Verification {
    pub fn verified(method: VerificationMethod, at: Option<u64>) -> Self {
        Self {
            status: VerificationStatus::Verified,
            verified_at: at,
            method: Some(method),
        }
    }

    pub fn is_unverified(&self) -> bool {
        self.status == VerificationStatus::Unverified
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingCategory {
    Repository,
    Language,
    Runtime,
    Framework,
    PackageManager,
    Dependency,
    Database,
    Data,
    Integration,
    Infrastructure,
    Testing,
    Build,
    Tooling,
    Ci,
    EntryPoint,
    Environment,
    Architecture,
    Module,
    Convention,
}

/// Where a statement comes from: a file or folder and, when it helps, the field in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

impl Evidence {
    pub fn new(source: &str, field: Option<&str>) -> Self {
        Self {
            source: source.to_owned(),
            field: field.map(str::to_owned),
        }
    }
}

/// One structured, evidenced statement about the project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// `category:key`, stable across analyses. Corrections and exclusions refer to it.
    pub id: String,
    pub category: FindingCategory,
    pub key: String,
    /// A name for people: `Angular 18`, `GitHub Actions`. Technology names are not translated.
    pub label: String,
    /// A version, a name, a path, a count or `"true"`.
    pub value: String,
    pub confidence: Confidence,
    pub origin: Origin,
    /// What the user's word replaced: a confirmed inference stays recognisable as one
    /// (`origin: user_confirmed`, `original_origin: inference`), never becoming a fact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_origin: Option<Origin>,
    /// Whether the statement was checked. Facts read from a repository file are verified by it;
    /// everything else is unverified until something checks it.
    #[serde(default)]
    pub verification: Verification,
    /// Why this is concluded, for inferences.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Never empty for a finding that came from analysis.
    pub evidence: Vec<Evidence>,
    /// Proposed by a model rather than by rules. Kept so a refresh can carry it over (and drop
    /// it when its evidence is gone) without asking the model again.
    #[serde(default)]
    pub by_model: bool,
}

impl Finding {
    /// A finding with a single piece of evidence: `source` and, when it adds something, the
    /// `field` (a JSON path, a dependency name, a file name).
    pub fn new(
        category: FindingCategory,
        key: &str,
        value: &str,
        confidence: Confidence,
        origin: Origin,
        source: &str,
        field: &str,
    ) -> Self {
        let category_name = serde_json::to_value(category)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        Self {
            id: format!("{category_name}:{key}"),
            category,
            key: key.to_owned(),
            label: label_for(key, value),
            value: value.to_owned(),
            confidence,
            origin,
            original_origin: None,
            verification: if origin == Origin::Fact {
                Verification::verified(VerificationMethod::RepositoryFile, None)
            } else {
                Verification::default()
            },
            reason: None,
            by_model: false,
            evidence: vec![Evidence::new(
                source,
                Some(field).filter(|f| !f.is_empty() && *f != source),
            )],
        }
    }

    /// Statements from analysis must point at something real; one that cannot is never kept.
    pub fn has_valid_evidence(&self) -> bool {
        !self.evidence.is_empty() && self.evidence.iter().all(|e| !e.source.trim().is_empty())
    }

    #[must_use]
    pub fn with_evidence(mut self, evidence: Vec<Evidence>) -> Self {
        self.evidence = evidence;
        self
    }

    #[must_use]
    pub fn with_reason(mut self, reason: &str) -> Self {
        self.reason = Some(reason.to_owned());
        self
    }

    #[must_use]
    pub fn with_label(mut self, label: &str) -> Self {
        label.clone_into(&mut self.label);
        self
    }
}

/// Whether, and how, the semantic (model) analysis ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticStatus {
    #[default]
    NotRun,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticReport {
    pub status: SemanticStatus,
    /// An error code (`snake_case`) when it failed.
    #[serde(default)]
    pub error: Option<String>,
    /// The technical reason it failed (what the runtime said, what the answer looked like), so
    /// a failure is never just a code.
    #[serde(default)]
    pub error_detail: Option<String>,
    /// Exactly the files whose content was sent to the model.
    #[serde(default)]
    pub sent_files: Vec<String>,
    /// Statements the model made that were dropped for lacking valid evidence.
    #[serde(default)]
    pub rejected: usize,
    /// The agent explored the project with its own read tools, so Atlas cannot list the files
    /// it read (`sent_files` is empty).
    #[serde(default)]
    pub explored: bool,
}

/// How the analysis went. Recorded in the Harness so it never pretends to be complete.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisInfo {
    /// A limit stopped the scan or the sampling: not everything was seen.
    pub partial: bool,
    pub scanned_entries: usize,
    #[serde(default)]
    pub sampled_files: Vec<String>,
    #[serde(default)]
    pub semantic: SemanticReport,
    #[serde(default)]
    pub analyzed_at: u64,
    /// The relevant state of the project when this was analysed. `None` in a Harness from
    /// before V0.7.2, whose staleness therefore cannot be known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<ProjectFingerprint>,
}

/// A cheap, deterministic digest of the parts of a project that Harness knowledge depends on
/// (manifests, lockfiles, configuration, CI, docs and the folder structure), kept with the
/// parts so a later check can say *what* changed, not only that something did.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFingerprint {
    /// One digest over every part.
    pub digest: String,
    /// Project-relative part (`package.json`, `dir:src/domain`) -> its digest.
    pub parts: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Removed,
    Modified,
}

/// One relevant change in the project since the Harness was analysed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelevantChange {
    /// A file, or a folder (`src/domain`, without the internal `dir:` marker).
    pub path: String,
    pub kind: ChangeKind,
}

/// The project changed in relevant ways since the Harness was analysed. Nothing is deleted
/// because of it: the knowledge stays, and says it may be outdated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Staleness {
    pub analyzed_at: u64,
    /// At most [`MAX_LISTED_CHANGES`], in path order.
    pub changes: Vec<RelevantChange>,
    /// How many changes there are in all.
    pub total_changes: usize,
}

/// How many changes a [`Staleness`] lists.
pub const MAX_LISTED_CHANGES: usize = 20;

/// What is missing from the Harness, and why it is missing. The two kinds are never the same
/// statement: *not found* is a bounded search that came back empty; *unknown* is the absence of
/// any search that could tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GapKind {
    /// Atlas looked, in the stated scope, and did not find it. Not proof that it does not exist.
    NotFound,
    /// Nothing available could establish it.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Gap {
    pub kind: GapKind,
    /// What it is about, as a stable key (`database`, `commands`, `business_rules`).
    pub subject: String,
    /// One honest sentence.
    pub statement: String,
}

/// What a Harness holds, counted for a glance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeStats {
    pub findings: usize,
    pub verified: usize,
    pub inferred: usize,
    pub user: usize,
    pub stale: usize,
    pub unknown: usize,
}

/// One side of a [`Conflict`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Claim {
    /// How the claim reads to a person (`Angular 21`).
    pub value: String,
    /// The value that would be stored if the user chose this side (`21`).
    pub choice: String,
    pub origin: Origin,
    pub evidence: Vec<Evidence>,
}

/// Two sources that disagree. Atlas records it; it never picks a side silently.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    /// The finding the disagreement is about; a user correction for this id resolves it.
    pub finding_id: String,
    pub label: String,
    pub claims: Vec<Claim>,
    /// The value the user chose. The user's decision has precedence.
    pub resolution: Option<String>,
}

/// `.atlas/knowledge/findings.yaml`: what the analysis found, before the user's choices are
/// applied. Generated: replaced on update (after the user reviewed the diff).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessKnowledge {
    pub version: u32,
    pub analysis: AnalysisInfo,
    pub findings: Vec<Finding>,
    #[serde(default)]
    pub conflicts: Vec<Conflict>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthState {
    /// Confirmed and consistent.
    Healthy,
    /// Relevant inferences nobody confirmed yet, or a Harness from before knowledge existed.
    NeedsReview,
    /// Part of the project could not be analysed.
    Partial,
    /// Evidence contradicts itself and nobody decided.
    Conflicted,
    /// The project changed in relevant ways since the analysis (see [`Staleness`]).
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessHealth {
    pub state: HealthState,
    /// Machine-readable reasons (`project_changed`, `unresolved_conflicts`, `partial_analysis`,
    /// `unconfirmed_inferences`, `legacy_harness`, `no_fingerprint`) for the UI to word.
    pub reasons: Vec<String>,
}

/// A change between the Harness's stored knowledge and a new analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindingChange {
    pub id: String,
    pub label: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessDiff {
    pub added: Vec<FindingChange>,
    pub removed: Vec<FindingChange>,
    pub changed: Vec<FindingChange>,
    pub unchanged: Vec<String>,
}

/// What the user wrote about the project. Never produced or corrected by Atlas.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserKnowledge {
    pub purpose: String,
    pub users: String,
    pub concepts: String,
    pub business_rules: String,
    pub constraints: String,
    /// Decisions to preserve, e.g. "this architecture is intentional; do not migrate it".
    pub decisions: String,
}

/// What the deterministic (and, if asked, semantic) analysis found. Reading only: nothing was
/// run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAnalysis {
    pub project_name: String,
    pub path: String,
    pub findings: Vec<Finding>,
    pub conflicts: Vec<Conflict>,
    /// What is not known, and what was looked for and not found.
    pub gaps: Vec<Gap>,
    /// The scan or the sampling hit a limit and did not see everything.
    pub partial: bool,
    pub scanned_entries: usize,
    pub analysis: AnalysisInfo,
    /// A Harness already in the project, if any, so the UI can offer what to do with it.
    pub existing: Option<HarnessSummary>,
    /// How the new analysis differs from the existing Harness's knowledge.
    pub diff: Option<HarnessDiff>,
    /// What the user corrected, confirmed or excluded last time, so a review can start from it.
    pub previous_corrections: BTreeMap<String, String>,
    pub previous_excluded: Vec<String>,
    pub previous_confirmed: Vec<String>,
    /// The user's own knowledge as it is in `.atlas/` now.
    pub user: UserKnowledge,
    /// What a model drafted from the README and docs for the fields the user has not filled.
    /// Only a suggestion: it becomes the user's knowledge when they confirm it in the review.
    pub suggested_user: UserKnowledge,
    /// User files Atlas cannot round-trip (hand-edited); it will not rewrite them.
    pub unmanaged_user_files: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessStatus {
    NotInitialized,
    Initialized,
    /// `.atlas/` is there but cannot be trusted as it is (unreadable or newer manifest).
    NeedsReview,
}

/// The Harness of a workspace's project, in brief.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessSummary {
    pub status: HarnessStatus,
    pub project_name: Option<String>,
    /// Stack labels as the manifest records them, e.g. `Angular 21`.
    pub stack: Vec<String>,
    pub version: Option<u32>,
    pub initialized_at: Option<u64>,
    /// `.atlas/` exists in the project (with or without a valid manifest).
    pub has_atlas_dir: bool,
    /// An `ErrorCode` name (`snake_case`) when the status is `needs_review`.
    pub problem: Option<String>,
    /// How far the Harness can be trusted. `None` when there is no valid Harness.
    pub health: Option<HarnessHealth>,
    /// When the project last analysed (ms since the epoch), if the knowledge says.
    #[serde(default)]
    pub analyzed_at: Option<u64>,
    /// Present when the project changed in relevant ways since the analysis.
    #[serde(default)]
    pub staleness: Option<Staleness>,
    /// Counts of what the Harness holds (verified, inferred, ...). `None` without knowledge.
    #[serde(default)]
    pub stats: Option<KnowledgeStats>,
}

impl HarnessSummary {
    pub fn not_initialized(has_atlas_dir: bool) -> Self {
        Self {
            status: HarnessStatus::NotInitialized,
            project_name: None,
            stack: Vec::new(),
            version: None,
            initialized_at: None,
            has_atlas_dir,
            problem: None,
            health: None,
            analyzed_at: None,
            staleness: None,
            stats: None,
        }
    }
}

/// What to do about a Harness that may already exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InitMode {
    /// First initialization; refuses to run over an existing manifest.
    #[default]
    Create,
    /// Keep what is there; only checks it.
    UseExisting,
    /// Regenerate what Atlas generates (after the user reviewed the diff), backing up whatever it
    /// replaces. The user's own files are never rewritten unless the user changed them.
    UpdateExisting,
}

/// Which agent's model may be asked to read the selected evidence, and nothing else.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticRequest {
    pub agent_id: String,
    /// What the user wants the analysis to know or focus on ("this is an ERP; look at the API").
    #[serde(default)]
    pub instructions: String,
    /// Only show the model a selected set of files, with every tool off (runtimes that can).
    /// By default the agent explores the project with its own read tools.
    #[serde(default)]
    pub restricted: bool,
}

/// What the user decided in the review step.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeInput {
    #[serde(default)]
    pub mode: InitMode,
    /// Finding ids the user unticked.
    #[serde(default)]
    pub excluded: Vec<String>,
    /// Finding id -> value the user says is right. Also how a conflict is resolved.
    #[serde(default)]
    pub corrections: BTreeMap<String, String>,
    /// Inferences the user explicitly confirmed.
    #[serde(default)]
    pub confirmed: Vec<String>,
    #[serde(default)]
    pub purpose: String,
    #[serde(default)]
    pub users: String,
    #[serde(default)]
    pub concepts: String,
    #[serde(default)]
    pub business_rules: String,
    #[serde(default)]
    pub constraints: String,
    #[serde(default)]
    pub decisions: String,
    /// Keep `.atlas/` out of Git: add it to the project's `.gitignore`. On unless the user says
    /// otherwise.
    #[serde(default = "yes")]
    pub ignore_in_git: bool,
}

fn yes() -> bool {
    true
}

/// What was done about keeping `.atlas/` out of Git.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GitIgnoreStatus {
    /// Not asked for.
    #[default]
    Skipped,
    /// `.atlas/` was added to the project's `.gitignore`.
    Added,
    /// The `.gitignore` already ignored it: nothing was changed.
    AlreadyIgnored,
    /// The project is not in a Git repository: there is nothing to ignore it from.
    NoRepository,
    /// The `.gitignore` could not be changed. The Harness itself was written all the same.
    Failed,
}

impl InitializeInput {
    pub fn user_knowledge(&self) -> UserKnowledge {
        UserKnowledge {
            purpose: self.purpose.clone(),
            users: self.users.clone(),
            concepts: self.concepts.clone(),
            business_rules: self.business_rules.clone(),
            constraints: self.constraints.clone(),
            decisions: self.decisions.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeOutcome {
    pub summary: HarnessSummary,
    /// Paths relative to `.atlas/`.
    pub written: Vec<String>,
    pub backed_up: Vec<String>,
    /// User files left as they are because Atlas cannot rewrite them without losing text.
    pub left_untouched: Vec<String>,
    pub git_ignore: GitIgnoreStatus,
}

/// What a refresh would change. Nothing is applied until the user confirms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshOutcome {
    /// Why the Harness is stale, when it is.
    pub staleness: Option<Staleness>,
    pub diff: HarnessDiff,
    pub conflicts: Vec<Conflict>,
    pub applied: Option<InitializeOutcome>,
}

/// `.atlas/project.yaml`: structural metadata and the user's decisions about the findings.
/// Knowledge for humans lives in `context/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessManifest {
    pub version: u32,
    pub project: ManifestProject,
    pub repository: ManifestRepository,
    pub stack: ManifestStack,
    pub context: ManifestContext,
    pub harness: ManifestHarness,
    /// What the user corrected, kept so a refresh does not undo it.
    #[serde(default)]
    pub corrections: BTreeMap<String, String>,
    /// What the user said does not apply.
    #[serde(default)]
    pub excluded: Vec<String>,
    /// Inferences the user confirmed.
    #[serde(default)]
    pub confirmed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestProject {
    pub id: String,
    pub name: String,
    pub initialized_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestRepository {
    #[serde(rename = "type")]
    pub kind: String,
    /// Always relative to the manifest's project (`.`), so the file is the same on every machine.
    pub root: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestStack {
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub frameworks: Vec<String>,
    #[serde(default)]
    pub runtimes: Vec<String>,
    #[serde(default)]
    pub databases: Vec<String>,
    #[serde(default)]
    pub infrastructure: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestContext {
    pub generated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestHarness {
    pub version: u32,
}

/// A file to write under `.atlas/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessFile {
    /// Relative to `.atlas/`, with `/` separators.
    pub path: String,
    pub content: String,
    /// Leave an existing file alone (it may hold the user's own words).
    pub keep_if_exists: bool,
}

/// What a store did when asked to write files.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WriteReport {
    pub written: Vec<String>,
    pub skipped: Vec<String>,
    pub backed_up: Vec<String>,
}

/// `angular`, `18` -> `Angular 18`. A value of `true`/`possible`/`unknown` adds nothing.
pub fn label_for(key: &str, value: &str) -> String {
    let name = match key {
        "nodejs" => "Node.js",
        "typescript" => "TypeScript",
        "javascript" => "JavaScript",
        "nextjs" => "Next.js",
        "nestjs" => "NestJS",
        "csharp" => "C#",
        "fsharp" => "F#",
        "dotnet" => ".NET",
        "aspnet" => "ASP.NET Core",
        "jvm" => "JVM",
        "spring_boot" => "Spring Boot",
        "postgresql" => "PostgreSQL",
        "sqlserver" => "SQL Server",
        "mysql" => "MySQL",
        "mariadb" => "MariaDB",
        "mongodb" => "MongoDB",
        "sqlite" => "SQLite",
        "github_actions" => "GitHub Actions",
        "gitlab_ci" => "GitLab CI",
        "azure_pipelines" => "Azure Pipelines",
        "circleci" => "CircleCI",
        "docker_compose" => "Docker Compose",
        "go_modules" => "Go modules",
        "go_test" => "go test",
        "cargo_test" => "cargo test",
        "env_file" => ".env file",
        "msbuild_props" => "Directory.Build.props",
        "xunit" => "xUnit",
        "nunit" => "NUnit",
        "mstest" => "MSTest",
        "junit" => "JUnit",
        "eslint" => "ESLint",
        "editorconfig" => "EditorConfig",
        "rustfmt" => "rustfmt",
        "npm" | "pnpm" | "pytest" | "cargo" | "pip" | "bun" => key,
        other => return with_version(&humanise(other), value),
    };
    with_version(name, value)
}

fn capitalise(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

pub fn humanise(key: &str) -> String {
    capitalise(&key.replace('_', " "))
}

fn with_version(name: &str, value: &str) -> String {
    match value {
        "true" | "possible" | "unknown" | "" => name.to_owned(),
        v => format!("{name} {v}"),
    }
}
