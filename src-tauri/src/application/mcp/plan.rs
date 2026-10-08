//! Which MCP tools one step may be given: a pure function of the workspace's connections, the
//! grants, the agent's policy and what the runtime can do. Same inputs, same answer, in the same
//! order. It starts nothing and reads no secret: it only says what Atlas will arrange.
//!
//! ```text
//! connection ──enabled?── grant covers this agent / workflow / step? ── policy allows MCP? ──
//!   runtime has an MCP adapter? ── configuration valid? ── tools can be held back? ── secrets there?
//!                                           ▼
//!                                   exposed to the runtime
//! ```
//!
//! The first thing that fails is the problem reported for that connection. Nothing here turns a
//! failure into a permission: a connection that cannot be exposed is simply not exposed, and
//! whether the step may go on without it is the guardrails' call (`required`).

use std::collections::BTreeSet;

use super::validate::validate;
use crate::domain::mcp::{
    McpConnection, McpGrant, McpProblem, McpRecord, McpServerRecord, McpSupport, McpToolState,
    ToolSelection,
};
use crate::domain::security::Permission;

/// What the step is: which workspace, agent, workflow and step the grants are matched against.
#[derive(Debug, Clone, Copy)]
pub struct McpContext<'a> {
    pub workspace: &'a str,
    pub agent: &'a str,
    pub workflow: Option<&'a str>,
    pub node: Option<&'a str>,
}

pub struct McpInputs<'a> {
    pub connections: &'a [McpConnection],
    pub grants: &'a [McpGrant],
    pub context: McpContext<'a>,
    /// The agent's resolved MCP policy.
    pub policy: Permission,
    pub support: McpSupport,
}

/// One connection as this step sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedServer {
    pub connection: McpConnection,
    /// What the matching grants cover together. `None`: no grant matches.
    pub selection: Option<ToolSelection>,
    /// Why it is not exposed. `None`: it is.
    pub problem: Option<McpProblem>,
}

impl PlannedServer {
    pub fn exposed(&self) -> bool {
        self.selection.is_some() && self.problem.is_none()
    }

    fn authorizes(&self, tool: &str) -> bool {
        match &self.selection {
            None => false,
            Some(ToolSelection::Server) => true,
            Some(ToolSelection::Only { tools }) => tools.iter().any(|t| t == tool),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct McpPlan {
    pub servers: Vec<PlannedServer>,
    /// The policy asks a person before a step is given MCP tools, and this one would be.
    pub requires_approval: bool,
    policy: Option<Permission>,
}

/// Several grants that match one step add up: the whole server if any says so, else the union.
fn combine(matching: &[&McpGrant]) -> Option<ToolSelection> {
    if matching.is_empty() {
        return None;
    }
    if matching.iter().any(|g| g.tools == ToolSelection::Server) {
        return Some(ToolSelection::Server);
    }
    let tools: BTreeSet<String> = matching
        .iter()
        .flat_map(|g| match &g.tools {
            ToolSelection::Only { tools } => tools.clone(),
            ToolSelection::Server => Vec::new(),
        })
        .collect();
    Some(ToolSelection::Only {
        tools: tools.into_iter().collect(),
    })
}

pub(super) fn covers(grant: &McpGrant, connection: &McpConnection, ctx: &McpContext<'_>) -> bool {
    grant.connection_id == connection.id
        && grant.agent_id.as_deref().is_none_or(|a| a == ctx.agent)
        && grant
            .workflow_id
            .as_deref()
            .is_none_or(|w| Some(w) == ctx.workflow)
        && grant.node_id.as_deref().is_none_or(|n| Some(n) == ctx.node)
}

pub fn plan(inputs: &McpInputs<'_>) -> McpPlan {
    let ctx = &inputs.context;
    let mut servers: Vec<PlannedServer> = inputs
        .connections
        .iter()
        .filter(|c| c.workspace_id == ctx.workspace)
        .map(|connection| {
            let matching: Vec<&McpGrant> = inputs
                .grants
                .iter()
                .filter(|g| covers(g, connection, ctx))
                .collect();
            let selection = combine(&matching);
            let problem = problem_of(connection, selection.as_ref(), inputs);
            PlannedServer {
                connection: connection.clone(),
                selection,
                problem,
            }
        })
        .collect();
    servers.sort_by(|a, b| a.connection.name.cmp(&b.connection.name));
    let requires_approval =
        inputs.policy == Permission::ApprovalRequired && servers.iter().any(PlannedServer::exposed);
    McpPlan {
        servers,
        requires_approval,
        policy: Some(inputs.policy),
    }
}

fn problem_of(
    connection: &McpConnection,
    selection: Option<&ToolSelection>,
    inputs: &McpInputs<'_>,
) -> Option<McpProblem> {
    if !connection.enabled {
        return Some(McpProblem::NotEnabled);
    }
    let selection = selection?;
    if inputs.policy == Permission::Denied {
        return Some(McpProblem::PolicyDenied);
    }
    if inputs.support != McpSupport::Supported {
        return Some(McpProblem::RuntimeUnsupported);
    }
    if let Err(invalid) = validate(connection) {
        return Some(McpProblem::InvalidConfiguration {
            reason: invalid.code().to_owned(),
        });
    }
    if matches!(selection, ToolSelection::Only { .. }) && connection.discovery.is_none() {
        return Some(McpProblem::NeedsDiscovery);
    }
    connection
        .missing_secret()
        .map(|name| McpProblem::SecretMissing {
            name: name.to_owned(),
        })
}

impl McpPlan {
    /// A connection that no grant covers is a connection; it is only a problem when someone
    /// granted it and it cannot be given.
    fn without_grant(server: &PlannedServer) -> bool {
        server.selection.is_none()
    }

    pub fn exposed(&self) -> impl Iterator<Item = &PlannedServer> {
        self.servers.iter().filter(|s| s.exposed())
    }

    /// Takes a server out of the plan after the fact, because it could not be started as planned
    /// (a credential vanished from the store): it is then not exposed, for that reason.
    pub fn exclude(&mut self, server: &str, problem: McpProblem) {
        if let Some(planned) = self
            .servers
            .iter_mut()
            .find(|s| s.connection.name == server)
        {
            planned.problem = Some(problem);
        }
    }

    /// Whether a tool the runtime listed for `server` is one a grant covers. A server that was not
    /// exposed authorizes nothing.
    pub fn authorizes_reported(&self, server: &str, tool: &str) -> bool {
        self.exposed()
            .find(|s| s.connection.name == server)
            .is_some_and(|s| s.authorizes(tool))
    }

    /// Granted, enabled and still not given, for a reason: the ones the guardrails must hear about.
    pub fn failed(&self) -> impl Iterator<Item = &PlannedServer> {
        self.servers.iter().filter(|s| {
            !Self::without_grant(s)
                && s.connection.enabled
                && s.problem.is_some()
                && !matches!(s.problem, Some(McpProblem::PolicyDenied))
        })
    }

    /// Discovered tools of an exposed server that no grant covers: held back so the runtime does
    /// not have them. (`server`, `tool`.)
    pub fn held_back(&self) -> Vec<(String, String)> {
        let mut held = Vec::new();
        for server in self.exposed() {
            if let Some(discovery) = &server.connection.discovery {
                for tool in &discovery.tools {
                    if !server.authorizes(tool) {
                        held.push((server.connection.name.clone(), tool.clone()));
                    }
                }
            }
        }
        held
    }

    #[cfg(test)]
    /// The tools the runtime may legitimately list for an exposed server, when Atlas can know:
    /// `None` for a whole-server grant (it covers what the server has now or adds later).
    pub fn allowed_tools(&self, server: &str) -> Option<Vec<String>> {
        let planned = self.exposed().find(|s| s.connection.name == server)?;
        match &planned.selection {
            Some(ToolSelection::Only { tools }) => Some(tools.clone()),
            _ => None,
        }
    }

    /// The six states of each tool Atlas knows of, for the manifest.
    pub fn tools(&self) -> Vec<McpToolState> {
        let mut states = Vec::new();
        for server in &self.servers {
            let name = &server.connection.name;
            let discovered: Vec<&String> = server
                .connection
                .discovery
                .as_ref()
                .map(|d| d.tools.iter().collect())
                .unwrap_or_default();
            let mut seen: BTreeSet<&str> = BTreeSet::new();
            let state = |tool: &str, discovered: bool| McpToolState {
                server: name.clone(),
                tool: tool.to_owned(),
                discovered,
                enabled: server.connection.enabled,
                authorized: server.authorizes(tool),
                // A tool the server does not have cannot be given, whatever a grant says.
                exposed: server.exposed() && server.authorizes(tool) && discovered,
                reported_exposed: None,
                used: None,
            };
            for tool in &discovered {
                seen.insert(tool);
                states.push(state(tool, true));
            }
            // A tool a grant names that the server does not have.
            if let Some(ToolSelection::Only { tools }) = &server.selection {
                for tool in tools.iter().filter(|t| !seen.contains(t.as_str())) {
                    states.push(state(tool, false));
                }
            }
        }
        states
    }

    pub fn record(&self) -> McpRecord {
        McpRecord {
            servers: self
                .servers
                .iter()
                .map(|s| McpServerRecord {
                    connection_id: s.connection.id.clone(),
                    name: s.connection.name.clone(),
                    enabled: s.connection.enabled,
                    required: s.connection.required,
                    authorized: s.selection.is_some(),
                    exposed: s.exposed(),
                    problem: s.problem.clone(),
                    discovered_status: s.connection.discovery.as_ref().map(|d| d.status),
                    reported_status: None,
                })
                .collect(),
            tools: self.tools(),
            held_back: self
                .held_back()
                .into_iter()
                .map(|(server, tool)| format!("{server}/{tool}"))
                .collect(),
            unauthorized: Vec::new(),
        }
    }

    /// Everything about the plan an approval must not outlive, as one stable text: the policy,
    /// each connection's configuration and credential versions, what each grant covers, what is
    /// held back and why a connection is not exposed.
    pub fn canonical(&self) -> String {
        let mut lines = vec![format!(
            "policy={:?}|approval={}",
            self.policy, self.requires_approval
        )];
        for server in &self.servers {
            let discovered = server
                .connection
                .discovery
                .as_ref()
                .map(|d| d.tools.join(","))
                .unwrap_or_default();
            lines.push(format!(
                "server|{}|{}|{}|selection={}|problem={}|discovered={}",
                server.connection.id,
                server.connection.name,
                server.connection.configuration_digest(),
                serde_json::to_string(&server.selection).unwrap_or_default(),
                serde_json::to_string(&server.problem).unwrap_or_default(),
                discovered
            ));
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::mcp::tests::connection;
    use crate::domain::mcp::{McpDiscovery, McpServerStatus};

    const CTX: McpContext<'static> = McpContext {
        workspace: "w1",
        agent: "a1",
        workflow: Some("wf1"),
        node: Some("n1"),
    };

    fn discovered(mut c: McpConnection, tools: &[&str]) -> McpConnection {
        c.discovery = Some(McpDiscovery {
            discovered_at: 1,
            runtime_id: "claude".to_owned(),
            status: McpServerStatus::Connected,
            tools: tools.iter().map(|t| (*t).to_owned()).collect(),
        });
        c
    }

    fn grant(connection: &McpConnection, tools: ToolSelection) -> McpGrant {
        McpGrant {
            id: format!("g-{}", connection.name),
            connection_id: connection.id.clone(),
            agent_id: None,
            workflow_id: None,
            node_id: None,
            tools,
        }
    }

    fn run(
        connections: &[McpConnection],
        grants: &[McpGrant],
        policy: Permission,
        support: McpSupport,
    ) -> McpPlan {
        plan(&McpInputs {
            connections,
            grants,
            context: CTX,
            policy,
            support,
        })
    }

    fn allowed(connections: &[McpConnection], grants: &[McpGrant]) -> McpPlan {
        run(
            connections,
            grants,
            Permission::Allowed,
            McpSupport::Supported,
        )
    }

    #[test]
    fn a_connection_nobody_granted_is_known_and_not_exposed() {
        let figma = connection("figma");

        let result = allowed(&[figma], &[]);

        assert_eq!(result.servers.len(), 1);
        assert_eq!(result.servers[0].problem, None);
        assert!(!result.servers[0].exposed());
        assert_eq!(result.exposed().count(), 0);
        // And it is not a failure the guardrails must hear about.
        assert_eq!(result.failed().count(), 0);
    }

    #[test]
    fn a_grant_to_an_enabled_connection_exposes_it() {
        let files = connection("files");
        let grants = [grant(&files, ToolSelection::Server)];

        let result = allowed(&[files], &grants);

        assert!(result.servers[0].exposed());
        assert_eq!(result.servers[0].selection, Some(ToolSelection::Server));
    }

    #[test]
    fn enabling_a_connection_grants_nothing_and_a_grant_enables_nothing() {
        let mut off = connection("files");
        off.enabled = false;
        let grants = [grant(&off, ToolSelection::Server)];

        let result = allowed(&[off], &grants);

        assert_eq!(result.servers[0].problem, Some(McpProblem::NotEnabled));
        assert!(!result.servers[0].exposed());
    }

    #[test]
    fn a_grant_covers_only_the_agent_workflow_and_step_it_names() {
        let files = connection("files");
        let mut for_other_agent = grant(&files, ToolSelection::Server);
        for_other_agent.agent_id = Some("a2".to_owned());
        let mut other_workflow = grant(&files, ToolSelection::Server);
        other_workflow.workflow_id = Some("wf2".to_owned());
        let mut other_step = grant(&files, ToolSelection::Server);
        other_step.node_id = Some("n2".to_owned());
        for none in [for_other_agent, other_workflow, other_step] {
            assert!(!allowed(std::slice::from_ref(&files), &[none]).servers[0].exposed());
        }

        let mut exact = grant(&files, ToolSelection::Server);
        exact.agent_id = Some("a1".to_owned());
        exact.workflow_id = Some("wf1".to_owned());
        exact.node_id = Some("n1".to_owned());
        assert!(allowed(&[files], &[exact]).servers[0].exposed());
    }

    #[test]
    fn another_workspaces_connection_is_not_even_listed() {
        let mut elsewhere = connection("files");
        elsewhere.workspace_id = "w2".to_owned();
        let grants = [grant(&elsewhere, ToolSelection::Server)];

        assert_eq!(allowed(&[elsewhere], &grants).servers.len(), 0);
    }

    #[test]
    fn the_policy_decides_whether_a_grant_counts() {
        let files = connection("files");
        let grants = [grant(&files, ToolSelection::Server)];

        let denied = run(
            std::slice::from_ref(&files),
            &grants,
            Permission::Denied,
            McpSupport::Supported,
        );
        assert_eq!(denied.servers[0].problem, Some(McpProblem::PolicyDenied));
        assert!(!denied.servers[0].exposed());
        // A denial by policy is not a failure to report; it is the policy working.
        assert_eq!(denied.failed().count(), 0);

        let asks = run(
            &[files],
            &grants,
            Permission::ApprovalRequired,
            McpSupport::Supported,
        );
        assert!(asks.servers[0].exposed());
        assert!(asks.requires_approval);
    }

    #[test]
    fn a_policy_that_asks_does_not_ask_when_nothing_would_be_exposed() {
        let files = connection("files");

        let result = run(
            &[files],
            &[],
            Permission::ApprovalRequired,
            McpSupport::Supported,
        );

        assert!(!result.requires_approval);
    }

    #[test]
    fn a_runtime_without_an_adapter_gets_nothing_whether_unsupported_or_not_investigated() {
        let files = connection("files");
        let grants = [grant(&files, ToolSelection::Server)];

        for support in [McpSupport::Unsupported, McpSupport::NotInvestigated] {
            let result = run(
                std::slice::from_ref(&files),
                &grants,
                Permission::Allowed,
                support,
            );
            assert_eq!(
                result.servers[0].problem,
                Some(McpProblem::RuntimeUnsupported)
            );
            assert!(!result.servers[0].exposed());
        }
    }

    #[test]
    fn a_configuration_edited_by_hand_into_something_invalid_is_not_exposed() {
        let mut bad = connection("files");
        bad.transport = crate::domain::mcp::McpTransport::Stdio {
            executable: "bash".to_owned(),
            args: vec!["-c".to_owned(), "x".to_owned()],
            env: vec![],
        };
        let grants = [grant(&bad, ToolSelection::Server)];

        let result = allowed(&[bad], &grants);

        assert_eq!(
            result.servers[0].problem,
            Some(McpProblem::InvalidConfiguration {
                reason: "shell".to_owned()
            })
        );
    }

    #[test]
    fn a_missing_secret_keeps_a_connection_out() {
        let mut api = connection("api");
        api.transport = crate::domain::mcp::McpTransport::Stdio {
            executable: "node".to_owned(),
            args: vec![],
            env: vec![crate::domain::mcp::McpEnv {
                name: "API_TOKEN".to_owned(),
                value: crate::domain::mcp::McpEnvValue::Secret,
            }],
        };
        let grants = [grant(&api, ToolSelection::Server)];

        let without = allowed(&[api.clone()], &grants);
        assert_eq!(
            without.servers[0].problem,
            Some(McpProblem::SecretMissing {
                name: "API_TOKEN".to_owned()
            })
        );
        assert_eq!(without.failed().count(), 1);

        api.secrets.insert("API_TOKEN".to_owned(), 5);
        assert!(allowed(&[api], &grants).servers[0].exposed());
    }

    #[test]
    fn named_tools_need_a_discovery_so_the_rest_can_be_held_back() {
        let files = connection("files");
        let only = ToolSelection::Only {
            tools: vec!["read".to_owned()],
        };

        let blind = allowed(std::slice::from_ref(&files), &[grant(&files, only.clone())]);
        assert_eq!(blind.servers[0].problem, Some(McpProblem::NeedsDiscovery));

        let known = discovered(files, &["read", "write", "delete"]);
        let result = allowed(std::slice::from_ref(&known), &[grant(&known, only)]);
        assert!(result.servers[0].exposed());
        assert_eq!(
            result.held_back(),
            [
                ("files".to_owned(), "write".to_owned()),
                ("files".to_owned(), "delete".to_owned())
            ]
        );
        assert_eq!(result.allowed_tools("files"), Some(vec!["read".to_owned()]));
    }

    #[test]
    fn a_whole_server_grant_holds_nothing_back_and_allows_whatever_the_server_has() {
        let files = discovered(connection("files"), &["read", "write"]);

        let result = allowed(
            std::slice::from_ref(&files),
            &[grant(&files, ToolSelection::Server)],
        );

        assert_eq!(result.held_back().len(), 0);
        assert_eq!(result.allowed_tools("files"), None);
    }

    #[test]
    fn grants_that_match_together_add_up() {
        let files = discovered(connection("files"), &["a", "b", "c"]);
        let one = grant(
            &files,
            ToolSelection::Only {
                tools: vec!["a".to_owned()],
            },
        );
        let two = McpGrant {
            id: "g2".to_owned(),
            tools: ToolSelection::Only {
                tools: vec!["b".to_owned()],
            },
            ..one.clone()
        };

        let result = allowed(std::slice::from_ref(&files), &[one.clone(), two.clone()]);
        assert_eq!(
            result.allowed_tools("files"),
            Some(vec!["a".to_owned(), "b".to_owned()])
        );

        let whole = McpGrant {
            id: "g3".to_owned(),
            tools: ToolSelection::Server,
            ..one
        };
        assert_eq!(
            allowed(&[files], &[two, whole]).servers[0].selection,
            Some(ToolSelection::Server)
        );
    }

    #[test]
    fn each_tool_says_all_six_things_apart() {
        let files = discovered(connection("files"), &["read", "write"]);
        let grants = [grant(
            &files,
            ToolSelection::Only {
                tools: vec!["read".to_owned(), "ghost".to_owned()],
            },
        )];

        let tools = allowed(&[files], &grants).tools();

        let state = |name: &str| tools.iter().find(|t| t.tool == name).unwrap();
        // Discovered, enabled, authorized and exposed.
        let read = state("read");
        assert!(read.discovered && read.enabled && read.authorized && read.exposed);
        assert_eq!((read.reported_exposed, read.used), (None, None));
        // Discovered and enabled, but not authorized, so not exposed.
        let write = state("write");
        assert!(write.discovered && write.enabled && !write.authorized && !write.exposed);
        // Authorized by a grant but the server does not have it.
        let ghost = state("ghost");
        assert!(!ghost.discovered && ghost.authorized && !ghost.exposed);
    }

    #[test]
    fn the_plan_is_deterministic_whatever_the_order_of_the_inputs() {
        let a = connection("alpha");
        let b = connection("beta");
        let grants = [
            grant(&a, ToolSelection::Server),
            grant(&b, ToolSelection::Server),
        ];

        let forward = allowed(&[a.clone(), b.clone()], &grants);
        let backward = allowed(&[b, a], &[grants[1].clone(), grants[0].clone()]);

        assert_eq!(forward.canonical(), backward.canonical());
        assert_eq!(forward, backward);
    }

    #[test]
    fn the_canonical_text_changes_with_anything_an_approval_depended_on() {
        let files = discovered(connection("files"), &["read"]);
        let grants = [grant(&files, ToolSelection::Server)];
        let base = allowed(std::slice::from_ref(&files), &grants).canonical();

        let mut edited = files.clone();
        edited.transport = crate::domain::mcp::McpTransport::Stdio {
            executable: "node".to_owned(),
            args: vec!["other.js".to_owned()],
            env: vec![],
        };
        let mut new_credential = files.clone();
        new_credential.secrets.insert("TOKEN".to_owned(), 9);
        let mut grew = files.clone();
        grew.discovery
            .as_mut()
            .unwrap()
            .tools
            .push("write".to_owned());
        let mut off = files.clone();
        off.enabled = false;

        for changed in [edited, new_credential, grew, off] {
            assert_ne!(base, allowed(&[changed], &grants).canonical());
        }
        // A narrower grant, or none.
        let narrower = [grant(
            &files,
            ToolSelection::Only {
                tools: vec!["read".to_owned()],
            },
        )];
        assert_ne!(
            base,
            allowed(std::slice::from_ref(&files), &narrower).canonical()
        );
        assert_ne!(base, allowed(std::slice::from_ref(&files), &[]).canonical());
        // The policy, too.
        assert_ne!(
            base,
            run(
                &[files],
                &grants,
                Permission::ApprovalRequired,
                McpSupport::Supported
            )
            .canonical()
        );
    }

    #[test]
    fn the_record_says_what_each_connection_is_and_why_not() {
        let files = connection("files");
        let mut off = connection("zeta");
        off.enabled = false;

        let record = allowed(
            &[files.clone(), off],
            &[grant(&files, ToolSelection::Server)],
        )
        .record();

        assert_eq!(record.servers[0].name, "files");
        assert!(record.servers[0].authorized && record.servers[0].exposed);
        assert_eq!(record.servers[1].problem, Some(McpProblem::NotEnabled));
        assert!(!record.servers[1].authorized);
    }
}
