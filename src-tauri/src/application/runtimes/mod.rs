mod antigravity;
mod claude;
mod claude_mcp;
mod cli;
mod codex;
mod codex_mcp;
mod delivery;
#[cfg(test)]
pub(crate) mod detect_only;
mod gemini;
mod gemini_mcp;
mod mcp_adapter;
mod opencode;
mod opencode_mcp;

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use super::mcp::{LaunchServer, McpLaunch, McpProbe};
use super::process::{ExecutionScope, ProcessRunner};
use super::prompt::Prompt;
use crate::domain::context::{ReportedLimits, RuntimeSurface};
use crate::domain::execution::{ExecutionFailure, FailureKind};
use crate::domain::interaction::InteractionDetection;
use crate::domain::mcp::ReportedMcpTool;
use crate::domain::runtime::{
    Authentication, Availability, ModelDiscovery, ModelInfo, RuntimeInfo, RuntimeNotice,
    RuntimeStatus,
};
use crate::domain::security::Reason;
use crate::domain::usage::{QuotaInfo, UsageMetrics};

pub use antigravity::AntigravityRuntime;
pub use claude::ClaudeRuntime;
pub use codex::CodexRuntime;
pub use delivery::{sha256_hex, Delivery};
pub use gemini::GeminiRuntime;
pub use opencode::OpenCodeRuntime;

/// Everything a runtime needs to answer one task. The runtime does not decide what the
/// personality means: it only delivers the prompt.
#[derive(Debug, Clone)]
pub struct RuntimeRequest {
    pub model_id: String,
    pub prompt: Prompt,
    pub working_dir: PathBuf,
    /// Which execution this is. The runtime hands it to the process port unchanged; it neither
    /// reads policy from it nor can widen it.
    pub scope: ExecutionScope,
    /// The runtime must not be given any tool: it may only answer from the prompt. Used when
    /// the prompt is the whole of what the model may see (semantic analysis).
    pub text_only: bool,
    /// The execution may create and edit files in its working directory (and nothing else): set
    /// by the execution service only when the agent's policy allows file writes and the agent
    /// works in an isolated worktree. A runtime that cannot ignores it.
    pub allow_edits: bool,
    /// The MCP servers this execution is given, and only those. `None`: none (the runtime loads
    /// no MCP server of any kind). Never set for a `text_only` request; a runtime ignores it
    /// when it has no adapter.
    pub mcp: Option<McpLaunch>,
}

/// A runtime's answer, normalized: provider-specific output formats stop here.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeOutput {
    pub text: String,
    /// Facts the runtime reported (duration, turns…) as text, keyed in camelCase.
    pub metadata: BTreeMap<String, String>,
    /// What the execution consumed, if the runtime reported it. Never estimated by Atlas.
    pub usage: Option<UsageMetrics>,
    /// Quota the runtime or provider reported alongside, if any.
    pub quota: Option<QuotaInfo>,
}

/// What a runtime reports while executing. A runtime emits only what its tool really
/// provides: the three stages always; `Output` and `Tool*` only when the tool streams them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeEvent {
    Starting,
    Sending,
    Waiting,
    /// A piece of the answer text, in order. Concatenating every piece gives the live
    /// response (which can include narration that the final answer leaves out).
    Output(String),
    ToolStarted(String),
    ToolCompleted(String),
    /// The tools the model could call, as the runtime itself listed them when it started and
    /// before it asked the model anything. MCP tools are among them.
    ToolsReported(Vec<String>),
}

/// What a runtime found out about itself on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    pub installed: bool,
    pub version: Option<String>,
    /// Set when the tool is installed but unusable; the text explains why.
    pub unavailable: Option<String>,
    pub authentication: Authentication,
    /// What the user should know about the runtime's state (e.g. that it must be signed in).
    pub notice: Option<RuntimeNotice>,
}

/// Port: one way of reaching an AI provider on this machine (a CLI today, an API later).
///
/// Adding a runtime means adding an implementation and registering it in `lib.rs`; agents,
/// prompts and executions do not change. Runtimes own their executable, arguments,
/// environment and working directory: none of that ever comes from the webview.
pub trait ModelRuntime: Send + Sync {
    fn info(&self) -> RuntimeInfo;

    /// Is the runtime installed and signed in? Must be cheap and side-effect free.
    fn detect(&self) -> Detection;

    /// Lists the models the runtime offers. Only called when
    /// `info().capabilities.model_discovery` is true.
    ///
    /// # Errors
    ///
    /// Fails with a [`RuntimeError`] if the list cannot be obtained.
    fn list_models(&self) -> Result<Vec<ModelInfo>, RuntimeError>;

    /// Sends the prompt to the selected model and returns its answer.
    ///
    /// # Errors
    ///
    /// Fails with a [`RuntimeError`] describing what went wrong.
    fn execute(
        &self,
        request: &RuntimeRequest,
        progress: &dyn Fn(RuntimeEvent),
    ) -> Result<RuntimeOutput, RuntimeError>;

    /// The payload this runtime hands to its process for `request`. `execute` must take its
    /// prompt from here and nowhere else: the Context Manifest hashes this very value, so what was
    /// hashed is what was delivered. The default follows the runtime's declared
    /// `system_prompt` channel: the whole prompt as one text for `Unsupported`, split for a runtime
    /// that really has one (and whose `execute` then sends each part where it belongs).
    fn delivery(&self, request: &RuntimeRequest) -> Delivery {
        // The channel the runtime declares, and nothing more: a runtime that says `Unsupported`
        // gets one text and is never given a split it cannot honour.
        Delivery::for_channel(&request.prompt, self.info().capabilities.system_prompt)
    }

    /// What the runtime itself states about the limits of `model_id`. `None`: it states none, and
    /// Atlas does not guess (no runtime reports limits today).
    fn reported_limits(&self, _model_id: &str) -> Option<ReportedLimits> {
        None
    }

    /// What can shape an execution of `request` besides the prompt, as far as Atlas knows by its
    /// own design (`Declared`) before the runtime has run. The default knows only the prompt it
    /// delivers and admits that the rest is not observed.
    fn surface(&self, _request: &RuntimeRequest) -> RuntimeSurface {
        RuntimeSurface::baseline(&self.info().id, self.info().capabilities.system_prompt)
    }

    /// Starts one MCP server through this runtime, as it would for a step, only to see what the
    /// runtime reports: its status and the tools it lists. No model is asked anything. Only
    /// called for a runtime whose `capabilities.mcp` is `Supported`.
    ///
    /// # Errors
    ///
    /// Fails if the runtime has no such adapter or the probe cannot run.
    fn probe_mcp(&self, _server: &LaunchServer) -> Result<McpProbe, RuntimeError> {
        Err(RuntimeError::Unavailable(
            "This runtime has no MCP adapter.".to_owned(),
        ))
    }

    /// Which of the tools the runtime listed at start-up are MCP tools, and of which of the
    /// servers Atlas launched. A tool of a server Atlas did not launch has no server. The default
    /// knows none (a runtime with no adapter has no MCP tools to tell apart).
    fn mcp_tools_in(&self, _reported: &[String], _launched: &[&str]) -> Vec<ReportedMcpTool> {
        Vec::new()
    }

    /// Whether this answer is the runtime's own way of asking a person something (a tool's
    /// approval event, a question tool…). The adapter is the only place that may know a tool's
    /// protocol; the orchestrator never does. The default knows none, and the generic detector
    /// (the `atlas-interaction` block, then conservative text analysis) takes over.
    fn detect_interaction(&self, _output: &RuntimeOutput) -> Option<InteractionDetection> {
        None
    }
}

/// Runtime errors, normalized so the UI never needs to know which runtime produced them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    /// Atlas's security policy refused to start the runtime.
    PermissionDenied(Reason),
    NotInstalled,
    Unavailable(String),
    AuthenticationRequired,
    ModelUnavailable(String),
    /// The provider answered "too many requests" (HTTP 429, a usage limit). Worth trying again later.
    RateLimited(String),
    Timeout,
    ExecutionFailed(String),
    InvalidRequest(String),
    UnexpectedResponse(String),
}

impl RuntimeError {
    /// A message that is fine to show as the primary error in the UI.
    pub fn user_message(&self) -> String {
        match self {
            Self::PermissionDenied(_) => {
                "Atlas's security policy did not allow this execution.".to_owned()
            }
            Self::NotInstalled => {
                "The AI runtime is not installed or could not be found on this machine.".to_owned()
            }
            Self::AuthenticationRequired => {
                "The runtime needs you to sign in. Authenticate it in its own tool, then try again."
                    .to_owned()
            }
            Self::ModelUnavailable(model) => {
                format!("The model \"{model}\" is not available from this runtime.")
            }
            Self::RateLimited(_) => {
                "The provider is rate-limiting this model (too many requests or its free allowance is \
                 used up). Wait a while and try again, or pick another model."
                    .to_owned()
            }
            Self::Timeout => "The runtime stopped responding (no output for too long).".to_owned(),
            Self::ExecutionFailed(_) => "The runtime could not complete the request.".to_owned(),
            Self::Unavailable(reason) | Self::InvalidRequest(reason) => reason.clone(),
            Self::UnexpectedResponse(_) => {
                "The runtime returned a response Atlas could not understand.".to_owned()
            }
        }
    }

    fn kind(&self) -> FailureKind {
        match self {
            Self::PermissionDenied(_) => FailureKind::PermissionDenied,
            Self::NotInstalled => FailureKind::RuntimeNotInstalled,
            Self::Unavailable(_) => FailureKind::RuntimeUnavailable,
            Self::AuthenticationRequired => FailureKind::AuthenticationRequired,
            Self::ModelUnavailable(_) => FailureKind::ModelUnavailable,
            Self::RateLimited(_) => FailureKind::RateLimited,
            Self::Timeout => FailureKind::Timeout,
            Self::ExecutionFailed(_) => FailureKind::ExecutionFailed,
            Self::InvalidRequest(_) => FailureKind::InvalidRequest,
            Self::UnexpectedResponse(_) => FailureKind::UnexpectedResponse,
        }
    }

    pub fn into_failure(self) -> ExecutionFailure {
        let details = match &self {
            Self::ExecutionFailed(details)
            | Self::UnexpectedResponse(details)
            | Self::RateLimited(details)
                if !details.is_empty() =>
            {
                Some(details.clone())
            }
            Self::PermissionDenied(reason) => Some(reason.as_str().to_owned()),
            _ => None,
        };
        ExecutionFailure {
            kind: self.kind(),
            message: self.user_message(),
            details,
        }
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.user_message())
    }
}

impl std::error::Error for RuntimeError {}

/// The programs of the runtimes in [`RuntimeRegistry::with_default_runtimes`]: the only ones the
/// process port starts on Atlas's own behalf. Adding a runtime means adding its program here (the
/// Antigravity CLI installs as `agy`).
pub const RUNTIME_PROGRAMS: [&str; 5] = ["opencode", "claude", "codex", "gemini", "agy"];

/// All known runtimes, in display order.
pub struct RuntimeRegistry {
    runtimes: Vec<Arc<dyn ModelRuntime>>,
}

impl RuntimeRegistry {
    pub fn new(runtimes: Vec<Arc<dyn ModelRuntime>>) -> Self {
        Self { runtimes }
    }

    /// The runtimes Atlas knows about. Every one can run tasks.
    pub fn with_default_runtimes(runner: &Arc<dyn ProcessRunner>) -> Self {
        Self::new(vec![
            Arc::new(OpenCodeRuntime::new(runner.clone())),
            Arc::new(ClaudeRuntime::new(runner.clone())),
            Arc::new(CodexRuntime::new(runner.clone())),
            Arc::new(GeminiRuntime::new(runner.clone())),
            Arc::new(AntigravityRuntime::new(runner.clone())),
        ])
    }

    #[cfg(test)]
    pub fn into_runtimes(self) -> Vec<Arc<dyn ModelRuntime>> {
        self.runtimes
    }

    pub fn find(&self, id: &str) -> Option<Arc<dyn ModelRuntime>> {
        self.runtimes.iter().find(|r| r.info().id == id).cloned()
    }

    /// Detects every runtime and lists its models where it can (in parallel: each probe
    /// starts a process).
    pub fn inspect_all(&self) -> Vec<RuntimeStatus> {
        std::thread::scope(|scope| {
            let handles: Vec<_> = self
                .runtimes
                .iter()
                .map(|r| scope.spawn(|| inspect(r.as_ref())))
                .collect();
            handles
                .into_iter()
                .zip(&self.runtimes)
                .map(|(handle, runtime)| {
                    handle.join().unwrap_or_else(|_| {
                        status(
                            runtime.info(),
                            &Detection {
                                installed: true,
                                version: None,
                                unavailable: Some("Detection failed unexpectedly.".to_owned()),
                                authentication: Authentication {
                                    kind: None,
                                    state: crate::domain::runtime::AuthState::Unknown,
                                },
                                notice: None,
                            },
                            Vec::new(),
                            ModelDiscovery::Unavailable,
                            None,
                        )
                    })
                })
                .collect()
        })
    }
}

/// Detects one runtime and, when it is usable and supports it, discovers its models.
/// This is the single place a [`RuntimeStatus`] is assembled.
pub fn inspect(runtime: &dyn ModelRuntime) -> RuntimeStatus {
    let info = runtime.info();
    let detection = runtime.detect();
    let usable = detection.installed && detection.unavailable.is_none();
    let (models, discovery, error) = if !usable {
        (Vec::new(), ModelDiscovery::Unavailable, None)
    } else if !info.capabilities.model_discovery {
        (Vec::new(), ModelDiscovery::Unsupported, None)
    } else {
        match runtime.list_models() {
            Ok(models) => (models, ModelDiscovery::Discovered, None),
            Err(error) => (
                Vec::new(),
                ModelDiscovery::Failed,
                Some(error.user_message()),
            ),
        }
    };
    status(info, &detection, models, discovery, error)
}

fn status(
    runtime: RuntimeInfo,
    detection: &Detection,
    models: Vec<ModelInfo>,
    discovery: ModelDiscovery,
    discovery_error: Option<String>,
) -> RuntimeStatus {
    use crate::domain::runtime::AuthState;
    let availability = if !detection.installed {
        Availability::NotInstalled
    } else if detection.unavailable.is_some() {
        Availability::Unavailable
    } else if detection.authentication.state == AuthState::Required {
        Availability::AuthenticationRequired
    } else {
        Availability::Ready
    };
    RuntimeStatus {
        runtime,
        availability,
        version: detection.version.clone(),
        authentication: detection.authentication,
        available_models: models,
        model_discovery: discovery,
        discovery_error,
        notice: detection
            .unavailable
            .as_ref()
            .map(|_| RuntimeNotice::Unavailable)
            .or(detection.notice),
    }
}

/// The surface of a runtime that takes Atlas's MCP servers **besides** the user's own (it cannot
/// be told to leave those out): what Atlas gave, and an honest entry for what it cannot see.
pub(crate) fn merged_mcp_surface(
    runtime_id: &str,
    channel: crate::domain::runtime::SystemPromptChannel,
    request: &RuntimeRequest,
) -> RuntimeSurface {
    use crate::domain::context::{Observation, SurfaceControl, SurfaceEntry, SurfaceKind};
    let mut surface = RuntimeSurface::baseline(runtime_id, channel);
    let given = request
        .mcp
        .as_ref()
        .filter(|_| !request.text_only)
        .map(|m| {
            m.servers
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
        })
        .filter(|names| !names.is_empty());
    surface.declare(
        SurfaceEntry::new(
            SurfaceKind::McpServers,
            SurfaceControl::AtlasControlled,
            Observation::Declared,
        )
        .with_detail(given.map_or_else(|| "none".to_owned(), |names| names.join(","))),
    );
    // The runtime merges its own configuration: servers the user set up there load as well.
    surface.declare(SurfaceEntry::new(
        SurfaceKind::McpServers,
        SurfaceControl::UserControlled,
        Observation::NotObserved,
    ));
    surface
}

#[cfg(test)]
pub mod fake {
    use std::sync::Mutex;

    use super::{
        Detection, LaunchServer, McpProbe, ModelRuntime, RuntimeError, RuntimeEvent, RuntimeOutput,
        RuntimeRequest,
    };
    use crate::domain::mcp::{
        McpFeatures, McpProbeKind, McpSupport, McpToolFilter, ReportedMcpTool,
    };
    use crate::domain::runtime::{
        AuthKind, AuthState, Authentication, ModelInfo, ProviderRef, RuntimeCapabilities,
        RuntimeInfo, Transport,
    };
    use crate::domain::security::ToolAccess;

    /// Runtime with a scripted answer; records the requests it receives.
    pub struct FakeRuntime {
        pub id: String,
        pub answer: Result<RuntimeOutput, RuntimeError>,
        pub requests: Mutex<Vec<RuntimeRequest>>,
        /// When set, `execute` waits here after reporting `Waiting`, so a test can prove two
        /// executions are in flight at the same time.
        pub barrier: Option<std::sync::Arc<std::sync::Barrier>>,
        /// Pieces of live output reported (as `Output` events) before the answer.
        pub chunks: Vec<String>,
        /// What the "agent" does in its working directory before answering (writes files…).
        pub work: Option<Work>,
        /// Whether it can be run with every tool off.
        pub text_only: bool,
        /// Whether it can be launched with file-editing tools.
        pub file_edit: bool,
        /// What it declares about MCP (`NotInvestigated`, like the runtimes nobody measured).
        pub mcp: McpSupport,
        /// How its MCP support filters tools and probes.
        pub mcp_features: McpFeatures,
        /// Tools it reports having when it starts (as a CLI's start-up report does).
        pub reported_tools: Option<Vec<String>>,
        /// What starting an MCP server to look at it shows (none: the probe fails).
        pub probe: Option<McpProbe>,
        /// The MCP servers it was asked to probe, by name.
        pub probed: Mutex<Vec<String>>,
    }

    pub type Work = Box<dyn Fn(&RuntimeRequest) + Send + Sync>;

    impl FakeRuntime {
        pub fn new(id: &str, answer: Result<&str, RuntimeError>) -> Self {
            Self {
                id: id.to_owned(),
                answer: answer.map(|text| RuntimeOutput {
                    text: text.to_owned(),
                    metadata: [("durationMs".to_owned(), "5".to_owned())].into(),
                    usage: None,
                    quota: None,
                }),
                requests: Mutex::new(Vec::new()),
                barrier: None,
                chunks: Vec::new(),
                work: None,
                text_only: true,
                file_edit: false,
                mcp: McpSupport::NotInvestigated,
                mcp_features: McpFeatures::default(),
                reported_tools: None,
                probe: None,
                probed: Mutex::new(Vec::new()),
            }
        }

        pub fn with_work(mut self, work: impl Fn(&RuntimeRequest) + Send + Sync + 'static) -> Self {
            self.work = Some(Box::new(work));
            self
        }

        pub fn with_mcp(mut self) -> Self {
            self.mcp = McpSupport::Supported;
            // What the Claude adapter measured: a deny-list, and a probe that lists tools.
            self.mcp_features = McpFeatures {
                tool_filter: McpToolFilter::DenyList,
                probe: McpProbeKind::Tools,
                strict: true,
            };
            self
        }

        pub fn with_mcp_features(mut self, features: McpFeatures) -> Self {
            self.mcp = McpSupport::Supported;
            self.mcp_features = features;
            self
        }

        pub fn probing(
            mut self,
            status: crate::domain::mcp::McpServerStatus,
            tools: &[&str],
        ) -> Self {
            self.probe = Some(McpProbe {
                status,
                tools: tools.iter().map(|t| (*t).to_owned()).collect(),
            });
            self
        }

        pub fn reporting_tools(mut self, tools: &[&str]) -> Self {
            self.reported_tools = Some(tools.iter().map(|t| (*t).to_owned()).collect());
            self
        }

        pub fn with_file_edit(mut self) -> Self {
            self.file_edit = true;
            self
        }

        pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
            if let Ok(output) = &mut self.answer {
                output.metadata.insert(key.to_owned(), value.to_owned());
            }
            self
        }

        pub fn with_usage(mut self, usage: crate::domain::usage::UsageMetrics) -> Self {
            if let Ok(output) = &mut self.answer {
                output.usage = Some(usage);
            }
            self
        }

        pub fn with_chunks(mut self, chunks: &[&str]) -> Self {
            self.chunks = chunks.iter().map(|c| (*c).to_owned()).collect();
            self
        }

        pub fn without_text_only(mut self) -> Self {
            self.text_only = false;
            self
        }

        pub fn with_barrier(mut self, barrier: std::sync::Arc<std::sync::Barrier>) -> Self {
            self.barrier = Some(barrier);
            self
        }
    }

    impl ModelRuntime for FakeRuntime {
        fn mcp_tools_in(&self, reported: &[String], launched: &[&str]) -> Vec<ReportedMcpTool> {
            // The same naming as the one adapter that exists, so the guard can be tested.
            super::claude_mcp::ClaudeMcpAdapter::classify(reported, launched)
        }

        fn probe_mcp(&self, server: &LaunchServer) -> Result<McpProbe, RuntimeError> {
            self.probed.lock().unwrap().push(server.name.clone());
            self.probe
                .clone()
                .ok_or_else(|| RuntimeError::Unavailable("no probe".to_owned()))
        }

        fn info(&self) -> RuntimeInfo {
            RuntimeInfo {
                id: self.id.clone(),
                name: format!("Fake {}", self.id),
                provider: ProviderRef {
                    id: "fake".to_owned(),
                    name: "Fake".to_owned(),
                },
                transport: Transport::Cli,
                capabilities: RuntimeCapabilities {
                    model_discovery: true,
                    streaming: false,
                    system_prompt: crate::domain::runtime::SystemPromptChannel::Unsupported,
                    mcp: self.mcp,
                    mcp_features: self.mcp_features,
                    non_interactive_execution: true,
                    authentication: vec![AuthKind::CliSession],
                    usage_metrics: true,
                    cost_metrics: true,
                    quota_metrics: false,
                    interactive_terminal: false,
                    interrupt: false,
                    terminal_input: false,
                    terminal_resize: false,
                    text_only: self.text_only,
                    file_edit: self.file_edit,
                    tool_access: ToolAccess::NONE,
                },
                model_hint: None,
            }
        }

        fn detect(&self) -> Detection {
            Detection {
                installed: true,
                version: Some("1.0".to_owned()),
                unavailable: None,
                authentication: Authentication {
                    kind: Some(AuthKind::CliSession),
                    state: AuthState::Authenticated,
                },
                notice: None,
            }
        }

        fn list_models(&self) -> Result<Vec<ModelInfo>, RuntimeError> {
            Ok(vec![ModelInfo {
                id: "m1".to_owned(),
                name: "m1".to_owned(),
            }])
        }

        fn execute(
            &self,
            request: &RuntimeRequest,
            progress: &dyn Fn(RuntimeEvent),
        ) -> Result<RuntimeOutput, RuntimeError> {
            progress(RuntimeEvent::Starting);
            progress(RuntimeEvent::Sending);
            progress(RuntimeEvent::Waiting);
            if let Some(tools) = &self.reported_tools {
                progress(RuntimeEvent::ToolsReported(tools.clone()));
            }
            for chunk in &self.chunks {
                progress(RuntimeEvent::Output(chunk.clone()));
            }
            if let Some(barrier) = &self.barrier {
                barrier.wait();
            }
            self.requests.lock().unwrap().push(request.clone());
            if let Some(work) = &self.work {
                work(request);
            }
            self.answer.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::process::fake::{ok, FakeProcessRunner};
    use crate::application::process::ProcessOutput;
    use crate::domain::runtime::AuthState;

    /// `OpenCode` and Claude installed; Claude signed in; nothing else.
    fn registry() -> RuntimeRegistry {
        let runner: Arc<dyn ProcessRunner> = Arc::new(FakeProcessRunner::new(
            &["opencode", "claude"],
            |spec| match (spec.program.as_str(), spec.args[0].as_str()) {
                ("opencode", "models") => ok("opencode/big-pickle\n"),
                ("claude", "auth") => Ok(ProcessOutput {
                    exit_code: Some(0),
                    stdout: r#"{"loggedIn":true}"#.to_owned(),
                    stderr: String::new(),
                }),
                _ => ok("9.9.9\n"),
            },
        ));
        RuntimeRegistry::with_default_runtimes(&runner)
    }

    #[test]
    fn registers_the_runtimes_that_can_execute() {
        let registry = registry();

        for id in ["opencode", "claude", "codex", "gemini", "antigravity"] {
            let info = registry.find(id).unwrap().info();
            assert!(info.capabilities.non_interactive_execution, "{id}");
        }
        assert!(registry.find("nope").is_none());
    }

    #[test]
    fn inspects_every_runtime_with_structured_availability() {
        let statuses = registry().inspect_all();

        let summary: Vec<_> = statuses
            .iter()
            .map(|s| (s.runtime.id.as_str(), s.availability))
            .collect();
        assert_eq!(
            summary,
            [
                ("opencode", Availability::Ready),
                ("claude", Availability::Ready),
                ("codex", Availability::NotInstalled),
                ("gemini", Availability::NotInstalled),
                ("antigravity", Availability::NotInstalled),
            ]
        );
        assert_eq!(statuses[1].authentication.state, AuthState::Authenticated);
    }

    #[test]
    fn model_discovery_follows_the_runtime_capability() {
        let statuses = registry().inspect_all();

        // Supported: models come from the runtime.
        assert!(statuses[0].runtime.capabilities.model_discovery);
        assert_eq!(statuses[0].model_discovery, ModelDiscovery::Discovered);
        assert_eq!(statuses[0].available_models.len(), 1);
        // Not supported: nothing is invented and the runtime explains what to enter.
        assert!(!statuses[1].runtime.capabilities.model_discovery);
        assert_eq!(statuses[1].model_discovery, ModelDiscovery::Unsupported);
        assert_eq!(statuses[1].available_models, []);
        assert!(statuses[1].runtime.model_hint.is_some());
        // Not installed: nothing to discover.
        assert_eq!(statuses[2].model_discovery, ModelDiscovery::Unavailable);
    }
}
