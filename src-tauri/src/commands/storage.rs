//! How much disk Atlas uses, and the person's cleanup of what executions leave behind. The
//! webview chooses only what to clean; every path comes from the core.

use tauri::State;

use crate::application::errors::{AppError, ErrorCode};
use crate::application::storage::{CleanupChoice, CleanupReport, StorageUsage};
use crate::state::AppState;

/// The size of Atlas's data folder by part, and what can be cleaned. Walking big worktrees takes
/// a moment: not on the thread that serves the window.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub async fn get_storage_usage(state: State<'_, AppState>) -> Result<StorageUsage, AppError> {
    let storage = state.storage.clone();
    super::off_window(ErrorCode::StorageFailed, move || storage.usage()).await
}

/// Cleans what the person chose (see `application::storage`).
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub async fn clean_storage(
    state: State<'_, AppState>,
    choice: CleanupChoice,
) -> Result<CleanupReport, AppError> {
    let storage = state.storage.clone();
    super::off_window(ErrorCode::StorageFailed, move || storage.clean(choice)).await?
}
