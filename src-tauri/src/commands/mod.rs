//! Tauri command handlers: thin adapters between the webview and `application/`.
//! No business logic here. Failures are returned as [`AppError`] (a stable code plus
//! parameters); the UI words them in the user's language.

pub mod agents;
pub mod app;
pub mod chat;
pub mod events;
pub mod exit;
pub mod harness;
pub mod live_workspace;
pub mod mcp;
pub mod memory;
pub mod personalities;
pub mod portability;
pub mod presence;
pub mod remote;
pub mod runtimes;
pub mod sandbox;
pub mod security;
pub mod settings;
pub mod specs;
pub mod storage;
pub mod terminal;
pub mod updater;
pub mod usage;
pub mod workflow;
pub mod workspace;
pub mod worktree;

use crate::application::errors::{AppError, ErrorCode};

/// Runs `work` off the thread that serves the window: Git, the disk or a CLI can take a while. A
/// task that could not finish (it panicked) is reported as `code`.
async fn off_window<T: Send + 'static>(
    code: ErrorCode,
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, AppError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| AppError::new(code).with_detail(error.to_string()))
}

#[cfg(test)]
mod real_app_tests;
#[cfg(test)]
mod tests;
