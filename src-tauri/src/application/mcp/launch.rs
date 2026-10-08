//! What a runtime adapter is handed to start MCP servers: runtime-agnostic, secrets resolved,
//! nothing that names a CLI. The adapter translates it (`runtimes/claude_mcp.rs` for Claude).

use crate::domain::mcp::{McpServerStatus, Secret};

/// One variable of a server's environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchEnv {
    Plain {
        name: String,
        value: String,
    },
    /// The value is in memory only for the launch; its `Debug` shows nothing.
    Secret {
        name: String,
        value: Secret,
    },
}

impl LaunchEnv {
    pub fn name(&self) -> &str {
        match self {
            Self::Plain { name, .. } | Self::Secret { name, .. } => name,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchServer {
    /// The connection's name: how the runtime names the server and so its tools.
    pub name: String,
    pub executable: String,
    pub args: Vec<String>,
    pub env: Vec<LaunchEnv>,
}

/// The servers a step is given and the tools of those servers it is not. Built only after the
/// guardrails allowed the step, so a secret is read from the credential store no earlier.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct McpLaunch {
    pub servers: Vec<LaunchServer>,
    /// (server, tool) discovered and not authorized: kept from the runtime where it can be.
    pub held_back: Vec<(String, String)>,
}

impl McpLaunch {
    pub fn is_empty(&self) -> bool {
        self.servers.is_empty()
    }
}

/// What starting one server (to look at it) showed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpProbe {
    pub status: McpServerStatus,
    pub tools: Vec<String>,
}
