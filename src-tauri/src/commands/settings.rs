use tauri::State;

use crate::application::config::AppSettings;
use crate::application::errors::AppError;
use crate::state::AppState;

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppSettings {
    state.settings.get()
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn set_specs_max_tokens(
    state: State<'_, AppState>,
    max: Option<u64>,
) -> Result<AppSettings, AppError> {
    state.settings.set_specs_max_tokens(max)
}

/// How long a question an agent asks in a workflow run waits for a person (`null`: no limit).
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn set_question_timeout(
    state: State<'_, AppState>,
    minutes: Option<u32>,
) -> Result<AppSettings, AppError> {
    state.settings.set_question_timeout(minutes)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn set_language(state: State<'_, AppState>, language: String) -> Result<AppSettings, AppError> {
    state.settings.set_language(&language)
}

/// Remembers which workspace is open (`null`: none).
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn select_workspace(
    state: State<'_, AppState>,
    workspace_id: Option<String>,
) -> Result<AppSettings, AppError> {
    let saved = state.settings.select_workspace(workspace_id.as_deref())?;
    state.presence.poke();
    Ok(saved)
}
