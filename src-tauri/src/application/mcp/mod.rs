//! MCP in the application layer (ADR 0028): the plan of what a step may be given, the checks on a
//! connection's configuration, the port for secrets, and the service that keeps connections and
//! grants. Runtime-agnostic: a runtime adapter turns a plan into whatever its CLI needs.

mod catalog;
mod credentials;
mod launch;
mod plan;
mod service;
pub(crate) mod validate;

pub use crate::domain::mcp::ReportedMcpTool;
pub use catalog::{McpCatalog, McpPresetInfo};
pub use credentials::{CredentialError, CredentialStore};
pub use launch::{LaunchEnv, LaunchServer, McpLaunch, McpProbe};
pub use plan::{plan, McpContext, McpInputs, McpPlan};
pub use service::{LaunchProblem, McpOverview, McpService};

#[cfg(test)]
pub use credentials::memory::MemoryCredentials;
