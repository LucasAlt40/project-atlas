mod claude;
mod cli;
mod detect_only;
mod opencode;

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use super::process::ProcessRunner;
use super::prompt::Prompt;
use crate::domain::execution::{ExecutionFailure, FailureKind};
use crate::domain::runtime::{
    Authentication, Availability, ModelDiscovery, ModelInfo, RuntimeInfo, RuntimeNotice,
    RuntimeStatus,
};
use crate::domain::usage::{QuotaInfo, UsageMetrics};

pub use claude::ClaudeRuntime;
pub use opencode::OpenCodeRuntime;

/// Everything a runtime needs to answer one task. The runtime does not decide what the
/// personality means: it only delivers the prompt.
#[derive(Debug, Clone)]
pub struct RuntimeRequest {
    pub model_id: String,
    pub prompt: Prompt,
    pub working_dir: PathBuf,
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
}

/// Runtime errors, normalized so the UI never needs to know which runtime produced them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    NotInstalled,
    Unavailable(String),
    AuthenticationRequired,
    ModelUnavailable(String),
    Timeout,
    ExecutionFailed(String),
    InvalidRequest(String),
    UnexpectedResponse(String),
}

impl RuntimeError {
    /// A message that is fine to show as the primary error in the UI.
    pub fn user_message(&self) -> String {
        match self {
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
            Self::Timeout => "The runtime did not respond in time.".to_owned(),
            Self::ExecutionFailed(_) => "The runtime could not complete the request.".to_owned(),
            Self::Unavailable(reason) | Self::InvalidRequest(reason) => reason.clone(),
            Self::UnexpectedResponse(_) => {
                "The runtime returned a response Atlas could not understand.".to_owned()
            }
        }
    }

    fn kind(&self) -> FailureKind {
        match self {
            Self::NotInstalled => FailureKind::RuntimeNotInstalled,
            Self::Unavailable(_) => FailureKind::RuntimeUnavailable,
            Self::AuthenticationRequired => FailureKind::AuthenticationRequired,
            Self::ModelUnavailable(_) => FailureKind::ModelUnavailable,
            Self::Timeout => FailureKind::Timeout,
            Self::ExecutionFailed(_) => FailureKind::ExecutionFailed,
            Self::InvalidRequest(_) => FailureKind::InvalidRequest,
            Self::UnexpectedResponse(_) => FailureKind::UnexpectedResponse,
        }
    }

    pub fn into_failure(self) -> ExecutionFailure {
        let details = match &self {
            Self::ExecutionFailed(details) | Self::UnexpectedResponse(details)
                if !details.is_empty() =>
            {
                Some(details.clone())
            }
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

/// All known runtimes, in display order.
pub struct RuntimeRegistry {
    runtimes: Vec<Arc<dyn ModelRuntime>>,
}

impl RuntimeRegistry {
    pub fn new(runtimes: Vec<Arc<dyn ModelRuntime>>) -> Self {
        Self { runtimes }
    }

    /// The runtimes Atlas knows about. `OpenCode` and Claude CLI can run tasks; the rest are
    /// detected only.
    pub fn with_default_runtimes(runner: &Arc<dyn ProcessRunner>) -> Self {
        let detect_only = |id: &str, name: &str, provider: (&str, &str), program: &str| {
            Arc::new(detect_only::DetectOnlyRuntime::new(
                id,
                name,
                provider,
                program,
                runner.clone(),
            )) as Arc<dyn ModelRuntime>
        };
        Self::new(vec![
            Arc::new(OpenCodeRuntime::new(runner.clone())),
            Arc::new(ClaudeRuntime::new(runner.clone())),
            detect_only("codex", "Codex CLI", ("openai", "OpenAI"), "codex"),
            detect_only("gemini", "Gemini CLI", ("google", "Google"), "gemini"),
            detect_only(
                "antigravity",
                "Antigravity CLI",
                ("google", "Google"),
                "antigravity",
            ),
        ])
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

#[cfg(test)]
pub mod fake {
    use std::sync::Mutex;

    use super::{
        Detection, ModelRuntime, RuntimeError, RuntimeEvent, RuntimeOutput, RuntimeRequest,
    };
    use crate::domain::runtime::{
        AuthKind, AuthState, Authentication, ModelInfo, ProviderRef, RuntimeCapabilities,
        RuntimeInfo, Transport,
    };

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
    }

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
            }
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

        pub fn with_barrier(mut self, barrier: std::sync::Arc<std::sync::Barrier>) -> Self {
            self.barrier = Some(barrier);
            self
        }
    }

    impl ModelRuntime for FakeRuntime {
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
                    system_prompt: false,
                    non_interactive_execution: true,
                    authentication: vec![AuthKind::CliSession],
                    usage_metrics: true,
                    cost_metrics: true,
                    quota_metrics: false,
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
            for chunk in &self.chunks {
                progress(RuntimeEvent::Output(chunk.clone()));
            }
            if let Some(barrier) = &self.barrier {
                barrier.wait();
            }
            self.requests.lock().unwrap().push(request.clone());
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
    fn registers_opencode_and_claude_that_can_execute() {
        let registry = registry();

        for id in ["opencode", "claude"] {
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
