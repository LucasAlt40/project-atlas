use tauri::State;

use crate::application::errors::{AppError, ErrorCode};
use crate::domain::worktree::ExecutionWorktree;
use crate::state::AppState;

/// The Git worktrees of executions that ran isolated, oldest first, of every workspace and
/// agent or narrowed to one workspace and/or agent. Metadata only: the diff is read from Git
/// when someone needs it.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn list_execution_worktrees(
    state: State<'_, AppState>,
    workspace_id: Option<String>,
    agent_id: Option<String>,
) -> Vec<ExecutionWorktree> {
    state
        .worktrees
        .list(workspace_id.as_deref(), agent_id.as_deref())
}

/// The user's explicit "merge this execution's work into its base branch". Counts as the
/// approval a policy may ask for; it does not override a policy that denies Git writes, and a
/// merge Git cannot do cleanly is reported in the returned worktree (conflict, blocked), not
/// forced. The webview names the execution, workspace and agent; all three must match.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub async fn merge_execution(
    state: State<'_, AppState>,
    workspace_id: String,
    agent_id: String,
    execution_id: String,
) -> Result<ExecutionWorktree, AppError> {
    let worktrees = state.worktrees.clone();
    // Git can take a while on a big repository: not on the thread that serves the window.
    tauri::async_runtime::spawn_blocking(move || {
        let known = worktrees
            .get(&execution_id)
            .filter(|w| w.workspace_id == workspace_id && w.agent_id == agent_id)
            .ok_or_else(|| AppError::new(ErrorCode::WorktreeNotFound))?;
        worktrees
            .merge(&known.execution_id)
            .map_err(|error| AppError::from(&error))
    })
    .await
    .map_err(|error| AppError::new(ErrorCode::WorktreeFailed).with_detail(error.to_string()))?
}
