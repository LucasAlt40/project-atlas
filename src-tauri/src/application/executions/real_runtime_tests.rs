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
        system_channel: SYSTEM_CHANNEL.with(std::cell::Cell::get),
        ..FixedFlags::default()
    }))
    .with_policies(security)
    .with_mcp(mcp)
    .with_sessions(sessions);

    service
        .run_step(
            service.next_execution_id(),
            RunAgentRequest {
                task_id: "t".to_owned(),
                workspace_id,
                agent_id: agent,
                description: prompt.to_owned(),
                scope: None,
            },
            StepOptions {
                announce_integrations: ANNOUNCE.with(std::cell::Cell::get),
                ..StepOptions::default()
            },
            &Quiet,
        )
        .unwrap()
}

thread_local! {
    /// Whether the step is told which integrations it was given (the default); a test sets it to
    /// compare.
    static ANNOUNCE: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
    /// Whether Atlas's instructions go through the CLI's system channel (off by default).
    static SYSTEM_CHANNEL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// What `init` cannot prove, measured once with a real model (M7, ADR 0026): what a server dying in
/// the middle does to a step, whether the project's `CLAUDE.md` reaches the model, and what the
/// system channel does to the CLI's own system prompt. Prints what it saw; asserts only what
/// Atlas promises (the step ends, nothing is invented). Three short runs:
/// `cargo test application::executions::real_runtime_tests::real_model_claude_what_init_does_not_prove -- --ignored --exact --nocapture`.
#[test]
#[ignore = "calls a real model (a few tokens): three short Claude runs"]
fn real_model_claude_what_init_does_not_prove() {
    let model = std::env::var("ATLAS_M7_MODEL").unwrap_or_else(|_| "haiku".to_owned());
    let tokens = |record: &ExecutionRecord| {
        record
            .execution
            .usage
            .as_ref()
            .map(|u| {
                format!(
                    "input={:?} cached={:?}",
                    u.input_tokens, u.cached_input_tokens
                )
            })
            .unwrap_or_default()
    };

    // 1. The server answers tools/list, then exits: the model's call reaches nothing.
    let nonce = format!("d{}", std::process::id());
    let died = run_with("claude", &model, PROMPT, &|mcp, workspace_id, agent| {
        grant_dying_spike(mcp, workspace_id, agent, &nonce);
    });
    let answer = report("M7 server-died", &died);
    eprintln!("REAL M7 server-died {}", tokens(&died));
    // Measured 2026-11-12: the call still returned the nonce seconds after the server exited, so
    // the CLI starts a stdio server that died again when a tool is called. Atlas only promises the
    // step ends and says what happened.
    assert_eq!(
        died.execution.status,
        crate::domain::execution::ExecutionStatus::Completed
    );
    eprintln!(
        "REAL M7 server-died the call reached a server again: {}",
        answer.contains(&format!("atlas-spike-called:{nonce}"))
    );

    // 2. A `CLAUDE.md` in the project with a word nothing else holds.
    let project = std::env::temp_dir().join(format!("atlas-real-claude-{}", std::process::id()));
    std::fs::create_dir_all(&project).unwrap();
    let word = format!("ZEBRA{}", std::process::id());
    std::fs::write(
        project.join("CLAUDE.md"),
        format!(
            "# Project notes

The project codeword is {word}.
"
        ),
    )
    .unwrap();
    let asked = run_with(
        "claude",
        &model,
        "Without reading any file or using any tool: if the instructions you were given before          this message mention a project codeword, reply with exactly that codeword. Otherwise          reply with exactly NONE.",
        &|_, _, _| {},
    );
    let answer = report("M7 claude-md", &asked);
    eprintln!(
        "REAL M7 claude-md reached the model: {} {}",
        answer.contains(&word),
        tokens(&asked)
    );

    // 3. The system channel appends to the CLI's own system prompt or replaces it.
    SYSTEM_CHANNEL.with(|on| on.set(true));
    let channel = run_with(
        "claude",
        &model,
        "Without using any tool, reply with exactly two lines and nothing else: `CLI:<the name of          the coding tool or CLI your system prompt says you are running in, or UNKNOWN>` and          `ATLAS:<YES if your system prompt contains instructions that mention Atlas, NO otherwise>`.",
        &|_, _, _| {},
    );
    SYSTEM_CHANNEL.with(|on| on.set(false));
    let answer = report("M7 system-channel", &channel);
    eprintln!("REAL M7 system-channel {}", tokens(&channel));
    assert_eq!(
        channel.execution.status,
        crate::domain::execution::ExecutionStatus::Completed
    );
    assert!(
        answer.contains("CLI:") && answer.contains("ATLAS:"),
        "{answer}"
    );
}

/// The harmless server in `MODE=die`: it lists its tools and exits, as a server that crashes.
fn grant_dying_spike(mcp: &McpService, workspace_id: &str, agent: &str, nonce: &str) {
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
                env: vec![plain("NONCE", nonce), plain("MODE", "die")],
            },
            false,
        )
        .unwrap();
    mcp.set_enabled(&connection.id, true).unwrap();
    mcp.grant(
        &connection.id,
        Some(agent),
        None,
        None,
        ToolSelection::Server,
    )
    .unwrap();
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

#[test]
#[ignore = "calls a real model (a few tokens): Copilot"]
fn real_model_copilot_calls_a_granted_tool_and_not_the_one_left_out() {
    let (record, nonce) = run_real("copilot", "auto", true);
    assert_called_and_filtered("copilot", &record, &nonce);
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

#[test]
#[ignore = "calls a real model (a few tokens): Copilot, whole server"]
fn real_model_copilot_whole_server_grant_gives_every_tool() {
    let (record, nonce) = run_real("copilot", "auto", false);
    assert_whole_server("copilot-whole", &record, &nonce);
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

#[test]
#[ignore = "calls a real model and runs npx (downloads chrome-devtools-mcp): Copilot"]
fn real_model_copilot_uses_the_devtools_entry_of_the_catalogue() {
    devtools_on("copilot", "auto");
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

/// The Jira entry of the catalogue (Atlassian's remote server through the pinned `mcp-remote`),
/// with the sign-in the person already saved. The model is asked to call one read-only tool and to
/// say only whether it worked, so no data of the account is repeated.
#[test]
#[ignore = "calls a real model and Atlassian with the saved sign-in (read only): Claude"]
fn real_model_claude_uses_the_jira_entry_of_the_catalogue() {
    use crate::application::mcp::McpCatalog;
    let prompt = "Use the Jira (Atlassian) MCP tools. Call the tool atlassianUserInfo once. Reply \
with exactly `OK` if it returned information about a user, or `FAILED` if it did not. Do not repeat \
any of the information.";
    let record = run_with("claude", "haiku", prompt, &|mcp, workspace_id, agent| {
        let (name, transport) = McpCatalog::connection_for("jira").unwrap();
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

    // Only the shape of the outcome is printed: no field of the account.
    let manifest = record.execution.manifest.as_ref().expect("a manifest");
    eprintln!("REAL jira status={:?}", record.execution.status);
    eprintln!("REAL jira answer={:?}", record.execution.result);
    eprintln!("REAL jira servers={:?}", manifest.mcp.servers);
    eprintln!(
        "REAL jira used={:?}",
        manifest
            .mcp
            .tools
            .iter()
            .filter(|t| t.used == Some(true))
            .map(|t| t.tool.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        record.execution.status,
        crate::domain::execution::ExecutionStatus::Completed
    );
    assert_eq!(
        record.execution.result.as_deref().map(str::trim),
        Some("OK")
    );
    assert!(manifest
        .mcp
        .tools
        .iter()
        .any(|t| t.tool == "atlassianUserInfo" && t.used == Some(true)));
}

/// A task that never names an integration, on a runtime and model of modest ability, with the
/// `DevTools` entry granted: does the model open the page? Printed for the announcement on and off.
fn unprompted_browse(runtime_id: &str, model: &str, announce: bool) -> (bool, String) {
    use crate::application::mcp::McpCatalog;
    ANNOUNCE.with(|a| a.set(announce));
    let prompt =
        "Check what is at https://example.com by actually loading it in a browser, and reply \
with exactly one line: `TITLE:<the title of the page>`.";
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
    let used = record
        .execution
        .manifest
        .as_ref()
        .is_some_and(|m| m.mcp.tools.iter().any(|t| t.used == Some(true)));
    let answer = record.execution.result.clone().unwrap_or_default();
    eprintln!(
        "REAL unprompted {runtime_id} announce={announce} used_a_tool={used} answer={answer:?}"
    );
    (used, answer)
}

#[test]
#[ignore = "calls real models and runs npx: compares the announcement on and off, OpenCode and Codex"]
fn real_model_unprompted_use_with_the_announcement_on_and_off() {
    for (runtime, model) in [
        ("opencode", "opencode/big-pickle"),
        ("codex", "gpt-5.6-luna"),
    ] {
        let (with, answer) = unprompted_browse(runtime, model, true);
        let _ = unprompted_browse(runtime, model, false);
        assert!(
            with,
            "{runtime}: with the announcement the model did not use the tool: {answer}"
        );
    }
}
