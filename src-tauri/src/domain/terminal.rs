//! What the UI may know about the live process of an execution: its output, its state and
//! who stopped it. Never a PID, a signal or a PTY: those stay in Rust (ADR 0007).

use serde::Serialize;

/// A PTY merges the child's stdout and stderr into one stream, so today every chunk is
/// `stdout`; the field is part of the contract so a pipe-backed session can tell them apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalStream {
    Stdout,
    #[allow(dead_code)]
    Stderr,
}

/// What the process is doing. `Interrupting` and `Terminating` are the time between the user's
/// request and the process actually exiting; they are live states, not stored on the execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Running,
    Interrupting,
    Terminating,
    Exited,
}

/// What the *user* did to the process. Distinguishes a stop the user asked for from a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UserAction {
    Interrupted,
    Terminated,
}

/// A piece of the process's output, in order (`seq` counts chunks of one session).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalChunk {
    pub execution_id: String,
    pub process_session_id: String,
    pub workspace_id: String,
    pub agent_id: String,
    pub timestamp: u64,
    pub stream: TerminalStream,
    pub seq: u64,
    pub data: String,
}

/// The process changed state. Sent on every transition, so the UI never infers one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatusEvent {
    pub execution_id: String,
    pub process_session_id: String,
    pub workspace_id: String,
    pub agent_id: String,
    pub status: SessionStatus,
    pub user_action: Option<UserAction>,
    pub exit_code: Option<i32>,
    pub timestamp: u64,
}

/// Everything the UI needs to (re)draw an execution's terminal: the retained output plus the
/// state. Output is ephemeral and bounded: it is neither saved nor kept forever.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSnapshot {
    pub execution_id: String,
    pub process_session_id: String,
    pub workspace_id: String,
    pub agent_id: String,
    pub status: SessionStatus,
    pub user_action: Option<UserAction>,
    /// The command as one line of text, for people to read.
    pub command: String,
    pub started_at: u64,
    pub ended_at: Option<u64>,
    pub exit_code: Option<i32>,
    pub cols: u16,
    pub rows: u16,
    /// The runtime accepts manual input; when `false` the terminal is read-only.
    pub input_enabled: bool,
    /// The output retained so far (the newest part when it was cut).
    pub output: String,
    /// The `seq` the next chunk will carry: chunks below it are already in `output`.
    pub next_seq: u64,
    /// Older output was dropped to stay within the retention limit.
    pub truncated: bool,
}
