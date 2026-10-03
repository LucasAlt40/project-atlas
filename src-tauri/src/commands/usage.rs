use tauri::State;

use crate::application::errors::AppError;
use crate::domain::usage::{AgentUsageSummary, UsageWindows, WorkspaceUsageSummary};
use crate::state::AppState;

/// What Atlas observed one agent consume in one workspace. `windows` says where "today", "this
/// week" and "this month" start in the user's time zone.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn get_agent_usage(
    state: State<'_, AppState>,
    workspace_id: String,
    agent_id: String,
    windows: UsageWindows,
) -> Result<AgentUsageSummary, AppError> {
    state.usage.agent_summary(&workspace_id, &agent_id, windows)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn get_workspace_usage(
    state: State<'_, AppState>,
    workspace_id: String,
    windows: UsageWindows,
) -> WorkspaceUsageSummary {
    state.usage.workspace_summary(&workspace_id, windows)
}
