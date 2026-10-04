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
    Transport,
};
use crate::domain::security::ToolAccess;
use crate::domain::usage::{UsageMetrics, UsageSource};

/// The Antigravity CLI installs as `agy`.
const PROGRAM: &str = "agy";
/// Idle limit: silence from the CLI (long thinking or a slow tool) for this long ends the run.
const RUN_TIMEOUT: Duration = Duration::from_secs(900);
const LIST_TIMEOUT: Duration = Duration::from_secs(60);

/// Runtime for the Antigravity CLI (`agy`), using the user's existing Google sign-in.
///
/// - version: `agy --version`
/// - models: `agy models` (one `id<TAB>name` per line; the ids already carry the reasoning
///   effort, e.g. `gemini-3.8-flash-high`)
/// - run: `agy --output-format stream-json --model <id> --prompt=<prompt>`. The CLI streams
///   `step_update` events (answer text as `text_delta`, tools as `tool` steps) and ends with a
///   `result` event carrying the response, token usage and the actions it had to deny
/// - sign-in: the CLI has no status command, so it is not reported (`Unknown`); a run that fails
///   for it is classified from its output.
///
/// `-p` takes its prompt as a value and the CLI does not read stdin for it, so the prompt goes as
/// `--prompt=<text>` (see `cli::deliver_prompt_option`).
///
/// There is no flag that turns tools off or picks them. What a headless run may do is decided by
/// the CLI's permission mode: by default every tool that would need a prompt (writing a file,
/// running a command) is denied, and reading the project is allowed. With `allow_edits` the run
/// uses `--mode accept-edits`. `--dangerously-skip-permissions` is never used.
pub struct AntigravityRuntime {
    runner: Arc<dyn ProcessRunner>,
}

impl AntigravityRuntime {
    pub fn new(runner: Arc<dyn ProcessRunner>) -> Self {
        Self { runner }
    }
}

impl ModelRuntime for AntigravityRuntime {
    fn detect_interaction(
        &self,
        output: &RuntimeOutput,
    ) -> Option<crate::domain::interaction::InteractionDetection> {
        cli::detection_from_asking_tool(output)
    }

    fn info(&self) -> RuntimeInfo {
        RuntimeInfo {
            id: "antigravity".to_owned(),
            name: "Antigravity CLI".to_owned(),
            provider: ProviderRef {
                id: "google".to_owned(),
                name: "Google".to_owned(),
            },
            transport: Transport::Cli,
            capabilities: RuntimeCapabilities {
                model_discovery: true,
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
                // `--mode accept-edits` approves file edits; shell commands stay refused.
                file_edit: true,
                // The CLI's own permission rules decide, not Atlas: the user's `settings.json`
                // can allow commands (`permissions.allow`) and files outside the workspace
                // (`allowNonWorkspaceAccess`), and it has web and browser tools. Atlas cannot
                // narrow that from outside, so it is reported as it is.
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
        let stdout = cli::probe(
            self.runner.as_ref(),
            PROGRAM,
            &["models"],
            Some(LIST_TIMEOUT),
        )
        .map_err(|error| {
            RuntimeError::ExecutionFailed(format!("`agy models` failed: {error:?}"))
        })?;
        Ok(parse_models(&stdout))
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

        let mut args = [
            "--output-format",
            "stream-json",
            "--model",
            &request.model_id,
        ]
        .map(str::to_owned)
        .to_vec();
        if request.allow_edits && !request.text_only {
            args.extend(["--mode".to_owned(), "accept-edits".to_owned()]);
        }
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
        let output = cli::execute(self.runner.as_ref(), &spec, progress, &|line| {
            if let Some(event) = stream_event(line) {
                progress(event);
            }
        })?;
        parse_run_output(&output, &request.model_id)
    }
}

/// `id<TAB>name` lines; anything else (the "Fetching…" banner) is not a model.
fn parse_models(stdout: &str) -> Vec<ModelInfo> {
    stdout
        .lines()
        .filter_map(|line| {
            let (id, name) = line.split_once('\t')?;
            let (id, name) = (id.trim(), name.trim());
            (!id.is_empty() && !id.contains(char::is_whitespace)).then(|| ModelInfo {
                id: id.to_owned(),
                name: if name.is_empty() { id } else { name }.to_owned(),
            })
        })
        .collect()
}

fn step_update(line: &str) -> Option<Value> {
    let value: Value = serde_json::from_str(line).ok()?;
    (value["event"] == "step_update").then(|| value["step_update"].clone())
}

/// Turns one `stream-json` line into a live event: answer text as it is written, and tool calls.
/// The model's "thinking" is not part of the stream, so it is not shown.
fn stream_event(line: &str) -> Option<RuntimeEvent> {
    let step = step_update(line)?;
    match step["step_type"].as_str()? {
        "agent_response" => {
            let text = step["text_delta"]
                .as_str()
                .filter(|text| !text.is_empty())?;
            Some(RuntimeEvent::Output(text.to_owned()))
        }
        "tool" => {
            let name = step["tool_name"].as_str()?.to_owned();
            match step["state"].as_str()? {
                "ACTIVE" => Some(RuntimeEvent::ToolStarted(name)),
                "DONE" | "ERROR" => Some(RuntimeEvent::ToolCompleted(name)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Tokens exactly as the `result` event reports them. "Input" is every token sent to the model
/// (fresh and cache reads); the CLI counts thinking tokens inside the output.
fn usage_of(result: &Value) -> Option<UsageMetrics> {
    let usage = &result["usage"];
    let part = |key: &str| usage[key].as_u64();
    let input_tokens = match (part("input_tokens"), part("cache_read_tokens")) {
        (Some(fresh), cached) => Some(fresh.saturating_add(cached.unwrap_or(0))),
        (None, _) => None,
    };
    let output_tokens = part("output_tokens");
    if input_tokens.is_none() && output_tokens.is_none() {
        return None;
    }
    let total_tokens = match (input_tokens, output_tokens) {
        (Some(i), Some(o)) => Some(i.saturating_add(o)),
        _ => None,
    };
    Some(UsageMetrics {
        input_tokens,
        output_tokens,
        total_tokens,
        cost: None,
        currency: None,
        source: UsageSource::RuntimeReported,
    })
}

/// Reads the `result` event that ends the stream. Its `response` is the concluding message;
/// when it is empty the text streamed before it is what the agent said.
fn parse_run_output(output: &ProcessOutput, model_id: &str) -> Result<RuntimeOutput, RuntimeError> {
    let events: Vec<Value> = output
        .stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line.trim()).ok())
        .collect();
    let Some(result) = events
        .iter()
        .rev()
        .find(|value| value["event"] == "result")
        .map(|value| &value["result"])
    else {
        return if output.exit_code == Some(0) {
            Err(RuntimeError::UnexpectedResponse(
                "Antigravity did not return a result event.".to_owned(),
            ))
        } else {
            Err(cli::classify_failure(&cli::diagnostics(output), model_id))
        };
    };

    let response = result["response"].as_str().unwrap_or("").trim().to_owned();
    if result["status"].as_str().is_some_and(|s| s != "SUCCESS") || output.exit_code != Some(0) {
        let details = if response.is_empty() {
            cli::diagnostics(output)
        } else {
            response
        };
        return Err(cli::classify_failure(&details, model_id));
    }

    let streamed: String = events
        .iter()
        .filter(|value| value["event"] == "step_update")
        .map(|value| &value["step_update"])
        .filter(|step| step["step_type"] == "agent_response")
        .filter_map(|step| step["text_delta"].as_str())
        .collect();
    let text = if response.is_empty() {
        streamed.trim().to_owned()
    } else {
        response
    };
    if text.is_empty() {
        let denied: Vec<&str> = result["denied_actions"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|action| action["action"].as_str())
            .collect();
        return Err(if denied.is_empty() {
            RuntimeError::UnexpectedResponse("Antigravity returned an empty result.".to_owned())
        } else {
            // The run needed a permission a headless run cannot ask for, and said nothing else.
            RuntimeError::ExecutionFailed(format!(
                "Antigravity was refused permission it cannot ask for in a non-interactive run ({}). {}",
                denied.join(", "),
                cli::diagnostics(output)
            ))
        });
    }

    let mut metadata = BTreeMap::new();
    // A tool that asks the person was called: nobody can answer it in this run.
    for step in events
        .iter()
        .filter(|value| value["event"] == "step_update")
        .map(|value| &value["step_update"])
        .filter(|step| step["step_type"] == "tool")
    {
        if let Some(tool) = step["tool_name"].as_str() {
            cli::note_asking_tool(&mut metadata, tool, &step["tool_info"]["parameters"]);
        }
    }
    if let Some(seconds) = result["duration_seconds"].as_f64() {
        metadata.insert(
            "durationMs".to_owned(),
            (seconds * 1000.0).round().to_string(),
        );
    }
    if let Some(turns) = result["num_turns"].as_u64() {
        metadata.insert("turns".to_owned(), turns.to_string());
    }
    Ok(RuntimeOutput {
        text,
        metadata,
        usage: usage_of(result),
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

    const MODELS: &str = "Fetching available models...\n\
        gemini-3.8-flash-high\tGemini 3.8 Flash (High)\n\
        claude-sonnet-5-5-low\tClaude Sonnet 5.5 (Low)\n";
    const SUCCESS: &str = r#"{"event":"result","result":{"status":"SUCCESS","response":"Three improvements.\n","duration_seconds":1.5,"num_turns":2,"usage":{"input_tokens":100,"output_tokens":40,"thinking_tokens":10,"cache_read_tokens":900,"total_tokens":140}}}"#;

    fn runner(run_output: Result<ProcessOutput, ProcessError>) -> Arc<FakeProcessRunner> {
        Arc::new(FakeProcessRunner::new(&["agy"], move |spec| {
            match spec.args[0].as_str() {
                "--version" => ok("1.2.16\n"),
                "models" => ok(MODELS),
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
    fn detects_the_agy_binary_and_discovers_its_models() {
        let status = inspect(&AntigravityRuntime::new(runner(ok(""))));

        assert_eq!(status.availability, Availability::Ready);
        assert_eq!(status.version.as_deref(), Some("1.2.16"));
        assert_eq!(status.model_discovery, ModelDiscovery::Discovered);
        let ids: Vec<_> = status
            .available_models
            .iter()
            .map(|m| m.id.as_str())
            .collect();
        assert_eq!(ids, ["gemini-3.8-flash-high", "claude-sonnet-5-5-low"]);
        assert_eq!(status.available_models[0].name, "Gemini 3.8 Flash (High)");
        assert!(status.runtime.capabilities.non_interactive_execution);
    }

    #[test]
    fn detects_a_missing_installation_without_running_anything() {
        let fake = Arc::new(FakeProcessRunner::new(&[], |_| ok("")));

        let status = inspect(&AntigravityRuntime::new(fake.clone()));

        assert_eq!(status.availability, Availability::NotInstalled);
        assert_eq!(fake.calls.lock().unwrap().len(), 0);
    }

    #[test]
    fn runs_with_the_selected_model_in_a_terminal_with_the_prompt_as_an_option_value() {
        let fake = runner(output(0, SUCCESS));

        let result = AntigravityRuntime::new(fake.clone())
            .execute(&request("gemini-3.8-flash-high"), &|_| {})
            .unwrap();

        assert_eq!(result.text, "Three improvements.");
        assert_eq!(result.metadata["durationMs"], "1500");
        assert_eq!(result.metadata["turns"], "2");
        let usage = result.usage.unwrap();
        assert_eq!(usage.input_tokens, Some(1000));
        assert_eq!(usage.output_tokens, Some(40));
        assert_eq!(usage.total_tokens, Some(1040));
        assert_eq!(usage.cost, None);
        let calls = fake.calls.lock().unwrap();
        let run = calls.last().unwrap();
        assert_eq!(run.program, "agy");
        assert_eq!(
            run.args,
            [
                "--output-format",
                "stream-json",
                "--model",
                "gemini-3.8-flash-high",
                &format!("--prompt={}", request("m").prompt.combined()),
            ]
        );
        assert_eq!(run.cwd, Some(PathBuf::from("/atlas")));
        assert_eq!(run.stdin, None);
        assert_eq!(run.terminal, Some(TerminalRequest::new(false)));
        assert!(!run.args.iter().any(|a| a.contains("dangerously")));
    }

    #[test]
    fn edits_are_approved_only_when_the_execution_may_edit() {
        let fake = runner(output(0, SUCCESS));
        let mut edit = request("m");
        edit.allow_edits = true;

        AntigravityRuntime::new(fake.clone())
            .execute(&edit, &|_| {})
            .unwrap();

        let calls = fake.calls.lock().unwrap();
        let args = &calls.last().unwrap().args;
        let at = args.iter().position(|a| a == "--mode").unwrap();
        assert_eq!(args[at + 1], "accept-edits");
    }

    #[test]
    fn streams_answer_text_and_tool_calls_live() {
        let stream = [
            r#"{"event":"init","init":{"tools":[]}}"#,
            r#"{"event":"step_update","step_update":{"step_index":0,"state":"DONE","step_type":"user_input"}}"#,
            r#"{"event":"step_update","step_update":{"step_index":1,"state":"ACTIVE","step_type":"agent_response","text_delta":"Let me look. "}}"#,
            r#"{"event":"step_update","step_update":{"step_index":2,"state":"ACTIVE","step_type":"tool","tool_name":"view_file"}}"#,
            r#"{"event":"step_update","step_update":{"step_index":2,"state":"DONE","step_type":"tool","tool_name":"view_file"}}"#,
            r#"{"event":"step_update","step_update":{"step_index":3,"state":"DONE","step_type":"agent_response","text_delta":"Found it.\n"}}"#,
            r#"{"event":"result","result":{"status":"SUCCESS","response":"Found it.\n"}}"#,
        ]
        .join("\n");
        let events = RefCell::new(Vec::new());

        let result = AntigravityRuntime::new(runner(output(0, &stream)))
            .execute(&request("m"), &|e| events.borrow_mut().push(e))
            .unwrap();

        let live: Vec<_> = events.borrow().iter().skip(3).cloned().collect();
        assert_eq!(
            live,
            [
                RuntimeEvent::Output("Let me look. ".to_owned()),
                RuntimeEvent::ToolStarted("view_file".to_owned()),
                RuntimeEvent::ToolCompleted("view_file".to_owned()),
                RuntimeEvent::Output("Found it.\n".to_owned()),
            ]
        );
        assert_eq!(result.text, "Found it.");
    }

    #[test]
    fn an_empty_response_falls_back_to_the_streamed_text() {
        let stream = [
            r#"{"event":"step_update","step_update":{"step_type":"agent_response","state":"DONE","text_delta":"All done."}}"#,
            r#"{"event":"result","result":{"status":"SUCCESS","response":""}}"#,
        ]
        .join("\n");

        let result = AntigravityRuntime::new(runner(output(0, &stream)))
            .execute(&request("m"), &|_| {})
            .unwrap();

        assert_eq!(result.text, "All done.");
    }

    #[test]
    fn a_run_refused_a_permission_it_cannot_ask_for_says_so() {
        let stream = [
            r#"{"event":"step_update","step_update":{"step_type":"tool","state":"ERROR","tool_name":"write_to_file"}}"#,
            r#"jetski: no output produced — a tool required the "write_file" permission that headless mode cannot prompt for"#,
            r#"{"event":"result","result":{"status":"SUCCESS","response":"","denied_actions":[{"action":"write_file","display_name":"WriteToFile"}]}}"#,
        ]
        .join("\n");

        let error = AntigravityRuntime::new(runner(output(0, &stream)))
            .execute(&request("m"), &|_| {})
            .unwrap_err();

        assert!(
            matches!(&error, RuntimeError::ExecutionFailed(d) if d.contains("write_file") && d.contains("headless")),
            "{error:?}"
        );
    }

    #[test]
    fn a_tool_that_asks_a_person_is_reported_for_the_interaction_flow() {
        let stream = [
            r#"{"event":"step_update","step_update":{"step_type":"tool","state":"DONE","tool_name":"ask_question","tool_info":{"parameters":{"question":"Which database?","options":[{"label":"Postgres"}]}}}}"#,
            r#"{"event":"result","result":{"status":"SUCCESS","response":"Which database?"}}"#,
        ]
        .join("\n");
        let runtime = AntigravityRuntime::new(runner(output(0, &stream)));

        let result = runtime.execute(&request("m"), &|_| {}).unwrap();

        let detection = runtime.detect_interaction(&result).unwrap();
        assert_eq!(detection.question, "Which database?");
        assert_eq!(detection.options.len(), 1);
    }

    #[test]
    fn classifies_failures() {
        let run = |out: Result<ProcessOutput, ProcessError>| {
            AntigravityRuntime::new(runner(out)).execute(&request("m1"), &|_| {})
        };

        assert_eq!(
            run(output(
                1,
                r#"{"event":"result","result":{"status":"ERROR","response":"Failed to authenticate: login required"}}"#
            )),
            Err(RuntimeError::AuthenticationRequired)
        );
        assert!(matches!(
            run(output(
                1,
                r#"{"event":"result","result":{"status":"ERROR","response":"RESOURCE_EXHAUSTED: 429"}}"#
            )),
            Err(RuntimeError::RateLimited(_))
        ));
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
        let fake = runner(output(0, SUCCESS));

        let result = AntigravityRuntime::new(fake.clone())
            .execute(&request("--dangerously-skip-permissions"), &|_| {});

        assert!(matches!(result, Err(RuntimeError::InvalidRequest(_))));
        assert_eq!(fake.calls.lock().unwrap().len(), 0);
    }
    /// Talks to the real Antigravity CLI on this machine. Run with
    /// `cargo test real_antigravity -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the agy CLI installed and signed in; makes a real model call"]
    fn real_antigravity_answers_through_the_system_runner() {
        // Through the real guard, like the app: the scope's project folder is the temp dir.
        let runner: Arc<dyn ProcessRunner> =
            crate::application::security::testutil::guarded_system_runner(&std::env::temp_dir());
        let runtime = AntigravityRuntime::new(runner);

        let status = inspect(&runtime);
        println!("detected: {status:#?}");
        assert_eq!(status.availability, Availability::Ready);
        let model = status
            .available_models
            .iter()
            .find(|m| m.id.contains("flash-low"))
            .or(status.available_models.first())
            .expect("no models")
            .id
            .clone();

        let mut ping = request(&model);
        ping.prompt.instruction = "Reply with exactly the word: pong".to_owned();
        ping.working_dir = std::env::temp_dir();
        let output = runtime
            .execute(&ping, &|stage| println!("stage: {stage:?}"))
            .unwrap();
        println!("answer: {} / usage: {:?}", output.text, output.usage);
        assert!(output.text.to_lowercase().contains("pong"));
    }
}
