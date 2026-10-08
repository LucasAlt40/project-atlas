//! MCP in Atlas's own words. Nothing here names a runtime, a CLI flag or a config format: a
//! runtime adapter translates this to whatever its tool understands (`application/runtimes`).
//!
//! The vocabulary the rest of the platform needs:
//!
//! ```text
//! Workspace → McpConnection ("this workspace knows this integration")
//!           → McpGrant      ("this agent / workflow / step may use these tools")
//!           → effective tool set, per step (application/mcp/plan.rs)
//! ```
//!
//! A connection is **not** an authorization. Six things are kept apart and never folded into one
//! flag (see [`McpToolState`]): the server was discovered, the tool was discovered, it is enabled,
//! it is authorized for this step, it is exposed to the runtime, it was used.
//!
//! No secret value is ever in these types: an environment variable that holds one is declared as
//! [`McpEnvValue::Secret`], and the value lives in the OS credential store.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// How Atlas reaches an MCP server. STDIO is the only transport implemented; HTTP is part of the
/// contract so the model does not change when it arrives, and it is refused until its own layer
/// exists (destination and redirect validation, private networks, TLS, authentication).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum McpTransport {
    /// A local process the user authorizes: an executable and its arguments, kept apart. Never a
    /// shell string.
    Stdio {
        executable: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: Vec<McpEnv>,
    },
    /// Reserved. Not implemented, and refused by validation.
    Http { url: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpEnv {
    pub name: String,
    pub value: McpEnvValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum McpEnvValue {
    /// A value that is not a secret, kept in the configuration.
    Plain { value: String },
    /// The value is in the credential store, under this connection and this name.
    Secret,
}

/// What a connection last looked like when it was tried.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpServerStatus {
    Connected,
    Failed,
    NeedsAuth,
    Pending,
    /// The runtime said something Atlas has no word for.
    Unknown,
}

/// What a runtime reported about a server when it started it: the tools it listed. Names only:
/// the runtimes' start-up reports do not include descriptions or schemas, and Atlas does not
/// invent them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpDiscovery {
    /// Milliseconds since the Unix epoch.
    pub discovered_at: u64,
    pub runtime_id: String,
    pub status: McpServerStatus,
    pub tools: Vec<String>,
}

/// An integration a workspace knows. Saved with the configuration: it contains no secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpConnection {
    pub id: String,
    pub workspace_id: String,
    /// The server's name as agents and the audit trail know it: `[a-z0-9_-]`, short.
    pub name: String,
    pub transport: McpTransport,
    /// Off until the user turns it on. Turning it on authorizes nothing: grants do that.
    #[serde(default)]
    pub enabled: bool,
    /// A step that is granted this connection cannot run without it. Otherwise a connection that
    /// cannot be started is left out, and the step goes on without its tools.
    #[serde(default)]
    pub required: bool,
    /// When each secret was last stored (milliseconds), by environment variable name. Never the
    /// secrets; enough to tell a changed credential from an unchanged one.
    #[serde(default)]
    pub secrets: BTreeMap<String, u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovery: Option<McpDiscovery>,
    pub created_at: u64,
}

impl McpConnection {
    /// The environment variables of a STDIO connection that hold secrets.
    pub fn secret_names(&self) -> Vec<&str> {
        match &self.transport {
            McpTransport::Stdio { env, .. } => env
                .iter()
                .filter(|e| matches!(e.value, McpEnvValue::Secret))
                .map(|e| e.name.as_str())
                .collect(),
            McpTransport::Http { .. } => Vec::new(),
        }
    }

    /// The first secret the connection needs and does not have.
    pub fn missing_secret(&self) -> Option<&str> {
        self.secret_names()
            .into_iter()
            .find(|name| !self.secrets.contains_key(*name))
    }

    /// Everything about the connection an approval must not outlive, as text: the transport
    /// (secrets as names only), whether it is enabled and required, and when each secret was
    /// last stored. Not the secrets themselves, and not the discovery.
    pub fn configuration_digest(&self) -> String {
        format!(
            "{}|enabled={}|required={}|secrets={:?}",
            serde_json::to_string(&self.transport).unwrap_or_default(),
            self.enabled,
            self.required,
            self.secrets
        )
    }
}

/// Which tools of a connection a grant covers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolSelection {
    /// Every tool the server has, now or later. Authorization at the server's granularity: what
    /// the server adds later is covered by the user's decision to trust the server.
    Server,
    /// Only these. Needs the server to have been discovered, so the others can be held back; a
    /// tool the server adds afterwards is not on the list and is not authorized.
    Only { tools: Vec<String> },
}

/// Who may use a connection, and which of its tools. The more fields are set, the narrower it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpGrant {
    pub id: String,
    pub connection_id: String,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub workflow_id: Option<String>,
    /// A step of a workflow.
    #[serde(default)]
    pub node_id: Option<String>,
    pub tools: ToolSelection,
}

/// What a runtime can do with MCP as Atlas drives it. A runtime nobody has measured is not
/// "unsupported": it is not investigated.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpSupport {
    Unsupported,
    #[default]
    NotInvestigated,
    /// An adapter exists and its behaviour was measured.
    Supported,
}

/// How a runtime can keep a granted server's other tools from the model. Measured per runtime;
/// what was only read in a CLI's configuration reference is marked so in the runtime matrix.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpToolFilter {
    /// No way found: a grant that names tools cannot be honoured, so the server is not given.
    #[default]
    Unsupported,
    /// The runtime hides tools Atlas names. Needs the tools to be known (a discovery), and a tool
    /// the server adds later is not hidden.
    DenyList,
    /// The runtime exposes only the tools Atlas names. Needs no discovery and fails closed.
    AllowList,
}

/// What looking at a server through a runtime shows, without asking any model.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpProbeKind {
    #[default]
    None,
    /// Whether the server started and connected; its tools are not listed.
    StatusOnly,
    /// Status and the names of its tools.
    Tools,
}

/// The finer facts about a runtime that has MCP support.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpFeatures {
    pub tool_filter: McpToolFilter,
    pub probe: McpProbeKind,
    /// The runtime loads **only** the servers Atlas gives it (measured). `false`: it merges them
    /// with whatever the user configured in the runtime itself, which Atlas cannot see or hide.
    pub strict: bool,
}

/// Why a connection is not exposed to a step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum McpProblem {
    /// Known to the workspace, switched off.
    NotEnabled,
    /// Enabled, but no grant covers this agent, workflow and step.
    NoGrant,
    /// The agent's policy does not allow MCP.
    PolicyDenied,
    /// The agent's runtime has no MCP adapter (or it was not investigated).
    RuntimeUnsupported,
    /// The configuration is not valid (and why, in a short code).
    InvalidConfiguration { reason: String },
    /// A secret the connection needs was never stored.
    #[serde(rename_all = "camelCase")]
    SecretMissing { name: String },
    /// A grant names tools, but the server was never discovered, so the rest cannot be held back.
    NeedsDiscovery,
    /// A grant names tools and the runtime has no way to hold the others back.
    ToolFilterUnsupported,
}

/// The six things kept apart for one tool.
#[allow(clippy::struct_excessive_bools)] // six facts, kept apart on purpose `exposed` is what Atlas arranged; `reported` is what
/// the runtime said afterwards (`None`: it said nothing, which is not "no").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolState {
    pub server: String,
    pub tool: String,
    /// The server is known to have it (a discovery listed it).
    pub discovered: bool,
    /// The connection is switched on.
    pub enabled: bool,
    /// A grant covers it for this step.
    pub authorized: bool,
    /// Atlas arranged for the runtime to have it.
    pub exposed: bool,
    /// The runtime listed it among its tools after it started.
    pub reported_exposed: Option<bool>,
    /// The runtime reported the model called it (`None`: no report).
    pub used: Option<bool>,
}

/// A server as one step sees it.
#[allow(clippy::struct_excessive_bools)] // enabled, required, authorized, exposed: not one flag
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerRecord {
    pub connection_id: String,
    pub name: String,
    pub enabled: bool,
    pub required: bool,
    /// A grant covers it for this step.
    pub authorized: bool,
    /// Atlas arranged for the runtime to start it.
    pub exposed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub problem: Option<McpProblem>,
    /// The status of the last discovery.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovered_status: Option<McpServerStatus>,
    /// The status the runtime reported when this step started it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reported_status: Option<McpServerStatus>,
}

/// What a step did with MCP, for the manifest.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpRecord {
    pub servers: Vec<McpServerRecord>,
    pub tools: Vec<McpToolState>,
    /// The tool names (as the runtime calls them) held back with a deny-list.
    #[serde(default)]
    pub held_back: Vec<String>,
    /// Tools the runtime reported that Atlas did not authorize: the step is stopped and failed.
    #[serde(default)]
    pub unauthorized: Vec<String>,
}

/// An MCP tool a runtime listed (or a model called). `server` is `None` when it belongs to no
/// server Atlas launched for this step: something the runtime loaded on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportedMcpTool {
    pub server: Option<String>,
    pub tool: String,
}

impl McpRecord {
    /// Adds what the runtime said once the step ran. Each tool's `reported_exposed` and `used`
    /// become `Some` only if the runtime reported at all (no report is not "no"); a reported tool
    /// Atlas had no state for is added, and each server learns the status the runtime gave it.
    /// `authorized` says whether a grant covers a (server, tool) pair.
    pub fn observe(
        &mut self,
        reported: Option<&[ReportedMcpTool]>,
        statuses: &BTreeMap<String, McpServerStatus>,
        used: Option<&[ReportedMcpTool]>,
        authorized: &dyn Fn(&str, &str) -> bool,
    ) {
        let has = |list: &[ReportedMcpTool], server: &str, tool: &str| {
            list.iter()
                .any(|t| t.server.as_deref() == Some(server) && t.tool == tool)
        };
        for server in &mut self.servers {
            server.reported_status = statuses.get(&server.name).copied();
        }
        if let Some(reported) = reported {
            for item in reported {
                let Some(server) = item.server.as_deref() else {
                    continue;
                };
                if self
                    .tools
                    .iter()
                    .any(|t| t.server == server && t.tool == item.tool)
                {
                    continue;
                }
                let record = self.servers.iter().find(|s| s.name == server);
                let covered = authorized(server, &item.tool);
                self.tools.push(McpToolState {
                    server: server.to_owned(),
                    tool: item.tool.clone(),
                    discovered: false,
                    enabled: record.is_some_and(|s| s.enabled),
                    authorized: covered,
                    exposed: record.is_some_and(|s| s.exposed) && covered,
                    reported_exposed: None,
                    used: None,
                });
            }
        }
        // A call the runtime streamed is a fact even when the runtime never listed its tools: a
        // tool Atlas had no state for is added for it.
        if reported.is_none() {
            for item in used.unwrap_or_default() {
                let Some(server) = item.server.as_deref() else {
                    continue;
                };
                if self
                    .tools
                    .iter()
                    .any(|t| t.server == server && t.tool == item.tool)
                {
                    continue;
                }
                let record = self.servers.iter().find(|s| s.name == server);
                let covered = authorized(server, &item.tool);
                self.tools.push(McpToolState {
                    server: server.to_owned(),
                    tool: item.tool.clone(),
                    discovered: false,
                    enabled: record.is_some_and(|s| s.enabled),
                    authorized: covered,
                    exposed: record.is_some_and(|s| s.exposed) && covered,
                    reported_exposed: None,
                    used: None,
                });
            }
        }
        for tool in &mut self.tools {
            tool.reported_exposed = reported.map(|r| has(r, &tool.server, &tool.tool));
            tool.used = match (reported, used) {
                (_, None) => None,
                (Some(_), Some(u)) => Some(has(u, &tool.server, &tool.tool)),
                // Without a list, a call seen is a "yes"; no call seen is not a "no".
                (None, Some(u)) => has(u, &tool.server, &tool.tool).then_some(true),
            };
        }
    }
}

/// A secret value while it is in memory. It has no `Display`, no `Serialize` and a `Debug` that
/// shows nothing, so it cannot end up in a log, an event or a stored file by accident.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// The value, for the one place that hands it to a process.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn connection(name: &str) -> McpConnection {
        McpConnection {
            id: format!("conn-{name}"),
            workspace_id: "w1".to_owned(),
            name: name.to_owned(),
            transport: McpTransport::Stdio {
                executable: "node".to_owned(),
                args: vec!["server.js".to_owned()],
                env: Vec::new(),
            },
            enabled: true,
            required: false,
            secrets: BTreeMap::new(),
            discovery: None,
            created_at: 1,
        }
    }

    #[test]
    fn a_secret_never_shows_in_a_debug_print() {
        let secret = Secret::new("hunter2hunter2".to_owned());

        assert!(!format!("{secret:?}").contains("hunter2"));
        assert_eq!(secret.expose(), "hunter2hunter2");
    }

    #[test]
    fn a_stored_connection_is_off_and_optional_until_someone_says_otherwise() {
        let stored: McpConnection = serde_json::from_str(
            r#"{"id":"c","workspaceId":"w","name":"files",
                "transport":{"kind":"stdio","executable":"node"},"createdAt":1}"#,
        )
        .unwrap();

        assert!(!stored.enabled && !stored.required);
        assert!(stored.secrets.is_empty() && stored.discovery.is_none());
    }

    #[test]
    fn the_secrets_a_connection_needs_are_named_and_the_missing_one_is_found() {
        let mut found = connection("api");
        found.transport = McpTransport::Stdio {
            executable: "node".to_owned(),
            args: vec![],
            env: vec![
                McpEnv {
                    name: "API_TOKEN".to_owned(),
                    value: McpEnvValue::Secret,
                },
                McpEnv {
                    name: "REGION".to_owned(),
                    value: McpEnvValue::Plain {
                        value: "eu".to_owned(),
                    },
                },
            ],
        };

        assert_eq!(found.secret_names(), ["API_TOKEN"]);
        assert_eq!(found.missing_secret(), Some("API_TOKEN"));
        found.secrets.insert("API_TOKEN".to_owned(), 7);
        assert_eq!(found.missing_secret(), None);
    }

    #[test]
    fn the_digest_follows_the_configuration_and_the_credential_version_not_the_secret() {
        let base = connection("api");
        let mut other_secret_version = base.clone();
        other_secret_version.secrets.insert("TOKEN".to_owned(), 99);
        let mut other_arg = base.clone();
        other_arg.transport = McpTransport::Stdio {
            executable: "node".to_owned(),
            args: vec!["other.js".to_owned()],
            env: vec![],
        };
        let mut switched_off = base.clone();
        switched_off.enabled = false;

        let digest = base.configuration_digest();

        assert_eq!(digest, base.clone().configuration_digest());
        for changed in [other_secret_version, other_arg, switched_off] {
            assert_ne!(digest, changed.configuration_digest());
        }
        // Discovery is information about the server, not a part of what was approved.
        let mut discovered = base;
        discovered.discovery = Some(McpDiscovery {
            discovered_at: 5,
            runtime_id: "claude".to_owned(),
            status: McpServerStatus::Connected,
            tools: vec!["t".to_owned()],
        });
        assert_eq!(digest, discovered.configuration_digest());
    }

    fn record() -> McpRecord {
        let state = |tool: &str, authorized: bool| McpToolState {
            server: "files".to_owned(),
            tool: tool.to_owned(),
            discovered: true,
            enabled: true,
            authorized,
            exposed: authorized,
            reported_exposed: None,
            used: None,
        };
        McpRecord {
            servers: vec![McpServerRecord {
                connection_id: "c".to_owned(),
                name: "files".to_owned(),
                enabled: true,
                required: false,
                authorized: true,
                exposed: true,
                problem: None,
                discovered_status: Some(McpServerStatus::Connected),
                reported_status: None,
            }],
            tools: vec![state("read", true), state("write", false)],
            held_back: vec!["files/write".to_owned()],
            unauthorized: vec![],
        }
    }

    fn tool(server: Option<&str>, name: &str) -> ReportedMcpTool {
        ReportedMcpTool {
            server: server.map(str::to_owned),
            tool: name.to_owned(),
        }
    }

    #[test]
    fn no_report_from_the_runtime_leaves_exposed_and_used_unknown_not_false() {
        let mut observed = record();

        observed.observe(None, &BTreeMap::new(), None, &|_, _| true);

        assert!(observed
            .tools
            .iter()
            .all(|t| t.reported_exposed.is_none() && t.used.is_none()));
        assert_eq!(observed.servers[0].reported_status, None);
    }

    #[test]
    fn a_call_seen_without_a_list_is_a_yes_and_a_call_not_seen_is_not_a_no() {
        let mut observed = record();
        let used = [tool(Some("files"), "read"), tool(Some("files"), "extra")];

        observed.observe(None, &BTreeMap::new(), Some(&used), &|_, t| t == "read");

        let find = |name: &str| observed.tools.iter().find(|t| t.tool == name).unwrap();
        assert_eq!(find("read").used, Some(true));
        // Never called as far as the stream showed: unknown, not false.
        assert_eq!(find("write").used, None);
        // A tool called that Atlas had no state for is added, with what a grant says of it.
        assert_eq!(find("extra").used, Some(true));
        assert!(!find("extra").authorized);
        assert!(observed.tools.iter().all(|t| t.reported_exposed.is_none()));
    }

    #[test]
    fn a_report_says_which_tools_the_runtime_listed_and_which_were_used() {
        let mut observed = record();
        let reported = [tool(Some("files"), "read")];
        let used = [tool(Some("files"), "read")];
        let statuses = BTreeMap::from([("files".to_owned(), McpServerStatus::Connected)]);

        observed.observe(Some(&reported), &statuses, Some(&used), &|_, t| t == "read");

        let read = &observed.tools[0];
        assert_eq!((read.reported_exposed, read.used), (Some(true), Some(true)));
        // The one held back was not listed, and was not used.
        let write = &observed.tools[1];
        assert_eq!(
            (write.reported_exposed, write.used),
            (Some(false), Some(false))
        );
        assert_eq!(
            observed.servers[0].reported_status,
            Some(McpServerStatus::Connected)
        );
    }

    #[test]
    fn a_tool_the_runtime_listed_that_atlas_did_not_know_is_added_with_what_a_grant_says() {
        let mut observed = record();
        let reported = [tool(Some("files"), "read"), tool(Some("files"), "extra")];

        observed.observe(Some(&reported), &BTreeMap::new(), None, &|_, t| t == "read");

        let extra = observed.tools.iter().find(|t| t.tool == "extra").unwrap();
        assert!(!extra.discovered && !extra.authorized && !extra.exposed);
        assert_eq!(extra.reported_exposed, Some(true));
        // A tool of no server Atlas launched is not given a state: it is reported as unauthorized.
        let mut with_stranger = record();
        with_stranger.observe(
            Some(&[tool(None, "mcp__other__x")]),
            &BTreeMap::new(),
            None,
            &|_, _| false,
        );
        assert_eq!(with_stranger.tools.len(), 2);
    }

    #[test]
    fn an_unmeasured_runtime_is_not_investigated_never_unsupported() {
        assert_eq!(McpSupport::default(), McpSupport::NotInvestigated);
    }
}
