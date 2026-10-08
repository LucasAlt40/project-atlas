//! Looking for and installing a new version of Atlas. The update is fetched from the project's
//! GitHub releases and its signature is verified against the public key in `tauri.conf.json`
//! before anything is installed.

use serde::Serialize;
use tauri::{AppHandle, Runtime};
use tauri_plugin_updater::UpdaterExt;

use crate::application::errors::{AppError, ErrorCode};

/// A newer version that can be installed.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub notes: Option<String>,
}

fn check_failed(error: &impl std::fmt::Display) -> AppError {
    AppError::new(ErrorCode::UpdateCheckFailed).with_detail(error.to_string())
}

/// `None` when this is already the newest version.
#[tauri::command]
pub async fn check_for_update<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Option<UpdateInfo>, AppError> {
    let updater = app.updater().map_err(|e| check_failed(&e))?;
    let update = updater.check().await.map_err(|e| check_failed(&e))?;
    Ok(update.map(|update| UpdateInfo {
        version: update.version,
        notes: update.body,
    }))
}

/// Downloads and installs the newest version, then asks the app to restart. The restart goes
/// through the normal exit path, so running agents are not dropped without asking.
#[tauri::command]
pub async fn install_update<R: Runtime>(app: AppHandle<R>) -> Result<(), AppError> {
    let install_failed = |e: &dyn std::fmt::Display| {
        AppError::new(ErrorCode::UpdateInstallFailed).with_detail(e.to_string())
    };
    let updater = app.updater().map_err(|e| check_failed(&e))?;
    let Some(update) = updater.check().await.map_err(|e| check_failed(&e))? else {
        return Ok(());
    };
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| install_failed(&e))?;
    app.request_restart();
    Ok(())
}
