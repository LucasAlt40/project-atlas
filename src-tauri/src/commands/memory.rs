//! Memory (ADR 0037): thin adapters between the Memory screen and `application::memory`. A
//! command names a memory and what the person typed; the id, the times, the counters and the
//! origin are Atlas's. Nothing here lets a memory be written as something the person did not
//! decide: a memory made here is the person's.

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

/// Which scopes of the Memory are on, and the most a prompt may spend on them.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn set_memory_settings(
    state: State<'_, AppState>,
    memory: MemorySettings,
) -> Result<AppSettings, AppError> {
    state.settings.set_memory_settings(memory)
}
