use tauri::{AppHandle, Emitter, Runtime, State};

use crate::application::errors::AppError;
use crate::application::harness::workspace_context::{
    WorkspaceContextInfo, WorkspaceHarnessOverview, WorkspaceInitReport,
};
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
    project_id: Option<String>,
    semantic: Option<SemanticRequest>,
) -> Result<ProjectAnalysis, AppError> {
    let harness = state.harness.clone();
    // An agent exploring a project can take minutes: never on the thread that serves the window.
    tauri::async_runtime::spawn_blocking(move || {
        harness.analyze_in(&workspace_id, project_id.as_deref(), semantic.as_ref())
    })
    .await
    .map_err(|error| AppError::storage(error.to_string()))?
}

/// Creates or updates `.atlas/` from a fresh analysis and the user's review.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn initialize_project(
    state: State<'_, AppState>,
    workspace_id: String,
    project_id: Option<String>,
    input: InitializeInput,
) -> Result<InitializeOutcome, AppError> {
    state
        .harness
        .initialize_in(&workspace_id, project_id.as_deref(), input)
}

/// The state of the workspace's Harness: not initialized, initialized or needs review.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn get_project_harness(
    state: State<'_, AppState>,
    workspace_id: String,
    project_id: Option<String>,
) -> Result<HarnessSummary, AppError> {
    state.harness.get_in(&workspace_id, project_id.as_deref())
}

/// Wipes the Harness of one project so it can be generated again. Nothing else of the project
/// is touched.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn reset_project_harness(
    state: State<'_, AppState>,
    workspace_id: String,
    project_id: Option<String>,
) -> Result<HarnessSummary, AppError> {
    state.harness.reset_in(&workspace_id, project_id.as_deref())
}

/// Wipes the Harness of the workspace's projects (or of the ones named) and rebuilds the
/// workspace context. The workspace notes stay.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn reset_workspace_harness(
    state: State<'_, AppState>,
    workspace_id: String,
    project_ids: Option<Vec<String>>,
) -> Result<WorkspaceContextInfo, AppError> {
    state
        .harness
        .reset_workspace(&workspace_id, project_ids.as_deref())
}

/// Compares a new analysis with the existing Harness. Without `confirm` it only reports the
/// diff and conflicts; with it, the generated files are updated and the user's own are kept.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn refresh_project_harness(
    state: State<'_, AppState>,
    workspace_id: String,
    project_id: Option<String>,
    confirm: Option<bool>,
) -> Result<RefreshOutcome, AppError> {
    state.harness.refresh_in(
        &workspace_id,
        project_id.as_deref(),
        confirm.unwrap_or(false),
    )
}

/// What an agent would be told for a task, chosen from the workspace's Harness, with the reason
/// for each choice. Inspection only: nothing is written and nothing runs.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn preview_task_context(
    state: State<'_, AppState>,
    request: TaskContextRequest,
) -> Result<TaskContextPreview, AppError> {
    state.harness.preview_task_context(&request)
}

/// Where each project of the workspace stands (its Harness) and whether the workspace's own
/// context is current.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn get_workspace_harness(
    state: State<'_, AppState>,
    workspace_id: String,
) -> Result<WorkspaceHarnessOverview, AppError> {
    state.harness.workspace_overview(&workspace_id)
}

/// Starts the Harness of the workspace's projects (or of the ones named) and rebuilds the
/// workspace context. A project that already has a Harness is not analysed again; one that fails
/// does not stop the others. Progress per project arrives as `harness:workspace-progress`.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub async fn initialize_workspace_harness<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    workspace_id: String,
    project_ids: Option<Vec<String>>,
    ignore_in_git: Option<bool>,
) -> Result<WorkspaceInitReport, AppError> {
    let harness = state.harness.clone();
    // Reading many projects takes a while: never on the thread that serves the window.
    tauri::async_runtime::spawn_blocking(move || {
        harness.initialize_workspace(
            &workspace_id,
            project_ids.as_deref(),
            ignore_in_git.unwrap_or(true),
            &|update| {
                if let Err(error) = app.emit("harness:workspace-progress", update) {
                    eprintln!("failed to emit harness:workspace-progress: {error}");
                }
            },
        )
    })
    .await
    .map_err(|error| AppError::storage(error.to_string()))?
}

/// Rebuilds the generated part of the workspace's context from the projects' Harnesses. The
/// user's notes are untouched.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn build_workspace_context(
    state: State<'_, AppState>,
    workspace_id: String,
) -> Result<WorkspaceContextInfo, AppError> {
    state.harness.build_workspace_context(&workspace_id)
}

/// The user's own notes about the workspace as a whole.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn get_workspace_notes(state: State<'_, AppState>, workspace_id: String) -> String {
    state.harness.workspace_notes(&workspace_id)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn save_workspace_notes(
    state: State<'_, AppState>,
    workspace_id: String,
    notes: String,
) -> Result<(), AppError> {
    state.harness.save_workspace_notes(&workspace_id, &notes)
}
