//! Codex's way of taking MCP servers, as an adapter over Atlas's own model.
//!
//! Measured with the real CLI (0.160.0), no model call:
//!
//! - `-c mcp_servers.<name>.<key>=<toml>` adds a server for one run; nothing is written to
//!   `~/.codex/config.toml`. `codex mcp get <name> --json -c …` shows how the CLI read it, which is
//!   how the arguments below were checked. The user's own servers still load (the table is
//!   merged): this runtime is **not strict**.
//! - `enabled_tools` is an allow-list per server (a tool the server adds later is not given, and no
//!   discovery is needed); `startup_timeout_sec` is how long the CLI waits for it to start. That an
//!   allow-list is honoured by the model's calls was read in the reference, not run.
//! - `default_tools_approval_mode="approve"` is what lets a model's call go through in `codex exec`
//!   (measured with a real model: the nonce the tool returned came back; with the default the call
//!   failed with "requires approval, but approval policy is never").
//! - A secret reaches a server through `env_vars = ["NAME"]`, which forwards `NAME` from the CLI's
//!   own environment under the same name: so the secret travels in the CLI's environment under the
//!   server's own variable name, and two servers using one name cannot be launched together.
//! - `codex exec` starts the servers when the session starts and says nothing about them in its
//!   JSON events (no status, no tool list): there is no probe. A call shows as an `mcp_tool_call`
//!   item.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use super::claude_mcp::ClaudeMcpAdapter;
use super::mcp_adapter::{McpAdapter, McpPreparation};
use super::RuntimeError;
use crate::application::mcp::{LaunchEnv, McpLaunch};
use crate::domain::mcp::{McpFeatures, McpProbeKind, McpToolFilter, ReportedMcpTool};

/// How long the CLI waits for a server to start, in seconds.
const STARTUP_SECONDS: u32 = 120;

pub struct CodexMcpAdapter;

impl McpAdapter for CodexMcpAdapter {
    fn program(&self) -> &'static str {
        "codex"
    }

    /// Measured with a real model: an allow-list (`enabled_tools`), and the user's own servers
    /// still load. No probe: `codex exec` says nothing of the servers it starts.
    fn features(&self) -> McpFeatures {
        McpFeatures {
            tool_filter: McpToolFilter::AllowList,
            probe: McpProbeKind::None,
            strict: false,
        }
    }

    fn prepare(&self, launch: &McpLaunch) -> Result<McpPreparation, RuntimeError> {
        Ok(McpPreparation::with_files(
            Self::arguments(launch),
            Self::environment(launch).map_err(RuntimeError::ExecutionFailed)?,
            Vec::new(),
        ))
    }

    /// A call is reported as `mcp__<server>__<tool>` (see [`CodexMcpAdapter::tool_name`]).
    fn classify(&self, reported: &[String], launched: &[&str]) -> Vec<ReportedMcpTool> {
        ClaudeMcpAdapter::classify(reported, launched)
    }
}

/// A TOML basic string, escaped.
fn toml_string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                let _ = write!(out, "\\u{:04X}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn toml_array(items: &[String]) -> String {
    format!(
        "[{}]",
        items
            .iter()
            .map(|i| toml_string(i))
            .collect::<Vec<_>>()
            .join(",")
    )
}

impl CodexMcpAdapter {
    /// The `-c` overrides that give the CLI these servers (each a key and a value, joined by `=`
    /// when passed). Secrets are not in them: only the names `env_vars` forwards.
    pub fn overrides(launch: &McpLaunch) -> Vec<String> {
        let mut out = Vec::new();
        for server in &launch.servers {
            let key = |field: &str| format!("mcp_servers.{}.{field}", server.name);
            out.push(format!(
                "{}={}",
                key("command"),
                toml_string(&server.executable)
            ));
            out.push(format!("{}={}", key("args"), toml_array(&server.args)));
            let plain: Vec<String> = server
                .env
                .iter()
                .filter_map(|var| match var {
                    LaunchEnv::Plain { name, value } => {
                        Some(format!("{name}={}", toml_string(value)))
                    }
                    LaunchEnv::Secret { .. } => None,
                })
                .collect();
            if !plain.is_empty() {
                out.push(format!("{}={{{}}}", key("env"), plain.join(",")));
            }
            let secret: Vec<String> = server
                .env
                .iter()
                .filter_map(|var| match var {
                    LaunchEnv::Secret { name, .. } => Some(name.clone()),
                    LaunchEnv::Plain { .. } => None,
                })
                .collect();
            if !secret.is_empty() {
                out.push(format!("{}={}", key("env_vars"), toml_array(&secret)));
            }
            out.push(format!("{}={STARTUP_SECONDS}", key("startup_timeout_sec")));
            // `codex exec` cannot ask a person, and without this a call is refused with "MCP tool
            // call requires approval, but approval policy is never" (measured with a real model).
            // The person's grant is the decision; nothing else gets this.
            out.push(format!(
                "{}={}",
                key("default_tools_approval_mode"),
                toml_string("approve")
            ));
            if let Some((_, tools)) = launch.only.iter().find(|(name, _)| name == &server.name) {
                out.push(format!("{}={}", key("enabled_tools"), toml_array(tools)));
            }
        }
        out
    }

    /// The arguments for `exec`: each override after `-c`.
    pub fn arguments(launch: &McpLaunch) -> Vec<String> {
        Self::overrides(launch)
            .into_iter()
            .flat_map(|o| ["-c".to_owned(), o])
            .collect()
    }

    /// The secrets, under the names the servers read them by (what `env_vars` forwards).
    ///
    /// # Errors
    ///
    /// Two servers use one variable name for different things: they would share it.
    pub fn environment(launch: &McpLaunch) -> Result<Vec<(String, String)>, String> {
        let mut seen: BTreeMap<String, (String, String)> = BTreeMap::new();
        for server in &launch.servers {
            for var in &server.env {
                if let LaunchEnv::Secret { name, value } = var {
                    if let Some((owner, _)) = seen.get(name) {
                        return Err(format!(
                            "{owner} and {} both take a secret named {name}",
                            server.name
                        ));
                    }
                    seen.insert(
                        name.clone(),
                        (server.name.clone(), value.expose().to_owned()),
                    );
                }
            }
        }
        Ok(seen
            .into_iter()
            .map(|(name, (_, value))| (name, value))
            .collect())
    }

    /// The name a call is reported under: `mcp__<server>__<tool>`, the shape the other adapters
    /// and the manifest use.
    pub fn tool_name(server: Option<&str>, tool: &str) -> String {
        server.map_or_else(|| tool.to_owned(), |s| format!("mcp__{s}__{tool}"))
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
            args: vec!["server.js".to_owned(), "a \"b\"".to_owned()],
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
    fn strings_are_toml_strings() {
        assert_eq!(toml_string("a\\b\"c\nd"), "\"a\\\\b\\\"c\\nd\"");
        assert_eq!(toml_string("\u{7}"), "\"\\u0007\"");
    }

    #[test]
    fn a_server_becomes_overrides_with_no_secret_in_them() {
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
            only: vec![("files".to_owned(), vec!["read".to_owned()])],
        };

        let overrides = CodexMcpAdapter::overrides(&launch);

        assert_eq!(
            overrides,
            [
                "mcp_servers.files.command=\"node\"",
                "mcp_servers.files.args=[\"server.js\",\"a \\\"b\\\"\"]",
                "mcp_servers.files.env={REGION=\"eu\"}",
                "mcp_servers.files.env_vars=[\"API_TOKEN\"]",
                "mcp_servers.files.startup_timeout_sec=120",
                "mcp_servers.files.default_tools_approval_mode=\"approve\"",
                "mcp_servers.files.enabled_tools=[\"read\"]",
            ]
        );
        assert!(!overrides.join(" ").contains("hunter2"));
        let args = CodexMcpAdapter::arguments(&launch);
        assert_eq!(args[0], "-c");
        assert_eq!(args.len(), overrides.len() * 2);
        assert_eq!(CodexMcpAdapter::arguments(&McpLaunch::default()).len(), 0);
    }

    #[test]
    fn secrets_travel_under_the_names_the_servers_read_and_never_twice() {
        let one = McpLaunch {
            servers: vec![server("files", vec![secret("TOKEN", "one")])],
            held_back: vec![],
            only: vec![],
        };
        assert_eq!(
            CodexMcpAdapter::environment(&one).unwrap(),
            [("TOKEN".to_owned(), "one".to_owned())]
        );

        let clash = McpLaunch {
            servers: vec![
                server("files", vec![secret("TOKEN", "one")]),
                server("web", vec![secret("TOKEN", "two")]),
            ],
            held_back: vec![],
            only: vec![],
        };
        assert!(CodexMcpAdapter::environment(&clash).is_err());
    }

    #[test]
    fn a_call_is_reported_in_the_shape_the_manifest_uses() {
        assert_eq!(
            CodexMcpAdapter::tool_name(Some("files"), "read"),
            "mcp__files__read"
        );
        assert_eq!(CodexMcpAdapter::tool_name(None, "read"), "read");
    }
}
