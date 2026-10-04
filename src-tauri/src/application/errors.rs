use std::collections::BTreeMap;
use std::fmt;

use serde::Serialize;

/// Every user-facing failure the core can report. The UI turns a code into a message in the
/// user's language; the core never translates. Adding a code means adding its text on the
/// frontend (`src/i18n`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    NameRequired,
    NameTooLong,
    InstructionsRequired,
    PersonalityRequired,
    RuntimeRequired,
    RuntimeNotSupported,
    ModelRequired,
    ModelInvalid,
    PersonalityNotFound,
    PersonalityInUse,
    AgentNotFound,
    AgentBusy,
    AgentAlreadyPlaced,
    WorkspaceNotFound,
    WorkspaceFull,
    WorkspaceBusy,
    ProjectFolderRequired,
    ProjectFolderNotFound,
    MessageEmpty,
    ApprovalNotFound,
    /// The question the answer is for does not exist.
    InteractionNotFound,
    /// The question was already answered or cancelled.
    InteractionNotPending,
    /// The answer is empty, too long, or not one of the question's options.
    InteractionAnswerInvalid,
    ExecutionNotFound,
    ExecutionNotRunning,
    TerminalInputUnsupported,
    TerminalInputInvalid,
    ProcessControlFailed,
    PermissionProfileInvalid,
    /// The result contract has a malformed or repeated outcome id, no label, or too many outcomes.
    ResultContractInvalid,
    /// The agent needs Git worktree isolation and the project is not a Git repository.
    GitRepositoryRequired,
    GitUnavailable,
    /// No base branch to start a worktree from (detached HEAD, no commits).
    WorktreeBaseUnavailable,
    WorktreeNotFound,
    WorktreeInvalidState,
    WorktreeBusy,
    WorktreeFailed,
    /// The agent's policy does not let Atlas merge.
    MergeNotAllowed,
    LanguageUnsupported,
    ProjectNotFound,
    ProjectNotDirectory,
    ProjectAlreadyInitialized,
    /// `.atlas/` exists but its manifest cannot be read or is not a version Atlas understands.
    HarnessInvalid,
    HarnessNotInitialized,
    /// The model's answer was not the structured findings Atlas asked for.
    SemanticAnalysisFailed,
    /// The agent's runtime cannot be run without tools, so it cannot read evidence safely.
    SemanticUnsupported,
    ProjectAnalysisFailed,
    HarnessGenerationFailed,
    /// The project path is relative or tries to leave its folder.
    UnsafeProjectPath,
    WorkflowNotFound,
    /// The workflow did not pass validation and cannot start. The UI lists what is wrong.
    WorkflowInvalid,
    /// The workflow has a run in progress: it cannot be edited or deleted until that ends.
    WorkflowRunning,
    WorkflowTemplateNotFound,
    WorkflowExecutionNotFound,
    /// The run is not in a state that allows what was asked (pausing a finished run…).
    WorkflowStateInvalid,
    /// A failed run cannot be picked up again (`reason` says why: no route for its result, the
    /// loop limit, its worktree is gone…).
    WorkflowNotRecoverable,
    /// The run has no code in a worktree that the asked-for action applies to.
    IntegrationNotAvailable,
    IdeUnknown,
    IdeNotInstalled,
    IdeLaunchFailed,
    StorageFailed,
}

/// A failure with a stable code, values for the message (`params`), and an optional technical
/// `detail` that is for logs and "technical details", never the primary message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppError {
    pub code: ErrorCode,
    pub params: BTreeMap<String, String>,
    pub detail: Option<String>,
}

impl AppError {
    pub fn new(code: ErrorCode) -> Self {
        Self {
            code,
            params: BTreeMap::new(),
            detail: None,
        }
    }

    #[must_use]
    pub fn with(mut self, key: &str, value: impl Into<String>) -> Self {
        self.params.insert(key.to_owned(), value.into());
        self
    }

    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn storage(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::StorageFailed).with_detail(detail)
    }

    #[cfg(test)]
    pub fn is(&self, code: ErrorCode) -> bool {
        self.code == code
    }
}

/// English text, for logs and as a last-resort fallback; the UI uses the code.
impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.code)?;
        if let Some(detail) = &self.detail {
            write!(f, ": {detail}")?;
        }
        Ok(())
    }
}

impl std::error::Error for AppError {}
