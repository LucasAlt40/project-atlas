//! The `OpenSpec` command, as an extra and never a need (ADR 0035). Atlas writes, checks, publishes
//! and imports without it; when it is installed and a version Atlas knows, the person can ask it to
//! look at the published change or to archive it. Atlas runs it itself, through the guarded
//! process port (`ProcessContext::Tool`: a known tool, no shell, a project folder, one of four
//! subcommands), with its telemetry off, and never lets an agent near it.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use crate::application::errors::{AppError, ErrorCode};
use crate::application::process::{ProcessContext, ProcessRunner, ProcessSpec};
use crate::domain::spec::CliIssue;

const PROGRAM: &str = "openspec";
/// The oldest major version Atlas has read the contract of (the machine-readable output of
/// `validate`, `archive`'s result). Older ones are not tried.
const MIN_MAJOR: u32 = 1;
const VERSION_TIMEOUT: Duration = Duration::from_secs(20);
const VALIDATE_TIMEOUT: Duration = Duration::from_secs(60);
const ARCHIVE_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_ISSUES: usize = 100;
const MAX_TEXT: usize = 400;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliStatus {
    pub available: bool,
    pub version: Option<String>,
    /// Installed and a version whose output Atlas can read.
    pub compatible: bool,
    /// Why not, as a code: `not_installed`, `inside_project`, `version_unreadable`, `too_old`.
    pub reason: Option<String>,
}

impl CliStatus {
    fn unavailable(reason: &str) -> Self {
        Self {
            available: false,
            version: None,
            compatible: false,
            reason: Some(reason.to_owned()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliValidation {
    pub version: String,
    pub valid: bool,
    pub issues: Vec<CliIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliArchive {
    /// The name the change was archived under (`2026-10-13-add-dark-mode`), when the command said.
    pub archived_as: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    /// Not installed, in the project's own folder, or a version Atlas does not read.
    Unavailable(String),
    /// It ran and refused or failed; what it said.
    Failed(String),
    /// It ran and its answer was not the JSON that was expected.
    Unreadable,
}

impl From<CliError> for AppError {
    fn from(error: CliError) -> Self {
        match error {
            CliError::Unavailable(reason) => {
                AppError::new(ErrorCode::SpecCliUnavailable).with("reason", reason)
            }
            CliError::Failed(output) => AppError::new(ErrorCode::SpecCliFailed).with_detail(output),
            CliError::Unreadable => AppError::new(ErrorCode::SpecCliFailed)
                .with_detail("The command's answer could not be read."),
        }
    }
}

pub struct OpenSpecCli {
    runner: Arc<dyn ProcessRunner>,
}

fn clip(text: &str, max: usize) -> String {
    text.trim().chars().take(max).collect()
}

/// `1.3.1` from `1.3.1`, `openspec 1.3.1` or `v1.3.1\n`.
fn parse_version(text: &str) -> Option<(u32, String)> {
    text.split_whitespace().find_map(|word| {
        let word = word.trim_start_matches('v');
        let mut parts = word.split('.');
        let major: u32 = parts.next()?.parse().ok()?;
        let minor = parts.next()?;
        let patch = parts.next()?;
        let digits =
            |s: &str| !s.is_empty() && s.chars().take_while(char::is_ascii_digit).count() > 0;
        (digits(minor) && digits(patch)).then(|| (major, word.to_owned()))
    })
}

impl OpenSpecCli {
    pub fn new(runner: Arc<dyn ProcessRunner>) -> Self {
        Self { runner }
    }

    fn spec(root: &str, args: &[&str], timeout: Duration) -> ProcessSpec {
        ProcessSpec {
            program: PROGRAM.to_owned(),
            args: args.iter().map(|a| (*a).to_owned()).collect(),
            stdin: None,
            cwd: Some(root.into()),
            // The command's own opt-out; nothing about the project or the person is sent anywhere
            // by Atlas's use of it.
            env: vec![
                ("OPENSPEC_TELEMETRY".to_owned(), "0".to_owned()),
                ("DO_NOT_TRACK".to_owned(), "1".to_owned()),
            ],
            timeout,
            context: ProcessContext::Tool,
            terminal: None,
        }
    }

    /// Whether the command is there and a version Atlas reads. Starts nothing but `--version`.
    pub fn status(&self, root: &str) -> CliStatus {
        let Some(located) = self.runner.locate(PROGRAM) else {
            return CliStatus::unavailable("not_installed");
        };
        // A program found inside the project is the project's, not the person's: not run.
        if located.is_absolute() && located.starts_with(Path::new(root)) {
            return CliStatus::unavailable("inside_project");
        }
        let output = self
            .runner
            .run(&Self::spec(root, &["--version"], VERSION_TIMEOUT), &|_| {});
        let Some((major, version)) = output
            .ok()
            .filter(|o| o.exit_code == Some(0))
            .and_then(|o| parse_version(&o.stdout))
        else {
            return CliStatus {
                available: true,
                version: None,
                compatible: false,
                reason: Some("version_unreadable".to_owned()),
            };
        };
        let compatible = major >= MIN_MAJOR;
        CliStatus {
            available: true,
            version: Some(version),
            compatible,
            reason: (!compatible).then(|| "too_old".to_owned()),
        }
    }

    fn ready(&self, root: &str) -> Result<String, CliError> {
        let status = self.status(root);
        match (status.compatible, status.version) {
            (true, Some(version)) => Ok(version),
            _ => Err(CliError::Unavailable(
                status.reason.unwrap_or_else(|| "not_installed".to_owned()),
            )),
        }
    }

    /// Asks the command to validate a change of the project, read-only.
    ///
    /// # Errors
    ///
    /// When the command is not usable, or its answer is not the JSON of its `validate` contract.
    pub fn validate(&self, root: &str, change_id: &str) -> Result<CliValidation, CliError> {
        let version = self.ready(root)?;
        let output = self
            .runner
            .run(
                &Self::spec(
                    root,
                    &[
                        "validate",
                        change_id,
                        "--type",
                        "change",
                        "--json",
                        "--no-interactive",
                    ],
                    VALIDATE_TIMEOUT,
                ),
                &|_| {},
            )
            .map_err(|e| CliError::Failed(format!("{e:?}")))?;
        // Exit 1 with a JSON document is "it looked and found problems", not a failure to look.
        let value: Value = serde_json::from_str(output.stdout.trim()).map_err(|_| {
            if output.exit_code == Some(0) {
                CliError::Unreadable
            } else {
                CliError::Failed(clip(
                    &format!("{}{}", output.stdout, output.stderr),
                    MAX_TEXT,
                ))
            }
        })?;
        let item = value
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .ok_or(CliError::Unreadable)?;
        let valid = item
            .get("valid")
            .and_then(Value::as_bool)
            .ok_or(CliError::Unreadable)?;
        let issues = item
            .get("issues")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .take(MAX_ISSUES)
                    .map(|issue| CliIssue {
                        level: clip(issue.get("level").and_then(Value::as_str).unwrap_or(""), 20),
                        path: clip(issue.get("path").and_then(Value::as_str).unwrap_or(""), 200),
                        message: clip(
                            issue.get("message").and_then(Value::as_str).unwrap_or(""),
                            MAX_TEXT,
                        ),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(CliValidation {
            version,
            valid,
            issues,
        })
    }

    /// Archives a change: the command merges its deltas into `openspec/specs/` and moves the
    /// folder to `openspec/changes/archive/`. It validates first and refuses what is invalid.
    ///
    /// # Errors
    ///
    /// When the command is not usable or refuses (what it said is in the error).
    pub fn archive(&self, root: &str, change_id: &str) -> Result<CliArchive, CliError> {
        self.ready(root)?;
        let output = self
            .runner
            .run(
                &Self::spec(root, &["archive", change_id, "--yes"], ARCHIVE_TIMEOUT),
                &|_| {},
            )
            .map_err(|e| CliError::Failed(format!("{e:?}")))?;
        if output.exit_code != Some(0) {
            return Err(CliError::Failed(clip(
                &format!("{}\n{}", output.stdout, output.stderr),
                MAX_TEXT * 2,
            )));
        }
        // "Change 'x' archived as '2026-10-13-x'."
        let archived_as = output.stdout.lines().find_map(|line| {
            let rest = line.split("archived as '").nth(1)?;
            let name = rest.split('\'').next()?;
            (!name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))
            .then(|| name.to_owned())
        });
        Ok(CliArchive { archived_as })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::application::process::fake::{ok, FakeProcessRunner};
    use crate::application::process::{ProcessError, ProcessOutput};

    fn cli(
        installed: bool,
        handler: impl Fn(&ProcessSpec) -> Result<ProcessOutput, ProcessError> + Send + Sync + 'static,
    ) -> (OpenSpecCli, Arc<FakeProcessRunner>) {
        let runner = Arc::new(FakeProcessRunner::new(
            if installed { &["openspec"] } else { &[] },
            handler,
        ));
        (OpenSpecCli::new(runner.clone()), runner)
    }

    #[allow(clippy::unnecessary_wraps)]
    fn outcome(code: i32, stdout: &str, stderr: &str) -> Result<ProcessOutput, ProcessError> {
        Ok(ProcessOutput {
            exit_code: Some(code),
            stdout: stdout.to_owned(),
            stderr: stderr.to_owned(),
        })
    }

    const VALID: &str = r#"{"items":[{"id":"dark-mode","type":"change","valid":true,"issues":[],"durationMs":1}],"summary":{},"version":"1.0"}"#;
    const INVALID: &str = r#"{"items":[{"id":"broken","type":"change","valid":false,"issues":[{"level":"ERROR","path":"a/spec.md","message":"ADDED \"Weak\" must contain SHALL or MUST"}]}]}"#;

    #[test]
    fn without_the_command_everything_says_so_and_nothing_is_started() {
        let (cli, runner) = cli(false, |_| ok(""));

        let status = cli.status("/project");

        assert!(!status.available && !status.compatible);
        assert_eq!(status.reason.as_deref(), Some("not_installed"));
        assert_eq!(
            cli.validate("/project", "x"),
            Err(CliError::Unavailable("not_installed".to_owned()))
        );
        assert_eq!(
            cli.archive("/project", "x"),
            Err(CliError::Unavailable("not_installed".to_owned()))
        );
        assert!(runner.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn a_version_it_reads_is_compatible_and_one_it_does_not_is_not_tried() {
        let (new, _) = cli(true, |_| ok("1.3.1\n"));
        let (ancient, runner) = cli(true, |_| ok("0.9.0\n"));
        let (garbled, _) = cli(true, |_| ok("who knows"));

        assert_eq!(new.status("/p").version.as_deref(), Some("1.3.1"));
        assert!(new.status("/p").compatible);
        let old = ancient.status("/p");
        assert!(old.available && !old.compatible);
        assert_eq!(old.reason.as_deref(), Some("too_old"));
        assert_eq!(
            ancient.validate("/p", "x"),
            Err(CliError::Unavailable("too_old".to_owned()))
        );
        assert_eq!(
            runner.calls.lock().unwrap().len(),
            2,
            "only --version, twice"
        );
        assert_eq!(
            garbled.status("/p").reason.as_deref(),
            Some("version_unreadable")
        );
    }

    #[test]
    fn every_call_is_a_tool_run_in_the_project_with_telemetry_off() {
        let (cli, runner) = cli(true, |spec| match spec.args[0].as_str() {
            "--version" => ok("1.3.1"),
            _ => ok(VALID),
        });

        cli.validate("/project", "dark-mode").unwrap();

        for call in runner.calls.lock().unwrap().iter() {
            assert_eq!(call.program, "openspec");
            assert_eq!(call.context, ProcessContext::Tool);
            assert_eq!(call.cwd.as_deref(), Some(Path::new("/project")));
            assert!(call.stdin.is_none() && call.terminal.is_none());
            assert!(call
                .env
                .contains(&("OPENSPEC_TELEMETRY".to_owned(), "0".to_owned())));
            assert!(call
                .env
                .contains(&("DO_NOT_TRACK".to_owned(), "1".to_owned())));
        }
        let validate = runner.calls.lock().unwrap().last().cloned().unwrap();
        assert_eq!(
            validate.args,
            [
                "validate",
                "dark-mode",
                "--type",
                "change",
                "--json",
                "--no-interactive"
            ]
        );
    }

    #[test]
    fn a_valid_change_and_an_invalid_one_are_both_answers_not_failures() {
        let (good, _) = cli(true, |spec| match spec.args[0].as_str() {
            "--version" => ok("1.3.1"),
            _ => outcome(0, VALID, ""),
        });
        let (bad, _) = cli(true, |spec| match spec.args[0].as_str() {
            "--version" => ok("1.3.1"),
            _ => outcome(1, INVALID, ""),
        });

        let valid = good.validate("/p", "dark-mode").unwrap();
        let invalid = bad.validate("/p", "broken").unwrap();

        assert!(valid.valid && valid.issues.is_empty());
        assert_eq!(valid.version, "1.3.1");
        assert!(!invalid.valid);
        assert_eq!(invalid.issues[0].level, "ERROR");
        assert!(invalid.issues[0].message.contains("SHALL or MUST"));
    }

    #[test]
    fn an_answer_that_is_not_the_contract_is_unreadable_and_a_crash_is_a_failure() {
        let (junk, _) = cli(true, |spec| match spec.args[0].as_str() {
            "--version" => ok("1.3.1"),
            _ => outcome(0, "not json", ""),
        });
        let (crash, _) = cli(true, |spec| match spec.args[0].as_str() {
            "--version" => ok("1.3.1"),
            _ => outcome(2, "", "TypeError: boom"),
        });

        assert_eq!(junk.validate("/p", "x"), Err(CliError::Unreadable));
        assert!(matches!(
            crash.validate("/p", "x"),
            Err(CliError::Failed(_))
        ));
    }

    #[test]
    fn archiving_reports_where_the_command_put_the_change() {
        let (cli, runner) = cli(true, |spec| {
            match spec.args[0].as_str() {
            "--version" => ok("1.3.1"),
            _ => ok("Specs updated successfully.\nChange 'dark-mode' archived as '2026-10-13-dark-mode'.\n"),
        }
        });

        let archived = cli.archive("/p", "dark-mode").unwrap();

        assert_eq!(
            archived.archived_as.as_deref(),
            Some("2026-10-13-dark-mode")
        );
        let last = runner.calls.lock().unwrap().last().cloned().unwrap();
        assert_eq!(last.args, ["archive", "dark-mode", "--yes"]);
    }

    #[test]
    fn a_refused_archive_says_what_the_command_said() {
        let (cli, _) = cli(true, |spec| match spec.args[0].as_str() {
            "--version" => ok("1.3.1"),
            _ => outcome(1, "", "Validation failed: ADDED requirement exists"),
        });

        let error = cli.archive("/p", "x").unwrap_err();

        assert!(matches!(&error, CliError::Failed(text) if text.contains("Validation failed")));
        let app: AppError = error.into();
        assert!(app.is(ErrorCode::SpecCliFailed));
    }

    #[test]
    fn a_name_in_the_answer_that_is_not_a_plain_folder_name_is_not_believed() {
        let (cli, _) = cli(true, |spec| match spec.args[0].as_str() {
            "--version" => ok("1.3.1"),
            _ => ok("Change 'x' archived as '../../etc'."),
        });

        assert_eq!(cli.archive("/p", "x").unwrap().archived_as, None);
    }

    #[test]
    fn versions_are_read_from_the_words_commands_print() {
        assert_eq!(parse_version("1.3.1").map(|v| v.0), Some(1));
        assert_eq!(parse_version("openspec v2.0.0-beta").map(|v| v.0), Some(2));
        assert_eq!(parse_version("nope"), None);
    }
}
