//! `OpenCode`'s way of taking MCP servers, as an adapter over Atlas's own model.
//!
//! Measured with the real CLI (1.18.34), no model call:
//!
//! - `OPENCODE_CONFIG_CONTENT` carries a whole config as JSON for one run: nothing is written to
//!   the user's configuration, and no file is needed. A `mcp` entry of `type: local` is started and
//!   shows `connected` in `opencode mcp list`.
//! - `{env:VAR}` in a server's `environment` values is expanded from the CLI's own environment
//!   (a server that needs the secret and does not get it is `failed`), so a secret is never in the
//!   config or an argument.
//! - The user's own servers in their configuration still load: `OpenCode` merges, and Atlas has no
//!   way to name what it does not know. This runtime is **not strict**.
//! - `tools: {"<server>_*": false, "<server>_<tool>": true}` holds a server to the tools named
//!   (an allow-list; measured with a real model: the tool left on was called, the other was "not
//!   available"). A `permission` rule for the same did not give a clear result and is not used.
//! - A tool is called `<server>_<tool>` (measured), shown in the run's `tool_use` events.
//! - `opencode mcp list` reports a status per server and no tool names.

use serde_json::{json, Map, Value};

use std::time::Duration;

use super::claude_mcp::ClaudeMcpAdapter;
use super::mcp_adapter::{
    list_probe_arguments, listed_probe, McpAdapter, McpPreparation, ProbePlan,
};
use super::RuntimeError;
use crate::application::mcp::{LaunchEnv, LaunchServer, McpLaunch, McpProbe};
use crate::application::process::ProcessOutput;
use crate::domain::mcp::{
    McpFeatures, McpProbeKind, McpServerStatus, McpToolFilter, ReportedMcpTool,
};

/// How long `OpenCode` waits for a server to start, in milliseconds.
const STARTUP_MS: u64 = 120_000;

pub struct OpenCodeMcpAdapter;

impl McpAdapter for OpenCodeMcpAdapter {
    fn program(&self) -> &'static str {
        "opencode"
    }

    /// Measured with a real model: an allow-list (the `tools` map), a status-only probe, and the
    /// user's own servers still load.
    fn features(&self) -> McpFeatures {
        McpFeatures {
            tool_filter: McpToolFilter::AllowList,
            probe: McpProbeKind::StatusOnly,
            strict: false,
        }
    }

    /// The whole config travels in the environment: no file, no argument.
    fn prepare(&self, launch: &McpLaunch) -> Result<McpPreparation, RuntimeError> {
        Ok(McpPreparation::with_files(
            Vec::new(),
            Self::environment(launch),
            Vec::new(),
        ))
    }

    fn probe_plan(&self, server: &LaunchServer) -> Option<Result<ProbePlan, RuntimeError>> {
        Some(Ok(ProbePlan::new(
            list_probe_arguments(),
            Self::probe_environment(server),
            Duration::from_secs(180),
            read_probe,
            Vec::new(),
        )))
    }

    fn classify(&self, reported: &[String], launched: &[&str]) -> Vec<ReportedMcpTool> {
        OpenCodeMcpAdapter::classify(reported, launched)
    }
}

/// The server's status from `opencode mcp list`. It lists no tools.
fn read_probe(output: &ProcessOutput, name: &str) -> Result<McpProbe, RuntimeError> {
    listed_probe(OpenCodeMcpAdapter::status_in(&output.stdout, name))
}

impl OpenCodeMcpAdapter {
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
                            "{{env:{}}}",
                            ClaudeMcpAdapter::secret_variable(&server.name, name)
                        )
                    }
                };
                env.insert(var.name().to_owned(), Value::String(value));
            }
            let mut command = vec![server.executable.clone()];
            command.extend(server.args.iter().cloned());
            let mut entry = Map::new();
            entry.insert("type".to_owned(), json!("local"));
            entry.insert("command".to_owned(), json!(command));
            if !env.is_empty() {
                entry.insert("environment".to_owned(), Value::Object(env));
            }
            entry.insert("enabled".to_owned(), json!(true));
            entry.insert("timeout".to_owned(), json!(STARTUP_MS));
            servers.insert(server.name.clone(), Value::Object(entry));
        }
        let mut config = json!({ "mcp": servers });
        // A server held to the tools named: every tool of it off, then those on (measured with a
        // real model: the tool left on was called and the other was "not available"). The prefix
        // is the server's name, and OpenCode may write it with `-` as `_`, so both are covered.
        let mut tools = Map::new();
        for (server, allowed) in &launch.only {
            for prefix in Self::prefixes(server) {
                tools.insert(format!("{prefix}_*"), json!(false));
                for tool in allowed {
                    tools.insert(format!("{prefix}_{tool}"), json!(true));
                }
            }
        }
        if !tools.is_empty() {
            config["tools"] = Value::Object(tools);
        }
        config.to_string()
    }

    /// The names a server's tools are prefixed with.
    fn prefixes(server: &str) -> Vec<String> {
        let mut names = vec![server.to_owned()];
        let sanitized = server.replace('-', "_");
        if sanitized != server {
            names.push(sanitized);
        }
        names
    }

    /// Which of the tools a run used were a launched server's: `<server>_<tool>`.
    pub fn classify(
        reported: &[String],
        launched: &[&str],
    ) -> Vec<crate::domain::mcp::ReportedMcpTool> {
        reported
            .iter()
            .filter_map(|name| {
                launched
                    .iter()
                    .flat_map(|server| {
                        Self::prefixes(server)
                            .into_iter()
                            .map(move |p| ((*server).to_owned(), p))
                    })
                    .filter_map(|(server, prefix)| {
                        name.strip_prefix(&format!("{prefix}_"))
                            .map(|tool| (server, tool.to_owned()))
                    })
                    .max_by_key(|(server, _)| server.len())
            })
            .map(|(server, tool)| crate::domain::mcp::ReportedMcpTool {
                server: Some(server),
                tool,
            })
            .collect()
    }

    /// The CLI's environment: the config, and the secrets it refers to.
    pub fn environment(launch: &McpLaunch) -> Vec<(String, String)> {
        if launch.is_empty() {
            return Vec::new();
        }
        let mut env = vec![(
            "OPENCODE_CONFIG_CONTENT".to_owned(),
            Self::config_json(launch),
        )];
        env.extend(ClaudeMcpAdapter::secret_environment(launch));
        env
    }

    /// What `opencode mcp list` says about `name`, once its colours are taken off:
    /// `●  ✓ name connected` or `●  ✗ name failed`.
    pub fn status_in(stdout: &str, name: &str) -> Option<McpServerStatus> {
        let plain = strip_ansi(stdout);
        plain.lines().find_map(|line| {
            let rest = line
                .trim_start_matches(|c: char| c == '│' || c == '●' || c.is_whitespace())
                .strip_prefix(['✓', '✗'])?
                .trim_start();
            let mut words = rest.split_whitespace();
            (words.next()? == name).then(|| match words.next() {
                Some("connected") => McpServerStatus::Connected,
                _ => McpServerStatus::Failed,
            })
        })
    }

    pub fn probe_environment(server: &LaunchServer) -> Vec<(String, String)> {
        Self::environment(&McpLaunch::of_one(server))
    }
}

/// The text without terminal colour codes (`ESC [ … m`).
fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::mcp::Secret;

    fn server(env: Vec<LaunchEnv>) -> LaunchServer {
        LaunchServer {
            name: "files".to_owned(),
            executable: "node".to_owned(),
            args: vec![
                "server.js".to_owned(),
                "--root".to_owned(),
                "/a b".to_owned(),
            ],
            env,
        }
    }

    #[test]
    fn the_config_carries_a_reference_where_a_secret_goes_and_never_the_secret() {
        let launch = McpLaunch {
            servers: vec![server(vec![
                LaunchEnv::Secret {
                    name: "API_TOKEN".to_owned(),
                    value: Secret::new("hunter2hunter2".to_owned()),
                },
                LaunchEnv::Plain {
                    name: "REGION".to_owned(),
                    value: "eu".to_owned(),
                },
            ])],
            held_back: vec![],
            only: vec![],
        };

        let json = OpenCodeMcpAdapter::config_json(&launch);
        let parsed: Value = serde_json::from_str(&json).unwrap();

        assert!(!json.contains("hunter2"));
        let entry = &parsed["mcp"]["files"];
        assert_eq!(entry["type"], "local");
        assert_eq!(
            entry["command"],
            json!(["node", "server.js", "--root", "/a b"])
        );
        assert_eq!(
            entry["environment"]["API_TOKEN"],
            "{env:ATLAS_MCP_FILES_API_TOKEN}"
        );
        assert_eq!(entry["environment"]["REGION"], "eu");
        let env = OpenCodeMcpAdapter::environment(&launch);
        assert_eq!(env[0].0, "OPENCODE_CONFIG_CONTENT");
        assert!(env.contains(&(
            "ATLAS_MCP_FILES_API_TOKEN".to_owned(),
            "hunter2hunter2".to_owned()
        )));
        assert_eq!(
            OpenCodeMcpAdapter::environment(&McpLaunch::default()).len(),
            0
        );
    }

    #[test]
    fn named_tools_turn_every_other_tool_of_the_server_off() {
        let launch = McpLaunch {
            servers: vec![server(vec![])],
            held_back: vec![],
            only: vec![("files".to_owned(), vec!["read".to_owned()])],
        };

        let parsed: Value =
            serde_json::from_str(&OpenCodeMcpAdapter::config_json(&launch)).unwrap();

        assert_eq!(parsed["tools"]["files_*"], false);
        assert_eq!(parsed["tools"]["files_read"], true);
        // Nothing named, nothing turned off.
        let whole = McpLaunch {
            only: vec![],
            ..launch
        };
        let parsed: Value = serde_json::from_str(&OpenCodeMcpAdapter::config_json(&whole)).unwrap();
        assert!(parsed.get("tools").is_none());
    }

    #[test]
    fn a_used_tool_is_told_back_to_its_server_even_when_the_name_has_a_dash() {
        let found = OpenCodeMcpAdapter::classify(
            &[
                "chrome_devtools_navigate_page".to_owned(),
                "files_read".to_owned(),
                "bash".to_owned(),
            ],
            &["chrome-devtools", "files"],
        );

        assert_eq!(found.len(), 2);
        assert_eq!(found[0].server.as_deref(), Some("chrome-devtools"));
        assert_eq!(found[0].tool, "navigate_page");
        assert_eq!(found[1].tool, "read");
    }

    #[test]
    fn the_status_is_read_from_the_coloured_listing() {
        let out = "\u{1b}[0m\n┌  MCP Servers\n│\n\u{1b}[32m●\u{1b}[0m  \u{1b}[32m✓\u{1b}[0m files \u{1b}[2mconnected\u{1b}[0m\n│      node /a.js\n\u{1b}[31m●\u{1b}[0m  ✗ web failed\n└  2 server(s)\n";

        assert_eq!(
            OpenCodeMcpAdapter::status_in(out, "files"),
            Some(McpServerStatus::Connected)
        );
        assert_eq!(
            OpenCodeMcpAdapter::status_in(out, "web"),
            Some(McpServerStatus::Failed)
        );
        assert_eq!(OpenCodeMcpAdapter::status_in(out, "ghost"), None);
    }
}
