use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use super::claude_mcp::{ClaudeMcpAdapter, CONFIG_PLACEHOLDER};
use super::mcp_adapter::{prepare_for, McpAdapter, McpAdapterFactory, McpDialect};
use super::{
    cli, Detection, ModelRuntime, RuntimeError, RuntimeEvent, RuntimeOutput, RuntimeRequest,
};
use crate::application::mcp::{LaunchServer, McpProbe, ReportedMcpTool};
use crate::application::process::{ProcessContext, ProcessOutput, ProcessRunner, ProcessSpec};
use crate::domain::context::{
    Observation, RuntimeSurface, SurfaceControl, SurfaceEntry, SurfaceKind,
};
use crate::domain::runtime::{
    AuthKind, AuthState, Authentication, ModelInfo, ProviderRef, RuntimeCapabilities, RuntimeInfo,
    RuntimeNotice, SystemPromptChannel, Transport,
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
    mcp: Arc<dyn McpAdapter>,
}

impl ClaudeRuntime {
    pub fn new(runner: Arc<dyn ProcessRunner>) -> Self {
        Self {
            runner,
            mcp: McpAdapterFactory::create(McpDialect::Claude),
        }
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
        let (mcp, mcp_features) = McpAdapterFactory::declared(Some(&self.mcp));
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
                system_prompt: SystemPromptChannel::Unsupported,
                mcp,
                mcp_features,
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

    fn mcp_tools_in(&self, reported: &[String], launched: &[&str]) -> Vec<ReportedMcpTool> {
        self.mcp.classify(reported, launched)
    }

    fn probe_mcp(&self, server: &LaunchServer) -> Result<McpProbe, RuntimeError> {
        self.mcp.probe(self.runner.as_ref(), server)
    }

    /// What shapes a Claude run, by evidence (ADR 0024 and the ADR 0026 spike). Atlas-controlled
    /// entries are what it passes; the user's instruction files, hooks and settings are listed as
    /// not observed because the CLI's start-up report does not mention them, and the CLI's own
    /// help says it loads them (a real model run would be needed to see whether they reach the
    /// model). Nothing here says the model received anything but the prompt.
    fn surface(&self, request: &RuntimeRequest) -> RuntimeSurface {
        let args = launch_args(request, &display_mcp_args(request));
        let mut flags = Vec::new();
        let mut skip_next = false;
        for arg in &args {
            if std::mem::take(&mut skip_next) {
                continue;
            }
            // The model id is the user's choice and is shown elsewhere.
            skip_next = arg == "--model";
            flags.push(arg.as_str());
        }
        let tools = args
            .iter()
            .position(|a| a == "--tools")
            .and_then(|at| args.get(at + 1))
            .cloned()
            .unwrap_or_default();
        let atlas = SurfaceControl::AtlasControlled;
        let user = SurfaceControl::UserControlled;
        RuntimeSurface::of(
            &self.info().id,
            vec![
                SurfaceEntry::delivered_prompt(),
                // Atlas does not use the CLI's `--append-system-prompt` (its Windows and escaping
                // behaviour and its semantics are untested): the instructions are in the body.
                SurfaceEntry::system_channel(self.info().capabilities.system_prompt),
                SurfaceEntry::new(SurfaceKind::LaunchFlags, atlas, Observation::Declared)
                    .with_detail(flags.join(" ")),
                SurfaceEntry::new(SurfaceKind::Tools, atlas, Observation::Declared)
                    .with_detail(tools),
                // `--strict-mcp-config` with no `--mcp-config`: Atlas passes no MCP server.
                SurfaceEntry::new(SurfaceKind::McpServers, atlas, Observation::Declared)
                    .with_detail(match request.mcp.as_ref().filter(|_| !request.text_only) {
                        Some(mcp) if !mcp.is_empty() => mcp
                            .servers
                            .iter()
                            .map(|s| s.name.as_str())
                            .collect::<Vec<_>>()
                            .join(","),
                        _ => "none".to_owned(),
                    }),
                // `--disable-slash-commands`.
                SurfaceEntry::new(SurfaceKind::Skills, atlas, Observation::Declared)
                    .with_detail("disabled"),
                SurfaceEntry::new(
                    SurfaceKind::SystemPrompt,
                    SurfaceControl::RuntimeControlled,
                    Observation::Declared,
                ),
                SurfaceEntry::new(
                    SurfaceKind::UserInstructions,
                    user,
                    Observation::NotObserved,
                ),
                SurfaceEntry::new(SurfaceKind::Hooks, user, Observation::NotObserved),
                SurfaceEntry::new(SurfaceKind::UserSettings, user, Observation::NotObserved),
                SurfaceEntry::new(
                    SurfaceKind::Other,
                    SurfaceControl::Unknown,
                    Observation::NotObserved,
                ),
            ],
        )
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

        // The servers of this step, as the CLI takes them; the files live as long as `mcp`.
        let mcp = prepare_for(self.mcp.as_ref(), request)?;
        let args = launch_args(request, &mcp.args);
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
            // The secrets of the MCP servers this execution is given, for the references in the
            // config. Nothing else is added to the CLI's environment.
            env: mcp.env.clone(),
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
/// What the servers' arguments look like, with a stand-in for the per-run config file: for a
/// description of the launch (the surface), never for a run.
fn display_mcp_args(request: &RuntimeRequest) -> Vec<String> {
    request
        .mcp
        .as_ref()
        .filter(|m| !request.text_only && !m.is_empty())
        .map(|m| ClaudeMcpAdapter::arguments(m, CONFIG_PLACEHOLDER))
        .unwrap_or_default()
}

fn launch_args(request: &RuntimeRequest, mcp_args: &[String]) -> Vec<String> {
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
    ]
    .map(str::to_owned)
    .to_vec();
    // The servers Atlas chose for this execution, and only those: `--strict-mcp-config` below
    // keeps every other one out. A text-only run gets none (`--tools ""` does not turn MCP tools
    // off, so the only way to keep them from it is not to give them).
    if !request.text_only {
        args.extend(mcp_args.iter().cloned());
    }
    args.extend(["--strict-mcp-config", "--disable-slash-commands"].map(str::to_owned));
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
        // And how each one came up: `connected` says the process answered, not that its tools
        // are usable.
        let statuses = ClaudeMcpAdapter::report_of(&init)
            .servers
            .iter()
            .map(|(name, status)| {
                format!(
                    "{name}:{}",
                    serde_json::to_string(status)
                        .unwrap_or_default()
                        .trim_matches('"')
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        metadata.insert("mcpStatus".to_owned(), statuses);
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
    // Where each plugin comes from (`name@marketplace`, or `name@builtin` for the CLI's own),
    // which is what tells the user's plugins from the runtime's.
    if let Some(plugins) = init["plugins"].as_array() {
        let sources = plugins
            .iter()
            .filter_map(|p| p["source"].as_str())
            .collect::<Vec<_>>()
            .join(",");
        metadata.insert("pluginSources".to_owned(), sources);
    }
    // The directory of the CLI's own auto-memory. That it exists is reported; whether its text
    // reaches the model is not.
    if let Some(path) = init["memory_paths"]["auto"].as_str() {
        metadata.insert("autoMemoryPath".to_owned(), path.to_owned());
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
            // What the CLI loaded, before it asks the model anything.
            "system" if value["subtype"] == "init" && value["tools"].is_array() => Some(
                RuntimeEvent::ToolsReported(ClaudeMcpAdapter::report_of(&value).tools),
            ),
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
    use crate::application::mcp::{LaunchServer, McpLaunch};
    use crate::application::process::fake::{ok, FakeProcessRunner};
    use crate::application::process::{ProcessError, TerminalRequest};
    use crate::application::prompt::Prompt;
    use crate::application::runtimes::inspect;
    use crate::domain::mcp::McpServerStatus;
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

    fn files_server(secret: bool) -> LaunchServer {
        use crate::application::mcp::LaunchEnv;
        LaunchServer {
            name: "files".to_owned(),
            executable: "node".to_owned(),
            args: vec!["server.js".to_owned()],
            env: if secret {
                vec![LaunchEnv::Secret {
                    name: "API_TOKEN".to_owned(),
                    value: crate::domain::mcp::Secret::new("hunter2hunter2".to_owned()),
                }]
            } else {
                vec![]
            },
        }
    }

    fn with_mcp(mut request: RuntimeRequest, secret: bool) -> RuntimeRequest {
        request.mcp = Some(McpLaunch {
            servers: vec![files_server(secret)],
            held_back: vec![("files".to_owned(), "delete".to_owned())],
            only: Vec::new(),
        });
        request
    }

    #[test]
    fn the_servers_atlas_chose_are_the_only_ones_and_the_secret_is_in_the_environment_not_the_arguments(
    ) {
        let fake = runner(true, output(0, SUCCESS));

        let _ =
            ClaudeRuntime::new(fake.clone()).execute(&with_mcp(request("sonnet"), true), &|_| {});

        let calls = fake.calls.lock().unwrap();
        let run = calls
            .iter()
            .find(|c| c.args.contains(&"--tools".to_owned()))
            .unwrap();
        let at = |flag: &str| run.args.iter().position(|a| a == flag).unwrap();
        // The servers, what is held back, and then the flag that keeps every other one out.
        // The config is a file of this run, no longer there once the run is over.
        let config = std::path::Path::new(&run.args[at("--mcp-config") + 1]);
        assert!(config.starts_with(std::env::temp_dir()));
        assert_eq!(config.extension().and_then(|e| e.to_str()), Some("json"));
        assert!(!config.exists());
        assert_eq!(run.args[at("--disallowedTools") + 1], "mcp__files__delete");
        assert!(at("--mcp-config") < at("--disallowedTools"));
        assert!(at("--disallowedTools") < at("--strict-mcp-config"));
        assert!(run.args.contains(&"--disable-slash-commands".to_owned()));
        // The secret reaches the CLI's process through its environment, under the name the config
        // refers to, and appears nowhere in the arguments or the prompt.
        assert_eq!(
            run.env,
            [
                (
                    "ATLAS_MCP_FILES_API_TOKEN".to_owned(),
                    "hunter2hunter2".to_owned()
                ),
                ("MCP_TIMEOUT".to_owned(), "120000".to_owned())
            ]
        );
        assert!(!run.args.join(" ").contains("hunter2"));
        assert!(!run.stdin.clone().unwrap_or_default().contains("hunter2"));
        // The terminal case puts the prompt last, after `--`, so a variadic option never takes it.
        assert_eq!(run.args[run.args.len() - 2], "--");
    }

    #[test]
    fn without_servers_the_launch_is_what_it_always_was() {
        let fake = runner(true, output(0, SUCCESS));
        let _ = ClaudeRuntime::new(fake.clone()).execute(&request("sonnet"), &|_| {});

        let calls = fake.calls.lock().unwrap();
        let run = calls
            .iter()
            .find(|c| c.args.contains(&"--tools".to_owned()))
            .unwrap();
        assert!(!run.args.contains(&"--mcp-config".to_owned()));
        assert!(run.args.contains(&"--strict-mcp-config".to_owned()));
        assert_eq!(run.env.len(), 0);
    }

    #[test]
    fn a_run_with_every_tool_off_is_never_given_a_server_whatever_the_request_says() {
        let fake = runner(true, output(0, SUCCESS));
        let mut text_only = with_mcp(request("sonnet"), true);
        text_only.text_only = true;

        let _ = ClaudeRuntime::new(fake.clone()).execute(&text_only, &|_| {});

        let calls = fake.calls.lock().unwrap();
        let run = calls
            .iter()
            .find(|c| c.args.contains(&"--tools".to_owned()))
            .unwrap();
        assert!(!run.args.contains(&"--mcp-config".to_owned()));
        assert_eq!(run.env.len(), 0);
    }

    #[test]
    fn the_start_up_report_is_streamed_as_the_tools_the_model_could_call() {
        let stream = ClaudeStream::default();
        let line = r#"{"type":"system","subtype":"init","tools":["Read","mcp__files__read"],"mcp_servers":[]}"#;

        assert_eq!(
            stream.event(line),
            Some(RuntimeEvent::ToolsReported(vec![
                "Read".to_owned(),
                "mcp__files__read".to_owned()
            ]))
        );
    }

    #[test]
    fn the_surface_names_the_servers_a_step_was_given() {
        use crate::domain::context::{SurfaceControl, SurfaceKind};
        let surface =
            ClaudeRuntime::new(runner(true, ok(""))).surface(&with_mcp(request("sonnet"), false));

        let mcp = surface
            .entries
            .iter()
            .find(|e| e.kind == SurfaceKind::McpServers)
            .unwrap();
        assert_eq!(mcp.detail.as_deref(), Some("files"));
        assert_eq!(mcp.control, SurfaceControl::AtlasControlled);
    }

    const PROBE_OUTPUT: &str = r#"{"type":"system","subtype":"init","tools":["Glob","mcp__files__read","mcp__files__write","mcp__other__x"],"mcp_servers":[{"name":"files","status":"connected"},{"name":"other","status":"connected"}]}"#;

    #[test]
    fn a_probe_reports_the_servers_status_and_only_its_own_tools_and_asks_no_model() {
        let fake = runner(true, output(1, PROBE_OUTPUT));

        let probe = ClaudeRuntime::new(fake.clone())
            .probe_mcp(&files_server(true))
            .unwrap();

        assert_eq!(probe.status, McpServerStatus::Connected);
        assert_eq!(probe.tools, ["read", "write"]);
        let calls = fake.calls.lock().unwrap();
        let spec = &calls[0];
        // A model that does not exist, a probe context, no working directory, the secret in the
        // environment only.
        let at = spec.args.iter().position(|a| a == "--model").unwrap();
        assert_eq!(spec.args[at + 1], "atlas-probe-no-such-model");
        assert_eq!(spec.context, ProcessContext::Probe);
        assert!(spec.cwd.is_none());
        // The secret, and the allowance the CLI gives the server to start.
        assert_eq!(spec.env.len(), 2);
        assert!(spec
            .env
            .iter()
            .any(|(k, v)| k == "MCP_TIMEOUT" && v == "120000"));
        assert!(!spec.args.join(" ").contains("hunter2"));
    }

    #[test]
    fn a_probe_of_a_server_that_did_not_start_says_failed_with_no_tools() {
        let out = r#"{"type":"system","subtype":"init","tools":["Glob"],"mcp_servers":[{"name":"files","status":"failed"}]}"#;

        let probe = ClaudeRuntime::new(runner(true, output(1, out)))
            .probe_mcp(&files_server(false))
            .unwrap();

        assert_eq!(probe.status, McpServerStatus::Failed);
        assert_eq!(probe.tools.len(), 0);
    }

    #[test]
    fn a_probe_that_gets_no_report_or_not_the_server_is_an_error_not_a_guess() {
        for out in [
            "",
            "nothing useful",
            r#"{"type":"system","subtype":"init","tools":[],"mcp_servers":[]}"#,
        ] {
            let result =
                ClaudeRuntime::new(runner(true, output(1, out))).probe_mcp(&files_server(false));
            assert!(
                matches!(result, Err(RuntimeError::UnexpectedResponse(_))),
                "{out}"
            );
        }
        let missing = Arc::new(FakeProcessRunner::new(&[], |_| ok("")));
        assert_eq!(
            ClaudeRuntime::new(missing).probe_mcp(&files_server(false)),
            Err(RuntimeError::NotInstalled)
        );
    }

    #[test]
    fn the_start_up_report_also_says_where_plugins_come_from_and_where_the_auto_memory_is() {
        let init = r#"{"type":"system","subtype":"init","tools":["Read"],"plugins":[{"name":"figma","source":"figma@claude-plugins-official"},{"name":"cc-plugin-telemetry","source":"cc-plugin-telemetry@builtin"}],"memory_paths":{"auto":"/home/u/.claude/projects/p/memory/"}}"#;

        let metadata = surface_of(init);

        assert_eq!(
            metadata["pluginSources"],
            "figma@claude-plugins-official,cc-plugin-telemetry@builtin"
        );
        assert_eq!(
            metadata["autoMemoryPath"],
            "/home/u/.claude/projects/p/memory/"
        );
        // Nothing reported, nothing claimed.
        let bare = surface_of(INIT_ISOLATED);
        assert!(!bare.contains_key("autoMemoryPath"));
    }

    #[test]
    fn the_declared_surface_lists_what_atlas_passes_and_admits_what_it_cannot_see() {
        use crate::domain::context::{Observation, SurfaceControl, SurfaceKind};

        let surface = ClaudeRuntime::new(runner(true, ok(""))).surface(&request("sonnet"));

        let entry = |kind| surface.entries.iter().find(|e| e.kind == kind).unwrap();
        let flags = entry(SurfaceKind::LaunchFlags).detail.clone().unwrap();
        assert!(
            flags.contains("--strict-mcp-config") && flags.contains("--disable-slash-commands")
        );
        // The model id is shown elsewhere and is not a flag of the surface.
        assert!(!flags.contains("sonnet"));
        assert_eq!(
            entry(SurfaceKind::Tools).detail.as_deref(),
            Some("Read,Grep,Glob")
        );
        assert_eq!(
            entry(SurfaceKind::McpServers).detail.as_deref(),
            Some("none")
        );
        for kind in [
            SurfaceKind::UserInstructions,
            SurfaceKind::Hooks,
            SurfaceKind::UserSettings,
        ] {
            let user = entry(kind);
            assert_eq!(user.control, SurfaceControl::UserControlled);
            assert_eq!(user.observation, Observation::NotObserved);
        }
        // Before the run nothing has been reported yet.
        assert!(surface
            .entries
            .iter()
            .all(|e| e.observation != Observation::Reported));
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
            let args = launch_args(&req, &display_mcp_args(&req));

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

    /// The harmless MCP server of the Phase A spike: no filesystem, network or process access,
    /// static answers. `MODE=grow` makes it list one more tool than it did when discovered;
    /// `REPORT_ENV=<name>` makes it list a tool named after that variable's value.
    fn spike_server(extra_env: &[(&str, &str)]) -> LaunchServer {
        use crate::application::mcp::LaunchEnv;
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../docs/architecture/spikes/mcp-claude/echo-mcp.js");
        LaunchServer {
            name: "atlasspike".to_owned(),
            executable: "node".to_owned(),
            args: vec![path.to_string_lossy().into_owned()],
            env: extra_env
                .iter()
                .map(|(name, value)| LaunchEnv::Plain {
                    name: (*name).to_owned(),
                    value: (*value).to_owned(),
                })
                .collect(),
        }
    }

    /// Against the real CLI, with a model that does not exist (no model is asked anything):
    /// `cargo test real_claude_mcp -- --ignored`.
    #[test]
    #[ignore = "needs the Claude CLI and node installed; makes no model call"]
    fn real_claude_mcp_probe_sees_the_server_its_tools_and_the_secret_that_reached_it() {
        use crate::application::mcp::LaunchEnv;
        let runtime =
            ClaudeRuntime::new(Arc::new(crate::infrastructure::SystemProcessRunner::new()));
        let plain = runtime
            .probe_mcp(&spike_server(&[]))
            .expect("the CLI probed it");
        eprintln!("REAL PROBE {plain:?}");
        assert_eq!(plain.status, McpServerStatus::Connected);
        assert_eq!(plain.tools, ["echo_static"]);

        // A secret goes through the environment, as a reference in the config, and reaches the
        // server: it reports a tool named after the value it received.
        let mut with_secret = spike_server(&[("REPORT_ENV", "MYSECRET")]);
        with_secret.env.push(LaunchEnv::Secret {
            name: "MYSECRET".to_owned(),
            value: crate::domain::mcp::Secret::new("topsecret42".to_owned()),
        });
        let probed = runtime.probe_mcp(&with_secret).expect("the CLI probed it");
        assert!(
            probed.tools.contains(&"env_topsecret42".to_owned()),
            "{probed:?}"
        );

        // A server that cannot start is reported failed, with no tools.
        let ghost = LaunchServer {
            executable: "/nonexistent/atlas-mcp-ghost".to_owned(),
            ..spike_server(&[])
        };
        let failed = runtime.probe_mcp(&ghost).expect("the CLI still reported");
        assert_eq!(failed.status, McpServerStatus::Failed);
        assert_eq!(failed.tools.len(), 0);
    }

    /// What Atlas relies on, measured: the deny-list holds back a tool it knows of, and a tool the
    /// server adds later is listed all the same, which is why the step is stopped when the runtime
    /// says so.
    #[test]
    #[ignore = "needs the Claude CLI and node installed; makes no model call"]
    fn real_claude_mcp_holds_back_a_known_tool_and_lists_one_the_server_added_later() {
        let runner = Arc::new(crate::infrastructure::SystemProcessRunner::new());
        let runtime = ClaudeRuntime::new(runner);
        let mut req = request("atlas-probe-no-such-model");
        req.working_dir = std::env::temp_dir();
        req.mcp = Some(McpLaunch {
            servers: vec![spike_server(&[("MODE", "grow")])],
            held_back: vec![("atlasspike".to_owned(), "echo_static".to_owned())],
            only: Vec::new(),
        });
        let listed = RefCell::new(Vec::new());

        let _ = runtime.execute(&req, &|event| {
            if let RuntimeEvent::ToolsReported(tools) = event {
                *listed.borrow_mut() = tools;
            }
        });

        let listed = listed.into_inner();
        eprintln!("REAL LISTED {listed:?}");
        assert!(
            !listed.contains(&"mcp__atlasspike__echo_static".to_owned()),
            "held back"
        );
        assert!(
            listed.contains(&"mcp__atlasspike__extra_tool".to_owned()),
            "added later"
        );
        // And the classification the service uses names it as that server's.
        let found = runtime.mcp_tools_in(&listed, &["atlasspike"]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].server.as_deref(), Some("atlasspike"));
        assert_eq!(found[0].tool, "extra_tool");
    }

    /// A server that needs a while to start (an `npx` downloading its package the first time) is
    /// connected thanks to the startup allowance Atlas gives the CLI. No model call.
    #[test]
    #[ignore = "needs the Claude CLI and node installed; makes no model call; takes about 45 s"]
    fn real_claude_mcp_a_slow_starting_server_still_connects() {
        let runtime =
            ClaudeRuntime::new(Arc::new(crate::infrastructure::SystemProcessRunner::new()));
        let probed = runtime
            .probe_mcp(&spike_server(&[("MODE", "slow"), ("DELAY_MS", "40000")]))
            .expect("the CLI probed it");

        assert_eq!(probed.status, McpServerStatus::Connected, "{probed:?}");
        assert_eq!(probed.tools, ["echo_static"]);
    }

    /// Atlas stops a run (an MCP tool nobody authorized): the CLI is killed without a chance to
    /// clean up, and a server that ignores the end of its input must not be left behind. No model
    /// call: the model does not exist.
    #[test]
    #[ignore = "needs the Claude CLI and node installed; makes no model call"]
    fn real_claude_mcp_a_stopped_run_leaves_no_server_behind() {
        let marker = format!("atlas-orphan-check-{}", std::process::id());
        let running = || {
            std::process::Command::new("pgrep")
                .args(["-f", &marker])
                .output()
                .is_ok_and(|out| !out.stdout.is_empty())
        };
        let sessions = Arc::new(crate::application::sessions::SessionRegistry::default());
        let runtime = ClaudeRuntime::new(Arc::new(
            crate::infrastructure::SystemProcessRunner::new().with_sessions(sessions.clone()),
        ));
        let mut server = spike_server(&[("MODE", "stubborn")]);
        server.args.push(marker.clone());
        let mut req = request("atlas-probe-no-such-model");
        req.working_dir = std::env::temp_dir();
        req.mcp = Some(McpLaunch {
            servers: vec![server],
            held_back: vec![],
            only: Vec::new(),
        });
        let execution = req.scope.execution_id.clone();
        let stopped = std::sync::atomic::AtomicBool::new(false);

        // Exactly what the execution service does when the runtime lists a tool nobody authorized.
        let _ = runtime.execute(&req, &|event| {
            if matches!(event, RuntimeEvent::ToolsReported(_)) {
                sessions.stop(&execution);
                stopped.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        });

        assert!(
            stopped.load(std::sync::atomic::Ordering::SeqCst),
            "the CLI never listed its tools, so the check proves nothing"
        );
        let gone = (0..50).any(|_| {
            std::thread::sleep(std::time::Duration::from_millis(100));
            !running()
        });
        assert!(gone, "an MCP server outlived the stopped run");
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
        let mut args = launch_args(&req, &display_mcp_args(&req));
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
