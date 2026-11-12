//! Atlas asking an MCP server about its tools (ADR 0045): `initialize`, then `tools/list`, over the
//! server's own STDIO, through the process guard (`ProcessContext::McpServer`). Only when the
//! person asks, on a connection they switched on; no model, no agent and no tool call.
//!
//! What the server answers is the server's text: kept for the person to read and for the budget,
//! never put in a prompt, never given authority. Its hints (`readOnlyHint`, `destructiveHint`) are
//! only ever used to give a read-only step *less*.
//!
//! Bounded everywhere: one deadline for the whole conversation, a cap on pages, tools, lines that
//! are not the protocol, and on what is kept of each description.

use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::launch::{LaunchEnv, LaunchServer};
use crate::application::process::{
    ProcessContext, ProcessError, ProcessRunner, ProcessSession, ProcessSpec,
};
use crate::domain::mcp::{McpToolCatalog, McpToolInfo, MAX_DESCRIPTION_CHARS};

/// The whole conversation, start included: a launcher (`npx`) may download the server first.
pub const DESCRIBE_TIMEOUT: Duration = Duration::from_secs(60);
const PROTOCOL_VERSION: &str = "2025-06-18";
const MAX_PAGES: usize = 20;
const MAX_TOOLS: usize = 500;
const MAX_NAME: usize = 128;
/// Lines that are not a message for Atlas (a server that logs on stdout, notifications).
const MAX_OTHER_LINES: usize = 1000;
const MAX_ERROR_CHARS: usize = 300;

/// Why a server could not be described. The detail is for a person; it never holds a secret
/// (Atlas never echoes the environment).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescribeError {
    /// The process could not be started (or the guard refused it).
    Start(String),
    /// The server ended, or said nothing in time.
    NoAnswer,
    /// The server answered with an error.
    Refused(String),
    /// The server said too much that was not an answer.
    TooNoisy,
}

impl DescribeError {
    pub fn detail(&self) -> String {
        match self {
            Self::Start(detail) => format!("The server could not be started: {detail}"),
            Self::NoAnswer => "The server ended or did not answer in time.".to_owned(),
            Self::Refused(message) => format!("The server answered with an error: {message}"),
            Self::TooNoisy => "The server printed too much that was not an answer.".to_owned(),
        }
    }
}

/// How the server is started to be asked: as the person configured it, its secrets in its own
/// environment (never an argument), nothing else.
pub fn spec_for(server: &LaunchServer) -> ProcessSpec {
    ProcessSpec {
        program: server.executable.clone(),
        args: server.args.clone(),
        stdin: None,
        cwd: None,
        env: server
            .env
            .iter()
            .map(|e| match e {
                LaunchEnv::Plain { name, value } => (name.clone(), value.clone()),
                LaunchEnv::Secret { name, value } => (name.clone(), value.expose().to_owned()),
            })
            .collect(),
        timeout: DESCRIBE_TIMEOUT,
        context: ProcessContext::McpServer,
        terminal: None,
    }
}

/// Starts `server`, asks for its tools and ends it.
///
/// # Errors
///
/// It could not be started, did not answer in time, answered with an error or said too much
/// that was not an answer.
pub fn describe(
    runner: &dyn ProcessRunner,
    server: &LaunchServer,
    now_ms: u64,
) -> Result<McpToolCatalog, DescribeError> {
    let session = runner
        .open_session(&spec_for(server))
        .map_err(|e| match e {
            ProcessError::NotFound => DescribeError::Start("program not found".to_owned()),
            ProcessError::PermissionDenied(reason) => {
                DescribeError::Start(reason.as_str().to_owned())
            }
            ProcessError::Spawn(detail) | ProcessError::Io(detail) => DescribeError::Start(detail),
            ProcessError::Timeout(_) => DescribeError::NoAnswer,
        })?;
    let mut talk = Conversation {
        session,
        deadline: Instant::now() + DESCRIBE_TIMEOUT,
        other_lines: 0,
    };
    talk.request(
        1,
        "initialize",
        &json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "project-atlas", "version": env!("CARGO_PKG_VERSION") },
        }),
    )?;
    talk.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))?;

    let mut tools: Vec<McpToolInfo> = Vec::new();
    let mut truncated = false;
    let mut cursor: Option<String> = None;
    for page in 0..MAX_PAGES {
        let params = cursor
            .take()
            .map_or_else(|| json!({}), |c| json!({ "cursor": c }));
        let result = talk.request(2 + page as u64, "tools/list", &params)?;
        for tool in result["tools"].as_array().into_iter().flatten() {
            if let Some(info) = tool_info(tool) {
                if !tools.iter().any(|t| t.name == info.name) {
                    tools.push(info);
                }
            }
        }
        cursor = result["nextCursor"]
            .as_str()
            .filter(|c| !c.is_empty())
            .map(str::to_owned);
        if cursor.is_none() {
            break;
        }
        if page + 1 == MAX_PAGES {
            truncated = true;
        }
    }
    if tools.len() > MAX_TOOLS {
        tools.truncate(MAX_TOOLS);
        truncated = true;
    }
    // Dropping the session ends the server.
    Ok(McpToolCatalog {
        described_at: now_ms,
        tools,
        truncated,
    })
}

struct Conversation {
    session: Box<dyn ProcessSession>,
    deadline: Instant,
    other_lines: usize,
}

impl Conversation {
    fn send(&mut self, message: &Value) -> Result<(), DescribeError> {
        self.session
            .send_line(&message.to_string())
            .map_err(|_| DescribeError::NoAnswer)
    }

    /// Sends a request and waits for its answer, answering what the server asks in between (a
    /// `ping`, or anything else with "not supported") and skipping what is not for Atlas.
    fn request(&mut self, id: u64, method: &str, params: &Value) -> Result<Value, DescribeError> {
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))?;
        loop {
            let left = self.deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(DescribeError::NoAnswer);
            }
            let Ok(Some(line)) = self.session.read_line(left) else {
                return Err(DescribeError::NoAnswer);
            };
            let message: Option<Value> = serde_json::from_str(line.trim())
                .ok()
                .filter(Value::is_object);
            let Some(message) = message else {
                self.other(1)?;
                continue;
            };
            // The server asking Atlas something: answered, so it does not wait on Atlas.
            if let (Some(asked), Some(their_id)) = (message["method"].as_str(), message.get("id")) {
                let reply = if asked == "ping" {
                    json!({ "jsonrpc": "2.0", "id": their_id, "result": {} })
                } else {
                    json!({
                        "jsonrpc": "2.0",
                        "id": their_id,
                        "error": { "code": -32601, "message": "not supported by Atlas" },
                    })
                };
                self.send(&reply)?;
                self.other(1)?;
                continue;
            }
            if message.get("id").and_then(Value::as_u64) != Some(id) {
                self.other(1)?;
                continue;
            }
            if let Some(error) = message.get("error") {
                let text = error["message"].as_str().unwrap_or("unknown error");
                return Err(DescribeError::Refused(clean(text, MAX_ERROR_CHARS)));
            }
            return Ok(message.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    fn other(&mut self, n: usize) -> Result<(), DescribeError> {
        self.other_lines += n;
        if self.other_lines > MAX_OTHER_LINES {
            Err(DescribeError::TooNoisy)
        } else {
            Ok(())
        }
    }
}

/// Text from the server, made safe to keep and show: control characters out (new lines kept),
/// at most `max` characters.
fn clean(text: &str, max: usize) -> String {
    let kept: String = text
        .chars()
        .filter(|c| *c == '\n' || !c.is_control())
        .collect();
    let kept = kept.trim();
    if kept.chars().count() > max {
        let mut cut: String = kept.chars().take(max).collect();
        cut.push('…');
        cut
    } else {
        kept.to_owned()
    }
}

fn tool_info(tool: &Value) -> Option<McpToolInfo> {
    let name = tool["name"].as_str()?.trim();
    if name.is_empty() || name.len() > MAX_NAME || name.chars().any(char::is_control) {
        return None;
    }
    let description = tool["description"].as_str().unwrap_or_default();
    let annotations = &tool["annotations"];
    let title = tool["title"]
        .as_str()
        .or_else(|| annotations["title"].as_str())
        .map(|t| clean(t, MAX_NAME))
        .filter(|t| !t.is_empty());
    // What a model is sent for the tool: its name, description and input schema.
    let definition = json!({
        "name": name,
        "description": description,
        "inputSchema": tool.get("inputSchema").cloned().unwrap_or(Value::Null),
    });
    Some(McpToolInfo {
        name: name.to_owned(),
        title,
        description: clean(description, MAX_DESCRIPTION_CHARS),
        read_only: annotations["readOnlyHint"].as_bool(),
        destructive: annotations["destructiveHint"].as_bool(),
        definition_chars: definition.to_string().chars().count() as u64,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::application::process::fake::FakeSessionRunner;
    use crate::domain::mcp::Secret;

    fn server() -> LaunchServer {
        LaunchServer {
            name: "files".to_owned(),
            executable: "npx".to_owned(),
            args: vec!["-y".to_owned(), "files-mcp@1".to_owned()],
            env: vec![
                LaunchEnv::Plain {
                    name: "MODE".to_owned(),
                    value: "read".to_owned(),
                },
                LaunchEnv::Secret {
                    name: "API_TOKEN".to_owned(),
                    value: Secret::new("s3cr3t".to_owned()),
                },
            ],
        }
    }

    fn method_and_id(line: &str) -> (String, Value) {
        let message: Value = serde_json::from_str(line).unwrap();
        (
            message["method"].as_str().unwrap_or("").to_owned(),
            message.get("id").cloned().unwrap_or(Value::Null),
        )
    }

    /// A well-behaved server that logs on stdout, pings Atlas once, and lists its tools in two pages.
    fn chatty_server() -> FakeSessionRunner {
        let pinged = Arc::new(Mutex::new(false));
        FakeSessionRunner::new(move |line| {
            let (method, id) = method_and_id(line);
            match method.as_str() {
                "initialize" => vec![
                    "server starting...".to_owned(),
                    json!({"jsonrpc":"2.0","id":id,"result":{"protocolVersion":"2025-06-18","capabilities":{"tools":{}}}}).to_string(),
                ],
                "tools/list" if line.contains("cursor") => vec![json!({"jsonrpc":"2.0","id":id,"result":{"tools":[
                    {"name":"write","description":"Writes a file.","annotations":{"readOnlyHint":false,"destructiveHint":true}},
                    {"name":"read","description":"duplicate"}
                ]}}).to_string()],
                "tools/list" => {
                    let mut out = Vec::new();
                    let mut once = pinged.lock().unwrap();
                    if !*once {
                        *once = true;
                        out.push(json!({"jsonrpc":"2.0","id":"p1","method":"ping"}).to_string());
                        out.push(json!({"jsonrpc":"2.0","method":"notifications/message","params":{}}).to_string());
                    }
                    out.push(json!({"jsonrpc":"2.0","id":id,"result":{"tools":[
                        {"name":"read","title":"Read","description":"Reads\u{7} a file.\nSafely.","inputSchema":{"type":"object","properties":{"path":{"type":"string"}}},"annotations":{"readOnlyHint":true}},
                        {"description":"no name"}
                    ],"nextCursor":"page-2"}}).to_string());
                    out
                }
                _ => Vec::new(),
            }
        })
    }

    #[test]
    fn a_server_is_asked_for_its_tools_through_the_guard_as_the_person_configured_it() {
        let runner = chatty_server();

        let catalog = describe(&runner, &server(), 7).unwrap();

        let spec = runner.opened.lock().unwrap()[0].clone();
        assert_eq!(spec.context, ProcessContext::McpServer);
        assert_eq!(spec.program, "npx");
        assert_eq!(spec.args, ["-y", "files-mcp@1"]);
        assert!(spec.stdin.is_none() && spec.cwd.is_none() && spec.terminal.is_none());
        // The secret travels in the server's environment, and nowhere it could be read back.
        assert!(spec
            .env
            .contains(&("API_TOKEN".to_owned(), "s3cr3t".to_owned())));
        assert!(spec.args.iter().all(|a| !a.contains("s3cr3t")));

        let sent: Vec<(String, Value)> = runner
            .sent
            .lock()
            .unwrap()
            .iter()
            .map(|l| method_and_id(l))
            .collect();
        let methods: Vec<&str> = sent.iter().map(|(m, _)| m.as_str()).collect();
        // The handshake first; the ping answered; then the second page.
        assert_eq!(
            methods,
            [
                "initialize",
                "notifications/initialized",
                "tools/list",
                "",
                "tools/list"
            ]
        );
        assert_eq!(sent[3].1, json!("p1"));

        assert_eq!(catalog.described_at, 7);
        assert!(!catalog.truncated);
        let names: Vec<&str> = catalog.tools.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["read", "write"]);
        let read = &catalog.tools[0];
        assert_eq!(read.title.as_deref(), Some("Read"));
        assert_eq!(read.description, "Reads a file.\nSafely.");
        assert_eq!(read.read_only, Some(true));
        assert_eq!(read.destructive, None);
        assert!(read.definition_chars > 60, "{}", read.definition_chars);
        let write = &catalog.tools[1];
        assert_eq!(
            (write.read_only, write.destructive),
            (Some(false), Some(true))
        );
    }

    #[test]
    fn a_long_description_is_cut_for_display_and_counted_whole_for_the_budget() {
        let long = "x".repeat(MAX_DESCRIPTION_CHARS * 3);
        let tool = json!({ "name": "big", "description": long });

        let info = tool_info(&tool).unwrap();

        assert_eq!(info.description.chars().count(), MAX_DESCRIPTION_CHARS + 1);
        assert!(info.description.ends_with('…'));
        assert!(info.definition_chars > (MAX_DESCRIPTION_CHARS * 3) as u64);
    }

    #[test]
    fn a_server_that_refuses_ends_or_says_nothing_is_not_described() {
        let refusing = FakeSessionRunner::new(|line| {
            let (_, id) = method_and_id(line);
            vec![json!({"jsonrpc":"2.0","id":id,"error":{"code":-32600,"message":"bad \u{1b}[31mrequest"}}).to_string()]
        });
        assert_eq!(
            describe(&refusing, &server(), 1),
            Err(DescribeError::Refused("bad [31mrequest".to_owned()))
        );

        let silent = FakeSessionRunner::new(|_| Vec::new());
        assert_eq!(
            describe(&silent, &server(), 1),
            Err(DescribeError::NoAnswer)
        );

        let noisy = FakeSessionRunner::new(|_| vec!["log line".to_owned(); MAX_OTHER_LINES + 1]);
        assert_eq!(describe(&noisy, &server(), 1), Err(DescribeError::TooNoisy));
    }

    #[test]
    fn a_server_with_endless_pages_is_cut_and_says_so() {
        let endless = FakeSessionRunner::new(|line| {
            let (method, id) = method_and_id(line);
            let result = if method == "initialize" {
                json!({})
            } else {
                json!({"tools":[{"name": format!("t{id}")}], "nextCursor": "more"})
            };
            vec![json!({"jsonrpc":"2.0","id":id,"result":result}).to_string()]
        });

        let catalog = describe(&endless, &server(), 1).unwrap();

        assert!(catalog.truncated);
        assert_eq!(catalog.tools.len(), MAX_PAGES);
    }
}
