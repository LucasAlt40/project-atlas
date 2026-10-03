//! Tauri command handlers: thin adapters between the webview and `application/`.
//! No business logic here. Failures are returned as [`AppError`] (a stable code plus
//! parameters); the UI words them in the user's language.

pub mod agents;
pub mod app;
pub mod chat;
pub mod events;
pub mod personalities;
pub mod runtimes;
pub mod settings;
pub mod usage;
pub mod workspace;

#[cfg(test)]
mod tests;
