//! Git worktree isolation: each execution of an agent that asks for it works in its own
//! worktree and branch instead of the project's checkout.
//!
//! ```text
//! ExecutionService -> WorktreeService -> WorktreeManager (port) -> Git
//!                          |
//!                          +-- persists ExecutionWorktree, asks the policy about merging
//! ```
//!
//! - [`WorktreeManager`] is the port to Git. Nothing else in the core runs `git worktree`.
//! - [`WorktreeService`] is the lifecycle: prepare before the runtime starts, finalize when it
//!   ends, merge when the policy and the user allow it, clean up what was merged.
//! - [`WorktreeLayout`] decides names and places, and is the only place that derives a branch or
//!   a path from an id, so no caller-supplied string ever becomes a Git argument or a path.
//!
//! The runtime knows none of this: it is handed a working directory.

mod layout;
mod service;

use std::fmt;
use std::path::{Path, PathBuf};

pub use layout::WorktreeLayout;
pub use service::{RunOutcome, WorktreeService};

use super::errors::{AppError, ErrorCode};
use crate::domain::execution::FailureKind;

/// Why a worktree operation did not happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorktreeError {
    /// The project is not inside a Git repository. Atlas never runs `git init` itself.
    GitRepositoryRequired,
    /// The `git` program could not be run.
    GitUnavailable,
    /// HEAD is not on a branch (detached) or the repository has no commit yet: there is no base
    /// to start from.
    BaseUnavailable(String),
    /// An id that is not one Atlas generated (it would not make a safe branch or folder name).
    InvalidIdentifier(String),
    /// The branch or folder for this execution already exists. Never reused, never overwritten.
    Collision(String),
    /// A path that is not inside the folder Atlas keeps worktrees in.
    OutsideRoot(String),
    NotFound(String),
    /// The worktree is not in a state this operation applies to.
    InvalidState(String),
    /// The agent's policy does not allow Git writes, so Atlas will not merge.
    PolicyDenied,
    /// Another operation on this execution's worktree is running.
    Busy,
    /// The merge did not happen; see the worktree's merge status for why.
    NotMergeable(String),
    Git(String),
    Storage(AppError),
}

impl fmt::Display for WorktreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GitRepositoryRequired => write!(
                f,
                "This execution requires Git worktree isolation, but the project is not a Git repository"
            ),
            Self::GitUnavailable => write!(f, "Git could not be run"),
            Self::BaseUnavailable(why) => write!(f, "No base branch to start from: {why}"),
            Self::InvalidIdentifier(id) => write!(f, "Not a valid identifier for a worktree: {id}"),
            Self::Collision(what) => write!(f, "Already exists, not reused: {what}"),
            Self::OutsideRoot(path) => write!(f, "Outside Atlas's worktree folder: {path}"),
            Self::NotFound(id) => write!(f, "No worktree for execution {id}"),
            Self::InvalidState(why) => write!(f, "Not possible in this state: {why}"),
            Self::PolicyDenied => write!(f, "The agent's policy does not allow merging"),
            Self::Busy => write!(f, "Another operation on this worktree is running"),
            Self::NotMergeable(why) => write!(f, "Not merged: {why}"),
            Self::Git(detail) => write!(f, "Git failed: {detail}"),
            Self::Storage(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for WorktreeError {}

impl From<AppError> for WorktreeError {
    fn from(error: AppError) -> Self {
        Self::Storage(error)
    }
}

impl From<&WorktreeError> for AppError {
    fn from(error: &WorktreeError) -> Self {
        let code = match error {
            WorktreeError::GitRepositoryRequired => ErrorCode::GitRepositoryRequired,
            WorktreeError::GitUnavailable => ErrorCode::GitUnavailable,
            WorktreeError::BaseUnavailable(_) => ErrorCode::WorktreeBaseUnavailable,
            WorktreeError::NotFound(_) => ErrorCode::WorktreeNotFound,
            WorktreeError::InvalidState(_) | WorktreeError::NotMergeable(_) => {
                ErrorCode::WorktreeInvalidState
            }
            WorktreeError::PolicyDenied => ErrorCode::MergeNotAllowed,
            WorktreeError::Busy => ErrorCode::WorktreeBusy,
            WorktreeError::Storage(inner) => return inner.clone(),
            WorktreeError::InvalidIdentifier(_)
            | WorktreeError::Collision(_)
            | WorktreeError::OutsideRoot(_)
            | WorktreeError::Git(_) => ErrorCode::WorktreeFailed,
        };
        AppError::new(code).with_detail(error.to_string())
    }
}

impl WorktreeError {
    /// How an execution that could not get its worktree is recorded.
    pub fn failure_kind(&self) -> FailureKind {
        match self {
            Self::GitRepositoryRequired => FailureKind::GitRepositoryRequired,
            _ => FailureKind::WorktreeFailed,
        }
    }
}

/// What Git says about the checkout a worktree will start from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseInfo {
    /// The root of the repository (the project may be a folder inside it).
    pub toplevel: PathBuf,
    /// The project's place inside the repository, empty when the project is the root. The agent
    /// starts in the same place of its worktree.
    pub subdir: PathBuf,
    pub branch: String,
    pub commit: String,
    /// Uncommitted changes in the checkout. They are not in the worktree.
    pub dirty: bool,
}

/// Where and what to create. Every field is derived by Atlas ([`WorktreeLayout`], the base from
/// Git): none of it comes from the webview.
#[derive(Debug, Clone)]
pub struct NewWorktree<'a> {
    pub toplevel: &'a Path,
    pub path: &'a Path,
    pub branch: &'a str,
    pub base_commit: &'a str,
}

/// What a trial merge of the execution branch into the base found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Assessment {
    pub files: Vec<String>,
    pub files_changed: u32,
    pub commits_ahead: u32,
    pub commits_behind: u32,
    pub conflicts: Vec<String>,
    /// `None`: Git could not say.
    pub mergeable: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome {
    Merged {
        commit: String,
    },
    /// Git stopped on conflicts and the attempt was undone; branch and worktree are untouched.
    Conflict {
        files: Vec<String>,
    },
}

/// Port: everything Atlas does with Git for worktrees. Implemented in `infrastructure/`. Every
/// method takes values Atlas derived (see [`WorktreeLayout`]); implementations still check
/// them, and never go through a shell. Nothing here discards work: no reset, clean or stash,
/// and no forced removal.
pub trait WorktreeManager: Send + Sync {
    fn is_git_repository(&self, project: &Path) -> bool;

    /// # Errors
    ///
    /// [`WorktreeError::GitRepositoryRequired`] outside a repository;
    /// [`WorktreeError::BaseUnavailable`] on a detached HEAD or an empty repository.
    fn inspect_base(&self, project: &Path) -> Result<BaseInfo, WorktreeError>;

    /// Creates the branch and the worktree. Fails, touching nothing, if either exists.
    ///
    /// # Errors
    ///
    /// [`WorktreeError::Collision`] if the branch or the folder exists.
    fn create(&self, new: &NewWorktree<'_>) -> Result<(), WorktreeError>;

    /// The branch checked out in a worktree (or checkout).
    ///
    /// # Errors
    ///
    /// Fails if it is not on a branch.
    fn get_branch(&self, path: &Path) -> Result<String, WorktreeError>;

    /// The files with uncommitted changes in a worktree.
    ///
    /// # Errors
    ///
    /// Fails if Git cannot read it.
    fn get_status(&self, path: &Path) -> Result<Vec<String>, WorktreeError>;

    /// The unified diff of the execution branch against where it forked from the base, cut at
    /// `max_bytes`. Read again from Git whenever it is needed; never stored.
    ///
    /// # Errors
    ///
    /// Fails if Git cannot produce it.
    #[allow(dead_code)] // for the diff view of a later milestone; tested
    fn get_diff(
        &self,
        toplevel: &Path,
        base_branch: &str,
        branch: &str,
        max_bytes: usize,
    ) -> Result<String, WorktreeError>;

    /// Commits everything in the worktree to its own branch (hooks off), when there is
    /// something. Returns the commit, or `None` when nothing changed.
    ///
    /// # Errors
    ///
    /// Fails if Git cannot commit.
    fn commit_all(&self, path: &Path, message: &str) -> Result<Option<String>, WorktreeError>;

    /// Measures the branch against the base and trial-merges it without touching any checkout.
    ///
    /// # Errors
    ///
    /// Fails if Git cannot compare them.
    fn assess(
        &self,
        toplevel: &Path,
        base_branch: &str,
        branch: &str,
    ) -> Result<Assessment, WorktreeError>;

    /// Whether the checkout that would receive a merge is on `base_branch` and has no
    /// uncommitted changes.
    ///
    /// # Errors
    ///
    /// Fails if Git cannot read the checkout.
    fn base_ready(
        &self,
        toplevel: &Path,
        base_branch: &str,
    ) -> Result<BaseReadiness, WorktreeError>;

    /// Merges the branch into the base checkout with a merge commit. On conflicts the attempt is
    /// undone and reported; nothing is deleted.
    ///
    /// # Errors
    ///
    /// Fails if the checkout is not on the base branch or not clean, or Git fails.
    fn merge(
        &self,
        toplevel: &Path,
        base_branch: &str,
        branch: &str,
        message: &str,
    ) -> Result<MergeOutcome, WorktreeError>;

    /// Removes the worktree folder (never forced: a worktree with untracked files is kept).
    ///
    /// # Errors
    ///
    /// Fails if Git refuses, or the path is not one Atlas created.
    fn remove(&self, toplevel: &Path, path: &Path) -> Result<(), WorktreeError>;

    /// Deletes the branch if (and only if) it is fully merged.
    ///
    /// # Errors
    ///
    /// Fails if the branch is not merged or Git refuses.
    fn delete_merged_branch(&self, toplevel: &Path, branch: &str) -> Result<(), WorktreeError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseReadiness {
    Ready,
    Dirty,
    BranchChanged,
}

#[cfg(test)]
pub mod tests;
