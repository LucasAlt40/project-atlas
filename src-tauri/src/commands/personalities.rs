use tauri::State;

use crate::application::errors::AppError;

use crate::application::personalities::CreatePersonalityRequest;
use crate::domain::personality::PersonalityProfile;
use crate::state::AppState;

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn list_personalities(state: State<'_, AppState>) -> Vec<PersonalityProfile> {
    state.personalities.list()
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn create_personality(
    state: State<'_, AppState>,
    request: CreatePersonalityRequest,
) -> Result<PersonalityProfile, AppError> {
    state.personalities.create(&request)
}

/// Edits a personality. For a built-in this saves a changed copy next to it.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn update_personality(
    state: State<'_, AppState>,
    id: String,
    request: CreatePersonalityRequest,
) -> Result<PersonalityProfile, AppError> {
    state.personalities.update(&id, &request)
}

/// Removes a personality (a built-in is hidden). Refused while agents use it.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn delete_personality(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    state.personalities.delete(&id)
}

/// Brings back removed built-in personalities and undoes edits to them.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn restore_default_personalities(
    state: State<'_, AppState>,
) -> Result<Vec<PersonalityProfile>, AppError> {
    state.personalities.restore_defaults()
}
