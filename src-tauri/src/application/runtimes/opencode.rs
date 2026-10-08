use std::cell::Cell;
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
    SystemPromptChannel, Transport,
};
use crate::domain::security::ToolAccess;
use crate::domain::usage::{UsageMetrics, UsageSource};

const PROGRAM: &str = "opencode";
const LIST_TIMEOUT: Duration = Duration::from_secs(30);
/// Idle limit: silence from the CLI (long thinking or a slow tool) for this long ends the run.
const RUN_TIMEOUT: Duration = Duration::from_secs(900);

/// Runtime for the `OpenCode` CLI.
///
/// - models: `opencode models` (one `provider/model` per line)
/// - run: `opencode run --agent plan -m <model> --format json`, prompt on stdin, answer in `text`
///   events
///
/// `--agent plan` is `OpenCode`'s planning agent: it denies the `edit` tool. Without it a plain
/// `opencode run` edits files on its own (verified). It is *not* a sandbox, though: the plan
/// agent's permission table is `* -> allow` plus `edit -> deny`, so `OpenCode`'s shell and web
/// tools remain available to it. That is `OpenCode`'s own permission model, an extra
/// restriction layer that Atlas does not assume equals its own policy; see `tool_access` below
/// and ADR 0006. Atlas also never passes `--auto`.
pub struct OpenCodeRuntime {
    runner: Arc<dyn ProcessRunner>,
}

impl OpenCodeRuntime {
    pub fn new(runner: Arc<dyn ProcessRunner>) -> Self {
        Self { runner }
    }
}

impl ModelRuntime for OpenCodeRuntime {
    fn detect_interaction(
        &self,
        output: &RuntimeOutput,
    ) -> Option<crate::domain::interaction::InteractionDetection> {
        cli::detection_from_asking_tool(output)
    }

    fn info(&self) -> RuntimeInfo {
        RuntimeInfo {
            id: "opencode".to_owned(),
            name: "OpenCode CLI".to_owned(),
            provider: ProviderRef {
                id: "opencode".to_owned(),
                name: "OpenCode".to_owned(),
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
                cost_metrics: true,
                quota_metrics: false,
                // Runs attached to a terminal. `opencode run` takes its message from the command
                // line and ignores a terminal's stdin (verified), so the terminal is read-only.
                interactive_terminal: true,
                interrupt: true,
                terminal_input: false,
                terminal_resize: true,
                text_only: false,
                // `--agent build` (OpenCode's default agent) has the edit tool; `--agent plan` denies it.
                // Whether an execution gets it is decided per execution, like every runtime.
                file_edit: true,
                // `--agent plan` denies OpenCode's `edit` tool, but its permission table still
                // allows everything else (`* -> allow`, checked with `opencode agent list`):
                // the shell and web tools can write files and reach the network. Atlas cannot
                // narrow that without changing how OpenCode is launched, so it is reported as
                // it is.
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
            // Which models work depends on the credentials OpenCode itself holds, and it
            // has no cheap way to say. The model list reflects what is usable.
            authentication: Authentication {
                kind: Some(AuthKind::CliSession),
                state: AuthState::Unknown,
            },
            notice: None,
        }
    }

    fn list_models(&self) -> Result<Vec<ModelInfo>, RuntimeError> {
        let output = cli::probe(
            self.runner.as_ref(),
            PROGRAM,
            &["models"],
            Some(LIST_TIMEOUT),
        )
        .map_err(|error| {
            RuntimeError::ExecutionFailed(format!("`opencode models` failed: {error:?}"))
        })?;
        Ok(output
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.contains(char::is_whitespace))
            .map(|line| ModelInfo {
                id: line.to_owned(),
                name: line.to_owned(),
            })
            .collect())
    }

    fn execute(
        &self,
        request: &RuntimeRequest,
        progress: &dyn Fn(RuntimeEvent),
    ) -> Result<RuntimeOutput, RuntimeError> {
        progress(RuntimeEvent::Starting);
        if request.text_only {
            // OpenCode's shell and web tools cannot be switched off from here.
            return Err(RuntimeError::InvalidRequest(
                "OpenCode cannot run without tools".to_owned(),
            ));
        }
        cli::validate_model_id(&request.model_id)?;
        if self.runner.locate(PROGRAM).is_none() {
            return Err(RuntimeError::NotInstalled);
        }
        // If the model list is unavailable we still try: OpenCode will report the problem.
        if let Ok(models) = self.list_models() {
            if !models.iter().any(|model| model.id == request.model_id) {
                return Err(RuntimeError::ModelUnavailable(request.model_id.clone()));
            }
        }

        // OpenCode has no system-prompt flag, so everything goes in as one labelled prompt.
        // `plan` is its read-only agent; `build` can edit files, and is used only when the
        // execution was granted that (see `RuntimeRequest::allow_edits`).
        let agent = if request.allow_edits { "build" } else { "plan" };
        let args = [
            "run",
            "--agent",
            agent,
            "-m",
            &request.model_id,
            "--format",
            "json",
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
        let emitted_text = Cell::new(false);
        let output = cli::execute(self.runner.as_ref(), &spec, progress, &|line| {
            if let Some(event) = stream_event(line, &emitted_text) {
                progress(event);
            }
        })?;
        parse_run_output(&output, &request.model_id)
    }
}

/// Turns one line of `OpenCode`'s JSON stream into a live event. `OpenCode` writes each text
/// part when it is complete (not token by token) and reports a tool call once it has
/// finished, so those are the only live events it can honestly provide.
fn stream_event(line: &str, emitted_text: &Cell<bool>) -> Option<RuntimeEvent> {
    let event: Value = serde_json::from_str(line).ok()?;
    let part = &event["part"];
    match event["type"].as_str()? {
        "text" => {
            let text = part["text"].as_str()?.trim();
            if text.is_empty() {
                return None;
            }
            // Parts are separated by a blank line, so the pieces concatenate into readable text.
            let separator = if emitted_text.replace(true) {
                "\n\n"
            } else {
                ""
            };
            Some(RuntimeEvent::Output(format!("{separator}{text}")))
        }
        "tool_use" if part["state"]["status"] == "completed" => Some(RuntimeEvent::ToolCompleted(
            part["tool"].as_str()?.to_owned(),
        )),
        _ => None,
    }
}

/// Adds up the `step_finish` events: each reports the tokens and cost of one model step. Nothing
/// is estimated: if no step reported tokens or cost, the result is `None`. "Input" is every token
/// sent to the model (fresh plus cache reads and writes), "output" includes reasoning tokens. The
/// currency is not stated by `OpenCode`, so none is claimed.
fn usage_of(stdout: &str) -> Option<UsageMetrics> {
    let mut input: Option<u64> = None;
    let mut output: Option<u64> = None;
    let mut total: Option<u64> = None;
    let mut cached: Option<u64> = None;
    let mut cost: Option<f64> = None;
    let add = |sum: &mut Option<u64>, value: Option<u64>| {
        if let Some(value) = value {
            *sum = Some(sum.unwrap_or(0).saturating_add(value));
        }
    };
    for event in stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|event| event["type"] == "step_finish")
    {
        let part = &event["part"];
        let tokens = &part["tokens"];
        let n = |value: &Value| value.as_u64();
        let fresh = n(&tokens["input"]);
        let cache_read = n(&tokens["cache"]["read"]);
        let cache_write = n(&tokens["cache"]["write"]);
        add(
            &mut input,
            [fresh, cache_read, cache_write]
                .into_iter()
                .flatten()
                .reduce(u64::saturating_add),
        );
        add(
            &mut output,
            [n(&tokens["output"]), n(&tokens["reasoning"])]
                .into_iter()
                .flatten()
                .reduce(u64::saturating_add),
        );
        add(&mut total, n(&tokens["total"]));
        add(&mut cached, cache_read);
        if let Some(step_cost) = part["cost"].as_f64() {
            cost = Some(cost.unwrap_or(0.0) + step_cost);
        }
    }
    if input.is_none() && output.is_none() && total.is_none() && cost.is_none() {
        return None;
    }
    Some(UsageMetrics {
        input_tokens: input,
        output_tokens: output,
        total_tokens: total,
        cached_input_tokens: cached,
        cost,
        currency: None,
        source: UsageSource::RuntimeReported,
    })
}

/// Extracts the answer from `OpenCode`'s JSON event stream, or classifies its error.
///
/// The answer is the text written after the last tool call: the model's concluding message.
/// Narration before and between tool calls was already shown live. If the model wrote nothing
/// after its last tool call, everything it wrote is used instead.
fn parse_run_output(output: &ProcessOutput, model_id: &str) -> Result<RuntimeOutput, RuntimeError> {
    let mut all_texts = Vec::new();
    let mut texts_after_last_tool = Vec::new();
    let mut error: Option<String> = None;
    let mut metadata = std::collections::BTreeMap::new();
    // How the run ended: the tool call it stopped on, when that call failed and nothing was
    // written after it. OpenCode then exits 0 although the work was cut short.
    let mut stopped_on_tool: Option<String> = None;
    let rejected = output
        .stdout
        .lines()
        .find(|line| line.contains("permission requested") && line.contains("auto-rejecting"))
        .map(|line| line.trim_start_matches(['!', ' ']).to_owned());
    for event in output
        .stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
    {
        match event["type"].as_str() {
            Some("text") => {
                if let Some(text) = event["part"]["text"].as_str() {
                    let text = text.trim().to_owned();
                    all_texts.push(text.clone());
                    if !text.is_empty() {
                        // It wrote something after the failed call: the run went on.
                        stopped_on_tool = None;
                    }
                    texts_after_last_tool.push(text);
                }
            }
            Some("tool_use") => {
                texts_after_last_tool.clear();
                let state = &event["part"]["state"];
                if let Some(tool) = event["part"]["tool"].as_str() {
                    cli::note_asking_tool(&mut metadata, tool, &state["input"]);
                }
                stopped_on_tool = (state["status"] == "error").then(|| {
                    let tool = event["part"]["tool"].as_str().unwrap_or("tool");
                    let why = state["error"].as_str().unwrap_or("it failed");
                    format!("{tool}: {why}")
                });
            }
            Some("error") => {
                let name = event["error"]["name"].as_str().unwrap_or("Error");
                let message = event["error"]["data"]["message"].as_str().unwrap_or("");
                error = Some(format!("{name}: {message}"));
            }
            _ => {}
        }
    }

    if error.is_some() || output.exit_code != Some(0) {
        let details = error.unwrap_or_else(|| cli::diagnostics(output));
        return Err(cli::classify_failure(&details, model_id));
    }
    if let Some(call) = stopped_on_tool.filter(|_| !metadata.contains_key(cli::ASKED_TOOL)) {
        // The model's last act was a tool call that failed, and it wrote nothing after it: the
        // work was cut short, so what it said before is not an answer.
        let cause = match rejected {
            Some(why) => format!("{why} ({call})"),
            None => call,
        };
        return Err(RuntimeError::ExecutionFailed(format!(
            "OpenCode stopped after a tool call failed, with the work unfinished: {cause}"
        )));
    }
    let join = |texts: Vec<String>| {
        texts
            .into_iter()
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
    };
    let mut text = join(texts_after_last_tool);
    if text.is_empty() {
        text = join(all_texts);
    }
    if text.is_empty() {
        // It asked through a tool and wrote nothing: the question is the answer.
        text = metadata.get(cli::ASKED_INPUT).cloned().unwrap_or_default();
    }
    if text.is_empty() {
        return Err(RuntimeError::UnexpectedResponse(
            "OpenCode finished without producing any text.".to_owned(),
        ));
    }
    Ok(RuntimeOutput {
        text,
        metadata,
        usage: usage_of(&output.stdout),
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
    use crate::domain::usage::UsageSource;

    const EVENTS: &str = concat!(
        r#"{"type":"step_start","part":{}}"#,
        "\n",
        r#"{"type":"text","part":{"text":"Three improvements.\n"}}"#,
        "\n",
        r#"{"type":"step_finish","part":{}}"#,
        "\n"
    );

    fn runner(run_output: Result<ProcessOutput, ProcessError>) -> Arc<FakeProcessRunner> {
        Arc::new(FakeProcessRunner::new(
            &["opencode"],
            move |spec| match spec.args[0].as_str() {
                "--version" => ok("1.18.34\n"),
                "models" => ok("opencode/big-pickle\nopencode/other\n"),
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

    #[test]
    fn detects_version_and_discovers_models() {
        let status = inspect(&OpenCodeRuntime::new(runner(ok(""))));

        assert_eq!(status.availability, Availability::Ready);
        assert_eq!(status.version.as_deref(), Some("1.18.34"));
        assert_eq!(status.model_discovery, ModelDiscovery::Discovered);
        assert_eq!(status.runtime.provider.name, "OpenCode");
        let ids: Vec<_> = status
            .available_models
            .iter()
            .map(|m| m.id.as_str())
            .collect();
        assert_eq!(ids, ["opencode/big-pickle", "opencode/other"]);
    }

    #[test]
    fn reports_failed_discovery_instead_of_inventing_models() {
        let fake = Arc::new(FakeProcessRunner::new(&["opencode"], |spec| {
            if spec.args[0] == "models" {
                Err(ProcessError::Timeout)
            } else {
                ok("1.0\n")
            }
        }));

        let status = inspect(&OpenCodeRuntime::new(fake));

        assert_eq!(status.availability, Availability::Ready);
        assert_eq!(status.model_discovery, ModelDiscovery::Failed);
        assert_eq!(status.available_models, []);
        assert!(status.discovery_error.is_some());
    }

    #[test]
    fn the_build_agent_that_can_edit_is_used_only_when_the_execution_may_edit() {
        let run = |allow_edits: bool| {
            let fake = runner(ok(r#"{"type":"text","part":{"text":"done"}}"#));
            let mut edit = request("opencode/big-pickle");
            edit.allow_edits = allow_edits;
            OpenCodeRuntime::new(fake.clone())
                .execute(&edit, &|_| {})
                .unwrap();
            let calls = fake.calls.lock().unwrap();
            let args = calls.last().unwrap().args.clone();
            args[args.iter().position(|a| a == "--agent").unwrap() + 1].clone()
        };

        assert_eq!(run(false), "plan");
        assert_eq!(run(true), "build");
    }

    #[test]
    fn runs_the_selected_model_in_a_terminal_with_the_prompt_as_the_last_argument() {
        let fake = runner(ok(EVENTS));
        let stages = RefCell::new(Vec::new());

        let output = OpenCodeRuntime::new(fake.clone())
            .execute(&request("opencode/big-pickle"), &|s| {
                stages.borrow_mut().push(s);
            })
            .unwrap();

        assert_eq!(output.text, "Three improvements.");
        assert_eq!(
            *stages.borrow(),
            [
                RuntimeEvent::Starting,
                RuntimeEvent::Sending,
                RuntimeEvent::Waiting,
                RuntimeEvent::Output("Three improvements.".to_owned()),
            ]
        );
        let calls = fake.calls.lock().unwrap();
        let run = calls.last().unwrap();
        assert_eq!(
            run.args,
            [
                "run",
                "--agent",
                "plan",
                "-m",
                "opencode/big-pickle",
                "--format",
                "json",
                "--",
                &request("opencode/big-pickle").prompt.combined(),
            ]
        );
        assert_eq!(run.cwd, Some(PathBuf::from("/atlas")));
        assert_eq!(run.stdin, None);
        assert_eq!(run.terminal, Some(TerminalRequest::new(false)));
        let prompt = run.args.last().unwrap();
        assert!(prompt.contains("SYS") && prompt.contains("CTX") && prompt.contains("INS"));
        assert!(!run.args.iter().any(|a| a == "--auto"));
    }

    #[test]
    fn rejects_unlisted_or_unsafe_models_without_running() {
        let fake = runner(ok(EVENTS));
        let runtime = OpenCodeRuntime::new(fake.clone());

        assert_eq!(
            runtime.execute(&request("nope/x"), &|_| {}),
            Err(RuntimeError::ModelUnavailable("nope/x".to_owned()))
        );
        assert!(matches!(
            runtime.execute(&request("--auto"), &|_| {}),
            Err(RuntimeError::InvalidRequest(_))
        ));
        assert!(fake
            .calls
            .lock()
            .unwrap()
            .iter()
            .all(|c| c.args[0] != "run"));
    }

    #[test]
    fn reports_a_missing_installation() {
        let fake = Arc::new(FakeProcessRunner::new(&[], |_| ok("")));

        assert_eq!(
            OpenCodeRuntime::new(fake.clone()).execute(&request("m"), &|_| {}),
            Err(RuntimeError::NotInstalled)
        );
        assert_eq!(
            inspect(&OpenCodeRuntime::new(fake)).availability,
            Availability::NotInstalled
        );
    }

    #[test]
    fn classifies_failures_and_process_errors() {
        let failure = |stdout: &str| ProcessOutput {
            exit_code: Some(1),
            stdout: stdout.to_owned(),
            stderr: String::new(),
        };
        let run = |output: Result<ProcessOutput, ProcessError>| {
            OpenCodeRuntime::new(runner(output)).execute(&request("opencode/big-pickle"), &|_| {})
        };

        assert_eq!(
            run(Ok(failure(
                r#"{"type":"error","error":{"name":"ProviderAuthError","data":{"message":"x"}}}"#
            ))),
            Err(RuntimeError::AuthenticationRequired)
        );
        assert!(matches!(
            run(Ok(failure(r#"{"type":"error","error":{"name":"UnknownError","data":{"message":"boom"}}}"#))),
            Err(RuntimeError::ExecutionFailed(d)) if d == "UnknownError: boom"
        ));
        assert!(matches!(
            run(Ok(ProcessOutput {
                exit_code: Some(0),
                stdout: "garbage".to_owned(),
                stderr: String::new()
            })),
            Err(RuntimeError::UnexpectedResponse(_))
        ));
        assert_eq!(run(Err(ProcessError::Timeout)), Err(RuntimeError::Timeout));
        assert_eq!(
            run(Err(ProcessError::Spawn("bad cwd".to_owned()))),
            Err(RuntimeError::ExecutionFailed("bad cwd".to_owned()))
        );
    }

    fn narrated_run() -> String {
        [
            r#"{"type":"step_start","part":{}}"#,
            r#"{"type":"text","part":{"text":"I'll read the README first."}}"#,
            r#"{"type":"tool_use","part":{"tool":"read","state":{"status":"completed"}}}"#,
            r#"{"type":"step_finish","part":{}}"#,
            r#"{"type":"text","part":{"text":"Now I'll summarize it.\n\nAtlas is a desktop app."}}"#,
            r#"{"type":"step_finish","part":{}}"#,
        ]
        .join("\n")
    }

    #[test]
    fn streams_narration_and_tool_calls_live_but_answers_with_the_concluding_message() {
        let fake = runner(ok(&narrated_run()));
        let events = RefCell::new(Vec::new());

        let output = OpenCodeRuntime::new(fake)
            .execute(&request("opencode/big-pickle"), &|e| {
                events.borrow_mut().push(e);
            })
            .unwrap();

        let live: Vec<_> = events.borrow().iter().skip(3).cloned().collect();
        assert_eq!(
            live,
            [
                RuntimeEvent::Output("I'll read the README first.".to_owned()),
                RuntimeEvent::ToolCompleted("read".to_owned()),
                RuntimeEvent::Output(
                    "\n\nNow I'll summarize it.\n\nAtlas is a desktop app.".to_owned()
                ),
            ]
        );
        // The stored answer is what the model wrote after its last tool call.
        assert_eq!(
            output.text,
            "Now I'll summarize it.\n\nAtlas is a desktop app."
        );
    }

    fn run_output(stdout: String) -> ProcessOutput {
        ProcessOutput {
            stdout,
            ..ok("").unwrap()
        }
    }

    #[test]
    fn a_run_that_ends_on_a_failed_tool_call_is_cut_short_and_not_an_answer() {
        let stdout = [
            r#"{"type":"text","part":{"text":"I'll analyze the payment system."}}"#,
            "! permission requested: external_directory (/Users/me/Library/*); auto-rejecting",
            r#"{"type":"tool_use","part":{"tool":"bash","state":{"status":"error","error":"The user rejected permission to use this specific tool call."}}}"#,
            r#"{"type":"step_finish","part":{}}"#,
        ]
        .join("\n");

        let result = parse_run_output(&run_output(stdout), "m");

        let Err(RuntimeError::ExecutionFailed(details)) = result else {
            panic!("expected the run to count as cut short: {result:?}");
        };
        assert!(details.contains("external_directory"));
        assert!(details.contains("bash"));
    }

    #[test]
    fn a_failed_tool_call_followed_by_an_answer_is_still_an_answer() {
        let stdout = [
            r#"{"type":"tool_use","part":{"tool":"read","state":{"status":"error","error":"no such file"}}}"#,
            r#"{"type":"text","part":{"text":"It does not exist, so I did X."}}"#,
        ]
        .join("\n");

        let result = parse_run_output(&run_output(stdout), "m").unwrap();

        assert_eq!(result.text, "It does not exist, so I did X.");
    }

    #[test]
    fn falls_back_to_everything_written_when_nothing_follows_the_last_tool() {
        let only_narration = [
            r#"{"type":"text","part":{"text":"Checking the file."}}"#,
            r#"{"type":"tool_use","part":{"tool":"read","state":{"status":"completed"}}}"#,
        ]
        .join("\n");

        let output = OpenCodeRuntime::new(runner(ok(&only_narration)))
            .execute(&request("opencode/big-pickle"), &|_| {})
            .unwrap();

        assert_eq!(output.text, "Checking the file.");
    }

    #[test]
    fn adds_up_the_tokens_and_cost_of_every_step_the_cli_reports() {
        let steps = [
            r#"{"type":"text","part":{"text":"hi"}}"#,
            r#"{"type":"step_finish","part":{"cost":0.25,"tokens":{"total":120,"input":100,"output":5,"reasoning":5,"cache":{"write":0,"read":10}}}}"#,
            r#"{"type":"step_finish","part":{"cost":0.5,"tokens":{"total":60,"input":50,"output":2,"reasoning":0,"cache":{"write":3,"read":5}}}}"#,
        ]
        .join("\n");

        let output = OpenCodeRuntime::new(runner(ok(&steps)))
            .execute(&request("opencode/big-pickle"), &|_| {})
            .unwrap();

        let usage = output.usage.unwrap();
        assert_eq!(usage.input_tokens, Some(100 + 10 + 50 + 3 + 5));
        assert_eq!(usage.output_tokens, Some(5 + 5 + 2));
        assert_eq!(usage.total_tokens, Some(180));
        assert_eq!(usage.cost, Some(0.75));
        assert_eq!(
            usage.currency, None,
            "OpenCode does not say which currency, so none is claimed"
        );
        assert_eq!(usage.source, UsageSource::RuntimeReported);
        assert_eq!(output.quota, None, "OpenCode reports no quota");
    }

    #[test]
    fn a_free_model_reporting_zero_cost_is_zero_and_missing_data_is_unavailable() {
        let free = concat!(
            r#"{"type":"text","part":{"text":"hi"}}"#,
            "\n",
            r#"{"type":"step_finish","part":{"cost":0,"tokens":{"total":10,"input":8,"output":2,"cache":{"write":0,"read":0}}}}"#
        );
        let silent = r#"{"type":"text","part":{"text":"hi"}}"#;
        let run = |stdout: &str| {
            OpenCodeRuntime::new(runner(ok(stdout)))
                .execute(&request("opencode/big-pickle"), &|_| {})
                .unwrap()
        };

        assert_eq!(run(free).usage.unwrap().cost, Some(0.0));
        assert_eq!(run(silent).usage, None);
    }

    /// Talks to the real `OpenCode` on this machine. Run with
    /// `cargo test real_opencode -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs OpenCode installed and a free model; makes a real model call"]
    fn real_opencode_answers_through_the_system_runner() {
        // Through the real guard, like the app: the scope's project folder is the temp dir.
        let runner: Arc<dyn ProcessRunner> =
            crate::application::security::testutil::guarded_system_runner(&std::env::temp_dir());
        let runtime = OpenCodeRuntime::new(runner);

        let status = inspect(&runtime);
        println!("detected: {status:#?}");
        assert_eq!(status.availability, Availability::Ready);
        let model = status
            .available_models
            .first()
            .expect("no models")
            .id
            .clone();

        let mut ping = request(&model);
        ping.prompt.instruction = "Reply with exactly the word: pong".to_owned();
        ping.working_dir = std::env::temp_dir();
        let output = runtime
            .execute(&ping, &|stage| println!("stage: {stage:?}"))
            .unwrap();
        println!("answer: {}", output.text);
        assert!(output.text.to_lowercase().contains("pong"));

        let missing = runtime.execute(&request("nope/nope"), &|_| {});
        assert!(matches!(missing, Err(RuntimeError::ModelUnavailable(_))));
    }

    #[test]
    fn a_call_to_the_question_tool_is_a_request_for_the_person_and_not_a_cut_short_run() {
        let stdout = concat!(
            r#"{"type":"tool_use","part":{"tool":"question","state":{"status":"error","error":"denied","#,
            r#""input":{"questions":[{"question":"Qual banco?","options":[{"label":"Postgres"}]}]}}}}"#,
        );
        let out = parse_run_output(&run_output(stdout.to_owned()), "p/m").unwrap();
        let found = cli::detection_from_asking_tool(&out).expect("a structured signal");
        assert_eq!(found.question, "Qual banco?");
        assert_eq!(found.options[0].label, "Postgres");
    }
}
