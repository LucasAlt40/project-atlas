//! Adapters for the world outside the process: child processes and the filesystem.
//! Each implements a port declared in `application/`; `lib.rs` wires them.

mod ai_memory;
mod atomic_file;
pub mod config_file;
mod credential_store;
mod curl_registry;
mod discord_ipc;
mod fs_watcher;
mod git_worktree;
mod harness_store;
mod ide;
mod json_config_store;
mod memory_store;
mod process;
mod project_files;
mod project_inspector;
mod project_scanner;
mod pty;
mod published_ledger;
mod remote_server;
mod runtime_config_files;
mod skill_store;
mod spec_store;
mod system_network;
mod workspace_context_store;

pub use ai_memory::AiMemoryHttp;
pub use credential_store::KeyringCredentialStore;
pub use curl_registry::CurlRegistryClient;
pub use discord_ipc::DiscordIpc;
pub use fs_watcher::NotifyWatcher;
pub use git_worktree::GitWorktreeManager;
pub use harness_store::FsHarnessStore;
pub use ide::SystemIdeLauncher;
pub use json_config_store::JsonConfigStore;
pub use memory_store::{FsMemoryStore, FsProjectProbe};
pub use process::SystemProcessRunner;
pub use project_files::FsProjectFiles;
pub use project_inspector::FsProjectInspector;
pub use project_scanner::FsProjectScanner;
pub use published_ledger::FsPublishedLedger;
#[cfg(test)]
pub use remote_server::start_in as start_remote_server_in;
pub use remote_server::{start as start_remote_server, ServerDeps};
pub use runtime_config_files::FsRuntimeConfigFiles;
pub use skill_store::FsSkillStore;
pub use spec_store::FsSpecStore;
pub use system_network::SystemInterfaces;
pub use workspace_context_store::FsWorkspaceContextStore;
