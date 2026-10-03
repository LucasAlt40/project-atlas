use std::collections::BTreeMap;

use serde::Serialize;

use super::task::Task;
use super::usage::{QuotaInfo, UsageMetrics};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    RuntimeNotInstalled,
    RuntimeUnavailable,
    AuthenticationRequired,
    ModelUnavailable,
    Timeout,
    ExecutionFailed,
    InvalidRequest,
    UnexpectedResponse,
}

impl FailureKind {
    /// The wire name (same as the serialized form).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RuntimeNotInstalled => "runtime_not_installed",
            Self::RuntimeUnavailable => "runtime_unavailable",
            Self::AuthenticationRequired => "authentication_required",
            Self::ModelUnavailable => "model_unavailable",
            Self::Timeout => "timeout",
            Self::ExecutionFailed => "execution_failed",
            Self::InvalidRequest => "invalid_request",
            Self::UnexpectedResponse => "unexpected_response",
        }
    }
}

/// Why an execution failed: a message safe to show, plus optional technical details.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionEventKind {
    Started,
    StartingRuntime,
    SendingPrompt,
    WaitingForModel,
    OutputChunk,
    ToolStarted,
    ToolCompleted,
    Completed,
    Failed,
}

/// Progress notification emitted while an execution runs. Carries both ids so a listener
/// can keep several agents' executions apart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
