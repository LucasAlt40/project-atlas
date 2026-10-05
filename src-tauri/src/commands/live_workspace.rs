use tauri::State;

use crate::application::errors::AppError;
use crate::domain::live_workspace::{LiveFile, LiveWorkspaceState};
use crate::state::AppState;

/// The live state of a workflow run's worktree: the files that differ from the commit the run
/// started from, as they are now. The run is named; the folder is the one Atlas stored for it.
/// `None` when the run has no code worktree. Reads only.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub async fn get_live_workspace(
    state: State<'_, AppState>,
    execution_id: String,
) -> Result<Option<LiveWorkspaceState>, AppError> {
    let live = state.live.clone();
    // Git can take a while on a big worktree: not on the thread that serves the window.
    tauri::async_runtime::spawn_blocking(move || live.state_for_run(&execution_id))
        .await
        .map_err(|e| {
            AppError::new(crate::application::errors::ErrorCode::WorktreeFailed)
                .with_detail(e.to_string())
        })
}

/// Compares the whole worktree with the baseline now. For "refresh", or when the UI notices an
/// update it cannot apply.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub async fn refresh_live_workspace(
    state: State<'_, AppState>,
    execution_id: String,
) -> Result<LiveWorkspaceState, AppError> {
    let live = state.live.clone();
    tauri::async_runtime::spawn_blocking(move || live.refresh_for_run(&execution_id))
        .await
        .map_err(|e| {
            AppError::new(crate::application::errors::ErrorCode::WorktreeFailed)
                .with_detail(e.to_string())
        })?
        .map_err(AppError::from)
}

/// One file of the run's worktree as it is now. The webview names the run and a path inside the
/// worktree; the folder is the one Atlas stored for the run, and a path that is not a plain
/// relative one is refused.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub async fn get_live_file(
    state: State<'_, AppState>,
    execution_id: String,
    path: String,
) -> Result<LiveFile, AppError> {
    let live = state.live.clone();
    tauri::async_runtime::spawn_blocking(move || live.file_for_run(&execution_id, &path))
        .await
        .map_err(|e| {
            AppError::new(crate::application::errors::ErrorCode::WorktreeFailed)
                .with_detail(e.to_string())
        })?
        .map_err(AppError::from)
}

/// The real diff of the run's worktree as it is now against the commit the run started from, of
/// one file when `path` is given.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub async fn get_live_diff(
    state: State<'_, AppState>,
    execution_id: String,
    path: Option<String>,
) -> Result<String, AppError> {
    let live = state.live.clone();
    tauri::async_runtime::spawn_blocking(move || live.diff_for_run(&execution_id, path.as_deref()))
        .await
        .map_err(|e| {
            AppError::new(crate::application::errors::ErrorCode::WorktreeFailed)
                .with_detail(e.to_string())
        })?
        .map_err(AppError::from)
}
