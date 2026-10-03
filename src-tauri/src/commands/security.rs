use tauri::State;

use crate::application::errors::{AppError, ErrorCode};
use crate::application::security::approvals::PendingApproval;
use crate::application::security::service::{AgentPermissions, WorkspaceSecurity};
use crate::domain::agent::Agent;
use crate::state::AppState;

/// What agents may do in a workspace (read-only view of the stored policy, narrowed by Atlas's
/// global maximum).
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn get_workspace_security(
    state: State<'_, AppState>,
    workspace_id: String,
) -> Result<WorkspaceSecurity, AppError> {
    state.security.workspace(&workspace_id)
}

/// An agent's permissions in a workspace: its profile, what that grants there, and what applies
/// once its runtime's own limits are counted.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn get_agent_permissions(
    state: State<'_, AppState>,
    workspace_id: String,
    agent_id: String,
) -> Result<AgentPermissions, AppError> {
    state.security.agent(&workspace_id, &agent_id)
}

/// Switches an agent to another permission profile. The workspace's policy still bounds it.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn set_agent_permission_profile(
    state: State<'_, AppState>,
    agent_id: String,
    profile_id: String,
) -> Result<Agent, AppError> {
    state.agents.set_permission_profile(&agent_id, &profile_id)
}

/// Commands waiting for the user's decision, optionally for one workspace.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn list_pending_approvals(
    state: State<'_, AppState>,
    workspace_id: Option<String>,
) -> Vec<PendingApproval> {
    state.approvals.pending(workspace_id.as_deref())
}

/// The user's explicit answer to an approval request ("Allow once" or "Deny"). This command is
/// the only way a waiting command is released, and only the webview's UI calls it: nothing a
/// model writes is ever routed here.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn resolve_approval(
    state: State<'_, AppState>,
    approval_id: String,
    approve: bool,
) -> Result<(), AppError> {
    state
        .approvals
        .resolve(&approval_id, approve)
        .map(|_| ())
        .map_err(|error| {
            AppError::new(ErrorCode::ApprovalNotFound).with_detail(format!("{error:?}"))
        })
}
