//! The Live Shell through the whole backend, with real processes: services -> the real Claude
//! runtime -> the guard -> the real runner in a real PTY. The CLI is a stand-in script named
//! `claude` (so no network, no account), which is enough: what is under test is Atlas's control
//! of the process, not the model.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use super::testutil::TempDir;
use super::{ApprovalBroker, AuditLog, GuardedProcessRunner, NoSandbox, SecurityService};
use crate::application::agents::{AgentService, CreateAgentRequest};
use crate::application::chat::{ChatObserver, ChatService, SendMessageRequest};
use crate::application::config::memory::MemoryStore;
use crate::application::config::ConfigRepository;
use crate::application::executions::{ExecutionObserver, ExecutionService, RunAgentRequest};
use crate::application::personalities::PersonalityService;
use crate::application::process::{
    ExecutionScope, ProcessContext, ProcessError, ProcessRunner, ProcessSpec, TerminalRequest,
};
use crate::application::projects::fake::FakeInspector;
use crate::application::runtimes::{RuntimeRegistry, RUNTIME_PROGRAMS};
use crate::application::sessions::{ControlError, SessionRegistry, SessionSink, SessionTarget};
use crate::application::usage::UsageLedger;
use crate::application::workspace::{WorkspaceInput, WorkspaceService};
use crate::domain::conversation::{Message, MessageRole};
use crate::domain::execution::{
    ExecutionEvent, ExecutionEventKind, ExecutionRecord, ExecutionStatus,
};
use crate::domain::task::TaskStatus;
use crate::domain::terminal::{SessionStatus, SessionStatusEvent, TerminalChunk, UserAction};
use crate::infrastructure::SystemProcessRunner;

/// The stand-in for `claude`: announces itself, then works for a long time in a *child* process
/// (so the tree is two deep), then would answer.
const SLOW_CLAUDE: &str = r#"#!/bin/sh
echo '{"type":"system","subtype":"init"}'
sh -c 'echo "grandchild $$"; exec sleep 30'
echo '{"type":"result","subtype":"success","is_error":false,"result":"never"}'
"#;

/// Like the slow one, but ignores Ctrl+C (and so does everything it starts).
const STUBBORN_CLAUDE: &str = r#"#!/bin/sh
trap '' INT
echo '{"type":"system","subtype":"init"}'
sh -c 'echo "grandchild $$"; exec sleep 30'
echo '{"type":"result","subtype":"success","is_error":false,"result":"never"}'
"#;

/// Answers at once, the way the real CLI does in a terminal: colour and cursor codes around
/// JSON lines, CRLF line ends.
const QUICK_CLAUDE: &str = r#"#!/bin/sh
printf '\033[?25l{"type":"system","subtype":"init"}\r\n'
printf '{"type":"result","subtype":"success","is_error":false,"duration_ms":5,"num_turns":1,"result":"All good.","total_cost_usd":0.001}\r\n\033[?25h'
"#;

#[derive(Default)]
struct Collector(Mutex<Vec<ExecutionEvent>>);

impl ExecutionObserver for Collector {
    fn on_event(&self, event: &ExecutionEvent) {
        self.0.lock().unwrap().push(event.clone());
    }
}

/// What the app's webview would hear from the sessions: the activity they announce.
#[derive(Default)]
struct SessionActivity(Mutex<Vec<ExecutionEvent>>);

impl SessionSink for SessionActivity {
    fn on_output(&self, _: &TerminalChunk) {}
    fn on_status(&self, _: &SessionStatusEvent) {}
    fn on_activity(&self, event: &ExecutionEvent) {
        self.0.lock().unwrap().push(event.clone());
    }
}

struct Stack {
    dir: TempDir,
    sessions: Arc<SessionRegistry>,
    activity: Arc<SessionActivity>,
    executions: Arc<ExecutionService>,
    chat: ChatService,
    workspace_id: String,
    other_workspace_id: String,
    agents: Vec<String>,
    runner: Arc<dyn ProcessRunner>,
}

fn stack(claude_script: &str) -> Stack {
    build_stack(Some(claude_script), "claude", "sonnet")
}

/// `claude_script`: a stand-in `claude` to put first on the search path; `None` uses whatever is
/// really installed. Agents run `runtime_id` with `model_id`.
fn build_stack(claude_script: Option<&str>, runtime_id: &str, model_id: &str) -> Stack {
    let dir = TempDir::new("live-shell");
    let bin = dir.path().join("bin");
    let projects = [dir.path().join("p1"), dir.path().join("p2")];
    for folder in [&bin, &projects[0], &projects[1]] {
        fs::create_dir_all(folder).unwrap();
    }
    if let Some(claude_script) = claude_script {
        let script = bin.join("claude");
        fs::write(&script, claude_script).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let config = Arc::new(ConfigRepository::load(Box::<MemoryStore>::default()));
    let activity = Arc::new(SessionActivity::default());
    let sessions = Arc::new(SessionRegistry::with_sink(activity.clone()));
    let audit = Arc::new(AuditLog::default());
    let runner: Arc<dyn ProcessRunner> = Arc::new(GuardedProcessRunner::new(
        Arc::new(
            SystemProcessRunner::new()
                .with_first_search_dir(bin)
                .with_sessions(sessions.clone()),
        ),
        Arc::new(SecurityService::new(config.clone())),
        Arc::new(ApprovalBroker::new()),
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
    let paths: Vec<String> = projects
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let ledger = Arc::new(UsageLedger::new(config.clone()));
    let workspaces = Arc::new(WorkspaceService::new(
        config,
        agents.clone(),
        Arc::new(FakeInspector::with(&[
            (paths[0].as_str(), &[]),
            (paths[1].as_str(), &[]),
        ])),
    ));
    let workspace = |name: &str, path: &str| {
        workspaces
            .create(&WorkspaceInput {
                name: name.to_owned(),
                project_path: path.to_owned(),
                description: None,
            })
            .unwrap()
            .id
    };
    let workspace_id = workspace("One", &paths[0]);
    let other_workspace_id = workspace("Two", &paths[1]);
    let agent_ids = ["A", "B"]
        .map(|name| {
            agents
                .create(CreateAgentRequest {
                    name: name.to_owned(),
                    personality_id: "architect".to_owned(),
                    runtime_id: runtime_id.to_owned(),
                    model_id: model_id.to_owned(),
                    instructions: String::new(),
                    worktree_isolation: Some(false),
                    result_contract: None,
                })
                .unwrap()
                .id
        })
        .to_vec();
    let executions = Arc::new(
        ExecutionService::new(
            agents.clone(),
            personalities,
            registry,
            workspaces.clone(),
            audit,
        )
        .with_sessions(sessions.clone()),
    );
    let chat = ChatService::new(agents, executions.clone(), workspaces, ledger);
    Stack {
        dir,
        sessions,
        activity,
        executions,
        chat,
        workspace_id,
        other_workspace_id,
        agents: agent_ids,
        runner,
    }
}

impl Stack {
    fn target(&self, agent: usize, execution: &str) -> SessionTarget {
        SessionTarget {
            execution_id: execution.to_owned(),
            workspace_id: self.workspace_id.clone(),
            agent_id: self.agents[agent].clone(),
        }
    }

    /// Runs an execution of agent `agent` on its own thread, like the app does.
    fn start(
        &self,
        agent: usize,
        execution: &str,
    ) -> thread::JoinHandle<(ExecutionRecord, Vec<ExecutionEvent>)> {
        let executions = self.executions.clone();
        let request = RunAgentRequest {
            task_id: format!("task-{execution}"),
            workspace_id: self.workspace_id.clone(),
            agent_id: self.agents[agent].clone(),
            description: "Work for a long time".to_owned(),
        };
        let id = execution.to_owned();
        thread::spawn(move || {
            let observer = Collector::default();
            let record = executions.run_with_id(id, request, &observer).unwrap();
            let events = observer.0.into_inner().unwrap();
            (record, events)
        })
    }

    /// The kinds of activity the sessions announced for `execution`, in order.
    fn session_activity(&self, execution: &str) -> Vec<ExecutionEventKind> {
        self.activity
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.execution_id == execution)
            .map(|e| e.kind)
            .collect()
    }

    fn output_of(&self, agent: usize, execution: &str) -> String {
        self.sessions
            .snapshot(&self.target(agent, execution))
            .map(|s| s.output)
            .unwrap_or_default()
    }

    /// Waits until the stand-in is working and returns the pid of its innermost process.
    fn wait_for_grandchild(&self, agent: usize, execution: &str) -> i32 {
        for _ in 0..500 {
            let output = self.output_of(agent, execution);
            if let Some(pid) = output
                .lines()
                .find_map(|l| l.trim().strip_prefix("grandchild "))
                .and_then(|p| p.trim().parse().ok())
            {
                return pid;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!(
            "the process never started working: {:?}",
            self.output_of(agent, execution)
        );
    }
}

/// Collects the messages the chat announces.
#[derive(Default)]
struct Chatter(Mutex<Vec<Message>>);

impl ExecutionObserver for Chatter {
    fn on_event(&self, _: &ExecutionEvent) {}
}

impl ChatObserver for Chatter {
    fn on_message(&self, message: &Message) {
        self.0.lock().unwrap().push(message.clone());
    }
}

#[test]
fn a_cancelled_execution_is_not_reported_as_a_failure_and_the_agent_can_run_again() {
    let s = stack(SLOW_CLAUDE);
    let send = |content: &str| {
        s.chat
            .send(SendMessageRequest {
                workspace_id: s.workspace_id.clone(),
                agent_id: s.agents[0].clone(),
                content: content.to_owned(),
            })
            .unwrap()
    };
    let (started, pending) = send("Work for a long time");
    let chatter = Arc::new(Chatter::default());
    let job = {
        let chatter = chatter.clone();
        thread::spawn(move || pending.run(chatter.as_ref()))
    };
    let target = s.target(0, &started.execution_id);
    s.wait_for_grandchild(0, &started.execution_id);

    // While it runs the agent is busy: one execution at a time per agent.
    assert!(s
        .chat
        .send(SendMessageRequest {
            workspace_id: s.workspace_id.clone(),
            agent_id: s.agents[0].clone(),
            content: "again".to_owned(),
        })
        .is_err());
    s.sessions.interrupt(&target).unwrap();
    job.join().unwrap();

    let messages = chatter.0.lock().unwrap().clone();
    let answer = messages
        .iter()
        .find(|m| m.role == MessageRole::Assistant)
        .unwrap();
    assert_eq!(answer.failure_kind.as_deref(), Some("cancelled"));
    let (_, again) = send("Work again");
    drop(again);
}

fn process_exists(pid: i32) -> bool {
    use nix::sys::signal::kill;
    use nix::unistd::Pid;
    kill(Pid::from_raw(pid), None).is_ok()
}

fn assert_gone(pid: i32) {
    for _ in 0..200 {
        if !process_exists(pid) {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("process {pid} is still alive: it was left behind");
}

fn kinds(events: &[ExecutionEvent]) -> Vec<ExecutionEventKind> {
    events.iter().map(|e| e.kind).collect()
}

#[test]
fn an_execution_gets_a_terminal_and_completes_with_its_structured_result() {
    let s = stack(QUICK_CLAUDE);

    let (record, _) = s.start(0, "exec-1").join().unwrap();

    assert_eq!(record.execution.status, ExecutionStatus::Completed);
    // The structured result survives the terminal: colour codes and CRLF did not break it.
    assert_eq!(record.execution.result.as_deref(), Some("All good."));
    assert_eq!(record.execution.metadata["durationMs"], "5");
    assert_eq!(
        s.session_activity("exec-1"),
        [
            ExecutionEventKind::TerminalConnected,
            ExecutionEventKind::ProcessExited
        ]
    );
    let exited = s
        .activity
        .0
        .lock()
        .unwrap()
        .iter()
        .find(|e| e.kind == ExecutionEventKind::ProcessExited)
        .cloned()
        .unwrap();
    assert_eq!(exited.metadata["exitCode"], "0");
    // The raw terminal output is kept apart from the chat: it has the codes, the result has not.
    let snapshot = s.sessions.snapshot(&s.target(0, "exec-1")).unwrap();
    assert!(snapshot.output.contains("\u{1b}[?25l"));
    assert!(snapshot.command.starts_with("claude -p"));
    assert_eq!(snapshot.status, SessionStatus::Exited);
    assert_eq!(s.sessions.live_executions(), Vec::<String>::new());
}

#[test]
fn interrupting_stops_the_whole_process_tree_and_the_execution_is_cancelled() {
    let s = stack(SLOW_CLAUDE);
    let job = s.start(0, "exec-1");
    let grandchild = s.wait_for_grandchild(0, "exec-1");
    assert!(process_exists(grandchild));

    s.sessions.interrupt(&s.target(0, "exec-1")).unwrap();
    let (record, events) = job.join().unwrap();

    // Ctrl+C reached the process the CLI had started, not just the CLI.
    assert_gone(grandchild);
    assert_eq!(record.execution.status, ExecutionStatus::Cancelled);
    assert_eq!(record.task.status, TaskStatus::Cancelled);
    assert!(
        record.execution.failure.is_none(),
        "a user stop is not a failure"
    );
    // What the user did, then what the process did, announced by the session...
    assert_eq!(
        s.session_activity("exec-1"),
        [
            ExecutionEventKind::TerminalConnected,
            ExecutionEventKind::UserInterrupted,
            ExecutionEventKind::ProcessExited
        ]
    );
    // ...and the execution ends as cancelled, never as failed.
    let kinds = kinds(&events);
    assert_eq!(kinds.last(), Some(&ExecutionEventKind::Cancelled));
    assert!(!kinds.contains(&ExecutionEventKind::Failed));
    assert_eq!(s.sessions.live_executions(), Vec::<String>::new());
    assert_eq!(
        s.sessions.interrupt(&s.target(0, "exec-1")),
        Err(ControlError::NotRunning)
    );
}

#[test]
fn terminate_ends_a_process_that_ignores_ctrl_c() {
    let s = stack(STUBBORN_CLAUDE);
    let job = s.start(0, "exec-1");
    let grandchild = s.wait_for_grandchild(0, "exec-1");

    s.sessions.interrupt(&s.target(0, "exec-1")).unwrap();
    thread::sleep(Duration::from_millis(500));
    assert!(process_exists(grandchild), "it ignores Ctrl+C");
    assert_eq!(
        s.sessions.snapshot(&s.target(0, "exec-1")).unwrap().status,
        SessionStatus::Interrupting
    );

    s.sessions.terminate(&s.target(0, "exec-1")).unwrap();
    let (record, _) = job.join().unwrap();

    assert_gone(grandchild);
    assert_eq!(record.execution.status, ExecutionStatus::Cancelled);
    assert_eq!(
        s.sessions.user_action("exec-1"),
        Some(UserAction::Terminated)
    );
}

#[test]
fn interrupting_agent_a_leaves_agent_b_running() {
    let s = stack(SLOW_CLAUDE);
    let (a, b) = (s.start(0, "exec-a"), s.start(1, "exec-b"));
    let (pid_a, pid_b) = (
        s.wait_for_grandchild(0, "exec-a"),
        s.wait_for_grandchild(1, "exec-b"),
    );
    assert_ne!(pid_a, pid_b);

    s.sessions.interrupt(&s.target(0, "exec-a")).unwrap();
    let (record_a, _) = a.join().unwrap();

    assert_eq!(record_a.execution.status, ExecutionStatus::Cancelled);
    assert_gone(pid_a);
    // B is untouched: alive, running, and nobody has blamed the user for it.
    assert!(process_exists(pid_b));
    assert_eq!(
        s.sessions.snapshot(&s.target(1, "exec-b")).unwrap().status,
        SessionStatus::Running
    );
    assert_eq!(s.sessions.user_action("exec-b"), None);

    s.sessions.interrupt(&s.target(1, "exec-b")).unwrap();
    let (record_b, _) = b.join().unwrap();
    assert_eq!(record_b.execution.status, ExecutionStatus::Cancelled);
    assert_gone(pid_b);
}

#[test]
fn a_session_cannot_be_controlled_from_another_workspace_or_agent() {
    let s = stack(SLOW_CLAUDE);
    let job = s.start(0, "exec-1");
    let grandchild = s.wait_for_grandchild(0, "exec-1");

    let foreign_workspace = SessionTarget {
        workspace_id: s.other_workspace_id.clone(),
        ..s.target(0, "exec-1")
    };
    let foreign_agent = s.target(1, "exec-1");
    for target in [&foreign_workspace, &foreign_agent] {
        assert_eq!(s.sessions.interrupt(target), Err(ControlError::NotFound));
        assert_eq!(s.sessions.terminate(target), Err(ControlError::NotFound));
        assert_eq!(s.sessions.input(target, b"x"), Err(ControlError::NotFound));
        assert!(s.sessions.snapshot(target).is_err());
    }
    assert!(
        process_exists(grandchild),
        "nothing was done to the process"
    );
    assert_eq!(s.sessions.user_action("exec-1"), None);

    s.sessions.interrupt(&s.target(0, "exec-1")).unwrap();
    job.join().unwrap();
}

#[test]
fn the_terminal_of_these_runtimes_is_read_only() {
    let s = stack(SLOW_CLAUDE);
    let job = s.start(0, "exec-1");
    s.wait_for_grandchild(0, "exec-1");

    assert_eq!(
        s.sessions.input(&s.target(0, "exec-1"), b"rm -rf /\n"),
        Err(ControlError::InputNotSupported)
    );
    assert!(
        !s.sessions
            .snapshot(&s.target(0, "exec-1"))
            .unwrap()
            .input_enabled
    );

    s.sessions.interrupt(&s.target(0, "exec-1")).unwrap();
    job.join().unwrap();
}

#[test]
fn the_terminal_is_a_view_of_an_authorized_process_not_a_way_to_start_one() {
    let s = stack(QUICK_CLAUDE);
    let spec = |program: &str, context: ProcessContext| ProcessSpec {
        program: program.to_owned(),
        args: vec!["-c".to_owned(), "touch atlas-pwned".to_owned()],
        context,
        terminal: Some(TerminalRequest::new(true)),
        cwd: Some(s.dir.path().join("p1")),
        ..ProcessSpec::probe(program, &[], Duration::from_secs(5))
    };
    let scope = ExecutionScope {
        workspace_id: s.workspace_id.clone(),
        agent_id: s.agents[0].clone(),
        execution_id: "exec-1".to_owned(),
        ..ExecutionScope::for_tests()
    };

    // A shell, an unknown program and a probe: the guard refuses each before anything starts,
    // so no session exists for any of them.
    for (program, context) in [
        ("sh", ProcessContext::Runtime(scope.clone())),
        ("bash", ProcessContext::Runtime(scope.clone())),
        ("rm", ProcessContext::Runtime(scope.clone())),
        ("sh", ProcessContext::Probe),
    ] {
        let result = s.runner.run(&spec(program, context), &|_| {});
        assert!(
            matches!(result, Err(ProcessError::PermissionDenied(_))),
            "{program}: {result:?}"
        );
    }
    assert_eq!(s.sessions.live_executions(), Vec::<String>::new());
    assert!(s.sessions.snapshot(&s.target(0, "exec-1")).is_err());
    assert!(!Path::new("atlas-pwned").exists());
    assert!(!s.dir.path().join("p1/atlas-pwned").exists());
}

/// The real CLIs, a real model call, the real guard, a real PTY: start a long task, watch the
/// terminal fill, press Ctrl+C, and check the process is gone and the execution cancelled.
/// Opt-in because they need the CLI installed and signed in (they spend a little quota):
/// `cargo test real_ -- --ignored --nocapture --test-threads=1`.
fn real_cli_can_be_interrupted(runtime_id: &str, model_id: &str) {
    let s = build_stack(None, runtime_id, model_id);
    let execution = s.executions.next_execution_id();
    let job = {
        let executions = s.executions.clone();
        let request = RunAgentRequest {
            task_id: "task-real".to_owned(),
            workspace_id: s.workspace_id.clone(),
            agent_id: s.agents[0].clone(),
            description: "Write a very long, detailed essay (at least 3000 words) about the \
                          history of computing. Do not use any tools."
                .to_owned(),
        };
        let id = execution.clone();
        thread::spawn(move || {
            executions
                .run_with_id(id, request, &Collector::default())
                .unwrap()
        })
    };

    let target = s.target(0, &execution);
    let mut output = String::new();
    for _ in 0..1500 {
        output = s
            .sessions
            .snapshot(&target)
            .map(|t| t.output)
            .unwrap_or_default();
        if output.len() > 40 || job.is_finished() {
            break;
        }
        thread::sleep(Duration::from_millis(40));
    }
    println!("terminal after the first seconds:\n{output}");
    assert!(
        !output.is_empty(),
        "the terminal received nothing from the real {runtime_id} process"
    );

    let started = std::time::Instant::now();
    s.sessions.interrupt(&target).unwrap();
    let record = job.join().unwrap();
    println!(
        "{runtime_id}: interrupted, ended as {:?} after {:?}",
        record.execution.status,
        started.elapsed()
    );

    assert_eq!(record.execution.status, ExecutionStatus::Cancelled);
    assert!(
        s.sessions.live_executions().is_empty(),
        "the process is gone"
    );
    assert_eq!(
        s.sessions.snapshot(&target).unwrap().status,
        SessionStatus::Exited
    );
}

#[test]
#[ignore = "needs the Claude CLI installed and signed in; makes a real model call"]
fn real_claude_can_be_interrupted() {
    real_cli_can_be_interrupted("claude", "haiku");
}

#[test]
#[ignore = "needs OpenCode installed and a free model; makes a real model call"]
fn real_opencode_can_be_interrupted() {
    real_cli_can_be_interrupted("opencode", "opencode/big-pickle");
}
