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
    /// The folder (or one that resolves to it) is already a project of the workspace.
    ProjectAlreadyInWorkspace,
    /// The workspace has no project with that id.
    WorkspaceProjectNotFound,
    /// A workspace always keeps at least one project.
    WorkspaceNeedsAProject,
    /// The execution scope names a project the workspace does not have.
    ScopeInvalid,
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
    /// A built-in template cannot be changed or deleted; save a copy of a workflow instead.
    WorkflowTemplateBuiltin,
    WorkflowExecutionNotFound,
    /// The run is not in a state that allows what was asked (pausing a finished run…).
    WorkflowStateInvalid,
    /// The chain waits for the person to decide about the code of its last run (apply or keep it)
    /// before the next project runs (ADR 0043).
    ChainAwaitingDelivery,
    /// The code of the chain's last run was discarded while the chain waited for it: the next
    /// project is not run on work that was thrown away.
    ChainChangesDiscarded,
    /// The run has no project of its chain left to go on to, or the chain already went on.
    ChainNothingToContinue,
    /// A failed run cannot be picked up again (`reason` says why: no route for its result, the
    /// loop limit, its worktree is gone…).
    WorkflowNotRecoverable,
    /// The run has no code in a worktree that the asked-for action applies to.
    IntegrationNotAvailable,
    McpConnectionNotFound,
    /// The connection's configuration is not acceptable (`reason` is a code: name, shell,
    /// executable, expansion…).
    McpConnectionInvalid,
    /// Another connection of the workspace already has that name.
    McpNameTaken,
    /// The memory does not exist.
    MemoryNotFound,
    /// The memory is not in the shape a memory may have (`reason` says which: `empty_title`,
    /// `empty_content`, `title_too_long`, `content_too_long`, `owner_mismatch`, `bad_status`).
    MemoryInvalid,
    /// The text of the memory looks like a secret; a memory is stored in the clear and sent to a
    /// model.
    MemoryLooksLikeSecret,
    McpGrantInvalid,
    /// The OS credential store could not be reached (locked, missing).
    McpCredentialStoreUnavailable,
    /// The secret is not one the connection declares.
    McpSecretUnknown,
    /// The connection has to be enabled (and valid, with its secrets) before it is started.
    McpNotReady,
    /// The runtime cannot give MCP servers (no adapter, or it was not investigated).
    McpRuntimeUnsupported,
    /// Starting the server to look at it failed (`detail` says what the runtime reported).
    McpProbeFailed,
    /// The catalogue entry does not exist or cannot be added yet (Figma: HTTP and OAuth).
    McpPresetUnavailable,
    /// The text or file is not the configuration format it was said to be.
    McpConfigUnreadable,
    /// The runtime has no configuration file Atlas can read or write.
    McpConfigUnsupported,
    /// The runtime's configuration file could not be read or written (`detail` says why).
    McpConfigIo,
    /// The MCP Registry could not be reached or answered with something unusable.
    McpRegistryUnavailable,
    /// The file is larger than Atlas reads as a configuration file.
    ImportFileTooLarge,
    /// The file is not JSON, or not a file Atlas exported (`reason` says which).
    ImportNotAtlasFile,
    /// The file comes from a newer Atlas: its schema version is not one this Atlas understands.
    ImportUnsupportedVersion,
    /// The file is an Atlas file with something wrong in it (`reason` is a code, `at` where).
    ImportInvalid,
    /// The import was refused as a whole, nothing was changed (`reason` is a code, `item` names
    /// the entry it is about).
    ImportRejected,
    /// The configuration to export does not exist.
    ExportTargetNotFound,
    /// The configuration file could not be read or written (`detail` says why).
    ConfigFileIo,
    IdeUnknown,
    IdeNotInstalled,
    IdeLaunchFailed,
    StorageFailed,
    /// Cleaning up Atlas's data cannot start while an agent is running.
    StorageBusy,
    /// The update server could not be reached or its answer was not usable (`detail` says why).
    UpdateCheckFailed,
    /// The update was found but downloading, verifying or installing it failed.
    UpdateInstallFailed,
    SpecNotFound,
    SpecArtifactNotFound,
    SpecQuestionNotFound,
    SpecFindingNotFound,
    /// What was asked for a new spec is not acceptable (`reason` is a code: title, request, agent…).
    SpecInvalid,
    /// The change id is not kebab-case (`reason`: `empty`, `format`, `too_long`).
    SpecChangeIdInvalid,
    /// The spec is not in a state that allows what was asked (`reason` is a code).
    SpecStateInvalid,
    /// The documents this one is written from are not all approved (`artifacts` lists them).
    SpecDependenciesNotApproved,
    /// An agent is already working on the spec: one run at a time.
    SpecRunInProgress,
    /// What the spec's runs used reached the budget (`spent`, `limit`): the person decides.
    SpecBudgetReached,
    /// The person was looking at an older revision (`current` is the latest).
    SpecRevisionOutdated,
    /// Asking for changes, accepting a risk or going without the review needs the person's words.
    SpecNoteRequired,
    /// The document has errors the checks found: they have to be fixed first.
    SpecValidationBlocking,
    /// Something still keeps the spec from being ready (the interface lists what).
    SpecNotReady,
    /// The change folder is already in the project (`change` names it); nothing was written.
    SpecChangeExists,
    /// Writing the files stopped half way (`detail` says why); doing it again picks up from there.
    SpecPublishFailed,
    /// That change is already one of this workspace's specs.
    SpecAlreadyImported,
    /// The change could not be read from the project (`reason`).
    SpecImportFailed,
    /// The `OpenSpec` command is not installed or is a version Atlas does not know.
    SpecCliUnavailable,
    /// The `OpenSpec` command ran and said no (`detail`).
    SpecCliFailed,
    /// An implementation run still uses the spec, so it cannot be deleted.
    SpecInUse,
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
