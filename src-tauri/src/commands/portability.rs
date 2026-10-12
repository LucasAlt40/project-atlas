//! Export and import of configurations (ADR 0032). The files are chosen in the operating system's
//! own dialogs, opened here by the core: the webview never names a path to read or write.

use serde::Serialize;
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_dialog::DialogExt;

use crate::application::errors::AppError;
use crate::application::portability::format::MAX_FILE_BYTES;
use crate::application::portability::{
    load, ExportTarget, ExportedFile, ImportPreview, ImportReport, ImportRequest,
};
use crate::infrastructure::config_file;
use crate::state::AppState;

/// A file the person picked, read.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PickedFile {
    pub file_name: String,
    pub text: String,
}

/// The chosen configurations (and, if asked, what they need) as the text of a file, with what the
/// person should know before sharing it. Writes nothing.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn export_configs(
    state: State<'_, AppState>,
    targets: Vec<ExportTarget>,
    include_dependencies: bool,
) -> Result<ExportedFile, AppError> {
    state.portability.export(&targets, include_dependencies)
}

/// Asks where to save an exported file and writes it. `None` when the person cancels. Only text
/// that is an Atlas file is written: the command cannot be used to write anything else.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn save_config_file<R: Runtime>(
    app: AppHandle<R>,
    file_name: String,
    text: String,
) -> Result<Option<String>, AppError> {
    load(&text)?;
    let suggested = std::path::Path::new(&file_name)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("atlas-config.atlas.json")
        .to_owned();
    let Some(chosen) = app
        .dialog()
        .file()
        .set_file_name(suggested)
        .add_filter("Atlas", &["json"])
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = chosen.into_path().map_err(|error| {
        AppError::new(crate::application::errors::ErrorCode::ConfigFileIo)
            .with_detail(error.to_string())
    })?;
    config_file::write_atomic(&path, &text)?;
    Ok(Some(
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned(),
    ))
}

/// Asks which file to import and reads it (at most 2 MiB). `None` when the person cancels.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn pick_config_file<R: Runtime>(app: AppHandle<R>) -> Result<Option<PickedFile>, AppError> {
    let Some(chosen) = app
        .dialog()
        .file()
        .add_filter("Atlas", &["json"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = chosen.into_path().map_err(|error| {
        AppError::new(crate::application::errors::ErrorCode::ConfigFileIo)
            .with_detail(error.to_string())
    })?;
    let text = config_file::read_limited(&path, MAX_FILE_BYTES)?;
    Ok(Some(PickedFile {
        file_name: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned(),
        text,
    }))
}

/// What importing the file would do, changing nothing. Asks the runtimes which models they have,
/// which starts processes: it runs off the async executor.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn preview_config_import(
    state: State<'_, AppState>,
    text: String,
    workspace_id: Option<String>,
) -> Result<ImportPreview, AppError> {
    state.portability.preview(&text, workspace_id.as_deref())
}

/// Imports the file as decided in the preview: all of it, or nothing.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn import_configs(
    state: State<'_, AppState>,
    text: String,
    request: ImportRequest,
) -> Result<ImportReport, AppError> {
    state.portability.import(&text, &request)
}
