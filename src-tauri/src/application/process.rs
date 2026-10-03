use std::path::PathBuf;
use std::time::Duration;

use crate::domain::security::{Reason, ToolAccess};

/// Port: the only way the core starts an external program.
///
/// Callers (provider adapters) name a bare program and fixed arguments; nothing from the
/// webview ever reaches this. The system implementation lives in `infrastructure/`; what the
/// rest of the core is handed is always wrapped by `application::security::GuardedProcessRunner`,
/// which decides whether a [`ProcessSpec`] may run at all. A program and its arguments stay
/// separate all the way down: no implementation builds a command line or starts a shell.
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

/// Which execution a process belongs to. The ids are Atlas's own (never from the webview) and
/// are what the security layer uses to look up the workspace, agent and policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionScope {
    pub workspace_id: String,
    pub agent_id: String,
    pub execution_id: String,
    pub task_id: String,
    /// What the runtime's own tools can do (a restriction on the policy, never an expansion).
    pub runtime_access: ToolAccess,
    /// The execution runs in its own Git worktree. Only a flag: the security layer finds the
    /// worktree in what Atlas stored for this execution, so a path can never be smuggled in
    /// here, and holds the process to that folder instead of the project's.
    pub isolated: bool,
}

#[cfg(test)]
impl ExecutionScope {
    pub fn for_tests() -> Self {
        Self {
            workspace_id: "ws-1".to_owned(),
            agent_id: "agent-1".to_owned(),
            execution_id: "exec-1".to_owned(),
            task_id: "task-1".to_owned(),
            runtime_access: ToolAccess::NONE,
            isolated: false,
        }
    }
}

/// Why a process is being started. Every spec carries one; there is no default, so no caller
/// can reach the process port without saying what it is doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessContext {
    /// Atlas asking a runtime's own tool about itself (`--version`, auth status, model list).
    Probe,
    /// Atlas starting a runtime's CLI to perform an execution.
    Runtime(ExecutionScope),
    /// A command an agent asked Atlas to run on its behalf. No Atlas feature issues these yet
    /// (the runtimes run their own tools inside their process); the security layer is ready
    /// for the first one.
    #[allow(dead_code)]
    AgentRequested(ExecutionScope),
}

/// The structured request to start a process: program and arguments kept apart, never joined
/// into a command line. (This is the `ProcessRequest` of the security design.)
#[derive(Debug, Clone)]
pub struct ProcessSpec {
    pub program: String,
    pub args: Vec<String>,
    pub stdin: Option<String>,
    pub cwd: Option<PathBuf>,
    /// Extra environment variables for the child.
    pub env: Vec<(String, String)>,
    pub timeout: Duration,
    pub context: ProcessContext,
    /// `Some`: run attached to a real terminal (PTY) instead of pipes, and let the user watch
    /// and control the live process (see [`TerminalRequest`]). A terminal has no separate
    /// input channel for a prompt, so such a spec must have `stdin: None`: the runtime passes
    /// its prompt as an argument.
    pub terminal: Option<TerminalRequest>,
}

/// How a process that runs in a terminal is set up. Chosen by the runtime, never the webview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalRequest {
    pub size: TerminalSize,
    /// Manual input from the user may reach the process (the runtime's `terminal_input`).
    pub input: bool,
}

impl TerminalRequest {
    /// Size used until the UI reports its real one.
    pub const DEFAULT_SIZE: TerminalSize = TerminalSize {
        cols: 120,
        rows: 30,
    };

    pub fn new(input: bool) -> Self {
        Self {
            size: Self::DEFAULT_SIZE,
            input,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    pub cols: u16,
    pub rows: u16,
}

impl ProcessSpec {
    /// A harmless query to a runtime's tool: no input, no working directory.
    pub fn probe(program: &str, args: &[&str], timeout: Duration) -> Self {
        Self {
            program: program.to_owned(),
            args: args.iter().map(|a| (*a).to_owned()).collect(),
            stdin: None,
            cwd: None,
            env: Vec::new(),
            timeout,
            context: ProcessContext::Probe,
            terminal: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessEvent {
    Spawned,
    InputSent,
    /// One line of stdout, without its line ending. In a terminal the lines are the cleaned
    /// text of the merged output: control sequences and carriage returns removed.
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
    /// The security layer refused to start the process (it never started).
    PermissionDenied(Reason),
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
