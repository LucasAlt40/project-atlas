//! What Atlas knows, while a workflow runs, about the files of the run's isolated worktree.
//!
//! The state is *derived*: the worktree's files, compared by Git with the commit the workflow
//! started from (`baseline_revision`), are the only source. Nothing here comes from what an
//! agent says it did, and nothing needs a UI to exist.
//!
//! ```text
//! baseline_revision (commit)  --  files of the workflow worktree, as they are now
//!                         = the live changes
//! ```
//!
//! The project's own checkout is never part of it.

use serde::{Deserialize, Serialize};

use super::worktree::FileChange;

/// Whether the run's worktree can be looked at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorktreeAvailability {
    Available,
    /// The folder is gone (applied and removed, discarded, deleted by hand). The last known
    /// files are kept; nothing new can be learned.
    Missing,
    /// The folder is there but is not what Atlas made (another folder, its branch was
    /// switched): it is not read.
    Invalid,
}

/// What the run is doing, as far as the worktree is concerned. It only sets how often the
/// worktree is looked at when nothing reports a change; the files are always read as they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LivePhase {
    /// The worktree exists and no step has started (or the steps are between two).
    Idle,
    /// A step is working in it.
    Running,
    /// A step stopped to ask a person. The worktree is still watched.
    WaitingForInput,
    /// The step that was working in it was cancelled.
    Cancelled,
    /// The run completed: this is the final state.
    Ended,
    /// The run failed or was cancelled: the work is kept, this is the final state.
    Stopped,
}

impl LivePhase {
    pub fn is_final(self) -> bool {
        matches!(self, Self::Ended | Self::Stopped)
    }
}

/// How the worktree is being observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationMode {
    /// Filesystem events, with a slow full check behind them.
    Events,
    /// The platform could not watch the folder: it is looked at on a schedule that slows down
    /// when nothing changes.
    Polling,
    /// Not being observed (nothing started yet, or the run ended): the state is what the last
    /// full check found.
    Stopped,
}

/// The live state of one workflow run's worktree. A snapshot: `revision` orders it against the
/// [`LiveWorkspaceUpdate`]s the UI also receives (apply those with a greater revision).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveWorkspaceState {
    pub run_id: String,
    /// The primary worktree of the run: the only thing the UI names. The folder is derived by
    /// Atlas from it, never taken from the webview.
    pub worktree_execution_id: String,
    pub branch: String,
    /// The commit the workflow started from. What every file is compared with.
    pub baseline_revision: String,
    /// The commit the worktree is at now (the agents' own saved work), when it can be read.
    pub current_revision: Option<String>,
    pub availability: WorktreeAvailability,
    pub phase: LivePhase,
    pub observation: ObservationMode,
    /// Every file that differs from the baseline, in the worktree as it is now. At most
    /// [`crate::domain::worktree::MAX_CHANGESET_FILES`]; `files_changed` counts them all.
    pub files: Vec<FileChange>,
    pub files_changed: u32,
    pub additions: u32,
    pub deletions: u32,
    /// Counts every change of the state (not of the clock): a UI that has revision N and sees an
    /// update with N+1 can apply it; any other number means "read the state again".
    pub revision: u64,
    /// Milliseconds since the Unix epoch: the last time the state changed.
    pub updated_at: u64,
    /// The last time the whole worktree was compared with the baseline (rather than only the
    /// paths an event named).
    pub last_reconciled_at: Option<u64>,
}

/// What changed in a [`LiveWorkspaceState`], sent to the UI as it happens
/// (`live-workspace:changed`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveWorkspaceUpdate {
    pub run_id: String,
    pub worktree_execution_id: String,
    /// The revision of the state after this update.
    pub revision: u64,
    /// The whole worktree was compared again: `changed` is then *every* file, and whatever the UI
    /// had that is not in it is gone.
    pub full: bool,
    /// Files that were created, modified or renamed, or changed again.
    pub changed: Vec<FileChange>,
    /// Paths that no longer differ from the baseline (deleted files are *not* here: they are
    /// changes, with the status `deleted`).
    pub removed: Vec<String>,
    pub current_revision: Option<String>,
    pub availability: WorktreeAvailability,
    pub phase: LivePhase,
    pub observation: ObservationMode,
    pub files_changed: u32,
    pub additions: u32,
    pub deletions: u32,
    pub updated_at: u64,
}

/// What a file of the run's worktree holds right now, as the Live Workspace's viewer asks for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveFileKind {
    /// Text; `content` has it (cut at the limit, then `truncated`).
    Text,
    /// Not text: not sent.
    Binary,
    /// The file does not exist in the worktree (a deleted file, or one that is gone again).
    Deleted,
    /// A link: it is never followed, so nothing outside the worktree can be read through it.
    Symlink,
    /// A folder or something that is not a file.
    NotFile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveFile {
    pub path: String,
    pub kind: LiveFileKind,
    pub content: Option<String>,
    /// Bytes on disk, when there is a file.
    pub size: Option<u64>,
    /// `content` is only the beginning of the file.
    pub truncated: bool,
}

/// The most of a file the viewer is given.
pub const MAX_LIVE_FILE_BYTES: usize = 1024 * 1024;
/// The most diff text one request returns.
pub const MAX_LIVE_DIFF_BYTES: usize = 400_000;

/// Whether `path` is a plain path inside a worktree: relative, no `..` or `.` parts, no NUL or
/// newline, not inside Git's own folder. Everything that names a file for the Live Workspace
/// passes this first (and is checked again where the file is read).
pub fn is_plain_relative_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    let windows_drive = bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic();
    !(path.is_empty()
        || path.len() > 1000
        || path.starts_with(['/', '\\'])
        || windows_drive
        || path.contains(['\0', '\n', '\\'])
        || path
            .split('/')
            .any(|part| part.is_empty() || part == ".." || part == ".")
        || path.split('/').next() == Some(".git"))
}
