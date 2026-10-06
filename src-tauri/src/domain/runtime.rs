use serde::Serialize;

use super::security::ToolAccess;

/// Who provides the AI capability (Anthropic, `OpenCode`, `OpenAI`…). A runtime is how Atlas
/// reaches that provider on this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    Cli,
    /// Direct API access: not implemented yet, but the model allows it.
    #[allow(dead_code)]
    Api,
}

/// How a runtime authenticates. Credentials themselves never appear in this model (and
/// must never be written to `config.json`): API keys will live in the OS credential store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum AuthKind {
    /// The tool's own signed-in session (for example a CLI after the user signed in).
    CliSession,
    ApiKey,
    EnvironmentVariable,
    CredentialStore,
}

/// What a runtime actually supports, so the UI and executor adapt instead of assuming.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct RuntimeCapabilities {
    /// The runtime can list the models it offers.
    pub model_discovery: bool,
    pub streaming: bool,
    /// Atlas sends the system prompt through a dedicated channel (otherwise it is part of
    /// the single prompt text).
    pub system_prompt: bool,
    /// Atlas can run a task through this runtime today.
    pub non_interactive_execution: bool,
    pub authentication: Vec<AuthKind>,
    /// The runtime reports token counts for an execution.
    pub usage_metrics: bool,
    /// The runtime reports a cost for an execution.
    pub cost_metrics: bool,
    /// The runtime or provider reports quota (how much of an allowance is used).
    pub quota_metrics: bool,
    /// An execution runs attached to a real terminal (PTY) that the UI can show.
    pub interactive_terminal: bool,
    /// The user can interrupt (Ctrl+C) the running process through that terminal.
    pub interrupt: bool,
    /// The process reads manual input from the terminal. `false` for the runtimes Atlas runs
    /// non-interactively (`claude -p`, `opencode run`): they ignore a terminal's stdin, so
    /// their terminal is read-only.
    pub terminal_input: bool,
    /// The terminal's size can change while the process runs.
    pub terminal_resize: bool,
    /// Can run with every tool off, answering only from the prompt (needed to show it evidence
    /// without giving it the filesystem, as semantic analysis does).
    pub text_only: bool,
    /// Can be launched with tools that create and edit files (and only those: no shell, no
    /// network). Whether an execution gets them is decided per execution, from the agent's
    /// policy and whether it works in an isolated worktree; see `RuntimeRequest::allow_edits`.
    pub file_edit: bool,
    /// What the runtime's own tools can do as Atlas launches it. Facts about the external tool:
    /// `true` means Atlas cannot stop the tool from doing it.
    pub tool_access: ToolAccess,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfo {
    pub id: String,
    pub name: String,
    pub provider: ProviderRef,
    pub transport: Transport,
    pub capabilities: RuntimeCapabilities,
    /// A code the UI maps to a hint shown next to the model field when models cannot be
    /// discovered (for example `claude_alias`).
    pub model_hint: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthState {
    Authenticated,
    Required,
    /// Atlas could not tell (the runtime has no reliable way to report it).
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Authentication {
    pub kind: Option<AuthKind>,
    pub state: AuthState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    NotInstalled,
    /// Installed but not usable (it failed its own checks).
    Unavailable,
    AuthenticationRequired,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
}

/// How the model list was obtained: never pretend discovery succeeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelDiscovery {
    Discovered,
    /// The runtime cannot list models; the user must enter a model id.
    Unsupported,
    /// The runtime is not installed or not usable, so models cannot be listed.
    Unavailable,
    Failed,
}

/// What the user should know about a runtime's state, as a code the UI translates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeNotice {
    /// The tool is installed but not signed in.
    SignInRequired,
    /// Detected, but Atlas cannot run tasks with it yet. No runtime is in that state today; the
    /// code stays because the UI knows it.
    #[cfg_attr(not(test), allow(dead_code))]
    ExecutionNotSupported,
    /// Installed but failing its own checks.
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    pub runtime: RuntimeInfo,
    pub availability: Availability,
    pub version: Option<String>,
    pub authentication: Authentication,
    pub available_models: Vec<ModelInfo>,
    pub model_discovery: ModelDiscovery,
    /// Technical detail about a failed model listing (for the technical details, not the headline).
    pub discovery_error: Option<String>,
    pub notice: Option<RuntimeNotice>,
}
