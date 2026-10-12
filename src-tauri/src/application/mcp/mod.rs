//! MCP in the application layer (ADR 0028): the plan of what a step may be given, the checks on a
//! connection's configuration, the port for secrets, and the service that keeps connections and
//! grants. Runtime-agnostic: a runtime adapter turns a plan into whatever its CLI needs.

mod catalog;
pub mod config_io;
mod credentials;
mod launch;
mod plan;
pub mod registry;
mod runtime_files;
mod service;
pub(crate) mod validate;

pub use crate::domain::mcp::ReportedMcpTool;
pub use catalog::{McpCatalog, McpPresetInfo};
pub use credentials::{CredentialError, CredentialStore};
pub use launch::{LaunchEnv, LaunchServer, McpLaunch, McpProbe};
pub use plan::{plan, CliServer, McpContext, McpInputs, McpPlan, PlannedServer};
pub use runtime_files::{RuntimeConfigFiles, RuntimeFileError};
pub use service::{
    ApplyReport, ImportReport, LaunchProblem, McpOverview, McpService, RuntimeSource, SyncPlan,
    SyncReport,
};

#[cfg(test)]
pub use runtime_files::memory::MemoryRuntimeFiles;

#[cfg(test)]
pub use credentials::memory::MemoryCredentials;
