use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use super::copilot_mcp::CopilotMcpAdapter;
use super::mcp_adapter::{prepare_for, McpAdapter, McpAdapterFactory, McpDialect};
use super::{
    cli, Detection, ModelRuntime, RuntimeError, RuntimeEvent, RuntimeOutput, RuntimeRequest,
};
use crate::application::process::{ProcessContext, ProcessOutput, ProcessRunner, ProcessSpec};
use crate::domain::context::RuntimeSurface;
use crate::domain::mcp::ReportedMcpTool;
use crate::domain::runtime::{
    AuthKind, AuthState, Authentication, ModelInfo, ProviderRef, RuntimeCapabilities, RuntimeInfo,
    SystemPromptChannel, Transport,
};
use crate::domain::security::ToolAccess;
use crate::domain::usage::{UsageMetrics, UsageSource};

const PROGRAM: &str = "copilot";
/// Idle limit: silence from the CLI (long reasoning or a slow tool) for this long ends the run.
const RUN_TIMEOUT: Duration = Duration::from_secs(900);
/// The model the CLI picks by itself, and what it uses when none is named.
#[cfg(test)]
const AUTO_MODEL: &str = "auto";
/// How long listing the account's models may take (it starts the CLI, makes no model call).
const LIST_TIMEOUT: Duration = Duration::from_secs(30);
/// The one name given to `--available-tools` by a run that may use none: no tool has it.
const NO_TOOLS: &str = "atlas_no_tools";

/// Runtime for the GitHub Copilot CLI, using the user's own GitHub sign-in (`copilot login`).
///
/// - version: `copilot --version`
/// - sign-in: the CLI has no status command (only `login`), so the state is `Unknown` and a run
///   that finds no credential fails as "sign-in required"
/// - models: `copilot --acp` (the Agent Client Protocol server) answers `session/new` with the models
///   **this account** can choose, at no cost. `copilot help config` lists far more and was measured
///   to offer models the account cannot use ("not available"), so it is not used
/// - run: `copilot --prompt=<prompt> --output-format json --no-ask-user --no-auto-update
///   --disable-builtin-mcps --disallow-temp-dir --model <model>`. The CLI streams one JSON event
///   per line: `assistant.message` (the model's text and its tool requests),
///   `tool.execution_start` / `tool.execution_complete`, `session.usage_checkpoint` with the token
///   counts and a final `result` with the exit code
///
/// A run that cannot ask refuses what it was not allowed (measured): shell commands and file
/// writes are denied by default and reading stays inside the working directory. `write` is allowed
/// only when `allow_edits` is set. `--allow-all` / `--yolo` is never used.
pub struct CopilotRuntime {
    runner: Arc<dyn ProcessRunner>,
    mcp: Arc<dyn McpAdapter>,
}

impl CopilotRuntime {
    pub fn new(runner: Arc<dyn ProcessRunner>) -> Self {
        Self {
            runner,
            mcp: McpAdapterFactory::create(McpDialect::Copilot),
        }
    }
}

impl ModelRuntime for CopilotRuntime {
    /// A headless run refuses every shell command that was not allowed beforehand, and Atlas
    /// allows none: the agent is told it cannot run any (a refused command ends such a run).
    fn runs_commands(&self) -> bool {
        false
    }

    fn info(&self) -> RuntimeInfo {
        let (mcp, mcp_features) = McpAdapterFactory::declared(Some(&self.mcp));
        RuntimeInfo {
            id: "copilot".to_owned(),
            name: "GitHub Copilot CLI".to_owned(),
            provider: ProviderRef {
                id: "github".to_owned(),
                name: "GitHub Copilot".to_owned(),
            },
            transport: Transport::Cli,
            capabilities: RuntimeCapabilities {
                model_discovery: true,
                streaming: false,
                system_prompt: SystemPromptChannel::Unsupported,
                mcp,
                mcp_features,
                non_interactive_execution: true,
                authentication: vec![AuthKind::CliSession],
                usage_metrics: true,
                cost_metrics: false,
                quota_metrics: false,
                // Runs attached to a terminal; the prompt is an option, so it is read-only.
                interactive_terminal: true,
                interrupt: true,
                terminal_input: false,
                terminal_resize: true,
                // `--available-tools` with nothing that exists leaves the model no tool at all
                // (measured: the request carried zero tools).
                text_only: true,
                file_edit: true,
                // The CLI's own permissions decide, not Atlas: the user's settings can allow
                // commands and URLs, which Atlas cannot see. Reported as it is.
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
        let spec = ProcessSpec {
            program: PROGRAM.to_owned(),
            args: ["--acp", "--no-auto-update", "--disable-builtin-mcps"]
                .map(str::to_owned)
                .to_vec(),
            stdin: Some(acp_requests()),
            cwd: None,
            env: Vec::new(),
            timeout: LIST_TIMEOUT,
            context: ProcessContext::Probe,
            terminal: None,
        };
        let output = self.runner.run(&spec, &|_| {}).map_err(|_| {
            RuntimeError::Unavailable("Copilot could not list its models.".to_owned())
        })?;
        parse_models(&output.stdout)
    }

    fn accepts_extra_dirs(&self) -> bool {
        true
    }

    fn mcp_tools_in(&self, reported: &[String], launched: &[&str]) -> Vec<ReportedMcpTool> {
        self.mcp.classify(reported, launched)
    }

    fn surface(&self, request: &RuntimeRequest) -> RuntimeSurface {
        // The CLI also loads the servers in `~/.copilot/mcp-config.json`.
        super::merged_mcp_surface(
            &self.info().id,
            self.info().capabilities.system_prompt,
            request,
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

        // The servers of this step, as the CLI takes them; the file lives as long as `mcp`.
        let mcp = prepare_for(self.mcp.as_ref(), request)?;
        let mut args = [
            "--output-format",
            "json",
            "--no-ask-user",
            "--no-auto-update",
            "--disable-builtin-mcps",
            "--disallow-temp-dir",
            "--model",
            &request.model_id,
        ]
        .map(str::to_owned)
        .to_vec();
        if request.text_only {
            // Only tools that exist are offered, and none is named: the model can only answer
            // from the prompt.
            args.push(format!("--available-tools={NO_TOOLS}"));
        } else {
            args.extend(mcp.args.iter().cloned());
            // The other project folders of the execution, each as its own option so a prompt or a
            // later flag is never read as another folder. Without it a read there is refused
            // (measured). Only given to a run that reads (see `ExecutionService`).
            args.extend(
                request
                    .extra_dirs
                    .iter()
                    .map(|dir| format!("--add-dir={}", dir.to_string_lossy())),
            );
            if request.allow_edits {
                args.push("--allow-tool=write".to_owned());
            }
        }
        let delivery = cli::deliver_prompt_option(
            self.runner.as_ref(),
            PROGRAM,
            "--prompt",
            args,
            &self.delivery(request).into_payload(),
        )?;
        let spec = ProcessSpec {
            program: PROGRAM.to_owned(),
            args: delivery.args,
            stdin: delivery.stdin,
            cwd: Some(request.working_dir.clone()),
            env: mcp.env.clone(),
            timeout: RUN_TIMEOUT,
            context: ProcessContext::Runtime(request.scope.clone()),
            terminal: delivery.terminal,
        };
        let stream = Stream::default();
        let output = cli::execute(self.runner.as_ref(), &spec, progress, &|line| {
            for event in stream.event(line) {
                progress(event);
            }
        })?;
        parse_run_output(&output, &request.model_id)
    }
}

/// The two requests that make the server tell the models of the account, one per line: the CLI
/// answers each and ends when its input does.
fn acp_requests() -> String {
    let initialize = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": 1, "clientCapabilities": {}}});
    let session = json!({"jsonrpc": "2.0", "id": 2, "method": "session/new",
        "params": {"cwd": std::env::temp_dir(), "mcpServers": []}});
    format!("{initialize}\n{session}\n")
}

/// The models the account can choose, from the answer to `session/new`. An entry the account has
/// not enabled is left out, and the same model listed twice is one.
fn parse_models(stdout: &str) -> Result<Vec<ModelInfo>, RuntimeError> {
    let answer = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line.trim()).ok())
        .find(|message| message["id"] == 2)
        .ok_or_else(|| {
            RuntimeError::UnexpectedResponse("Copilot did not list its models.".into())
        })?;
    let mut models: Vec<ModelInfo> = Vec::new();
    for entry in answer["result"]["models"]["availableModels"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let enabled = entry["_meta"]["copilotEnablement"]
            .as_str()
            .is_none_or(|state| state == "enabled");
        let Some(id) = entry["modelId"].as_str().filter(|_| enabled) else {
            continue;
        };
        if !models.iter().any(|m| m.id == id) {
            models.push(ModelInfo {
                id: id.to_owned(),
                name: entry["name"].as_str().unwrap_or(id).to_owned(),
            });
        }
    }
    if models.is_empty() {
        return Err(RuntimeError::UnexpectedResponse(
            "Copilot stated no models.".to_owned(),
        ));
    }
    Ok(models)
}

/// What a tool is called in the live view: the command for a shell call, a call to an MCP server
/// in the shape the other runtimes use (`mcp__<server>__<tool>`), otherwise the tool.
fn tool_label(data: &Value) -> Option<String> {
    let name = data["toolName"].as_str()?;
    if let Some(tool) = data["mcpToolName"].as_str() {
        return Some(CopilotMcpAdapter::tool_name(
            data["mcpServerName"].as_str(),
            tool,
        ));
    }
    Some(
        data["arguments"]["command"]
            .as_str()
            .filter(|_| name == "bash")
            .unwrap_or(name)
            .to_owned(),
    )
}

/// The tools the model was sent, as the first `session.usage_checkpoint` lists them. `None` when
/// the event lists none at all (an empty list is a fact: the model had no tool).
fn tools_in(checkpoint: &Value) -> Option<Vec<String>> {
    let conversations = checkpoint["data"]["promptCacheBreakState"].as_array()?;
    let mut names: Vec<String> = conversations
        .iter()
        .filter_map(|conversation| conversation["models"].as_object())
        .flat_map(|models| models.values())
        .flat_map(|model| model["tools"].as_array().into_iter().flatten())
        .filter_map(|tool| tool["name"].as_str().map(str::to_owned))
        .collect();
    names.sort();
    names.dedup();
    Some(names)
}

/// What a run's stream remembers between lines.
#[derive(Default)]
struct Stream {
    /// A tool's end event carries only its call id: the start names it.
    started: Mutex<HashMap<String, String>>,
    /// The tool list is told once, at the first call the CLI makes.
    tools_reported: AtomicBool,
}

impl Stream {
    /// Turns one JSON line into live events: each message of the model as it is complete, tool
    /// calls as they start and finish, and the tools the model was given.
    fn event(&self, line: &str) -> Vec<RuntimeEvent> {
        let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
            return Vec::new();
        };
        let data = &value["data"];
        let event = match value["type"].as_str() {
            Some("assistant.message") => data["content"]
                .as_str()
                .filter(|t| !t.trim().is_empty())
                .map(|text| RuntimeEvent::Output(format!("{}\n\n", text.trim_end()))),
            Some("tool.execution_start") => tool_label(data).and_then(|label| {
                self.started
                    .lock()
                    .ok()?
                    .insert(data["toolCallId"].as_str()?.to_owned(), label.clone());
                Some(RuntimeEvent::ToolStarted(label))
            }),
            Some("tool.execution_complete") => data["toolCallId"].as_str().and_then(|id| {
                self.started
                    .lock()
                    .ok()?
                    .remove(id)
                    .map(RuntimeEvent::ToolCompleted)
            }),
            Some("session.usage_checkpoint") => tools_in(&value)
                .filter(|_| !self.tools_reported.swap(true, Ordering::Relaxed))
                .map(RuntimeEvent::ToolsReported),
            _ => None,
        };
        event.into_iter().collect()
    }
}

/// Tokens as the last `session.usage_checkpoint` reports them, summed over the models used.
/// "Input" already includes the cache reads and writes.
fn usage_of(events: &[Value]) -> Option<UsageMetrics> {
    let checkpoint = events
        .iter()
        .rev()
        .find(|e| e["type"] == "session.usage_checkpoint")?;
    let models = checkpoint["data"]["accountingSnapshot"]["modelMetrics"].as_object()?;
    let sum = |key: &str| {
        models
            .values()
            .map(|m| m["usage"][key].as_u64())
            .try_fold(0_u64, |total, n| n.map(|n| total.saturating_add(n)))
    };
    let (input_tokens, output_tokens) = (sum("inputTokens"), sum("outputTokens"));
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
        cached_input_tokens: sum("cacheReadTokens"),
        cost: None,
        currency: None,
        source: UsageSource::RuntimeReported,
    })
}

/// Reads the events of a finished run. The answer is the last message that asks for no tool
/// (earlier ones are narration); the closing `result` event carries the exit code.
fn parse_run_output(output: &ProcessOutput, model_id: &str) -> Result<RuntimeOutput, RuntimeError> {
    let events: Vec<Value> = output
        .stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line.trim()).ok())
        .collect();
    let result = events.iter().rev().find(|e| e["type"] == "result");
    let succeeded = output.exit_code == Some(0)
        && result.is_some_and(|r| r["exitCode"].as_i64().unwrap_or(0) == 0);
    if !succeeded {
        let details = events
            .iter()
            .rev()
            .find_map(|e| match e["type"].as_str()? {
                "session.error" => e["data"]["message"].as_str(),
                _ => None,
            })
            .map(str::to_owned)
            .filter(|details| !details.is_empty())
            .unwrap_or_else(|| cli::diagnostics(output));
        return if details.is_empty() && output.exit_code == Some(0) {
            Err(RuntimeError::UnexpectedResponse(
                "Copilot did not complete the run.".to_owned(),
            ))
        } else if details.contains("is not available") {
            Err(RuntimeError::ModelUnavailable(model_id.to_owned()))
        } else {
            Err(cli::classify_failure(&details, model_id))
        };
    }

    let text = events
        .iter()
        .rev()
        .filter(|e| e["type"] == "assistant.message")
        .filter(|e| {
            e["data"]["toolRequests"]
                .as_array()
                .is_none_or(Vec::is_empty)
        })
        .find_map(|e| {
            e["data"]["content"]
                .as_str()
                .filter(|t| !t.trim().is_empty())
        })
        .map(|text| text.trim().to_owned())
        .unwrap_or_default();
    if text.is_empty() {
        return Err(RuntimeError::UnexpectedResponse(
            "Copilot returned an empty result.".to_owned(),
        ));
    }

    let mut metadata = BTreeMap::new();
    let tool_calls = events
        .iter()
        .filter(|e| e["type"] == "tool.execution_complete")
        .count();
    metadata.insert("toolCalls".to_owned(), tool_calls.to_string());
    if let Some(requests) = result.and_then(|r| r["usage"]["premiumRequests"].as_f64()) {
        metadata.insert("premiumRequests".to_owned(), requests.to_string());
    }
    Ok(RuntimeOutput {
        text,
        metadata,
        usage: usage_of(&events),
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
        r#"{"type":"session.mcp_servers_loaded","data":{},"ephemeral":true}"#,
        "\n",
        r#"{"type":"assistant.message","data":{"content":"Looking.","toolRequests":[{"name":"bash"}]}}"#,
        "\n",
        r#"{"type":"tool.execution_start","data":{"toolCallId":"c1","toolName":"bash","arguments":{"command":"ls"}}}"#,
        "\n",
        r#"{"type":"tool.execution_partial_result","data":{"toolCallId":"c1"},"ephemeral":true}"#,
        "\n",
        r#"{"type":"tool.execution_complete","data":{"toolCallId":"c1","success":true}}"#,
        "\n",
        r#"{"type":"assistant.message","data":{"content":"Three improvements.\n","toolRequests":[]}}"#,
        "\n",
        r#"{"type":"session.usage_checkpoint","data":{"promptCacheBreakState":[{"models":{"gpt-6-luna":{"tools":[{"name":"bash"},{"name":"files-read"}]}}}],"#,
        r#""accountingSnapshot":{"modelMetrics":{"gpt-6-luna":{"usage":{"inputTokens":100,"outputTokens":50,"cacheReadTokens":40}}}}}}"#,
        "\n",
        r#"{"type":"result","exitCode":0,"usage":{"premiumRequests":1}}"#,
    );

    /// What `copilot --acp` answers: the account's models, `auto` twice (as measured), one it has
    /// not enabled and one it has.
    const ACP: &str = concat!(
        r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","method":"session/update","params":{}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":2,"result":{"models":{"availableModels":["#,
        r#"{"modelId":"auto","name":"Auto","description":"Let Copilot pick"},"#,
        r#"{"modelId":"auto","name":"Auto","_meta":{"copilotEnablement":"enabled"}},"#,
        r#"{"modelId":"claude-sonnet-5.5","name":"Claude Sonnet 5.5","_meta":{"copilotEnablement":"disabled"}},"#,
        r#"{"modelId":"gpt-6-luna","name":"GPT-6 Luna","_meta":{"copilotEnablement":"enabled"}}]}}}"#,
    );

    fn runner(run_output: Result<ProcessOutput, ProcessError>) -> Arc<FakeProcessRunner> {
        Arc::new(FakeProcessRunner::new(
            &["copilot"],
            move |spec| match spec.args[0].as_str() {
                "--version" => ok("GitHub Copilot CLI 1.0.97.\n"),
                "--acp" => ok(ACP),
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
                memories: None,
                memory_rule: None,
                rules: None,
                system: "SYS".to_owned(),
                context: "CTX".to_owned(),
                instruction: "INS".to_owned(),
            },
            working_dir: PathBuf::from("/atlas"),
            extra_dirs: Vec::new(),
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
    fn detects_the_cli_and_offers_the_models_this_account_can_choose() {
        let status = inspect(&CopilotRuntime::new(runner(ok(""))));

        assert_eq!(status.availability, Availability::Ready);
        assert_eq!(
            status.version.as_deref(),
            Some("GitHub Copilot CLI 1.0.97.")
        );
        // No status command: the sign-in is not claimed.
        assert_eq!(status.authentication.state, AuthState::Unknown);
        assert_eq!(status.model_discovery, ModelDiscovery::Discovered);
        let ids: Vec<_> = status
            .available_models
            .iter()
            .map(|m| m.id.as_str())
            .collect();
        // `auto` once, and not the model the account has not enabled.
        assert_eq!(ids, ["auto", "gpt-6-luna"]);
    }

    #[test]
    fn an_answer_without_models_is_a_failed_discovery_not_an_empty_list() {
        let fake = Arc::new(FakeProcessRunner::new(&["copilot"], |spec| {
            match spec.args[0].as_str() {
                "--acp" => ok("{\"id\":2,\"result\":{\"models\":{\"availableModels\":[]}}}"),
                _ => ok("1.0.97\n"),
            }
        }));

        let status = inspect(&CopilotRuntime::new(fake));

        assert_eq!(status.model_discovery, ModelDiscovery::Failed);
    }

    #[test]
    fn runs_read_only_with_the_prompt_as_an_option_and_never_allows_everything() {
        let fake = runner(output(0, SUCCESS));

        let result = CopilotRuntime::new(fake.clone())
            .execute(&request("gpt-6-luna"), &|_| {})
            .unwrap();

        assert_eq!(result.text, "Three improvements.");
        assert_eq!(result.metadata["toolCalls"], "1");
        assert_eq!(result.metadata["premiumRequests"], "1");
        let usage = result.usage.unwrap();
        assert_eq!(usage.input_tokens, Some(100));
        assert_eq!(usage.cached_input_tokens, Some(40));
        assert_eq!(usage.total_tokens, Some(150));
        let calls = fake.calls.lock().unwrap();
        let run = calls.last().unwrap();
        assert_eq!(run.program, "copilot");
        assert_eq!(
            run.args,
            [
                "--output-format",
                "json",
                "--no-ask-user",
                "--no-auto-update",
                "--disable-builtin-mcps",
                "--disallow-temp-dir",
                "--model",
                "gpt-6-luna",
                &format!("--prompt={}", request("m").prompt.combined()),
            ]
        );
        assert_eq!(run.terminal, Some(TerminalRequest::new(false)));
        assert!(!run
            .args
            .iter()
            .any(|a| a.contains("allow-all") || a.contains("yolo") || a.contains("allow-tool")));
    }

    #[test]
    fn writing_is_allowed_only_when_the_execution_may_edit_and_never_commands() {
        let fake = runner(output(0, SUCCESS));
        let mut edit = request("m");
        edit.allow_edits = true;

        CopilotRuntime::new(fake.clone())
            .execute(&edit, &|_| {})
            .unwrap();

        let calls = fake.calls.lock().unwrap();
        let args = &calls.last().unwrap().args;
        assert!(args.contains(&"--allow-tool=write".to_owned()));
        assert!(!args.iter().any(|a| a.starts_with("--allow-tool=shell")));
        assert!(!CopilotRuntime::new(runner(ok(""))).runs_commands());
    }

    #[test]
    fn streams_messages_and_tool_calls_live() {
        let events = RefCell::new(Vec::new());

        CopilotRuntime::new(runner(output(0, SUCCESS)))
            .execute(&request("m"), &|e| events.borrow_mut().push(e))
            .unwrap();

        let live: Vec<_> = events.borrow().iter().skip(3).cloned().collect();
        assert_eq!(
            live,
            [
                RuntimeEvent::Output("Looking.\n\n".to_owned()),
                RuntimeEvent::ToolStarted("ls".to_owned()),
                RuntimeEvent::ToolCompleted("ls".to_owned()),
                RuntimeEvent::Output("Three improvements.\n\n".to_owned()),
                RuntimeEvent::ToolsReported(vec!["bash".to_owned(), "files-read".to_owned()]),
            ]
        );
    }

    #[test]
    fn classifies_failures() {
        let run = |out: Result<ProcessOutput, ProcessError>| {
            CopilotRuntime::new(runner(out)).execute(&request("m1"), &|_| {})
        };
        let failed = |stderr: &str| {
            Ok(ProcessOutput {
                exit_code: Some(1),
                stdout: String::new(),
                stderr: stderr.to_owned(),
            })
        };

        assert_eq!(
            run(failed("Error: No authentication information found.")),
            Err(RuntimeError::AuthenticationRequired)
        );
        assert_eq!(
            run(failed(
                "Error: Model \"m1\" from --model flag is not available."
            )),
            Err(RuntimeError::ModelUnavailable("m1".to_owned()))
        );
        assert!(matches!(
            run(failed("429 Too Many Requests")),
            Err(RuntimeError::RateLimited(_))
        ));
        assert!(matches!(
            run(output(0, "not json")),
            Err(RuntimeError::ExecutionFailed(_) | RuntimeError::UnexpectedResponse(_))
        ));
        assert_eq!(run(Err(ProcessError::Timeout)), Err(RuntimeError::Timeout));
    }

    #[test]
    fn a_run_whose_result_reports_a_failure_is_not_a_success() {
        let events = concat!(
            r#"{"type":"assistant.message","data":{"content":"half","toolRequests":[]}}"#,
            "\n",
            r#"{"type":"result","exitCode":1}"#,
        );

        let result = CopilotRuntime::new(runner(output(0, events))).execute(&request("m"), &|_| {});

        assert!(result.is_err());
    }

    #[test]
    fn refuses_model_ids_that_could_be_options() {
        let fake = runner(output(0, SUCCESS));

        let result = CopilotRuntime::new(fake.clone()).execute(&request("--yolo"), &|_| {});

        assert!(matches!(result, Err(RuntimeError::InvalidRequest(_))));
        assert_eq!(fake.calls.lock().unwrap().len(), 0);
    }

    fn args_of(fake: &FakeProcessRunner) -> Vec<String> {
        fake.calls.lock().unwrap().last().unwrap().args.clone()
    }

    fn with_mcp(mut request: RuntimeRequest) -> RuntimeRequest {
        use crate::application::mcp::{LaunchEnv, LaunchServer, McpLaunch};
        use crate::domain::mcp::Secret;
        request.mcp = Some(McpLaunch {
            servers: vec![LaunchServer {
                name: "files".to_owned(),
                executable: "node".to_owned(),
                args: vec!["s.js".to_owned()],
                env: vec![LaunchEnv::Secret {
                    name: "TOKEN".to_owned(),
                    value: Secret::new("hunter2".to_owned()),
                }],
            }],
            held_back: vec![],
            only: vec![],
        });
        request
    }

    #[test]
    fn a_text_only_run_is_given_no_tool_no_server_and_no_other_folder() {
        let fake = runner(output(0, SUCCESS));
        let mut req = with_mcp(request("m"));
        req.text_only = true;
        req.allow_edits = true;
        req.extra_dirs = vec![PathBuf::from("/shop/back")];

        CopilotRuntime::new(fake.clone())
            .execute(&req, &|_| {})
            .unwrap();

        let args = args_of(&fake);
        assert!(args.contains(&"--available-tools=atlas_no_tools".to_owned()));
        for forbidden in ["--additional-mcp-config", "--add-dir", "--allow-tool"] {
            assert!(
                !args.iter().any(|a| a.starts_with(forbidden)),
                "{forbidden}: {args:?}"
            );
        }
        let calls = fake.calls.lock().unwrap();
        assert_eq!(calls.last().unwrap().env, []);
        assert!(
            CopilotRuntime::new(runner(ok("")))
                .info()
                .capabilities
                .text_only
        );
    }

    #[test]
    fn the_servers_of_a_step_are_given_with_their_secret_in_the_environment_and_each_allowed() {
        let fake = runner(output(0, SUCCESS));

        CopilotRuntime::new(fake.clone())
            .execute(&with_mcp(request("m")), &|_| {})
            .unwrap();

        let args = args_of(&fake);
        let at = args
            .iter()
            .position(|a| a == "--additional-mcp-config")
            .unwrap();
        assert!(args[at + 1].starts_with('@'));
        assert!(args.contains(&"--allow-tool=files".to_owned()));
        // The prompt stays last, so nothing after it can be read as an option.
        assert!(args.last().unwrap().starts_with("--prompt="));
        assert!(!args.iter().any(|a| a.contains("hunter2")));
        let calls = fake.calls.lock().unwrap();
        assert_eq!(
            calls.last().unwrap().env,
            [("ATLAS_MCP_FILES_TOKEN".to_owned(), "hunter2".to_owned())]
        );
    }

    #[test]
    fn the_other_project_folders_are_given_one_option_each() {
        let fake = runner(output(0, SUCCESS));
        let mut req = request("m");
        req.extra_dirs = vec![PathBuf::from("/shop/back"), PathBuf::from("/shop/orders")];

        CopilotRuntime::new(fake.clone())
            .execute(&req, &|_| {})
            .unwrap();

        let args = args_of(&fake);
        let dirs: Vec<_> = args
            .iter()
            .filter(|a| a.starts_with("--add-dir="))
            .collect();
        assert_eq!(dirs, ["--add-dir=/shop/back", "--add-dir=/shop/orders"]);
        assert!(CopilotRuntime::new(runner(ok(""))).accepts_extra_dirs());
    }

    #[test]
    fn a_call_to_an_mcp_server_is_shown_in_the_shape_the_other_runtimes_use() {
        let stream = Stream::default();
        let start = r#"{"type":"tool.execution_start","data":{"toolCallId":"c9","toolName":"files-read","mcpServerName":"files","mcpToolName":"read","arguments":{}}}"#;
        let done = r#"{"type":"tool.execution_complete","data":{"toolCallId":"c9"}}"#;

        assert_eq!(
            stream.event(start),
            [RuntimeEvent::ToolStarted("mcp__files__read".to_owned())]
        );
        assert_eq!(
            stream.event(done),
            [RuntimeEvent::ToolCompleted("mcp__files__read".to_owned())]
        );
        let runtime = CopilotRuntime::new(runner(ok("")));
        let told = runtime.mcp_tools_in(
            &[
                "files-read".to_owned(),
                "mcp__files__read".to_owned(),
                "rg".to_owned(),
            ],
            &["files"],
        );
        assert_eq!(told.len(), 2);
        assert!(told
            .iter()
            .all(|t| t.server.as_deref() == Some("files") && t.tool == "read"));
    }

    #[test]
    fn the_tool_list_is_told_once_and_a_checkpoint_without_one_tells_nothing() {
        let stream = Stream::default();
        let without = r#"{"type":"session.usage_checkpoint","data":{}}"#;
        let empty = r#"{"type":"session.usage_checkpoint","data":{"promptCacheBreakState":[{"models":{"m":{"tools":[]}}}]}}"#;

        assert_eq!(stream.event(without), Vec::<RuntimeEvent>::new());
        // An empty list is a fact (the model had no tool); the second one is not told again.
        assert_eq!(stream.event(empty), [RuntimeEvent::ToolsReported(vec![])]);
        assert_eq!(stream.event(empty), Vec::<RuntimeEvent>::new());
    }

    /// The harmless MCP server of the Phase A spike; a call to its tool answers with the nonce.
    fn spike(secret: &str, nonce: &str) -> crate::application::mcp::LaunchServer {
        use crate::application::mcp::{LaunchEnv, LaunchServer};
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../docs/architecture/spikes/mcp-claude/echo-mcp.js");
        let plain = |name: &str, value: &str| LaunchEnv::Plain {
            name: name.to_owned(),
            value: value.to_owned(),
        };
        LaunchServer {
            name: "atlasspike".to_owned(),
            executable: "node".to_owned(),
            args: vec![path.to_string_lossy().into_owned()],
            env: vec![
                plain("MODE", "requireenv"),
                plain("REPORT_ENV", "MYSECRET"),
                plain("EXPECT", secret),
                plain("NONCE", nonce),
                LaunchEnv::Secret {
                    name: "MYSECRET".to_owned(),
                    value: crate::domain::mcp::Secret::new(secret.to_owned()),
                },
            ],
        }
    }

    fn guarded() -> CopilotRuntime {
        CopilotRuntime::new(
            crate::application::security::testutil::guarded_system_runner(&std::env::temp_dir()),
        )
    }

    /// Run with `cargo test real_copilot_calls -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the Copilot CLI signed in and node; makes a real model call (1 premium request)"]
    fn real_copilot_calls_a_granted_mcp_server_and_the_secret_reached_it() {
        use crate::application::mcp::McpLaunch;
        let runtime = guarded();
        let mut call = request(AUTO_MODEL);
        call.prompt.instruction = "Call the echo_static tool of the MCP server atlasspike and \
            reply with exactly the text it returned, nothing else."
            .to_owned();
        call.working_dir = std::env::temp_dir();
        call.mcp = Some(McpLaunch {
            servers: vec![spike("topsecret42", "nonce-7c1")],
            held_back: vec![],
            only: vec![("atlasspike".to_owned(), vec!["echo_static".to_owned()])],
        });
        let events = RefCell::new(Vec::new());

        let output = runtime
            .execute(&call, &|e| events.borrow_mut().push(e))
            .unwrap();

        println!("answer: {}\nevents: {:?}", output.text, events.borrow());
        // The nonce only comes back if the model really called the tool, so the server started
        // (the secret reached it: it exits otherwise) and the grant let the call through.
        assert!(
            output.text.contains("atlas-spike-called:nonce-7c1"),
            "{}",
            output.text
        );
        assert!(events.borrow().contains(&RuntimeEvent::ToolStarted(
            "mcp__atlasspike__echo_static".to_owned()
        )));
        let told: Vec<_> = events
            .borrow()
            .iter()
            .filter_map(|e| match e {
                RuntimeEvent::ToolsReported(tools) => {
                    Some(runtime.mcp_tools_in(tools, &["atlasspike"]))
                }
                _ => None,
            })
            .flatten()
            .collect();
        assert!(told
            .iter()
            .any(|t| t.tool == "echo_static" && t.server.as_deref() == Some("atlasspike")));
        // The allow-list kept the server's other tool (the one named after the secret) away.
        assert!(!told.iter().any(|t| t.tool.starts_with("env_")), "{told:?}");
    }

    /// Run with `cargo test real_copilot_text_only -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the Copilot CLI signed in; makes a real model call (1 premium request)"]
    fn real_copilot_text_only_is_sent_no_tool_even_with_the_users_own_servers() {
        let runtime = guarded();
        let mut ask = request(AUTO_MODEL);
        ask.text_only = true;
        ask.prompt.instruction = "Reply with exactly the word: pong".to_owned();
        ask.working_dir = std::env::temp_dir();
        let events = RefCell::new(Vec::new());

        let output = runtime
            .execute(&ask, &|e| events.borrow_mut().push(e))
            .unwrap();

        println!("answer: {}\nevents: {:?}", output.text, events.borrow());
        assert!(output.text.to_lowercase().contains("pong"));
        assert!(
            events
                .borrow()
                .contains(&RuntimeEvent::ToolsReported(Vec::new())),
            "the model was sent tools: {:?}",
            events.borrow()
        );
    }

    /// Talks to the real Copilot CLI on this machine. Run with
    /// `cargo test real_copilot -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the Copilot CLI installed and signed in; makes a real model call (1 premium request)"]
    fn real_copilot_answers_through_the_system_runner() {
        // Through the real guard, like the app: the scope's project folder is the temp dir.
        let runner: Arc<dyn ProcessRunner> =
            crate::application::security::testutil::guarded_system_runner(&std::env::temp_dir());
        let runtime = CopilotRuntime::new(runner);

        let status = inspect(&runtime);
        println!("detected: {status:#?}");
        assert_eq!(status.availability, Availability::Ready);
        // What the account can choose, and `auto` is always among it.
        assert!(status.available_models.iter().any(|m| m.id == AUTO_MODEL));
        println!("models of the account: {:?}", status.available_models);

        let mut ping = request(AUTO_MODEL);
        ping.prompt.instruction = "Reply with exactly the word: pong".to_owned();
        ping.working_dir = std::env::temp_dir();
        let output = runtime
            .execute(&ping, &|stage| println!("stage: {stage:?}"))
            .unwrap();
        println!("answer: {}\nusage: {:?}", output.text, output.usage);
        assert!(output.text.to_lowercase().contains("pong"));
        assert!(output.usage.is_some());

        let mut unknown = request("nope-nope");
        unknown.working_dir = std::env::temp_dir();
        let missing = runtime.execute(&unknown, &|_| {});
        assert!(matches!(missing, Err(RuntimeError::ModelUnavailable(_))));
    }
}
