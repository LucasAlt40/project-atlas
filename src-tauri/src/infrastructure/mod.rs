//! Adapters for the world outside the process: child processes and the filesystem.
//! Each implements a port declared in `application/`; `lib.rs` wires them.

pub mod config_file;
mod credential_store;
mod curl_registry;
mod fs_watcher;
mod git_worktree;
mod harness_store;
mod ide;
mod json_config_store;
mod process;
mod project_inspector;
mod project_scanner;
mod pty;
mod runtime_config_files;
mod skill_store;

pub use credential_store::KeyringCredentialStore;
pub use curl_registry::CurlRegistryClient;
pub use fs_watcher::NotifyWatcher;
pub use git_worktree::GitWorktreeManager;
pub use harness_store::FsHarnessStore;
pub use ide::SystemIdeLauncher;
pub use json_config_store::JsonConfigStore;
pub use process::SystemProcessRunner;
pub use project_inspector::FsProjectInspector;
pub use project_scanner::FsProjectScanner;
pub use runtime_config_files::FsRuntimeConfigFiles;
pub use skill_store::FsSkillStore;
