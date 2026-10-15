//! The OS sandbox of runtime processes (ADR 0038): what this system can do, and the person's
//! choice. Nothing here takes a path or a profile from the webview.

use serde::Serialize;
use tauri::State;

use crate::application::config::AppSettings;
use crate::application::errors::AppError;
use crate::domain::security::SandboxSettings;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxStatus {
    /// This system can confine a process (macOS with `sandbox-exec`).
    pub available: bool,
    pub platform: String,
    pub settings: SandboxSettings,
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn get_sandbox_status(state: State<'_, AppState>) -> SandboxStatus {
    SandboxStatus {
        available: state.sandbox.available(),
        platform: std::env::consts::OS.to_owned(),
        settings: state.sandbox.settings(),
    }
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn set_sandbox_settings(
    state: State<'_, AppState>,
    sandbox: SandboxSettings,
) -> Result<AppSettings, AppError> {
    state.settings.set_sandbox_settings(sandbox)
}
