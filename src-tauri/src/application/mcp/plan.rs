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
use crate::domain::mcp::reads_tool;
use crate::domain::mcp::{
    McpConnection, McpFeatures, McpGrant, McpProblem, McpRecord, McpServerRecord, McpSupport,
    McpToolFilter, McpToolState, McpTransport, ToolSelection,
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

/// A server a CLI already has in its own configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliServer {
    pub name: String,
    /// The program and arguments it starts, as text.
    pub command_line: String,
}

/// What a name comes to when a CLI and a person each wrote it their way: `chrome-devtools-mcp`,
/// `Chrome_DevTools` and `mcp-chrome-devtools` are one.
fn name_stem(name: &str) -> String {
    let lower = name.to_lowercase().replace('_', "-");
    let lower = lower.strip_prefix("mcp-").unwrap_or(&lower);
    let lower = lower.strip_suffix("-server").unwrap_or(lower);
    lower.strip_suffix("-mcp").unwrap_or(lower).to_owned()
}

/// The package or script a connection starts, without its version: `chrome-devtools-mcp` for
/// `npx -y chrome-devtools-mcp@1.10.1`.
fn program_token(connection: &McpConnection) -> Option<String> {
    let McpTransport::Stdio {
        executable, args, ..
    } = &connection.transport
    else {
        return None;
    };
    let launcher = matches!(
        executable.rsplit(['/', '\\']).next().unwrap_or(""),
        "npx" | "uvx" | "bunx" | "pnpx" | "pipx"
    );
    let token = if launcher {
        args.iter().find(|a| !a.starts_with('-'))?
    } else {
        executable
    };
    // `@scope/name@1.2` keeps its scope; the version after the last `@` goes.
    let trimmed = match token.rfind('@') {
        Some(at) if at > 0 => &token[..at],
        _ => token.as_str(),
    };
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// The CLI's own server that is this connection, if it has one: by name, or by starting the same
/// package.
pub fn cli_match<'a>(
    connection: &McpConnection,
    servers: &'a [CliServer],
) -> Option<&'a CliServer> {
    let stem = name_stem(&connection.name);
    let token = program_token(connection);
    servers.iter().find(|server| {
        name_stem(&server.name) == stem
            || token
                .as_deref()
                .is_some_and(|t| server.command_line.contains(t))
    })
}

pub struct McpInputs<'a> {
    pub connections: &'a [McpConnection],
    pub grants: &'a [McpGrant],
    pub context: McpContext<'a>,
    /// The agent's resolved MCP policy.
    pub policy: Permission,
    pub support: McpSupport,
    /// The servers the runtime's CLI has in its own configuration; `Some` only for
    /// `McpSupport::CliConfigured`.
    pub cli_servers: Option<&'a [CliServer]>,
    /// How the runtime filters tools (what a grant naming tools needs).
    pub features: McpFeatures,
    /// The step may not change anything. A server Atlas asked for its tools' descriptions is then
    /// held to the ones that only read (ADR 0045); one never asked is given as before.
    pub read_only: bool,
}

/// One connection as this step sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedServer {
    pub connection: McpConnection,
    /// What the matching grants cover together. `None`: no grant matches.
    pub selection: Option<ToolSelection>,
    /// Why it is not exposed. `None`: it is.
    pub problem: Option<McpProblem>,
    /// The CLI's own name for the server when its own copy is what the agent uses.
    pub cli_name: Option<String>,
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
    /// The runtime exposes only the tools named: a tool is exposed without having been discovered.
    allow_list: bool,
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

/// What a read-only step may have of `selection`: the tools that only read, by what the server
/// said about them ([`reads_tool`]). A server Atlas never asked keeps its selection: the person did
/// not ask for its descriptions, and nothing about the step changes until they do.
fn reading_only(connection: &McpConnection, selection: ToolSelection) -> ToolSelection {
    let Some(catalog) = &connection.catalog else {
        return selection;
    };
    let candidates: Vec<String> = match selection {
        ToolSelection::Only { tools } => tools,
        ToolSelection::Server => {
            let mut all: BTreeSet<String> = catalog.tools.iter().map(|t| t.name.clone()).collect();
            if let Some(discovery) = &connection.discovery {
                all.extend(discovery.tools.iter().cloned());
            }
            all.into_iter().collect()
        }
    };
    ToolSelection::Only {
        tools: candidates
            .into_iter()
            .filter(|tool| reads_tool(tool, catalog.tool(tool)))
            .collect(),
    }
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
            let selection = if inputs.read_only {
                selection.map(|s| reading_only(connection, s))
            } else {
                selection
            };
            let cli = (inputs.support == McpSupport::CliConfigured)
                .then(|| cli_match(connection, inputs.cli_servers.unwrap_or_default()))
                .flatten();
            let problem = problem_of(connection, selection.as_ref(), cli.is_some(), inputs);
            PlannedServer {
                connection: connection.clone(),
                selection,
                problem,
                cli_name: cli.map(|c| c.name.clone()),
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
        allow_list: inputs.features.tool_filter == McpToolFilter::AllowList,
    }
}

fn problem_of(
    connection: &McpConnection,
    selection: Option<&ToolSelection>,
    in_cli: bool,
    inputs: &McpInputs<'_>,
) -> Option<McpProblem> {
    if !connection.enabled {
        return Some(McpProblem::NotEnabled);
    }
    let selection = selection?;
    if inputs.policy == Permission::Denied {
        return Some(McpProblem::PolicyDenied);
    }
    if inputs.read_only
        && connection.catalog.is_some()
        && matches!(selection, ToolSelection::Only { tools } if tools.is_empty())
    {
        return Some(McpProblem::NoReadingTools);
    }
    match inputs.support {
        McpSupport::Supported => {}
        McpSupport::CliConfigured if in_cli => {}
        McpSupport::CliConfigured => return Some(McpProblem::NotInCliConfig),
        McpSupport::Unsupported | McpSupport::NotInvestigated => {
            return Some(McpProblem::RuntimeUnsupported)
        }
    }
    if let Err(invalid) = validate(connection) {
        return Some(McpProblem::InvalidConfiguration {
            reason: invalid.code().to_owned(),
        });
    }
    if matches!(selection, ToolSelection::Only { .. }) {
        match inputs.features.tool_filter {
            // The rest cannot be held back: better not to give the server at all.
            McpToolFilter::Unsupported => return Some(McpProblem::ToolFilterUnsupported),
            // The tools are the ones a discovery listed.
            McpToolFilter::DenyList if connection.discovery.is_none() => {
                return Some(McpProblem::NeedsDiscovery)
            }
            McpToolFilter::DenyList | McpToolFilter::AllowList => {}
        }
    }
    // The CLI's own copy has its own credentials: Atlas hands it none.
    if in_cli {
        return None;
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

    /// What the definitions of the MCP tools this step is given weigh, as Atlas estimates it
    /// (`chars / 4` of what each server said about its tools). Unknown unless every server given
    /// was described and every tool it is given is in the description: Atlas does not guess the
    /// rest, and a step given no MCP at all says nothing either.
    pub fn tool_definitions(&self) -> crate::domain::context::Figure {
        use crate::domain::context::Figure;
        let held = self.held_back();
        let mut chars: u64 = 0;
        let mut any = false;
        for server in self.exposed() {
            any = true;
            let Some(catalog) = server
                .connection
                .catalog
                .as_ref()
                .filter(|_| server.cli_name.is_none())
            else {
                return Figure::unknown();
            };
            let names: Vec<&str> = match &server.selection {
                Some(ToolSelection::Only { tools }) => tools.iter().map(String::as_str).collect(),
                _ if catalog.truncated => return Figure::unknown(),
                _ => catalog
                    .tools
                    .iter()
                    .map(|t| t.name.as_str())
                    .filter(|t| {
                        !held
                            .iter()
                            .any(|(s, h)| s == &server.connection.name && h == t)
                    })
                    .collect(),
            };
            for name in names {
                match catalog.tool(name) {
                    Some(info) => chars += info.definition_chars,
                    None => return Figure::unknown(),
                }
            }
        }
        if !any {
            return Figure::unknown();
        }
        Figure::estimated_from_chars(usize::try_from(chars).unwrap_or(usize::MAX))
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
                exposed: server.exposed()
                    && server.authorizes(tool)
                    && (discovered || self.allow_list),
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
                    cli_name: s.cli_name.clone(),
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
                "server|{}|{}|{}|selection={}|problem={}|discovered={}|cli={}",
                server.connection.id,
                server.connection.name,
                server.connection.configuration_digest(),
                serde_json::to_string(&server.selection).unwrap_or_default(),
                serde_json::to_string(&server.problem).unwrap_or_default(),
                discovered,
                server.cli_name.as_deref().unwrap_or("")
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
        run_as(connections, grants, policy, support, false)
    }

    fn run_as(
        connections: &[McpConnection],
        grants: &[McpGrant],
        policy: Permission,
        support: McpSupport,
        read_only: bool,
    ) -> McpPlan {
        plan(&McpInputs {
            connections,
            grants,
            context: CTX,
            policy,
            cli_servers: None,
            support,
            features: crate::domain::mcp::McpFeatures {
                tool_filter: McpToolFilter::DenyList,
                probe: crate::domain::mcp::McpProbeKind::Tools,
                strict: false,
            },
            read_only,
        })
    }

    fn described(mut c: McpConnection, tools: &[(&str, Option<bool>)]) -> McpConnection {
        c.catalog = Some(crate::domain::mcp::McpToolCatalog {
            described_at: 2,
            tools: tools
                .iter()
                .map(|(name, read_only)| crate::domain::mcp::McpToolInfo {
                    name: (*name).to_owned(),
                    title: None,
                    description: format!("{name} does things"),
                    read_only: *read_only,
                    destructive: None,
                    definition_chars: 40,
                })
                .collect(),
            truncated: false,
        });
        c
    }

    fn read_only_step(connections: &[McpConnection], grants: &[McpGrant]) -> McpPlan {
        run_as(
            connections,
            grants,
            Permission::Allowed,
            McpSupport::Supported,
            true,
        )
    }

    #[test]
    fn a_read_only_step_is_given_only_the_tools_a_described_server_says_only_read() {
        let files = described(
            discovered(connection("files"), &["read", "take_snapshot", "write"]),
            &[
                ("read", Some(true)),
                ("take_snapshot", Some(true)),
                ("write", Some(false)),
            ],
        );
        let grants = [grant(&files, ToolSelection::Server)];

        let narrowed = read_only_step(std::slice::from_ref(&files), &grants);

        assert_eq!(
            narrowed.servers[0].selection,
            Some(ToolSelection::Only {
                tools: vec!["read".to_owned(), "take_snapshot".to_owned()]
            })
        );
        assert_eq!(
            narrowed.held_back(),
            [("files".to_owned(), "write".to_owned())]
        );
        // The same grant in a step that may write: the whole server, as before.
        let writing = allowed(&[files], &grants);
        assert_eq!(writing.servers[0].selection, Some(ToolSelection::Server));
        assert_eq!(
            writing.held_back(),
            [] as [(std::string::String, std::string::String); 0]
        );
    }

    #[test]
    fn the_tool_definitions_a_step_is_given_are_estimated_only_when_all_are_described() {
        let files = described(
            discovered(connection("files"), &["read", "write"]),
            &[("read", Some(true)), ("write", Some(false))],
        );
        let whole = [grant(&files, ToolSelection::Server)];
        // Two definitions of 40 characters: 80 / 4.
        let figure = allowed(std::slice::from_ref(&files), &whole).tool_definitions();
        assert_eq!(figure.value, Some(20));
        assert_eq!(
            figure.precision,
            crate::domain::context::Precision::Estimated
        );
        // A read-only step is given one of them.
        assert_eq!(
            read_only_step(std::slice::from_ref(&files), &whole)
                .tool_definitions()
                .value,
            Some(10)
        );
        // A tool the description does not have: unknown, not a guess.
        let named = [grant(
            &files,
            ToolSelection::Only {
                tools: vec!["read".to_owned(), "other".to_owned()],
            },
        )];
        assert!(!allowed(std::slice::from_ref(&files), &named)
            .tool_definitions()
            .is_known());
        // A server never described, or no MCP at all: unknown.
        let plain = discovered(connection("plain"), &["read"]);
        let plain_grants = [grant(&plain, ToolSelection::Server)];
        assert!(!allowed(&[plain], &plain_grants)
            .tool_definitions()
            .is_known());
        assert!(!allowed(&[files], &[]).tool_definitions().is_known());
    }

    #[test]
    fn a_server_atlas_never_asked_is_given_to_a_read_only_step_as_before() {
        let files = discovered(connection("files"), &["read", "write"]);
        let grants = [grant(&files, ToolSelection::Server)];

        assert_eq!(
            read_only_step(std::slice::from_ref(&files), &grants),
            allowed(&[files], &grants)
        );
    }

    #[test]
    fn a_read_only_step_narrows_a_named_grant_and_never_widens_it() {
        let files = described(
            discovered(connection("files"), &["read", "list", "write"]),
            &[
                ("read", Some(true)),
                ("list", Some(true)),
                ("write", Some(false)),
            ],
        );
        let grants = [grant(
            &files,
            ToolSelection::Only {
                tools: vec!["read".to_owned(), "write".to_owned()],
            },
        )];

        let narrowed = read_only_step(&[files], &grants);

        assert_eq!(
            narrowed.servers[0].selection,
            Some(ToolSelection::Only {
                tools: vec!["read".to_owned()]
            })
        );
    }

    #[test]
    fn a_described_server_with_nothing_that_only_reads_is_not_given_to_a_read_only_step() {
        let files = described(
            discovered(connection("files"), &["write"]),
            &[("write", Some(false))],
        );
        let grants = [grant(&files, ToolSelection::Server)];

        let narrowed = read_only_step(&[files], &grants);

        assert!(!narrowed.servers[0].exposed());
        assert_eq!(
            narrowed.servers[0].problem,
            Some(McpProblem::NoReadingTools)
        );
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

    fn devtools() -> McpConnection {
        let mut c = connection("chrome-devtools");
        c.transport = McpTransport::Stdio {
            executable: "npx".to_owned(),
            args: vec![
                "-y".to_owned(),
                "chrome-devtools-mcp@1.10.1".to_owned(),
                "--headless".to_owned(),
            ],
            env: Vec::new(),
        };
        c
    }

    fn cli(name: &str, command: &str) -> CliServer {
        CliServer {
            name: name.to_owned(),
            command_line: command.to_owned(),
        }
    }

    fn through_cli(
        connection: McpConnection,
        grants: &[McpGrant],
        servers: &[CliServer],
        filter: McpToolFilter,
    ) -> McpPlan {
        plan(&McpInputs {
            connections: &[connection],
            grants,
            context: CTX,
            policy: Permission::Allowed,
            support: McpSupport::CliConfigured,
            cli_servers: Some(servers),
            features: McpFeatures {
                tool_filter: filter,
                probe: crate::domain::mcp::McpProbeKind::None,
                strict: false,
            },
            read_only: false,
        })
    }

    #[test]
    fn a_cli_that_has_the_server_in_its_own_configuration_gets_it_by_the_name_it_knows() {
        let connection = devtools();
        let grants = [grant(&connection, ToolSelection::Server)];
        // Another name, the same package.
        let servers = [cli(
            "chrome-devtools-mcp",
            "npx -y chrome-devtools-mcp@latest",
        )];

        let result = through_cli(connection, &grants, &servers, McpToolFilter::Unsupported);

        assert!(result.servers[0].exposed());
        assert_eq!(
            result.servers[0].cli_name.as_deref(),
            Some("chrome-devtools-mcp")
        );
    }

    #[test]
    fn a_cli_without_it_is_told_so_and_a_named_tool_grant_is_refused_for_a_cli_that_cannot_hold_tools(
    ) {
        let connection = devtools();
        let whole = [grant(&connection, ToolSelection::Server)];
        let only = [grant(
            &connection,
            ToolSelection::Only {
                tools: vec!["take_screenshot".to_owned()],
            },
        )];
        let other = [cli("figma", "http://127.0.0.1:3845/mcp")];
        let has = [cli(
            "chrome-devtools-mcp",
            "npx -y chrome-devtools-mcp@latest",
        )];

        let missing = through_cli(
            connection.clone(),
            &whole,
            &other,
            McpToolFilter::Unsupported,
        );
        let narrowed = through_cli(connection, &only, &has, McpToolFilter::Unsupported);

        assert_eq!(missing.servers[0].problem, Some(McpProblem::NotInCliConfig));
        assert!(!missing.servers[0].exposed());
        assert_eq!(
            narrowed.servers[0].problem,
            Some(McpProblem::ToolFilterUnsupported)
        );
    }

    #[test]
    fn the_same_name_in_another_spelling_is_the_same_server_and_another_server_is_not() {
        assert_eq!(
            name_stem("Chrome_DevTools"),
            name_stem("mcp-chrome-devtools")
        );
        assert_eq!(name_stem("chrome-devtools-mcp"), "chrome-devtools");
        assert_ne!(name_stem("github"), name_stem("gitlab"));
        assert_eq!(
            program_token(&devtools()).as_deref(),
            Some("chrome-devtools-mcp")
        );
        let mut scoped = connection("x");
        scoped.transport = McpTransport::Stdio {
            executable: "npx".to_owned(),
            args: vec!["@acme/jira-mcp@1.2.0".to_owned()],
            env: Vec::new(),
        };
        assert_eq!(program_token(&scoped).as_deref(), Some("@acme/jira-mcp"));
    }
}
