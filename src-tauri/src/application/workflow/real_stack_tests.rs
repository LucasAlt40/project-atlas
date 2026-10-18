//! A workflow through the whole backend with real processes: orchestrator -> chat -> execution
//! service -> the real Claude runtime -> the guard -> a real PTY. The CLI is a stand-in script
//! named `claude` (no network, no account): what is under test is Atlas's control of the run,
//! not the model. One opt-in test at the end uses the real CLI.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::chat_runner::ChatStepRunner;
use super::orchestrator::Orchestrator;
use super::runner::WorkflowObserver;
use super::service::{NewWorkflow, WorkflowService};
use super::test_support::{agent, edge, end, when, with_loop};
use crate::application::agents::{AgentService, CreateAgentRequest};
use crate::application::chat::{ChatObserver, ChatService};
use crate::application::config::memory::MemoryStore;
use crate::application::config::ConfigRepository;
use crate::application::executions::{ExecutionObserver, ExecutionService};
use crate::application::personalities::PersonalityService;
use crate::application::process::ProcessRunner;
use crate::application::projects::fake::FakeInspector;
use crate::application::runtimes::{RuntimeRegistry, RUNTIME_PROGRAMS};
use crate::application::security::testutil::TempDir;
use crate::application::security::{
    ApprovalBroker, AuditLog, GuardedProcessRunner, NoSandbox, SecurityService,
};
use crate::application::sessions::{SessionRegistry, SessionTarget};
use crate::application::usage::UsageLedger;
use crate::application::workspace::{WorkspaceInput, WorkspaceService};
use crate::domain::conversation::Message;
use crate::domain::execution::ExecutionEvent;
use crate::domain::workflow::{
    EndOutcome, NodeKind, WorkflowEvent, WorkflowExecutionStatus, WorkflowMode,
};
use crate::infrastructure::SystemProcessRunner;

/// Answers the way the real CLI does in a terminal (colour and cursor codes around JSON lines,
/// CRLF line ends), with a result block that depends on which step it was asked to do. The prompt
/// arrives as an argument, so it can see "Current step: QA (pass 1 of at most 3)".
const STAND_IN_CLAUDE: &str = r#"#!/bin/sh
status=success
case "$*" in
  *"Current step: QA (pass 1 "*) status=fail ;;
  *"Current step: QA (pass 2 "*) status=pass ;;
esac
printf '\033[?25l{"type":"system","subtype":"init"}\r\n'
printf '{"type":"result","subtype":"success","is_error":false,"duration_ms":5,"num_turns":1,"result":"Did it.\\n```atlas-result\\n{\\"status\\":\\"%s\\",\\"summary\\":\\"step done with %s\\"}\\n```","total_cost_usd":0.001}\r\n\033[?25h' "$status" "$status"
"#;

#[derive(Default)]
struct Collector(Mutex<Vec<WorkflowEvent>>);

impl ExecutionObserver for Collector {
    fn on_event(&self, _: &ExecutionEvent) {}
}

impl ChatObserver for Collector {
    fn on_message(&self, _: &Message) {}
}

impl WorkflowObserver for Collector {
    fn on_workflow_event(&self, event: &WorkflowEvent) {
        self.0.lock().unwrap().push(event.clone());
    }
}

struct Stack {
    _dir: TempDir,
    project: std::path::PathBuf,
    worktrees: Option<Arc<crate::application::worktree::WorktreeService>>,
    workspace_id: String,
    agents: Vec<(&'static str, String)>,
    workflows: Arc<WorkflowService>,
    orchestrator: Orchestrator,
    sessions: Arc<SessionRegistry>,
    chat: ChatService,
    config: Arc<ConfigRepository>,
}

fn stack(stand_in: Option<&str>, model: &str) -> Stack {
    stack_with(stand_in, model, false)
}

fn stack_with(stand_in: Option<&str>, model: &str, git: bool) -> Stack {
    stack_on("claude", stand_in, model, git)
}

/// With `git`, the project is a real Git repository and the agents work in the run's shared
/// worktree, with the developer permission profile (they may write files there).
#[allow(clippy::too_many_lines)]
fn stack_on(runtime_id: &str, stand_in: Option<&str>, model: &str, git: bool) -> Stack {
    let dir = TempDir::new("workflow-real");
    let bin = dir.path().join("bin");
    let project = dir.path().join("project");
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(&project).unwrap();
    if let Some(script) = stand_in {
        let path = bin.join("claude");
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    if git {
        let run = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .args(["-c", "user.name=Test", "-c", "user.email=t@example.com"])
                .args(["-c", "commit.gpgsign=false"])
                .args(args)
                .current_dir(&project)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        run(&["init", "--quiet", "-b", "main"]);
        fs::write(project.join("README.md"), "# Project\n").unwrap();
        run(&["add", "--all"]);
        run(&["commit", "--quiet", "-m", "initial"]);
    }
    let config = Arc::new(ConfigRepository::load(Box::<MemoryStore>::default()));
    let sessions = Arc::new(SessionRegistry::default());
    let approvals = Arc::new(ApprovalBroker::new());
    let audit = Arc::new(AuditLog::default());
    let mut system = SystemProcessRunner::new().with_sessions(sessions.clone());
    if stand_in.is_some() {
        system = system.with_first_search_dir(bin);
    }
    // Through the real guard, like the app.
    let runner: Arc<dyn ProcessRunner> = Arc::new(GuardedProcessRunner::new(
        Arc::new(system),
        Arc::new(SecurityService::new(config.clone())),
        approvals.clone(),
        audit.clone(),
        Arc::new(NoSandbox),
        RUNTIME_PROGRAMS.map(str::to_owned).to_vec(),
    ));
    let registry = Arc::new(RuntimeRegistry::with_default_runtimes(&runner));
    let personalities = Arc::new(PersonalityService::new(config.clone()));
    let agents = Arc::new(AgentService::new(
        config.clone(),
        personalities.clone(),
        registry.clone(),
    ));
    let path = project.to_string_lossy().into_owned();
    let workspaces = Arc::new(WorkspaceService::new(
        config.clone(),
        agents.clone(),
        Arc::new(FakeInspector::with(&[(path.as_str(), &[])])),
    ));
    let workspace_id = workspaces
        .create(&WorkspaceInput {
            name: "Project".to_owned(),
            project_path: path,
            description: None,
        })
        .unwrap()
        .id;
    let created: Vec<(&'static str, String)> = [
        ("developer", "developer"),
        ("qa", "qa"),
        ("fixer", "bug-fixer"),
        // The other roles of the product's templates, so any of them can be built here.
        ("architect", "architect"),
        ("validator", "architecture-validator"),
    ]
    .into_iter()
    .map(|(key, personality)| {
        let agent = agents
            .create(CreateAgentRequest {
                permission_profile_id: None,
                name: key.to_owned(),
                personality_id: personality.to_owned(),
                runtime_id: runtime_id.to_owned(),
                model_id: model.to_owned(),
                instructions: String::new(),
                // Without Git there is no worktree to isolate in.
                worktree_isolation: Some(git),
                result_contract: None,
            })
            .unwrap();
        (key, agent.id)
    })
    .collect();
    let worktrees = git.then(|| {
        for (_, id) in &created {
            agents.set_permission_profile(id, "developer").unwrap();
        }
        config
            .modify(|c| {
                for workspace in &mut c.workspaces {
                    workspace.security = crate::domain::security::SecurityPolicy::developer();
                }
                Ok(())
            })
            .unwrap();
        let layout =
            crate::application::worktree::WorktreeLayout::new(dir.path().join("worktrees"));
        Arc::new(crate::application::worktree::WorktreeService::new(
            Arc::new(crate::infrastructure::GitWorktreeManager::new(
                "git".into(),
                layout.clone(),
            )),
            layout,
            config.clone(),
            Arc::new(SecurityService::new(config.clone())),
        ))
    });
    let mut executions = ExecutionService::new(
        agents.clone(),
        personalities,
        registry,
        workspaces.clone(),
        audit,
    )
    .with_sessions(sessions.clone())
    .with_optimization(config.clone())
    // Skills are looked for in the project's own `.atlas/skills` (only when the setting is on).
    .with_skills(Arc::new(
        crate::application::optimization::skills::SkillService::new(
            Arc::new(crate::infrastructure::FsSkillStore),
            None,
        ),
    ));
    if let Some(worktrees) = &worktrees {
        executions = executions
            .with_worktrees(worktrees.clone())
            .with_policies(Arc::new(SecurityService::new(config.clone())));
    }
    let executions = Arc::new(executions);
    let ledger = Arc::new(UsageLedger::new(config.clone()));
    let chat = ChatService::new(
        agents.clone(),
        executions.clone(),
        workspaces.clone(),
        ledger,
    );
    let settings_config = config.clone();
    let workflows = Arc::new(WorkflowService::new(config, agents, workspaces.clone()));
    let mut runner = ChatStepRunner::new(chat.clone(), sessions.clone(), approvals);
    if let Some(worktrees) = &worktrees {
        runner = runner.with_shared_worktrees(worktrees.clone(), workspaces, executions);
    }
    let orchestrator =
        Orchestrator::new(workflows.clone(), Arc::new(runner)).with_poll(Duration::from_millis(20));
    Stack {
        _dir: dir,
        project,
        worktrees,
        workspace_id,
        agents: created,
        workflows,
        orchestrator,
        sessions,
        chat,
        config: settings_config,
    }
}

impl Stack {
    fn agent(&self, key: &str) -> String {
        self.agents
            .iter()
            .find(|(k, _)| *k == key)
            .unwrap()
            .1
            .clone()
    }

    fn workflow(
        &self,
        nodes: Vec<crate::domain::workflow::WorkflowNode>,
        edges: Vec<crate::domain::workflow::WorkflowEdge>,
    ) -> crate::domain::workflow::Workflow {
        let mut nodes = nodes;
        for node in &mut nodes {
            if let NodeKind::Agent(a) = &mut node.kind {
                a.agent_id = self.agent(&a.agent_id);
            }
        }
        self.workflows
            .create(NewWorkflow {
                workspace_id: self.workspace_id.clone(),
                name: "Real workflow".to_owned(),
                description: String::new(),
                mode: WorkflowMode::Custom,
                nodes,
                edges,
                viewport: None,
            })
            .unwrap()
    }
}

#[test]
fn a_workflow_runs_through_the_real_runtime_guard_and_terminal_with_a_loop() {
    let s = stack(Some(STAND_IN_CLAUDE), "sonnet");
    let workflow = s.workflow(
        vec![
            agent("developer", "developer"),
            with_loop(agent("QA", "qa"), "qa_bug_fix", 3),
            agent("fixer", "fixer"),
            end("done", EndOutcome::Done),
        ],
        vec![
            edge("developer", "QA"),
            when("QA", "done", "pass"),
            when("QA", "fixer", "fail"),
            edge("fixer", "QA"),
        ],
    );
    let run = s
        .workflows
        .start(&workflow.id, "Implement password recovery")
        .unwrap();
    let observer = Arc::new(Collector::default());

    s.orchestrator.run(&run.id, observer.clone()).unwrap();

    let done = s.workflows.execution(&run.id).unwrap();
    assert_eq!(
        done.status,
        WorkflowExecutionStatus::Completed,
        "{:?}",
        done.failure
    );
    // The stand-in saw the workflow context in its prompt, so QA failed once and passed once.
    assert_eq!(done.nodes["QA"].attempts.len(), 2);
    assert_eq!(done.nodes["QA"].facts["result.status"], "pass");
    assert_eq!(done.state.iteration_count["qa_bug_fix"], 2);
    // Every step is an execution like any other: stored, linked to its step, and run in a terminal.
    let stored = s.chat.executions(Some(&s.workspace_id), None);
    assert_eq!(stored.len(), 4);
    for execution in &stored {
        let link = execution.workflow.as_ref().expect("a workflow step");
        assert_eq!(link.workflow_execution_id, run.id);
        let target = SessionTarget {
            execution_id: execution.id.clone(),
            workspace_id: execution.workspace_id.clone(),
            agent_id: execution.agent_id.clone(),
        };
        assert!(
            s.sessions.snapshot(&target).is_ok(),
            "{} had a terminal",
            execution.id
        );
    }
    // The agents' answers (with the result block) are in their conversations.
    let messages = s.chat.messages(Some(&s.workspace_id), None);
    assert_eq!(messages.len(), 8);
    assert_eq!(s.sessions.live_executions().len(), 0);

    // Every step was measured, and the measure of each one explains its whole prompt. The steps
    // after the first were handed something by the one before; the first was handed nothing.
    let mut ordered = stored.clone();
    ordered.sort_by_key(|e| e.started_at);
    let rows: Vec<_> = ordered
        .iter()
        .map(|e| {
            let metrics = e.optimization.as_ref().expect("measured");
            assert_eq!(
                metrics
                    .prompt
                    .sections
                    .iter()
                    .map(|x| x.bytes)
                    .sum::<usize>(),
                metrics.prompt.total_bytes
            );
            assert!(metrics.handoff.bytes.is_some());
            assert!(metrics.latency.runtime_ms.is_some());
            let link = e.workflow.as_ref().unwrap();
            (link.node_label.clone(), metrics.handoff.bytes.unwrap_or(0))
        })
        .collect();
    assert_eq!(rows[0].1, 0, "{rows:?}");
    assert!(rows[1..].iter().all(|(_, bytes)| *bytes > 0), "{rows:?}");
    // The runtime reported a cost but no tokens: the tokens stay unavailable, never estimated.
    assert!(ordered
        .iter()
        .all(|e| e.usage.as_ref().is_none_or(|u| u.input_tokens.is_none())));
}

#[test]
fn cancelling_a_workflow_ends_the_real_process_of_the_running_step() {
    const SLOW: &str = r#"#!/bin/sh
echo '{"type":"system","subtype":"init"}'
sh -c 'exec sleep 30'
echo '{"type":"result","subtype":"success","is_error":false,"result":"never"}'
"#;
    let s = stack(Some(SLOW), "sonnet");
    let workflow = s.workflow(
        vec![
            agent("developer", "developer"),
            end("done", EndOutcome::Done),
        ],
        vec![edge("developer", "done")],
    );
    let run = s.workflows.start(&workflow.id, "task").unwrap();
    let observer = Arc::new(Collector::default());
    let orchestrator = Arc::new(s.orchestrator);
    let driver = {
        let (orchestrator, observer, id) = (orchestrator.clone(), observer.clone(), run.id.clone());
        std::thread::spawn(move || orchestrator.run(&id, observer).unwrap())
    };
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while s.sessions.live_executions().is_empty() {
        assert!(
            std::time::Instant::now() < deadline,
            "the step never started its process"
        );
        std::thread::sleep(Duration::from_millis(20));
    }

    orchestrator
        .control(
            &run.id,
            super::orchestrator::Control::Cancel,
            observer.as_ref(),
        )
        .unwrap();
    driver.join().unwrap();

    let done = s.workflows.execution(&run.id).unwrap();
    assert_eq!(done.status, WorkflowExecutionStatus::Cancelled);
    assert_eq!(
        done.nodes["developer"].status,
        crate::domain::workflow::NodeStatus::Cancelled
    );
    assert!(
        s.sessions.live_executions().is_empty(),
        "the process is gone"
    );
    let stored = s.chat.executions(Some(&s.workspace_id), None);
    assert_eq!(
        stored[0].status,
        crate::domain::execution::ExecutionStatus::Cancelled
    );
}

/// The real Claude CLI, a real model call, the real guard and PTY, through a two-step workflow.
/// Opt-in: it needs the CLI installed and signed in, and spends a little quota. Without a
/// session it verifies the honest failure instead (the step fails, nothing is called a success).
/// Run with `cargo test real_workflow -- --ignored --nocapture`.
fn real_workflow_through_the_claude_cli_on(runtime_id: &str, model: &str) {
    let s = stack_on(runtime_id, None, model, false);
    // Two steps, the second routing on what the first reports: the result block the model
    // writes is the only thing that decides the route.
    let workflow = s.workflow(
        vec![
            agent("developer", "developer"),
            agent("QA", "qa"),
            end("done", EndOutcome::Done),
            end("not-done", EndOutcome::Failed),
        ],
        vec![
            edge("developer", "QA"),
            when("QA", "done", "pass"),
            when("QA", "not-done", "fail"),
        ],
    );
    let run = s
        .workflows
        .start(
            &workflow.id,
            "The previous step was asked to reply with the word pong. Review that: if the \
             task is understood, report status pass. This is a protocol check, nothing needs \
             to be built or tested; do not use any tools.",
        )
        .unwrap();
    let observer = Arc::new(Collector::default());

    s.orchestrator.run(&run.id, observer).unwrap();

    let done = s.workflows.execution(&run.id).unwrap();
    println!("status: {:?}, failure: {:?}", done.status, done.failure);
    for (id, node) in &done.nodes {
        println!("{id}: {:?} {:?}", node.status, node.facts);
    }
    for artifact in &done.state.artifacts {
        println!("artifact {}: {}", artifact.name, artifact.summary);
    }
    for message in s.chat.messages(Some(&s.workspace_id), None) {
        println!("[{:?}] {}", message.role, message.content);
    }
    // Whatever the model decided, the run ended, and ended the way its result block said: a
    // run is never called a success that its route did not lead to.
    assert!(done.status.is_final());
    let verdict = &done.nodes["QA"].facts;
    match verdict.get("result.status").map(String::as_str) {
        Some("pass") => assert_eq!(done.status, WorkflowExecutionStatus::Completed),
        // Anything else (a fail, or no usable result) is a failed run, never a success.
        _ => assert_eq!(done.status, WorkflowExecutionStatus::Failed),
    }
}

#[test]
#[ignore = "needs the Claude CLI installed and signed in; makes real model calls"]
fn real_workflow_through_the_claude_cli() {
    real_workflow_through_the_claude_cli_on("claude", "haiku");
}

#[test]
#[ignore = "needs the Copilot CLI installed and signed in; makes real model calls (premium requests)"]
fn real_workflow_through_the_copilot_cli() {
    real_workflow_through_the_claude_cli_on("copilot", "auto");
}

/// The real Claude CLI through a real Git repository: the first agent writes a file in the run's
/// worktree, the second one reads it, Git measures the change, and the project's own checkout is
/// left alone. Opt-in like the other real tests (CLI installed and signed in; spends a little quota).
/// Run with `cargo test real_workflow_shares -- --ignored --nocapture`.
fn real_workflow_shares_code_between_agents_through_the_claude_cli_on(
    runtime_id: &str,
    model: &str,
) {
    let s = stack_on(runtime_id, None, model, true);
    let workflow = s.workflow(
        vec![
            agent("developer", "developer"),
            agent("QA", "qa"),
            end("done", EndOutcome::Done),
            end("not-done", EndOutcome::Failed),
        ],
        vec![
            edge("developer", "QA"),
            when("QA", "done", "pass"),
            when("QA", "not-done", "fail"),
        ],
    );
    let run = s
        .workflows
        .start(
            &workflow.id,
            "Create a file named hello.txt in the current directory whose whole content is the \
             text: hello atlas. Then, if you are asked to check it, open hello.txt and report \
             pass only if its content is exactly: hello atlas.",
        )
        .unwrap();
    let observer = Arc::new(Collector::default());

    s.orchestrator.run(&run.id, observer).unwrap();

    let done = s.workflows.execution(&run.id).unwrap();
    println!("status: {:?}, failure: {:?}", done.status, done.failure);
    for (id, node) in &done.nodes {
        println!("{id}: {:?} {:?}", node.status, node.facts);
    }
    println!("integration: {:?}", done.integration);
    println!("changes: {:?}", done.changes);
    for handoff in &done.handoffs {
        println!(
            "handoff {} -> {}: {:?} files={:?}",
            handoff.from_node_id,
            handoff.to_node_id,
            handoff.summary,
            handoff
                .changed_files
                .iter()
                .map(|f| f.path.clone())
                .collect::<Vec<_>>()
        );
    }
    let worktrees = s.worktrees.as_ref().unwrap();
    // The main checkout is untouched whatever the model did.
    assert!(!s.project.join("hello.txt").exists());
    assert!(done.status.is_final());
    if let Some(changes) = &done.changes {
        println!(
            "the worktree holds: {:?}",
            changes.files.iter().map(|f| &f.path).collect::<Vec<_>>()
        );
        let primaries = worktrees
            .list(None, None)
            .iter()
            .filter(|w| w.shared_with.is_none())
            .count();
        assert_eq!(primaries, 1);
    }
}

#[test]
#[ignore = "needs the Claude CLI installed and signed in; makes real model calls"]
fn real_workflow_shares_code_between_agents_through_the_claude_cli() {
    real_workflow_shares_code_between_agents_through_the_claude_cli_on("claude", "haiku");
}

#[test]
#[ignore = "needs the Copilot CLI installed and signed in; makes real model calls (premium requests)"]
fn real_workflow_shares_code_between_agents_through_the_copilot_cli() {
    real_workflow_shares_code_between_agents_through_the_claude_cli_on("copilot", "auto");
}

// ---- an agent that asks a person -------------------------------------------------------------------

/// Asks on its first run and finishes once the prompt carries the person's answer: what Atlas
/// does with a runtime that stops to ask, through the real adapter, guard and terminal.
const ASKING_CLAUDE: &str = r#"#!/bin/sh
case "$*" in
  *"HUMAN INPUT"*) body='Used it.\n```atlas-result\n{\"status\":\"success\",\"summary\":\"done with the answer\"}\n```' ;;
  *) body='I need one thing.\n```atlas-interaction\n{\"type\":\"clarification\",\"question\":\"Which API should I use?\"}\n```' ;;
esac
printf '{"type":"system","subtype":"init"}\r\n'
printf '{"type":"result","subtype":"success","is_error":false,"duration_ms":5,"num_turns":1,"result":"%s","total_cost_usd":0.001}\r\n' "$body"
"#;

fn wait_for(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    while !condition() {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for {what}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_runtime_that_stops_to_ask_pauses_the_run_and_goes_on_from_the_answer() {
    use crate::domain::execution::ExecutionStatus;
    use crate::domain::interaction::InteractionAnswer;

    let s = stack(Some(ASKING_CLAUDE), "sonnet");
    let workflow = s.workflow(
        vec![
            agent("developer", "developer"),
            end("done", EndOutcome::Done),
        ],
        vec![edge("developer", "done")],
    );
    let run = s
        .workflows
        .start(&workflow.id, "Build the users API")
        .unwrap();
    let observer = Arc::new(Collector::default());
    let orchestrator = Arc::new(s.orchestrator);
    let driver = {
        let (orchestrator, observer, id) = (orchestrator.clone(), observer.clone(), run.id.clone());
        std::thread::spawn(move || orchestrator.run(&id, observer).unwrap())
    };

    // The process ended after asking; the run does not read that as an ending.
    wait_for("the run to wait for the person", || {
        s.workflows.execution(&run.id).unwrap().status == WorkflowExecutionStatus::WaitingForInput
    });
    let waiting = s.workflows.execution(&run.id).unwrap();
    assert_eq!(
        waiting.nodes["developer"].status,
        crate::domain::workflow::NodeStatus::WaitingForInput
    );
    assert_eq!(
        waiting.nodes["done"].status,
        crate::domain::workflow::NodeStatus::Pending
    );
    let question = waiting.pending_interaction("developer").unwrap().clone();
    assert_eq!(question.question, "Which API should I use?");
    let stored = s.chat.executions(Some(&s.workspace_id), None);
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].status, ExecutionStatus::WaitingForInput);
    assert_eq!(
        stored[0].interaction.as_ref().map(|i| i.question.as_str()),
        Some("Which API should I use?")
    );
    assert_eq!(waiting.handoffs.len(), 0);

    orchestrator
        .answer_interaction(
            &run.id,
            &question.id,
            InteractionAnswer {
                choice: None,
                text: Some("/api/users".to_owned()),
            },
            observer.as_ref(),
        )
        .unwrap();
    driver.join().unwrap();

    let done = s.workflows.execution(&run.id).unwrap();
    assert_eq!(
        done.status,
        WorkflowExecutionStatus::Completed,
        "{:?}",
        done.failure
    );
    // The second process was started with the answer in its prompt: only then did it finish.
    assert_eq!(done.nodes["developer"].attempts.len(), 2);
    assert_eq!(
        done.nodes["developer"]
            .last_attempt()
            .unwrap()
            .summary
            .as_deref(),
        Some("done with the answer")
    );
    let stored = s.chat.executions(Some(&s.workspace_id), None);
    assert_eq!(stored.len(), 2);
    assert!(stored
        .iter()
        .any(|e| e.status == ExecutionStatus::Completed));
    assert!(stored
        .iter()
        .any(|e| e.status == ExecutionStatus::WaitingForInput));
    assert_eq!(s.sessions.live_executions().len(), 0);
}

/// The real Claude CLI asked to stop and ask: Atlas detects the question, pauses, takes the
/// answer, and the model carries on with it. Opt-in like the other real tests.
/// Run with `cargo test real_workflow_pauses -- --ignored --nocapture`.
fn real_workflow_pauses_when_the_claude_cli_asks_and_goes_on_after_the_answer_on(
    runtime_id: &str,
    model: &str,
) {
    use crate::domain::interaction::InteractionAnswer;

    let s = stack_on(runtime_id, None, model, false);
    let workflow = s.workflow(
        vec![
            agent("developer", "developer"),
            end("done", EndOutcome::Done),
        ],
        vec![edge("developer", "done")],
    );
    let run = s
        .workflows
        .start(
            &workflow.id,
            "Pick the database for a new service. You do not know which one the team uses and \
             you must not guess: ask the person which database to use, with the asking protocol, \
             and stop. Do not use any tools. After you are given the answer, reply with a short \
             sentence naming the database and finish with the result block.",
        )
        .unwrap();
    let observer = Arc::new(Collector::default());
    let orchestrator = Arc::new(s.orchestrator);
    let driver = {
        let (orchestrator, observer, id) = (orchestrator.clone(), observer.clone(), run.id.clone());
        std::thread::spawn(move || orchestrator.run(&id, observer).unwrap())
    };

    wait_for("the model to ask, or the run to end", || {
        let state = s.workflows.execution(&run.id).unwrap();
        state.status == WorkflowExecutionStatus::WaitingForInput || state.status.is_final()
    });
    let state = s.workflows.execution(&run.id).unwrap();
    println!("after the first step: {:?}", state.status);
    for message in s.chat.messages(Some(&s.workspace_id), None) {
        println!("[{:?}] {}", message.role, message.content);
    }
    let Some(question) = state.pending_interaction("developer").cloned() else {
        driver.join().unwrap();
        panic!("the model did not stop to ask: {:?}", state.status);
    };
    println!(
        "detected {:?} via {:?}: {}",
        question.kind, question.source, question.question
    );
    assert_eq!(
        state.nodes["done"].status,
        crate::domain::workflow::NodeStatus::Pending
    );

    orchestrator
        .answer_interaction(
            &run.id,
            &question.id,
            InteractionAnswer {
                choice: None,
                text: Some("PostgreSQL".to_owned()),
            },
            observer.as_ref(),
        )
        .unwrap();
    driver.join().unwrap();

    let done = s.workflows.execution(&run.id).unwrap();
    println!("final: {:?} {:?}", done.status, done.failure);
    for message in s.chat.messages(Some(&s.workspace_id), None) {
        println!("[{:?}] {}", message.role, message.content);
    }
    assert!(done.status.is_final());
    assert_eq!(done.nodes["developer"].attempts.len(), 2);
}

#[test]
#[ignore = "needs the Claude CLI installed and signed in; makes real model calls"]
fn real_workflow_pauses_when_the_claude_cli_asks_and_goes_on_after_the_answer() {
    real_workflow_pauses_when_the_claude_cli_asks_and_goes_on_after_the_answer_on(
        "claude", "haiku",
    );
}

#[test]
#[ignore = "needs the Copilot CLI installed and signed in; makes real model calls (premium requests)"]
fn real_workflow_pauses_when_the_copilot_cli_asks_and_goes_on_after_the_answer() {
    real_workflow_pauses_when_the_claude_cli_asks_and_goes_on_after_the_answer_on(
        "copilot", "auto",
    );
}

// ---- a failed run goes on in the worktree it had -------------------------------------------------

/// Writes into the directory it is run in (the run's worktree): the developer a file, the fixer
/// another one; QA fails until it sees the fixer's file.
const WRITING_CLAUDE: &str = r#"#!/bin/sh
status=success
case "$*" in
  *"Current step: developer"*) echo implemented > feature.txt ;;
  *"Current step: fixer"*) echo repaired > repaired.txt ;;
  *"Current step: QA"*) if [ -f repaired.txt ]; then status=pass; else status=fail; fi ;;
esac
printf '{"type":"system","subtype":"init"}\r\n'
printf '{"type":"result","subtype":"success","is_error":false,"duration_ms":5,"num_turns":1,"result":"Did it.\\n```atlas-result\\n{\\"status\\":\\"%s\\",\\"summary\\":\\"step done with %s\\"}\\n```","total_cost_usd":0.001}\r\n' "$status" "$status"
"#;

#[test]
fn a_failed_run_resumes_in_the_same_worktree_keeping_its_files_and_never_touching_the_project() {
    let s = stack_with(Some(WRITING_CLAUDE), "sonnet", true);
    let workflow = s.workflow(
        vec![
            agent("developer", "developer"),
            agent("QA", "qa"),
            end("done", EndOutcome::Done),
        ],
        vec![edge("developer", "QA"), when("QA", "done", "pass")],
    );
    let run = s
        .workflows
        .start(&workflow.id, "Build the feature")
        .unwrap();
    s.orchestrator
        .run(&run.id, Arc::new(Collector::default()))
        .unwrap();

    // QA failed and nothing routes its `fail`: the run stops, its code waits in its worktree.
    let failed = s.workflows.execution(&run.id).unwrap();
    assert_eq!(failed.status, WorkflowExecutionStatus::Failed);
    let primary = failed.integration.worktree_execution_id.clone().unwrap();
    let worktrees = s.worktrees.as_ref().unwrap();
    let before = worktrees.get(&primary).unwrap();
    assert!(std::path::Path::new(&before.worktree_path)
        .join("feature.txt")
        .exists());
    assert!(!s.project.join("feature.txt").exists());

    // The route is added; the run goes on from the fixer, in the same worktree.
    let mut edited = s.workflows.get(&workflow.id).unwrap();
    let mut fixer = agent("fixer", "fixer");
    if let NodeKind::Agent(a) = &mut fixer.kind {
        a.agent_id = s.agent("fixer");
    }
    edited.nodes.push(fixer);
    let qa = edited.nodes.iter().position(|n| n.id == "QA").unwrap();
    edited.nodes[qa] = with_loop(edited.nodes[qa].clone(), "qa_bug_fix", 3);
    edited.edges.push(when("QA", "fixer", "fail"));
    edited.edges.push(edge("fixer", "QA"));
    s.workflows.update(edited).unwrap();
    s.orchestrator
        .resume_failed(&run.id, Arc::new(Collector::default()))
        .unwrap();

    let done = s.workflows.execution(&run.id).unwrap();
    assert_eq!(
        done.status,
        WorkflowExecutionStatus::Completed,
        "{:?}",
        done.failure
    );
    assert_eq!(done.nodes["developer"].attempts.len(), 1, "not run again");
    assert_eq!(done.nodes["QA"].attempts.len(), 2);
    assert_eq!(done.nodes["fixer"].attempts.len(), 1);
    // The same worktree and branch, with what the developer wrote and what the fixer added.
    assert_eq!(
        done.integration.worktree_execution_id.as_deref(),
        Some(primary.as_str())
    );
    assert_eq!(
        done.integration.branch.as_deref(),
        Some(before.branch_name.as_str())
    );
    let path = std::path::Path::new(&before.worktree_path);
    assert!(path.join("feature.txt").exists());
    assert!(path.join("repaired.txt").exists());
    let changes = done.changes.expect("the code of the run");
    let files: Vec<_> = changes.files.iter().map(|f| f.path.as_str()).collect();
    assert!(
        files.contains(&"feature.txt") && files.contains(&"repaired.txt"),
        "{files:?}"
    );
    // Resuming is not applying: the project is as it was and the user still decides.
    assert!(!s.project.join("feature.txt").exists());
    assert!(!s.project.join("repaired.txt").exists());
    assert_ne!(
        done.integration.status,
        crate::domain::workflow::IntegrationStatus::Integrated
    );
}

#[test]
fn a_failed_run_whose_worktree_is_gone_asks_for_recovery_instead_of_recreating_it() {
    let s = stack_with(Some(WRITING_CLAUDE), "sonnet", true);
    let workflow = s.workflow(
        vec![
            agent("developer", "developer"),
            agent("QA", "qa"),
            end("done", EndOutcome::Done),
        ],
        vec![edge("developer", "QA"), when("QA", "done", "pass")],
    );
    let run = s
        .workflows
        .start(&workflow.id, "Build the feature")
        .unwrap();
    s.orchestrator
        .run(&run.id, Arc::new(Collector::default()))
        .unwrap();
    let failed = s.workflows.execution(&run.id).unwrap();
    let primary = failed.integration.worktree_execution_id.clone().unwrap();
    let worktree = s.worktrees.as_ref().unwrap().get(&primary).unwrap();
    fs::remove_dir_all(&worktree.worktree_path).unwrap();

    let mut edited = s.workflows.get(&workflow.id).unwrap();
    let mut fixer = agent("fixer", "fixer");
    if let NodeKind::Agent(a) = &mut fixer.kind {
        a.agent_id = s.agent("fixer");
    }
    edited.nodes.push(fixer);
    let qa = edited.nodes.iter().position(|n| n.id == "QA").unwrap();
    edited.nodes[qa] = with_loop(edited.nodes[qa].clone(), "qa_bug_fix", 3);
    edited.edges.push(when("QA", "fixer", "fail"));
    edited.edges.push(edge("fixer", "QA"));
    s.workflows.update(edited).unwrap();

    let plan = s.orchestrator.recovery_plan(&run.id).unwrap();
    assert_eq!(
        plan.problem,
        Some(crate::domain::workflow::RecoveryProblem::RecoveryRequired)
    );
    let error = s
        .orchestrator
        .resume_failed(&run.id, Arc::new(Collector::default()))
        .unwrap_err();
    assert!(error.is(crate::application::errors::ErrorCode::WorkflowNotRecoverable));
    assert_eq!(s.workflows.execution(&run.id).unwrap(), failed);
}

/// A stand-in `claude` that finishes whatever it is asked, with a result block.
const FINISHING_CLAUDE: &str = r#"#!/bin/sh
printf '{"type":"system","subtype":"init","tools":["Glob","Grep","Read"],"mcp_servers":[],"skills":[],"slash_commands":[]}\r\n'
printf '{"type":"result","subtype":"success","is_error":false,"duration_ms":5,"num_turns":1,"result":"Done.\\n```atlas-result\\n{\\"status\\":\\"success\\",\\"summary\\":\\"stored it\\"}\\n```","total_cost_usd":0.001}\r\n'
"#;

/// A project whose skill says PostgreSQL and a step whose own instructions say MongoDB: two
/// sources of the user's own setup disagreeing about the database, which no agent should be left
/// to pick between. The workflow's one step names the skill so it is loaded.
fn conflicted_run(s: &Stack) -> crate::domain::workflow::WorkflowExecution {
    let skill = s.project.join(".atlas/skills/storage-rules");
    fs::create_dir_all(&skill).unwrap();
    fs::write(
        skill.join("SKILL.md"),
        "---\nname: storage-rules\ndescription: Use when choosing the database and storage for a feature\n---\nUse PostgreSQL for all persistence.\n",
    )
    .unwrap();
    s.config
        .modify(|c| {
            c.settings.optimization.skills_enabled = true;
            Ok(())
        })
        .unwrap();
    let mut developer = agent("developer", "developer");
    if let NodeKind::Agent(a) = &mut developer.kind {
        "Use MongoDB for storage, following /storage-rules".clone_into(&mut a.instructions);
    }
    let workflow = s.workflow(
        vec![developer, end("done", EndOutcome::Done)],
        vec![edge("developer", "done")],
    );
    s.workflows
        .start(&workflow.id, "Build the storage layer")
        .unwrap()
}

#[test]
fn a_context_that_needs_a_look_pauses_the_run_for_a_person_and_goes_on_only_with_their_yes() {
    use crate::domain::execution::ExecutionStatus;
    use crate::domain::interaction::{DetectionSource, InteractionAnswer};

    let s = stack(Some(FINISHING_CLAUDE), "sonnet");
    let run = conflicted_run(&s);
    let observer = Arc::new(Collector::default());
    let orchestrator = Arc::new(s.orchestrator);
    let driver = {
        let (orchestrator, observer, id) = (orchestrator.clone(), observer.clone(), run.id.clone());
        std::thread::spawn(move || orchestrator.run(&id, observer).unwrap())
    };

    // Before the agent started, Atlas asked: the run waits and no process was ever launched.
    wait_for("the run to wait for the person", || {
        s.workflows.execution(&run.id).unwrap().status == WorkflowExecutionStatus::WaitingForInput
    });
    let waiting = s.workflows.execution(&run.id).unwrap();
    let question = waiting.pending_interaction("developer").unwrap().clone();
    assert_eq!(question.source, DetectionSource::Guardrail);
    assert!(
        question.context.contains("conflicting_instructions"),
        "{}",
        question.context
    );
    assert_eq!(
        waiting.nodes["done"].status,
        crate::domain::workflow::NodeStatus::Pending
    );
    let stored = s.chat.executions(Some(&s.workspace_id), None);
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].status, ExecutionStatus::WaitingForInput);
    assert_eq!(s.sessions.live_executions().len(), 0, "nothing was started");
    assert_eq!(
        stored[0]
            .optimization
            .as_ref()
            .unwrap()
            .guardrails
            .unwrap()
            .asked,
        1
    );

    // The person says yes through the real mechanism.
    orchestrator
        .answer_interaction(
            &run.id,
            &question.id,
            InteractionAnswer {
                choice: Some("allow".to_owned()),
                text: None,
            },
            observer.as_ref(),
        )
        .unwrap();
    driver.join().unwrap();

    let done = s.workflows.execution(&run.id).unwrap();
    assert_eq!(
        done.status,
        WorkflowExecutionStatus::Completed,
        "{:?}",
        done.failure
    );
    let stored = s.chat.executions(Some(&s.workspace_id), None);
    assert_eq!(stored.len(), 2);
    // The second execution ran, and says a person approved it; the agent was never told a thing.
    let second = stored
        .iter()
        .find(|e| e.status == ExecutionStatus::Completed)
        .unwrap();
    let review = second
        .optimization
        .as_ref()
        .unwrap()
        .context_review
        .clone()
        .unwrap();
    assert_eq!(
        review.health,
        crate::domain::guardrail::ContextHealth::NeedsReview
    );
    // It is not asked about again: the person's yes was taken, and counted as allowed.
    let guard = second.optimization.as_ref().unwrap().guardrails.unwrap();
    assert_eq!((guard.asked, guard.allowed, guard.denied), (0, 1, 0));
}

#[test]
fn a_persons_no_stops_the_step_without_ever_starting_the_agent() {
    use crate::domain::execution::{ExecutionStatus, FailureKind};
    use crate::domain::interaction::InteractionAnswer;

    let s = stack(Some(FINISHING_CLAUDE), "sonnet");
    let run = conflicted_run(&s);
    let observer = Arc::new(Collector::default());
    let orchestrator = Arc::new(s.orchestrator);
    let driver = {
        let (orchestrator, observer, id) = (orchestrator.clone(), observer.clone(), run.id.clone());
        std::thread::spawn(move || orchestrator.run(&id, observer).unwrap())
    };
    wait_for("the run to wait for the person", || {
        s.workflows.execution(&run.id).unwrap().status == WorkflowExecutionStatus::WaitingForInput
    });
    let question = s
        .workflows
        .execution(&run.id)
        .unwrap()
        .pending_interaction("developer")
        .unwrap()
        .clone();

    orchestrator
        .answer_interaction(
            &run.id,
            &question.id,
            InteractionAnswer {
                choice: Some("deny".to_owned()),
                text: None,
            },
            observer.as_ref(),
        )
        .unwrap();
    driver.join().unwrap();

    // The workflow stays safe: the step failed with the reason and nothing after it ran.
    let done = s.workflows.execution(&run.id).unwrap();
    assert_eq!(done.status, WorkflowExecutionStatus::Failed);
    assert_ne!(
        done.nodes["done"].status,
        crate::domain::workflow::NodeStatus::Completed
    );
    let stored = s.chat.executions(Some(&s.workspace_id), None);
    let ended = stored
        .iter()
        .find(|e| e.status == ExecutionStatus::Failed)
        .unwrap();
    assert_eq!(
        ended.failure.as_ref().unwrap().kind,
        FailureKind::PermissionDenied
    );
    assert!(ended.failure.as_ref().unwrap().message.contains("declined"));
    assert_eq!(s.sessions.live_executions().len(), 0);
}

/// The whole path with the real Claude CLI, without spending a token: the model does not exist, so
/// the CLI starts, reports what it loaded and fails at its first request. It proves, with the real
/// adapter, guard, terminal and CLI, that Atlas asks first, starts nothing before the answer, and
/// after a yes really starts the runtime (which then fails for its model, not for Atlas).
/// Run with `cargo test real_claude_guardrail -- --ignored --nocapture`.
#[test]
#[ignore = "needs the Claude CLI installed; makes no model call (the model does not exist)"]
fn real_claude_guardrail_asks_before_starting_and_starts_the_real_cli_after_a_yes() {
    use crate::domain::execution::{ExecutionStatus, FailureKind};
    use crate::domain::interaction::InteractionAnswer;

    let s = stack(None, "not-a-real-model-xyz");
    let run = conflicted_run(&s);
    let observer = Arc::new(Collector::default());
    let orchestrator = Arc::new(s.orchestrator);
    let driver = {
        let (orchestrator, observer, id) = (orchestrator.clone(), observer.clone(), run.id.clone());
        std::thread::spawn(move || orchestrator.run(&id, observer).unwrap())
    };
    wait_for("the run to wait for the person", || {
        s.workflows.execution(&run.id).unwrap().status == WorkflowExecutionStatus::WaitingForInput
    });
    // Nothing was started before the question was answered.
    assert_eq!(s.sessions.live_executions().len(), 0);
    let stored = s.chat.executions(Some(&s.workspace_id), None);
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].status, ExecutionStatus::WaitingForInput);
    let question = s
        .workflows
        .execution(&run.id)
        .unwrap()
        .pending_interaction("developer")
        .unwrap()
        .clone();

    orchestrator
        .answer_interaction(
            &run.id,
            &question.id,
            InteractionAnswer {
                choice: Some("allow".to_owned()),
                text: None,
            },
            observer.as_ref(),
        )
        .unwrap();
    driver.join().unwrap();

    // After the yes the real CLI ran: it failed because the model does not exist.
    let stored = s.chat.executions(Some(&s.workspace_id), None);
    let started = stored
        .iter()
        .find(|e| e.status == ExecutionStatus::Failed)
        .expect("the runtime was started after the answer");
    println!("REAL FAILURE {:?}", started.failure);
    assert_eq!(
        started.failure.as_ref().unwrap().kind,
        FailureKind::ModelUnavailable
    );
    let guard = started.optimization.as_ref().unwrap();
    println!(
        "REAL REVIEW {:?}",
        guard.context_review.as_ref().map(|r| r.health)
    );
    assert_eq!(
        guard.context_review.as_ref().unwrap().health,
        crate::domain::guardrail::ContextHealth::NeedsReview
    );
}

// ---- the product's templates, through a real model ------------------------------------------------

/// What a template run left behind, read back from Git and the run itself.
struct TemplateRun {
    status: WorkflowExecutionStatus,
    /// Every step that started, in order (a step that ran twice is listed twice).
    started: Vec<String>,
    /// `git show <run's branch>:<path>`, for the files asked about.
    files: std::collections::BTreeMap<String, Option<String>>,
    /// Paths Git says the run changed.
    changed: Vec<String>,
    failure: Option<String>,
    /// The run's branch, to take its whole tree out and run it.
    branch: Option<String>,
}

fn git_out(project: &std::path::Path, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(project)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// A real Git project with `files` committed, the product template `template_id` built from the
/// agents of the workspace (one per role, all on `runtime_id`/`model`), run on `task` with plans
/// approved. Reads back the files named in `read` from the run's branch.
#[allow(clippy::too_many_lines)] // setup, run and the read-back, printed for the record
fn run_template(
    runtime_id: &str,
    model: &str,
    template_id: &str,
    files: &[(&str, &str)],
    task: &str,
    read: &[&str],
) -> (Stack, TemplateRun) {
    let s = stack_on(runtime_id, None, model, true);
    for (path, content) in files {
        let target = s.project.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, content).unwrap();
    }
    if !files.is_empty() {
        for args in [
            vec!["add", "--all"],
            vec![
                "-c",
                "user.name=Test",
                "-c",
                "user.email=t@example.com",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "-m",
                "fixture",
            ],
        ] {
            assert!(git_out(&s.project, &args).is_some(), "git {args:?}");
        }
    }
    // As the agent form does: each agent starts from its personality's suggested contract.
    // Read first: the config's lock is not reentrant, so nothing inside `modify` may read it.
    let personalities = PersonalityService::new(s.config.clone());
    let suggested: std::collections::BTreeMap<String, _> = s
        .config
        .agents()
        .into_iter()
        .map(|a| {
            let contract = personalities
                .find(&a.personality_id)
                .unwrap()
                .suggested_contract;
            (a.id, contract)
        })
        .collect();
    s.config
        .modify(|c| {
            for agent in &mut c.agents {
                agent.result_contract = suggested[&agent.id].clone();
            }
            Ok(())
        })
        .unwrap();
    let built = s
        .workflows
        .create_from_template(&s.workspace_id, template_id, None, WorkflowMode::Custom)
        .unwrap();
    assert!(
        built.missing_roles.is_empty(),
        "every role has an agent: {:?}",
        built.missing_roles
    );
    let run = s
        .workflows
        .start_with_plans(&built.workflow.id, task, true)
        .unwrap();
    let observer = Arc::new(Collector::default());
    let began = std::time::Instant::now();
    s.orchestrator.run(&run.id, observer.clone()).unwrap();
    let done = s.workflows.execution(&run.id).unwrap();

    let started: Vec<String> = observer
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|e| e.kind == crate::domain::workflow::WorkflowEventKind::NodeStarted)
        .filter_map(|e| e.node_id.clone())
        .collect();
    println!(
        "--- {template_id} on {runtime_id}/{model} in {:?}",
        began.elapsed()
    );
    println!("status: {:?}, failure: {:?}", done.status, done.failure);
    println!("steps started: {started:?}");
    for (id, node) in &done.nodes {
        println!(
            "{id}: {:?} attempts={} facts={:?}",
            node.status,
            node.attempts.len(),
            node.facts
        );
    }
    println!("integration: {:?}", done.integration);
    let branch = s
        .worktrees
        .as_ref()
        .unwrap()
        .list(None, None)
        .into_iter()
        .find(|w| w.shared_with.is_none())
        .map(|w| w.branch_name);
    println!("branch: {branch:?}");
    let changed = branch
        .as_deref()
        .and_then(|b| git_out(&s.project, &["diff", "--name-only", "main", b]))
        .map(|text| text.lines().map(str::to_owned).collect())
        .unwrap_or_default();
    println!("changed: {changed:?}");
    let files = read
        .iter()
        .map(|path| {
            let shown = branch
                .as_deref()
                .and_then(|b| git_out(&s.project, &["show", &format!("{b}:{path}")]));
            println!("{path} on the branch: {shown:?}");
            ((*path).to_owned(), shown)
        })
        .collect();
    // The project's own checkout is never touched by a run: changes wait for the person.
    let status = git_out(&s.project, &["status", "--porcelain"]).unwrap_or_default();
    assert!(status.trim().is_empty(), "the checkout changed: {status}");
    let failure = done.failure.as_ref().map(|f| format!("{f:?}"));
    let result = TemplateRun {
        status: done.status,
        started,
        files,
        changed,
        failure,
        branch,
    };
    (s, result)
}

/// Runs `node` on `script` (a `CommonJS` check) inside a folder holding `files`; its stdout.
fn node_check(files: &[(&str, &str)], script: &str) -> String {
    let dir = TempDir::new("template-check");
    for (path, content) in files {
        let target = dir.path().join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, content).unwrap();
    }
    let out = std::process::Command::new("node")
        .args(["-e", script])
        .current_dir(dir.path())
        .output()
        .expect("node is installed");
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout).trim(),
        String::from_utf8_lossy(&out.stderr).trim()
    )
}

/// Index of the first start of `step` (panics with the order when it never started).
fn first(run: &TemplateRun, step: &str) -> usize {
    run.started
        .iter()
        .position(|s| s == step)
        .unwrap_or_else(|| panic!("{step} never started: {:?}", run.started))
}

const SUM_CHECK: &str = "const {sum}=require('./src/sum.js');\
     console.log(sum(2,3)===5&&sum(-1,1)===0?'OK':'WRONG '+sum(2,3))";

/// "Quick change" (Architect -> Developer -> QA): a small function with explicit acceptance
/// criteria, checked by running it.
fn real_template_quick_change_on(runtime_id: &str, model: &str) {
    let (_s, run) = run_template(
        runtime_id,
        model,
        "feature_basic",
        &[(
            "README.md",
            "# Calc\nA tiny CommonJS project. No dependencies.\n",
        )],
        "Create the file src/sum.js, a CommonJS module that exports a function sum(a, b) \
         returning a + b, exactly as: module.exports = { sum }. Acceptance: sum(2, 3) === 5 and \
         sum(-1, 1) === 0. Touch no other file. Keep it to a few lines.",
        &["src/sum.js"],
    );
    assert_eq!(
        run.status,
        WorkflowExecutionStatus::Completed,
        "{:?}",
        run.failure
    );
    assert!(first(&run, "architect") < first(&run, "developer"));
    assert!(first(&run, "developer") < first(&run, "qa"));
    let sum = run.files["src/sum.js"]
        .clone()
        .expect("src/sum.js on the branch");
    assert_eq!(node_check(&[("src/sum.js", &sum)], SUM_CHECK), "OK");
}

/// "Feature with fix loop": the defect is already in the project and the developer is told to
/// leave it, so QA has to fail it, the Bug Fixer has to repair it, and QA has to pass it again.
fn real_template_fix_loop_on(runtime_id: &str, model: &str) {
    let (_s, run) = run_template(
        runtime_id,
        model,
        "feature_with_bug_fix_loop",
        &[
            (
                "README.md",
                "# Calc\nA tiny CommonJS project. No dependencies.\n",
            ),
            (
                "src/sum.js",
                "function sum(a, b) {\n  return a - b;\n}\nmodule.exports = { sum };\n",
            ),
        ],
        "Two parts. Part 1, for the Architect and the Developer: add one line '- sum' to a new \
         file FEATURES.md and change nothing else; in particular the Developer must NOT open or \
         edit src/sum.js. Part 2, for QA: read src/sum.js; the acceptance criterion is that \
         sum(2, 3) === 5. If src/sum.js does not return a + b, the outcome is fail with a \
         finding on src/sum.js. For the Bug Fixer: make src/sum.js return a + b, nothing else.",
        &["src/sum.js", "FEATURES.md"],
    );
    let qa_runs = run.started.iter().filter(|s| *s == "qa").count();
    println!("qa ran {qa_runs} time(s)");
    assert_eq!(
        run.status,
        WorkflowExecutionStatus::Completed,
        "{:?}",
        run.failure
    );
    let fixer = first(&run, "bug-fixer");
    assert!(
        first(&run, "qa") < fixer,
        "QA failed first, then the fixer ran"
    );
    assert!(
        run.started[fixer..].iter().any(|s| s == "qa"),
        "QA ran again after the fix: {:?}",
        run.started
    );
    let sum = run.files["src/sum.js"]
        .clone()
        .expect("src/sum.js on the branch");
    assert_eq!(node_check(&[("src/sum.js", &sum)], SUM_CHECK), "OK");
}

/// "Review & audit": an intentional defect must be reported and no file may change.
fn real_template_review_on(runtime_id: &str, model: &str) {
    let (s, run) = run_template(
        runtime_id,
        model,
        "code_review",
        &[(
            "src/average.js",
            "// Average of a list of numbers.\nfunction average(values) {\n  let total = 0;\n  \
             for (const v of values) total += v;\n  return total / values.length;\n}\n\
             module.exports = { average };\n",
        )],
        "Review src/average.js for correctness. Report each defect as a finding with the file \
         and the line. Change nothing.",
        &[],
    );
    assert!(run.status.is_final());
    assert!(
        run.changed.is_empty(),
        "a review changed files: {:?}",
        run.changed
    );
    // What the review said, as the person reads it.
    let said: String = s
        .chat
        .messages(Some(&s.workspace_id), None)
        .iter()
        .map(|m| m.content.to_lowercase())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        [
            "empty",
            "length === 0",
            "length is 0",
            "zero",
            "nan",
            "division"
        ]
        .iter()
        .any(|w| said.contains(w)),
        "the empty-list defect was reported:\n{said}"
    );
}

/// "Debug & fix": a reported symptom, fixed minimally and checked by running it.
fn real_template_debug_fix_on(runtime_id: &str, model: &str) {
    let (_s, run) = run_template(
        runtime_id,
        model,
        "bug_fix",
        &[(
            "src/parse.js",
            "function parseList(text) {\n  return text.split(',').map(Number);\n}\n\
             module.exports = { parseList };\n",
        )],
        "Bug: parseList('1,2,') in src/parse.js returns [1, 2, 0] but must return [1, 2]: empty \
         items (and items that are only spaces) are ignored. parseList('3, 4') must still return \
         [3, 4]. Fix it in src/parse.js.",
        &["src/parse.js"],
    );
    assert_eq!(
        run.status,
        WorkflowExecutionStatus::Completed,
        "{:?}",
        run.failure
    );
    let code = run.files["src/parse.js"]
        .clone()
        .expect("src/parse.js on the branch");
    assert_eq!(
        node_check(
            &[("src/parse.js", &code)],
            "const {parseList}=require('./src/parse.js');\
             const a=JSON.stringify(parseList('1,2,')),b=JSON.stringify(parseList('3, 4'));\
             console.log(a==='[1,2]'&&b==='[3,4]'?'OK':'WRONG '+a+' '+b)"
        ),
        "OK"
    );
}

/// "Implement" (the recommended template): plan, change, review, QA.
fn real_template_implement_on(runtime_id: &str, model: &str) {
    let (_s, run) = run_template(
        runtime_id,
        model,
        "software_feature",
        &[(
            "README.md",
            "# Calc\nA tiny CommonJS project. No dependencies.\n",
        )],
        "Create the file src/sum.js, a CommonJS module that exports a function sum(a, b) \
         returning a + b, exactly as: module.exports = { sum }. Acceptance: sum(2, 3) === 5 and \
         sum(-1, 1) === 0. Touch no other file.",
        &["src/sum.js"],
    );
    assert_eq!(
        run.status,
        WorkflowExecutionStatus::Completed,
        "{:?}",
        run.failure
    );
    assert_eq!(run.started.first().map(String::as_str), Some("architect"));
    let sum = run.files["src/sum.js"]
        .clone()
        .expect("src/sum.js on the branch");
    assert_eq!(node_check(&[("src/sum.js", &sum)], SUM_CHECK), "OK");
}

// OpenCode's free models: no cost.
#[test]
#[ignore = "needs the OpenCode CLI; makes real (free) model calls"]
fn real_template_quick_change_opencode() {
    real_template_quick_change_on("opencode", "opencode/big-pickle");
}

#[test]
#[ignore = "needs the OpenCode CLI; makes real (free) model calls"]
fn real_template_fix_loop_opencode() {
    real_template_fix_loop_on("opencode", "opencode/big-pickle");
}

#[test]
#[ignore = "needs the OpenCode CLI; makes real (free) model calls"]
fn real_template_review_opencode() {
    real_template_review_on("opencode", "opencode/big-pickle");
}

#[test]
#[ignore = "needs the OpenCode CLI; makes real (free) model calls"]
fn real_template_debug_fix_opencode() {
    real_template_debug_fix_on("opencode", "opencode/big-pickle");
}

#[test]
#[ignore = "needs the OpenCode CLI; makes real (free) model calls"]
fn real_template_implement_opencode() {
    real_template_implement_on("opencode", "opencode/big-pickle");
}

#[test]
#[ignore = "needs the Copilot CLI signed in; one premium request per step"]
fn real_template_fix_loop_copilot() {
    real_template_fix_loop_on("copilot", "auto");
}

#[test]
#[ignore = "needs the Codex CLI signed in; makes real model calls"]
fn real_template_quick_change_codex() {
    real_template_quick_change_on("codex", "gpt-5.6-luna");
}

#[test]
#[ignore = "needs the Claude CLI signed in; makes real model calls"]
fn real_template_fix_loop_claude() {
    real_template_fix_loop_on("claude", "haiku");
}

/// The run's branch as a folder of files, to run what the agents wrote.
fn tree_of(s: &Stack, run: &TemplateRun) -> TempDir {
    let branch = run.branch.as_deref().expect("the run has a branch");
    let dir = TempDir::new("template-tree");
    let archive = std::process::Command::new("git")
        .args(["archive", "--format=tar", branch])
        .current_dir(&s.project)
        .output()
        .unwrap();
    assert!(archive.status.success());
    let mut untar = std::process::Command::new("tar")
        .args(["-x", "-C"])
        .arg(dir.path())
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    std::io::Write::write_all(untar.stdin.as_mut().unwrap(), &archive.stdout).unwrap();
    assert!(untar.wait().unwrap().success());
    dir
}

/// `node <args>` in `dir`: (succeeded, stdout + stderr).
fn node_in(dir: &std::path::Path, args: &[&str]) -> (bool, String) {
    let out = std::process::Command::new("node")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("node is installed");
    (
        out.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

const README_JSDOC: &str = "# Calc\nA tiny CommonJS project. No dependencies.\n\n## Conventions\n\n\
Every exported function MUST have a JSDoc comment (/** ... */) directly above it. Code without it is \
not accepted.\n";

/// "Feature with validation", accepted: the change follows the convention, the validator and QA
/// pass, and the code works.
fn real_template_feature_with_validation_accepted_on(runtime_id: &str, model: &str) {
    let (s, run) = run_template(
        runtime_id,
        model,
        "feature_with_validation",
        &[("README.md", README_JSDOC)],
        "Create src/sum.js, a CommonJS module that exports sum(a, b) returning a + b, exactly as \
         module.exports = { sum }, with a JSDoc comment directly above the function as the README \
         requires. Acceptance: sum(2, 3) === 5. Touch no other file.",
        &["src/sum.js"],
    );
    assert_eq!(
        run.status,
        WorkflowExecutionStatus::Completed,
        "{:?}",
        run.failure
    );
    assert!(first(&run, "developer") < first(&run, "validator"));
    assert!(first(&run, "validator") < first(&run, "qa"));
    let tree = tree_of(&s, &run);
    let (ok, out) = node_in(tree.path(), &["-e", SUM_CHECK]);
    assert!(ok && out.trim() == "OK", "{out}");
    assert!(run.files["src/sum.js"]
        .as_deref()
        .unwrap_or("")
        .contains("/**"));
}

/// "Feature with validation", rejected: the task makes the developer break the project's
/// convention, so the validator must fail it, and the run must fail without QA.
fn real_template_feature_with_validation_rejected_on(runtime_id: &str, model: &str) {
    let (_s, run) = run_template(
        runtime_id,
        model,
        "feature_with_validation",
        &[("README.md", README_JSDOC)],
        "Create src/sum.js, a CommonJS module that exports sum(a, b) returning a + b, exactly as \
         module.exports = { sum }. The Developer must write NO comment of any kind in the file. The \
         Validator must check the change against the README's conventions and fail it if any \
         convention is broken.",
        &["src/sum.js"],
    );
    let sum = run.files["src/sum.js"].clone().unwrap_or_default();
    println!("the developer wrote a comment: {}", sum.contains("/**"));
    assert_eq!(
        run.status,
        WorkflowExecutionStatus::Failed,
        "a broken convention was accepted"
    );
    assert!(run.started.contains(&"validator".to_owned()));
    assert!(
        !run.started.contains(&"qa".to_owned()),
        "QA ran after a rejection"
    );
}

/// "Full-stack development": a backend and a frontend that must talk to each other, checked by
/// starting the backend and calling it through the frontend's client.
fn real_template_full_stack_on(runtime_id: &str, model: &str) {
    let (s, run) = run_template(
        runtime_id,
        model,
        "full_development",
        &[(
            "README.md",
            "# Sum service\nTwo parts, CommonJS, Node's standard library only, no dependencies:\n\
             - backend/server.js\n- frontend/api.js\n",
        )],
        "Build a tiny full-stack sum feature, Node standard library only, CommonJS, no dependencies. \
         Backend: backend/server.js exports createServer(), returning a node:http Server (not \
         listening) that answers GET /api/sum?a=<n>&b=<n> with status 200 and the JSON body \
         {\"result\": a + b} (numbers), and 400 when a or b is not a number. Frontend: \
         frontend/api.js exports async function getSum(baseUrl, a, b) that calls the backend with \
         the global fetch and returns the number in result. module.exports = { createServer } and \
         module.exports = { getSum }. Acceptance: getSum against a running createServer() returns 5 \
         for 2 and 3.",
        &["backend/server.js", "frontend/api.js"],
    );
    assert_eq!(
        run.status,
        WorkflowExecutionStatus::Completed,
        "{:?}",
        run.failure
    );
    assert!(first(&run, "backend") < first(&run, "validator"));
    assert!(first(&run, "frontend") < first(&run, "validator"));
    let tree = tree_of(&s, &run);
    let check = "const {createServer}=require('./backend/server.js');\
        const {getSum}=require('./frontend/api.js');\
        const server=createServer();\
        server.listen(0,'127.0.0.1',async()=>{\
          const base='http://127.0.0.1:'+server.address().port;\
          try{\
            const five=await getSum(base,2,3);\
            const bad=await fetch(base+'/api/sum?a=x&b=1');\
            console.log(five===5&&bad.status===400?'OK':'WRONG '+five+' '+bad.status);\
          }catch(e){console.log('ERROR '+e.message);}\
          server.close();\
        });";
    let (ok, out) = node_in(tree.path(), &["-e", check]);
    assert!(ok && out.trim() == "OK", "{out}");
}

const PRICE_BEFORE: &str = "// Price of an order line, with the discounts the shop gives.\n\
function price(kind, qty, unit) {\n\
  if (kind === 'book') {\n    let total = qty * unit;\n    if (qty >= 10) { total = total - total * 0.1; }\n    return Math.round(total * 100) / 100;\n  }\n\
  if (kind === 'food') {\n    let total = qty * unit;\n    if (qty >= 10) { total = total - total * 0.05; }\n    return Math.round(total * 100) / 100;\n  }\n\
  if (kind === 'toy') {\n    let total = qty * unit;\n    if (qty >= 10) { total = total - total * 0.2; }\n    return Math.round(total * 100) / 100;\n  }\n\
  let total = qty * unit;\n  return Math.round(total * 100) / 100;\n}\nmodule.exports = { price };\n";

const PRICE_TESTS: &str =
    "const test = require('node:test');\nconst assert = require('node:assert');\n\
const { price } = require('../src/price.js');\n\
test('book discount from 10', () => { assert.strictEqual(price('book', 10, 2), 18); });\n\
test('food no discount under 10', () => { assert.strictEqual(price('food', 9, 1.5), 13.5); });\n\
test('toy discount', () => { assert.strictEqual(price('toy', 12, 3.33), 31.97); });\n\
test('other kinds', () => { assert.strictEqual(price('pen', 10, 1), 10); });\n";

/// "Refactor": duplication removed, behaviour kept. Checked by the project's own tests and by
/// comparing the new code with the old one on many inputs.
fn real_template_refactor_on(runtime_id: &str, model: &str) {
    let (s, run) = run_template(
        runtime_id,
        model,
        "refactoring",
        &[
            ("src/price.js", PRICE_BEFORE),
            ("test/price.test.js", PRICE_TESTS),
            ("README.md", "# Shop\nRun the tests with: node --test\n"),
        ],
        "Refactor src/price.js to remove the duplicated discount logic (for example a table of \
         discount rates by kind), keeping its behaviour exactly the same for every input, \
         including rounding. Keep module.exports = { price }. Do not change test/price.test.js.",
        &["src/price.js", "test/price.test.js"],
    );
    assert_eq!(
        run.status,
        WorkflowExecutionStatus::Completed,
        "{:?}",
        run.failure
    );
    let after = run.files["src/price.js"]
        .clone()
        .expect("src/price.js on the branch");
    assert_ne!(after, PRICE_BEFORE, "nothing was refactored");
    assert_eq!(
        run.files["test/price.test.js"].as_deref(),
        Some(PRICE_TESTS),
        "the regression tests were changed"
    );
    let tree = tree_of(&s, &run);
    let (ok, out) = node_in(tree.path(), &["--test"]);
    assert!(ok, "the project's tests fail after the refactor:\n{out}");
    fs::write(tree.path().join("old.js"), PRICE_BEFORE).unwrap();
    let compare = "const a=require('./old.js').price,b=require('./src/price.js').price;\
        let bad=[];for(const k of ['book','food','toy','pen',''])\
        for(const q of [0,1,9,10,11,25])for(const u of [0,0.1,1.5,3.33,19.99])\
        if(a(k,q,u)!==b(k,q,u))bad.push([k,q,u,a(k,q,u),b(k,q,u)]);\
        console.log(bad.length?'DIFF '+JSON.stringify(bad.slice(0,5)):'SAME');";
    let (ok, out) = node_in(tree.path(), &["-e", compare]);
    assert!(ok && out.trim() == "SAME", "behaviour changed: {out}");
}

#[test]
#[ignore = "needs the OpenCode CLI; makes real (free) model calls"]
fn real_template_feature_with_validation_accepted_opencode() {
    real_template_feature_with_validation_accepted_on("opencode", "opencode/big-pickle");
}

#[test]
#[ignore = "needs the OpenCode CLI; makes real (free) model calls"]
fn real_template_feature_with_validation_rejected_opencode() {
    real_template_feature_with_validation_rejected_on("opencode", "opencode/big-pickle");
}

#[test]
#[ignore = "needs the OpenCode CLI; makes real (free) model calls"]
fn real_template_full_stack_opencode() {
    real_template_full_stack_on("opencode", "opencode/big-pickle");
}

#[test]
#[ignore = "needs the OpenCode CLI; makes real (free) model calls"]
fn real_template_refactor_opencode() {
    real_template_refactor_on("opencode", "opencode/big-pickle");
}

// ---- isolation: an agent that leaves its worktree ---------------------------------------------------

/// A stand-in agent that does its work in the worktree and also, at the developer step, finds the
/// project's own checkout through Git (the worktree's `.git` names it) and writes a file there.
/// It keeps every prompt it was given, to see which folders it was told about.
const ESCAPING_CLAUDE: &str = r#"#!/bin/sh
printf '%s\n----\n' "$*" >> "$ATLAS_TEST_PROMPTS"
printf '%s\n' "$PWD" >> "$ATLAS_TEST_PROMPTS.cwd"
status=success
case "$*" in
  *"Current step: developer"*)
    echo work > feature.txt
    main=$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")
    echo escaped > "$main/ESCAPED.txt" ;;
  *"Current step: QA"*) status=pass ;;
esac
printf '{"type":"system","subtype":"init"}\r\n'
printf '{"type":"result","subtype":"success","is_error":false,"duration_ms":5,"num_turns":1,"result":"Did it.\\n```atlas-result\\n{\\"status\\":\\"%s\\",\\"summary\\":\\"step done with %s\\"}\\n```","total_cost_usd":0.001}\r\n' "$status" "$status"
"#;

/// What Atlas tells an agent about where it works, and what happens when one writes into the
/// project's own checkout anyway (the OS sandbox is off, as it is by default).
#[test]
fn an_agent_that_writes_into_the_project_checkout_is_named_by_the_run() {
    let prompts = TempDir::new("escape-prompts");
    let log = prompts.path().join("prompts.txt");
    std::env::set_var("ATLAS_TEST_PROMPTS", &log);
    let s = stack_with(Some(ESCAPING_CLAUDE), "sonnet", true);
    let workflow = s.workflow(
        vec![
            agent("developer", "developer"),
            agent("QA", "qa"),
            end("done", EndOutcome::Done),
        ],
        vec![edge("developer", "QA"), when("QA", "done", "pass")],
    );
    let run = s
        .workflows
        .start(&workflow.id, "Build the feature")
        .unwrap();
    s.orchestrator
        .run(&run.id, Arc::new(Collector::default()))
        .unwrap();
    let done = s.workflows.execution(&run.id).unwrap();

    let told = fs::read_to_string(&log).unwrap_or_default();
    let dirs = fs::read_to_string(log.with_extension("txt.cwd")).unwrap_or_default();
    let project = s.project.canonicalize().unwrap();
    println!(
        "status {:?}, integration {:?}",
        done.status, done.integration.status
    );
    println!("working directories: {dirs}");
    // Where the agents were told they work: the worktree, never the checkout.
    assert!(
        !told.contains(&project.to_string_lossy().to_string()),
        "a prompt names the project's checkout"
    );
    assert!(dirs.lines().all(|d| !Path::new(d).starts_with(&project)));
    // The agent did get out (nothing at the OS level stops it with the sandbox off) ...
    assert!(s.project.join("ESCAPED.txt").exists());
    // ... and the run says so instead of offering its changes as if nothing had happened.
    assert_eq!(
        done.integration.checkout_changed,
        vec!["ESCAPED.txt".to_owned()]
    );
}
