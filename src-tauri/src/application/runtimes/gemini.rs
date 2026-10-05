use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use super::{
    cli, Detection, ModelRuntime, RuntimeError, RuntimeEvent, RuntimeOutput, RuntimeRequest,
};
use crate::application::process::{ProcessContext, ProcessOutput, ProcessRunner, ProcessSpec};
use crate::domain::runtime::{
    AuthKind, AuthState, Authentication, ModelInfo, ProviderRef, RuntimeCapabilities, RuntimeInfo,
    Transport,
};
use crate::domain::security::ToolAccess;
use crate::domain::usage::{UsageMetrics, UsageSource};

const PROGRAM: &str = "gemini";
/// Idle limit: silence from the CLI (long thinking or a slow tool) for this long ends the run.
const RUN_TIMEOUT: Duration = Duration::from_secs(900);

/// Runtime for the Gemini CLI, using the user's existing sign-in or API key.
///
/// - version: `gemini --version`
/// - run: `gemini --output-format stream-json --model <model> --approval-mode <mode>
///   --skip-trust --prompt=<prompt>`. The CLI streams one JSON event per line: `message` (the
///   assistant's text in `delta` pieces), `tool_use` / `tool_result`, `error`, and a final
///   `result` with the token statistics
/// - models: the CLI has no model-list command, only `--model`, so discovery is unsupported
///   rather than faked
/// - sign-in: no status command either (it may be a Google login, an API key or Vertex), so it
///   is not reported (`Unknown`); a run that fails for it is classified from its output.
///
/// Without `--approval-mode yolo` a headless run cannot use the tools that need approval (file
/// edits, shell), so `default` leaves it reading the project; with `allow_edits` the run uses
/// `auto_edit`. `--yolo` is never used. `--skip-trust` is needed because a headless run in a
/// folder the CLI has not been told to trust is refused; the folder is the project the user
/// chose in Atlas (or its isolated worktree).
pub struct GeminiRuntime {
    runner: Arc<dyn ProcessRunner>,
}

impl GeminiRuntime {
    pub fn new(runner: Arc<dyn ProcessRunner>) -> Self {
        Self { runner }
    }
}

impl ModelRuntime for GeminiRuntime {
    fn info(&self) -> RuntimeInfo {
        RuntimeInfo {
            id: "gemini".to_owned(),
            name: "Gemini CLI".to_owned(),
            provider: ProviderRef {
                id: "google".to_owned(),
                name: "Google".to_owned(),
            },
            transport: Transport::Cli,
            capabilities: RuntimeCapabilities {
                model_discovery: false,
                streaming: false,
                system_prompt: false,
                non_interactive_execution: true,
                authentication: vec![AuthKind::CliSession],
                usage_metrics: true,
                cost_metrics: false,
                quota_metrics: false,
                // Runs attached to a terminal; `--prompt` is the only way in, so it is read-only.
                interactive_terminal: true,
                interrupt: true,
                terminal_input: false,
                terminal_resize: true,
                // No flag removes the tools, so the prompt cannot be the whole of what it sees.
                text_only: false,
                // `--approval-mode auto_edit` approves file edits; shell commands stay refused.
                file_edit: true,
                // The CLI's own approval rules and policy files decide, not Atlas, and it has
                // web tools. Atlas cannot narrow that from outside, so it is reported as it is.
                tool_access: ToolAccess {
                    filesystem_write: true,
                    process_execution: true,
                    network: true,
                },
            },
            model_hint: Some("gemini_model".to_owned()),
        }
    }

    fn detect(&self) -> Detection {
        let installed = self.runner.locate(PROGRAM).is_some();
        Detection {
            installed,
            version: if installed {
                cli::version(self.runner.as_ref(), PROGRAM)
            } else {
                None
            },
            unavailable: None,
            authentication: Authentication {
                kind: Some(AuthKind::CliSession),
                state: AuthState::Unknown,
            },
            notice: None,
        }
    }

    fn list_models(&self) -> Result<Vec<ModelInfo>, RuntimeError> {
        Err(RuntimeError::Unavailable(
            "The Gemini CLI does not expose model discovery.".to_owned(),
        ))
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

        let approval = if request.allow_edits && !request.text_only {
            "auto_edit"
        } else {
            "default"
        };
        let args = [
            "--output-format",
            "stream-json",
            "--model",
            &request.model_id,
            "--approval-mode",
            approval,
            "--skip-trust",
        ]
        .map(str::to_owned)
        .to_vec();
        let delivery = cli::deliver_prompt_option(
            self.runner.as_ref(),
            PROGRAM,
            "--prompt",
            args,
            &request.prompt.combined(),
        )?;
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
        let stream = GeminiStream::default();
        let output = cli::execute(self.runner.as_ref(), &spec, progress, &|line| {
            if let Some(event) = stream.event(line) {
                progress(event);
            }
        })?;
        parse_run_output(&output, &request.model_id)
    }
}

/// Turns Gemini's `stream-json` lines into live events: the assistant's text as it is written,
/// and tool calls (a `tool_result` carries only the call's id, so the name is remembered).
#[derive(Default)]
struct GeminiStream {
    running_tools: RefCell<HashMap<String, String>>,
}

impl GeminiStream {
    fn event(&self, line: &str) -> Option<RuntimeEvent> {
        let value: Value = serde_json::from_str(line.trim()).ok()?;
        match value["type"].as_str()? {
            "message" if value["role"] == "assistant" => {
                let text = value["content"].as_str().filter(|text| !text.is_empty())?;
                Some(RuntimeEvent::Output(text.to_owned()))
            }
            "tool_use" => {
                let name = value["tool_name"].as_str()?.to_owned();
                if let Some(id) = value["tool_id"].as_str() {
                    self.running_tools
                        .borrow_mut()
                        .insert(id.to_owned(), name.clone());
                }
                Some(RuntimeEvent::ToolStarted(name))
            }
            "tool_result" => self
                .running_tools
                .borrow_mut()
                .remove(value["tool_id"].as_str()?)
                .map(RuntimeEvent::ToolCompleted),
            _ => None,
        }
    }
}

/// Token counts exactly as the `result` event's `stats` report them; anything left out stays
/// `None`. "Input" is every token sent to the model, cached ones included.
fn usage_of(stats: &Value) -> Option<UsageMetrics> {
    let part = |key: &str| stats[key].as_u64();
    let input_tokens = part("input_tokens");
    let output_tokens = part("output_tokens");
    if input_tokens.is_none() && output_tokens.is_none() {
        return None;
    }
    let total_tokens = part("total_tokens").or(match (input_tokens, output_tokens) {
        (Some(i), Some(o)) => Some(i.saturating_add(o)),
        _ => None,
    });
    Some(UsageMetrics {
        input_tokens,
        output_tokens,
        total_tokens,
        // Not read from Gemini's `stats`: nothing is claimed.
        cached_input_tokens: None,
        cost: None,
        currency: None,
        source: UsageSource::RuntimeReported,
    })
}

/// Google has ended Gemini CLI sign-in for individual accounts in favour of Antigravity. That is
/// not a sign-in problem the user can fix by signing in again, so it is told apart.
fn failure(details: &str, model_id: &str) -> RuntimeError {
    let lower = details.to_lowercase();
    if lower.contains("ineligibletier") || lower.contains("no longer supported for gemini") {
        RuntimeError::Unavailable(
            "Google no longer supports the Gemini CLI for this account (individual accounts \
             moved to Antigravity). Use the Antigravity runtime, or sign the Gemini CLI in with \
             an API key or a Workspace account."
                .to_owned(),
        )
    } else if lower.contains("not running in a trusted directory") {
        RuntimeError::Unavailable("The Gemini CLI does not trust this folder.".to_owned())
    } else {
        cli::classify_failure(details, model_id)
    }
}

/// Reads the events of a finished run. The answer is every assistant `message` in order; the
/// `result` event closes the run with its status and statistics.
fn parse_run_output(output: &ProcessOutput, model_id: &str) -> Result<RuntimeOutput, RuntimeError> {
    let events: Vec<Value> = output
        .stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line.trim()).ok())
        .collect();
    let result = events.iter().rev().find(|value| value["type"] == "result");
    let error_message = || {
        events
            .iter()
            .rev()
            .find(|value| value["type"] == "error")
            .and_then(|value| value["message"].as_str())
            .map(str::to_owned)
    };
    let Some(result) = result else {
        let details = cli::diagnostics(output);
        return if output.exit_code == Some(0) && details.is_empty() {
            Err(RuntimeError::UnexpectedResponse(
                "Gemini did not return a result event.".to_owned(),
            ))
        } else {
            Err(failure(&details, model_id))
        };
    };
    if result["status"].as_str().is_some_and(|s| s != "success") || output.exit_code != Some(0) {
        let details = result["error"]["message"]
            .as_str()
            .map(str::to_owned)
            .or_else(error_message)
            .filter(|details| !details.is_empty())
            .unwrap_or_else(|| cli::diagnostics(output));
        return Err(failure(&details, model_id));
    }

    let text: String = events
        .iter()
        .filter(|value| value["type"] == "message" && value["role"] == "assistant")
        .filter_map(|value| value["content"].as_str())
        .collect();
    let text = text.trim().to_owned();
    if text.is_empty() {
        return Err(RuntimeError::UnexpectedResponse(
            "Gemini returned an empty result.".to_owned(),
        ));
    }

    let stats = &result["stats"];
    let mut metadata = BTreeMap::new();
    if let Some(ms) = stats["duration_ms"].as_u64() {
        metadata.insert("durationMs".to_owned(), ms.to_string());
    }
    if let Some(calls) = stats["tool_calls"].as_u64() {
        metadata.insert("toolCalls".to_owned(), calls.to_string());
    }
    Ok(RuntimeOutput {
        text,
        metadata,
        usage: usage_of(stats),
        quota: None,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::application::process::fake::{ok, FakeProcessRunner};
    use crate::application::process::{ProcessError, TerminalRequest};
    use crate::application::prompt::Prompt;
    use crate::application::runtimes::inspect;
    use crate::domain::runtime::{Availability, ModelDiscovery};

    const SUCCESS: &str = concat!(
        r#"{"type":"init","session_id":"s","model":"gemini-2.5-pro"}"#,
        "\n",
        r#"{"type":"message","role":"user","content":"hi"}"#,
        "\n",
        r#"{"type":"message","role":"assistant","content":"Three ","delta":true}"#,
        "\n",
        r#"{"type":"message","role":"assistant","content":"improvements.\n","delta":true}"#,
        "\n",
        r#"{"type":"result","status":"success","stats":{"total_tokens":150,"input_tokens":100,"output_tokens":50,"duration_ms":1234,"tool_calls":1}}"#,
    );

    fn runner(run_output: Result<ProcessOutput, ProcessError>) -> Arc<FakeProcessRunner> {
        Arc::new(FakeProcessRunner::new(
            &["gemini"],
            move |spec| match spec.args[0].as_str() {
                "--version" => ok("0.46.0\n"),
                _ => run_output.clone(),
            },
        ))
    }

    fn request(model: &str) -> RuntimeRequest {
        RuntimeRequest {
            model_id: model.to_owned(),
            prompt: Prompt {
                harness: None,
                task_aware: false,
                skills: None,
                system: "SYS".to_owned(),
                context: "CTX".to_owned(),
                instruction: "INS".to_owned(),
            },
            working_dir: PathBuf::from("/atlas"),
            scope: crate::application::process::ExecutionScope::for_tests(),
            text_only: false,
            allow_edits: false,
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
    fn detects_the_installation_without_inventing_models() {
        let status = inspect(&GeminiRuntime::new(runner(ok(""))));

        assert_eq!(status.availability, Availability::Ready);
        assert_eq!(status.version.as_deref(), Some("0.46.0"));
        assert_eq!(status.model_discovery, ModelDiscovery::Unsupported);
        assert_eq!(status.available_models, []);
        assert!(status.runtime.capabilities.non_interactive_execution);
        assert!(status.runtime.model_hint.is_some());
    }

    #[test]
    fn runs_read_only_in_a_terminal_with_the_prompt_as_an_option_value() {
        let fake = runner(output(0, SUCCESS));

        let result = GeminiRuntime::new(fake.clone())
            .execute(&request("gemini-2.5-pro"), &|_| {})
            .unwrap();

        assert_eq!(result.text, "Three improvements.");
        assert_eq!(result.metadata["durationMs"], "1234");
        assert_eq!(result.metadata["toolCalls"], "1");
        let usage = result.usage.unwrap();
        assert_eq!(usage.input_tokens, Some(100));
        assert_eq!(usage.total_tokens, Some(150));
        let calls = fake.calls.lock().unwrap();
        let run = calls.last().unwrap();
        assert_eq!(run.program, "gemini");
        assert_eq!(
            run.args,
            [
                "--output-format",
                "stream-json",
                "--model",
                "gemini-2.5-pro",
                "--approval-mode",
                "default",
                "--skip-trust",
                &format!("--prompt={}", request("m").prompt.combined()),
            ]
        );
        assert_eq!(run.stdin, None);
        assert_eq!(run.terminal, Some(TerminalRequest::new(false)));
        assert!(!run.args.iter().any(|a| a == "--yolo" || a == "yolo"));
    }

    #[test]
    fn edits_are_approved_only_when_the_execution_may_edit() {
        let fake = runner(output(0, SUCCESS));
        let mut edit = request("m");
        edit.allow_edits = true;

        GeminiRuntime::new(fake.clone())
            .execute(&edit, &|_| {})
            .unwrap();

        let calls = fake.calls.lock().unwrap();
        let args = &calls.last().unwrap().args;
        let at = args.iter().position(|a| a == "--approval-mode").unwrap();
        assert_eq!(args[at + 1], "auto_edit");
    }

    #[test]
    fn streams_assistant_text_and_tool_calls_live() {
        let stream = [
            r#"{"type":"message","role":"user","content":"hi"}"#,
            r#"{"type":"message","role":"assistant","content":"Looking. ","delta":true}"#,
            r#"{"type":"tool_use","tool_name":"read_file","tool_id":"t1","parameters":{}}"#,
            r#"{"type":"tool_result","tool_id":"t1","status":"success","output":"ok"}"#,
            r#"{"type":"message","role":"assistant","content":"Done.","delta":true}"#,
            r#"{"type":"result","status":"success","stats":{}}"#,
        ]
        .join("\n");
        let events = RefCell::new(Vec::new());

        let result = GeminiRuntime::new(runner(output(0, &stream)))
            .execute(&request("m"), &|e| events.borrow_mut().push(e))
            .unwrap();

        let live: Vec<_> = events.borrow().iter().skip(3).cloned().collect();
        assert_eq!(
            live,
            [
                RuntimeEvent::Output("Looking. ".to_owned()),
                RuntimeEvent::ToolStarted("read_file".to_owned()),
                RuntimeEvent::ToolCompleted("read_file".to_owned()),
                RuntimeEvent::Output("Done.".to_owned()),
            ]
        );
        assert_eq!(result.text, "Looking. Done.");
        assert_eq!(result.usage, None);
    }

    #[test]
    fn classifies_failures() {
        let run = |out: Result<ProcessOutput, ProcessError>| {
            GeminiRuntime::new(runner(out)).execute(&request("m1"), &|_| {})
        };

        // The account Google no longer serves through this CLI: not a sign-in to retry.
        assert!(matches!(
            run(Ok(ProcessOutput {
                exit_code: Some(1),
                stdout: "Error authenticating: IneligibleTierError: This client is no longer supported for Gemini Code Assist for individuals.".to_owned(),
                stderr: String::new(),
            })),
            Err(RuntimeError::Unavailable(m)) if m.contains("Antigravity")
        ));
        assert_eq!(
            run(output(
                1,
                r#"{"type":"result","status":"error","error":{"type":"Error","message":"401 Unauthorized: invalid API key"}}"#
            )),
            Err(RuntimeError::AuthenticationRequired)
        );
        assert!(matches!(
            run(output(
                1,
                "{\"type\":\"error\",\"message\":\"429 Too Many Requests\"}\n{\"type\":\"result\",\"status\":\"error\"}"
            )),
            Err(RuntimeError::RateLimited(_))
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

        let result = GeminiRuntime::new(fake.clone()).execute(&request("--yolo"), &|_| {});

        assert!(matches!(result, Err(RuntimeError::InvalidRequest(_))));
        assert_eq!(fake.calls.lock().unwrap().len(), 0);
    }
}
