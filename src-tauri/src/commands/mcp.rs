use tauri::State;

use crate::application::errors::AppError;
use crate::application::mcp::{McpCatalog, McpOverview, McpPresetInfo};
use crate::domain::mcp::{McpConnection, McpGrant, McpTransport, Secret, ToolSelection};
use crate::state::AppState;

/// The workspace's MCP connections and who may use them. Nothing secret: a connection lists the
/// names of its secrets and when each was stored, never a value.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn list_mcp_connections(state: State<'_, AppState>, workspace_id: String) -> McpOverview {
    state.mcp.overview(&workspace_id)
}

/// Adds a connection (switched off, granted to nobody). The transport names an executable and its
/// arguments; a shell is refused.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn add_mcp_connection(
    state: State<'_, AppState>,
    workspace_id: String,
    name: String,
    transport: McpTransport,
    required: bool,
) -> Result<McpConnection, AppError> {
    state.mcp.add(&workspace_id, &name, transport, required)
}

/// Changes how a connection starts. It is switched off again and what was learned about it is
/// forgotten.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn update_mcp_connection(
    state: State<'_, AppState>,
    connection_id: String,
    transport: McpTransport,
    required: bool,
) -> Result<McpConnection, AppError> {
    state.mcp.update(&connection_id, transport, required)
}

/// Switches a connection on or off. On is the user's decision to let Atlas start this local
/// process; it grants no agent anything.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn set_mcp_connection_enabled(
    state: State<'_, AppState>,
    connection_id: String,
    enabled: bool,
) -> Result<McpConnection, AppError> {
    state.mcp.set_enabled(&connection_id, enabled)
}

/// Removes a connection with its grants and its stored secrets.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn remove_mcp_connection(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<(), AppError> {
    state.mcp.remove(&connection_id)
}

/// Stores one secret of a connection in the OS credential store. The value is never returned,
/// logged or saved with the configuration.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn set_mcp_secret(
    state: State<'_, AppState>,
    connection_id: String,
    name: String,
    value: String,
) -> Result<McpConnection, AppError> {
    state
        .mcp
        .set_secret(&connection_id, &name, &Secret::new(value))
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn clear_mcp_secret(
    state: State<'_, AppState>,
    connection_id: String,
    name: String,
) -> Result<McpConnection, AppError> {
    state.mcp.clear_secret(&connection_id, &name)
}

/// Starts the server through the runtime only to see what it reports (its status and the names of
/// its tools); no model is asked anything. Keeps the result as the connection's discovery.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub async fn probe_mcp_connection(
    state: State<'_, AppState>,
    connection_id: String,
    runtime_id: String,
) -> Result<McpConnection, AppError> {
    let mcp = state.mcp.clone();
    // Starting a server can take as long as its runtime waits for it: never on the window's thread.
    tauri::async_runtime::spawn_blocking(move || mcp.probe(&connection_id, &runtime_id))
        .await
        .map_err(|error| AppError::storage(error.to_string()))?
}

/// Lets an agent (optionally in one workflow, optionally in one of its steps) use a connection:
/// all of its tools, or the ones named.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn grant_mcp_connection(
    state: State<'_, AppState>,
    connection_id: String,
    agent_id: Option<String>,
    workflow_id: Option<String>,
    node_id: Option<String>,
    tools: ToolSelection,
) -> Result<McpGrant, AppError> {
    state.mcp.grant(
        &connection_id,
        agent_id.as_deref(),
        workflow_id.as_deref(),
        node_id.as_deref(),
        tools,
    )
}

/// Takes a grant back; it applies from the next step that starts.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn revoke_mcp_grant(state: State<'_, AppState>, grant_id: String) -> Result<(), AppError> {
    state.mcp.revoke(&grant_id)
}

/// The integrations Atlas knows by name, with what each needs and risks. Nothing is started to
/// answer: a requirement is looked for, not run.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn list_mcp_catalog(state: State<'_, AppState>) -> Vec<McpPresetInfo> {
    state.mcp_catalog.list()
}

/// Adds a catalogue entry as an ordinary connection: switched off, granted to nobody, not
/// started. It becomes real only when the user switches it on and grants it.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn add_mcp_preset(
    state: State<'_, AppState>,
    workspace_id: String,
    preset_id: String,
) -> Result<McpConnection, AppError> {
    let (name, transport) = McpCatalog::connection_for(&preset_id)?;
    state.mcp.add(&workspace_id, name, transport, false)
}
