//! The Git facts Atlas keeps about an execution that ran in an isolated worktree. Only
//! metadata: the diff, the terminal output and the file contents are always re-read from Git.

use serde::{Deserialize, Serialize};

/// Where the worktree is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorktreeStatus {
    /// Being created; no runtime has started yet.
    Creating,
    /// The execution is running in it.
    Active,
    /// The execution completed; the worktree is kept until its work is merged or discarded.
    Completed,
    /// The execution failed, was cancelled or was cut short. Always kept: the work is the
    /// user's to recover.
    Failed,
    /// The work was merged but the worktree could not be removed yet.
    CleanupPending,
    /// Merged and removed.
    Cleaned,
}

/// What became of the work of the execution with respect to the base branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeStatus {
    /// Nothing decided yet (running, or ended without a merge being considered).
    NotEvaluated,
    /// The execution changed nothing: there is nothing to merge.
    NothingToMerge,
    /// Mergeable and waiting for the user: the policy asks for approval, or the user is meant
    /// to review first.
    Pending,
    Merged,
    /// A workflow's changes were brought into the project's working tree as *uncommitted*
    /// changes. Nothing was committed, merged or pushed; the branch is kept as the record.
    Applied,
    /// Git found conflicts; branch and worktree are kept.
    Conflict,
    /// Not merged on purpose (policy, failed execution, failed validation, unclean base…);
    /// see [`ExecutionWorktree::block_reason`].
    Blocked,
}

/// Outcome of whatever validated the changes (tests…). Atlas does not run tests itself yet, so
/// today every execution is `NotRun`; the gate exists so that a failure can never be merged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Validation {
    NotRun,
    Passed,
    Failed,
}

/// Why a merge was not done (or not attempted). Wire names are stable: the UI words them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockReason {
    /// The agent's policy does not allow Git writes.
    PolicyDenied,
    /// The execution did not complete (failed, cancelled, interrupted).
    ExecutionNotCompleted,
    ValidationFailed,
    /// The checkout that would receive the merge has uncommitted changes.
    BaseDirty,
    /// The checkout is no longer on the base branch.
    BaseBranchChanged,
    /// Git could not tell (too old, an error); nothing was attempted.
    Undetermined,
    /// The worktree has changes Atlas could not commit.
    UncommittedChanges,
    /// The worktree is not what Atlas made: another folder, or its branch was switched.
    WorktreeInconsistent,
    Conflict,
    /// The `ChangeSet` touches what must never enter the project (Git's files, paths that leave it).
    ProtectedPaths,
    /// The `ChangeSet` holds something a person must look at first (a secret, a key file, Atlas's
    /// own folder); applying again, having seen it, is the person's decision.
    NeedsReview,
}

/// The Git side of one execution. Created before the runtime starts and kept after it ends, so
/// the work can always be found again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionWorktree {
    pub execution_id: String,
    pub workspace_id: String,
    pub agent_id: String,
    /// The branch the execution started from (the one checked out in the project then).
    pub base_branch: String,
    /// The commit of `base_branch` the worktree started at.
    pub base_commit: String,
    /// The execution's own branch: `atlas/exec-000042`.
    pub branch_name: String,
    pub worktree_path: String,
    /// Where the runtime works: the worktree itself, or the project's folder inside it when
    /// the project is a subfolder of the repository.
    pub working_dir: String,
    /// The repository root of the project's checkout, where merges happen.
    pub repository_path: String,
    pub status: WorktreeStatus,
    pub merge_status: MergeStatus,
    pub block_reason: Option<BlockReason>,
    /// The project checkout had uncommitted changes when the worktree was created. The agent
    /// does not see them.
    #[serde(default)]
    pub base_dirty_at_start: bool,
    /// Milliseconds since the Unix epoch.
    pub created_at: u64,
    #[serde(default)]
    pub changes: Option<WorktreeChanges>,
    #[serde(default = "default_validation")]
    pub validation: Validation,
    /// What Atlas recommends doing next, as a stable code the UI words.
    #[serde(default)]
    pub recommendation: Option<Recommendation>,
    /// The workflow run this worktree belongs to, when it is a workflow's. A workflow has one
    /// worktree for all its steps (the *primary*); each step also has a record of its own
    /// (a *lease*, see `shared_with`) so that the security layer can tell who is working in it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_execution_id: Option<String>,
    /// For a step's lease: the execution id of the workflow's primary worktree. The lease has
    /// the same folder and branch and is never merged or removed on its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared_with: Option<String>,
    /// For a lease: the commit the worktree was at when the step finished.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_commit: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
}

/// One file changed, as Git reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,
    /// For a rename: where it was.
    #[serde(default)]
    pub old_path: Option<String>,
    pub status: FileChangeStatus,
    /// `None` for a binary file.
    #[serde(default)]
    pub additions: Option<u32>,
    #[serde(default)]
    pub deletions: Option<u32>,
    #[serde(default)]
    pub binary: bool,
}

/// What a worktree holds compared with where it started, measured by Git and never by an agent.
/// The diff text is not kept: it is read from Git when asked for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeSet {
    /// The commit the worktree started from.
    pub base_revision: String,
    /// The commit it is at now.
    pub current_revision: String,
    /// At most [`MAX_CHANGESET_FILES`].
    pub files: Vec<FileChange>,
    pub files_changed: u32,
    pub additions: u32,
    pub deletions: u32,
    /// Files with changes that are not in a commit yet (Git writes denied, or work in progress).
    pub uncommitted: Vec<String>,
    /// Milliseconds since the Unix epoch.
    pub captured_at: u64,
}

pub const MAX_CHANGESET_FILES: usize = 500;

impl ChangeSet {
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.files_changed == 0 && self.uncommitted.is_empty()
    }
}

fn default_validation() -> Validation {
    Validation::NotRun
}

/// A summary of the work in the worktree, measured by Git (never by the model).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeChanges {
    pub files_changed: u32,
    /// Commits on the execution branch that the base does not have.
    pub commits_ahead: u32,
    /// Commits on the base branch that the execution branch does not have.
    pub commits_behind: u32,
    /// Files changed (relative paths), at most [`MAX_LISTED_FILES`].
    pub files: Vec<String>,
    /// Files Git reported as conflicting in a trial merge.
    pub conflicts: Vec<String>,
    /// `Some(true)` when a trial merge is clean, `Some(false)` when it conflicts, `None` when
    /// Git could not say.
    pub mergeable: Option<bool>,
}

pub const MAX_LISTED_FILES: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Recommendation {
    /// Merge it: clean, nothing against it.
    Merge,
    /// Look at the changes first (diverged base, no validation, approval needed).
    Review,
    ResolveConflicts,
    /// The execution did not finish well; recover or discard the work by hand.
    Inspect,
    /// Nothing changed.
    Discard,
}
