use tauri::State;

use crate::application::errors::AppError;

use crate::application::agents::CreateAgentRequest;
use crate::domain::agent::Agent;
use crate::domain::workspace::Workspace;
use crate::state::AppState;

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn list_agents(state: State<'_, AppState>) -> Vec<Agent> {
    state.agents.list()
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn create_agent(
    state: State<'_, AppState>,
    request: CreateAgentRequest,
) -> Result<Agent, AppError> {
    state.agents.create(request)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn update_agent(
    state: State<'_, AppState>,
    id: String,
    request: CreateAgentRequest,
) -> Result<Agent, AppError> {
    state.agents.update(&id, request)
}

/// Deletes an agent together with its workspace positions and conversations. Returns the
/// updated workspaces.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn delete_agent(state: State<'_, AppState>, id: String) -> Result<Vec<Workspace>, AppError> {
    state.lifecycle.delete_agent(&id)
}
