//! The Copilot CLI's way of taking MCP servers, as an adapter over Atlas's own model.
//!
//! Measured with the real CLI (1.0.97) and a real model, on 2026-10-15:
//!
//! - `--additional-mcp-config @<file>` adds servers for one run: nothing is written to
//!   `~/.copilot/mcp-config.json`. The file is `{"mcpServers": {name: {type, command, args, env,
//!   tools, timeout}}}` (the shape `copilot mcp add` writes, with `type: "local"` for a stdio
//!   server). A server listed there showed `connected` in the run's `session.mcp_server_status_changed`
//!   events and a failed one carries the reason (`session.mcp_servers_loaded`).
//! - `$VAR`, `${VAR}` and `${VAR:-default}` in a server's `env` values are expanded from the CLI's
//!   own environment: a secret reaches a server without being in a file or an argument, and a server
//!   that needs a secret that did not arrive exits (`failed`). The CLI's own environment is also
//!   inherited by the server.
//! - `tools` is an allow-list per server (`["*"]` is all of it): the tool list the model was sent held
//!   only the named tools, so nothing is given that was not named and no discovery is needed.
//! - A model's call to an MCP tool is **refused** in a run that cannot ask (`code: denied`) unless the
//!   server is allowed with `--allow-tool=<server>`: with it the call went through and returned the
//!   nonce the server holds, and a second server that was not allowed was refused in the same run.
//!   The person's grant is the decision, so only the servers Atlas gives are allowed.
//! - A tool is named `<server>-<tool>` to the model, and the events carry `mcpServerName` and
//!   `mcpToolName` apart.
//! - The user's own servers (`~/.copilot/mcp-config.json`, a project's `.mcp.json`) load as well and
//!   can only be switched off by name (`--disable-mcp-server`): this runtime is **not strict**.
//! - There is no way to look at a server without a model (`copilot mcp list` reads the configuration
//!   and starts nothing), so there is no probe.

use serde_json::{json, Map, Value};

use super::claude_mcp::ClaudeMcpAdapter;
use super::mcp_adapter::{write_config, McpAdapter, McpPreparation};
use super::RuntimeError;
use crate::application::mcp::{LaunchEnv, McpLaunch};
use crate::domain::mcp::{McpFeatures, McpProbeKind, McpToolFilter, ReportedMcpTool};

/// How long the CLI waits for a server to start, in milliseconds (a first `npx` downloads).
const STARTUP_MS: u64 = 120_000;

pub struct CopilotMcpAdapter;

impl McpAdapter for CopilotMcpAdapter {
    fn program(&self) -> &'static str {
        "copilot"
    }

    /// Measured: an allow-list (`tools`), no probe, and the user's own servers still load.
    fn features(&self) -> McpFeatures {
        McpFeatures {
            tool_filter: McpToolFilter::AllowList,
            probe: McpProbeKind::None,
            strict: false,
        }
    }

    fn prepare(&self, launch: &McpLaunch) -> Result<McpPreparation, RuntimeError> {
        if launch.is_empty() {
            return Ok(McpPreparation::empty());
        }
        let file = write_config(&Self::config_json(launch))?;
        let mut args = vec![
            "--additional-mcp-config".to_owned(),
            format!("@{}", file.path()),
        ];
        args.extend(Self::allowed(launch));
        Ok(McpPreparation::with_files(
            args,
            ClaudeMcpAdapter::secret_environment(launch),
            vec![file],
        ))
    }

    fn classify(&self, reported: &[String], launched: &[&str]) -> Vec<ReportedMcpTool> {
        Self::classify_names(reported, launched)
    }
}

impl CopilotMcpAdapter {
    /// The config of one run: references where a secret goes, never the secret.
    pub fn config_json(launch: &McpLaunch) -> String {
        let mut servers = Map::new();
        for server in &launch.servers {
            let mut env = Map::new();
            for var in &server.env {
                let value = match var {
                    LaunchEnv::Plain { value, .. } => value.clone(),
                    LaunchEnv::Secret { name, .. } => {
                        format!(
                            "${{{}}}",
                            ClaudeMcpAdapter::secret_variable(&server.name, name)
                        )
                    }
                };
                env.insert(var.name().to_owned(), Value::String(value));
            }
            let tools = launch
                .only
                .iter()
                .find(|(name, _)| name == &server.name)
                .map_or_else(|| json!(["*"]), |(_, tools)| json!(tools));
            let mut entry = Map::new();
            entry.insert("type".to_owned(), json!("local"));
            entry.insert("command".to_owned(), json!(server.executable));
            entry.insert("args".to_owned(), json!(server.args));
            if !env.is_empty() {
                entry.insert("env".to_owned(), Value::Object(env));
            }
            entry.insert("tools".to_owned(), tools);
            entry.insert("timeout".to_owned(), json!(STARTUP_MS));
            servers.insert(server.name.clone(), Value::Object(entry));
        }
        json!({ "mcpServers": servers }).to_string()
    }

    /// What lets a model's call go through in a run that cannot ask: each given server, by name.
    /// Each is its own option, so a prompt or a later flag is never read as another server.
    pub fn allowed(launch: &McpLaunch) -> Vec<String> {
        launch
            .servers
            .iter()
            .map(|server| format!("--allow-tool={}", server.name))
            .collect()
    }

    /// A call is reported as `mcp__<server>__<tool>`, the shape the other adapters and the
    /// manifest use.
    pub fn tool_name(server: Option<&str>, tool: &str) -> String {
        server.map_or_else(|| tool.to_owned(), |s| format!("mcp__{s}__{tool}"))
    }

    /// Which of the names a run listed or used are tools of the launched servers: the shape this
    /// adapter reports a call in, and the model's own `<server>-<tool>`. The longest server name
    /// wins, so a server whose name starts with another's is not mistaken for it.
    pub fn classify_names(reported: &[String], launched: &[&str]) -> Vec<ReportedMcpTool> {
        let mut by_length: Vec<&str> = launched.to_vec();
        by_length.sort_by_key(|name| std::cmp::Reverse(name.len()));
        reported
            .iter()
            .filter_map(|name| {
                if name.starts_with("mcp__") {
                    return Some(
                        by_length
                            .iter()
                            .find_map(|server| {
                                name.strip_prefix(&format!("mcp__{server}__"))
                                    .map(|tool| ((*server).to_owned(), tool.to_owned()))
                            })
                            .map_or_else(
                                || ReportedMcpTool {
                                    server: None,
                                    tool: name.clone(),
                                },
                                |(server, tool)| ReportedMcpTool {
                                    server: Some(server),
                                    tool,
                                },
                            ),
                    );
                }
                by_length.iter().find_map(|server| {
                    name.strip_prefix(&format!("{server}-"))
                        .map(|tool| ReportedMcpTool {
                            server: Some((*server).to_owned()),
                            tool: tool.to_owned(),
                        })
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::mcp::LaunchServer;
    use crate::domain::mcp::Secret;

    fn server(name: &str, env: Vec<LaunchEnv>) -> LaunchServer {
        LaunchServer {
            name: name.to_owned(),
            executable: "node".to_owned(),
            args: vec!["server.js".to_owned()],
            env,
        }
    }

    fn secret(name: &str, value: &str) -> LaunchEnv {
        LaunchEnv::Secret {
            name: name.to_owned(),
            value: Secret::new(value.to_owned()),
        }
    }

    fn launch(servers: Vec<LaunchServer>, only: Vec<(String, Vec<String>)>) -> McpLaunch {
        McpLaunch {
            servers,
            held_back: vec![],
            only,
        }
    }

    #[test]
    fn the_config_carries_a_reference_where_a_secret_goes_and_never_the_secret() {
        let launch = launch(
            vec![server(
                "files",
                vec![
                    LaunchEnv::Plain {
                        name: "MODE".to_owned(),
                        value: "ro".to_owned(),
                    },
                    secret("TOKEN", "hunter2"),
                ],
            )],
            vec![],
        );

        let config = CopilotMcpAdapter::config_json(&launch);
        let value: Value = serde_json::from_str(&config).unwrap();

        assert!(!config.contains("hunter2"));
        let files = &value["mcpServers"]["files"];
        assert_eq!(files["type"], "local");
        assert_eq!(files["command"], "node");
        assert_eq!(files["args"][0], "server.js");
        assert_eq!(files["env"]["MODE"], "ro");
        assert_eq!(files["env"]["TOKEN"], "${ATLAS_MCP_FILES_TOKEN}");
        assert_eq!(files["tools"], json!(["*"]));
        assert_eq!(files["timeout"], 120_000);
    }

    #[test]
    fn named_tools_become_an_allow_list_of_that_server_only() {
        let launch = launch(
            vec![server("a", vec![]), server("b", vec![])],
            vec![("a".to_owned(), vec!["read".to_owned()])],
        );

        let value: Value = serde_json::from_str(&CopilotMcpAdapter::config_json(&launch)).unwrap();

        assert_eq!(value["mcpServers"]["a"]["tools"], json!(["read"]));
        assert_eq!(value["mcpServers"]["b"]["tools"], json!(["*"]));
    }

    #[test]
    fn a_run_gives_the_config_file_the_allowed_servers_and_the_secrets_by_environment() {
        let launch = launch(
            vec![
                server("files", vec![secret("TOKEN", "hunter2")]),
                server("web", vec![]),
            ],
            vec![],
        );

        let prep = CopilotMcpAdapter.prepare(&launch).unwrap();

        assert_eq!(prep.args[0], "--additional-mcp-config");
        assert!(prep.args[1].starts_with('@'));
        assert_eq!(prep.args[2..], ["--allow-tool=files", "--allow-tool=web"]);
        assert_eq!(
            prep.env,
            [("ATLAS_MCP_FILES_TOKEN".to_owned(), "hunter2".to_owned())]
        );
        // The file holds no secret, and goes with the preparation.
        let path = prep.args[1].trim_start_matches('@').to_owned();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("hunter2"));
        drop(prep);
        assert!(!std::path::Path::new(&path).exists());
    }

    #[test]
    fn no_servers_is_no_preparation_at_all() {
        let prep = CopilotMcpAdapter.prepare(&launch(vec![], vec![])).unwrap();

        assert!(prep.args.is_empty() && prep.env.is_empty());
    }

    #[test]
    fn the_models_own_names_and_the_reported_ones_are_told_back_to_their_server() {
        let reported: Vec<String> = [
            "files-read",
            "files-v2-read",
            "mcp__files__write",
            "mcp__other__x",
            "bash",
            "rg",
        ]
        .map(str::to_owned)
        .to_vec();

        let tools = CopilotMcpAdapter::classify_names(&reported, &["files", "files-v2"]);

        let found: Vec<_> = tools
            .iter()
            .map(|t| (t.server.as_deref(), t.tool.as_str()))
            .collect();
        assert_eq!(
            found,
            [
                (Some("files"), "read"),
                (Some("files-v2"), "read"),
                (Some("files"), "write"),
                (None, "mcp__other__x"),
            ]
        );
    }

    #[test]
    fn this_runtime_has_no_probe_and_says_so_without_starting_anything() {
        use crate::application::process::fake::FakeProcessRunner;
        let runner = FakeProcessRunner::new(&["copilot"], |_| unreachable!("nothing is started"));

        let error = CopilotMcpAdapter
            .probe(&runner, &server("files", vec![]))
            .unwrap_err();

        assert!(matches!(error, RuntimeError::Unavailable(_)), "{error:?}");
    }
}
