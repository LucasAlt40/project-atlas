//! Helpers shared by runtimes that are command-line tools.

use std::time::Duration;

use super::{RuntimeError, RuntimeEvent, RuntimeOutput};
use crate::application::process::{
    ProcessError, ProcessEvent, ProcessOutput, ProcessRunner, ProcessSpec, TerminalRequest,
};

const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_DETAILS_CHARS: usize = 2000;

/// Runs `program <args…>` with no input and returns its output whatever the exit code
/// (some tools report state, such as "signed out", with a non-zero exit). The arguments are
/// fixed by the calling runtime: probes only ever run harmless commands like `--version`.
pub fn capture(
    runner: &dyn ProcessRunner,
    program: &str,
    args: &[&str],
    timeout: Option<Duration>,
) -> Result<ProcessOutput, ProcessError> {
    let spec = ProcessSpec::probe(program, args, timeout.unwrap_or(PROBE_TIMEOUT));
    runner.run(&spec, &|_| {})
}

/// Like [`capture`], but only returns stdout of a command that exited cleanly.
pub fn probe(
    runner: &dyn ProcessRunner,
    program: &str,
    args: &[&str],
    timeout: Option<Duration>,
) -> Result<String, ProcessError> {
    let output = capture(runner, program, args, timeout)?;
    if output.exit_code == Some(0) {
        Ok(output.stdout)
    } else {
        Err(ProcessError::Io(format!(
            "`{program} {}` exited with {:?}",
            args.join(" "),
            output.exit_code
        )))
    }
}

/// The first non-empty line of `program --version`, if the tool reports one.
pub fn version(runner: &dyn ProcessRunner, program: &str) -> Option<String> {
    probe(runner, program, &["--version"], None)
        .ok()
        .and_then(|out| {
            out.lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .map(|line| line.chars().take(100).collect())
        })
}

/// How a prompt reaches a CLI that is run non-interactively.
pub struct PromptDelivery {
    pub args: Vec<String>,
    pub stdin: Option<String>,
    pub terminal: Option<TerminalRequest>,
}

/// Decides how to run a CLI whose prompt is `prompt`: attached to a terminal, so the user can
/// watch and control the live process, or (when that is not possible) through pipes as before.
///
/// A terminal's input is the keyboard, and `claude -p` / `opencode run` do not read it, so in a
/// terminal the prompt goes as the last argument, after `--` so that text starting with `-`
/// is never taken for an option. `args` must not end with an option that takes several values.
/// `terminal_input` is the runtime's own `terminal_input` capability.
pub fn deliver_prompt(
    runner: &dyn ProcessRunner,
    program: &str,
    mut args: Vec<String>,
    prompt: String,
    terminal_input: bool,
) -> PromptDelivery {
    if !terminal_fits(runner, program, &prompt) {
        return PromptDelivery {
            args,
            stdin: Some(prompt),
            terminal: None,
        };
    }
    args.push("--".to_owned());
    args.push(prompt);
    PromptDelivery {
        args,
        stdin: None,
        terminal: Some(TerminalRequest::new(terminal_input)),
    }
}

/// The longest prompt that still fits on a command line (the operating system's limit for one
/// argument is about 128 KB on Linux and 32 KB on Windows).
const MAX_PROMPT_ARGUMENT_BYTES: usize = if cfg!(windows) { 30_000 } else { 120_000 };

/// Like [`deliver_prompt`], for a CLI that only takes its prompt as the value of an option
/// (`agy --prompt=…`, `gemini --prompt=…`) and does not read it from stdin. The `=` form keeps a
/// prompt that starts with `-` from being taken for another option. The run is attached to a
/// terminal (read-only, `terminal_input` off) so the user can watch it.
///
/// # Errors
///
/// Fails when the prompt cannot be passed as one argument: it is too long, or it is multi-line
/// for an npm `.cmd` shim on Windows.
pub fn deliver_prompt_option(
    runner: &dyn ProcessRunner,
    program: &str,
    option: &str,
    mut args: Vec<String>,
    prompt: &str,
) -> Result<PromptDelivery, RuntimeError> {
    if prompt.len() > MAX_PROMPT_ARGUMENT_BYTES {
        return Err(RuntimeError::InvalidRequest(format!(
            "The prompt is too large to pass to {program} on the command line ({} KB; the limit is {} KB).",
            prompt.len() / 1000,
            MAX_PROMPT_ARGUMENT_BYTES / 1000
        )));
    }
    if !terminal_fits(runner, program, prompt) {
        return Err(RuntimeError::Unavailable(format!(
            "{program} is an npm .cmd shim, which cannot take a multi-line prompt as an argument on Windows."
        )));
    }
    args.push(format!("{option}={prompt}"));
    Ok(PromptDelivery {
        args,
        stdin: None,
        terminal: Some(TerminalRequest::new(false)),
    })
}

/// On Windows an npm `.cmd` shim runs through `cmd.exe`, which cannot take a multi-line
/// argument; such a run keeps using pipes (it has no live terminal).
fn terminal_fits(runner: &dyn ProcessRunner, program: &str, prompt: &str) -> bool {
    if !cfg!(windows) || !prompt.contains(['\n', '\r']) {
        return true;
    }
    !runner.locate(program).is_some_and(|path| {
        path.extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat"))
    })
}

/// What a failed run left for people to read. A pipe run has a separate stderr; in a terminal
/// the two streams are one, so the lines that are not the runtime's own JSON events are used.
pub fn diagnostics(output: &ProcessOutput) -> String {
    if !output.stderr.trim().is_empty() {
        return output.stderr.trim().to_owned();
    }
    output
        .stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('{'))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Executes a runtime's CLI call, translating process events into runtime progress and
/// process errors into normalized runtime errors. `on_line` receives each stdout line as it
/// is written, so a runtime can turn its tool's stream into live [`RuntimeEvent`]s. Every CLI runtime goes through here, so
/// process handling is not duplicated per runtime.
pub fn execute(
    runner: &dyn ProcessRunner,
    spec: &ProcessSpec,
    progress: &dyn Fn(RuntimeEvent),
    on_line: &dyn Fn(&str),
) -> Result<ProcessOutput, RuntimeError> {
    runner
        .run(spec, &|event| match event {
            ProcessEvent::Spawned => progress(RuntimeEvent::Sending),
            ProcessEvent::InputSent => progress(RuntimeEvent::Waiting),
            ProcessEvent::StdoutLine(line) => on_line(&line),
        })
        .map_err(|error| match error {
            ProcessError::PermissionDenied(reason) => RuntimeError::PermissionDenied(reason),
            ProcessError::NotFound => RuntimeError::NotInstalled,
            ProcessError::Timeout => RuntimeError::Timeout,
            ProcessError::Spawn(details) | ProcessError::Io(details) => {
                RuntimeError::ExecutionFailed(details)
            }
        })
}

/// Model ids become command-line arguments, so they must not be able to act as options or
/// carry whitespace.
pub fn validate_model_id(model_id: &str) -> Result<(), RuntimeError> {
    let ok = !model_id.is_empty()
        && !model_id.starts_with('-')
        && !model_id
            .chars()
            .any(|c| c.is_whitespace() || c.is_control());
    if ok {
        Ok(())
    } else {
        Err(RuntimeError::InvalidRequest(format!(
            "\"{model_id}\" is not a valid model identifier."
        )))
    }
}

/// Whether `code` appears as a number of its own (not inside a longer one, like a timestamp).
fn has_status_code(text: &str, code: &str) -> bool {
    text.split(|c: char| !c.is_ascii_digit()).any(|n| n == code)
}

/// Classifies failure text common to CLI tools.
pub fn classify_failure(details: &str, model_id: &str) -> RuntimeError {
    let lower = details.to_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| lower.contains(n));
    if has(&[
        "rate limit",
        "rate_limit",
        "ratelimit",
        "too many requests",
        "freeusagelimit",
    ]) || has_status_code(&lower, "429")
    {
        RuntimeError::RateLimited(details.chars().take(MAX_DETAILS_CHARS).collect())
    } else if has(&[
        "authenticate",
        "authentication",
        "unauthorized",
        "api key",
        "api_key",
        "credential",
        "oauth",
        "autherror",
        "log in",
        "login",
        "not logged",
        "401",
    ]) {
        RuntimeError::AuthenticationRequired
    } else if has(&[
        "model not found",
        "modelnotfound",
        "unknown model",
        "selected model",
        "may not exist",
    ]) {
        RuntimeError::ModelUnavailable(model_id.to_owned())
    } else {
        RuntimeError::ExecutionFailed(details.chars().take(MAX_DETAILS_CHARS).collect())
    }
}

/// Tools through which a CLI agent asks a person itself, by name. A call to one of these is a
/// structured signal that needs no reading of the agent's prose, in any language.
const ASKING_TOOLS: [&str; 5] = [
    "ExitPlanMode",
    "AskUserQuestion",
    "question",
    "plan_exit",
    "ask_question",
];

/// Metadata keys an adapter fills when the run called one of [`ASKING_TOOLS`].
pub(super) const ASKED_TOOL: &str = "askedTool";
pub(super) const ASKED_INPUT: &str = "askedInput";

/// Whether `tool` is one through which a CLI agent asks a person.
pub(super) fn is_asking_tool(tool: &str) -> bool {
    ASKING_TOOLS.contains(&tool)
}

/// Records, in the output's metadata, the first call to a tool that asks a person.
pub(super) fn note_asking_tool(
    metadata: &mut std::collections::BTreeMap<String, String>,
    tool: &str,
    input: &serde_json::Value,
) {
    if ASKING_TOOLS.contains(&tool) && !metadata.contains_key(ASKED_TOOL) {
        metadata.insert(ASKED_TOOL.to_owned(), tool.to_owned());
        metadata.insert(ASKED_INPUT.to_owned(), input.to_string());
    }
}

/// The detection for a run that called a tool that asks a person: the plan or the question the
/// call carried, falling back to the agent's own message.
pub(super) fn detection_from_asking_tool(
    output: &RuntimeOutput,
) -> Option<crate::domain::interaction::InteractionDetection> {
    use crate::application::interaction::clip;
    use crate::domain::interaction::{
        decision_options, DetectionSource, InteractionDetection, InteractionKind,
        InteractionOption, MAX_CONTEXT, MAX_DOCUMENT, MAX_OPTION, MAX_OPTIONS, MAX_QUESTION,
    };
    let tool = output.metadata.get(ASKED_TOOL)?;
    let input: serde_json::Value = output
        .metadata
        .get(ASKED_INPUT)
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or_default();
    let plan = matches!(tool.as_str(), "ExitPlanMode" | "plan_exit");
    let first = input["questions"].get(0).unwrap_or(&input);
    let question = first["question"]
        .as_str()
        .or_else(|| first["header"].as_str())
        .map_or_else(
            || {
                if plan {
                    "Approve this plan?".to_owned()
                } else {
                    output.text.lines().last().unwrap_or_default().to_owned()
                }
            },
            str::to_owned,
        );
    let document = input["plan"].as_str().unwrap_or(&output.text);
    let kind = if plan {
        InteractionKind::Approval
    } else {
        InteractionKind::Clarification
    };
    let options: Vec<InteractionOption> = if plan {
        decision_options(kind)
    } else {
        first["options"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|o| o["label"].as_str().or_else(|| o.as_str()))
            .take(MAX_OPTIONS)
            .map(|label| InteractionOption::new(&clip(label, MAX_OPTION), &clip(label, MAX_OPTION)))
            .collect()
    };
    Some(InteractionDetection {
        detected: true,
        kind: Some(kind),
        confidence: 95,
        question: clip(&question, MAX_QUESTION),
        context: clip(&output.text, MAX_CONTEXT),
        document: clip(document, MAX_DOCUMENT),
        options,
        source: DetectionSource::Adapter,
        evaluation: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rate_limit_is_told_apart_from_other_failures() {
        let opencode =
            "APIError: Error from provider (Console): Rate limit exceeded. Please try again later.";
        assert!(matches!(
            classify_failure(opencode, "m"),
            RuntimeError::RateLimited(_)
        ));
        assert!(matches!(
            classify_failure("HTTP 429 from the provider", "m"),
            RuntimeError::RateLimited(_)
        ));
        // A number that merely contains 429 is not a status code.
        assert!(matches!(
            classify_failure("failed at 1791142900 and 14290", "m"),
            RuntimeError::ExecutionFailed(_)
        ));
    }

    #[test]
    fn a_prompt_option_keeps_a_dashed_prompt_from_acting_as_an_option() {
        use crate::application::process::fake::{ok, FakeProcessRunner};
        let runner = FakeProcessRunner::new(&["agy"], |_| ok(""));

        let delivery =
            deliver_prompt_option(&runner, "agy", "--prompt", vec!["-x".to_owned()], "--help")
                .unwrap();

        assert_eq!(delivery.args, ["-x", "--prompt=--help"]);
        assert_eq!(delivery.stdin, None);
        assert_eq!(delivery.terminal, Some(TerminalRequest::new(false)));
        let huge = "x".repeat(MAX_PROMPT_ARGUMENT_BYTES + 1);
        assert!(matches!(
            deliver_prompt_option(&runner, "agy", "--prompt", Vec::new(), &huge),
            Err(RuntimeError::InvalidRequest(_))
        ));
    }

    #[test]
    fn rejects_model_ids_that_could_act_as_options() {
        assert!(validate_model_id("sonnet").is_ok());
        assert!(validate_model_id("opencode/big-pickle").is_ok());
        for bad in ["", "--help", "-x", "a b", "a\nb"] {
            assert!(
                matches!(validate_model_id(bad), Err(RuntimeError::InvalidRequest(_))),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn classifies_common_failures() {
        assert_eq!(
            classify_failure("Failed to authenticate: OAuth", "m"),
            RuntimeError::AuthenticationRequired
        );
        assert_eq!(
            classify_failure("There's an issue with the selected model (x)", "x"),
            RuntimeError::ModelUnavailable("x".to_owned())
        );
        assert!(
            matches!(classify_failure("boom", "m"), RuntimeError::ExecutionFailed(d) if d == "boom")
        );
    }
}
