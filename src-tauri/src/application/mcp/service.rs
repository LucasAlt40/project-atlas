use std::sync::Arc;

use super::credentials::{CredentialError, CredentialStore};
use super::launch::{LaunchEnv, LaunchServer, McpLaunch};
use super::plan::{covers, McpContext, McpPlan};
use super::validate::{name_key, validate};
use crate::application::config::ConfigRepository;
use crate::application::errors::{AppError, ErrorCode};
use crate::application::runtimes::RuntimeRegistry;
use crate::application::support::{new_id, now_ms};
use crate::domain::mcp::{
    McpConnection, McpDiscovery, McpEnvValue, McpGrant, McpSupport, McpTransport, Secret,
    ToolSelection,
};

/// What a workspace has: its connections and who may use them.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpOverview {
    pub connections: Vec<McpConnection>,
    pub grants: Vec<McpGrant>,
}

/// The use cases around connections: keep them, grant them, look at them. It does not start
/// anything during a step (the runtime does, from a plan) and it never returns a secret.
pub struct McpService {
    config: Arc<ConfigRepository>,
    credentials: Arc<dyn CredentialStore>,
    runtimes: Arc<RuntimeRegistry>,
    /// When a refresh of a connection last failed, so a server that cannot be probed is not
    /// started again before every step (each try can take as long as the probe's timeout).
    refresh_failed: std::sync::Mutex<std::collections::HashMap<String, u64>>,
}

fn invalid(reason: &str) -> AppError {
    AppError::new(ErrorCode::McpConnectionInvalid).with("reason", reason)
}

fn store_error(error: &CredentialError) -> AppError {
    let CredentialError::Unavailable(detail) = error;
    AppError::new(ErrorCode::McpCredentialStoreUnavailable).with_detail(detail.clone())
}

impl McpService {
    pub fn new(
        config: Arc<ConfigRepository>,
        credentials: Arc<dyn CredentialStore>,
        runtimes: Arc<RuntimeRegistry>,
    ) -> Self {
        Self {
            config,
            credentials,
            runtimes,
            refresh_failed: std::sync::Mutex::default(),
        }
    }

    /// What a workspace has.
    pub fn overview(&self, workspace_id: &str) -> McpOverview {
        self.config.read(|config| {
            let connections: Vec<McpConnection> = config
                .mcp_connections
                .iter()
                .filter(|c| c.workspace_id == workspace_id)
                .cloned()
                .collect();
            let grants = config
                .mcp_grants
                .iter()
                .filter(|g| connections.iter().any(|c| c.id == g.connection_id))
                .cloned()
                .collect();
            McpOverview {
                connections,
                grants,
            }
        })
    }

    fn find(&self, connection_id: &str) -> Result<McpConnection, AppError> {
        self.config
            .read(|config| {
                config
                    .mcp_connections
                    .iter()
                    .find(|c| c.id == connection_id)
                    .cloned()
            })
            .ok_or_else(|| AppError::new(ErrorCode::McpConnectionNotFound))
    }

    /// Adds a connection. It starts switched off, with nothing granted and nothing discovered.
    ///
    /// # Errors
    ///
    /// The workspace does not exist, the name is taken, or the configuration is not acceptable.
    pub fn add(
        &self,
        workspace_id: &str,
        name: &str,
        transport: McpTransport,
        required: bool,
    ) -> Result<McpConnection, AppError> {
        let connection = McpConnection {
            id: new_id("mcp"),
            workspace_id: workspace_id.to_owned(),
            name: name.trim().to_owned(),
            transport,
            enabled: false,
            required,
            secrets: std::collections::BTreeMap::new(),
            discovery: None,
            created_at: now_ms(),
        };
        validate(&connection).map_err(|e| invalid(e.code()))?;
        self.config.modify(|config| {
            if !config.workspaces.iter().any(|w| w.id == workspace_id) {
                return Err(AppError::new(ErrorCode::WorkspaceNotFound));
            }
            if config.mcp_connections.iter().any(|c| {
                c.workspace_id == workspace_id && name_key(&c.name) == name_key(&connection.name)
            }) {
                return Err(AppError::new(ErrorCode::McpNameTaken));
            }
            config.mcp_connections.push(connection.clone());
            Ok(connection.clone())
        })
    }

    /// Changes how a connection starts. What was learned about the old one is forgotten, it is
    /// switched off (it must be turned on again on purpose), and a secret it no longer declares
    /// is removed from the store.
    ///
    /// # Errors
    ///
    /// The connection does not exist or the new configuration is not acceptable.
    pub fn update(
        &self,
        connection_id: &str,
        transport: McpTransport,
        required: bool,
    ) -> Result<McpConnection, AppError> {
        let mut updated = self.find(connection_id)?;
        updated.transport = transport;
        updated.required = required;
        updated.enabled = false;
        updated.discovery = None;
        validate(&updated).map_err(|e| invalid(e.code()))?;
        let declared: Vec<String> = updated
            .secret_names()
            .iter()
            .map(|n| (*n).to_owned())
            .collect();
        let dropped: Vec<String> = updated
            .secrets
            .keys()
            .filter(|name| !declared.contains(name))
            .cloned()
            .collect();
        for name in &dropped {
            // Best effort: a secret nobody refers to any more is not worth failing the edit for.
            let _ = self.credentials.delete(&updated.id, name);
            updated.secrets.remove(name);
        }
        self.replace(&updated)
    }

    fn replace(&self, connection: &McpConnection) -> Result<McpConnection, AppError> {
        self.config.modify(|config| {
            let slot = config
                .mcp_connections
                .iter_mut()
                .find(|c| c.id == connection.id)
                .ok_or_else(|| AppError::new(ErrorCode::McpConnectionNotFound))?;
            *slot = connection.clone();
            Ok(connection.clone())
        })
    }

    /// Switches a connection on or off. Switching on authorizes the user's own decision to start
    /// this process; it grants no agent anything.
    ///
    /// # Errors
    ///
    /// The connection does not exist, or it cannot be switched on as configured.
    pub fn set_enabled(
        &self,
        connection_id: &str,
        enabled: bool,
    ) -> Result<McpConnection, AppError> {
        let mut connection = self.find(connection_id)?;
        if enabled {
            validate(&connection).map_err(|e| invalid(e.code()))?;
        }
        connection.enabled = enabled;
        self.replace(&connection)
    }

    /// Removes a connection with its grants and its secrets.
    ///
    /// # Errors
    ///
    /// The connection does not exist.
    pub fn remove(&self, connection_id: &str) -> Result<(), AppError> {
        let connection = self.find(connection_id)?;
        for name in connection.secrets.keys() {
            let _ = self.credentials.delete(&connection.id, name);
        }
        self.config.modify(|config| {
            config.mcp_connections.retain(|c| c.id != connection_id);
            config
                .mcp_grants
                .retain(|g| g.connection_id != connection_id);
            Ok(())
        })
    }

    /// Stores a secret the connection declares. The value goes to the credential store and is
    /// never kept or returned here; only the time it was stored is.
    ///
    /// # Errors
    ///
    /// The connection does not exist, does not declare that secret, or the store is unavailable.
    pub fn set_secret(
        &self,
        connection_id: &str,
        name: &str,
        value: &Secret,
    ) -> Result<McpConnection, AppError> {
        let mut connection = self.find(connection_id)?;
        if !connection.secret_names().contains(&name) || value.expose().is_empty() {
            return Err(AppError::new(ErrorCode::McpSecretUnknown));
        }
        self.credentials
            .put(&connection.id, name, value)
            .map_err(|e| store_error(&e))?;
        connection.secrets.insert(name.to_owned(), now_ms());
        self.replace(&connection)
    }

    /// Forgets a stored secret.
    ///
    /// # Errors
    ///
    /// The connection does not exist or the store is unavailable.
    pub fn clear_secret(&self, connection_id: &str, name: &str) -> Result<McpConnection, AppError> {
        let mut connection = self.find(connection_id)?;
        self.credentials
            .delete(&connection.id, name)
            .map_err(|e| store_error(&e))?;
        connection.secrets.remove(name);
        self.replace(&connection)
    }

    /// Lets an agent (and optionally one workflow and one of its steps) use a connection: all of
    /// its tools, or the ones named. More than one grant can cover a step; they add up.
    ///
    /// # Errors
    ///
    /// The connection or the agent does not exist, or the tools named are not valid.
    pub fn grant(
        &self,
        connection_id: &str,
        agent_id: Option<&str>,
        workflow_id: Option<&str>,
        node_id: Option<&str>,
        tools: ToolSelection,
    ) -> Result<McpGrant, AppError> {
        let connection = self.find(connection_id)?;
        if let ToolSelection::Only { tools } = &tools {
            if tools.is_empty() || tools.iter().any(|t| !valid_tool(t)) {
                return Err(AppError::new(ErrorCode::McpGrantInvalid));
            }
        }
        if node_id.is_some() && workflow_id.is_none() {
            return Err(AppError::new(ErrorCode::McpGrantInvalid));
        }
        let connection_workspace = connection.workspace_id.clone();
        let grant = McpGrant {
            id: new_id("grant"),
            connection_id: connection.id,
            agent_id: agent_id.map(str::to_owned),
            workflow_id: workflow_id.map(str::to_owned),
            node_id: node_id.map(str::to_owned),
            tools,
        };
        self.config.modify(|config| {
            if let Some(agent) = &grant.agent_id {
                if !config.agents.iter().any(|a| &a.id == agent) {
                    return Err(AppError::new(ErrorCode::AgentNotFound));
                }
            }
            if let Some(workflow_id) = &grant.workflow_id {
                // A grant for a workflow or step that does not exist would sit there doing
                // nothing, and look like protection: refuse it.
                let workflow = config
                    .workflows
                    .iter()
                    .find(|w| &w.id == workflow_id && w.workspace_id == connection_workspace)
                    .ok_or_else(|| AppError::new(ErrorCode::WorkflowNotFound))?;
                if let Some(node_id) = &grant.node_id {
                    let node = workflow
                        .nodes
                        .iter()
                        .find(|n| &n.id == node_id)
                        .ok_or_else(|| AppError::new(ErrorCode::McpGrantInvalid))?;
                    // Only an agent's step runs a model; a grant to another agent than the one
                    // the step runs can never apply.
                    match (&node.kind, &grant.agent_id) {
                        (crate::domain::workflow::NodeKind::Agent(step), agent) => {
                            if agent.as_ref().is_some_and(|a| a != &step.agent_id) {
                                return Err(AppError::new(ErrorCode::McpGrantInvalid));
                            }
                        }
                        _ => return Err(AppError::new(ErrorCode::McpGrantInvalid)),
                    }
                }
            }
            config.mcp_grants.push(grant.clone());
            Ok(grant.clone())
        })
    }

    /// Takes a grant back. Effective for the next step that starts.
    ///
    /// # Errors
    ///
    /// Saving fails.
    pub fn revoke(&self, grant_id: &str) -> Result<(), AppError> {
        self.config.modify(|config| {
            config.mcp_grants.retain(|g| g.id != grant_id);
            Ok(())
        })
    }

    fn launch_server(&self, connection: &McpConnection) -> Result<LaunchServer, LaunchProblem> {
        let McpTransport::Stdio {
            executable,
            args,
            env,
        } = &connection.transport
        else {
            return Err(LaunchProblem::Invalid);
        };
        let mut resolved = Vec::new();
        for var in env {
            match &var.value {
                McpEnvValue::Plain { value } => resolved.push(LaunchEnv::Plain {
                    name: var.name.clone(),
                    value: value.clone(),
                }),
                McpEnvValue::Secret => {
                    // The stored time says it was set; the store must still have it.
                    let secret = self
                        .credentials
                        .get(&connection.id, &var.name)
                        .map_err(|_| LaunchProblem::StoreUnavailable)?
                        .ok_or_else(|| LaunchProblem::SecretMissing(var.name.clone()))?;
                    resolved.push(LaunchEnv::Secret {
                        name: var.name.clone(),
                        value: secret,
                    });
                }
            }
        }
        Ok(LaunchServer {
            name: connection.name.clone(),
            executable: executable.clone(),
            args: args.clone(),
            env: resolved,
        })
    }

    /// Resolves the secrets of the servers a plan exposes, for the launch of the step. A server
    /// whose secret cannot be read is not in the launch and is named in the second list: the
    /// caller decides what that means (a required connection stops the step).
    pub fn launch(&self, plan: &McpPlan) -> (McpLaunch, Vec<(String, LaunchProblem)>) {
        let mut launch = McpLaunch::default();
        let mut left_out = Vec::new();
        for server in plan.exposed() {
            // Two servers whose names come to the same key would share the variable a secret
            // travels in: the second is left out rather than given the first one's value.
            if launch
                .servers
                .iter()
                .any(|s| name_key(&s.name) == name_key(&server.connection.name))
            {
                left_out.push((server.connection.name.clone(), LaunchProblem::Invalid));
                continue;
            }
            match self.launch_server(&server.connection) {
                Ok(started) => launch.servers.push(started),
                Err(problem) => left_out.push((server.connection.name.clone(), problem)),
            }
        }
        launch.only = plan
            .exposed()
            .filter(|s| launch.servers.iter().any(|l| l.name == s.connection.name))
            .filter_map(|s| match &s.selection {
                Some(ToolSelection::Only { tools }) => {
                    Some((s.connection.name.clone(), tools.clone()))
                }
                _ => None,
            })
            .collect();
        launch.held_back = plan
            .held_back()
            .into_iter()
            .filter(|(server, _)| launch.servers.iter().any(|s| &s.name == server))
            .collect();
        (launch, left_out)
    }

    /// Starts the server through `runtime_id` only to see what it reports (its status and the
    /// names of its tools), and keeps that as the connection's discovery. No model is asked
    /// anything. It runs the process the user configured, so the connection must be on.
    ///
    /// # Errors
    ///
    /// The connection or runtime does not exist, the runtime has no MCP adapter, the connection
    /// is not ready, or the runtime could not report.
    pub fn probe(&self, connection_id: &str, runtime_id: &str) -> Result<McpConnection, AppError> {
        let mut connection = self.find(connection_id)?;
        if !connection.enabled || connection.missing_secret().is_some() {
            return Err(AppError::new(ErrorCode::McpNotReady));
        }
        validate(&connection).map_err(|e| invalid(e.code()))?;
        let runtime = self
            .runtimes
            .find(runtime_id)
            .ok_or_else(|| AppError::new(ErrorCode::RuntimeNotSupported))?;
        let capabilities = runtime.info().capabilities;
        // A runtime without an adapter, or one that cannot look at a server without a model.
        if capabilities.mcp != McpSupport::Supported
            || capabilities.mcp_features.probe == crate::domain::mcp::McpProbeKind::None
        {
            return Err(AppError::new(ErrorCode::McpRuntimeUnsupported));
        }
        let server = self
            .launch_server(&connection)
            .map_err(|problem| match problem {
                LaunchProblem::StoreUnavailable => {
                    AppError::new(ErrorCode::McpCredentialStoreUnavailable)
                }
                _ => AppError::new(ErrorCode::McpNotReady),
            })?;
        let probe = runtime.probe_mcp(&server).map_err(|error| {
            AppError::new(ErrorCode::McpProbeFailed).with_detail(error.user_message())
        })?;
        connection.discovery = Some(McpDiscovery {
            discovered_at: now_ms(),
            runtime_id: runtime_id.to_owned(),
            status: probe.status,
            tools: probe.tools,
        });
        self.replace(&connection)
    }

    /// Looks again at the connections whose tools a grant names one by one, when what Atlas knows
    /// of them is older than `max_age_ms`. Such a grant is the one a server can get around by
    /// adding a tool after it was discovered, so it is worth a fresh look before the step. A
    /// connection nobody named tools for is not started (nothing would change in the plan), and
    /// one that cannot be probed keeps the discovery it had. Returns the connections refreshed.
    pub fn refresh_discovery(
        &self,
        ctx: &McpContext<'_>,
        runtime_id: &str,
        max_age_ms: u64,
    ) -> Vec<String> {
        let overview = self.overview(ctx.workspace);
        let now = now_ms();
        let due = |connection: &McpConnection| {
            connection.enabled
                && connection.discovery.as_ref().is_none_or(|d| {
                    d.runtime_id != runtime_id || now.saturating_sub(d.discovered_at) >= max_age_ms
                })
                && overview.grants.iter().any(|grant| {
                    covers(grant, connection, ctx)
                        && matches!(grant.tools, ToolSelection::Only { .. })
                })
        };
        let recently_failed = |id: &str| {
            self.refresh_failed
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(id)
                .is_some_and(|at| now.saturating_sub(*at) < max_age_ms)
        };
        overview
            .connections
            .iter()
            .filter(|connection| due(connection) && !recently_failed(&connection.id))
            .filter(|connection| {
                let ok = self.probe(&connection.id, runtime_id).is_ok();
                let mut failed = self
                    .refresh_failed
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if ok {
                    failed.remove(&connection.id);
                } else {
                    failed.insert(connection.id.clone(), now);
                }
                ok
            })
            .map(|connection| connection.name.clone())
            .collect()
    }

    /// What the workspace has, for planning a step.
    pub fn snapshot(&self, workspace_id: &str) -> McpOverview {
        self.overview(workspace_id)
    }
}

fn valid_tool(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

/// Why a server could not be put in a launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchProblem {
    Invalid,
    SecretMissing(String),
    StoreUnavailable,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::config::memory::MemoryStore;
    use crate::application::mcp::{plan, McpContext, McpInputs, MemoryCredentials};
    use crate::application::runtimes::fake::FakeRuntime;
    use crate::domain::mcp::{McpEnv, McpServerStatus};
    use crate::domain::security::Permission;
    use crate::domain::workspace::{Workspace, WorkspaceLayout};

    struct Setup {
        service: McpService,
        config: Arc<ConfigRepository>,
        credentials: Arc<MemoryCredentials>,
        runtime: Arc<FakeRuntime>,
    }

    fn setup() -> Setup {
        let config = Arc::new(ConfigRepository::load(Box::<MemoryStore>::default()));
        config
            .modify(|c| {
                c.workspaces.push(Workspace {
                    id: "w1".to_owned(),
                    name: "W".to_owned(),
                    project_path: "/p".to_owned(),
                    description: None,
                    created_at: 1,
                    updated_at: 1,
                    layout: WorkspaceLayout {
                        rows: 1,
                        columns: 1,
                        agent_placements: vec![],
                    },
                    security: crate::domain::security::SecurityPolicy::default(),
                });
                Ok(())
            })
            .unwrap();
        let credentials = Arc::new(MemoryCredentials::default());
        let runtime = Arc::new(
            FakeRuntime::new("rt-a", Ok("x"))
                .with_mcp()
                .probing(McpServerStatus::Connected, &["read", "write"]),
        );
        let registry = Arc::new(RuntimeRegistry::new(vec![runtime.clone() as Arc<_>]));
        Setup {
            service: McpService::new(config.clone(), credentials.clone(), registry),
            config,
            credentials,
            runtime,
        }
    }

    fn stdio(secret: bool) -> McpTransport {
        McpTransport::Stdio {
            executable: "node".to_owned(),
            args: vec!["server.js".to_owned()],
            env: if secret {
                vec![McpEnv {
                    name: "API_TOKEN".to_owned(),
                    value: McpEnvValue::Secret,
                }]
            } else {
                vec![]
            },
        }
    }

    #[test]
    fn a_new_connection_is_off_ungranted_and_undiscovered() {
        let s = setup();

        let added = s.service.add("w1", " files ", stdio(false), false).unwrap();

        assert_eq!(added.name, "files");
        assert!(!added.enabled && added.discovery.is_none());
        let overview = s.service.overview("w1");
        assert_eq!(overview.connections.len(), 1);
        assert_eq!(overview.grants.len(), 0);
        // Another workspace sees none of it.
        assert_eq!(s.service.overview("w2").connections.len(), 0);
    }

    #[test]
    fn a_bad_configuration_a_taken_name_or_an_unknown_workspace_is_refused() {
        let s = setup();
        s.service.add("w1", "files", stdio(false), false).unwrap();

        let shell = McpTransport::Stdio {
            executable: "bash".to_owned(),
            args: vec!["-c".to_owned(), "x".to_owned()],
            env: vec![],
        };
        assert!(s
            .service
            .add("w1", "other", shell, false)
            .unwrap_err()
            .is(ErrorCode::McpConnectionInvalid));
        assert!(s
            .service
            .add("w1", "files", stdio(false), false)
            .unwrap_err()
            .is(ErrorCode::McpNameTaken));
        assert!(s
            .service
            .add("nowhere", "files", stdio(false), false)
            .unwrap_err()
            .is(ErrorCode::WorkspaceNotFound));
        assert!(s
            .service
            .add("w1", "Bad Name", stdio(false), false)
            .unwrap_err()
            .is(ErrorCode::McpConnectionInvalid));
        // Nothing but the first one was saved.
        assert_eq!(s.service.overview("w1").connections.len(), 1);
    }

    #[test]
    fn a_secret_goes_to_the_store_and_only_its_time_is_kept() {
        let s = setup();
        let c = s.service.add("w1", "api", stdio(true), false).unwrap();

        let stored = s
            .service
            .set_secret(
                &c.id,
                "API_TOKEN",
                &Secret::new("hunter2hunter2".to_owned()),
            )
            .unwrap();

        assert!(stored.secrets.contains_key("API_TOKEN"));
        assert_eq!(
            s.credentials
                .items
                .lock()
                .unwrap()
                .get(&(c.id.clone(), "API_TOKEN".to_owned()))
                .map(String::as_str),
            Some("hunter2hunter2")
        );
        // Not in the saved configuration, in any form.
        let saved = serde_json::to_string(&s.config.snapshot().mcp_connections).unwrap();
        assert!(!saved.contains("hunter2"));
    }

    #[test]
    fn only_a_declared_secret_can_be_stored_and_a_locked_store_is_an_error() {
        let s = setup();
        let c = s.service.add("w1", "api", stdio(true), false).unwrap();

        assert!(s
            .service
            .set_secret(&c.id, "OTHER", &Secret::new("x".to_owned()))
            .unwrap_err()
            .is(ErrorCode::McpSecretUnknown));
        assert!(s
            .service
            .set_secret(&c.id, "API_TOKEN", &Secret::new(String::new()))
            .unwrap_err()
            .is(ErrorCode::McpSecretUnknown));
        s.credentials
            .locked
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(s
            .service
            .set_secret(&c.id, "API_TOKEN", &Secret::new("x".to_owned()))
            .unwrap_err()
            .is(ErrorCode::McpCredentialStoreUnavailable));
        // And the failed attempt left no trace of a credential.
        assert!(s.service.overview("w1").connections[0].secrets.is_empty());
    }

    #[test]
    fn removing_a_connection_removes_its_grants_and_its_secrets() {
        let s = setup();
        let c = s.service.add("w1", "api", stdio(true), false).unwrap();
        s.service
            .set_secret(&c.id, "API_TOKEN", &Secret::new("x".to_owned()))
            .unwrap();
        s.service
            .grant(&c.id, None, None, None, ToolSelection::Server)
            .unwrap();

        s.service.remove(&c.id).unwrap();

        assert_eq!(s.service.overview("w1").connections.len(), 0);
        assert_eq!(s.config.snapshot().mcp_grants.len(), 0);
        assert!(s.credentials.items.lock().unwrap().is_empty());
    }

    #[test]
    fn editing_a_connection_switches_it_off_forgets_what_was_learned_and_drops_unused_secrets() {
        let s = setup();
        let c = s.service.add("w1", "api", stdio(true), false).unwrap();
        s.service
            .set_secret(&c.id, "API_TOKEN", &Secret::new("x".to_owned()))
            .unwrap();
        s.service.set_enabled(&c.id, true).unwrap();
        s.service.probe(&c.id, "rt-a").unwrap();

        let edited = s.service.update(&c.id, stdio(false), true).unwrap();

        assert!(!edited.enabled && edited.discovery.is_none() && edited.required);
        assert!(edited.secrets.is_empty());
        assert!(s.credentials.items.lock().unwrap().is_empty());
    }

    #[test]
    fn a_grant_needs_a_real_connection_agent_and_tools_and_can_be_revoked() {
        let s = setup();
        let c = s.service.add("w1", "files", stdio(false), false).unwrap();

        assert!(s
            .service
            .grant("ghost", None, None, None, ToolSelection::Server)
            .unwrap_err()
            .is(ErrorCode::McpConnectionNotFound));
        assert!(s
            .service
            .grant(&c.id, Some("nobody"), None, None, ToolSelection::Server)
            .unwrap_err()
            .is(ErrorCode::AgentNotFound));
        for bad in [
            ToolSelection::Only { tools: vec![] },
            ToolSelection::Only {
                tools: vec!["a b".to_owned()],
            },
        ] {
            assert!(s
                .service
                .grant(&c.id, None, None, None, bad)
                .unwrap_err()
                .is(ErrorCode::McpGrantInvalid));
        }
        // A step without its workflow means nothing.
        assert!(s
            .service
            .grant(&c.id, None, None, Some("n1"), ToolSelection::Server)
            .unwrap_err()
            .is(ErrorCode::McpGrantInvalid));

        add_workflow(&s, "wf1", &[("n1", "someone")]);
        let granted = s
            .service
            .grant(&c.id, None, Some("wf1"), Some("n1"), ToolSelection::Server)
            .unwrap();
        assert_eq!(s.service.overview("w1").grants.len(), 1);
        s.service.revoke(&granted.id).unwrap();
        assert_eq!(s.service.overview("w1").grants.len(), 0);
    }

    #[test]
    fn a_probe_needs_the_connection_on_and_its_secrets_and_a_runtime_with_an_adapter() {
        let s = setup();
        let c = s.service.add("w1", "api", stdio(true), false).unwrap();

        // Off: starting the process is the user's decision, not yet made.
        assert!(s
            .service
            .probe(&c.id, "rt-a")
            .unwrap_err()
            .is(ErrorCode::McpNotReady));
        s.service.set_enabled(&c.id, true).unwrap();
        // On, but the secret was never stored.
        assert!(s
            .service
            .probe(&c.id, "rt-a")
            .unwrap_err()
            .is(ErrorCode::McpNotReady));
        assert!(s.runtime.probed.lock().unwrap().is_empty());
        s.service
            .set_secret(&c.id, "API_TOKEN", &Secret::new("x".to_owned()))
            .unwrap();

        let probed = s.service.probe(&c.id, "rt-a").unwrap();

        let discovery = probed.discovery.unwrap();
        assert_eq!(discovery.status, McpServerStatus::Connected);
        assert_eq!(discovery.tools, ["read", "write"]);
        assert_eq!(discovery.runtime_id, "rt-a");
        assert_eq!(*s.runtime.probed.lock().unwrap(), ["api"]);
        assert!(s
            .service
            .probe(&c.id, "ghost")
            .unwrap_err()
            .is(ErrorCode::RuntimeNotSupported));
    }

    #[test]
    fn a_runtime_nobody_investigated_is_not_probed() {
        let s = setup();
        let registry =
            Arc::new(RuntimeRegistry::new(vec![
                Arc::new(FakeRuntime::new("plain", Ok("x"))) as Arc<_>,
            ]));
        let service = McpService::new(s.config.clone(), s.credentials.clone(), registry);
        let c = service.add("w1", "files", stdio(false), false).unwrap();
        service.set_enabled(&c.id, true).unwrap();

        assert!(service
            .probe(&c.id, "plain")
            .unwrap_err()
            .is(ErrorCode::McpRuntimeUnsupported));
    }

    #[test]
    fn a_failed_probe_is_an_error_with_what_the_runtime_said_and_changes_nothing() {
        let s = setup();
        let registry = Arc::new(RuntimeRegistry::new(vec![Arc::new(
            FakeRuntime::new("rt-b", Ok("x")).with_mcp(),
        ) as Arc<_>]));
        let service = McpService::new(s.config.clone(), s.credentials.clone(), registry);
        let c = service.add("w1", "files", stdio(false), false).unwrap();
        service.set_enabled(&c.id, true).unwrap();

        let error = service.probe(&c.id, "rt-b").unwrap_err();

        assert!(error.is(ErrorCode::McpProbeFailed));
        assert!(service.overview("w1").connections[0].discovery.is_none());
    }

    #[test]
    fn the_launch_reads_secrets_only_for_what_the_plan_exposes_and_names_what_it_cannot() {
        let s = setup();
        let ok = s.service.add("w1", "ok", stdio(true), false).unwrap();
        let broken = s.service.add("w1", "broken", stdio(true), true).unwrap();
        let idle = s.service.add("w1", "idle", stdio(true), false).unwrap();
        for c in [&ok, &broken, &idle] {
            s.service
                .set_secret(
                    &c.id,
                    "API_TOKEN",
                    &Secret::new(format!("secret-of-{}", c.name)),
                )
                .unwrap();
            s.service.set_enabled(&c.id, true).unwrap();
        }
        for c in [&ok, &broken] {
            s.service
                .grant(&c.id, None, None, None, ToolSelection::Server)
                .unwrap();
        }
        // The credential of one vanished from the store after it was recorded.
        s.credentials
            .items
            .lock()
            .unwrap()
            .remove(&(broken.id.clone(), "API_TOKEN".to_owned()));
        let overview = s.service.overview("w1");
        let planned = plan(&McpInputs {
            connections: &overview.connections,
            grants: &overview.grants,
            context: McpContext {
                workspace: "w1",
                agent: "a1",
                workflow: None,
                node: None,
            },
            policy: Permission::Allowed,
            support: McpSupport::Supported,
            features: claude_like(),
        });

        let (launch, left_out) = s.service.launch(&planned);

        assert_eq!(launch.servers.len(), 1);
        assert_eq!(launch.servers[0].name, "ok");
        assert_eq!(
            left_out,
            [(
                "broken".to_owned(),
                LaunchProblem::SecretMissing("API_TOKEN".to_owned())
            )]
        );
        // The server nobody granted was never read.
        assert!(!format!("{launch:?}").contains("secret-of-idle"));
        assert!(!format!("{launch:?}").contains("secret-of-ok"));
    }

    fn claude_like() -> crate::domain::mcp::McpFeatures {
        crate::domain::mcp::McpFeatures {
            tool_filter: crate::domain::mcp::McpToolFilter::DenyList,
            probe: crate::domain::mcp::McpProbeKind::Tools,
            strict: false,
        }
    }

    fn ready_connection(s: &Setup, name: &str) -> McpConnection {
        let c = s.service.add("w1", name, stdio(false), false).unwrap();
        s.service.set_enabled(&c.id, true).unwrap();
        c
    }

    fn add_workflow(s: &Setup, id: &str, nodes: &[(&str, &str)]) {
        use crate::application::workflow::test_support::{agent, workflow};
        let mut w = workflow(
            nodes.iter().map(|(node, who)| agent(node, who)).collect(),
            vec![],
        );
        w.id = id.to_owned();
        w.workspace_id = "w1".to_owned();
        s.config
            .modify(|c| {
                c.workflows.push(w);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn a_grant_must_name_a_workflow_and_a_step_that_exist() {
        let s = setup();
        let c = ready_connection(&s, "files");
        add_workflow(&s, "wf1", &[("n1", "someone")]);
        let grant = |wf: Option<&str>, node: Option<&str>, agent: Option<&str>| {
            s.service
                .grant(&c.id, agent, wf, node, ToolSelection::Server)
        };

        assert!(grant(Some("wf1"), None, None).is_ok());
        assert!(grant(Some("wf1"), Some("n1"), None).is_ok());
        assert!(grant(Some("ghost"), None, None)
            .unwrap_err()
            .is(ErrorCode::WorkflowNotFound));
        assert!(grant(Some("wf1"), Some("nope"), None)
            .unwrap_err()
            .is(ErrorCode::McpGrantInvalid));
    }

    fn ctx() -> McpContext<'static> {
        McpContext {
            workspace: "w1",
            agent: "a1",
            workflow: None,
            node: None,
        }
    }

    #[test]
    fn a_grant_that_names_tools_gets_a_fresh_look_when_the_discovery_is_old() {
        let s = setup();
        let c = ready_connection(&s, "files");
        s.service.probe(&c.id, "rt-a").unwrap();
        s.service
            .grant(
                &c.id,
                None,
                None,
                None,
                ToolSelection::Only {
                    tools: vec!["read".to_owned()],
                },
            )
            .unwrap();
        s.runtime.probed.lock().unwrap().clear();

        // Fresh enough: nothing is started.
        assert_eq!(s.service.refresh_discovery(&ctx(), "rt-a", 60_000).len(), 0);
        assert_eq!(s.runtime.probed.lock().unwrap().len(), 0);
        // Older than the limit: it is looked at again.
        assert_eq!(s.service.refresh_discovery(&ctx(), "rt-a", 0), ["files"]);
        assert_eq!(*s.runtime.probed.lock().unwrap(), ["files"]);
    }

    #[test]
    fn a_connection_with_no_named_tools_for_this_step_is_never_started_for_a_refresh() {
        let s = setup();
        let c = ready_connection(&s, "files");
        add_workflow(&s, "wf-other", &[("n1", "someone")]);
        // Server-wide for this step, and a named-tools grant for another workflow.
        s.service
            .grant(&c.id, None, None, None, ToolSelection::Server)
            .unwrap();
        s.service
            .grant(
                &c.id,
                None,
                Some("wf-other"),
                None,
                ToolSelection::Only {
                    tools: vec!["read".to_owned()],
                },
            )
            .unwrap();
        let off = s.service.add("w1", "off", stdio(false), false).unwrap();
        s.service
            .grant(
                &off.id,
                None,
                None,
                None,
                ToolSelection::Only {
                    tools: vec!["read".to_owned()],
                },
            )
            .unwrap();

        assert_eq!(s.service.refresh_discovery(&ctx(), "rt-a", 0).len(), 0);
        assert_eq!(s.runtime.probed.lock().unwrap().len(), 0);
    }

    #[test]
    fn a_refresh_that_fails_keeps_what_was_known() {
        let s = setup();
        let c = ready_connection(&s, "files");
        s.service.probe(&c.id, "rt-a").unwrap();
        s.service
            .grant(
                &c.id,
                None,
                None,
                None,
                ToolSelection::Only {
                    tools: vec!["read".to_owned()],
                },
            )
            .unwrap();
        // A runtime that cannot probe now.
        let registry = Arc::new(RuntimeRegistry::new(vec![Arc::new(
            FakeRuntime::new("rt-a", Ok("x")).with_mcp(),
        ) as Arc<_>]));
        let service = McpService::new(s.config.clone(), s.credentials.clone(), registry);

        assert_eq!(service.refresh_discovery(&ctx(), "rt-a", 0).len(), 0);

        let kept = service.overview("w1").connections[0]
            .discovery
            .clone()
            .unwrap();
        assert_eq!(kept.tools, ["read", "write"]);
    }

    #[test]
    fn a_connection_that_failed_to_refresh_is_not_started_again_before_every_step() {
        let s = setup();
        let c = ready_connection(&s, "files");
        s.service
            .grant(
                &c.id,
                None,
                None,
                None,
                ToolSelection::Only {
                    tools: vec!["read".to_owned()],
                },
            )
            .unwrap();
        let broken = Arc::new(FakeRuntime::new("rt-a", Ok("x")).with_mcp());
        let registry = Arc::new(RuntimeRegistry::new(vec![broken.clone() as Arc<_>]));
        let service = McpService::new(s.config.clone(), s.credentials.clone(), registry);

        service.refresh_discovery(&ctx(), "rt-a", 60_000);
        service.refresh_discovery(&ctx(), "rt-a", 60_000);
        service.refresh_discovery(&ctx(), "rt-a", 60_000);

        assert_eq!(broken.probed.lock().unwrap().len(), 1);
    }

    #[test]
    fn names_that_differ_only_by_a_dash_or_case_are_the_same_connection() {
        let s = setup();
        s.service.add("w1", "web-2", stdio(false), false).unwrap();

        for same in ["web_2", "WEB-2"] {
            assert!(s
                .service
                .add("w1", same, stdio(false), false)
                .unwrap_err()
                .is(if same == "WEB-2" {
                    ErrorCode::McpConnectionInvalid
                } else {
                    ErrorCode::McpNameTaken
                }));
        }
    }

    #[test]
    fn two_servers_that_share_a_key_never_share_a_secret_variable() {
        use crate::application::mcp::{plan, McpInputs};
        let s = setup();
        // A configuration edited by hand can hold both: the guard is in the launch, not only in add.
        let a = ready_connection(&s, "web-2");
        let mut b = ready_connection(&s, "other");
        b.name = "web_2".to_owned();
        s.config
            .modify(|c| {
                c.mcp_connections
                    .iter_mut()
                    .find(|x| x.id == b.id)
                    .unwrap()
                    .name = "web_2".to_owned();
                Ok(())
            })
            .unwrap();
        for id in [&a.id, &b.id] {
            s.service
                .grant(id, None, None, None, ToolSelection::Server)
                .unwrap();
        }
        let overview = s.service.overview("w1");
        let plan = plan(&McpInputs {
            connections: &overview.connections,
            grants: &overview.grants,
            context: ctx(),
            policy: crate::domain::security::Permission::Allowed,
            support: McpSupport::Supported,
            features: claude_like(),
        });

        let (launch, left_out) = s.service.launch(&plan);

        assert_eq!(launch.servers.len(), 1);
        assert_eq!(left_out.len(), 1);
    }
}
