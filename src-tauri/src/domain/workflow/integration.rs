use serde::{Deserialize, Serialize};

use crate::domain::worktree::BlockReason;

/// Where the *code* of a workflow run stands, which is not the same as where the run stands: a
/// run can complete while its code is still only in an isolated worktree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationStatus {
    /// The run has no isolated worktree (its agents work in the checkout, or the project is
    /// not a Git repository): there is nothing to integrate.
    #[default]
    NotApplicable,
    /// The run is going and works in the shared worktree.
    InProgress,
    /// The run ended and changed nothing.
    NoChanges,
    /// The run changed the code, in the worktree. Nothing is in the project yet: the user decides.
    ChangesAvailable,
    /// Applying was tried and Git found conflicts. Nothing in the project changed.
    Conflicts,
    /// Applying is not possible now (see `block_reason`): the project has uncommitted changes,
    /// the policy denies it, or the run did not complete.
    Blocked,
    /// The changes are in the project.
    Integrated,
    /// The user chose to leave the changes in the worktree.
    KeptIsolated,
    /// The user discarded the worktree (its branch is kept).
    Discarded,
    /// Applying failed for a reason Git gave (see `message`).
    Failed,
}

/// The code side of a run: where it is, and what may be done with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowIntegration {
    pub status: IntegrationStatus,
    /// The execution id of the run's primary worktree.
    pub worktree_execution_id: Option<String>,
    pub branch: Option<String>,
    pub base_branch: Option<String>,
    pub base_revision: Option<String>,
    pub current_revision: Option<String>,
    /// Why applying is blocked, when it is.
    pub block_reason: Option<BlockReason>,
    /// The user may apply the changes now.
    pub can_apply: bool,
    /// Files Git reported in conflict.
    pub conflicts: Vec<String>,
    /// Why it failed, in the words Git gave.
    pub message: Option<String>,
    /// Where the project's `HEAD` was when the changes were applied to its working tree. The
    /// applied changes may be taken back out only while it is still there: once the project has a
    /// new commit, they may be part of it.
    #[serde(default)]
    pub applied_head: Option<String>,
    /// The changes are in the project's working tree, uncommitted, and may be taken back out.
    #[serde(default)]
    pub can_undo: bool,
    pub updated_at: u64,
}
