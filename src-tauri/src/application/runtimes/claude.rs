use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use super::{
    cli, Detection, ModelRuntime, RuntimeError, RuntimeEvent, RuntimeOutput, RuntimeRequest,
};
use crate::application::process::{ProcessOutput, ProcessRunner, ProcessSpec};
use crate::domain::runtime::{
    AuthKind, AuthState, Authentication, ModelInfo, ProviderRef, RuntimeCapabilities, RuntimeInfo,
    RuntimeNotice, Transport,
};
use crate::domain::usage::{QuotaInfo, QuotaWindow, UsageMetrics, UsageSource};

const PROGRAM: &str = "claude";
const RUN_TIMEOUT: Duration = Duration::from_secs(300);
/// The only tools a Claude run may use: reading and searching the project. Everything that
/// writes or executes (Edit, Write, Bash…) is not available, which is how Atlas's read-only
/// rule is enforced for this runtime.
const READ_ONLY_TOOLS: &str = "Read,Grep,Glob";

/// Runtime for the Claude CLI (Claude Code), using the user's existing sign-in.
///
/// - version: `claude --version`
/// - auth: `claude auth status` (JSON with `loggedIn`; exits 1 when signed out)
/// - run: `claude -p --output-format stream-json --verbose --include-partial-messages --model
///   <model> …`, prompt on stdin. Text arrives token by token as `text_delta` events, tool calls as
///   `tool_use` blocks, and the final answer in the `result` object that ends the stream
/// - models: the CLI has no model-list command, only `--model <alias or full name>`, so
///   discovery is unsupported rather than faked.
///
/// The prompt goes on stdin as one text (even though the CLI also has
/// `--append-system-prompt`): multi-line arguments are rejected for `.cmd` shims on Windows.
pub struct ClaudeRuntime {
    runner: Arc<dyn ProcessRunner>,
}

impl ClaudeRuntime {
    pub fn new(runner: Arc<dyn ProcessRunner>) -> Self {
        Self { runner }
    }

    fn authentication(&self) -> Authentication {
        let state = cli::capture(
            self.runner.as_ref(),
            PROGRAM,
            &["auth", "status", "--json"],
            None,
        )
        .ok()
        .and_then(|output| serde_json::from_str::<Value>(output.stdout.trim()).ok())
        .and_then(|status| status["loggedIn"].as_bool())
        .map_or(AuthState::Unknown, |logged_in| {
            if logged_in {
                AuthState::Authenticated
            } else {
                AuthState::Required
            }
        });
        Authentication {
            kind: Some(AuthKind::CliSession),
            state,
        }
    }
}

impl ModelRuntime for ClaudeRuntime {
    fn info(&self) -> RuntimeInfo {
        RuntimeInfo {
            id: "claude".to_owned(),
            name: "Claude CLI".to_owned(),
            provider: ProviderRef {
                id: "anthropic".to_owned(),
                name: "Anthropic".to_owned(),
            },
            transport: Transport::Cli,
            capabilities: RuntimeCapabilities {
                model_discovery: false,
                streaming: false,
                system_prompt: false,
                non_interactive_execution: true,
                authentication: vec![AuthKind::CliSession],
                usage_metrics: true,
                cost_metrics: true,
                // The CLI forwards the provider's rate-limit windows with each run.
                quota_metrics: true,
            },
            model_hint: Some("claude_alias".to_owned()),
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
        Err(RuntimeError::Unavailable(
            "The Claude CLI does not expose model discovery.".to_owned(),
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

        let spec = ProcessSpec {
            program: PROGRAM.to_owned(),
            args: [
                "-p",
                "--output-format",
                "stream-json",
                "--verbose",
                "--include-partial-messages",
                "--model",
                &request.model_id,
                "--tools",
                READ_ONLY_TOOLS,
                "--no-session-persistence",
            ]
            .map(str::to_owned)
            .to_vec(),
            stdin: Some(request.prompt.combined()),
            cwd: Some(request.working_dir.clone()),
            timeout: RUN_TIMEOUT,
        };
        let stream = ClaudeStream::default();
        let output = cli::execute(self.runner.as_ref(), &spec, progress, &|line| {
            if let Some(event) = stream.event(line) {
                progress(event);
            }
        })?;
        parse_run_output(&output, &request.model_id)
    }
}

/// Turns Claude's `stream-json` lines into live events. Only what the CLI really streams is
/// reported: answer text as it is written (token by token), and tool calls. Its "thinking"
/// content is not exposed, so it is not shown.
#[derive(Default)]
struct ClaudeStream {
    emitted_text: Cell<bool>,
    /// A new text block started after earlier text: separate it with a blank line.
    pending_separator: Cell<bool>,
    running_tool: RefCell<Option<String>>,
}

impl ClaudeStream {
    fn event(&self, line: &str) -> Option<RuntimeEvent> {
        let value: Value = serde_json::from_str(line).ok()?;
        match value["type"].as_str()? {
            "stream_event" => {
                let event = &value["event"];
                match event["type"].as_str()? {
                    "content_block_start" => match event["content_block"]["type"].as_str()? {
                        "text" => {
                            self.pending_separator.set(self.emitted_text.get());
                            None
                        }
                        "tool_use" => {
                            let name = event["content_block"]["name"].as_str()?.to_owned();
                            *self.running_tool.borrow_mut() = Some(name.clone());
                            Some(RuntimeEvent::ToolStarted(name))
                        }
                        _ => None,
                    },
                    "content_block_delta" if event["delta"]["type"] == "text_delta" => {
                        let text = event["delta"]["text"].as_str()?;
                        let separator = if self.pending_separator.replace(false) {
                            "\n\n"
                        } else {
                            ""
                        };
                        self.emitted_text.set(true);
                        Some(RuntimeEvent::Output(format!("{separator}{text}")))
                    }
                    _ => None,
                }
            }
            // A "user" message carries the result of a tool the model called.
            "user" => {
                let has_result = value["message"]["content"]
                    .as_array()?
                    .iter()
                    .any(|block| block["type"] == "tool_result");
                if has_result {
                    self.running_tool
                        .borrow_mut()
                        .take()
                        .map(RuntimeEvent::ToolCompleted)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// Token counts and cost exactly as the `result` object reports them; anything it leaves out
/// stays `None`. "Input" is every token sent to the model (fresh, cache writes and cache reads);
/// the total is input plus output. The cost is the CLI's own figure and is in US dollars.
fn usage_of(result: &Value) -> Option<UsageMetrics> {
    let usage = &result["usage"];
    let part = |key: &str| usage[key].as_u64();
    let input = [
        part("input_tokens"),
        part("cache_creation_input_tokens"),
        part("cache_read_input_tokens"),
    ];
    let input_tokens = input.iter().flatten().copied().reduce(u64::saturating_add);
    let output_tokens = part("output_tokens");
    let total_tokens = match (input_tokens, output_tokens) {
        (Some(i), Some(o)) => Some(i.saturating_add(o)),
        _ => None,
    };
    let cost = result["total_cost_usd"].as_f64();
    if input_tokens.is_none() && output_tokens.is_none() && cost.is_none() {
        return None;
    }
    Some(UsageMetrics {
        input_tokens,
        output_tokens,
        total_tokens,
        cost,
        currency: cost.map(|_| "USD".to_owned()),
        source: UsageSource::RuntimeReported,
    })
}

/// The last quota the stream reported: the provider's usage windows (for example five hours and
/// seven days) with how much of each is used, as a fraction.
fn quota_of(stdout: &str) -> Option<QuotaInfo> {
    let info = stdout
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|value| value["type"] == "rate_limit_event")?;
    let windows = info["rate_limit_info"]["unifiedWindows"].as_object()?;
    let mut windows: Vec<QuotaWindow> = windows
        .iter()
        .filter_map(|(id, window)| {
            Some(QuotaWindow {
                id: id.clone(),
                used_fraction: window["utilization"].as_f64()?,
                resets_at: window["resetsAt"].as_u64(),
            })
        })
        .collect();
    if windows.is_empty() {
        return None;
    }
    windows.sort_by(|a, b| a.id.cmp(&b.id));
    Some(QuotaInfo {
        windows,
        source: UsageSource::ProviderReported,
        observed_at: crate::application::support::now_ms(),
    })
}

/// Reads the `result` object that ends Claude's stream. Its text is the concluding message:
/// narration written before tool calls was already shown live.
fn parse_run_output(output: &ProcessOutput, model_id: &str) -> Result<RuntimeOutput, RuntimeError> {
    let parsed = output
        .stdout
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|value| value["type"] == "result");
    let Some(result) = parsed else {
        return if output.exit_code == Some(0) {
            Err(RuntimeError::UnexpectedResponse(
                "Claude did not return a result object.".to_owned(),
            ))
        } else {
            Err(cli::classify_failure(output.stderr.trim(), model_id))
        };
    };

    let text = result["result"].as_str().unwrap_or("").trim().to_owned();
    if result["is_error"].as_bool().unwrap_or(false) || output.exit_code != Some(0) {
        return Err(cli::classify_failure(&text, model_id));
    }
    if text.is_empty() {
        return Err(RuntimeError::UnexpectedResponse(
            "Claude returned an empty result.".to_owned(),
        ));
    }

    let mut metadata = BTreeMap::new();
    for (key, source) in [
        ("costUsd", "total_cost_usd"),
        ("durationMs", "duration_ms"),
        ("turns", "num_turns"),
    ] {
        if let Some(value) = result[source].as_f64() {
            metadata.insert(key.to_owned(), value.to_string());
        }
    }
    Ok(RuntimeOutput {
        text,
        metadata,
        usage: usage_of(&result),
        quota: quota_of(&output.stdout),
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::path::PathBuf;

    use super::*;
    use crate::application::process::fake::{ok, FakeProcessRunner};
    use crate::application::process::ProcessError;
    use crate::application::prompt::Prompt;
    use crate::application::runtimes::inspect;
    use crate::domain::runtime::{Availability, ModelDiscovery};

    const SUCCESS: &str = r#"{"type":"result","subtype":"success","is_error":false,"duration_ms":1234,"num_turns":1,"result":"Three improvements.\n","total_cost_usd":0.0123}"#;
    const AUTH_ERROR: &str = r#"{"type":"result","subtype":"success","is_error":true,"result":"Failed to authenticate: OAuth session expired and could not be refreshed"}"#;

    fn runner(
        logged_in: bool,
        run_output: Result<ProcessOutput, ProcessError>,
    ) -> Arc<FakeProcessRunner> {
        Arc::new(FakeProcessRunner::new(
            &["claude"],
            move |spec| match spec.args[0].as_str() {
                "--version" => ok("2.1.285 (Claude Code)\n"),
                // Like the real CLI: signed out still prints JSON, but exits 1.
                "auth" => Ok(ProcessOutput {
                    exit_code: Some(i32::from(!logged_in)),
                    stdout: format!(r#"{{"loggedIn":{logged_in}}}"#),
                    stderr: String::new(),
                }),
                _ => run_output.clone(),
            },
        ))
    }

    fn request(model: &str) -> RuntimeRequest {
        RuntimeRequest {
            model_id: model.to_owned(),
            prompt: Prompt {
                system: "SYS".to_owned(),
                context: "CTX".to_owned(),
                instruction: "INS".to_owned(),
            },
            working_dir: PathBuf::from("/atlas"),
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
    fn detects_a_signed_in_installation_without_inventing_models() {
        let status = inspect(&ClaudeRuntime::new(runner(true, ok(""))));

        assert_eq!(status.availability, Availability::Ready);
        assert_eq!(status.version.as_deref(), Some("2.1.285 (Claude Code)"));
        assert_eq!(status.authentication.state, AuthState::Authenticated);
        assert_eq!(status.model_discovery, ModelDiscovery::Unsupported);
        assert_eq!(status.available_models, []);
        assert_eq!(status.runtime.provider.name, "Anthropic");
        assert!(status.runtime.model_hint.is_some());
    }

    #[test]
    fn detects_that_authentication_is_required() {
        let status = inspect(&ClaudeRuntime::new(runner(false, ok(""))));

        assert_eq!(status.availability, Availability::AuthenticationRequired);
        assert_eq!(status.authentication.state, AuthState::Required);
        assert_eq!(status.notice, Some(RuntimeNotice::SignInRequired));
    }

    #[test]
    fn detects_a_missing_installation() {
        let fake = Arc::new(FakeProcessRunner::new(&[], |_| ok("")));

        let status = inspect(&ClaudeRuntime::new(fake.clone()));

        assert_eq!(status.availability, Availability::NotInstalled);
        assert_eq!(fake.calls.lock().unwrap().len(), 0);
    }

    #[test]
    fn runs_read_only_with_the_selected_model_and_prompt_on_stdin() {
        let fake = runner(true, output(0, SUCCESS));

        let result = ClaudeRuntime::new(fake.clone())
            .execute(&request("sonnet"), &|_| {})
            .unwrap();

        assert_eq!(result.text, "Three improvements.");
        assert_eq!(result.metadata["durationMs"], "1234");
        assert_eq!(result.metadata["costUsd"], "0.0123");
        let calls = fake.calls.lock().unwrap();
        let run = calls.last().unwrap();
        assert_eq!(run.program, "claude");
        assert_eq!(
            run.args,
            [
                "-p",
                "--output-format",
                "stream-json",
                "--verbose",
                "--include-partial-messages",
                "--model",
                "sonnet",
                "--tools",
                "Read,Grep,Glob",
                "--no-session-persistence"
            ]
        );
        assert_eq!(run.cwd, Some(PathBuf::from("/atlas")));
        let stdin = run.stdin.as_deref().unwrap();
        assert!(stdin.contains("SYS") && stdin.contains("CTX") && stdin.contains("INS"));
    }

    #[test]
    fn classifies_failures() {
        let run = |out: Result<ProcessOutput, ProcessError>| {
            ClaudeRuntime::new(runner(true, out)).execute(&request("sonnet"), &|_| {})
        };

        assert_eq!(
            run(output(1, AUTH_ERROR)),
            Err(RuntimeError::AuthenticationRequired)
        );
        assert_eq!(
            run(output(
                1,
                r#"{"type":"result","is_error":true,"result":"There's an issue with the selected model (x)."}"#
            )),
            Err(RuntimeError::ModelUnavailable("sonnet".to_owned()))
        );
        assert!(matches!(
            run(output(0, "not json")),
            Err(RuntimeError::UnexpectedResponse(_))
        ));
        assert!(matches!(
            run(Ok(ProcessOutput { exit_code: Some(2), stdout: String::new(), stderr: "boom".to_owned() })),
            Err(RuntimeError::ExecutionFailed(d)) if d == "boom"
        ));
        assert_eq!(run(Err(ProcessError::Timeout)), Err(RuntimeError::Timeout));
    }

    #[test]
    fn refuses_model_ids_that_could_be_options() {
        let fake = runner(true, output(0, SUCCESS));

        let result = ClaudeRuntime::new(fake.clone())
            .execute(&request("--dangerously-skip-permissions"), &|_| {});

        assert!(matches!(result, Err(RuntimeError::InvalidRequest(_))));
        assert_eq!(fake.calls.lock().unwrap().len(), 0);
    }

    fn delta(text: &str) -> String {
        format!(
            r#"{{"type":"stream_event","event":{{"type":"content_block_delta","delta":{{"type":"text_delta","text":{text:?}}}}}}}"#
        )
    }

    fn stream() -> String {
        [
            r#"{"type":"system","subtype":"init"}"#.to_owned(),
            r#"{"type":"stream_event","event":{"type":"content_block_start","content_block":{"type":"text","text":""}}}"#.to_owned(),
            delta("I'll search "),
            delta("the project."),
            r#"{"type":"stream_event","event":{"type":"content_block_start","content_block":{"type":"tool_use","name":"Glob","input":{}}}}"#.to_owned(),
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"input_json_delta","partial_json":"{}"}}}"#.to_owned(),
            r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"ok"}]}}"#.to_owned(),
            r#"{"type":"stream_event","event":{"type":"content_block_start","content_block":{"type":"thinking"}}}"#.to_owned(),
            r#"{"type":"stream_event","event":{"type":"content_block_start","content_block":{"type":"text","text":""}}}"#.to_owned(),
            delta("Found it."),
            r#"{"type":"result","subtype":"success","is_error":false,"duration_ms":9,"num_turns":2,"result":"Found it.","total_cost_usd":0.5}"#.to_owned(),
        ]
        .join("\n")
    }

    #[test]
    fn streams_tokens_and_tool_calls_live_and_never_shows_thinking() {
        let fake = runner(true, output(0, &stream()));
        let events = RefCell::new(Vec::new());

        let result = ClaudeRuntime::new(fake)
            .execute(&request("sonnet"), &|e| events.borrow_mut().push(e))
            .unwrap();

        let live: Vec<_> = events.borrow().iter().skip(3).cloned().collect();
        assert_eq!(
            live,
            [
                RuntimeEvent::Output("I'll search ".to_owned()),
                RuntimeEvent::Output("the project.".to_owned()),
                RuntimeEvent::ToolStarted("Glob".to_owned()),
                RuntimeEvent::ToolCompleted("Glob".to_owned()),
                // A new text block after earlier text is separated by a blank line.
                RuntimeEvent::Output("\n\nFound it.".to_owned()),
            ]
        );
        // The answer is the concluding message from the final `result` object.
        assert_eq!(result.text, "Found it.");
        assert_eq!(result.metadata["costUsd"], "0.5");
    }

    #[test]
    fn reports_the_tokens_cost_and_quota_the_cli_gives_and_labels_where_they_came_from() {
        let result = r#"{"type":"result","is_error":false,"result":"ok","total_cost_usd":0.015,"usage":{"input_tokens":2,"cache_creation_input_tokens":3567,"cache_read_input_tokens":3659,"output_tokens":4}}"#;
        let limits = r#"{"type":"rate_limit_event","rate_limit_info":{"unifiedWindows":{"five_hour":{"utilization":0.5,"resetsAt":1791006000},"seven_day":{"utilization":0.07,"resetsAt":1791345600}}}}"#;

        let output = ClaudeRuntime::new(runner(true, output(0, &format!("{limits}\n{result}"))))
            .execute(&request("sonnet"), &|_| {})
            .unwrap();

        let usage = output.usage.unwrap();
        assert_eq!(usage.input_tokens, Some(2 + 3567 + 3659));
        assert_eq!(usage.output_tokens, Some(4));
        assert_eq!(usage.total_tokens, Some(2 + 3567 + 3659 + 4));
        assert_eq!(usage.cost, Some(0.015));
        assert_eq!(usage.currency.as_deref(), Some("USD"));
        assert_eq!(usage.source, UsageSource::RuntimeReported);
        let quota = output.quota.unwrap();
        assert_eq!(quota.source, UsageSource::ProviderReported);
        let windows: Vec<_> = quota
            .windows
            .iter()
            .map(|w| (w.id.as_str(), w.used_fraction, w.resets_at))
            .collect();
        assert_eq!(
            windows,
            [
                ("five_hour", 0.5, Some(1_791_006_000)),
                ("seven_day", 0.07, Some(1_791_345_600))
            ]
        );
    }

    #[test]
    fn whatever_the_cli_leaves_out_stays_unavailable_and_a_reported_zero_stays_zero() {
        let bare = r#"{"type":"result","is_error":false,"result":"ok"}"#;
        let zero = r#"{"type":"result","is_error":false,"result":"ok","total_cost_usd":0,"usage":{"input_tokens":0,"output_tokens":0}}"#;
        let only_cost = r#"{"type":"result","is_error":false,"result":"ok","total_cost_usd":0.5}"#;
        let run = |json: &str| {
            ClaudeRuntime::new(runner(true, output(0, json)))
                .execute(&request("sonnet"), &|_| {})
                .unwrap()
        };

        let none = run(bare);
        assert_eq!((none.usage, none.quota), (None, None));
        let zeros = run(zero).usage.unwrap();
        assert_eq!(
            (
                zeros.input_tokens,
                zeros.output_tokens,
                zeros.total_tokens,
                zeros.cost
            ),
            (Some(0), Some(0), Some(0), Some(0.0))
        );
        let cost_only = run(only_cost).usage.unwrap();
        assert_eq!(
            (cost_only.input_tokens, cost_only.total_tokens),
            (None, None)
        );
        assert_eq!(cost_only.cost, Some(0.5));
    }

    /// Uses the real Claude CLI. It needs Claude installed; the full round trip also needs a
    /// signed-in session. Without one, this verifies the real "authentication required"
    /// behaviour instead. Run with `cargo test real_claude -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the Claude CLI installed; makes a real model call when signed in"]
    fn real_claude_runs_or_reports_authentication_required() {
        let runner: Arc<dyn ProcessRunner> =
            Arc::new(crate::infrastructure::SystemProcessRunner::new());
        let runtime = ClaudeRuntime::new(runner);

        let status = inspect(&runtime);
        println!("detected: {status:#?}");
        assert_ne!(
            status.availability,
            Availability::NotInstalled,
            "Claude CLI is not installed"
        );

        let mut ping = request("sonnet");
        ping.prompt.instruction = "Reply with exactly the word: pong".to_owned();
        ping.working_dir = std::env::temp_dir();
        let result = runtime.execute(&ping, &|stage| println!("stage: {stage:?}"));
        println!("result: {result:?}");

        if status.availability == Availability::Ready {
            assert!(result.unwrap().text.to_lowercase().contains("pong"));
        } else {
            println!("NOTE: Claude is not signed in here, so only the failure path was verified");
            assert_eq!(result, Err(RuntimeError::AuthenticationRequired));
        }
    }
}
