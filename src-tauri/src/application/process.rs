use std::path::PathBuf;
use std::time::Duration;

/// Port: the only way the core starts an external program.
///
/// Callers (provider adapters) name a bare program and fixed arguments; nothing from the
/// webview ever reaches this. The implementation lives in `infrastructure/`.
pub trait ProcessRunner: Send + Sync {
    /// Where `program` is installed, if anywhere.
    fn locate(&self, program: &str) -> Option<PathBuf>;

    /// Runs `program` to completion. `on_event` is told when the process has started, when its
    /// input has been delivered, and about each line of stdout as soon as it is written
    /// (the complete output is still returned at the end).
    ///
    /// # Errors
    ///
    /// Fails if the program is missing, cannot start, times out, or I/O breaks.
    fn run(
        &self,
        spec: &ProcessSpec,
        on_event: &dyn Fn(ProcessEvent),
    ) -> Result<ProcessOutput, ProcessError>;
}

#[derive(Debug, Clone)]
pub struct ProcessSpec {
    pub program: String,
    pub args: Vec<String>,
    pub stdin: Option<String>,
    pub cwd: Option<PathBuf>,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessEvent {
    Spawned,
    InputSent,
    /// One line of stdout, without its line ending.
    StdoutLine(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessError {
    NotFound,
    Spawn(String),
    Timeout,
    Io(String),
}

#[cfg(test)]
pub mod fake {
    use std::collections::HashSet;
    use std::sync::Mutex;

    use super::{PathBuf, ProcessError, ProcessEvent, ProcessOutput, ProcessRunner, ProcessSpec};

    type Handler = Box<dyn Fn(&ProcessSpec) -> Result<ProcessOutput, ProcessError> + Send + Sync>;

    /// Scripted runner: `installed` programs are "found"; `handler` answers every run.
    pub struct FakeProcessRunner {
        installed: HashSet<String>,
        handler: Handler,
        pub calls: Mutex<Vec<ProcessSpec>>,
    }

    impl FakeProcessRunner {
        pub fn new(
            installed: &[&str],
            handler: impl Fn(&ProcessSpec) -> Result<ProcessOutput, ProcessError>
                + Send
                + Sync
                + 'static,
        ) -> Self {
            Self {
                installed: installed.iter().map(|s| (*s).to_owned()).collect(),
                handler: Box::new(handler),
                calls: Mutex::new(Vec::new()),
            }
        }
    }

    #[allow(clippy::unnecessary_wraps)]
    pub fn ok(stdout: &str) -> Result<ProcessOutput, ProcessError> {
        Ok(ProcessOutput {
            exit_code: Some(0),
            stdout: stdout.to_owned(),
            stderr: String::new(),
        })
    }

    impl ProcessRunner for FakeProcessRunner {
        fn locate(&self, program: &str) -> Option<PathBuf> {
            self.installed
                .contains(program)
                .then(|| PathBuf::from(program))
        }

        fn run(
            &self,
            spec: &ProcessSpec,
            on_event: &dyn Fn(ProcessEvent),
        ) -> Result<ProcessOutput, ProcessError> {
            if !self.installed.contains(&spec.program) {
                return Err(ProcessError::NotFound);
            }
            self.calls.lock().unwrap().push(spec.clone());
            let result = (self.handler)(spec);
            on_event(ProcessEvent::Spawned);
            on_event(ProcessEvent::InputSent);
            // Like the real runner: stdout lines are announced as they are "written".
            if let Ok(output) = &result {
                for line in output.stdout.lines() {
                    on_event(ProcessEvent::StdoutLine(line.to_owned()));
                }
            }
            result
        }
    }
}
