use tauri::State;

use crate::application::errors::AppError;
use crate::application::sessions::SessionTarget;
use crate::domain::terminal::TerminalSnapshot;
use crate::state::AppState;

/// Controls the process behind an execution. The webview names the execution and the workspace
/// and agent it believes it belongs to; the core answers only if all three match the session it
/// opened. Nothing here starts a process, so none of it can widen what an execution may do.
fn target(workspace_id: String, agent_id: String, execution_id: String) -> SessionTarget {
    SessionTarget {
        execution_id,
        workspace_id,
        agent_id,
    }
}

/// The terminal of an execution (live, or finished and still retained): its output so far and
/// its state. `None` if there is none for these ids. Output is ephemeral and bounded.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn get_execution_terminal(
    state: State<'_, AppState>,
    workspace_id: String,
    agent_id: String,
    execution_id: String,
) -> Option<TerminalSnapshot> {
    state
        .sessions
        .snapshot(&target(workspace_id, agent_id, execution_id))
        .ok()
}

/// Ctrl+C for the execution's process. The first thing to try.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn execution_interrupt(
    state: State<'_, AppState>,
    workspace_id: String,
    agent_id: String,
    execution_id: String,
) -> Result<(), AppError> {
    state
        .sessions
        .interrupt(&target(workspace_id, agent_id, execution_id))
        .map_err(|error| AppError::from(&error))
}

/// Ends the execution's process by force. For a process that ignored the interrupt.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn execution_terminate(
    state: State<'_, AppState>,
    workspace_id: String,
    agent_id: String,
    execution_id: String,
) -> Result<(), AppError> {
    state
        .sessions
        .terminate(&target(workspace_id, agent_id, execution_id))
        .map_err(|error| AppError::from(&error))
}

/// Manual input for the process, only when its runtime reads input. Passed on untouched.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn execution_terminal_input(
    state: State<'_, AppState>,
    workspace_id: String,
    agent_id: String,
    execution_id: String,
    data: String,
) -> Result<(), AppError> {
    state
        .sessions
        .input(
            &target(workspace_id, agent_id, execution_id),
            data.as_bytes(),
        )
        .map_err(|error| AppError::from(&error))
}

/// The terminal view changed size.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn execution_terminal_resize(
    state: State<'_, AppState>,
    workspace_id: String,
    agent_id: String,
    execution_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), AppError> {
    state
        .sessions
        .resize(&target(workspace_id, agent_id, execution_id), cols, rows)
        .map_err(|error| AppError::from(&error))
}
