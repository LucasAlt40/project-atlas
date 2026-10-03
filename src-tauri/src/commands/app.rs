use tauri::State;

use crate::domain::app_info::AppInfo;
use crate::state::AppState;

// Tauri injects `State` by value; the signature cannot take a reference.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn get_app_info(state: State<'_, AppState>) -> AppInfo {
    state.app_info.get_app_info()
}
