//! Guardrails and Context Review: the vocabulary.
//!
//! Atlas already has one security authority (`application::security`: the policy, the permission
//! guard, the audit trail). A guardrail decision is **that** authority's answer put in one shape:
//! `ALLOW`, `ASK`, `DENY` or `TRANSFORM`. [`PermissionDecision`] is the same answer without
//! `TRANSFORM`; every evaluation here is recorded as a [`PermissionEvent`], so there is no second
//! audit trail and no second approval system (an `ASK` is a `PendingInteraction`).
//!
//! Context Review is not a security decision. It answers "is the context this agent is about to
//! receive whole, coherent and safe to send?" It reports; the guardrail decides what that means.
//!
//! Nothing the model, a skill, a handoff or a project file says can appear here as authority.

use serde::{Deserialize, Serialize};

use super::optimization::SectionKind;
use super::security::{
    PermissionAction, PermissionDecision, PermissionEvent, PermissionOutcome, Reason,
};

/// The four answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardrailDecision {
    Allow,
    /// May continue only after a person says so.
    Ask,
    /// Never happens.
    Deny,
    /// Happens, changed by a deterministic, safe rule (secrets redacted). Never a rewritten
    /// command: there is no such transformation.
    Transform,
}

impl From<PermissionDecision> for GuardrailDecision {
    fn from(decision: PermissionDecision) -> Self {
        match decision {
            PermissionDecision::Allowed => Self::Allow,
            PermissionDecision::RequiresApproval => Self::Ask,
            PermissionDecision::Denied => Self::Deny,
        }
    }
}

/// Where in an execution the question was asked. Only the moments Atlas really controls: the
/// runtimes run their own tools inside their process, so Atlas cannot stop one mid-action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
// The moments are named for when they happen, relative to what they guard.
#[allow(clippy::enum_variant_names)]
pub enum GuardrailStage {
    /// Before the runtime is started: what it will be given and what it may do.
    BeforeAgent,
    /// Before a step's result is handed to the next one.
    BeforeHandoff,
    /// Before a run's changes enter the project's working tree.
    BeforeApply,
}

/// What a person answered to an `ASK`, as the workflow run kept it (an answered
/// `PendingInteraction`). The only way an `ASK` is approved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answered {
    Allowed,
    Declined,
}

/// A person's answer together with what it answers: the fingerprint of the evaluation the
/// question was asked about. An answer approves (or declines) that evaluation and nothing else;
/// when the context, the policy or anything the evaluation looked at has changed, the
/// fingerprint no longer matches and the answer counts for nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewAnswer {
    pub answered: Answered,
    pub evaluation: String,
}

/// One evaluation, explainable and auditable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardrailEvaluation {
    pub stage: GuardrailStage,
    /// What was evaluated, as a stable name (`launch_context`, `edit_files`, `handoff_content`).
    pub action: String,
    pub decision: GuardrailDecision,
    /// Set when the answer is not a plain allow: the code the UI words.
    pub reason: Option<Reason>,
    /// The rule that decided, `area.name` (`context.invalid`, `write.not_isolated`).
    pub rule: String,
    /// Every rule that matched (the review's issue codes, for a context decision).
    pub matched_rules: Vec<String>,
    pub requires_approval: bool,
    /// What was changed, for `TRANSFORM` (a description, never the content).
    pub transformation: Option<String>,
    /// A person decided it (approving or declining an earlier `ASK`).
    pub decided_by_user: bool,
    /// What it concerned, for display: a section, a path class. Never content.
    pub target: String,
}

impl GuardrailEvaluation {
    pub fn new(
        stage: GuardrailStage,
        action: &str,
        decision: GuardrailDecision,
        rule: &str,
        target: &str,
    ) -> Self {
        Self {
            stage,
            action: action.to_owned(),
            decision,
            reason: None,
            rule: rule.to_owned(),
            matched_rules: Vec::new(),
            requires_approval: decision == GuardrailDecision::Ask,
            transformation: None,
            decided_by_user: false,
            target: target.to_owned(),
        }
    }

    #[must_use]
    pub fn because(mut self, reason: Reason) -> Self {
        self.reason = Some(reason);
        self
    }

    #[must_use]
    pub fn matching(mut self, rules: impl IntoIterator<Item = String>) -> Self {
        self.matched_rules = rules.into_iter().collect();
        self
    }

    /// The audit-trail entry for this evaluation: the same record every security decision
    /// leaves. Rule and transformation travel as notes; content never does.
    pub fn to_event(&self, scope: &AuditScope, timestamp: u64) -> PermissionEvent {
        let (decision, source) = match (self.decision, self.decided_by_user) {
            (GuardrailDecision::Allow, true) => {
                (PermissionOutcome::Approved, DecisionSourceOf::User)
            }
            (GuardrailDecision::Deny, true) => {
                (PermissionOutcome::Rejected, DecisionSourceOf::User)
            }
            (GuardrailDecision::Allow, false) => {
                (PermissionOutcome::Allowed, DecisionSourceOf::Policy)
            }
            (GuardrailDecision::Deny, false) => {
                (PermissionOutcome::Denied, DecisionSourceOf::Policy)
            }
            (GuardrailDecision::Ask, _) => (
                PermissionOutcome::ApprovalRequested,
                DecisionSourceOf::Policy,
            ),
            (GuardrailDecision::Transform, _) => {
                (PermissionOutcome::Transformed, DecisionSourceOf::Policy)
            }
        };
        let mut notes = vec![format!("rule:{}", self.rule)];
        notes.extend(self.matched_rules.iter().map(|r| format!("matched:{r}")));
        if let Some(what) = &self.transformation {
            notes.push(format!("transformed:{what}"));
        }
        PermissionEvent {
            timestamp,
            execution_id: scope.execution_id.clone(),
            workspace_id: scope.workspace_id.clone(),
            task_id: scope.task_id.clone(),
            agent_id: scope.agent_id.clone(),
            action: match self.action.as_str() {
                "launch_context" => PermissionAction::ReviewContext,
                "edit_files" => PermissionAction::EditFiles,
                _ => PermissionAction::ShareResult,
            },
            target: self.target.clone(),
            cwd: None,
            decision,
            source,
            reason: self.reason,
            approval_id: None,
            notes,
        }
    }
}

type DecisionSourceOf = super::security::DecisionSource;

/// Whose execution an evaluation belongs to (the ids the audit trail is keyed by).
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_field_names)]
pub struct AuditScope {
    pub execution_id: String,
    pub workspace_id: String,
    pub task_id: String,
    pub agent_id: String,
}

/// What the guardrails did during one execution. Counted from the evaluations, so the numbers
/// are the audit trail's, not a second tally.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardrailMetrics {
    pub evaluations: u32,
    pub allowed: u32,
    pub asked: u32,
    pub denied: u32,
    pub transformed: u32,
    /// Denials that stopped the execution before its runtime started.
    pub blocked: u32,
}

impl GuardrailMetrics {
    pub fn of(evaluations: &[GuardrailEvaluation], blocked: bool) -> Self {
        let count = |d: GuardrailDecision| {
            u32::try_from(evaluations.iter().filter(|e| e.decision == d).count())
                .unwrap_or(u32::MAX)
        };
        Self {
            evaluations: u32::try_from(evaluations.len()).unwrap_or(u32::MAX),
            allowed: count(GuardrailDecision::Allow),
            asked: count(GuardrailDecision::Ask),
            denied: count(GuardrailDecision::Deny),
            transformed: count(GuardrailDecision::Transform),
            blocked: u32::from(blocked),
        }
    }
}

// ---- Context Review ----

/// How far Atlas lets a piece of context speak for itself. Authority never comes from here: it
/// only says how much to suspect text that claims some.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceTrust {
    /// Atlas's own rules and protocols.
    Atlas,
    /// What the user configured or asked: the task, the agent's instructions, the workspace.
    Configured,
    /// Derived from files and from other agents: the Harness, skills, the handoff and the
    /// workflow state. Context, never authority.
    Untrusted,
}

impl SourceTrust {
    pub fn of(source: SectionKind) -> Self {
        match source {
            SectionKind::Personality
            | SectionKind::AtlasRules
            | SectionKind::LiveNarration
            | SectionKind::PlanRule
            | SectionKind::BriefProtocols
            | SectionKind::Framing => Self::Atlas,
            SectionKind::ProjectContext | SectionKind::AgentInstructions | SectionKind::Task => {
                Self::Configured
            }
            SectionKind::Harness
            | SectionKind::TaskContext
            | SectionKind::Skills
            | SectionKind::BriefWorkflowContext
            | SectionKind::BriefHandoff => Self::Untrusted,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueSeverity {
    /// Worth knowing; says nothing is wrong.
    Info,
    Warning,
    Error,
    /// The execution must not start.
    Blocking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueCode {
    /// A part the step cannot do without is not there.
    MissingRequired,
    /// Required context was changed by something other than its owner.
    AlteredRequired,
    /// Two sources say incompatible things.
    ConflictingInstructions,
    /// Project knowledge the project has changed since.
    StaleContext,
    /// The context says the same thing twice (efficiency, not validity).
    DuplicatedContext,
    /// The required parts alone do not fit the budget.
    BudgetExceeded,
    /// Context was left out or trimmed to fit.
    ContextTrimmed,
    /// Skills were loaded with quality problems.
    SkillIssues,
    /// Something that looks like a secret is in context that is about to be sent.
    SecretInContext,
    /// Untrusted text claims authority it cannot have.
    AuthorityClaim,
}

impl IssueCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MissingRequired => "missing_required",
            Self::AlteredRequired => "altered_required",
            Self::ConflictingInstructions => "conflicting_instructions",
            Self::StaleContext => "stale_context",
            Self::DuplicatedContext => "duplicated_context",
            Self::BudgetExceeded => "budget_exceeded",
            Self::ContextTrimmed => "context_trimmed",
            Self::SkillIssues => "skill_issues",
            Self::SecretInContext => "secret_in_context",
            Self::AuthorityClaim => "authority_claim",
        }
    }
}

/// One finding of the review, with where it is (and, for a conflict, where the other side is).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewIssue {
    pub code: IssueCode,
    pub severity: IssueSeverity,
    pub source: SectionKind,
    /// The other side of a conflict.
    pub other_source: Option<SectionKind>,
    /// A short English description for logs; the UI words the code itself.
    pub message: String,
    /// The start of the line concerned (never a whole text, and never a secret: the review
    /// redacts before it quotes).
    pub excerpt: String,
}

/// Whether the context is fit to send.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextHealth {
    /// Nothing but information.
    Healthy,
    /// Warnings: usable, with something worth knowing (stale, trimmed, suspicious text).
    Partial,
    /// An error: a person should look (a conflict, a budget that cannot be met).
    NeedsReview,
    /// A blocking issue: it must not be sent.
    Invalid,
}

/// How much each source contributes, for the inspector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSummary {
    pub source: SectionKind,
    pub trust: SourceTrust,
    pub estimated_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextReviewResult {
    pub health: ContextHealth,
    pub issues: Vec<ReviewIssue>,
    pub sources: Vec<SourceSummary>,
    /// Items by priority, as the Context Engine ranks them.
    pub required_items: u32,
    pub high_items: u32,
    pub normal_items: u32,
    pub optional_items: u32,
    /// Harness items the project has changed under since it was analysed.
    pub stale_items: u32,
}

impl ContextReviewResult {
    pub fn count(&self, severity: IssueSeverity) -> u32 {
        u32::try_from(
            self.issues
                .iter()
                .filter(|i| i.severity == severity)
                .count(),
        )
        .unwrap_or(u32::MAX)
    }

    #[cfg(test)]
    pub fn has(&self, code: IssueCode) -> bool {
        self.issues.iter().any(|i| i.code == code)
    }
}

/// The review in numbers, for the metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextReviewMetrics {
    pub health: ContextHealth,
    pub warnings: u32,
    pub errors: u32,
    pub blocking_issues: u32,
    pub stale_items: u32,
}

impl From<&ContextReviewResult> for ContextReviewMetrics {
    fn from(result: &ContextReviewResult) -> Self {
        Self {
            health: result.health,
            warnings: result.count(IssueSeverity::Warning),
            errors: result.count(IssueSeverity::Error),
            blocking_issues: result.count(IssueSeverity::Blocking),
            stale_items: result.stale_items,
        }
    }
}

// ---- the review of a ChangeSet, before it can be applied (phase 5.1) ----

/// Whether what a run changed may go into the project's working tree.
///
/// `Healthy` → `ALLOW`, `NeedsReview` → `ASK`, `Invalid` → `DENY`. None of them applies anything:
/// Apply is always a person's act, and never commits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeSetHealth {
    Healthy,
    /// Something a person should look at before it enters the project (a secret, a key file).
    NeedsReview,
    /// Must not enter the project, whatever anyone says (Git's own files, paths that leave it).
    Invalid,
}

impl ChangeSetHealth {
    pub fn decision(self) -> GuardrailDecision {
        match self {
            Self::Healthy => GuardrailDecision::Allow,
            Self::NeedsReview => GuardrailDecision::Ask,
            Self::Invalid => GuardrailDecision::Deny,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeIssueCode {
    /// Touches `.git` (any component, any depth).
    ProtectedGit,
    /// Touches `.atlas`: Atlas's own state and configuration in the project.
    ProtectedAtlas,
    /// An absolute path, or one that climbs out of the project (`../`).
    EscapesProject,
    /// A file whose real location, following links, is outside the project.
    SymlinkEscape,
    /// `.env`, credentials, private keys.
    SensitiveFile,
    /// An added line looks like a secret (the Harness's scanner).
    SecretInContent,
    /// Could not be checked (a link that does not resolve, a file that cannot be read).
    Unverifiable,
}

impl ChangeIssueCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProtectedGit => "protected_git",
            Self::ProtectedAtlas => "protected_atlas",
            Self::EscapesProject => "escapes_project",
            Self::SymlinkEscape => "symlink_escape",
            Self::SensitiveFile => "sensitive_file",
            Self::SecretInContent => "secret_in_content",
            Self::Unverifiable => "unverifiable",
        }
    }

    pub fn health(self) -> ChangeSetHealth {
        match self {
            Self::ProtectedGit | Self::EscapesProject | Self::SymlinkEscape => {
                ChangeSetHealth::Invalid
            }
            Self::ProtectedAtlas
            | Self::SensitiveFile
            | Self::SecretInContent
            | Self::Unverifiable => ChangeSetHealth::NeedsReview,
        }
    }
}

/// One finding. Names a path and a code; never carries file content.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeIssue {
    pub code: ChangeIssueCode,
    pub path: String,
}

/// The deterministic review of a `ChangeSet`: every file is looked at, none is skipped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeSetReview {
    pub health: ChangeSetHealth,
    pub issues: Vec<ChangeIssue>,
    pub files_reviewed: u32,
    /// What a person's acknowledgement is bound to: the revisions, the files and the findings.
    /// Any change to them is a different review.
    pub fingerprint: String,
}

impl ChangeSetReview {
    /// The findings as one line, for the run's record and the screen: codes and paths, no content.
    pub fn summary(&self) -> String {
        self.issues
            .iter()
            .map(|i| format!("{}: {}", i.code.as_str(), i.path))
            .collect::<Vec<_>>()
            .join("; ")
    }
}
