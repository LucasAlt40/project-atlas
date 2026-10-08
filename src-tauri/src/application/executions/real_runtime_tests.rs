//! A model really calling an MCP tool, on each runtime, through the whole of Atlas: the real guard,
//! the real CLI, the real MCP service over a harmless server, a step through `ExecutionService`.
//!
//! These cost a few tokens each (a tiny prompt) and run only when asked for, by exact name:
//! `cargo test <name> -- --ignored --exact --nocapture`. The server returns a nonce no model can
//! guess, so a reply that holds it proves the tool was called.

use std::sync::Arc;

use super::*;
use crate::application::agents::CreateAgentRequest;
use crate::application::config::memory::MemoryStore;
use crate::application::config::ConfigRepository;
use crate::application::mcp::{McpService, MemoryCredentials};
use crate::application::optimization::FixedFlags;
use crate::application::process::ProcessRunner;
use crate::application::projects::fake::FakeInspector;
use crate::application::security::{
    ApprovalBroker, AuditLog, GuardedProcessRunner, NoSandbox, SecurityService,
};
use crate::application::sessions::SessionRegistry;
use crate::application::workspace::WorkspaceInput;
use crate::domain::mcp::{McpEnv, McpEnvValue, McpTransport, ToolSelection};

#[derive(Default)]
struct Quiet;

impl ExecutionObserver for Quiet {
    fn on_event(&self, _event: &ExecutionEvent) {}
}

const PROMPT: &str = "Use the MCP tools of the server named atlasspike. Call the tool echo_static, \
and also call the tool env_zzz if it is available to you. Then reply with exactly two lines and \
nothing else: `A:<the exact text echo_static returned>` and `B:<the exact text env_zzz returned, or \
NOT AVAILABLE if you do not have that tool>`.";

/// Runs one step on `runtime_id` with `model`, the harmless server granted to the agent (whole, or
/// only `echo_static`), and returns what the step recorded and the nonce the server answers with.
fn run_real(runtime_id: &str, model: &str, only: bool) -> (ExecutionRecord, String) {
    let nonce = format!("n{}", std::process::id());
    let record = run_with(runtime_id, model, PROMPT, &|mcp, workspace_id, agent| {
        grant_spike(mcp, workspace_id, agent, &nonce, only);
    });
    (record, nonce)
}

/// The harmless server, with a second tool (`env_zzz`), granted whole or only `echo_static`.
fn grant_spike(mcp: &McpService, workspace_id: &str, agent: &str, nonce: &str, only: bool) {
    let server = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../docs/architecture/spikes/mcp-claude/echo-mcp.js");
    let plain = |name: &str, value: &str| McpEnv {
        name: name.to_owned(),
        value: McpEnvValue::Plain {
            value: value.to_owned(),
        },
    };
    let connection = mcp
        .add(
            workspace_id,
            "atlasspike",
            McpTransport::Stdio {
                executable: "node".to_owned(),
                args: vec![server.to_string_lossy().into_owned()],
                env: vec![
                    plain("NONCE", nonce),
                    plain("REPORT_ENV", "MYSECRET"),
                    plain("MYSECRET", "zzz"),
                ],
            },
            false,
        )
        .unwrap();
    mcp.set_enabled(&connection.id, true).unwrap();
    let tools = if only {
        ToolSelection::Only {
            tools: vec!["echo_static".to_owned()],
        }
    } else {
        ToolSelection::Server
    };
    mcp.grant(&connection.id, Some(agent), None, None, tools)
        .unwrap();
}

/// One step on a real runtime and model, with whatever `setup` connects and grants.
fn run_with(
    runtime_id: &str,
    model: &str,
    prompt: &str,
    setup: &dyn Fn(&McpService, &str, &str),
) -> ExecutionRecord {
    let project =
        std::env::temp_dir().join(format!("atlas-real-{runtime_id}-{}", std::process::id()));
    std::fs::create_dir_all(&project).unwrap();
    let project_path = project.to_string_lossy().into_owned();
    let config = Arc::new(ConfigRepository::load(Box::<MemoryStore>::default()));
    let security = Arc::new(SecurityService::new(config.clone()));
    let sessions = Arc::new(SessionRegistry::default());
    let runner: Arc<dyn ProcessRunner> = Arc::new(GuardedProcessRunner::new(
        Arc::new(crate::infrastructure::SystemProcessRunner::new().with_sessions(sessions.clone())),
        security.clone(),
        Arc::new(ApprovalBroker::new()),
        Arc::new(AuditLog::default()),
        Arc::new(NoSandbox),
        crate::application::runtimes::RUNTIME_PROGRAMS
            .map(str::to_owned)
            .to_vec(),
    ));
    let registry = Arc::new(RuntimeRegistry::with_default_runtimes(&runner));
    let personalities = Arc::new(PersonalityService::new(config.clone()));
    let agents = Arc::new(AgentService::new(
        config.clone(),
        personalities.clone(),
        registry.clone(),
    ));
    let workspaces = Arc::new(WorkspaceService::new(
        config.clone(),
        agents.clone(),
        Arc::new(FakeInspector::with(&[(project_path.as_str(), &[])])),
    ));
    let workspace_id = workspaces
        .create(&WorkspaceInput {
            name: "Real".to_owned(),
            project_path,
            description: None,
        })
        .unwrap()
        .id;
    let agent = agents
        .create(CreateAgentRequest {
            permission_profile_id: Some("developer".to_owned()),
            name: "dev".to_owned(),
            personality_id: "architect".to_owned(),
            runtime_id: runtime_id.to_owned(),
            model_id: model.to_owned(),
            instructions: String::new(),
            worktree_isolation: Some(false),
            result_contract: None,
        })
        .unwrap()
        .id;
    let mcp = Arc::new(McpService::new(
        config.clone(),
        Arc::new(MemoryCredentials::default()),
        registry.clone(),
    ));
    setup(&mcp, &workspace_id, &agent);
    let service = ExecutionService::new(
        agents,
        personalities,
        registry,
        workspaces,
        Arc::new(AuditLog::default()),
    )
    .with_optimization(Arc::new(FixedFlags {
        metrics: true,
        guardrails: true,
        ..FixedFlags::default()
    }))
    .with_policies(security)
    .with_mcp(mcp)
    .with_sessions(sessions);

    service
        .run_with_id(
            service.next_execution_id(),
            RunAgentRequest {
                task_id: "t".to_owned(),
                workspace_id,
                agent_id: agent,
                description: prompt.to_owned(),
            },
            &Quiet,
        )
        .unwrap()
}

fn report(label: &str, record: &ExecutionRecord) -> String {
    let answer = record.execution.result.clone().unwrap_or_default();
    let manifest = record.execution.manifest.as_ref().expect("a manifest");
    eprintln!("REAL {label} status={:?}", record.execution.status);
    eprintln!("REAL {label} answer={answer:?}");
    eprintln!("REAL {label} servers={:?}", manifest.mcp.servers);
    eprintln!("REAL {label} tools={:?}", manifest.mcp.tools);
    eprintln!("REAL {label} unauthorized={:?}", manifest.mcp.unauthorized);
    answer
}

fn assert_called_and_filtered(label: &str, record: &ExecutionRecord, nonce: &str) {
    let answer = report(label, record);
    assert_eq!(
        record.execution.status,
        crate::domain::execution::ExecutionStatus::Completed,
        "{label}: the step did not complete"
    );
    // A real call: the nonce only the server knows came back.
    assert!(
        answer.contains(&format!("atlas-spike-called:{nonce}")),
        "{label}: echo_static was not called: {answer}"
    );
    // And Atlas recorded the call, from the stream, as made.
    let manifest = record.execution.manifest.as_ref().unwrap();
    let echo = manifest
        .mcp
        .tools
        .iter()
        .find(|t| t.tool == "echo_static")
        .unwrap();
    assert_eq!(
        echo.used,
        Some(true),
        "{label}: the call was not recorded as used"
    );
    // The tool the grant left out was not available to the model.
    let b_line = answer
        .lines()
        .find(|l| l.trim_start().starts_with("B:"))
        .unwrap_or("");
    assert!(
        !b_line.contains("atlas-spike-called"),
        "{label}: the model reached a tool the grant did not name: {answer}"
    );
}

#[test]
#[ignore = "calls a real model (a few tokens): Claude"]
fn real_model_claude_calls_a_granted_tool_and_not_the_one_left_out() {
    let (record, nonce) = run_real("claude", "haiku", true);
    assert_called_and_filtered("claude", &record, &nonce);
}

#[test]
#[ignore = "calls a real model (a few tokens): Codex"]
fn real_model_codex_calls_a_granted_tool_and_not_the_one_left_out() {
    let (record, nonce) = run_real("codex", "gpt-5.6-luna", true);
    assert_called_and_filtered("codex", &record, &nonce);
}

#[test]
#[ignore = "calls a real model (a few tokens): OpenCode"]
fn real_model_opencode_calls_a_granted_tool_and_not_the_one_left_out() {
    let (record, nonce) = run_real("opencode", "opencode/big-pickle", true);
    assert_called_and_filtered("opencode", &record, &nonce);
}

fn assert_whole_server(label: &str, record: &ExecutionRecord, nonce: &str) {
    let answer = report(label, record);
    assert_eq!(
        record.execution.status,
        crate::domain::execution::ExecutionStatus::Completed,
        "{label}: the step did not complete"
    );
    let called = format!("atlas-spike-called:{nonce}");
    let line = |prefix: &str| {
        answer
            .lines()
            .find(|l| l.trim_start().starts_with(prefix))
            .unwrap_or("")
            .to_owned()
    };
    // Granted whole, both tools of the server were there for the model.
    assert!(line("A:").contains(&called), "{label}: {answer}");
    assert!(line("B:").contains(&called), "{label}: {answer}");
}

#[test]
#[ignore = "calls a real model (a few tokens): Claude, whole server"]
fn real_model_claude_whole_server_grant_gives_every_tool() {
    let (record, nonce) = run_real("claude", "haiku", false);
    assert_whole_server("claude-whole", &record, &nonce);
}

#[test]
#[ignore = "calls a real model (a few tokens): Codex, whole server"]
fn real_model_codex_whole_server_grant_gives_every_tool() {
    let (record, nonce) = run_real("codex", "gpt-5.6-luna", false);
    assert_whole_server("codex-whole", &record, &nonce);
}

#[test]
#[ignore = "calls a real model (a few tokens): OpenCode, whole server"]
fn real_model_opencode_whole_server_grant_gives_every_tool() {
    let (record, nonce) = run_real("opencode", "opencode/big-pickle", false);
    assert_whole_server("opencode-whole", &record, &nonce);
}

/// The Chrome `DevTools` entry of the catalogue, as the panel adds it (pinned, headless, isolated),
/// granted, then used by a real model to open a page. It runs `npx`, which downloads the pinned
/// package the first time.
fn devtools_on(runtime_id: &str, model: &str) {
    use crate::application::mcp::McpCatalog;
    let prompt = "Use the chrome-devtools MCP tools. First open a new page at https://example.com \
(new_page), then call list_pages. Reply with exactly one line: `PAGES:<the text list_pages returned, \
on one line>`.";
    let record = run_with(runtime_id, model, prompt, &|mcp, workspace_id, agent| {
        let (name, transport) = McpCatalog::connection_for("chrome-devtools").unwrap();
        let connection = mcp.add(workspace_id, name, transport, false).unwrap();
        mcp.set_enabled(&connection.id, true).unwrap();
        mcp.grant(
            &connection.id,
            Some(agent),
            None,
            None,
            ToolSelection::Server,
        )
        .unwrap();
    });

    let answer = report(&format!("devtools-{runtime_id}"), &record);
    assert_eq!(
        record.execution.status,
        crate::domain::execution::ExecutionStatus::Completed
    );
    assert!(answer.contains("example.com"), "{answer}");
    let manifest = record.execution.manifest.unwrap();
    assert!(
        manifest
            .mcp
            .tools
            .iter()
            .any(|t| t.tool == "new_page" && t.used == Some(true)),
        "{runtime_id}: new_page was not recorded as used"
    );
}

#[test]
#[ignore = "calls a real model and runs npx (downloads chrome-devtools-mcp): Claude"]
fn real_model_claude_uses_the_devtools_entry_of_the_catalogue() {
    devtools_on("claude", "haiku");
}

#[test]
#[ignore = "calls a real model and runs npx (downloads chrome-devtools-mcp): Codex"]
fn real_model_codex_uses_the_devtools_entry_of_the_catalogue() {
    devtools_on("codex", "gpt-5.6-luna");
}

#[test]
#[ignore = "calls a real model and runs npx (downloads chrome-devtools-mcp): OpenCode"]
fn real_model_opencode_uses_the_devtools_entry_of_the_catalogue() {
    devtools_on("opencode", "opencode/big-pickle");
}

/// Antigravity takes MCP servers only from the user's own configuration (Atlas gives it none), and
/// uses them in a headless run. A step that asks for one by name reaches it, and Atlas sees the call.
#[test]
#[ignore = "calls a real model: Antigravity, with the user's own chrome-devtools-mcp server"]
fn real_model_antigravity_reaches_the_users_own_mcp_server_when_the_task_asks() {
    let prompt = "Use the chrome-devtools MCP server (tool list_pages) and reply with exactly the \
text it returned.";
    let record = run_with("antigravity", "gemini-3.8-flash-low", prompt, &|_, _, _| {});

    let answer = report("antigravity", &record);
    eprintln!(
        "REAL antigravity metrics-tools={:?}",
        record
            .execution
            .optimization
            .as_ref()
            .map(|m| m.tools.clone())
    );
    assert_eq!(
        record.execution.status,
        crate::domain::execution::ExecutionStatus::Completed
    );
    assert!(answer.contains("Pages"), "{answer}");
}
