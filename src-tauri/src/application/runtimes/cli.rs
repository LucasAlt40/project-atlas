//! Helpers shared by runtimes that are command-line tools.

use std::time::Duration;

use super::{RuntimeError, RuntimeEvent};
use crate::application::process::{
    ProcessError, ProcessEvent, ProcessOutput, ProcessRunner, ProcessSpec,
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
    let spec = ProcessSpec {
        program: program.to_owned(),
        args: args.iter().map(|a| (*a).to_owned()).collect(),
        stdin: None,
        cwd: None,
        timeout: timeout.unwrap_or(PROBE_TIMEOUT),
    };
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

/// Classifies failure text common to CLI tools.
pub fn classify_failure(details: &str, model_id: &str) -> RuntimeError {
    let lower = details.to_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| lower.contains(n));
    if has(&[
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

#[cfg(test)]
mod tests {
    use super::*;

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
