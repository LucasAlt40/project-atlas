//! Memory (ADR 0037): thin adapters between the Memory screen and `application::memory`. A
//! command names a memory and what the person typed; the id, the times, the counters and the
//! origin are Atlas's. Nothing here lets a memory be written as something the person did not
//! decide: a memory made here is the person's.

use serde::Serialize;
use tauri::State;

use crate::application::config::AppSettings;
use crate::application::errors::{AppError, ErrorCode};
use crate::application::memory::NewMemory;
use crate::application::support::now_ms;
use crate::domain::memory::{
    Memory, MemoryKind, MemoryOrigin, MemoryScope, MemorySettings, MemoryStatus, MemoryValidity,
};
use crate::state::AppState;

/// Every memory, newest last. The person sees all of them, including what is stale or archived.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn list_memories(state: State<'_, AppState>) -> Vec<Memory> {
    state.memory.list()
}

/// Creates a memory the person wrote: active at once, and theirs.
#[allow(clippy::needless_pass_by_value, clippy::too_many_arguments)]
#[tauri::command(async)]
pub fn create_memory(
    state: State<'_, AppState>,
    scope: MemoryScope,
    owner: Option<String>,
    kind: MemoryKind,
    title: String,
    content: String,
    topic: Option<String>,
    validity: Option<MemoryValidity>,
) -> Result<Memory, AppError> {
    Ok(state.memory.create(
        NewMemory {
            scope,
            owner,
            kind,
            title,
            content,
            topic,
            validity: validity.unwrap_or_default(),
            origin: MemoryOrigin::User,
        },
        now_ms(),
    )?)
}

/// Rewrites a memory. What an agent wrote becomes the person's.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn edit_memory(
    state: State<'_, AppState>,
    id: String,
    title: String,
    content: String,
    topic: Option<String>,
) -> Result<Memory, AppError> {
    Ok(state.memory.edit(&id, &title, &content, topic, now_ms())?)
}

/// Accepts a candidate an agent proposed.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn approve_memory(state: State<'_, AppState>, id: String) -> Result<Memory, AppError> {
    Ok(state.memory.approve(&id, now_ms())?)
}

/// Moves a memory between active, stale and archived. A candidate is decided on with
/// `approve_memory` or `delete_memory`, never moved here.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn set_memory_status(
    state: State<'_, AppState>,
    id: String,
    status: MemoryStatus,
) -> Result<Memory, AppError> {
    if status == MemoryStatus::Candidate {
        return Err(AppError::new(ErrorCode::MemoryInvalid).with("reason", "bad_status"));
    }
    Ok(state.memory.set_status(&id, status, now_ms())?)
}

/// Makes an automatic note the person's own: it stops expiring and counting as automatic.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn promote_memory(state: State<'_, AppState>, id: String) -> Result<Memory, AppError> {
    Ok(state.memory.promote(&id, now_ms())?)
}

/// Deletes every automatic note nobody took as theirs; returns how many.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn delete_automatic_memories(state: State<'_, AppState>) -> Result<usize, AppError> {
    Ok(state.memory.delete_automatic()?)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn delete_memory(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    Ok(state.memory.delete(&id)?)
}

/// The memory server on this machine that Atlas may read notes from (ADR 0040). Off by default;
/// only a loopback address is accepted.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn set_external_memory(
    state: State<'_, AppState>,
    external: crate::domain::memory::ExternalMemorySettings,
) -> Result<AppSettings, AppError> {
    state.settings.set_external_memory(external)
}

/// What asking the memory server on this machine came to: it answered, or why it did not.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalCheckDto {
    pub reachable: bool,
    /// A stable code the screen words in the person's language.
    pub reason: Option<String>,
    pub detail: Option<String>,
}

/// What a sync of the approved notes did, or why it could not run.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalSyncDto {
    pub ok: bool,
    pub reason: Option<String>,
    pub detail: Option<String>,
    pub published: usize,
    pub retired: usize,
    pub failed: usize,
}

/// Whether the memory server answers with Atlas's token, using the saved settings.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn check_external_memory(state: State<'_, AppState>) -> ExternalCheckDto {
    match state.external_memory.check() {
        Ok(()) => ExternalCheckDto {
            reachable: true,
            reason: None,
            detail: None,
        },
        Err(why) => ExternalCheckDto {
            reachable: false,
            reason: Some(why.code().to_owned()),
            detail: why.detail(),
        },
    }
}

/// Publishes the notes a person approved to the memory server, and retires the ones out of use.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn sync_external_memory(state: State<'_, AppState>) -> ExternalSyncDto {
    match state.external_memory.sync() {
        Ok(report) => ExternalSyncDto {
            ok: true,
            reason: None,
            detail: None,
            published: report.published,
            retired: report.retired,
            failed: report.failed,
        },
        Err(why) => ExternalSyncDto {
            ok: false,
            reason: Some(why.code().to_owned()),
            detail: why.detail(),
            published: 0,
            retired: 0,
            failed: 0,
        },
    }
}

/// Which scopes of the Memory are on, and the most a prompt may spend on them.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn set_memory_settings(
    state: State<'_, AppState>,
    memory: MemorySettings,
) -> Result<AppSettings, AppError> {
    state.settings.set_memory_settings(memory)
}
