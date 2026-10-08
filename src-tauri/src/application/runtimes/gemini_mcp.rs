//! The Gemini CLI's way of taking MCP servers, as an adapter over Atlas's own model.
//!
//! Measured with the real CLI (0.46.0), no model call:
//!
//! - `GEMINI_CLI_SYSTEM_SETTINGS_PATH` points the CLI at a settings file for one run, so nothing of
//!   the user's own settings is written. A `mcpServers` entry there is started and shows
//!   `Connected` in `gemini mcp list`. (It replaces a managed system settings file if the machine
//!   has one; that is a limit.)
//! - `$VAR` in a server's `env` values is expanded from the CLI's own environment: a secret reaches
//!   a server without being in a file or an argument. A server that needs the secret and does not
//!   get it is `Disconnected`.
//! - In a folder the CLI does not trust the servers are `Disabled`. A real run passes
//!   `--skip-trust` (as Atlas already does); the probe sets `GEMINI_CLI_TRUST_WORKSPACE`, which is
//!   fine there because it runs nowhere in particular.
//! - `--allowed-mcp-server-names` exists but its effect on a run was not verified (with it, `mcp
//!   list` printed nothing), so Atlas does not pass it: this runtime is **not strict** and the
//!   user's own servers load as well.
//! - A headless run cannot confirm a tool call, so Atlas sets `trust` on the servers it gives (the
//!   person's grant is the decision). That a model's call then goes through was read in the CLI's
//!   reference, not run.
//! - `includeTools` limits a server to the tools named (an allow-list: no discovery is needed and
//!   a tool the server adds later is not given). Its effect on a model's calls was read in the
//!   CLI's reference, not run.
//! - `gemini mcp list` reports a status per server (on stderr when not attached to a terminal) and
//!   **no tool names**.

use serde_json::{json, Map, Value};

use std::time::Duration;

use super::claude_mcp::ClaudeMcpAdapter;
use super::mcp_adapter::{
    list_probe_arguments, listed_probe, write_config, McpAdapter, McpPreparation, ProbePlan,
};
use super::RuntimeError;
use crate::application::mcp::{LaunchEnv, LaunchServer, McpLaunch, McpProbe};
use crate::application::process::ProcessOutput;
use crate::domain::mcp::{
    McpFeatures, McpProbeKind, McpServerStatus, McpToolFilter, ReportedMcpTool,
};

/// How long the CLI waits for a server to start, in milliseconds (a first `npx` downloads).
const STARTUP_MS: u64 = 120_000;

pub struct GeminiMcpAdapter;

impl McpAdapter for GeminiMcpAdapter {
    fn program(&self) -> &'static str {
        "gemini"
    }

    /// Measured: an allow-list (`includeTools`), a status-only probe, and the user's own servers
    /// still load.
    fn features(&self) -> McpFeatures {
        McpFeatures {
            tool_filter: McpToolFilter::AllowList,
            probe: McpProbeKind::StatusOnly,
            strict: false,
        }
    }

    fn prepare(&self, launch: &McpLaunch) -> Result<McpPreparation, RuntimeError> {
        if launch.is_empty() {
            return Ok(McpPreparation::empty());
        }
        let file = write_config(&Self::settings_json(launch))?;
        Ok(McpPreparation::with_files(
            Vec::new(),
            Self::environment(launch, file.path()),
            vec![file],
        ))
    }

    fn probe_plan(&self, server: &LaunchServer) -> Option<Result<ProbePlan, RuntimeError>> {
        let launch = McpLaunch::of_one(server);
        Some(write_config(&Self::settings_json(&launch)).map(|file| {
            ProbePlan::new(
                list_probe_arguments(),
                Self::probe_environment(server, file.path()),
                Duration::from_secs(180),
                read_probe,
                vec![file],
            )
        }))
    }

    /// The CLI reports no tool names, so none is told back to a server.
    fn classify(&self, _reported: &[String], _launched: &[&str]) -> Vec<ReportedMcpTool> {
        Vec::new()
    }
}

/// The server's status from `gemini mcp list`, which writes to stderr when it is not attached to
/// a terminal. It lists no tools.
fn read_probe(output: &ProcessOutput, name: &str) -> Result<McpProbe, RuntimeError> {
    let listing = format!("{}\n{}", output.stdout, output.stderr);
    listed_probe(GeminiMcpAdapter::status_in(&listing, name))
}

impl GeminiMcpAdapter {
    /// The settings file of one run: references where a secret goes, never the secret.
    pub fn settings_json(launch: &McpLaunch) -> String {
        let mut servers = Map::new();
        for server in &launch.servers {
            let mut env = Map::new();
            for var in &server.env {
                let value = match var {
                    LaunchEnv::Plain { value, .. } => value.clone(),
                    LaunchEnv::Secret { name, .. } => {
                        format!("${}", ClaudeMcpAdapter::secret_variable(&server.name, name))
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
            entry.insert("timeout".to_owned(), json!(STARTUP_MS));
            // A headless run cannot ask a person to confirm a tool call, and without this it is
            // refused. The person decided when they granted the server (and the agent's policy and
            // the guardrails stand before the step); nothing else gets this.
            entry.insert("trust".to_owned(), json!(true));
            if let Some((_, tools)) = launch.only.iter().find(|(name, _)| name == &server.name) {
                entry.insert("includeTools".to_owned(), json!(tools));
            }
            servers.insert(server.name.clone(), Value::Object(entry));
        }
        json!({ "mcpServers": servers }).to_string()
    }

    /// The CLI's environment: where its settings are, and the secrets the settings refer to.
    pub fn environment(launch: &McpLaunch, settings_path: &str) -> Vec<(String, String)> {
        if launch.is_empty() {
            return Vec::new();
        }
        let mut env = vec![(
            "GEMINI_CLI_SYSTEM_SETTINGS_PATH".to_owned(),
            settings_path.to_owned(),
        )];
        env.extend(ClaudeMcpAdapter::secret_environment(launch));
        env
    }

    /// What `gemini mcp list` says about `name`: `✓ name: … - Connected`, `✗ … - Disconnected`,
    /// `○ … - Disabled`.
    pub fn status_in(stdout: &str, name: &str) -> Option<McpServerStatus> {
        stdout.lines().find_map(|line| {
            let line = line.trim_start();
            let rest = line.strip_prefix(['✓', '✗', '○', '●'])?.trim_start();
            let (listed, after) = rest.split_once(':')?;
            (listed.trim() == name).then(|| {
                if after.trim_end().ends_with("Connected") {
                    McpServerStatus::Connected
                } else {
                    McpServerStatus::Failed
                }
            })
        })
    }

    /// The probe's arguments: list the servers (the CLI starts each to say whether it connects).
    /// The probe's environment: the settings of the one server, its secrets, and trust (it runs
    /// in no project, so there is no trust to give away).
    pub fn probe_environment(server: &LaunchServer, settings_path: &str) -> Vec<(String, String)> {
        let launch = McpLaunch::of_one(server);
        let mut env = Self::environment(&launch, settings_path);
        env.push(("GEMINI_CLI_TRUST_WORKSPACE".to_owned(), "true".to_owned()));
        env
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

    #[test]
    fn the_settings_carry_a_reference_where_a_secret_goes_and_never_the_secret() {
        let launch = McpLaunch {
            servers: vec![server(
                "files",
                vec![
                    secret("API_TOKEN", "hunter2hunter2"),
                    LaunchEnv::Plain {
                        name: "REGION".to_owned(),
                        value: "eu".to_owned(),
                    },
                ],
            )],
            held_back: vec![],
            only: vec![],
        };

        let json = GeminiMcpAdapter::settings_json(&launch);
        let parsed: Value = serde_json::from_str(&json).unwrap();

        assert!(!json.contains("hunter2"));
        let entry = &parsed["mcpServers"]["files"];
        assert_eq!(entry["env"]["API_TOKEN"], "$ATLAS_MCP_FILES_API_TOKEN");
        assert_eq!(entry["env"]["REGION"], "eu");
        assert_eq!(entry["trust"], true);
        assert!(entry.get("includeTools").is_none());
    }

    #[test]
    fn named_tools_become_an_allow_list_of_that_server_only() {
        let launch = McpLaunch {
            servers: vec![server("files", vec![]), server("web", vec![])],
            held_back: vec![],
            only: vec![("files".to_owned(), vec!["read".to_owned()])],
        };

        let parsed: Value =
            serde_json::from_str(&GeminiMcpAdapter::settings_json(&launch)).unwrap();

        assert_eq!(
            parsed["mcpServers"]["files"]["includeTools"],
            json!(["read"])
        );
        assert!(parsed["mcpServers"]["web"].get("includeTools").is_none());
    }

    #[test]
    fn the_settings_travel_in_the_environment() {
        let launch = McpLaunch {
            servers: vec![server("files", vec![secret("TOKEN", "one")])],
            held_back: vec![],
            only: vec![],
        };

        assert_eq!(
            GeminiMcpAdapter::environment(&launch, "/run/s.json"),
            [
                (
                    "GEMINI_CLI_SYSTEM_SETTINGS_PATH".to_owned(),
                    "/run/s.json".to_owned()
                ),
                ("ATLAS_MCP_FILES_TOKEN".to_owned(), "one".to_owned())
            ]
        );
        assert_eq!(
            GeminiMcpAdapter::environment(&McpLaunch::default(), "/x").len(),
            0
        );
    }

    #[test]
    fn the_status_of_a_server_is_read_from_the_listing() {
        let out = "Configured MCP servers:\n\n✓ files: node /a/b.js (stdio) - Connected\n✗ web: node x (stdio) - Disconnected\n○ off: node y (stdio) - Disabled\n";

        assert_eq!(
            GeminiMcpAdapter::status_in(out, "files"),
            Some(McpServerStatus::Connected)
        );
        assert_eq!(
            GeminiMcpAdapter::status_in(out, "web"),
            Some(McpServerStatus::Failed)
        );
        assert_eq!(
            GeminiMcpAdapter::status_in(out, "off"),
            Some(McpServerStatus::Failed)
        );
        assert_eq!(GeminiMcpAdapter::status_in(out, "ghost"), None);
    }
}
