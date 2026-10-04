//! Runs a process attached to a pseudo-terminal (PTY) and exposes it as a
//! [`ProcessSession`]. See ADR 0007 for why a PTY and not pipes.
//!
//! What the PTY gives us that pipes do not:
//!
//! - the child sees a terminal (`isatty`), so tools that change behaviour without one behave
//!   as they would for a person;
//! - Ctrl+C is *the byte 0x03 written to the terminal*: the terminal driver turns it into an
//!   interrupt for the terminal's foreground process group (Unix) or a console control event
//!   (`ConPTY` on Windows). One mechanism, no `kill(pid)`, and it reaches the whole tree of
//!   whatever the CLI started in the foreground, which a signal to the root PID would not;
//! - the child is a session leader (`setsid`), so its process group is its own and can be
//!   ended as a unit without touching Atlas or anything else.
//!
//! What it costs: a PTY has one output stream (stdout and stderr merge) and one input stream
//! that is the user's keyboard. There is no second channel to hand the child a prompt, so a
//! process started here gets its prompt as an argument, and stderr is not separate.

use std::ffi::OsStr;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, Child, ChildKiller, CommandBuilder, MasterPty, PtySize};

use crate::application::process::{
    ProcessContext, ProcessError, ProcessEvent, ProcessOutput, ProcessSpec, TerminalRequest,
    TerminalSize,
};
use crate::application::security::guard::display_command;
use crate::application::sessions::{
    OpenSession, ProcessSession, SessionError, SessionHandle, SessionRegistry,
};

const POLL_INTERVAL: Duration = Duration::from_millis(20);
/// Output kept for the result, like the pipe runner.
const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
/// After the process exits, how long to wait for output still in flight.
const DRAIN_TIMEOUT: Duration = Duration::from_millis(500);
/// How long a process gets to leave after a terminate before it is killed outright.
const TERMINATE_GRACE: Duration = Duration::from_secs(2);
/// After a process exits on its own: how long what it left behind gets to react to the
/// terminal's hang-up before the rest of its group is ended.
#[cfg(unix)]
const LEFTOVER_GRACE: Duration = Duration::from_millis(100);
const CTRL_C: u8 = 0x03;

fn spawn_error(error: impl std::fmt::Display) -> ProcessError {
    ProcessError::Spawn(error.to_string())
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // A poisoned lock only means another thread panicked; the data is still usable.
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The OS side of one PTY process.
struct PtyShared {
    pid: u32,
    /// `true` once the child has been waited for. Held while signalling, so a signal is never
    /// sent to a PID the OS may already have given to someone else.
    reaped: Mutex<bool>,
    /// Released when the process ends: dropping them closes the terminal.
    writer: Mutex<Option<Box<dyn Write + Send>>>,
    master: Mutex<Option<Box<dyn MasterPty + Send>>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
}

impl PtyShared {
    /// Ends the process group and the process itself, right now.
    fn kill_now(&self) {
        let reaped = lock(&self.reaped);
        if !*reaped {
            self.signal_group(Signal::Kill);
            let _ = lock(&self.killer).kill();
        }
    }

    fn signal_group(&self, signal: Signal) {
        #[cfg(unix)]
        {
            use nix::sys::signal::{killpg, Signal as NixSignal};
            use nix::unistd::Pid;
            let nix_signal = match signal {
                Signal::Term => NixSignal::SIGTERM,
                Signal::Kill => NixSignal::SIGKILL,
            };
            // The child called `setsid`, so its process group id is its PID. Errors mean the
            // group is already gone, which is the goal. A PID that is not a real child's (0 would
            // mean *our own* group, 1 is init) is never signalled.
            if let Some(pid) = i32::try_from(self.pid).ok().filter(|pid| *pid > 1) {
                let _ = killpg(Pid::from_raw(pid), nix_signal);
            }
        }
        #[cfg(windows)]
        {
            // Windows has no graceful group signal; `taskkill /T` ends the tree.
            let _ = signal;
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &self.pid.to_string(), "/T", "/F"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
    }

    /// Ends whatever the exited process left in its group. The terminal's hang-up reaches what
    /// is attached to it, but a process that ignores the hang-up would outlive the execution it
    /// belonged to. The group id is the PID of the process that was just reaped: no PID is
    /// given to someone else while a group with that id has members, and with none left the
    /// signal finds nothing. Called right after the exit, so there is no window for reuse.
    /// (A process that left the group with `setsid` is out of reach; see ADR 0007.)
    #[cfg_attr(not(unix), allow(clippy::unused_self))]
    fn end_leftovers(&self) {
        #[cfg(unix)]
        {
            thread::sleep(LEFTOVER_GRACE);
            self.signal_group(Signal::Kill);
        }
    }

    /// The process has exited: closes the terminal (the kernel hangs up on whatever is still
    /// attached to it), then ends what ignored that.
    fn release(&self) {
        self.close();
        self.end_leftovers();
    }

    /// Closes the terminal. The kernel hangs up on whatever is still attached to it.
    fn close(&self) {
        lock(&self.writer).take();
        lock(&self.master).take();
    }
}

#[derive(Clone, Copy)]
enum Signal {
    Term,
    Kill,
}

struct PtySession(Arc<PtyShared>);

impl ProcessSession for PtySession {
    fn write(&self, data: &[u8]) -> Result<(), SessionError> {
        // Not holding `reaped` while writing: a process that does not read its input can make a
        // write wait, and `reaped` is what ending the process (terminate, timeout) needs.
        if *lock(&self.0.reaped) {
            return Err(SessionError::Exited);
        }
        let mut writer = lock(&self.0.writer);
        let writer = writer.as_mut().ok_or(SessionError::Exited)?;
        writer
            .write_all(data)
            .and_then(|()| writer.flush())
            .map_err(|error| SessionError::Io(error.to_string()))
    }

    /// Ctrl+C. If a paste is stuck in the terminal (the process is not reading) the interrupt
    /// cannot get in behind it: it says so instead of waiting, and the user can terminate.
    fn interrupt(&self) -> Result<(), SessionError> {
        if *lock(&self.0.reaped) {
            return Err(SessionError::Exited);
        }
        let mut writer = match self.0.writer.try_lock() {
            Ok(writer) => writer,
            Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => {
                return Err(SessionError::Io(
                    "the terminal is busy with earlier input".to_owned(),
                ))
            }
        };
        let writer = writer.as_mut().ok_or(SessionError::Exited)?;
        writer
            .write_all(&[CTRL_C])
            .and_then(|()| writer.flush())
            .map_err(|error| SessionError::Io(error.to_string()))
    }

    fn terminate(&self) -> Result<(), SessionError> {
        {
            let reaped = lock(&self.0.reaped);
            if *reaped {
                return Err(SessionError::Exited);
            }
            self.0.signal_group(Signal::Term);
            #[cfg(windows)]
            let _ = lock(&self.0.killer).kill();
        }
        // The polite request may be ignored: do not wait for it on the caller's thread.
        let shared = self.0.clone();
        thread::spawn(move || {
            let deadline = Instant::now() + TERMINATE_GRACE;
            while Instant::now() < deadline {
                if *lock(&shared.reaped) {
                    return;
                }
                thread::sleep(POLL_INTERVAL);
            }
            shared.kill_now();
        });
        Ok(())
    }

    fn resize(&self, size: TerminalSize) -> Result<(), SessionError> {
        if *lock(&self.0.reaped) {
            return Err(SessionError::Exited);
        }
        let master = lock(&self.0.master);
        let master = master.as_ref().ok_or(SessionError::Exited)?;
        master
            .resize(pty_size(size))
            .map_err(|error| SessionError::Io(error.to_string()))
    }
}

fn pty_size(size: TerminalSize) -> PtySize {
    PtySize {
        rows: size.rows,
        cols: size.cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

/// Starts `executable` in a new PTY and runs it to completion, like the pipe runner, while
/// publishing the live process as a session in `sessions` (when the spec has an execution).
pub fn run_in_terminal(
    executable: &Path,
    path: &OsStr,
    spec: &ProcessSpec,
    request: TerminalRequest,
    sessions: Option<&Arc<SessionRegistry>>,
    on_event: &dyn Fn(ProcessEvent),
) -> Result<ProcessOutput, ProcessError> {
    if spec.stdin.is_some() {
        return Err(ProcessError::Spawn(
            "a process in a terminal has no input channel for a prompt: pass it as an argument"
                .to_owned(),
        ));
    }
    let pair = native_pty_system()
        .openpty(pty_size(request.size))
        .map_err(spawn_error)?;
    let mut command = CommandBuilder::new(executable);
    command.args(&spec.args);
    for (key, value) in &spec.env {
        command.env(key, value);
    }
    // After `spec.env`, so nothing in it can replace the search path.
    command.env("PATH", path);
    command.env("TERM", "xterm-256color");
    if let Some(cwd) = &spec.cwd {
        // Tools that trust `PWD` (OpenCode does) must see the project, not Atlas's folder.
        command.cwd(cwd);
        command.env("PWD", cwd);
    }
    let mut child = pair.slave.spawn_command(command).map_err(spawn_error)?;
    // Only the child holds the slave side now: when it and its children are gone, reads end.
    drop(pair.slave);
    let reader = pair.master.try_clone_reader().map_err(spawn_error)?;
    let writer = pair.master.take_writer().map_err(spawn_error)?;
    let shared = Arc::new(PtyShared {
        pid: child.process_id().unwrap_or_default(),
        reaped: Mutex::new(false),
        writer: Mutex::new(Some(writer)),
        master: Mutex::new(Some(pair.master)),
        killer: Mutex::new(child.clone_killer()),
    });
    on_event(ProcessEvent::Spawned);
    // The prompt travels with the arguments: it is delivered at the start.
    on_event(ProcessEvent::InputSent);

    let scope = match &spec.context {
        ProcessContext::Runtime(scope) | ProcessContext::AgentRequested(scope) => Some(scope),
        ProcessContext::Probe => None,
    };
    let session = sessions.zip(scope).map(|(registry, scope)| {
        registry.open(OpenSession {
            scope: scope.clone(),
            command: display_command(spec),
            terminal: request,
            session: Arc::new(PtySession(shared.clone())),
        })
    });

    let (chunks, received) = mpsc::channel();
    let reader_thread = thread::spawn(move || read_chunks(reader, &chunks));

    let mut output = Output::default();
    // An idle limit, not a wall-clock one: any output restarts it.
    let mut deadline = Instant::now() + spec.timeout;
    let outcome = loop {
        match received.recv_timeout(POLL_INTERVAL) {
            Ok(chunk) => {
                deadline = Instant::now() + spec.timeout;
                output.accept(&chunk, session.as_ref(), on_event);
            }
            Err(RecvTimeoutError::Timeout) => {}
            // Output ended (every holder of the terminal is gone); wait for the exit status.
            Err(RecvTimeoutError::Disconnected) => thread::sleep(POLL_INTERVAL),
        }
        match reap(&shared, child.as_mut()) {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() >= deadline => break Err(ProcessError::Timeout),
            Ok(None) => {}
            Err(error) => break Err(ProcessError::Io(error.to_string())),
        }
    };

    let status = match outcome {
        Ok(status) => status,
        Err(error) => {
            // Timeout or a broken wait: the process must not outlive its execution.
            shared.kill_now();
            let _ = child.wait();
            *lock(&shared.reaped) = true;
            shared.close();
            let _ = reader_thread.join();
            if let Some(session) = session {
                session.finish(None);
            }
            return Err(error);
        }
    };

    // Output still in flight after the exit. Bounded: a grandchild that kept the terminal open
    // must not keep the execution open.
    let drain_until = Instant::now() + DRAIN_TIMEOUT;
    while let Some(left) = drain_until.checked_duration_since(Instant::now()) {
        match received.recv_timeout(left) {
            Ok(chunk) => output.accept(&chunk, session.as_ref(), on_event),
            Err(_) => break,
        }
    }
    output.finish(on_event);
    shared.release();
    let exit_code = status
        .signal()
        .is_none()
        .then(|| i32::try_from(status.exit_code()).unwrap_or(i32::MAX));
    if let Some(session) = session {
        session.finish(exit_code);
    }
    Ok(ProcessOutput {
        exit_code,
        stdout: output.text,
        // A terminal has one stream: the runtimes read their diagnostics from `stdout`.
        stderr: String::new(),
    })
}

fn reap(
    shared: &PtyShared,
    child: &mut (dyn Child + Send + Sync),
) -> std::io::Result<Option<portable_pty::ExitStatus>> {
    let mut reaped = lock(&shared.reaped);
    let status = child.try_wait()?;
    if status.is_some() {
        *reaped = true;
    }
    Ok(status)
}

/// Reads the terminal and sends its output as text. A character split between two reads is
/// held back until it is complete.
fn read_chunks(mut reader: Box<dyn Read + Send>, chunks: &mpsc::Sender<String>) {
    let mut buffer = [0_u8; 8192];
    let mut pending: Vec<u8> = Vec::new();
    loop {
        match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => {
                pending.extend_from_slice(&buffer[..read]);
                let text = take_text(&mut pending);
                if !text.is_empty() && chunks.send(text).is_err() {
                    return;
                }
            }
        }
    }
    if !pending.is_empty() {
        let _ = chunks.send(String::from_utf8_lossy(&pending).into_owned());
    }
}

/// Removes and returns the complete text at the front of `bytes`; an unfinished character at
/// the end stays. Invalid bytes become U+FFFD.
fn take_text(bytes: &mut Vec<u8>) -> String {
    let mut text = String::new();
    loop {
        match std::str::from_utf8(bytes) {
            Ok(valid) => {
                text.push_str(valid);
                bytes.clear();
                return text;
            }
            Err(error) => {
                let valid = error.valid_up_to();
                text.push_str(&String::from_utf8_lossy(&bytes[..valid]));
                match error.error_len() {
                    // Cut off at the end: wait for the rest.
                    None => {
                        bytes.drain(..valid);
                        return text;
                    }
                    Some(invalid) => {
                        text.push('\u{FFFD}');
                        bytes.drain(..valid + invalid);
                    }
                }
            }
        }
    }
}

/// What the process wrote, as the runtimes read it: the raw text goes to the terminal view,
/// the cleaned text (no control sequences, no carriage returns) to line events and the result.
#[derive(Default)]
struct Output {
    text: String,
    partial_line: String,
    stripper: AnsiStripper,
}

impl Output {
    fn accept(
        &mut self,
        raw: &str,
        session: Option<&SessionHandle>,
        on_event: &dyn Fn(ProcessEvent),
    ) {
        if let Some(session) = session {
            session.output(raw);
        }
        let clean = self.stripper.push(raw);
        if self.text.len() < MAX_OUTPUT_BYTES {
            self.text.push_str(&clean);
        }
        self.partial_line.push_str(&clean);
        while let Some(end) = self.partial_line.find('\n') {
            let line: String = self.partial_line.drain(..=end).collect();
            on_event(ProcessEvent::StdoutLine(
                line.trim_end_matches('\n').to_owned(),
            ));
        }
    }

    /// The last line, if the process ended without a final newline.
    fn finish(&mut self, on_event: &dyn Fn(ProcessEvent)) {
        if !self.partial_line.is_empty() {
            on_event(ProcessEvent::StdoutLine(std::mem::take(
                &mut self.partial_line,
            )));
        }
    }
}

/// Removes terminal control sequences (colours, cursor movement, window titles…) and carriage
/// returns from text that arrives in pieces. It keeps its place between pieces, so a sequence
/// cut in two is still removed whole.
#[derive(Default)]
struct AnsiStripper {
    state: StripState,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum StripState {
    #[default]
    Text,
    /// After ESC.
    Escape,
    /// In a CSI sequence (`ESC [` … final byte).
    Csi,
    /// In a string sequence (OSC, DCS…) that ends with BEL or `ESC \`.
    Str,
    /// ESC seen inside a string sequence.
    StrEscape,
}

impl AnsiStripper {
    #[allow(clippy::match_same_arms)] // one arm per state transition keeps the table readable
    fn push(&mut self, text: &str) -> String {
        let mut clean = String::with_capacity(text.len());
        for c in text.chars() {
            self.state = match (self.state, c) {
                (StripState::Text, '\u{1b}') => StripState::Escape,
                (StripState::Text, '\r') => StripState::Text,
                (StripState::Text, c) => {
                    // Keep the text and the layout (newline, tab); drop other control codes.
                    if !c.is_control() || c == '\n' || c == '\t' {
                        clean.push(c);
                    }
                    StripState::Text
                }
                (StripState::Escape, '[') => StripState::Csi,
                (StripState::Escape, ']' | 'P' | 'X' | '^' | '_') => StripState::Str,
                // `ESC ( B` and friends: intermediate bytes, then one final byte.
                (StripState::Escape, ' '..='/') => StripState::Escape,
                (StripState::Escape, _) => StripState::Text,
                (StripState::Csi, '\u{40}'..='\u{7e}') => StripState::Text,
                (StripState::Csi, _) => StripState::Csi,
                (StripState::Str, '\u{7}') => StripState::Text,
                (StripState::Str, '\u{1b}') => StripState::StrEscape,
                (StripState::Str, _) => StripState::Str,
                (StripState::StrEscape, '\\') => StripState::Text,
                (StripState::StrEscape, _) => StripState::Str,
            };
        }
        clean
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_colours_cursor_codes_titles_and_carriage_returns() {
        let mut stripper = AnsiStripper::default();

        let clean = stripper.push(
            "\u{1b}[?25l\u{1b}[91m\u{1b}[1mError: \u{1b}[0mboom\r\n\u{1b}]0;title\u{7}ok\u{1b}(B\r\n",
        );

        assert_eq!(clean, "Error: boom\nok\n");
    }

    #[test]
    fn a_sequence_cut_between_pieces_is_still_removed() {
        let mut stripper = AnsiStripper::default();

        let pieces = ["a\u{1b}", "[3", "1mb\u{1b}]0;ti", "tle\u{1b}", "\\c"];
        let clean: String = pieces.iter().map(|p| stripper.push(p)).collect();

        assert_eq!(clean, "abc");
    }

    #[test]
    fn text_is_cut_only_at_character_boundaries() {
        let bytes = "ação".as_bytes();
        let mut pending = bytes[..2].to_vec();

        assert_eq!(take_text(&mut pending), "a");
        pending.extend_from_slice(&bytes[2..]);
        assert_eq!(take_text(&mut pending), "ção");
        assert_eq!(pending, Vec::<u8>::new());

        let mut invalid = vec![b'a', 0xff, b'b'];
        assert_eq!(take_text(&mut invalid), "a\u{FFFD}b");
    }

    #[test]
    fn lines_are_reported_as_they_complete_and_the_last_one_at_the_end() {
        let lines = std::cell::RefCell::new(Vec::new());
        let on_event = |event| {
            if let ProcessEvent::StdoutLine(line) = event {
                lines.borrow_mut().push(line);
            }
        };
        let mut output = Output::default();

        output.accept("one\r\ntw", None, &on_event);
        output.accept("o\r\nthree", None, &on_event);
        assert_eq!(*lines.borrow(), ["one", "two"]);
        output.finish(&on_event);

        assert_eq!(*lines.borrow(), ["one", "two", "three"]);
        assert_eq!(output.text, "one\ntwo\nthree");
    }
}

/// These start real processes in real PTYs. Unix only: they use `sh` and `cat`.
#[cfg(all(test, unix))]
#[allow(clippy::assert_is_empty)]
mod process_tests {
    use std::sync::Mutex as StdMutex;

    use super::*;
    use crate::application::process::ExecutionScope;
    use crate::application::sessions::{ControlError, SessionSink, SessionTarget};
    use crate::domain::execution::ExecutionEvent;
    use crate::domain::terminal::{SessionStatus, SessionStatusEvent, TerminalChunk, UserAction};

    #[derive(Default)]
    struct Collector {
        output: StdMutex<String>,
    }

    impl SessionSink for Collector {
        fn on_output(&self, chunk: &TerminalChunk) {
            self.output.lock().unwrap().push_str(&chunk.data);
        }
        fn on_status(&self, _: &SessionStatusEvent) {}
        fn on_activity(&self, _: &ExecutionEvent) {}
    }

    fn scope(execution: &str) -> ExecutionScope {
        ExecutionScope {
            execution_id: execution.to_owned(),
            ..ExecutionScope::for_tests()
        }
    }

    fn target(execution: &str) -> SessionTarget {
        let scope = scope(execution);
        SessionTarget {
            execution_id: scope.execution_id,
            workspace_id: scope.workspace_id,
            agent_id: scope.agent_id,
        }
    }

    fn spec(execution: &str, script: &str, timeout: Duration) -> ProcessSpec {
        ProcessSpec {
            program: "sh".to_owned(),
            args: vec!["-c".to_owned(), script.to_owned()],
            context: ProcessContext::Runtime(scope(execution)),
            terminal: Some(TerminalRequest::new(true)),
            ..ProcessSpec::probe("sh", &[], timeout)
        }
    }

    fn registry() -> (Arc<SessionRegistry>, Arc<Collector>) {
        let sink = Arc::new(Collector::default());
        (Arc::new(SessionRegistry::with_sink(sink.clone())), sink)
    }

    /// Starts the process on its own thread; returns the join handle of its result.
    fn start(
        registry: &Arc<SessionRegistry>,
        spec: ProcessSpec,
    ) -> thread::JoinHandle<Result<ProcessOutput, ProcessError>> {
        let registry = registry.clone();
        thread::spawn(move || {
            run_in_terminal(
                Path::new("/bin/sh"),
                &std::env::var_os("PATH").unwrap_or_default(),
                &spec,
                spec.terminal.unwrap(),
                Some(&registry),
                &|_| {},
            )
        })
    }

    fn wait_until(what: &str, condition: impl Fn() -> bool) {
        for _ in 0..300 {
            if condition() {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("timed out waiting for: {what}");
    }

    fn is_live(registry: &SessionRegistry, execution: &str) -> bool {
        registry.live_executions().iter().any(|e| e == execution)
    }

    #[test]
    fn runs_in_a_real_terminal_streams_output_and_closes_its_session() {
        let (registry, sink) = registry();
        let spec = spec(
            "e1",
            "if [ -t 1 ]; then echo tty; else echo pipe; fi; echo out; echo err >&2; exit 3",
            Duration::from_secs(10),
        );

        let output = start(&registry, spec).join().unwrap().unwrap();

        assert_eq!(output.exit_code, Some(3));
        // stdout and stderr merge in a terminal; the child saw a real one.
        assert_eq!(output.stdout, "tty\nout\nerr\n");
        assert_eq!(*sink.output.lock().unwrap(), "tty\r\nout\r\nerr\r\n");
        let snapshot = registry.snapshot(&target("e1")).unwrap();
        assert_eq!(snapshot.status, SessionStatus::Exited);
        assert_eq!(snapshot.exit_code, Some(3));
        assert!(registry.live_executions().is_empty());
    }

    #[test]
    fn interrupt_stops_a_running_process_and_it_is_not_left_behind() {
        let (registry, _) = registry();
        let job = start(
            &registry,
            spec(
                "e1",
                "echo started; sleep 30; echo never",
                Duration::from_secs(60),
            ),
        );
        wait_until("the process to start", || {
            registry
                .snapshot(&target("e1"))
                .is_ok_and(|s| s.output.contains("started"))
        });

        registry.interrupt(&target("e1")).unwrap();
        let output = job.join().unwrap().unwrap();

        assert!(!output.stdout.contains("never"));
        assert_ne!(output.exit_code, Some(0));
        assert_eq!(registry.user_action("e1"), Some(UserAction::Interrupted));
        assert!(!is_live(&registry, "e1"));
        assert_eq!(
            registry.interrupt(&target("e1")),
            Err(ControlError::NotRunning)
        );
    }

    #[test]
    fn terminate_ends_a_process_that_ignores_the_interrupt_and_its_children() {
        let (registry, _) = registry();
        // Ignores INT and TERM, and has a child doing the same: only a forced end stops both.
        let script = "trap '' INT TERM; (trap '' INT TERM; sleep 30) & echo started; wait";
        let job = start(&registry, spec("e1", script, Duration::from_secs(60)));
        wait_until("the process to start", || {
            registry
                .snapshot(&target("e1"))
                .is_ok_and(|s| s.output.contains("started"))
        });

        registry.interrupt(&target("e1")).unwrap();
        thread::sleep(Duration::from_millis(300));
        assert!(is_live(&registry, "e1"), "the process ignores Ctrl+C");
        registry.terminate(&target("e1")).unwrap();
        let output = job.join().unwrap().unwrap();

        assert_ne!(output.exit_code, Some(0));
        assert_eq!(registry.user_action("e1"), Some(UserAction::Terminated));
        assert!(!is_live(&registry, "e1"));
    }

    #[test]
    fn interrupting_one_process_does_not_touch_another() {
        let (registry, _) = registry();
        let long = |execution: &str| {
            start(
                &registry,
                spec(
                    execution,
                    "echo started; sleep 30; echo done",
                    Duration::from_secs(60),
                ),
            )
        };
        let (a, b) = (long("a"), long("b"));
        for execution in ["a", "b"] {
            wait_until("both to start", || {
                registry
                    .snapshot(&target(execution))
                    .is_ok_and(|s| s.output.contains("started"))
            });
        }

        registry.interrupt(&target("a")).unwrap();
        a.join().unwrap().unwrap();

        assert!(is_live(&registry, "b"), "b was not affected");
        assert_eq!(registry.user_action("b"), None);
        registry.terminate(&target("b")).unwrap();
        b.join().unwrap().unwrap();
    }

    #[test]
    fn manual_input_reaches_a_process_that_reads_it() {
        let (registry, _) = registry();
        let job = start(
            &registry,
            spec(
                "e1",
                "read line; echo \"got:$line\"",
                Duration::from_secs(10),
            ),
        );
        wait_until("the session", || is_live(&registry, "e1"));

        registry.input(&target("e1"), b"hello\n").unwrap();
        let output = job.join().unwrap().unwrap();

        assert!(output.stdout.contains("got:hello"), "{:?}", output.stdout);
        assert_eq!(output.exit_code, Some(0));
    }

    #[test]
    fn resize_changes_the_size_the_process_sees() {
        let (registry, _) = registry();
        let job = start(
            &registry,
            spec(
                "e1",
                "echo ready; read x; stty size",
                Duration::from_secs(10),
            ),
        );
        wait_until("ready", || {
            registry
                .snapshot(&target("e1"))
                .is_ok_and(|s| s.output.contains("ready"))
        });

        registry.resize(&target("e1"), 91, 27).unwrap();
        registry.input(&target("e1"), b"\n").unwrap();
        let output = job.join().unwrap().unwrap();

        assert!(output.stdout.contains("27 91"), "{:?}", output.stdout);
    }

    #[test]
    fn a_process_that_exceeds_its_timeout_is_killed() {
        let (registry, _) = registry();
        let started = Instant::now();

        let result = start(
            &registry,
            spec(
                "e1",
                "trap '' INT TERM; sleep 30",
                Duration::from_millis(300),
            ),
        )
        .join()
        .unwrap();

        assert_eq!(result, Err(ProcessError::Timeout));
        assert!(started.elapsed() < Duration::from_secs(10));
        assert!(!is_live(&registry, "e1"));
    }

    #[test]
    fn the_working_directory_is_what_the_tool_sees_in_pwd() {
        let dir = std::fs::canonicalize(std::env::temp_dir()).unwrap();
        let (registry, _) = registry();
        let mut spec = spec("e1", "pwd -P; echo \"$PWD\"", Duration::from_secs(10));
        spec.cwd = Some(dir.clone());

        let output = start(&registry, spec).join().unwrap().unwrap();

        let lines: Vec<_> = output.stdout.lines().collect();
        assert_eq!(lines, [dir.to_str().unwrap(), dir.to_str().unwrap()]);
    }

    #[test]
    fn a_prompt_cannot_be_sent_on_stdin_to_a_terminal() {
        let (registry, _) = registry();
        let mut spec = spec("e1", "true", Duration::from_secs(5));
        spec.stdin = Some("prompt".to_owned());

        let result = start(&registry, spec).join().unwrap();

        assert!(matches!(result, Err(ProcessError::Spawn(_))));
        assert!(registry.snapshot(&target("e1")).is_err());
    }

    /// Whether a process with this id is alive (signal 0 only checks).
    fn alive(pid: i32) -> bool {
        use nix::sys::signal::kill;
        use nix::unistd::Pid;
        kill(Pid::from_raw(pid), None).is_ok()
    }

    #[test]
    fn a_big_paste_into_a_process_that_does_not_read_never_makes_it_unstoppable() {
        let (registry, _) = registry();
        let job = start(
            &registry,
            spec("e1", "echo started; sleep 30", Duration::from_secs(60)),
        );
        wait_until("the process to start", || {
            registry
                .snapshot(&target("e1"))
                .is_ok_and(|s| s.output.contains("started"))
        });
        // The process never reads its input: the terminal's buffer fills and the write waits.
        let writer = {
            let registry = registry.clone();
            thread::spawn(move || {
                let _ = registry.input(&target("e1"), &vec![b'x'; 60_000]);
            })
        };
        thread::sleep(Duration::from_millis(300));

        let started = Instant::now();
        registry.interrupt(&target("e1")).ok();
        registry.terminate(&target("e1")).unwrap();
        let result = job.join().unwrap();

        assert!(
            started.elapsed() < Duration::from_secs(10),
            "{:?}",
            started.elapsed()
        );
        assert!(result.is_ok());
        assert!(!is_live(&registry, "e1"));
        writer.join().unwrap();
    }

    #[test]
    fn a_process_left_behind_by_one_that_exited_does_not_outlive_its_execution() {
        let (registry, _) = registry();
        // The shell leaves at once; its background child ignores the hang-up the terminal sends.
        let script = "(trap '' HUP; exec sleep 30) & echo \"child:$!\"";

        let output = start(&registry, spec("e1", script, Duration::from_secs(10)))
            .join()
            .unwrap()
            .unwrap();

        let pid: i32 = output
            .stdout
            .lines()
            .find_map(|line| line.strip_prefix("child:"))
            .and_then(|pid| pid.trim().parse().ok())
            .expect("the script reports its child");
        // Give the kernel a moment to deliver whatever was sent to the group.
        wait_until("the leftover process to be gone", || !alive(pid));
    }
}
