use tauri::State;

use crate::application::errors::AppError;
use crate::application::workspace::WorkspaceInput;
use crate::domain::project::ProjectContext;
use crate::domain::workspace::Workspace;
use crate::state::AppState;

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn list_workspaces(state: State<'_, AppState>) -> Vec<Workspace> {
    state.workspaces.list()
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn create_workspace(
    state: State<'_, AppState>,
    input: WorkspaceInput,
) -> Result<Workspace, AppError> {
    state.workspaces.create(&input)
}

/// Renames a workspace and/or points it at another project folder.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn update_workspace(
    state: State<'_, AppState>,
    id: String,
    input: WorkspaceInput,
) -> Result<Workspace, AppError> {
    state.workspaces.update(&id, &input)
}

/// Deletes a workspace with its conversations and usage (agents are kept). Returns the
/// remaining workspaces.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn delete_workspace(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<Workspace>, AppError> {
    state.lifecycle.delete_workspace(&id)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn add_agent_to_workspace(
    state: State<'_, AppState>,
    workspace_id: String,
    agent_id: String,
) -> Result<Workspace, AppError> {
    state.workspaces.add_agent(&workspace_id, &agent_id)
}

/// Takes the agent out of the workspace; the agent itself is kept.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn remove_agent_from_workspace(
    state: State<'_, AppState>,
    workspace_id: String,
    agent_id: String,
) -> Result<Workspace, AppError> {
    state.workspaces.remove_agent(&workspace_id, &agent_id)
}

/// What agents are told about the workspace's project (folder name, path, and the technologies
/// recognised from marker files in the folder).
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn get_project_context(
    state: State<'_, AppState>,
    workspace_id: String,
) -> Result<ProjectContext, AppError> {
    state.workspaces.project_context(&workspace_id)
}
