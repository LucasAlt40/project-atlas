//! Discord Rich Presence settings. The webview sends the choices; what is shown is composed by the
//! core from its own events.

use tauri::State;

use crate::application::config::AppSettings;
use crate::application::errors::AppError;
use crate::domain::presence::{PresenceSettings, Screen};
use crate::state::AppState;

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn set_presence_settings(
    state: State<'_, AppState>,
    presence: PresenceSettings,
) -> Result<AppSettings, AppError> {
    let saved = state.settings.set_presence_settings(presence)?;
    state.presence.poke();
    Ok(saved)
}

/// The webview says which screen is open. Only the screen's name is kept: it decides a line of the
/// profile when nothing is running, and nothing else.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn set_presence_screen(state: State<'_, AppState>, screen: Screen) {
    state.presence.set_screen(screen);
}
