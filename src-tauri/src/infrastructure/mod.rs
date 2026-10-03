//! Adapters for the world outside the process: child processes and the filesystem.
//! Each implements a port declared in `application/`; `lib.rs` wires them.

mod json_config_store;
mod process;
mod project_inspector;

pub use json_config_store::JsonConfigStore;
pub use process::SystemProcessRunner;
pub use project_inspector::FsProjectInspector;
