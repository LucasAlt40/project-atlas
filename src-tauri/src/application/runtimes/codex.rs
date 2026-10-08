use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use super::{
    cli, Detection, ModelRuntime, RuntimeError, RuntimeEvent, RuntimeOutput, RuntimeRequest,
};
use crate::application::process::{ProcessContext, ProcessOutput, ProcessRunner, ProcessSpec};
use crate::domain::mcp::McpSupport;
use crate::domain::runtime::{
    AuthKind, AuthState, Authentication, ModelInfo, ProviderRef, RuntimeCapabilities, RuntimeInfo,
    RuntimeNotice, SystemPromptChannel, Transport,
};
use crate::domain::security::ToolAccess;
use crate::domain::usage::{UsageMetrics, UsageSource};

const PROGRAM: &str = "codex";
/// Idle limit: silence from the CLI (long reasoning or a slow tool) for this long ends the run.
const RUN_TIMEOUT: Duration = Duration::from_secs(900);

/// Runtime for the Codex CLI (`OpenAI`), using the user's existing `ChatGPT` sign-in or API key.
///
/// - version: `codex --version`
/// - sign-in: `codex login status` (exit 0 when signed in)
/// - models: `codex debug models` renders the catalog as JSON; only the entries the CLI itself
///   lists (`visibility: "list"`) are offered
/// - run: `codex exec --json --ephemeral --skip-git-repo-check --sandbox <mode> --model <model>
///   -- <prompt>`. The CLI streams one JSON event per line: `item.started` / `item.completed`
///   (agent messages, command executions, file changes…), `turn.completed` with the token
///   counts, `turn.failed` and `error`
///
/// `exec` never asks for approval, so the sandbox is what bounds it: `read-only` by default, and
/// `workspace-write` (the working directory only) when `allow_edits` is set. The sandbox
/// bypass flag is never used. `--ephemeral` keeps the run from writing session files.
pub struct CodexRuntime {
    runner: Arc<dyn ProcessRunner>,
}

impl CodexRuntime {
    pub fn new(runner: Arc<dyn ProcessRunner>) -> Self {
        Self { runner }
    }

    fn authentication(&self) -> Authentication {
        let state = cli::capture(self.runner.as_ref(), PROGRAM, &["login", "status"], None)
            .ok()
            .map_or(AuthState::Unknown, |output| match output.exit_code {
                Some(0) => AuthState::Authenticated,
                Some(1) => AuthState::Required,
                _ => AuthState::Unknown,
            });
        Authentication {
            kind: Some(AuthKind::CliSession),
            state,
        }
    }
}

impl ModelRuntime for CodexRuntime {
    fn info(&self) -> RuntimeInfo {
        RuntimeInfo {
            id: "codex".to_owned(),
            name: "Codex CLI".to_owned(),
            provider: ProviderRef {
                id: "openai".to_owned(),
                name: "OpenAI".to_owned(),
            },
            transport: Transport::Cli,
            capabilities: RuntimeCapabilities {
                model_discovery: true,
                streaming: false,
                system_prompt: SystemPromptChannel::Unsupported,
                mcp: McpSupport::NotInvestigated,
                non_interactive_execution: true,
                authentication: vec![AuthKind::CliSession],
                usage_metrics: true,
                cost_metrics: false,
                quota_metrics: false,
                // Runs attached to a terminal; the prompt is an argument, so it is read-only.
                interactive_terminal: true,
                interrupt: true,
                terminal_input: false,
                terminal_resize: true,
                // No flag removes the tools: the sandbox only bounds them.
                text_only: false,
                file_edit: true,
                // The sandbox decides what it can reach, not Atlas; reported as it is.
                tool_access: ToolAccess {
                    filesystem_write: true,
                    process_execution: true,
                    network: true,
                },
            },
            model_hint: None,
        }
    }

    fn detect(&self) -> Detection {
        if self.runner.locate(PROGRAM).is_none() {
            return Detection {
                installed: false,
                version: None,
                unavailable: None,
                authentication: Authentication {
                    kind: Some(AuthKind::CliSession),
                    state: AuthState::Unknown,
                },
                notice: None,
            };
        }
        let authentication = self.authentication();
        Detection {
            installed: true,
            version: cli::version(self.runner.as_ref(), PROGRAM),
            unavailable: None,
            notice: (authentication.state == AuthState::Required)
                .then_some(RuntimeNotice::SignInRequired),
            authentication,
        }
    }

    fn list_models(&self) -> Result<Vec<ModelInfo>, RuntimeError> {
        let stdout = cli::probe(self.runner.as_ref(), PROGRAM, &["debug", "models"], None)
            .map_err(|_| {
                RuntimeError::Unavailable("Codex could not list its models.".to_owned())
            })?;
        parse_models(&stdout)
    }

    fn execute(
        &self,
        request: &RuntimeRequest,
        progress: &dyn Fn(RuntimeEvent),
    ) -> Result<RuntimeOutput, RuntimeError> {
        progress(RuntimeEvent::Starting);
        cli::validate_model_id(&request.model_id)?;
        if self.runner.locate(PROGRAM).is_none() {
            return Err(RuntimeError::NotInstalled);
        }

        let sandbox = if request.allow_edits && !request.text_only {
            "workspace-write"
        } else {
            "read-only"
        };
        let args = [
            "exec",
            "--json",
            "--ephemeral",
            "--skip-git-repo-check",
            "--sandbox",
            sandbox,
            "--model",
            &request.model_id,
        ]
        .map(str::to_owned)
        .to_vec();
        let delivery = cli::deliver_prompt(
            self.runner.as_ref(),
            PROGRAM,
            args,
            self.delivery(request).into_payload(),
            false,
        );
        let spec = ProcessSpec {
            program: PROGRAM.to_owned(),
            args: delivery.args,
            stdin: delivery.stdin,
            cwd: Some(request.working_dir.clone()),
            env: Vec::new(),
            timeout: RUN_TIMEOUT,
            context: ProcessContext::Runtime(request.scope.clone()),
            terminal: delivery.terminal,
        };
        let output = cli::execute(self.runner.as_ref(), &spec, progress, &|line| {
            if let Some(event) = stream_event(line) {
                progress(event);
            }
        })?;
        parse_run_output(&output, &request.model_id)
    }
}

/// The models `codex debug models` lists for people to pick.
fn parse_models(stdout: &str) -> Result<Vec<ModelInfo>, RuntimeError> {
    let catalog: Value = serde_json::from_str(stdout.trim())
        .map_err(|_| RuntimeError::UnexpectedResponse("Codex returned no model catalog.".into()))?;
    Ok(catalog["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|model| model["visibility"] == "list")
        .filter_map(|model| {
            let id = model["slug"].as_str()?;
            Some(ModelInfo {
                id: id.to_owned(),
                name: model["display_name"].as_str().unwrap_or(id).to_owned(),
            })
        })
        .collect())
}

/// What a tool item is called in the live view: the command for a shell call, otherwise the
/// kind of item (`file_change`, `web_search`…).
fn tool_name(item: &Value) -> Option<String> {
    let kind = item["type"].as_str()?;
    if matches!(kind, "agent_message" | "reasoning" | "error") {
        return None;
    }
    Some(match kind {
        "command_execution" => item["command"].as_str().unwrap_or(kind).to_owned(),
        "mcp_tool_call" => item["tool"].as_str().unwrap_or(kind).to_owned(),
        _ => kind.to_owned(),
    })
}

/// Turns one `--json` line into a live event: each agent message as it is completed, and tool
/// items as they start and finish.
fn stream_event(line: &str) -> Option<RuntimeEvent> {
    let value: Value = serde_json::from_str(line.trim()).ok()?;
    let item = &value["item"];
    match (value["type"].as_str()?, item["type"].as_str()?) {
        ("item.completed", "agent_message") => {
            let text = item["text"].as_str().filter(|text| !text.is_empty())?;
            Some(RuntimeEvent::Output(format!("{}\n\n", text.trim_end())))
        }
        ("item.started", _) => tool_name(item).map(RuntimeEvent::ToolStarted),
        ("item.completed", _) => tool_name(item).map(RuntimeEvent::ToolCompleted),
        _ => None,
    }
}

/// Token counts exactly as `turn.completed` reports them. `OpenAI`'s "input" already includes the
/// cached tokens.
fn usage_of(usage: &Value) -> Option<UsageMetrics> {
    let part = |key: &str| usage[key].as_u64();
    let input_tokens = part("input_tokens");
    let output_tokens = part("output_tokens");
    if input_tokens.is_none() && output_tokens.is_none() {
        return None;
    }
    Some(UsageMetrics {
        input_tokens,
        output_tokens,
        total_tokens: match (input_tokens, output_tokens) {
            (Some(i), Some(o)) => Some(i.saturating_add(o)),
            _ => None,
        },
        cached_input_tokens: part("cached_input_tokens"),
        cost: None,
        currency: None,
        source: UsageSource::RuntimeReported,
    })
}

/// Reads the events of a finished run. The answer is the last agent message (earlier ones are
/// narration); `turn.completed` closes a good run, `turn.failed` / `error` a bad one.
fn parse_run_output(output: &ProcessOutput, model_id: &str) -> Result<RuntimeOutput, RuntimeError> {
    let events: Vec<Value> = output
        .stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line.trim()).ok())
        .collect();
    let completed = events.iter().rev().find(|e| e["type"] == "turn.completed");
    let Some(completed) = completed.filter(|_| output.exit_code == Some(0)) else {
        let details = events
            .iter()
            .rev()
            .find_map(|e| match e["type"].as_str()? {
                "turn.failed" => e["error"]["message"].as_str(),
                "error" => e["message"].as_str(),
                _ => None,
            })
            .map(str::to_owned)
            .filter(|details| !details.is_empty())
            .unwrap_or_else(|| cli::diagnostics(output));
        return if details.is_empty() && output.exit_code == Some(0) {
            Err(RuntimeError::UnexpectedResponse(
                "Codex did not complete the turn.".to_owned(),
            ))
        } else {
            Err(cli::classify_failure(&details, model_id))
        };
    };

    let text = events
        .iter()
        .rev()
        .filter(|e| e["type"] == "item.completed" && e["item"]["type"] == "agent_message")
        .find_map(|e| e["item"]["text"].as_str())
        .map(|text| text.trim().to_owned())
        .unwrap_or_default();
    if text.is_empty() {
        return Err(RuntimeError::UnexpectedResponse(
            "Codex returned an empty result.".to_owned(),
        ));
    }

    let mut metadata = BTreeMap::new();
    let tool_calls = events
        .iter()
        .filter(|e| e["type"] == "item.completed" && tool_name(&e["item"]).is_some())
        .count();
    metadata.insert("toolCalls".to_owned(), tool_calls.to_string());
    if let Some(reasoning) = completed["usage"]["reasoning_output_tokens"].as_u64() {
        metadata.insert("reasoningTokens".to_owned(), reasoning.to_string());
    }
    Ok(RuntimeOutput {
        text,
        metadata,
        usage: usage_of(&completed["usage"]),
        quota: None,
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::path::PathBuf;

    use super::*;
    use crate::application::process::fake::{ok, FakeProcessRunner};
    use crate::application::process::{ProcessError, TerminalRequest};
    use crate::application::prompt::Prompt;
    use crate::application::runtimes::inspect;
    use crate::domain::runtime::{Availability, ModelDiscovery};

    const SUCCESS: &str = concat!(
        r#"{"type":"thread.started","thread_id":"t"}"#,
        "\n",
        r#"{"type":"turn.started"}"#,
        "\n",
        r#"{"type":"item.completed","item":{"id":"i0","type":"agent_message","text":"Looking."}}"#,
        "\n",
        r#"{"type":"item.started","item":{"id":"i1","type":"command_execution","command":"/bin/zsh -lc ls","status":"in_progress"}}"#,
        "\n",
        r#"{"type":"item.completed","item":{"id":"i1","type":"command_execution","command":"/bin/zsh -lc ls","exit_code":0,"status":"completed"}}"#,
        "\n",
        r#"{"type":"item.completed","item":{"id":"i2","type":"agent_message","text":"Three improvements.\n"}}"#,
        "\n",
        r#"{"type":"turn.completed","usage":{"input_tokens":100,"cached_input_tokens":40,"output_tokens":50,"reasoning_output_tokens":10}}"#,
    );

    const CATALOG: &str = r#"{"models":[
        {"slug":"gpt-6-luna","display_name":"GPT-6-Luna","visibility":"list"},
        {"slug":"gpt-reserve","display_name":"GPT-Reserve","visibility":"hide"},
        {"slug":"gpt-5.6-terra","visibility":"list"}]}"#;

    fn runner(run_output: Result<ProcessOutput, ProcessError>) -> Arc<FakeProcessRunner> {
        Arc::new(FakeProcessRunner::new(&["codex"], move |spec| {
            match spec.args[0].as_str() {
                "--version" => ok("codex-cli 0.160.0\n"),
                "login" => ok("Logged in using ChatGPT\n"),
                "debug" => ok(CATALOG),
                _ => run_output.clone(),
            }
        }))
    }

    fn request(model: &str) -> RuntimeRequest {
        RuntimeRequest {
            model_id: model.to_owned(),
            prompt: Prompt {
                harness: None,
                task_aware: false,
                skills: None,
                rules: None,
                system: "SYS".to_owned(),
                context: "CTX".to_owned(),
                instruction: "INS".to_owned(),
            },
            working_dir: PathBuf::from("/atlas"),
            scope: crate::application::process::ExecutionScope::for_tests(),
            text_only: false,
            allow_edits: false,
            mcp: None,
        }
    }

    #[allow(clippy::unnecessary_wraps)]
    fn output(exit_code: i32, stdout: &str) -> Result<ProcessOutput, ProcessError> {
        Ok(ProcessOutput {
            exit_code: Some(exit_code),
            stdout: stdout.to_owned(),
            stderr: String::new(),
        })
    }

    #[test]
    fn detects_the_sign_in_and_discovers_the_listed_models() {
        let status = inspect(&CodexRuntime::new(runner(ok(""))));

        assert_eq!(status.availability, Availability::Ready);
        assert_eq!(status.version.as_deref(), Some("codex-cli 0.160.0"));
        assert_eq!(status.authentication.state, AuthState::Authenticated);
        assert_eq!(status.model_discovery, ModelDiscovery::Discovered);
        let models: Vec<_> = status
            .available_models
            .iter()
            .map(|m| (m.id.as_str(), m.name.as_str()))
            .collect();
        assert_eq!(
            models,
            [
                ("gpt-6-luna", "GPT-6-Luna"),
                ("gpt-5.6-terra", "gpt-5.6-terra")
            ]
        );
    }

    #[test]
    fn a_signed_out_cli_asks_for_sign_in() {
        let signed_out = Arc::new(FakeProcessRunner::new(&["codex"], |spec| {
            match spec.args[0].as_str() {
                "login" => Ok(ProcessOutput {
                    exit_code: Some(1),
                    stdout: String::new(),
                    stderr: "Not logged in\n".to_owned(),
                }),
                _ => ok("codex-cli 0.160.0\n"),
            }
        }));

        let status = inspect(&CodexRuntime::new(signed_out));

        assert_eq!(status.availability, Availability::AuthenticationRequired);
    }

    #[test]
    fn runs_read_only_in_a_sandbox_with_the_prompt_after_the_options() {
        let fake = runner(output(0, SUCCESS));

        let result = CodexRuntime::new(fake.clone())
            .execute(&request("gpt-6-luna"), &|_| {})
            .unwrap();

        assert_eq!(result.text, "Three improvements.");
        assert_eq!(result.metadata["toolCalls"], "1");
        assert_eq!(result.metadata["reasoningTokens"], "10");
        let usage = result.usage.unwrap();
        assert_eq!(usage.input_tokens, Some(100));
        assert_eq!(usage.cached_input_tokens, Some(40));
        assert_eq!(usage.total_tokens, Some(150));
        let calls = fake.calls.lock().unwrap();
        let run = calls.last().unwrap();
        assert_eq!(run.program, "codex");
        assert_eq!(
            run.args,
            [
                "exec",
                "--json",
                "--ephemeral",
                "--skip-git-repo-check",
                "--sandbox",
                "read-only",
                "--model",
                "gpt-6-luna",
                "--",
                &request("m").prompt.combined(),
            ]
        );
        assert_eq!(run.terminal, Some(TerminalRequest::new(false)));
        assert!(!run.args.iter().any(|a| a.contains("dangerously")));
    }

    #[test]
    fn edits_are_allowed_only_when_the_execution_may_edit() {
        let fake = runner(output(0, SUCCESS));
        let mut edit = request("m");
        edit.allow_edits = true;

        CodexRuntime::new(fake.clone())
            .execute(&edit, &|_| {})
            .unwrap();

        let calls = fake.calls.lock().unwrap();
        let args = &calls.last().unwrap().args;
        let at = args.iter().position(|a| a == "--sandbox").unwrap();
        assert_eq!(args[at + 1], "workspace-write");
    }

    #[test]
    fn streams_messages_and_tool_calls_live() {
        let events = RefCell::new(Vec::new());

        CodexRuntime::new(runner(output(0, SUCCESS)))
            .execute(&request("m"), &|e| events.borrow_mut().push(e))
            .unwrap();

        let live: Vec<_> = events.borrow().iter().skip(3).cloned().collect();
        assert_eq!(
            live,
            [
                RuntimeEvent::Output("Looking.\n\n".to_owned()),
                RuntimeEvent::ToolStarted("/bin/zsh -lc ls".to_owned()),
                RuntimeEvent::ToolCompleted("/bin/zsh -lc ls".to_owned()),
                RuntimeEvent::Output("Three improvements.\n\n".to_owned()),
            ]
        );
    }

    #[test]
    fn classifies_failures() {
        let run = |out: Result<ProcessOutput, ProcessError>| {
            CodexRuntime::new(runner(out)).execute(&request("m1"), &|_| {})
        };

        assert_eq!(
            run(output(
                1,
                r#"{"type":"turn.failed","error":{"message":"401 Unauthorized: invalid API key"}}"#
            )),
            Err(RuntimeError::AuthenticationRequired)
        );
        assert!(matches!(
            run(output(
                1,
                r#"{"type":"error","message":"429 Too Many Requests"}"#
            )),
            Err(RuntimeError::RateLimited(_))
        ));
        assert!(matches!(
            run(output(
                1,
                r#"{"type":"turn.failed","error":{"message":"The model `m1` does not exist"}}"#
            )),
            Err(RuntimeError::ExecutionFailed(_))
        ));
        assert!(matches!(
            run(output(0, "not json")),
            Err(RuntimeError::ExecutionFailed(_) | RuntimeError::UnexpectedResponse(_))
        ));
        assert_eq!(run(Err(ProcessError::Timeout)), Err(RuntimeError::Timeout));
    }

    #[test]
    fn refuses_model_ids_that_could_be_options() {
        let fake = runner(output(0, SUCCESS));

        let result = CodexRuntime::new(fake.clone()).execute(&request("--yolo"), &|_| {});

        assert!(matches!(result, Err(RuntimeError::InvalidRequest(_))));
        assert_eq!(fake.calls.lock().unwrap().len(), 0);
    }
}
