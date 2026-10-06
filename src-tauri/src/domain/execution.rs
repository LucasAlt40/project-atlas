use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::interaction::InteractionDetection;
use super::optimization::OptimizationMetrics;
use super::security::PermissionEvent;
use super::task::Task;
use super::task_context::ContextRecord;
use super::usage::{QuotaInfo, UsageMetrics};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Running,
    /// The agent needs a person before it can go on. Not completed (it produced no result) and
    /// not failed (nothing went wrong): the step is paused, and an answer starts it again.
    WaitingForInput,
    Completed,
    Failed,
    /// The user stopped it (interrupt or terminate). Not a failure: nothing went wrong.
    /// A timeout stays a `Failed` execution with `FailureKind::Timeout`.
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    RuntimeNotInstalled,
    RuntimeUnavailable,
    AuthenticationRequired,
    ModelUnavailable,
    /// The provider refused for now: too many requests or the free allowance is used up.
    RateLimited,
    Timeout,
    ExecutionFailed,
    InvalidRequest,
    UnexpectedResponse,
    /// Atlas's security policy refused to start the runtime.
    PermissionDenied,
    /// Only used to word the assistant message of a cancelled execution; a cancelled
    /// execution itself has no failure.
    Cancelled,
    /// Atlas was closed while the execution ran; recorded when it starts again.
    AppClosed,
    /// The agent asks for Git worktree isolation and the project is not a Git repository.
    GitRepositoryRequired,
    /// The worktree could not be created (no base branch, Git failed, a name was taken…).
    WorktreeFailed,
}

impl FailureKind {
    /// The wire name (same as the serialized form).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RuntimeNotInstalled => "runtime_not_installed",
            Self::RuntimeUnavailable => "runtime_unavailable",
            Self::AuthenticationRequired => "authentication_required",
            Self::ModelUnavailable => "model_unavailable",
            Self::RateLimited => "rate_limited",
            Self::Timeout => "timeout",
            Self::ExecutionFailed => "execution_failed",
            Self::InvalidRequest => "invalid_request",
            Self::UnexpectedResponse => "unexpected_response",
            Self::PermissionDenied => "permission_denied",
            Self::Cancelled => "cancelled",
            Self::AppClosed => "app_closed",
            Self::GitRepositoryRequired => "git_repository_required",
            Self::WorktreeFailed => "worktree_failed",
        }
    }
}

/// Why an execution failed: a message safe to show, plus optional technical details.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionFailure {
    pub kind: FailureKind,
    pub message: String,
    pub details: Option<String>,
}

/// One attempt to perform a [`Task`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Execution {
    pub id: String,
    pub workspace_id: String,
    pub task_id: String,
    pub status: ExecutionStatus,
    /// Milliseconds since the Unix epoch.
    pub started_at: u64,
    pub completed_at: Option<u64>,
    pub logs: Vec<String>,
    /// The runtime and model that ran it, as chosen when it started.
    pub runtime_id: String,
    pub model_id: String,
    /// The exact prompt sent to the model.
    pub prompt: String,
    pub result: Option<String>,
    pub failure: Option<ExecutionFailure>,
    /// Runtime-reported facts about the run (duration, cost…), already normalized to
    /// text. Which keys exist depends on the runtime.
    pub metadata: BTreeMap<String, String>,
    /// What the runtime reported it consumed; `None` if it reported nothing.
    pub usage: Option<UsageMetrics>,
    /// Provider quota the runtime reported alongside, if any.
    pub quota: Option<QuotaInfo>,
    /// Every permission decision made for this execution, in order: the security audit trail.
    pub permission_events: Vec<PermissionEvent>,
    /// How the Harness context of the prompt was chosen. `None` when the project has no Harness.
    pub context: Option<ContextRecord>,
    /// What Atlas observed about the prompt and the run. `None` when metrics are off.
    pub optimization: Option<OptimizationMetrics>,
    /// What the execution is waiting for, while it is `WaitingForInput`.
    pub interaction: Option<InteractionDetection>,
}

impl Execution {
    pub fn start(
        id: String,
        workspace_id: String,
        task_id: String,
        runtime_id: String,
        model_id: String,
        prompt: String,
        started_at: u64,
    ) -> Self {
        Self {
            id,
            workspace_id,
            task_id,
            runtime_id,
            model_id,
            metadata: BTreeMap::new(),
            usage: None,
            quota: None,
            permission_events: Vec::new(),
            context: None,
            interaction: None,
            optimization: None,
            status: ExecutionStatus::Running,
            started_at,
            completed_at: None,
            logs: Vec::new(),
            prompt,
            result: None,
            failure: None,
        }
    }

    pub fn complete(
        &mut self,
        result: String,
        metadata: BTreeMap<String, String>,
        usage: Option<UsageMetrics>,
        quota: Option<QuotaInfo>,
        at: u64,
    ) {
        self.status = ExecutionStatus::Completed;
        self.completed_at = Some(at);
        self.result = Some(result);
        self.metadata = metadata;
        self.usage = usage;
        self.quota = quota;
    }

    /// The runtime answered, but the answer is a request for a person rather than a result. The
    /// answer is kept (it is what the person is replying to); no result exists yet.
    pub fn wait_for_input(
        &mut self,
        text: String,
        interaction: InteractionDetection,
        metadata: BTreeMap<String, String>,
        usage: Option<UsageMetrics>,
        quota: Option<QuotaInfo>,
        at: u64,
    ) {
        self.status = ExecutionStatus::WaitingForInput;
        self.completed_at = Some(at);
        self.result = Some(text);
        self.interaction = Some(interaction);
        self.metadata = metadata;
        self.usage = usage;
        self.quota = quota;
    }

    pub fn cancel(&mut self, at: u64) {
        self.status = ExecutionStatus::Cancelled;
        self.completed_at = Some(at);
    }

    pub fn fail(&mut self, failure: ExecutionFailure, at: u64) {
        self.status = ExecutionStatus::Failed;
        self.completed_at = Some(at);
        self.failure = Some(failure);
    }
}

/// A task together with its execution.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExecutionRecord {
    pub task: Task,
    pub execution: Execution,
}

/// What happened during an execution, in the order it happens. Only events a runtime can
/// really report are emitted: `OutputChunk` (the `message` is a piece of the live answer) and
/// the `Tool*` kinds appear only for runtimes whose tool streams them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionEventKind {
    Started,
    StartingRuntime,
    SendingPrompt,
    WaitingForModel,
    OutputChunk,
    ToolStarted,
    ToolCompleted,
    /// A permission decision: the `metadata` carries `decision`, `action`, `target`, `source`
    /// and, when there is one, `reason`, `cwd` and `approvalId`.
    Permission,
    Completed,
    Failed,
    /// The agent stopped to ask a person: `metadata` has `interactionKind`, `source` and
    /// `confidence`; the question is the event's `message`.
    InteractionDetected,
    /// The process behind the execution got a terminal: `metadata` has `processSessionId`.
    TerminalConnected,
    /// The user asked the process to stop (Ctrl+C). A user action, not an agent action.
    UserInterrupted,
    /// The user forced the process to end.
    UserTerminated,
    /// The process ended: `metadata` has `exitCode` (when known) and `durationMs`.
    ProcessExited,
    /// The execution ended because the user stopped it.
    Cancelled,
    /// The execution's isolated worktree exists: `metadata` has `baseBranch`, `branch` and
    /// `worktreePath`.
    WorktreeCreated,
    /// The work of the execution was examined (and merged, if allowed): `metadata` has
    /// `worktreeStatus`, `mergeStatus` and, when known, `filesChanged`, `blockReason` and
    /// `recommendation`.
    WorktreeFinalized,
    /// The Harness part of the prompt was settled (or there is none): `metadata` has `buildMs`
    /// and, with a Harness, `selectedItems`, `omittedItems`. Observability only.
    OptimizationContextBuilt,
    /// The prompt was assembled: `metadata` has `totalBytes`, `estimatedTokens`, `tokenSource`
    /// (always `estimated`: Atlas has no tokenizer) and `buildMs`. Observability only.
    OptimizationPromptBuilt,
    /// The execution's metrics were saved with it: `metadata` has `totalMs` and, when the runtime
    /// reported them, `runtimeInputTokens` and `runtimeOutputTokens`. Observability only.
    OptimizationMetricsRecorded,
    /// The Context Engine reworked the prompt's inputs: `metadata` has `deduplicatedLines`,
    /// `compressedItems`, `omittedItems`, `savedBytes`, `rawEstimatedTokens`,
    /// `finalEstimatedTokens` and `tokenSource` (always `estimated`).
    OptimizationContextOptimized,
    /// The prompt is over its budget after everything that may be shortened was: `metadata` has
    /// `budgetTokens`, `estimatedTokens` and `requiredTokens`. Nothing was cut to force it.
    OptimizationBudgetWarning,
    /// The skills layer looked at the task: `metadata` has `discovered`, `usable`, `activated`
    /// (names, comma separated), `reasons`, `level2Tokens`, `level3Tokens`, `cacheHits`,
    /// `cacheMisses` and `tokenSource` (always `estimated`).
    OptimizationSkillsSelected,
    /// The context an agent is about to receive was reviewed: `metadata` has `health`,
    /// `warnings`, `errors`, `blockingIssues`, `staleItems`.
    OptimizationContextReviewed,
    /// The review found the context not fit to send and the execution did not start: `metadata`
    /// has `rule`, `reason` and `matched` (the issue codes).
    OptimizationContextReviewBlocked,
    /// A guardrail asked a person before the agent starts: `metadata` has `rule`, `reason` and
    /// `matched`. The answer comes through the workflow's pending interaction.
    OptimizationGuardrailAsked,
    /// A guardrail refused something: `metadata` has `action`, `rule` and `reason`.
    OptimizationGuardrailDenied,
    /// Everything the guardrails decided for this execution, counted: `evaluations`, `allowed`,
    /// `asked`, `denied`, `transformed`, `blocked`.
    OptimizationGuardrailEvaluated,
}

/// Progress notification emitted while an execution runs. Carries both ids so a listener
/// can keep several agents' executions apart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionEvent {
    pub execution_id: String,
    pub workspace_id: String,
    pub task_id: String,
    pub agent_id: String,
    pub kind: ExecutionEventKind,
    /// English description for logs. The UI builds its own localized text from `kind` and
    /// `metadata`, except for `output_chunk`, whose `message` is the streamed answer text.
    pub message: String,
    /// Milliseconds since the Unix epoch.
    pub timestamp: u64,
    /// Extra facts: `runtime` / `model` / `tool` names for the matching steps, `failureKind` on
    /// failure, runtime-reported values on completion.
    pub metadata: BTreeMap<String, String>,
}

/// What Atlas keeps of an execution after it ends: enough to list it and open it again. The
/// answer lives in the conversation (the assistant message with the same `execution_id`) and
/// the raw terminal output is never kept (ADR 0007), so neither is repeated here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredExecution {
    pub id: String,
    pub workspace_id: String,
    pub agent_id: String,
    pub status: ExecutionStatus,
    /// What the user asked.
    pub task: String,
    pub started_at: u64,
    pub completed_at: Option<u64>,
    pub runtime_id: String,
    pub model_id: String,
    pub failure: Option<ExecutionFailure>,
    pub metadata: BTreeMap<String, String>,
    pub usage: Option<UsageMetrics>,
    /// What the execution asked, when it ended waiting for a person.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interaction: Option<InteractionDetection>,
    /// How the Harness context of the prompt was chosen (mode and counts, not the text, which is
    /// the prompt). Absent for executions from before V0.8 and for projects without a Harness.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<ContextRecord>,
    /// What Atlas observed about the prompt and the run (sizes, estimated tokens, latency). Absent
    /// for executions from before the Optimization Layer and when metrics were off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optimization: Option<OptimizationMetrics>,
    /// The workflow step this execution ran as, when it was one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow: Option<WorkflowLink>,
    /// The timeline, without streamed answer text. The tail is kept when it is too long.
    pub events: Vec<ExecutionEvent>,
}

/// Where an execution sits in a workflow run, so the Execution Inspector can say so: the
/// breadcrumb *Workflow > Password Recovery > Backend > Execution #42*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowLink {
    pub workflow_id: String,
    pub workflow_name: String,
    pub workflow_execution_id: String,
    pub node_id: String,
    pub node_label: String,
    pub attempt: u32,
    pub iteration: u32,
}
