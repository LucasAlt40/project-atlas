//! A workflow through the whole backend with real processes: orchestrator -> chat -> execution
//! service -> the real Claude runtime -> the guard -> a real PTY. The CLI is a stand-in script
//! named `claude` (no network, no account): what is under test is Atlas's control of the run,
//! not the model. One opt-in test at the end uses the real CLI.

use std::fs;
use std::os::unix::fs::PermissionsExt;
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
}

fn stack(stand_in: Option<&str>, model: &str) -> Stack {
    stack_with(stand_in, model, false)
}

/// With `git`, the project is a real Git repository and the agents work in the run's shared
/// worktree, with the developer permission profile (they may write files there).
#[allow(clippy::too_many_lines)]
fn stack_with(stand_in: Option<&str>, model: &str, git: bool) -> Stack {
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
    ]
    .into_iter()
    .map(|(key, personality)| {
        let agent = agents
            .create(CreateAgentRequest {
                name: key.to_owned(),
                personality_id: personality.to_owned(),
                runtime_id: "claude".to_owned(),
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
    .with_sessions(sessions.clone());
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
#[test]
#[ignore = "needs the Claude CLI installed and signed in; makes real model calls"]
fn real_workflow_through_the_claude_cli() {
    let s = stack(None, "haiku");
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

/// The real Claude CLI through a real Git repository: the first agent writes a file in the run's
/// worktree, the second one reads it, Git measures the change, and the project's own checkout is
/// left alone. Opt-in like the other real tests (CLI installed and signed in; spends a little quota).
/// Run with `cargo test real_workflow_shares -- --ignored --nocapture`.
#[test]
#[ignore = "needs the Claude CLI installed and signed in; makes real model calls"]
fn real_workflow_shares_code_between_agents_through_the_claude_cli() {
    let s = stack_with(None, "haiku", true);
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
#[test]
#[ignore = "needs the Claude CLI installed and signed in; makes real model calls"]
fn real_workflow_pauses_when_the_claude_cli_asks_and_goes_on_after_the_answer() {
    use crate::domain::interaction::InteractionAnswer;

    let s = stack(None, "haiku");
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
