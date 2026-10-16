//! Moving connections between Atlas and the configuration files of the CLIs it drives.
//!
//! Every CLI keeps its MCP servers in its own dialect: `mcpServers` in JSON (Claude Code's
//! `.mcp.json`, Gemini's `settings.json`, Cursor, Claude Desktop), `mcp` in `OpenCode`'s JSON,
//! `[mcp_servers.*]` in Codex's TOML. This module reads and writes those shapes and nothing else:
//! it never starts a process, never touches a file (the caller does) and never sees a secret value
//! it was not handed.
//!
//! The rules it keeps, because the rest of Atlas depends on them:
//!
//! - **A secret never lands in a file.** On export an environment variable that holds one becomes
//!   a *reference* the CLI expands from its own environment (`${NAME}`, `$NAME`, `{env:NAME}`, or
//!   Codex's `env_vars`). On import a reference becomes a secret Atlas declares and has no value
//!   for yet, and a literal value under a name that looks like a credential becomes a secret
//!   whose value goes to the credential store, never to the configuration.
//! - **Nothing imported is trusted.** Every server goes through the same validation as one typed
//!   by hand (no shell, no expansion, no dangerous variables); what fails is reported, not fixed.
//! - **Only what Atlas can run is carried.** HTTP/SSE servers are not implemented
//!   ([`McpTransport::Http`] is refused), so they are skipped with a reason.
//! - **Writing never overwrites.** [`merge`] adds the servers a file does not have and leaves the
//!   ones it does, byte for byte; a file it cannot parse is left alone.

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use toml_edit::{value as toml_value, Array, DocumentMut, InlineTable, Item, Table};

use super::validate::{valid_name, MAX_NAME};
use crate::domain::mcp::{McpConnection, McpEnv, McpEnvValue, McpTransport};

/// A configuration dialect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigDialect {
    /// The common `{"mcpServers": {…}}` shape: Cursor, Claude Desktop, most READMEs.
    Mcp,
    /// Claude Code: a project's `.mcp.json`.
    Claude,
    /// Gemini CLI: `~/.gemini/settings.json`.
    Gemini,
    /// `OpenCode`: `opencode.json`.
    OpenCode,
    /// Codex: `~/.codex/config.toml`.
    Codex,
    /// GitHub Copilot CLI: `~/.copilot/mcp-config.json`.
    Copilot,
    /// Antigravity (`agy`): no file Atlas may edit; servers are added with `agy mcp add`.
    Antigravity,
}

/// Why a server was not carried over, as a code the UI words.
pub mod skip {
    pub const HTTP: &str = "http_not_supported";
    pub const NO_COMMAND: &str = "no_command";
    pub const NAME: &str = "name";
    pub const NAME_TAKEN: &str = "name_taken";
    pub const SHAPE: &str = "shape";
}

/// Something about a server that was understood and left out on purpose.
pub mod warn {
    /// The source held the server to some tools; grants do that in Atlas, per agent.
    pub const TOOL_FILTER: &str = "tool_filter_not_imported";
    /// The source marked the server disabled; an Atlas connection starts off anyway.
    pub const DISABLED: &str = "disabled_in_source";
    /// The source skips the CLI's confirmation (`trust`, an approval mode); Atlas does not copy it.
    pub const TRUST: &str = "trust_not_imported";
    /// The name had to change to be one Atlas accepts.
    pub const RENAMED: &str = "renamed";
}

/// A server read from a file, ready to be added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedServer {
    /// The name as Atlas will know it (sanitized).
    pub name: String,
    pub transport: McpTransport,
    /// Literal values found under a credential-looking name: for the credential store, never the
    /// configuration. By environment variable name.
    pub secret_values: Vec<(String, String)>,
    pub warnings: Vec<&'static str>,
}

/// A server left out, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Skipped {
    pub name: String,
    pub reason: String,
}

/// Everything a file held.
#[derive(Debug, Default)]
pub struct Parsed {
    pub servers: Vec<ParsedServer>,
    pub skipped: Vec<Skipped>,
}

/// The text could not be read as the dialect it was said to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// Not JSON / not TOML (comments in a JSON file count: Atlas will not guess).
    Unreadable,
    /// It parsed, and is not the shape of the dialect (no servers table, wrong type).
    NotMcpConfig,
    /// Antigravity has no file to read.
    Unsupported,
}

// ---------------------------------------------------------------------------------------------
// Names and environment

/// A name Atlas accepts, from one a file used (`GitHub MCP`, `filesystem_`…). `None` when nothing
/// usable is left.
pub fn sanitize_name(raw: &str) -> Option<String> {
    let mut name = String::new();
    for c in raw.trim().to_lowercase().chars() {
        let c = if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_' {
            c
        } else {
            '-'
        };
        // A runtime joins server and tool with `__`; and a run of dashes says nothing.
        if (c == '_' && name.ends_with('_')) || (c == '-' && name.ends_with('-')) {
            continue;
        }
        name.push(c);
    }
    let name = name.trim_matches(['-', '_']).to_owned();
    let name: String = name.chars().take(MAX_NAME).collect();
    let name = name.trim_end_matches(['-', '_']).to_owned();
    (valid_name(&name)).then_some(name)
}

/// What an environment value is.
#[derive(Debug, PartialEq, Eq)]
enum EnvKind {
    Plain(String),
    /// `${NAME}`, `$NAME`, `{env:NAME}`: the value lives in the CLI's environment.
    Reference,
    /// A literal under a name that reads like a credential.
    SecretLiteral(String),
}

fn reference_name(value: &str) -> Option<&str> {
    let inner = value
        .strip_prefix("${")
        .and_then(|v| v.strip_suffix('}'))
        .or_else(|| {
            value
                .strip_prefix("{env:")
                .and_then(|v| v.strip_suffix('}'))
        })
        .or_else(|| value.strip_prefix('$'))?;
    (!inner.is_empty() && inner.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        .then_some(inner)
}

fn looks_like_credential(name: &str) -> bool {
    let upper = name.to_uppercase();
    [
        "TOKEN",
        "KEY",
        "SECRET",
        "PASSWORD",
        "PASSWD",
        "CREDENTIAL",
        "AUTH",
        "COOKIE",
        "_PAT",
    ]
    .iter()
    .any(|marker| upper.contains(marker))
}

fn classify_env(name: &str, value: &str) -> EnvKind {
    if reference_name(value).is_some() {
        EnvKind::Reference
    } else if !value.is_empty() && looks_like_credential(name) {
        EnvKind::SecretLiteral(value.to_owned())
    } else {
        EnvKind::Plain(value.to_owned())
    }
}

/// Turns the `(name, value)` pairs of a file into Atlas's environment: plain values stay, anything
/// secret becomes a declaration (and its literal, when there is one, is returned apart).
fn environment(pairs: Vec<(String, String)>) -> (Vec<McpEnv>, Vec<(String, String)>) {
    let mut env = Vec::new();
    let mut literals = Vec::new();
    for (name, value) in pairs {
        match classify_env(&name, &value) {
            EnvKind::Plain(value) => env.push(McpEnv {
                name,
                value: McpEnvValue::Plain { value },
            }),
            EnvKind::Reference => env.push(McpEnv {
                name,
                value: McpEnvValue::Secret,
            }),
            EnvKind::SecretLiteral(value) => {
                literals.push((name.clone(), value));
                env.push(McpEnv {
                    name,
                    value: McpEnvValue::Secret,
                });
            }
        }
    }
    (env, literals)
}

/// A server being read: what the dialect-specific code found, before names and secrets are dealt
/// with.
struct Raw {
    name: String,
    executable: String,
    args: Vec<String>,
    env: Vec<(String, String)>,
    /// Environment variable names a file forwards from the CLI's environment (Codex `env_vars`).
    forwarded: Vec<String>,
    warnings: Vec<&'static str>,
}

fn finish(raw: Raw, parsed: &mut Parsed) {
    let Some(name) = sanitize_name(&raw.name) else {
        parsed.skipped.push(Skipped {
            name: raw.name,
            reason: skip::NAME.to_owned(),
        });
        return;
    };
    let mut warnings = raw.warnings;
    if name != raw.name {
        warnings.push(warn::RENAMED);
    }
    let (mut env, secret_values) = environment(raw.env);
    for forwarded in raw.forwarded {
        if !env.iter().any(|e| e.name == forwarded) {
            env.push(McpEnv {
                name: forwarded,
                value: McpEnvValue::Secret,
            });
        }
    }
    parsed.servers.push(ParsedServer {
        name,
        transport: McpTransport::Stdio {
            executable: raw.executable,
            args: raw.args,
            env,
        },
        secret_values,
        warnings,
    });
}

fn strings(value: Option<&Value>) -> Option<Vec<String>> {
    value?
        .as_array()?
        .iter()
        .map(|v| v.as_str().map(str::to_owned))
        .collect()
}

fn string_pairs(value: Option<&Value>) -> Option<Vec<(String, String)>> {
    let Some(value) = value else {
        return Some(Vec::new());
    };
    value
        .as_object()?
        .iter()
        .map(|(k, v)| match v {
            Value::String(s) => Some((k.clone(), s.clone())),
            Value::Number(n) => Some((k.clone(), n.to_string())),
            Value::Bool(b) => Some((k.clone(), b.to_string())),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Reading

/// Which dialect a text looks like, for a person who pasted it without saying.
pub fn detect(text: &str) -> Option<ConfigDialect> {
    if let Ok(Value::Object(root)) = serde_json::from_str::<Value>(text) {
        if root.contains_key("mcpServers") {
            return Some(ConfigDialect::Mcp);
        }
        if root.contains_key("mcp") {
            return Some(ConfigDialect::OpenCode);
        }
        // A bare table of servers: `{"files": {"command": …}}`.
        let bare = !root.is_empty()
            && root
                .values()
                .all(|v| v.get("command").is_some() || v.get("url").is_some());
        return bare.then_some(ConfigDialect::Mcp);
    }
    text.parse::<DocumentMut>()
        .ok()
        .filter(|doc| doc.get("mcp_servers").is_some())
        .map(|_| ConfigDialect::Codex)
}

/// The servers of a file of `dialect`.
///
/// # Errors
///
/// The text is not that dialect's format, or has no servers table.
pub fn parse(dialect: ConfigDialect, text: &str) -> Result<Parsed, ConfigError> {
    match dialect {
        ConfigDialect::Mcp
        | ConfigDialect::Claude
        | ConfigDialect::Gemini
        | ConfigDialect::Copilot => parse_mcp_servers(dialect, text),
        ConfigDialect::OpenCode => parse_opencode(text),
        ConfigDialect::Codex => parse_codex(text),
        ConfigDialect::Antigravity => Err(ConfigError::Unsupported),
    }
}

fn root_object(text: &str) -> Result<Map<String, Value>, ConfigError> {
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(root)) => Ok(root),
        Ok(_) => Err(ConfigError::NotMcpConfig),
        Err(_) => Err(ConfigError::Unreadable),
    }
}

fn is_remote(entry: &Map<String, Value>) -> bool {
    ["url", "httpUrl", "serverUrl", "sseUrl"]
        .iter()
        .any(|k| entry.contains_key(*k))
        || matches!(
            entry.get("type").and_then(Value::as_str),
            Some("sse" | "http" | "streamable-http" | "streamableHttp" | "remote")
        )
}

fn parse_mcp_servers(dialect: ConfigDialect, text: &str) -> Result<Parsed, ConfigError> {
    let root = root_object(text)?;
    let table = match root.get("mcpServers") {
        Some(Value::Object(table)) => table.clone(),
        // Gemini and Claude files hold much else; a bare table of servers is allowed for Mcp only.
        None if dialect == ConfigDialect::Mcp && detect(text) == Some(ConfigDialect::Mcp) => root,
        _ => return Err(ConfigError::NotMcpConfig),
    };
    let mut parsed = Parsed::default();
    for (name, entry) in table {
        let Value::Object(entry) = entry else {
            parsed.skipped.push(Skipped {
                name,
                reason: skip::SHAPE.to_owned(),
            });
            continue;
        };
        if is_remote(&entry) && !entry.contains_key("command") {
            parsed.skipped.push(Skipped {
                name,
                reason: skip::HTTP.to_owned(),
            });
            continue;
        }
        let Some(executable) = entry.get("command").and_then(Value::as_str) else {
            parsed.skipped.push(Skipped {
                name,
                reason: skip::NO_COMMAND.to_owned(),
            });
            continue;
        };
        let (Some(args), Some(env)) = (
            strings(entry.get("args")).or_else(|| entry.get("args").is_none().then(Vec::new)),
            string_pairs(entry.get("env")),
        ) else {
            parsed.skipped.push(Skipped {
                name,
                reason: skip::SHAPE.to_owned(),
            });
            continue;
        };
        let mut warnings = Vec::new();
        if entry.contains_key("includeTools") || entry.contains_key("excludeTools") {
            warnings.push(warn::TOOL_FILTER);
        }
        if entry.get("trust").and_then(Value::as_bool) == Some(true) {
            warnings.push(warn::TRUST);
        }
        if entry.get("disabled").and_then(Value::as_bool) == Some(true) {
            warnings.push(warn::DISABLED);
        }
        if dialect == ConfigDialect::Copilot {
            // `tools` is the CLI's allow-list (`["*"]` is all of it), and it keeps an off switch.
            let all = entry.get("tools").is_none_or(|tools| {
                strings(Some(tools))
                    .is_some_and(|list| !list.is_empty() && list.iter().all(|t| t == "*"))
            });
            if !all {
                warnings.push(warn::TOOL_FILTER);
            }
            if entry.get("enabled").and_then(Value::as_bool) == Some(false) {
                warnings.push(warn::DISABLED);
            }
        }
        finish(
            Raw {
                name,
                executable: executable.to_owned(),
                args,
                env,
                forwarded: Vec::new(),
                warnings,
            },
            &mut parsed,
        );
    }
    Ok(parsed)
}

fn parse_opencode(text: &str) -> Result<Parsed, ConfigError> {
    let root = root_object(text)?;
    let Some(Value::Object(table)) = root.get("mcp") else {
        return Err(ConfigError::NotMcpConfig);
    };
    let mut parsed = Parsed::default();
    for (name, entry) in table {
        let Some(entry) = entry.as_object() else {
            parsed.skipped.push(Skipped {
                name: name.clone(),
                reason: skip::SHAPE.to_owned(),
            });
            continue;
        };
        if entry.get("type").and_then(Value::as_str) == Some("remote") || entry.contains_key("url")
        {
            parsed.skipped.push(Skipped {
                name: name.clone(),
                reason: skip::HTTP.to_owned(),
            });
            continue;
        }
        let Some(command) = strings(entry.get("command")).filter(|c| !c.is_empty()) else {
            parsed.skipped.push(Skipped {
                name: name.clone(),
                reason: skip::NO_COMMAND.to_owned(),
            });
            continue;
        };
        let Some(env) = string_pairs(entry.get("environment")) else {
            parsed.skipped.push(Skipped {
                name: name.clone(),
                reason: skip::SHAPE.to_owned(),
            });
            continue;
        };
        let mut warnings = Vec::new();
        if entry.get("enabled").and_then(Value::as_bool) == Some(false) {
            warnings.push(warn::DISABLED);
        }
        let mut command = command.into_iter();
        let executable = command.next().unwrap_or_default();
        finish(
            Raw {
                name: name.clone(),
                executable,
                args: command.collect(),
                env,
                forwarded: Vec::new(),
                warnings,
            },
            &mut parsed,
        );
    }
    Ok(parsed)
}

fn toml_strings(item: Option<&Item>) -> Option<Vec<String>> {
    item?
        .as_array()?
        .iter()
        .map(|v| v.as_str().map(str::to_owned))
        .collect()
}

fn parse_codex(text: &str) -> Result<Parsed, ConfigError> {
    let doc: DocumentMut = text.parse().map_err(|_| ConfigError::Unreadable)?;
    let Some(table) = doc.get("mcp_servers").and_then(Item::as_table_like) else {
        return Err(ConfigError::NotMcpConfig);
    };
    let mut parsed = Parsed::default();
    for (name, entry) in table.iter() {
        let Some(entry) = entry.as_table_like() else {
            parsed.skipped.push(Skipped {
                name: name.to_owned(),
                reason: skip::SHAPE.to_owned(),
            });
            continue;
        };
        if entry.contains_key("url") && !entry.contains_key("command") {
            parsed.skipped.push(Skipped {
                name: name.to_owned(),
                reason: skip::HTTP.to_owned(),
            });
            continue;
        }
        let Some(executable) = entry.get("command").and_then(Item::as_str) else {
            parsed.skipped.push(Skipped {
                name: name.to_owned(),
                reason: skip::NO_COMMAND.to_owned(),
            });
            continue;
        };
        let args = if entry.contains_key("args") {
            toml_strings(entry.get("args"))
        } else {
            Some(Vec::new())
        };
        let env: Option<Vec<(String, String)>> = match entry.get("env") {
            None => Some(Vec::new()),
            Some(item) => item.as_table_like().and_then(|t| {
                t.iter()
                    .map(|(k, v)| v.as_str().map(|v| (k.to_owned(), v.to_owned())))
                    .collect()
            }),
        };
        let forwarded = if entry.contains_key("env_vars") {
            toml_strings(entry.get("env_vars"))
        } else {
            Some(Vec::new())
        };
        let (Some(args), Some(env), Some(forwarded)) = (args, env, forwarded) else {
            parsed.skipped.push(Skipped {
                name: name.to_owned(),
                reason: skip::SHAPE.to_owned(),
            });
            continue;
        };
        let mut warnings = Vec::new();
        if entry.contains_key("enabled_tools") || entry.contains_key("disabled_tools") {
            warnings.push(warn::TOOL_FILTER);
        }
        if entry.contains_key("default_tools_approval_mode") {
            warnings.push(warn::TRUST);
        }
        if entry.get("enabled").and_then(Item::as_bool) == Some(false) {
            warnings.push(warn::DISABLED);
        }
        finish(
            Raw {
                name: name.to_owned(),
                executable: executable.to_owned(),
                args,
                env,
                forwarded,
                warnings,
            },
            &mut parsed,
        );
    }
    Ok(parsed)
}

// ---------------------------------------------------------------------------------------------
// Writing

fn stdio(connection: &McpConnection) -> Option<(&str, &[String], &[McpEnv])> {
    match &connection.transport {
        McpTransport::Stdio {
            executable,
            args,
            env,
        } => Some((executable, args, env)),
        McpTransport::Http { .. } => None,
    }
}

/// How a secret is referred to in a dialect: the CLI expands it from its own environment.
fn reference(dialect: ConfigDialect, name: &str) -> String {
    match dialect {
        ConfigDialect::Gemini => format!("${name}"),
        ConfigDialect::OpenCode => format!("{{env:{name}}}"),
        _ => format!("${{{name}}}"),
    }
}

fn env_object(dialect: ConfigDialect, env: &[McpEnv]) -> Map<String, Value> {
    env.iter()
        .map(|var| {
            let value = match &var.value {
                McpEnvValue::Plain { value } => value.clone(),
                McpEnvValue::Secret => reference(dialect, &var.name),
            };
            (var.name.clone(), Value::String(value))
        })
        .collect()
}

/// The entry for one server in a JSON dialect.
fn json_entry(dialect: ConfigDialect, connection: &McpConnection) -> Option<Value> {
    let (executable, args, env) = stdio(connection)?;
    let env = env_object(dialect, env);
    let mut entry = Map::new();
    if dialect == ConfigDialect::OpenCode {
        let mut command = vec![executable.to_owned()];
        command.extend(args.iter().cloned());
        entry.insert("type".to_owned(), json!("local"));
        entry.insert("command".to_owned(), json!(command));
        if !env.is_empty() {
            entry.insert("environment".to_owned(), Value::Object(env));
        }
        // In the CLI's own file the server is meant to work: Atlas's own on/off is Atlas's.
        entry.insert("enabled".to_owned(), json!(true));
    } else {
        if dialect == ConfigDialect::Claude {
            entry.insert("type".to_owned(), json!("stdio"));
        }
        if dialect == ConfigDialect::Copilot {
            entry.insert("type".to_owned(), json!("local"));
        }
        entry.insert("command".to_owned(), json!(executable));
        entry.insert("args".to_owned(), json!(args));
        if !env.is_empty() {
            entry.insert("env".to_owned(), Value::Object(env));
        }
        if dialect == ConfigDialect::Copilot {
            // Every tool of the server: what a grant allows is Atlas's to decide, per agent.
            entry.insert("tools".to_owned(), json!(["*"]));
        }
    }
    Some(Value::Object(entry))
}

/// The table of one server in Codex's TOML.
fn toml_entry(connection: &McpConnection) -> Option<Table> {
    let (executable, args, env) = stdio(connection)?;
    let mut table = Table::new();
    table["command"] = toml_value(executable);
    let mut list = Array::new();
    for arg in args {
        list.push(arg.as_str());
    }
    table["args"] = toml_value(list);
    let mut plain = InlineTable::new();
    let mut forwarded = Array::new();
    for var in env {
        match &var.value {
            McpEnvValue::Plain { value } => {
                plain.insert(&var.name, value.as_str().into());
            }
            McpEnvValue::Secret => forwarded.push(var.name.as_str()),
        }
    }
    if !plain.is_empty() {
        table["env"] = toml_value(plain);
    }
    if !forwarded.is_empty() {
        table["env_vars"] = toml_value(forwarded);
    }
    Some(table)
}

fn root_key(dialect: ConfigDialect) -> &'static str {
    if dialect == ConfigDialect::OpenCode {
        "mcp"
    } else {
        "mcpServers"
    }
}

fn shell_quote(text: &str) -> String {
    if !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./:@=+,".contains(c))
    {
        text.to_owned()
    } else {
        format!("'{}'", text.replace('\'', "'\\''"))
    }
}

/// `agy mcp add` lines: the only way Antigravity takes a server. A secret is not in them.
fn antigravity_text(connections: &[&McpConnection]) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    for connection in connections {
        let Some((executable, args, env)) = stdio(connection) else {
            continue;
        };
        let mut line = String::from("agy mcp add");
        for var in env {
            match &var.value {
                McpEnvValue::Plain { value } => {
                    let _ = write!(
                        line,
                        " --env {}",
                        shell_quote(&format!("{}={value}", var.name))
                    );
                }
                McpEnvValue::Secret => {
                    let _ = writeln!(
                        out,
                        "# {}: secret. Add `--env {}=<value>` yourself; Atlas does not export secrets.",
                        connection.name, var.name
                    );
                }
            }
        }
        let _ = write!(
            line,
            " {} -- {}",
            shell_quote(&connection.name),
            shell_quote(executable)
        );
        for arg in args {
            line.push(' ');
            line.push_str(&shell_quote(arg));
        }
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// The connections as the text of a `dialect` file. Secrets are references, never values.
pub fn render(dialect: ConfigDialect, connections: &[&McpConnection]) -> String {
    match dialect {
        ConfigDialect::Codex => {
            let mut doc = DocumentMut::new();
            let mut servers = Table::new();
            servers.set_implicit(true);
            for connection in connections {
                if let Some(table) = toml_entry(connection) {
                    servers[&connection.name] = Item::Table(table);
                }
            }
            doc["mcp_servers"] = Item::Table(servers);
            doc.to_string()
        }
        ConfigDialect::Antigravity => antigravity_text(connections),
        _ => {
            let mut servers = Map::new();
            for connection in connections {
                if let Some(entry) = json_entry(dialect, connection) {
                    servers.insert(connection.name.clone(), entry);
                }
            }
            let mut root = Map::new();
            if dialect == ConfigDialect::OpenCode {
                root.insert(
                    "$schema".to_owned(),
                    json!("https://opencode.ai/config.json"),
                );
            }
            root.insert(root_key(dialect).to_owned(), Value::Object(servers));
            let mut text = serde_json::to_string_pretty(&Value::Object(root)).unwrap_or_default();
            text.push('\n');
            text
        }
    }
}

/// What adding servers to a file did.
#[derive(Debug, PartialEq, Eq)]
pub struct Merged {
    pub text: String,
    pub added: Vec<String>,
    /// Already in the file: left exactly as they were.
    pub kept: Vec<String>,
}

/// `existing` (a file of `dialect`, or nothing) with the connections it lacks added.
///
/// # Errors
///
/// The file does not parse (a JSON file with comments included) or has no place for servers: it
/// is not written to, and the caller says so. Antigravity has no file.
pub fn merge(
    dialect: ConfigDialect,
    existing: Option<&str>,
    connections: &[&McpConnection],
) -> Result<Merged, ConfigError> {
    let (mut added, mut kept) = (Vec::new(), Vec::new());
    let text = match dialect {
        ConfigDialect::Antigravity => return Err(ConfigError::Unsupported),
        ConfigDialect::Codex => {
            let mut doc: DocumentMut = match existing.map(str::trim) {
                None | Some("") => DocumentMut::new(),
                Some(_) => existing
                    .unwrap_or_default()
                    .parse()
                    .map_err(|_| ConfigError::Unreadable)?,
            };
            if doc.get("mcp_servers").is_none() {
                let mut servers = Table::new();
                servers.set_implicit(true);
                doc["mcp_servers"] = Item::Table(servers);
            }
            let servers = doc["mcp_servers"]
                .as_table_like_mut()
                .ok_or(ConfigError::NotMcpConfig)?;
            for connection in connections {
                if servers.contains_key(&connection.name) {
                    kept.push(connection.name.clone());
                } else if let Some(table) = toml_entry(connection) {
                    servers.insert(&connection.name, Item::Table(table));
                    added.push(connection.name.clone());
                }
            }
            doc.to_string()
        }
        _ => {
            let mut root = match existing.map(str::trim) {
                None | Some("") => Map::new(),
                Some(text) => root_object(text)?,
            };
            let key = root_key(dialect);
            let servers = root
                .entry(key.to_owned())
                .or_insert_with(|| Value::Object(Map::new()))
                .as_object_mut()
                .ok_or(ConfigError::NotMcpConfig)?;
            for connection in connections {
                if servers.contains_key(&connection.name) {
                    kept.push(connection.name.clone());
                } else if let Some(entry) = json_entry(dialect, connection) {
                    servers.insert(connection.name.clone(), entry);
                    added.push(connection.name.clone());
                }
            }
            let mut text = serde_json::to_string_pretty(&Value::Object(root)).unwrap_or_default();
            text.push('\n');
            text
        }
    };
    Ok(Merged { text, added, kept })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::mcp::tests::connection;

    fn with_env(name: &str, env: Vec<McpEnv>) -> McpConnection {
        McpConnection {
            transport: McpTransport::Stdio {
                executable: "npx".to_owned(),
                args: vec!["-y".to_owned(), "@scope/server".to_owned()],
                env,
            },
            ..connection(name)
        }
    }

    fn env(name: &str, value: Option<&str>) -> McpEnv {
        McpEnv {
            name: name.to_owned(),
            value: value.map_or(McpEnvValue::Secret, |v| McpEnvValue::Plain {
                value: v.to_owned(),
            }),
        }
    }

    fn sample() -> McpConnection {
        with_env(
            "github",
            vec![env("REGION", Some("eu")), env("GITHUB_TOKEN", None)],
        )
    }

    #[test]
    fn names_are_made_into_what_atlas_accepts() {
        assert_eq!(sanitize_name("GitHub MCP"), Some("github-mcp".to_owned()));
        assert_eq!(sanitize_name("__a__b__"), Some("a_b".to_owned()));
        assert_eq!(sanitize_name("  "), None);
        assert_eq!(sanitize_name(&"x".repeat(60)).unwrap().len(), MAX_NAME);
    }

    #[test]
    fn references_and_credential_names_are_told_from_plain_values() {
        assert_eq!(classify_env("T", "${GH}"), EnvKind::Reference);
        assert_eq!(classify_env("T", "$GH"), EnvKind::Reference);
        assert_eq!(classify_env("T", "{env:GH}"), EnvKind::Reference);
        assert_eq!(
            classify_env("API_TOKEN", "abc"),
            EnvKind::SecretLiteral("abc".to_owned())
        );
        assert_eq!(
            classify_env("REGION", "eu"),
            EnvKind::Plain("eu".to_owned())
        );
        // An empty credential is not a secret anyone has.
        assert_eq!(classify_env("API_TOKEN", ""), EnvKind::Plain(String::new()));
    }

    #[test]
    fn the_secret_is_a_reference_in_every_dialect_and_never_the_value() {
        let connection = sample();
        let list = [&connection];

        let claude = render(ConfigDialect::Claude, &list);
        assert!(
            claude.contains("\"GITHUB_TOKEN\": \"${GITHUB_TOKEN}\""),
            "{claude}"
        );
        assert!(claude.contains("\"type\": \"stdio\""));
        let gemini = render(ConfigDialect::Gemini, &list);
        assert!(
            gemini.contains("\"GITHUB_TOKEN\": \"$GITHUB_TOKEN\""),
            "{gemini}"
        );
        let opencode = render(ConfigDialect::OpenCode, &list);
        assert!(
            opencode.contains("\"GITHUB_TOKEN\": \"{env:GITHUB_TOKEN}\""),
            "{opencode}"
        );
        assert!(opencode.contains("\"command\": [\n"), "{opencode}");
        let codex = render(ConfigDialect::Codex, &list);
        assert!(codex.contains("[mcp_servers.github]"), "{codex}");
        assert!(codex.contains("env_vars = [\"GITHUB_TOKEN\"]"), "{codex}");
        assert!(codex.contains("REGION = \"eu\""), "{codex}");
    }

    #[test]
    fn copilot_files_name_the_server_local_and_hold_every_tool() {
        let copilot = render(ConfigDialect::Copilot, &[&sample()]);

        let value: Value = serde_json::from_str(&copilot).unwrap();
        let entry = &value["mcpServers"]["github"];
        assert_eq!(entry["type"], "local");
        assert_eq!(entry["command"], "npx");
        assert_eq!(entry["tools"], json!(["*"]));
        assert_eq!(entry["env"]["GITHUB_TOKEN"], "${GITHUB_TOKEN}");
        assert_eq!(entry["env"]["REGION"], "eu");
    }

    #[test]
    fn a_copilot_server_held_to_some_tools_or_switched_off_says_so_and_a_remote_one_is_skipped() {
        let text = r#"{"mcpServers":{
            "all":{"type":"local","command":"node","args":["a.js"],"tools":["*"]},
            "some":{"type":"local","command":"node","args":["b.js"],"tools":["read"]},
            "off":{"type":"local","command":"node","args":["c.js"],"enabled":false},
            "none":{"type":"local","command":"node","args":["d.js"],"tools":[]},
            "web":{"type":"http","url":"https://x/mcp","tools":["*"]}}}"#;

        let parsed = parse(ConfigDialect::Copilot, text).unwrap();

        let warnings = |name: &str| {
            parsed
                .servers
                .iter()
                .find(|s| s.name == name)
                .map(|s| s.warnings.clone())
                .unwrap()
        };
        assert_eq!(warnings("all"), Vec::<&str>::new());
        assert_eq!(warnings("some"), [warn::TOOL_FILTER]);
        assert_eq!(warnings("off"), [warn::DISABLED]);
        // An empty list gives the model no tool of the server: that is a filter too.
        assert_eq!(warnings("none"), [warn::TOOL_FILTER]);
        assert_eq!(parsed.skipped.len(), 1);
        assert_eq!(parsed.skipped[0].reason, skip::HTTP);
    }

    #[test]
    fn adding_to_a_copilot_file_leaves_what_it_has_byte_for_byte() {
        let existing =
            r#"{"mcpServers":{"github":{"type":"http","url":"https://x/mcp","tools":["*"]}}}"#;

        let merged = merge(ConfigDialect::Copilot, Some(existing), &[&sample()]).unwrap();

        assert_eq!(merged.kept, ["github"]);
        assert_eq!(merged.added, Vec::<String>::new());
        let other = with_env("jira", vec![]);
        let merged = merge(ConfigDialect::Copilot, Some(existing), &[&other]).unwrap();
        assert_eq!(merged.added, ["jira"]);
        let value: Value = serde_json::from_str(&merged.text).unwrap();
        assert_eq!(value["mcpServers"]["github"]["url"], "https://x/mcp");
        assert_eq!(value["mcpServers"]["jira"]["type"], "local");
    }

    #[test]
    fn what_is_exported_reads_back_as_the_same_server_in_each_dialect() {
        let connection = sample();
        for dialect in [
            ConfigDialect::Mcp,
            ConfigDialect::Claude,
            ConfigDialect::Gemini,
            ConfigDialect::OpenCode,
            ConfigDialect::Codex,
            ConfigDialect::Copilot,
        ] {
            let text = render(dialect, &[&connection]);
            let parsed =
                parse(dialect, &text).unwrap_or_else(|e| panic!("{dialect:?}: {e:?}\n{text}"));

            assert!(parsed.skipped.is_empty(), "{dialect:?}");
            assert_eq!(parsed.servers.len(), 1, "{dialect:?}");
            let server = &parsed.servers[0];
            assert_eq!(server.name, "github");
            assert!(
                server.secret_values.is_empty(),
                "{dialect:?}: a reference holds no value"
            );
            let McpTransport::Stdio {
                executable,
                args,
                env: vars,
            } = &server.transport
            else {
                panic!("stdio");
            };
            assert_eq!(executable, "npx");
            assert_eq!(args, &["-y", "@scope/server"]);
            assert!(
                vars.contains(&env("GITHUB_TOKEN", None)),
                "{dialect:?}: {vars:?}"
            );
            assert!(
                vars.contains(&env("REGION", Some("eu"))),
                "{dialect:?}: {vars:?}"
            );
        }
    }

    #[test]
    fn a_literal_credential_is_set_apart_to_go_to_the_store() {
        let text = r#"{"mcpServers":{"api":{"command":"node","args":["s.js"],
            "env":{"API_KEY":"sk-live-123","MODE":"fast"}}}}"#;

        let parsed = parse(ConfigDialect::Mcp, text).unwrap();

        let server = &parsed.servers[0];
        assert_eq!(
            server.secret_values,
            [("API_KEY".to_owned(), "sk-live-123".to_owned())]
        );
        let McpTransport::Stdio { env: vars, .. } = &server.transport else {
            panic!("stdio");
        };
        // The configuration declares the secret and carries no value of it.
        assert!(vars.contains(&env("API_KEY", None)));
        assert!(!format!("{:?}", server.transport).contains("sk-live-123"));
    }

    #[test]
    fn remote_servers_are_skipped_with_the_reason_and_the_rest_are_read() {
        let text = r#"{"mcpServers":{
            "figma":{"url":"http://127.0.0.1:3845/mcp"},
            "remote":{"type":"sse","url":"https://x/sse"},
            "files":{"command":"node","args":["f.js"]},
            "broken":{"args":["x"]},
            "Odd Name!":{"command":"node"}}}"#;

        let parsed = parse(ConfigDialect::Claude, text).unwrap();

        let reasons: Vec<_> = parsed
            .skipped
            .iter()
            .map(|s| (s.name.as_str(), s.reason.as_str()))
            .collect();
        assert!(reasons.contains(&("figma", skip::HTTP)));
        assert!(reasons.contains(&("remote", skip::HTTP)));
        assert!(reasons.contains(&("broken", skip::NO_COMMAND)));
        let names: Vec<_> = parsed.servers.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["files", "odd-name"]);
        assert!(parsed.servers[1].warnings.contains(&warn::RENAMED));
    }

    #[test]
    fn what_the_source_held_back_is_reported_not_copied() {
        let gemini = r#"{"mcpServers":{"a":{"command":"node","trust":true,"includeTools":["x"]}}}"#;
        let parsed = parse(ConfigDialect::Gemini, gemini).unwrap();
        assert!(parsed.servers[0].warnings.contains(&warn::TOOL_FILTER));
        assert!(parsed.servers[0].warnings.contains(&warn::TRUST));

        let codex =
            "[mcp_servers.a]\ncommand = \"node\"\nenabled_tools = [\"x\"]\nenabled = false\n";
        let parsed = parse(ConfigDialect::Codex, codex).unwrap();
        assert!(parsed.servers[0].warnings.contains(&warn::TOOL_FILTER));
        assert!(parsed.servers[0].warnings.contains(&warn::DISABLED));
    }

    #[test]
    fn a_file_that_is_not_the_dialect_is_refused_not_guessed() {
        assert_eq!(
            parse(ConfigDialect::Claude, "not json").unwrap_err(),
            ConfigError::Unreadable
        );
        // JSON with comments is not parsed: nothing is rewritten without being understood.
        assert_eq!(
            parse(ConfigDialect::OpenCode, "{ // hi\n \"mcp\": {} }").unwrap_err(),
            ConfigError::Unreadable
        );
        assert_eq!(
            parse(ConfigDialect::OpenCode, "{\"a\":1}").unwrap_err(),
            ConfigError::NotMcpConfig
        );
        assert_eq!(
            parse(ConfigDialect::Codex, "model = \"x\"").unwrap_err(),
            ConfigError::NotMcpConfig
        );
        assert_eq!(
            parse(ConfigDialect::Antigravity, "").unwrap_err(),
            ConfigError::Unsupported
        );
    }

    #[test]
    fn a_pasted_text_is_recognised_by_its_shape() {
        assert_eq!(detect("{\"mcpServers\":{}}"), Some(ConfigDialect::Mcp));
        assert_eq!(detect("{\"mcp\":{}}"), Some(ConfigDialect::OpenCode));
        assert_eq!(
            detect("{\"files\":{\"command\":\"node\"}}"),
            Some(ConfigDialect::Mcp)
        );
        assert_eq!(
            detect("[mcp_servers.a]\ncommand = \"x\"\n"),
            Some(ConfigDialect::Codex)
        );
        assert_eq!(detect("hello"), None);
    }

    #[test]
    fn merging_adds_what_is_missing_and_leaves_what_is_there() {
        let existing = r#"{"theme":"dark","mcpServers":{"github":{"command":"mine"}}}"#;
        let other = with_env("files", vec![]);
        let github = sample();

        let merged = merge(ConfigDialect::Gemini, Some(existing), &[&github, &other]).unwrap();

        assert_eq!(merged.added, ["files"]);
        assert_eq!(merged.kept, ["github"]);
        let value: Value = serde_json::from_str(&merged.text).unwrap();
        assert_eq!(value["theme"], "dark");
        assert_eq!(value["mcpServers"]["github"]["command"], "mine");
        assert_eq!(value["mcpServers"]["files"]["command"], "npx");
        // The order of the file is the one it had.
        assert!(merged.text.find("theme").unwrap() < merged.text.find("mcpServers").unwrap());
    }

    #[test]
    fn merging_into_toml_keeps_the_comments_and_the_other_settings() {
        let existing = "# my config\nmodel = \"gpt\"\n\n[mcp_servers.old]\ncommand = \"x\"\n";
        let merged = merge(ConfigDialect::Codex, Some(existing), &[&sample()]).unwrap();

        assert_eq!(merged.added, ["github"]);
        assert!(
            merged.text.starts_with("# my config\nmodel = \"gpt\""),
            "{}",
            merged.text
        );
        assert!(merged.text.contains("[mcp_servers.old]"));
        assert!(merged.text.contains("[mcp_servers.github]"));
        assert!(!merged.text.contains("ghp_"));
    }

    #[test]
    fn merging_into_nothing_makes_the_file() {
        let merged = merge(ConfigDialect::Claude, None, &[&sample()]).unwrap();
        assert_eq!(merged.added, ["github"]);
        assert!(parse(ConfigDialect::Claude, &merged.text).is_ok());

        let merged = merge(ConfigDialect::Codex, Some(""), &[&sample()]).unwrap();
        assert!(parse(ConfigDialect::Codex, &merged.text).is_ok());
    }

    #[test]
    fn merging_never_rewrites_a_file_it_cannot_read() {
        let commented = "{ // mine\n \"mcpServers\": {} }";
        assert_eq!(
            merge(ConfigDialect::Gemini, Some(commented), &[&sample()]).unwrap_err(),
            ConfigError::Unreadable
        );
        assert_eq!(
            merge(
                ConfigDialect::Gemini,
                Some("{\"mcpServers\":[]}"),
                &[&sample()]
            )
            .unwrap_err(),
            ConfigError::NotMcpConfig
        );
        assert_eq!(
            merge(ConfigDialect::Antigravity, None, &[&sample()]).unwrap_err(),
            ConfigError::Unsupported
        );
    }

    #[test]
    fn antigravity_gets_commands_and_a_note_where_a_secret_goes() {
        let text = render(ConfigDialect::Antigravity, &[&sample()]);

        assert!(
            text.contains("agy mcp add --env REGION=eu github -- npx -y @scope/server"),
            "{text}"
        );
        assert!(text.contains("# github: secret."), "{text}");
        assert!(!text.contains("GITHUB_TOKEN=<value>\nagy") || text.contains('#'));
    }

    #[test]
    fn a_shell_quote_keeps_what_has_spaces_as_one_word() {
        assert_eq!(shell_quote("a b"), "'a b'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
        assert_eq!(shell_quote("plain-1.0"), "plain-1.0");
    }
}
