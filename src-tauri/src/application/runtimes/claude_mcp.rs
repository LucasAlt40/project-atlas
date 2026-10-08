//! The Claude CLI's way of taking MCP servers, as an adapter over Atlas's own model.
//!
//! What was measured about it (ADR 0026's spike, repeated in Phase D, no model call):
//!
//! - `--mcp-config` takes a JSON file (or the JSON inline); with `--strict-mcp-config` only those
//!   servers load. Atlas uses a file: a long inline argument is mangled by Windows `.cmd` shims.
//! - A server's tools are named `mcp__<server>__<tool>` and appear in the start-up report (`init`).
//! - A tool that needs permission is refused in a non-interactive run, so Atlas also passes
//!   `--allowedTools` for what it granted (`mcp__<server>` for a whole server). That this is what lets
//!   a model call the tool was read in the CLI's reference and not run (it needs a model call).
//! - `--tools` does **not** limit them; `--disallowedTools mcp__<server>__<tool>` removes one. A
//!   tool the server adds later is not on any list Atlas made, so it is exposed: Atlas reads the
//!   report and stops the step when the runtime lists a tool nobody authorized.
//! - `${VAR}` in a server's `command`, `args` or `env` is expanded from the CLI's own environment.
//!   That is how a secret reaches a server without being in a file or an argument: the config
//!   carries `${ATLAS_MCP_…}` and the CLI's process environment carries the value. An unset
//!   variable is **not** an error (the text stays as written), so Atlas checks the secret is
//!   there before it launches. For the same reason no configuration may contain `${` itself.
//! - A server whose tool has an invalid schema is `connected` with **none** of its tools listed; a
//!   server that cannot start is `failed` and the run goes on without it.

use serde_json::{json, Map, Value};

use std::time::Duration;

use super::mcp_adapter::{write_config, McpAdapter, McpPreparation, ProbePlan};
use super::RuntimeError;
use crate::application::mcp::{LaunchEnv, LaunchServer, McpLaunch, McpProbe};
use crate::application::process::ProcessOutput;
use crate::domain::mcp::{
    McpFeatures, McpProbeKind, McpServerStatus, McpToolFilter, ReportedMcpTool,
};

/// How long the CLI waits for an MCP server to start, in milliseconds (its `MCP_TIMEOUT`).
pub const STARTUP_WAIT_MS: u64 = 120_000;

/// What stands in for the config file in a description of the launch (the real path is
/// per-run and means nothing to a reader).
pub const CONFIG_PLACEHOLDER: &str = "<mcp config file>";

/// What the CLI said about MCP when it started.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InitReport {
    pub tools: Vec<String>,
    pub servers: Vec<(String, McpServerStatus)>,
}

pub struct ClaudeMcpAdapter;

/// Idle limit of a probe: longer than the wait the CLI gives a server to start
/// ([`STARTUP_WAIT_MS`]), during which it prints nothing.
const PROBE_TIMEOUT: Duration = Duration::from_secs(180);

impl McpAdapter for ClaudeMcpAdapter {
    fn program(&self) -> &'static str {
        "claude"
    }

    /// Measured: `--disallowedTools` hides a known tool; the `init` report lists tools; only the
    /// servers given load (`--strict-mcp-config`).
    fn features(&self) -> McpFeatures {
        McpFeatures {
            tool_filter: McpToolFilter::DenyList,
            probe: McpProbeKind::Tools,
            strict: true,
        }
    }

    fn prepare(&self, launch: &McpLaunch) -> Result<McpPreparation, RuntimeError> {
        if launch.is_empty() {
            return Ok(McpPreparation::empty());
        }
        // The servers' config travels in a file that lives as long as the run.
        let file = write_config(&Self::config_json(launch))?;
        Ok(McpPreparation::with_files(
            Self::arguments(launch, file.path()),
            Self::environment(launch),
            vec![file],
        ))
    }

    /// Starts the server through the CLI with a model that does not exist: the CLI reports what it
    /// loaded and fails at its first request, so nothing is asked of any model and nothing is spent.
    fn probe_plan(&self, server: &LaunchServer) -> Option<Result<ProbePlan, RuntimeError>> {
        let launch = McpLaunch::of_one(server);
        Some(write_config(&Self::config_json(&launch)).map(|file| {
            ProbePlan::new(
                Self::probe_arguments(server, file.path()),
                Self::environment(&launch),
                PROBE_TIMEOUT,
                read_probe,
                vec![file],
            )
        }))
    }

    fn classify(&self, reported: &[String], launched: &[&str]) -> Vec<ReportedMcpTool> {
        ClaudeMcpAdapter::classify(reported, launched)
    }
}

/// The server's status and tools from the CLI's `init` report.
fn read_probe(output: &ProcessOutput, name: &str) -> Result<McpProbe, RuntimeError> {
    let report = ClaudeMcpAdapter::parse_init(&output.stdout).ok_or_else(|| {
        RuntimeError::UnexpectedResponse("the CLI did not report what it loaded".to_owned())
    })?;
    let status = report
        .servers
        .iter()
        .find(|(listed, _)| listed == name)
        .map(|(_, status)| *status)
        .ok_or_else(|| {
            RuntimeError::UnexpectedResponse("the CLI did not list the server".to_owned())
        })?;
    let tools = report
        .tools
        .iter()
        .filter_map(|tool| ClaudeMcpAdapter::tool_of(name, tool))
        .map(str::to_owned)
        .collect();
    Ok(McpProbe { status, tools })
}

impl ClaudeMcpAdapter {
    /// How the CLI names a server's tool.
    pub fn tool_name(server: &str, tool: &str) -> String {
        format!("mcp__{server}__{tool}")
    }

    /// The tool part of a CLI tool name, when it belongs to `server`.
    pub fn tool_of<'a>(server: &str, cli_name: &'a str) -> Option<&'a str> {
        cli_name.strip_prefix(&format!("mcp__{server}__"))
    }

    /// Which of the tools a runtime listed are MCP tools, and of which server. A tool named
    /// `mcp__<server>__<tool>` of a server Atlas launched is that server's; one named `mcp__…` of
    /// any other server (a connector of the user's, a server Atlas did not start) has no server.
    pub fn classify(reported: &[String], launched: &[&str]) -> Vec<ReportedMcpTool> {
        let mut by_length: Vec<&str> = launched.to_vec();
        by_length.sort_by_key(|name| std::cmp::Reverse(name.len()));
        reported
            .iter()
            .filter(|name| name.starts_with("mcp__"))
            .map(|name| {
                let owner = by_length
                    .iter()
                    .find_map(|server| Self::tool_of(server, name).map(|tool| (*server, tool)));
                match owner {
                    Some((server, tool)) => ReportedMcpTool {
                        server: Some(server.to_owned()),
                        tool: tool.to_owned(),
                    },
                    None => ReportedMcpTool {
                        server: None,
                        tool: name.clone(),
                    },
                }
            })
            .collect()
    }

    /// The environment variable that carries one secret of one server to the CLI.
    pub fn secret_variable(server: &str, name: &str) -> String {
        let server: String = server
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_uppercase()
                } else {
                    '_'
                }
            })
            .collect();
        format!("ATLAS_MCP_{server}_{name}")
    }

    /// The `--mcp-config` value: one JSON line, with references where a secret goes.
    pub fn config_json(launch: &McpLaunch) -> String {
        let mut servers = Map::new();
        for server in &launch.servers {
            let mut env = Map::new();
            for var in &server.env {
                let value = match var {
                    LaunchEnv::Plain { value, .. } => value.clone(),
                    LaunchEnv::Secret { name, .. } => {
                        format!("${{{}}}", Self::secret_variable(&server.name, name))
                    }
                };
                env.insert(var.name().to_owned(), Value::String(value));
            }
            let mut entry = Map::new();
            entry.insert("command".to_owned(), json!(server.executable));
            entry.insert("args".to_owned(), json!(server.args));
            if !env.is_empty() {
                entry.insert("env".to_owned(), Value::Object(env));
            }
            servers.insert(server.name.clone(), Value::Object(entry));
        }
        json!({ "mcpServers": servers }).to_string()
    }

    /// The arguments that give the CLI these servers and hold back the tools no grant covers.
    /// `config` is the value of `--mcp-config` (the path of the run's config file). Both options take
    /// several values, so each is followed by another option.
    pub fn arguments(launch: &McpLaunch, config: &str) -> Vec<String> {
        if launch.is_empty() {
            return Vec::new();
        }
        let mut args = vec!["--mcp-config".to_owned(), config.to_owned()];
        // `claude -p` cannot ask a person, so a tool that needs permission is simply refused: a
        // server a person granted must also be allowed to be called, the whole server
        // (`mcp__<server>`) or the tools named. What was held back below is denied, and a denial
        // wins over an allowance.
        args.push("--allowedTools".to_owned());
        for server in &launch.servers {
            match launch.only.iter().find(|(name, _)| name == &server.name) {
                Some((_, tools)) => {
                    args.extend(tools.iter().map(|tool| Self::tool_name(&server.name, tool)));
                }
                None => args.push(format!("mcp__{}", server.name)),
            }
        }
        if !launch.held_back.is_empty() {
            args.push("--disallowedTools".to_owned());
            args.extend(
                launch
                    .held_back
                    .iter()
                    .map(|(server, tool)| Self::tool_name(server, tool)),
            );
        }
        args
    }

    /// The secrets the config refers to, under the variable names it uses.
    pub fn secret_environment(launch: &McpLaunch) -> Vec<(String, String)> {
        launch
            .servers
            .iter()
            .flat_map(|server| {
                server.env.iter().filter_map(|var| match var {
                    LaunchEnv::Secret { name, value } => Some((
                        Self::secret_variable(&server.name, name),
                        value.expose().to_owned(),
                    )),
                    LaunchEnv::Plain { .. } => None,
                })
            })
            .collect()
    }

    /// The values the CLI's own environment must carry: the secrets the config refers to, and how
    /// long it waits for a server to come up. Measured: a server that needs 40 s to start (an
    /// `npx` that has to download its package, the first time) is `failed` under the CLI's default
    /// wait and `connected` with `MCP_TIMEOUT` raised.
    pub fn environment(launch: &McpLaunch) -> Vec<(String, String)> {
        if launch.is_empty() {
            return Vec::new();
        }
        let secrets = launch.servers.iter().flat_map(|server| {
            server.env.iter().filter_map(|var| match var {
                LaunchEnv::Secret { name, value } => Some((
                    Self::secret_variable(&server.name, name),
                    value.expose().to_owned(),
                )),
                LaunchEnv::Plain { .. } => None,
            })
        });
        secrets
            .chain([("MCP_TIMEOUT".to_owned(), STARTUP_WAIT_MS.to_string())])
            .collect()
    }

    /// The arguments of a start-up probe of one server: nothing is asked of any model (the model
    /// does not exist, so the CLI reports what it loaded and fails at its first request).
    pub fn probe_arguments(server: &LaunchServer, config: &str) -> Vec<String> {
        let launch = McpLaunch::of_one(server);
        let mut args: Vec<String> = [
            "-p",
            "--output-format",
            "stream-json",
            "--verbose",
            "--model",
            PROBE_MODEL,
            "--tools",
            "",
            "--no-session-persistence",
        ]
        .map(str::to_owned)
        .to_vec();
        args.extend(Self::arguments(&launch, config));
        args.extend(
            [
                "--strict-mcp-config",
                "--disable-slash-commands",
                "--",
                "ping",
            ]
            .map(str::to_owned),
        );
        args
    }

    /// The `system`/`init` message, as what Atlas needs of it.
    pub fn parse_init(stdout: &str) -> Option<InitReport> {
        let init = stdout
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .find(|v| v["type"] == "system" && v["subtype"] == "init")?;
        Some(Self::report_of(&init))
    }

    /// The same, from one already-parsed line.
    pub fn report_of(init: &Value) -> InitReport {
        let tools = init["tools"]
            .as_array()
            .map(|tools| {
                tools
                    .iter()
                    .filter_map(|t| t.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let servers = init["mcp_servers"]
            .as_array()
            .map(|servers| {
                servers
                    .iter()
                    .filter_map(|s| {
                        Some((
                            s["name"].as_str()?.to_owned(),
                            status_of(s["status"].as_str()?),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        InitReport { tools, servers }
    }
}

/// A model that does not exist, on purpose: see [`ClaudeMcpAdapter::probe_arguments`].
const PROBE_MODEL: &str = "atlas-probe-no-such-model";

fn status_of(status: &str) -> McpServerStatus {
    match status {
        "connected" => McpServerStatus::Connected,
        "failed" => McpServerStatus::Failed,
        "needs-auth" => McpServerStatus::NeedsAuth,
        "pending" => McpServerStatus::Pending,
        _ => McpServerStatus::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::mcp::Secret;

    fn server(name: &str, env: Vec<LaunchEnv>) -> LaunchServer {
        LaunchServer {
            name: name.to_owned(),
            executable: "node".to_owned(),
            args: vec![
                "server.js".to_owned(),
                "--root".to_owned(),
                "/tmp/a b".to_owned(),
            ],
            env,
        }
    }

    fn secret_env(name: &str, value: &str) -> LaunchEnv {
        LaunchEnv::Secret {
            name: name.to_owned(),
            value: Secret::new(value.to_owned()),
        }
    }

    #[test]
    fn the_config_carries_a_reference_where_a_secret_goes_and_never_the_secret() {
        let launch = McpLaunch {
            servers: vec![server(
                "chrome-devtools",
                vec![
                    secret_env("API_TOKEN", "hunter2hunter2"),
                    LaunchEnv::Plain {
                        name: "REGION".to_owned(),
                        value: "eu".to_owned(),
                    },
                ],
            )],
            held_back: vec![],
            only: Vec::new(),
        };

        let json = ClaudeMcpAdapter::config_json(&launch);
        let parsed: Value = serde_json::from_str(&json).unwrap();

        assert!(!json.contains("hunter2"));
        assert!(!json.contains('\n'));
        let entry = &parsed["mcpServers"]["chrome-devtools"];
        assert_eq!(entry["command"], "node");
        assert_eq!(entry["args"][2], "/tmp/a b");
        assert_eq!(entry["env"]["REGION"], "eu");
        assert_eq!(
            entry["env"]["API_TOKEN"],
            "${ATLAS_MCP_CHROME_DEVTOOLS_API_TOKEN}"
        );
    }

    #[test]
    fn the_secret_travels_in_the_environment_under_the_name_the_config_refers_to() {
        let launch = McpLaunch {
            servers: vec![
                server("files", vec![secret_env("TOKEN", "one")]),
                server("web-2", vec![secret_env("TOKEN", "two")]),
            ],
            held_back: vec![],
            only: Vec::new(),
        };

        let env = ClaudeMcpAdapter::environment(&launch);

        assert_eq!(
            env,
            [
                ("ATLAS_MCP_FILES_TOKEN".to_owned(), "one".to_owned()),
                ("ATLAS_MCP_WEB_2_TOKEN".to_owned(), "two".to_owned()),
                ("MCP_TIMEOUT".to_owned(), "120000".to_owned())
            ]
        );
        // And it is nowhere in the arguments.
        assert!(!ClaudeMcpAdapter::arguments(&launch, CONFIG_PLACEHOLDER)
            .join(" ")
            .contains("one"));
    }

    #[test]
    fn the_arguments_give_the_servers_and_hold_back_what_no_grant_covers() {
        let launch = McpLaunch {
            servers: vec![server("files", vec![])],
            held_back: vec![
                ("files".to_owned(), "write".to_owned()),
                ("files".to_owned(), "delete".to_owned()),
            ],
            only: Vec::new(),
        };

        let args = ClaudeMcpAdapter::arguments(&launch, "/run/config.json");

        assert_eq!(args[0], "--mcp-config");
        assert_eq!(args[1], "/run/config.json");
        assert_eq!(
            &args[2..],
            [
                // Granted whole, so allowed whole (a non-interactive run cannot ask).
                "--allowedTools",
                "mcp__files",
                "--disallowedTools",
                "mcp__files__write",
                "mcp__files__delete"
            ]
        );
    }

    #[test]
    fn a_server_held_to_named_tools_is_allowed_those_tools_only() {
        let launch = McpLaunch {
            servers: vec![server("files", vec![]), server("web", vec![])],
            held_back: vec![],
            only: vec![("files".to_owned(), vec!["read".to_owned()])],
        };

        let args = ClaudeMcpAdapter::arguments(&launch, "/run/config.json");

        assert_eq!(
            &args[2..],
            ["--allowedTools", "mcp__files__read", "mcp__web"]
        );
    }

    #[test]
    fn nothing_to_give_is_no_arguments() {
        assert_eq!(
            ClaudeMcpAdapter::arguments(&McpLaunch::default(), CONFIG_PLACEHOLDER).len(),
            0
        );
        assert_eq!(
            ClaudeMcpAdapter::environment(&McpLaunch::default()).len(),
            0
        );
    }

    #[test]
    fn a_probe_names_a_model_that_does_not_exist_and_one_server() {
        let args = ClaudeMcpAdapter::probe_arguments(&server("files", vec![]), "/run/config.json");

        let at = args.iter().position(|a| a == "--model").unwrap();
        assert_eq!(args[at + 1], PROBE_MODEL);
        assert!(args.contains(&"--strict-mcp-config".to_owned()));
        // The prompt is last, after `--`, so it can never be taken for an option.
        assert_eq!(&args[args.len() - 2..], ["--", "ping"]);
        // The variadic option is followed by a flag.
        let mcp = args.iter().position(|a| a == "--mcp-config").unwrap();
        assert!(args[mcp + 2].starts_with("--"));
    }

    #[test]
    fn the_start_up_report_is_read_for_tools_and_server_statuses() {
        let line = r#"{"type":"system","subtype":"init","tools":["Read","mcp__files__read"],"mcp_servers":[{"name":"files","status":"connected"},{"name":"web","status":"needs-auth"},{"name":"x","status":"failed"},{"name":"y","status":"???"}]}"#;

        let report = ClaudeMcpAdapter::parse_init(&format!("noise\n{line}\n")).unwrap();

        assert_eq!(report.tools, ["Read", "mcp__files__read"]);
        assert_eq!(
            report.servers,
            [
                ("files".to_owned(), McpServerStatus::Connected),
                ("web".to_owned(), McpServerStatus::NeedsAuth),
                ("x".to_owned(), McpServerStatus::Failed),
                ("y".to_owned(), McpServerStatus::Unknown)
            ]
        );
        assert_eq!(ClaudeMcpAdapter::parse_init("no json here"), None);
    }

    #[test]
    fn tool_names_round_trip() {
        let name = ClaudeMcpAdapter::tool_name("files", "read_file");

        assert_eq!(name, "mcp__files__read_file");
        assert_eq!(ClaudeMcpAdapter::tool_of("files", &name), Some("read_file"));
        assert_eq!(ClaudeMcpAdapter::tool_of("other", &name), None);
    }
}
