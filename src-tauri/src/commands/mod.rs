//! Tauri command handlers: thin adapters between the webview and `application/`.
//! No business logic here. Failures are returned as [`AppError`] (a stable code plus
//! parameters); the UI words them in the user's language.

pub mod agents;
pub mod app;
pub mod chat;
pub mod events;
pub mod exit;
pub mod harness;
pub mod personalities;
pub mod runtimes;
pub mod security;
pub mod settings;
pub mod terminal;
pub mod usage;
pub mod workflow;
pub mod workspace;
pub mod worktree;

#[cfg(test)]
mod tests;
