//! What becomes of the *code* of a workflow run. A run can complete while its code is still only
//! in the run's isolated worktree: the two are kept apart here and in the model. The run's worktree
//! is never merged by itself. When the run ends it is measured (by Git); the user then reviews the
//! real diff and decides: apply it to the project, keep it isolated, discard it, or open it in an
//! editor. Applying goes through the existing worktree merge, which keeps every one of its
//! rules: the agent policy, a clean checkout on the base branch, no forced anything, conflicts
//! reported and undone.

use std::path::PathBuf;
use std::sync::Arc;

use super::runner::WorkspaceClose;
use super::service::WorkflowService;
use crate::application::errors::{AppError, ErrorCode};
use crate::application::ide::{Ide, IdeError, IdeLauncher};
use crate::application::support::now_ms;
use crate::application::workspace::WorkspaceService;
use crate::application::worktree::{WorktreeError, WorktreeService};
use crate::domain::workflow::{
    IntegrationStatus, WorkflowEventKind, WorkflowExecution, WorkflowIntegration,
};
use crate::domain::worktree::{
    BlockReason, ChangeSet, ExecutionWorktree, MergeStatus, WorktreeStatus,
};

/// The most diff text one request returns.
const MAX_DIFF_BYTES: usize = 400_000;

/// Where the code of a run stands, from what Git and the worktree service say of its worktree.
pub fn integration_from_worktree(
    previous: &WorkflowIntegration,
    worktree: &ExecutionWorktree,
    changes: Option<&ChangeSet>,
    at: u64,
) -> WorkflowIntegration {
    let mut next = previous.clone();
    next.updated_at = at;
    next.message = None;
    next.conflicts = Vec::new();
    next.block_reason = None;
    next.can_apply = false;
    if let Some(changes) = changes {
        next.base_revision = Some(changes.base_revision.clone());
        next.current_revision = Some(changes.current_revision.clone());
    }
    let completed = worktree.status == WorktreeStatus::Completed;
    match worktree.merge_status {
        MergeStatus::NothingToMerge => next.status = IntegrationStatus::NoChanges,
        MergeStatus::Merged => next.status = IntegrationStatus::Integrated,
        MergeStatus::Pending => {
            next.status = IntegrationStatus::ChangesAvailable;
            next.can_apply = completed;
        }
        MergeStatus::Conflict => {
            next.status = IntegrationStatus::Conflicts;
            next.block_reason = Some(BlockReason::Conflict);
            next.conflicts = worktree
                .changes
                .as_ref()
                .map(|c| c.conflicts.clone())
                .unwrap_or_default();
            next.can_apply = completed;
        }
        MergeStatus::Blocked => match worktree.block_reason {
            // The run did not complete: its work is there to look at, not to apply.
            Some(BlockReason::ExecutionNotCompleted) => {
                next.status = IntegrationStatus::ChangesAvailable;
                next.block_reason = Some(BlockReason::ExecutionNotCompleted);
            }
            reason => {
                next.status = IntegrationStatus::Blocked;
                next.block_reason = reason;
                // What can be put right and tried again: a dirty checkout, a branch changed.
                next.can_apply = completed
                    && matches!(
                        reason,
                        Some(
                            BlockReason::BaseDirty
                                | BlockReason::BaseBranchChanged
                                | BlockReason::Undetermined
                        )
                    );
            }
        },
        MergeStatus::NotEvaluated => next.status = IntegrationStatus::InProgress,
    }
    next
}

/// The same, when the run's worktree was just closed.
pub fn integration_from_close(
    previous: &WorkflowIntegration,
    close: &WorkspaceClose,
    at: u64,
) -> WorkflowIntegration {
    integration_from_worktree(previous, &close.worktree, close.changes.as_ref(), at)
}

pub struct IntegrationService {
    workflows: Arc<WorkflowService>,
    worktrees: Arc<WorktreeService>,
    workspaces: Arc<WorkspaceService>,
    ide: Arc<dyn IdeLauncher>,
}

fn invalid() -> AppError {
    AppError::new(ErrorCode::IntegrationNotAvailable)
}

impl IntegrationService {
    pub fn new(
        workflows: Arc<WorkflowService>,
        worktrees: Arc<WorktreeService>,
        workspaces: Arc<WorkspaceService>,
        ide: Arc<dyn IdeLauncher>,
    ) -> Self {
        Self {
            workflows,
            worktrees,
            workspaces,
            ide,
        }
    }

    /// The run, and its worktree's execution id; only for a run that has ended and has one.
    fn load(&self, run_id: &str) -> Result<(WorkflowExecution, String), AppError> {
        let exec = self
            .workflows
            .execution(run_id)
            .ok_or_else(|| AppError::new(ErrorCode::WorkflowExecutionNotFound))?;
        if !exec.status.is_final() {
            return Err(AppError::new(ErrorCode::WorkflowStateInvalid));
        }
        let primary = exec
            .integration
            .worktree_execution_id
            .clone()
            .ok_or_else(invalid)?;
        Ok((exec, primary))
    }

    fn save(
        &self,
        mut exec: WorkflowExecution,
        next: WorkflowIntegration,
        what: &str,
    ) -> Result<WorkflowExecution, AppError> {
        let at = now_ms();
        let status = serde_json::to_value(next.status)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        exec.integration = next;
        exec.record(
            WorkflowEventKind::IntegrationChanged,
            None,
            what,
            std::collections::BTreeMap::from([("status".to_owned(), status)]),
            at,
        );
        self.workflows.save_execution(&exec)?;
        Ok(exec)
    }

    /// What the run changed: read from Git while its worktree exists, the summary kept with the
    /// run afterwards.
    ///
    /// # Errors
    ///
    /// Fails if the run is unknown or has no code worktree.
    pub fn changes(&self, run_id: &str) -> Result<Option<ChangeSet>, AppError> {
        let exec = self
            .workflows
            .execution(run_id)
            .ok_or_else(|| AppError::new(ErrorCode::WorkflowExecutionNotFound))?;
        let Some(primary) = exec.integration.worktree_execution_id.as_deref() else {
            return Ok(None);
        };
        Ok(self
            .worktrees
            .change_set(primary)
            .ok()
            .or_else(|| exec.changes.clone()))
    }

    /// The real diff of the run's code (of one file when `file` is given), from Git. After the
    /// worktree is gone (the changes were applied) it is read from the project's repository.
    ///
    /// # Errors
    ///
    /// Fails if the run has no changes or Git cannot produce the diff.
    pub fn diff(&self, run_id: &str, file: Option<&str>) -> Result<String, AppError> {
        let exec = self
            .workflows
            .execution(run_id)
            .ok_or_else(|| AppError::new(ErrorCode::WorkflowExecutionNotFound))?;
        let primary = exec
            .integration
            .worktree_execution_id
            .clone()
            .ok_or_else(invalid)?;
        if let Ok(text) = self.worktrees.diff(&primary, file, MAX_DIFF_BYTES) {
            return Ok(text);
        }
        let changes = exec.changes.as_ref().ok_or_else(invalid)?;
        self.worktrees
            .diff_revisions(
                &primary,
                &changes.base_revision,
                &changes.current_revision,
                file,
                MAX_DIFF_BYTES,
            )
            .map_err(|e| AppError::from(&e))
    }

    /// The user's explicit "apply": the changes enter the project through the worktree merge, with
    /// all its checks. A merge that cannot happen is not an error: the run says why.
    ///
    /// # Errors
    ///
    /// Fails if the run has not ended, was not completed, or has nothing waiting to be applied.
    pub fn apply(&self, run_id: &str) -> Result<WorkflowExecution, AppError> {
        let (exec, primary) = self.load(run_id)?;
        let waiting = matches!(
            exec.integration.status,
            IntegrationStatus::ChangesAvailable
                | IntegrationStatus::Blocked
                | IntegrationStatus::Conflicts
                | IntegrationStatus::Failed
                | IntegrationStatus::KeptIsolated
        );
        if !waiting {
            return Err(invalid());
        }
        let at = now_ms();
        let result = self.worktrees.merge(&primary);
        let (next, what) = match result {
            Ok(worktree) => {
                let changes = exec.changes.clone();
                let next =
                    integration_from_worktree(&exec.integration, &worktree, changes.as_ref(), at);
                let what = if next.status == IntegrationStatus::Integrated {
                    "Changes applied to the project"
                } else {
                    "The changes could not be applied"
                };
                (next, what)
            }
            Err(WorktreeError::PolicyDenied) => {
                let mut next = exec.integration.clone();
                next.status = IntegrationStatus::Blocked;
                next.block_reason = Some(BlockReason::PolicyDenied);
                next.can_apply = false;
                next.updated_at = at;
                (next, "The policy does not allow applying the changes")
            }
            Err(WorktreeError::InvalidState(_) | WorktreeError::NotFound(_)) => {
                return Err(invalid());
            }
            Err(error) => {
                let mut next = exec.integration.clone();
                next.status = IntegrationStatus::Failed;
                next.message = Some(error.to_string());
                next.can_apply = true;
                next.updated_at = at;
                (next, "Applying the changes failed")
            }
        };
        self.save(exec, next, what)
    }

    /// The user's choice to leave the changes where they are.
    ///
    /// # Errors
    ///
    /// Fails if there are no changes waiting.
    pub fn keep(&self, run_id: &str) -> Result<WorkflowExecution, AppError> {
        let (exec, _) = self.load(run_id)?;
        if !matches!(
            exec.integration.status,
            IntegrationStatus::ChangesAvailable
                | IntegrationStatus::Blocked
                | IntegrationStatus::Conflicts
                | IntegrationStatus::Failed
        ) {
            return Err(invalid());
        }
        let mut next = exec.integration.clone();
        next.status = IntegrationStatus::KeptIsolated;
        next.updated_at = now_ms();
        self.save(exec, next, "Changes kept isolated")
    }

    /// The user's confirmed "discard": the worktree folder is removed. The branch is kept (Atlas
    /// never deletes commits that were not merged), so the work can still be recovered with Git.
    ///
    /// # Errors
    ///
    /// Fails if there is no worktree to discard, or Git refuses to remove it.
    pub fn discard(&self, run_id: &str) -> Result<WorkflowExecution, AppError> {
        let (exec, primary) = self.load(run_id)?;
        if !matches!(
            exec.integration.status,
            IntegrationStatus::ChangesAvailable
                | IntegrationStatus::Blocked
                | IntegrationStatus::Conflicts
                | IntegrationStatus::Failed
                | IntegrationStatus::KeptIsolated
        ) {
            return Err(invalid());
        }
        self.worktrees
            .discard_shared(&primary)
            .map_err(|e| AppError::from(&e))?;
        let mut next = exec.integration.clone();
        next.status = IntegrationStatus::Discarded;
        next.can_apply = false;
        next.updated_at = now_ms();
        self.save(exec, next, "Worktree discarded")
    }

    pub fn ides(&self) -> Vec<Ide> {
        self.ide.available()
    }

    /// The folder the editor opens: the run's worktree while the code is only there (never the
    /// project's checkout), the project once the changes were applied.
    fn folder(&self, exec: &WorkflowExecution) -> Option<PathBuf> {
        let primary = exec.integration.worktree_execution_id.as_deref()?;
        if let Some(folder) = self.worktrees.editor_folder(primary) {
            return Some(folder);
        }
        (exec.integration.status == IntegrationStatus::Integrated)
            .then(|| self.workspaces.get(&exec.workspace_id))
            .flatten()
            .map(|w| PathBuf::from(w.project_path))
    }

    /// Opens the run's code in an editor. The editor is one of a fixed list and the folder is
    /// derived from what Atlas stored: nothing the webview or an agent sends picks either.
    ///
    /// # Errors
    ///
    /// Fails if the run has no folder to open or the editor cannot be started.
    pub fn open_in_ide(&self, run_id: &str, ide_id: &str) -> Result<(), AppError> {
        let exec = self
            .workflows
            .execution(run_id)
            .ok_or_else(|| AppError::new(ErrorCode::WorkflowExecutionNotFound))?;
        let folder = self.folder(&exec).ok_or_else(invalid)?;
        self.ide.open(ide_id, &folder).map_err(|error| {
            let code = match &error {
                IdeError::Unknown(_) | IdeError::NotAFolder => ErrorCode::IdeUnknown,
                IdeError::NotInstalled(_) => ErrorCode::IdeNotInstalled,
                IdeError::LaunchFailed(_) => ErrorCode::IdeLaunchFailed,
            };
            AppError::new(code).with_detail(error.to_string())
        })
    }
}
