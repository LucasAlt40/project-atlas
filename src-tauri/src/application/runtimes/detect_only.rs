use std::sync::Arc;

use super::{
    cli, Detection, ModelRuntime, RuntimeError, RuntimeEvent, RuntimeOutput, RuntimeRequest,
};
use crate::application::process::ProcessRunner;
use crate::domain::runtime::{
    AuthState, Authentication, ModelInfo, ProviderRef, RuntimeCapabilities, RuntimeInfo,
    RuntimeNotice, Transport,
};
use crate::domain::security::ToolAccess;

/// A CLI runtime Atlas can detect but cannot run tasks with yet. It does not pretend to
/// know the tool's models or sign-in state.
pub struct DetectOnlyRuntime {
    info: RuntimeInfo,
    program: String,
    runner: Arc<dyn ProcessRunner>,
}

impl DetectOnlyRuntime {
    pub fn new(
        id: &str,
        name: &str,
        provider: (&str, &str),
        program: &str,
        runner: Arc<dyn ProcessRunner>,
    ) -> Self {
        Self {
            info: RuntimeInfo {
                id: id.to_owned(),
                name: name.to_owned(),
                provider: ProviderRef {
                    id: provider.0.to_owned(),
                    name: provider.1.to_owned(),
                },
                transport: Transport::Cli,
                capabilities: RuntimeCapabilities {
                    model_discovery: false,
                    streaming: false,
                    system_prompt: false,
                    non_interactive_execution: false,
                    authentication: Vec::new(),
                    usage_metrics: false,
                    cost_metrics: false,
                    quota_metrics: false,
                    interactive_terminal: false,
                    interrupt: false,
                    terminal_input: false,
                    terminal_resize: false,
                    text_only: false,
                    file_edit: false,
                    tool_access: ToolAccess::NONE,
                },
                model_hint: None,
            },
            program: program.to_owned(),
            runner,
        }
    }
}

impl ModelRuntime for DetectOnlyRuntime {
    fn info(&self) -> RuntimeInfo {
        self.info.clone()
    }

    fn detect(&self) -> Detection {
        let installed = self.runner.locate(&self.program).is_some();
        Detection {
            installed,
            version: if installed {
                cli::version(self.runner.as_ref(), &self.program)
            } else {
                None
            },
            unavailable: None,
            authentication: Authentication {
                kind: None,
                state: AuthState::Unknown,
            },
            notice: installed.then_some(RuntimeNotice::ExecutionNotSupported),
        }
    }

    fn list_models(&self) -> Result<Vec<ModelInfo>, RuntimeError> {
        Ok(Vec::new())
    }

    fn execute(
        &self,
        _request: &RuntimeRequest,
        _progress: &dyn Fn(RuntimeEvent),
    ) -> Result<RuntimeOutput, RuntimeError> {
        Err(RuntimeError::Unavailable(
            "Atlas cannot run tasks with this runtime yet.".to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::process::fake::{ok, FakeProcessRunner};
    use crate::application::runtimes::inspect;
    use crate::domain::runtime::{Availability, ModelDiscovery};

    #[test]
    fn reports_version_and_unsupported_model_discovery() {
        let runner = Arc::new(FakeProcessRunner::new(&["codex"], |_| {
            ok("codex-cli 0.1\n")
        }));
        let runtime =
            DetectOnlyRuntime::new("codex", "Codex CLI", ("openai", "OpenAI"), "codex", runner);

        let status = inspect(&runtime);

        assert_eq!(status.availability, Availability::Ready);
        assert_eq!(status.version.as_deref(), Some("codex-cli 0.1"));
        assert_eq!(status.model_discovery, ModelDiscovery::Unsupported);
        assert!(!status.runtime.capabilities.non_interactive_execution);
        assert!(matches!(
            runtime.execute(
                &RuntimeRequest {
                    model_id: "m".to_owned(),
                    prompt: crate::application::prompt::Prompt {
                        harness: None,
                        task_aware: false,
                        system: String::new(),
                        context: String::new(),
                        instruction: String::new(),
                    },
                    working_dir: std::path::PathBuf::new(),
                    scope: crate::application::process::ExecutionScope::for_tests(),
                    text_only: false,
                    allow_edits: false,
                },
                &|_| {}
            ),
            Err(RuntimeError::Unavailable(_))
        ));
    }

    #[test]
    fn reports_missing_tools_without_running_anything() {
        let runner = Arc::new(FakeProcessRunner::new(&[], |_| ok("")));
        let runtime = DetectOnlyRuntime::new(
            "gemini",
            "Gemini CLI",
            ("google", "Google"),
            "gemini",
            runner.clone(),
        );

        let status = inspect(&runtime);

        assert_eq!(status.availability, Availability::NotInstalled);
        assert_eq!(status.model_discovery, ModelDiscovery::Unavailable);
        assert_eq!(runner.calls.lock().unwrap().len(), 0);
    }
}
