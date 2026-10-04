use tauri::State;

use crate::application::errors::AppError;
use crate::domain::harness::{
    HarnessSummary, InitializeInput, InitializeOutcome, ProjectAnalysis, RefreshOutcome,
    SemanticRequest,
};
use crate::domain::task_context::{TaskContextPreview, TaskContextRequest};
use crate::state::AppState;

/// Reads the workspace's project (structure and well-known manifests only; nothing is run) and
/// reports what it found. Writes nothing. With `semantic`, the model of that agent is shown the
/// selected evidence, which the result lists.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub async fn analyze_project(
    state: State<'_, AppState>,
    workspace_id: String,
    semantic: Option<SemanticRequest>,
) -> Result<ProjectAnalysis, AppError> {
    let harness = state.harness.clone();
    // An agent exploring a project can take minutes: never on the thread that serves the window.
    tauri::async_runtime::spawn_blocking(move || harness.analyze(&workspace_id, semantic.as_ref()))
        .await
        .map_err(|error| AppError::storage(error.to_string()))?
}

/// Creates or updates `.atlas/` from a fresh analysis and the user's review.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn initialize_project(
    state: State<'_, AppState>,
    workspace_id: String,
    input: InitializeInput,
) -> Result<InitializeOutcome, AppError> {
    state.harness.initialize(&workspace_id, input)
}

/// The state of the workspace's Harness: not initialized, initialized or needs review.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn get_project_harness(
    state: State<'_, AppState>,
    workspace_id: String,
) -> Result<HarnessSummary, AppError> {
    state.harness.get(&workspace_id)
}

/// Compares a new analysis with the existing Harness. Without `confirm` it only reports the
/// diff and conflicts; with it, the generated files are updated and the user's own are kept.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn refresh_project_harness(
    state: State<'_, AppState>,
    workspace_id: String,
    confirm: Option<bool>,
) -> Result<RefreshOutcome, AppError> {
    state
        .harness
        .refresh(&workspace_id, confirm.unwrap_or(false))
}

/// What an agent would be told for a task, chosen from the workspace's Harness, with the reason
/// for each choice. Inspection only: nothing is written and nothing runs.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn preview_task_context(
    state: State<'_, AppState>,
    request: TaskContextRequest,
) -> Result<TaskContextPreview, AppError> {
    state.harness.preview_task_context(&request)
}
