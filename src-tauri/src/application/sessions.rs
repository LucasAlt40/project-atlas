//! Live process sessions: the identity and control of the process behind an execution.
//!
//! An execution that runs in a terminal has, while its process lives, a *process session*
//! (`processSessionId`). The system process runner opens one when it starts the process and
//! closes it when the process is gone; in between this registry
//!
//! - keeps the (bounded, ephemeral) output so a terminal can be redrawn after a reload or a
//!   workspace switch,
//! - tells the UI about output and state changes through a [`SessionSink`],
//! - and is the only way to control the process: every command names the execution, workspace
//!   and agent, and a session answers only when all three match the ones it was opened for.
//!   There is no "current process" and no way to address a process by anything the OS knows
//!   (PID, process group, signal): those stay behind [`ProcessSession`].
//!
//! The registry never starts a process, so controlling one cannot widen what the execution may
//! do: the process was authorized by the guard before it existed (ADR 0007).

use crate::application::support::LockExt;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};

use super::errors::{AppError, ErrorCode};
use super::process::{ExecutionScope, TerminalRequest, TerminalSize};
use super::support::{new_id, now_ms};
use crate::domain::execution::{ExecutionEvent, ExecutionEventKind};
use crate::domain::terminal::{
    SessionStatus, SessionStatusEvent, TerminalChunk, TerminalSnapshot, TerminalStream, UserAction,
};

/// Output kept per session. Older output is dropped: the terminal is a live view, not a log.
pub const OUTPUT_LIMIT_BYTES: usize = 512 * 1024;
/// Finished sessions whose output stays available to re-open a terminal after the run.
const RETAINED_FINISHED: usize = 8;
/// One manual input is at most this large (a paste, not a file).
pub const MAX_INPUT_BYTES: usize = 64 * 1024;
const MAX_TERMINAL_DIMENSION: u16 = 1000;
const CTRL_C: u8 = 0x03;

/// What the operating system side of a session can fail with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    /// The process is already gone.
    Exited,
    Io(String),
}

/// Port: the live OS process behind a session. Implemented in `infrastructure/` (a PTY today).
/// Methods are best-effort requests to the process; the registry owns the *meaning* of the
/// state (who asked, what the execution becomes).
pub trait ProcessSession: Send + Sync {
    /// Sends bytes to the process as if typed in its terminal.
    fn write(&self, data: &[u8]) -> Result<(), SessionError>;
    /// Ctrl+C: the interrupt a terminal gives the process, reaching its foreground process
    /// group. The process may handle it, and may ignore it.
    fn interrupt(&self) -> Result<(), SessionError>;
    /// Ends the process and its process group: politely first, then by force.
    fn terminate(&self) -> Result<(), SessionError>;
    fn resize(&self, size: TerminalSize) -> Result<(), SessionError>;
}

/// Port: told about everything a session does, the moment it happens. The Tauri adapter in
/// `commands/` turns these into webview events; tests collect them.
pub trait SessionSink: Send + Sync {
    fn on_output(&self, chunk: &TerminalChunk);
    fn on_status(&self, event: &SessionStatusEvent);
    /// A step for the execution's activity (terminal connected, user interrupted, exited…).
    fn on_activity(&self, event: &ExecutionEvent);
}

/// Why a control request was not carried out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlError {
    /// No such session for this execution *in this workspace and agent*. The same answer for
    /// a session that never existed and one that belongs to someone else.
    NotFound,
    NotRunning,
    InputNotSupported,
    InvalidInput,
    Failed(String),
}

impl From<&ControlError> for AppError {
    fn from(error: &ControlError) -> Self {
        match error {
            ControlError::NotFound => AppError::new(ErrorCode::ExecutionNotFound),
            ControlError::NotRunning => AppError::new(ErrorCode::ExecutionNotRunning),
            ControlError::InputNotSupported => AppError::new(ErrorCode::TerminalInputUnsupported),
            ControlError::InvalidInput => AppError::new(ErrorCode::TerminalInputInvalid),
            ControlError::Failed(detail) => {
                AppError::new(ErrorCode::ProcessControlFailed).with_detail(detail.clone())
            }
        }
    }
}

impl From<SessionError> for ControlError {
    fn from(error: SessionError) -> Self {
        match error {
            SessionError::Exited => Self::NotRunning,
            SessionError::Io(detail) => Self::Failed(detail),
        }
    }
}

/// Which session a request is about. All three ids must match the session's own: the webview
/// is trusted with nothing but naming it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_field_names)] // the three ids are what they are called everywhere
pub struct SessionTarget {
    pub execution_id: String,
    pub workspace_id: String,
    pub agent_id: String,
}

/// What the runner hands over when its process starts.
pub struct OpenSession {
    /// The ids of the execution, from the guard-approved request (not from the webview).
    pub scope: ExecutionScope,
    /// The command as one line of text, for people to read.
    pub command: String,
    pub terminal: TerminalRequest,
    pub session: Arc<dyn ProcessSession>,
}

struct Entry {
    id: String,
    scope: ExecutionScope,
    command: String,
    started_at: u64,
    ended_at: Option<u64>,
    exit_code: Option<i32>,
    status: SessionStatus,
    user_action: Option<UserAction>,
    size: TerminalSize,
    input: bool,
    buffer: String,
    truncated: bool,
    next_seq: u64,
    session: Arc<dyn ProcessSession>,
}

impl Entry {
    fn matches(&self, target: &SessionTarget) -> bool {
        self.scope.execution_id == target.execution_id
            && self.scope.workspace_id == target.workspace_id
            && self.scope.agent_id == target.agent_id
    }

    fn status_event(&self) -> SessionStatusEvent {
        SessionStatusEvent {
            execution_id: self.scope.execution_id.clone(),
            process_session_id: self.id.clone(),
            workspace_id: self.scope.workspace_id.clone(),
            agent_id: self.scope.agent_id.clone(),
            status: self.status,
            user_action: self.user_action,
            exit_code: self.exit_code,
            timestamp: now_ms(),
        }
    }

    fn activity(
        &self,
        kind: ExecutionEventKind,
        message: &str,
        metadata: impl IntoIterator<Item = (&'static str, String)>,
    ) -> ExecutionEvent {
        ExecutionEvent {
            execution_id: self.scope.execution_id.clone(),
            workspace_id: self.scope.workspace_id.clone(),
            task_id: self.scope.task_id.clone(),
            agent_id: self.scope.agent_id.clone(),
            kind,
            message: message.to_owned(),
            timestamp: now_ms(),
            metadata: metadata
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value))
                .collect(),
        }
    }

    fn snapshot(&self) -> TerminalSnapshot {
        TerminalSnapshot {
            execution_id: self.scope.execution_id.clone(),
            process_session_id: self.id.clone(),
            workspace_id: self.scope.workspace_id.clone(),
            agent_id: self.scope.agent_id.clone(),
            status: self.status,
            user_action: self.user_action,
            command: self.command.clone(),
            started_at: self.started_at,
            ended_at: self.ended_at,
            exit_code: self.exit_code,
            cols: self.size.cols,
            rows: self.size.rows,
            input_enabled: self.input,
            output: self.buffer.clone(),
            next_seq: self.next_seq,
            truncated: self.truncated,
        }
    }

    fn append(&mut self, data: &str) {
        self.buffer.push_str(data);
        // Trim in steps of a quarter of the limit so a full buffer is not shifted per chunk.
        if self.buffer.len() > OUTPUT_LIMIT_BYTES + OUTPUT_LIMIT_BYTES / 4 {
            let mut cut = self.buffer.len() - OUTPUT_LIMIT_BYTES;
            while !self.buffer.is_char_boundary(cut) {
                cut += 1;
            }
            self.buffer.drain(..cut);
            self.truncated = true;
        }
    }
}

#[derive(Default)]
struct State {
    /// One session per execution: live ones and the most recent finished ones.
    entries: HashMap<String, Entry>,
    finished: VecDeque<String>,
}

/// Every live process session, keyed by execution. See the module documentation.
#[derive(Default)]
pub struct SessionRegistry {
    state: Mutex<State>,
    sink: Option<Arc<dyn SessionSink>>,
}

impl SessionRegistry {
    pub fn with_sink(sink: Arc<dyn SessionSink>) -> Self {
        Self {
            state: Mutex::default(),
            sink: Some(sink),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock_or_recover()
    }

    /// Registers the session of a process that has just started and announces it.
    pub fn open(self: &Arc<Self>, open: OpenSession) -> SessionHandle {
        let entry = Entry {
            id: new_id("ps"),
            scope: open.scope,
            command: open.command,
            started_at: now_ms(),
            ended_at: None,
            exit_code: None,
            status: SessionStatus::Running,
            user_action: None,
            size: open.terminal.size,
            input: open.terminal.input,
            buffer: String::new(),
            truncated: false,
            next_seq: 0,
            session: open.session,
        };
        let handle = SessionHandle {
            registry: self.clone(),
            execution_id: entry.scope.execution_id.clone(),
            session_id: entry.id.clone(),
            finished: false,
        };
        let status = entry.status_event();
        let connected = entry.activity(
            ExecutionEventKind::TerminalConnected,
            "Terminal connected",
            [("processSessionId", entry.id.clone())],
        );
        {
            let mut state = self.lock();
            let execution_id = entry.scope.execution_id.clone();
            state.finished.retain(|id| *id != execution_id);
            state.entries.insert(execution_id, entry);
        }
        if let Some(sink) = &self.sink {
            sink.on_status(&status);
            sink.on_activity(&connected);
        }
        handle
    }

    /// The terminal of an execution (live, or finished and still retained).
    ///
    /// # Errors
    ///
    /// [`ControlError::NotFound`] if there is none for these three ids.
    pub fn snapshot(&self, target: &SessionTarget) -> Result<TerminalSnapshot, ControlError> {
        let state = self.lock();
        state
            .entries
            .get(&target.execution_id)
            .filter(|entry| entry.matches(target))
            .map(Entry::snapshot)
            .ok_or(ControlError::NotFound)
    }

    /// What the user did to the execution's process, if anything. Asked by the execution
    /// service to tell a cancellation from a failure.
    pub fn user_action(&self, execution_id: &str) -> Option<UserAction> {
        self.lock()
            .entries
            .get(execution_id)
            .and_then(|entry| entry.user_action)
    }

    /// Executions whose process is still alive.
    pub fn live_executions(&self) -> Vec<String> {
        self.lock()
            .entries
            .values()
            .filter(|entry| entry.status != SessionStatus::Exited)
            .map(|entry| entry.scope.execution_id.clone())
            .collect()
    }

    /// Ctrl+C for the execution's process. The first thing to try: the process may stop
    /// gracefully, and its execution then ends as cancelled.
    ///
    /// # Errors
    ///
    /// Fails if the session is not found, already ended, or the OS refuses.
    pub fn interrupt(&self, target: &SessionTarget) -> Result<(), ControlError> {
        let (session, status, activity) = {
            let mut state = self.lock();
            let entry = Self::live_entry(&mut state, target)?;
            if entry.status == SessionStatus::Terminating {
                // Already being ended by force; a polite request adds nothing.
                return Ok(());
            }
            entry.status = SessionStatus::Interrupting;
            entry.user_action.get_or_insert(UserAction::Interrupted);
            (
                entry.session.clone(),
                entry.status_event(),
                entry.activity(
                    ExecutionEventKind::UserInterrupted,
                    "User interrupted the execution",
                    [("signal", "interrupt".to_owned())],
                ),
            )
        };
        self.announce(&status, Some(&activity));
        session.interrupt().map_err(|error| {
            self.revert(target, SessionStatus::Interrupting);
            ControlError::from(error)
        })
    }

    /// Ends the process by force (after a grace period, when the process ignores the
    /// interrupt). Its execution ends as cancelled.
    ///
    /// # Errors
    ///
    /// Fails if the session is not found, already ended, or the OS refuses.
    pub fn terminate(&self, target: &SessionTarget) -> Result<(), ControlError> {
        let (session, status, activity) = {
            let mut state = self.lock();
            let entry = Self::live_entry(&mut state, target)?;
            entry.status = SessionStatus::Terminating;
            entry.user_action = Some(UserAction::Terminated);
            (
                entry.session.clone(),
                entry.status_event(),
                entry.activity(
                    ExecutionEventKind::UserTerminated,
                    "User terminated the process",
                    [],
                ),
            )
        };
        self.announce(&status, Some(&activity));
        session.terminate().map_err(|error| {
            self.revert(target, SessionStatus::Terminating);
            ControlError::from(error)
        })
    }

    /// Manual input for the process. Allowed only when the runtime reads input; the bytes are
    /// passed on untouched (Atlas does not interpret them as commands). Ctrl+C is the one
    /// exception: it is an interrupt, so it takes the interrupt path and its bookkeeping.
    ///
    /// # Errors
    ///
    /// Fails if the session is not found or not running, the runtime takes no input, the
    /// input is too large or mixes Ctrl+C with other bytes, or the OS refuses.
    pub fn input(&self, target: &SessionTarget, data: &[u8]) -> Result<(), ControlError> {
        if data == [CTRL_C] {
            return self.interrupt(target);
        }
        if data.is_empty() || data.len() > MAX_INPUT_BYTES || data.contains(&CTRL_C) {
            return Err(ControlError::InvalidInput);
        }
        let session = {
            let mut state = self.lock();
            let entry = Self::live_entry(&mut state, target)?;
            if !entry.input {
                return Err(ControlError::InputNotSupported);
            }
            if entry.status != SessionStatus::Running {
                return Err(ControlError::NotRunning);
            }
            entry.session.clone()
        };
        session.write(data).map_err(ControlError::from)
    }

    /// Tells the process its terminal changed size.
    ///
    /// # Errors
    ///
    /// Fails if the session is not found or ended, the size is not sensible, or the OS refuses.
    pub fn resize(&self, target: &SessionTarget, cols: u16, rows: u16) -> Result<(), ControlError> {
        let valid = 1..=MAX_TERMINAL_DIMENSION;
        if !valid.contains(&cols) || !valid.contains(&rows) {
            return Err(ControlError::InvalidInput);
        }
        let size = TerminalSize { cols, rows };
        let session = {
            let mut state = self.lock();
            let entry = Self::live_entry(&mut state, target)?;
            entry.size = size;
            entry.session.clone()
        };
        session.resize(size).map_err(ControlError::from)
    }

    /// Ends every live process (the application is closing). Not a user action on one
    /// execution, so it records none.
    pub fn terminate_all(&self) {
        let sessions: Vec<_> = self
            .lock()
            .entries
            .values()
            .filter(|entry| entry.status != SessionStatus::Exited)
            .map(|entry| entry.session.clone())
            .collect();
        for session in sessions {
            let _ = session.terminate();
        }
    }

    /// [`Self::terminate_all`], then waits (up to `timeout`) for the processes to be gone, so
    /// that the application does not exit while they are still being ended.
    pub fn terminate_all_and_wait(&self, timeout: std::time::Duration) {
        self.terminate_all();
        let deadline = std::time::Instant::now() + timeout;
        while !self.live_executions().is_empty() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
    }

    fn live_entry<'a>(
        state: &'a mut State,
        target: &SessionTarget,
    ) -> Result<&'a mut Entry, ControlError> {
        let entry = state
            .entries
            .get_mut(&target.execution_id)
            .filter(|entry| entry.matches(target))
            .ok_or(ControlError::NotFound)?;
        if entry.status == SessionStatus::Exited {
            return Err(ControlError::NotRunning);
        }
        Ok(entry)
    }

    /// The OS refused a request: the process is as it was, so is the state.
    fn revert(&self, target: &SessionTarget, from: SessionStatus) {
        let event = {
            let mut state = self.lock();
            let Some(entry) = state
                .entries
                .get_mut(&target.execution_id)
                .filter(|entry| entry.matches(target) && entry.status == from)
            else {
                return;
            };
            entry.status = SessionStatus::Running;
            entry.user_action = None;
            entry.status_event()
        };
        self.announce(&event, None);
    }

    fn announce(&self, status: &SessionStatusEvent, activity: Option<&ExecutionEvent>) {
        if let Some(sink) = &self.sink {
            sink.on_status(status);
            if let Some(activity) = activity {
                sink.on_activity(activity);
            }
        }
    }
}

/// Held by the runner for as long as its process runs. Feeds the session with the process's
/// output and closes it with the exit code; dropped without [`SessionHandle::finish`] it closes
/// as "exited, code unknown", so a session can never stay "running" after its runner is gone.
pub struct SessionHandle {
    registry: Arc<SessionRegistry>,
    execution_id: String,
    session_id: String,
    finished: bool,
}

impl SessionHandle {
    /// A piece of the process's raw terminal output.
    pub fn output(&self, data: &str) {
        if data.is_empty() {
            return;
        }
        let chunk = {
            let mut state = self.registry.lock();
            let Some(entry) = state
                .entries
                .get_mut(&self.execution_id)
                .filter(|entry| entry.id == self.session_id)
            else {
                return;
            };
            let seq = entry.next_seq;
            entry.next_seq += 1;
            entry.append(data);
            TerminalChunk {
                execution_id: entry.scope.execution_id.clone(),
                process_session_id: entry.id.clone(),
                workspace_id: entry.scope.workspace_id.clone(),
                agent_id: entry.scope.agent_id.clone(),
                timestamp: now_ms(),
                stream: TerminalStream::Stdout,
                seq,
                data: data.to_owned(),
            }
        };
        if let Some(sink) = &self.registry.sink {
            sink.on_output(&chunk);
        }
    }

    /// The process has ended.
    pub fn finish(mut self, exit_code: Option<i32>) {
        self.close(exit_code);
    }

    fn close(&mut self, exit_code: Option<i32>) {
        if std::mem::replace(&mut self.finished, true) {
            return;
        }
        let (status, activity) = {
            let mut state = self.registry.lock();
            let Some(entry) = state
                .entries
                .get_mut(&self.execution_id)
                .filter(|entry| entry.id == self.session_id)
            else {
                return;
            };
            let ended = now_ms();
            entry.status = SessionStatus::Exited;
            entry.ended_at = Some(ended);
            entry.exit_code = exit_code;
            let mut metadata = vec![(
                "durationMs",
                ended.saturating_sub(entry.started_at).to_string(),
            )];
            if let Some(code) = exit_code {
                metadata.push(("exitCode", code.to_string()));
            }
            let event = (
                entry.status_event(),
                entry.activity(
                    ExecutionEventKind::ProcessExited,
                    "Process exited",
                    metadata,
                ),
            );
            state.finished.push_back(self.execution_id.clone());
            while state.finished.len() > RETAINED_FINISHED {
                if let Some(oldest) = state.finished.pop_front() {
                    state.entries.remove(&oldest);
                }
            }
            event
        };
        self.registry.announce(&status, Some(&activity));
    }
}

impl Drop for SessionHandle {
    fn drop(&mut self) {
        self.close(None);
    }
}

#[cfg(test)]
pub mod fake {
    use std::sync::Mutex;

    use super::{ProcessSession, SessionError, TerminalSize};

    /// Records what was asked of the process; asks can be made to fail.
    #[derive(Default)]
    pub struct FakeSession {
        pub calls: Mutex<Vec<String>>,
        pub fail: Mutex<Option<SessionError>>,
    }

    impl FakeSession {
        fn record(&self, call: String) -> Result<(), SessionError> {
            self.calls.lock().unwrap().push(call);
            self.fail.lock().unwrap().clone().map_or(Ok(()), Err)
        }

        pub fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl ProcessSession for FakeSession {
        fn write(&self, data: &[u8]) -> Result<(), SessionError> {
            self.record(format!("write:{}", String::from_utf8_lossy(data)))
        }
        fn interrupt(&self) -> Result<(), SessionError> {
            self.record("interrupt".to_owned())
        }
        fn terminate(&self) -> Result<(), SessionError> {
            self.record("terminate".to_owned())
        }
        fn resize(&self, size: TerminalSize) -> Result<(), SessionError> {
            self.record(format!("resize:{}x{}", size.cols, size.rows))
        }
    }
}

#[cfg(test)]
#[allow(clippy::assert_is_empty)]
mod tests {
    use super::fake::FakeSession;
    use super::*;

    #[derive(Default)]
    struct Collector {
        output: Mutex<Vec<TerminalChunk>>,
        status: Mutex<Vec<SessionStatusEvent>>,
        activity: Mutex<Vec<ExecutionEvent>>,
    }

    impl SessionSink for Collector {
        fn on_output(&self, chunk: &TerminalChunk) {
            self.output.lock().unwrap().push(chunk.clone());
        }
        fn on_status(&self, event: &SessionStatusEvent) {
            self.status.lock().unwrap().push(event.clone());
        }
        fn on_activity(&self, event: &ExecutionEvent) {
            self.activity.lock().unwrap().push(event.clone());
        }
    }

    struct World {
        registry: Arc<SessionRegistry>,
        sink: Arc<Collector>,
    }

    fn world() -> World {
        let sink = Arc::new(Collector::default());
        World {
            registry: Arc::new(SessionRegistry::with_sink(sink.clone())),
            sink,
        }
    }

    fn scope(workspace: &str, agent: &str, execution: &str) -> ExecutionScope {
        ExecutionScope {
            workspace_id: workspace.to_owned(),
            agent_id: agent.to_owned(),
            execution_id: execution.to_owned(),
            ..ExecutionScope::for_tests()
        }
    }

    fn target(workspace: &str, agent: &str, execution: &str) -> SessionTarget {
        SessionTarget {
            execution_id: execution.to_owned(),
            workspace_id: workspace.to_owned(),
            agent_id: agent.to_owned(),
        }
    }

    fn open(
        world: &World,
        scope: ExecutionScope,
        input: bool,
    ) -> (SessionHandle, Arc<FakeSession>) {
        let session = Arc::new(FakeSession::default());
        let handle = world.registry.open(OpenSession {
            scope,
            command: "claude -p".to_owned(),
            terminal: TerminalRequest::new(input),
            session: session.clone(),
        });
        (handle, session)
    }

    #[test]
    fn a_session_is_announced_and_streams_ordered_output() {
        let w = world();
        let (handle, _) = open(&w, scope("w", "a", "e1"), false);

        handle.output("one ");
        handle.output("two");

        let chunks = w.sink.output.lock().unwrap().clone();
        assert_eq!(
            chunks
                .iter()
                .map(|c| (c.seq, c.data.as_str()))
                .collect::<Vec<_>>(),
            [(0, "one "), (1, "two")]
        );
        let snapshot = w.registry.snapshot(&target("w", "a", "e1")).unwrap();
        assert_eq!(snapshot.output, "one two");
        assert_eq!(snapshot.next_seq, 2);
        assert_eq!(snapshot.status, SessionStatus::Running);
        assert!(snapshot.process_session_id.starts_with("ps-"));
        assert_eq!(
            w.sink.activity.lock().unwrap()[0].kind,
            ExecutionEventKind::TerminalConnected
        );
    }

    #[test]
    fn interrupt_reaches_the_process_and_records_what_the_user_did() {
        let w = world();
        let (_handle, session) = open(&w, scope("w", "a", "e1"), false);
        let t = target("w", "a", "e1");

        w.registry.interrupt(&t).unwrap();

        assert_eq!(session.calls(), ["interrupt"]);
        assert_eq!(w.registry.user_action("e1"), Some(UserAction::Interrupted));
        let snapshot = w.registry.snapshot(&t).unwrap();
        assert_eq!(snapshot.status, SessionStatus::Interrupting);
        let kinds: Vec<_> = w
            .sink
            .activity
            .lock()
            .unwrap()
            .iter()
            .map(|e| e.kind)
            .collect();
        assert_eq!(
            kinds,
            [
                ExecutionEventKind::TerminalConnected,
                ExecutionEventKind::UserInterrupted
            ]
        );
    }

    #[test]
    fn a_second_interrupt_is_harmless_and_terminate_still_follows() {
        let w = world();
        let (_handle, session) = open(&w, scope("w", "a", "e1"), false);
        let t = target("w", "a", "e1");

        w.registry.interrupt(&t).unwrap();
        w.registry.interrupt(&t).unwrap();
        w.registry.terminate(&t).unwrap();
        // Polite requests after a forced end change nothing.
        w.registry.interrupt(&t).unwrap();

        assert_eq!(session.calls(), ["interrupt", "interrupt", "terminate"]);
        assert_eq!(w.registry.user_action("e1"), Some(UserAction::Terminated));
        assert_eq!(
            w.registry.snapshot(&t).unwrap().status,
            SessionStatus::Terminating
        );
    }

    #[test]
    fn a_refused_request_leaves_the_state_as_it_was() {
        let w = world();
        let (_handle, session) = open(&w, scope("w", "a", "e1"), false);
        *session.fail.lock().unwrap() = Some(SessionError::Io("boom".to_owned()));
        let t = target("w", "a", "e1");

        assert_eq!(
            w.registry.interrupt(&t),
            Err(ControlError::Failed("boom".to_owned()))
        );

        assert_eq!(w.registry.user_action("e1"), None);
        assert_eq!(
            w.registry.snapshot(&t).unwrap().status,
            SessionStatus::Running
        );
    }

    #[test]
    fn a_session_answers_only_to_its_own_workspace_agent_and_execution() {
        let w = world();
        let (_a, session_a) = open(&w, scope("w1", "a", "e1"), true);

        for wrong in [
            target("w2", "a", "e1"),
            target("w1", "b", "e1"),
            target("w1", "a", "e2"),
        ] {
            assert_eq!(w.registry.interrupt(&wrong), Err(ControlError::NotFound));
            assert_eq!(w.registry.terminate(&wrong), Err(ControlError::NotFound));
            assert_eq!(w.registry.input(&wrong, b"x"), Err(ControlError::NotFound));
            assert_eq!(
                w.registry.resize(&wrong, 80, 24),
                Err(ControlError::NotFound)
            );
            assert_eq!(w.registry.snapshot(&wrong), Err(ControlError::NotFound));
        }
        assert!(session_a.calls().is_empty());
        assert_eq!(w.registry.user_action("e1"), None);
    }

    #[test]
    fn interrupting_one_execution_leaves_the_others_running() {
        let w = world();
        let (_a, session_a) = open(&w, scope("w", "a", "e1"), false);
        let (_b, session_b) = open(&w, scope("w", "b", "e2"), false);

        w.registry.interrupt(&target("w", "a", "e1")).unwrap();

        assert_eq!(session_a.calls(), ["interrupt"]);
        assert!(session_b.calls().is_empty());
        assert_eq!(
            w.registry.snapshot(&target("w", "b", "e2")).unwrap().status,
            SessionStatus::Running
        );
        assert_eq!(w.registry.user_action("e2"), None);
    }

    #[test]
    fn finishing_closes_the_session_and_nothing_can_control_it_any_more() {
        let w = world();
        let (handle, session) = open(&w, scope("w", "a", "e1"), true);
        let t = target("w", "a", "e1");
        handle.output("bye\n");

        handle.finish(Some(0));

        let snapshot = w.registry.snapshot(&t).unwrap();
        assert_eq!(snapshot.status, SessionStatus::Exited);
        assert_eq!(snapshot.exit_code, Some(0));
        assert_eq!(snapshot.output, "bye\n");
        assert!(w.registry.live_executions().is_empty());
        assert_eq!(w.registry.interrupt(&t), Err(ControlError::NotRunning));
        assert_eq!(w.registry.terminate(&t), Err(ControlError::NotRunning));
        assert_eq!(w.registry.input(&t, b"x"), Err(ControlError::NotRunning));
        assert!(session.calls().is_empty());
        let exited = w.sink.activity.lock().unwrap().last().unwrap().clone();
        assert_eq!(exited.kind, ExecutionEventKind::ProcessExited);
        assert_eq!(exited.metadata["exitCode"], "0");
    }

    #[test]
    fn a_dropped_handle_closes_its_session() {
        let w = world();
        let (handle, _) = open(&w, scope("w", "a", "e1"), false);

        drop(handle);

        let snapshot = w.registry.snapshot(&target("w", "a", "e1")).unwrap();
        assert_eq!(snapshot.status, SessionStatus::Exited);
        assert_eq!(snapshot.exit_code, None);
        assert!(w.registry.live_executions().is_empty());
    }

    #[test]
    fn only_the_latest_finished_sessions_are_retained() {
        let w = world();
        for n in 0..(RETAINED_FINISHED + 3) {
            let (handle, _) = open(&w, scope("w", "a", &format!("e{n}")), false);
            handle.finish(Some(0));
        }

        assert_eq!(w.registry.lock().entries.len(), RETAINED_FINISHED);
        assert_eq!(
            w.registry.snapshot(&target("w", "a", "e0")),
            Err(ControlError::NotFound)
        );
        assert!(w.registry.snapshot(&target("w", "a", "e10")).is_ok());
    }

    #[test]
    fn output_is_bounded_and_cut_on_a_character_boundary() {
        let w = world();
        let (handle, _) = open(&w, scope("w", "a", "e1"), false);
        let line = "é".repeat(1000);

        for _ in 0..(OUTPUT_LIMIT_BYTES / 100) {
            handle.output(&line);
        }

        let snapshot = w.registry.snapshot(&target("w", "a", "e1")).unwrap();
        assert!(snapshot.truncated);
        assert!(snapshot.output.len() <= OUTPUT_LIMIT_BYTES + OUTPUT_LIMIT_BYTES / 4);
        assert!(snapshot.output.chars().all(|c| c == 'é'));
    }

    #[test]
    fn manual_input_needs_a_runtime_that_reads_it() {
        let w = world();
        let (_read_only, _) = open(&w, scope("w", "a", "e1"), false);
        let (_interactive, session) = open(&w, scope("w", "b", "e2"), true);

        assert_eq!(
            w.registry.input(&target("w", "a", "e1"), b"hi"),
            Err(ControlError::InputNotSupported)
        );
        w.registry
            .input(&target("w", "b", "e2"), "olá\n".as_bytes())
            .unwrap();
        assert_eq!(session.calls(), ["write:olá\n"]);
    }

    #[test]
    fn input_is_passed_on_untouched_except_ctrl_c_which_is_an_interrupt() {
        let w = world();
        let (_h, session) = open(&w, scope("w", "a", "e1"), true);
        let t = target("w", "a", "e1");

        // Ctrl+D (EOF) and Ctrl+Z are bytes for the process's own terminal, not Atlas commands.
        w.registry.input(&t, &[0x04]).unwrap();
        w.registry.input(&t, &[0x1a]).unwrap();
        w.registry.input(&t, b"\x1b[A").unwrap();
        assert_eq!(w.registry.user_action("e1"), None);

        w.registry.input(&t, &[CTRL_C]).unwrap();
        assert_eq!(w.registry.user_action("e1"), Some(UserAction::Interrupted));
        assert_eq!(
            session.calls().last().map(String::as_str),
            Some("interrupt")
        );

        assert_eq!(
            w.registry.input(&target("w", "a", "e1"), b"a\x03b"),
            Err(ControlError::InvalidInput)
        );
        assert_eq!(w.registry.input(&t, &[]), Err(ControlError::InvalidInput));
        assert_eq!(
            w.registry.input(&t, &vec![b'x'; MAX_INPUT_BYTES + 1]),
            Err(ControlError::InvalidInput)
        );
    }

    #[test]
    fn resize_validates_the_size_and_reaches_the_process() {
        let w = world();
        let (_h, session) = open(&w, scope("w", "a", "e1"), false);
        let t = target("w", "a", "e1");

        w.registry.resize(&t, 100, 40).unwrap();

        assert_eq!(session.calls(), ["resize:100x40"]);
        let snapshot = w.registry.snapshot(&t).unwrap();
        assert_eq!((snapshot.cols, snapshot.rows), (100, 40));
        for (cols, rows) in [(0, 10), (10, 0), (1001, 10), (10, 5000)] {
            assert_eq!(
                w.registry.resize(&t, cols, rows),
                Err(ControlError::InvalidInput)
            );
        }
    }

    #[test]
    fn closing_the_application_terminates_every_live_process_without_blaming_the_user() {
        let w = world();
        let (_a, session_a) = open(&w, scope("w", "a", "e1"), false);
        let (done, session_done) = open(&w, scope("w", "b", "e2"), false);
        done.finish(Some(0));

        w.registry.terminate_all();

        assert_eq!(session_a.calls(), ["terminate"]);
        assert!(session_done.calls().is_empty());
        assert_eq!(w.registry.user_action("e1"), None);
    }
}
