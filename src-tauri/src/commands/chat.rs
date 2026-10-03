use tauri::{AppHandle, Runtime, State};

use super::events::TauriEventObserver;
use crate::application::chat::{SendMessageRequest, SentMessage};
use crate::application::errors::AppError;
use crate::domain::conversation::Message;
use crate::domain::execution::StoredExecution;
use crate::state::AppState;

/// Sends a message to an agent in a workspace. Returns at once with the recorded user message
/// and the id of the execution it started; the agent works in the background, reporting
/// through the `execution:progress` and `conversation:message` events.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn send_message<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    request: SendMessageRequest,
) -> Result<SentMessage, AppError> {
    let (sent, pending) = state
        .chat
        .send(request)
        .map_err(|error| AppError::from(&error))?;
    // The run is not awaited: its outcome reaches the UI through events.
    tauri::async_runtime::spawn_blocking(move || pending.run(&TauriEventObserver { app }));
    Ok(sent)
}

/// Messages of every workspace and agent, or narrowed to one workspace and/or agent.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn list_messages(
    state: State<'_, AppState>,
    workspace_id: Option<String>,
    agent_id: Option<String>,
) -> Vec<Message> {
    state
        .chat
        .messages(workspace_id.as_deref(), agent_id.as_deref())
}

/// Executions that have ended, newest first, of every workspace and agent or narrowed to one
/// workspace and/or agent. The one still running is known from the events, not from here.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn list_executions(
    state: State<'_, AppState>,
    workspace_id: Option<String>,
    agent_id: Option<String>,
) -> Vec<StoredExecution> {
    state
        .chat
        .executions(workspace_id.as_deref(), agent_id.as_deref())
}
