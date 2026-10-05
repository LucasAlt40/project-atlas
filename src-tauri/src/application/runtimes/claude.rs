use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use super::{
    cli, Detection, ModelRuntime, RuntimeError, RuntimeEvent, RuntimeOutput, RuntimeRequest,
};
use crate::application::process::{ProcessContext, ProcessOutput, ProcessRunner, ProcessSpec};
use crate::domain::runtime::{
    AuthKind, AuthState, Authentication, ModelInfo, ProviderRef, RuntimeCapabilities, RuntimeInfo,
    RuntimeNotice, Transport,
};
use crate::domain::security::ToolAccess;
use crate::domain::usage::{QuotaInfo, QuotaWindow, UsageMetrics, UsageSource};

const PROGRAM: &str = "claude";
/// Idle limit: silence from the CLI (long thinking or a slow tool) for this long ends the run.
const RUN_TIMEOUT: Duration = Duration::from_secs(900);
/// The only tools a Claude run may use: reading and searching the project. Everything that
/// writes or executes (Edit, Write, Bash…) is not available, which is how Atlas's read-only
/// rule is enforced for this runtime.
const READ_ONLY_TOOLS: &str = "Read,Grep,Glob";
/// Plus the two that create and change files, for an execution whose policy allows file writes
/// and that works in an isolated worktree (`RuntimeRequest::allow_edits`). Still no shell, no web:
/// `Bash`, `WebFetch` and the rest stay off.
const EDIT_TOOLS: &str = "Read,Grep,Glob,Edit,Write";

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
/// When the run is attached to a terminal (see `cli::deliver_prompt`) it is the last argument
/// instead, because `claude -p` ignores a terminal's stdin.
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
    fn detect_interaction(
        &self,
        output: &RuntimeOutput,
    ) -> Option<crate::domain::interaction::InteractionDetection> {
        cli::detection_from_asking_tool(output)
    }

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
                // Runs attached to a terminal. `claude -p` reads its prompt from the command
                // line and ignores a terminal's stdin (verified), so the terminal is read-only.
                interactive_terminal: true,
                interrupt: true,
                terminal_input: false,
                terminal_resize: true,
                text_only: true,
                file_edit: true,
                // `--tools Read,Grep,Glob`: no edit, no shell, no web tool.
                tool_access: ToolAccess::NONE,
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

        let args = launch_args(request);
        let delivery = cli::deliver_prompt(
            self.runner.as_ref(),
            PROGRAM,
            args,
            request.prompt.combined(),
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
        let stream = ClaudeStream::default();
        let output = cli::execute(self.runner.as_ref(), &spec, progress, &|line| {
            if let Some(event) = stream.event(line) {
                progress(event);
            }
        })?;
        parse_run_output(&output, &request.model_id)
    }
}

/// The arguments of a Claude run, without the prompt: the tools it gets, and nothing else around
/// them.
///
/// `--tools` only names the *built-in* tools. Left alone, `claude -p` also loads the user's own
/// MCP servers (connectors such as Docs, Drive, Gmail, Jira), skills, slash commands and plugins,
/// and their tools are callable by the model. Measured on a real machine with exactly the
/// arguments below minus the last two: 11 tools instead of 3 (eight of them `mcp__…Claude_Docs__*`,
/// including `create`, `update` and `delete`), 8 MCP servers, 48 skills, 84 slash commands. An agent
/// Atlas calls read-only must not reach any of that, and its prompt should not carry its listing.
/// `--strict-mcp-config` (with no `--mcp-config`) loads no MCP server; `--disable-slash-commands`
/// turns skills off. Atlas has its own skills layer and chooses what an agent gets.
fn launch_args(request: &RuntimeRequest) -> Vec<String> {
    let tools = if request.text_only {
        // An empty list turns every tool off: the model can only answer from the prompt.
        ""
    } else if request.allow_edits {
        EDIT_TOOLS
    } else {
        READ_ONLY_TOOLS
    };
    let mut args = [
        "-p",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--model",
        &request.model_id,
        "--tools",
        tools,
        "--no-session-persistence",
        "--strict-mcp-config",
        "--disable-slash-commands",
    ]
    .map(str::to_owned)
    .to_vec();
    if request.allow_edits && !request.text_only {
        // The run is not interactive, so a tool that needs to ask is refused. File edits
        // inside the working directory (the isolated worktree) are what was granted.
        args.extend(["--permission-mode".to_owned(), "acceptEdits".to_owned()]);
    }
    args
}

/// What the CLI says it loaded when it started (its `system`/`init` message): the tools the model
/// can call and the extensions around it. Facts, in the keys the rest of Atlas reads
/// (`toolsExposed`, `mcpServers`, `skillsLoaded`, `slashCommands`, `pluginsLoaded`).
fn note_surface(metadata: &mut BTreeMap<String, String>, stdout: &str) {
    let Some(init) = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|v| v["type"] == "system" && v["subtype"] == "init")
    else {
        return;
    };
    let names = |value: &Value| -> Option<String> {
        value.as_array().map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().or_else(|| item["name"].as_str()))
                .collect::<Vec<_>>()
                .join(",")
        })
    };
    let count = |value: &Value| value.as_array().map(Vec::len);
    if let Some(tools) = names(&init["tools"]) {
        metadata.insert("toolsExposed".to_owned(), tools);
    }
    if let Some(servers) = names(&init["mcp_servers"]) {
        metadata.insert("mcpServers".to_owned(), servers);
    }
    for (key, source) in [
        ("skillsLoaded", "skills"),
        ("slashCommands", "slash_commands"),
        ("pluginsLoaded", "plugins"),
    ] {
        if let Some(n) = count(&init[source]) {
            metadata.insert(key.to_owned(), n.to_string());
        }
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
        cached_input_tokens: part("cache_read_input_tokens"),
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
            Err(cli::classify_failure(&cli::diagnostics(output), model_id))
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
    note_surface(&mut metadata, &output.stdout);
    // A tool that asks the person was called: `-p` has nobody to answer it, so the CLI reports
    // it as denied. That is a structured signal, whatever language the agent wrote in.
    for denial in result["permission_denials"]
        .as_array()
        .into_iter()
        .flatten()
    {
        if let Some(tool) = denial["tool_name"].as_str() {
            cli::note_asking_tool(&mut metadata, tool, &denial["tool_input"]);
        }
    }
    for value in output
        .stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|value| value["type"] == "assistant")
    {
        for block in value["message"]["content"].as_array().into_iter().flatten() {
            if let Some(tool) = block["name"]
                .as_str()
                .filter(|_| block["type"] == "tool_use")
            {
                cli::note_asking_tool(&mut metadata, tool, &block["input"]);
            }
        }
    }
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
    use crate::application::process::{ProcessError, TerminalRequest};
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
    fn runs_read_only_with_the_selected_model_in_a_terminal_with_the_prompt_as_the_last_argument() {
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
                "--no-session-persistence",
                "--strict-mcp-config",
                "--disable-slash-commands",
                "--",
                &request("sonnet").prompt.combined(),
            ]
        );
        assert_eq!(run.cwd, Some(PathBuf::from("/atlas")));
        // A terminal has no stdin channel for a prompt. It is read-only: `claude -p` ignores it.
        assert_eq!(run.stdin, None);
        assert_eq!(run.terminal, Some(TerminalRequest::new(false)));
        let prompt = run.args.last().unwrap();
        assert!(prompt.contains("SYS") && prompt.contains("CTX") && prompt.contains("INS"));
    }

    #[test]
    fn a_failed_run_in_a_terminal_is_classified_from_its_merged_output() {
        // In a terminal stderr is part of stdout: the lines that are not JSON events are the
        // diagnosis.
        let fake = runner(
            true,
            Ok(ProcessOutput {
                exit_code: Some(1),
                stdout: "Failed to authenticate. API Error: 401\n".to_owned(),
                stderr: String::new(),
            }),
        );

        let error = ClaudeRuntime::new(fake)
            .execute(&request("sonnet"), &|_| {})
            .unwrap_err();

        assert_eq!(error, RuntimeError::AuthenticationRequired);
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

    /// The tools a run is launched with follow the request: read-only by default, files only when
    /// the execution service granted edits, nothing at all for a text-only run. A shell and the
    /// web are never among them.
    #[test]
    fn tools_follow_what_the_execution_was_granted_and_never_include_a_shell() {
        let runner = Arc::new(FakeProcessRunner::new(&["claude"], |_| ok("")));
        let runtime = ClaudeRuntime::new(runner.clone() as Arc<dyn ProcessRunner>);
        let tools = |edit: bool, text_only: bool| {
            let mut req = request("sonnet");
            req.allow_edits = edit;
            req.text_only = text_only;
            let _ = runtime.execute(&req, &|_| {});
            let call = runner
                .calls
                .lock()
                .unwrap()
                .clone()
                .into_iter()
                .rev()
                .find(|c| c.args.contains(&"--tools".to_owned()))
                .unwrap();
            let at = call.args.iter().position(|a| a == "--tools").unwrap();
            (
                call.args[at + 1].clone(),
                call.args.contains(&"--permission-mode".to_owned()),
            )
        };

        assert_eq!(tools(false, false), ("Read,Grep,Glob".to_owned(), false));
        assert_eq!(
            tools(true, false),
            ("Read,Grep,Glob,Edit,Write".to_owned(), true)
        );
        assert_eq!(tools(true, true), (String::new(), false));
        for edit in [false, true] {
            let (list, _) = tools(edit, false);
            for forbidden in ["Bash", "WebFetch", "WebSearch", "Task", "NotebookEdit"] {
                assert!(!list.contains(forbidden), "{forbidden}");
            }
        }
    }

    /// What the CLI reports when it starts (the shape of its `system`/`init` message, captured from
    /// Claude Code 2.1.285 with and without the isolation flags).
    const INIT_UNISOLATED: &str = r#"{"type":"system","subtype":"init","tools":["Glob","Grep","Read","mcp__claude_ai_Claude_Docs__create","mcp__claude_ai_Claude_Docs__delete"],"mcp_servers":[{"name":"claude.ai Claude Docs","status":"connected"},{"name":"claude.ai Gmail","status":"needs-auth"}],"slash_commands":["a","b","c"],"skills":["a","b"],"plugins":[{"name":"figma"}],"agents":["Plan"]}"#;
    const INIT_ISOLATED: &str = r#"{"type":"system","subtype":"init","tools":["Glob","Grep","Read"],"mcp_servers":[],"slash_commands":[],"skills":[],"plugins":[{"name":"figma"}]}"#;

    fn surface_of(init: &str) -> BTreeMap<String, String> {
        let stdout = format!("{init}\n{SUCCESS}");
        parse_run_output(&output(0, &stdout).unwrap(), "sonnet")
            .unwrap()
            .metadata
    }

    #[test]
    fn what_the_cli_loaded_around_the_model_is_reported_as_it_said_it() {
        let loose = surface_of(INIT_UNISOLATED);
        assert_eq!(
            loose["toolsExposed"],
            "Glob,Grep,Read,mcp__claude_ai_Claude_Docs__create,mcp__claude_ai_Claude_Docs__delete"
        );
        assert_eq!(loose["mcpServers"], "claude.ai Claude Docs,claude.ai Gmail");
        assert_eq!(loose["skillsLoaded"], "2");
        assert_eq!(loose["slashCommands"], "3");
        assert_eq!(loose["pluginsLoaded"], "1");

        let isolated = surface_of(INIT_ISOLATED);
        assert_eq!(isolated["toolsExposed"], "Glob,Grep,Read");
        assert_eq!(isolated["mcpServers"], "");
        assert_eq!(isolated["skillsLoaded"], "0");
        // Without an init message nothing is claimed.
        let none = parse_run_output(&output(0, SUCCESS).unwrap(), "sonnet")
            .unwrap()
            .metadata;
        assert!(!none.contains_key("toolsExposed") && !none.contains_key("mcpServers"));
    }

    #[test]
    fn every_way_atlas_launches_claude_keeps_the_users_connectors_and_skills_out() {
        for (edit, text_only) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut req = request("sonnet");
            req.allow_edits = edit;
            req.text_only = text_only;
            let args = launch_args(&req);

            // No MCP server (none is named with `--mcp-config`) and no skills.
            assert!(
                args.contains(&"--strict-mcp-config".to_owned()),
                "{edit} {text_only}"
            );
            assert!(args.contains(&"--disable-slash-commands".to_owned()));
            assert!(!args.contains(&"--mcp-config".to_owned()));
            // And it is still the tool list and the read-only/edit decision that were made.
            let at = args.iter().position(|a| a == "--tools").unwrap();
            let expected = if text_only {
                ""
            } else if edit {
                "Read,Grep,Glob,Edit,Write"
            } else {
                "Read,Grep,Glob"
            };
            assert_eq!(args[at + 1], expected);
        }
    }

    /// Uses the real Claude CLI. It needs Claude installed; the full round trip also needs a
    /// signed-in session. Without one, this verifies the real "authentication required"
    /// behaviour instead. Run with `cargo test real_claude -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the Claude CLI installed; makes a real model call when signed in"]
    fn real_claude_runs_or_reports_authentication_required() {
        // Through the real guard, like the app: the scope's project folder is the temp dir.
        let runner: Arc<dyn ProcessRunner> =
            crate::application::security::testutil::guarded_system_runner(&std::env::temp_dir());
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

    #[test]
    fn a_denied_plan_approval_tool_is_a_structured_request_for_the_person() {
        let stdout = concat!(
            r#"{"type":"result","subtype":"success","is_error":false,"result":"Plano pronto.","#,
            r#""permission_denials":[{"tool_name":"ExitPlanMode","tool_use_id":"t1","#,
            r##""tool_input":{"plan":"# Plan\n1. Do it"}}]}"##
        );
        let out = parse_run_output(&output(0, stdout).unwrap(), "m").unwrap();
        let found = cli::detection_from_asking_tool(&out).expect("a structured signal");
        assert_eq!(found.confidence, 95);
        assert_eq!(
            found.kind,
            Some(crate::domain::interaction::InteractionKind::Approval)
        );
        assert!(found.document.contains("1. Do it"));
    }

    /// Starts the real CLI with Atlas's exact arguments and a model that does not exist, so it
    /// reports what it loaded (its `init` message) and then fails at the first request: no model
    /// call is made and nothing is spent. Run with `cargo test real_claude_surface -- --ignored`.
    #[test]
    #[ignore = "needs the Claude CLI installed; makes no model call"]
    fn real_claude_surface_has_only_the_granted_tools_and_no_connectors_or_skills() {
        use crate::application::process::ProcessRunner as _;

        let system = crate::infrastructure::SystemProcessRunner::new();
        let mut req = request("not-a-real-model-xyz");
        req.working_dir = std::env::temp_dir();
        let mut args = launch_args(&req);
        args.extend(["--".to_owned(), "ping".to_owned()]);
        let spec = ProcessSpec {
            program: PROGRAM.to_owned(),
            args,
            stdin: None,
            cwd: Some(req.working_dir.clone()),
            env: Vec::new(),
            timeout: RUN_TIMEOUT,
            context: ProcessContext::Probe,
            terminal: None,
        };

        let out = system.run(&spec, &|_| {}).expect("the CLI ran");

        let mut metadata = BTreeMap::new();
        note_surface(&mut metadata, &out.stdout);
        eprintln!("REAL SURFACE {metadata:?}");
        assert_eq!(metadata["toolsExposed"], "Glob,Grep,Read");
        assert_eq!(metadata["mcpServers"], "");
        assert_eq!(metadata["skillsLoaded"], "0");
        assert_eq!(metadata["slashCommands"], "0");
    }
}
