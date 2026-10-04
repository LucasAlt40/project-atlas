//! Whole backend path, no UI and no real CLI: services -> real runtime adapters -> the guard ->
//! a scripted OS runner. Shows that the runtimes still run, that every launch passes the guard,
//! and that the audit trail lands on the execution record.

use std::fs;
use std::sync::{Arc, Mutex};

use super::{ApprovalBroker, AuditLog, GuardedProcessRunner, NoSandbox, SecurityService};
use crate::application::agents::{AgentService, CreateAgentRequest};
use crate::application::config::memory::MemoryStore;
use crate::application::config::ConfigRepository;
use crate::application::executions::{ExecutionObserver, ExecutionService, RunAgentRequest};
use crate::application::personalities::PersonalityService;
use crate::application::process::fake::{ok, FakeProcessRunner};
use crate::application::process::ProcessRunner;
use crate::application::projects::fake::FakeInspector;
use crate::application::runtimes::{RuntimeRegistry, RUNTIME_PROGRAMS};
use crate::application::security::testutil::TempDir;
use crate::application::workspace::{WorkspaceInput, WorkspaceService};
use crate::domain::execution::{ExecutionEvent, ExecutionEventKind, ExecutionStatus, FailureKind};
use crate::domain::security::{DecisionSource, PermissionAction, PermissionOutcome};

const CLAUDE_ANSWER: &str = r#"{"type":"result","subtype":"success","is_error":false,"duration_ms":5,"num_turns":1,"result":"All good.\n","total_cost_usd":0.001}"#;
const OPENCODE_ANSWER: &str = concat!(
    r#"{"type":"text","part":{"type":"text","text":"All good."}}"#,
    "\n"
);

#[derive(Default)]
struct Collector(Mutex<Vec<ExecutionEvent>>);

impl ExecutionObserver for Collector {
    fn on_event(&self, event: &ExecutionEvent) {
        self.0.lock().unwrap().push(event.clone());
    }
}

struct Stack {
    _dir: TempDir,
    project: std::path::PathBuf,
    config: Arc<ConfigRepository>,
    fake: Arc<FakeProcessRunner>,
    executions: ExecutionService,
    workspaces: Arc<WorkspaceService>,
    agents: Arc<AgentService>,
}

fn stack() -> Stack {
    stack_in(false)
}

/// The same stack; with `repository` the project is a Git repository and executions of agents
/// that ask for it get a worktree.
fn stack_in(repository: bool) -> Stack {
    let dir = TempDir::new("e2e");
    let project = dir.path().join("project");
    fs::create_dir_all(&project).unwrap();
    if repository {
        crate::application::worktree::tests::init_repo(&project, "main");
    }
    let config = Arc::new(ConfigRepository::load(Box::<MemoryStore>::default()));
    let fake = Arc::new(FakeProcessRunner::new(
        &["opencode", "claude"],
        |spec| match (spec.program.as_str(), spec.args[0].as_str()) {
            ("opencode", "models") => ok("opencode/m\n"),
            ("opencode", "run") => ok(OPENCODE_ANSWER),
            ("claude", "-p") => ok(CLAUDE_ANSWER),
            _ => ok("1.0\n"),
        },
    ));
    let audit = Arc::new(AuditLog::default());
    let runner: Arc<dyn ProcessRunner> = Arc::new(GuardedProcessRunner::new(
        fake.clone(),
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
    let path = project.to_string_lossy().into_owned();
    let workspaces = Arc::new(WorkspaceService::new(
        config.clone(),
        agents.clone(),
        Arc::new(FakeInspector::with(&[(path.as_str(), &["Rust"])])),
    ));
    let layout = crate::application::worktree::WorktreeLayout::new(dir.path().join("worktrees"));
    let worktrees = Arc::new(crate::application::worktree::WorktreeService::new(
        Arc::new(crate::infrastructure::GitWorktreeManager::new(
            "git".into(),
            layout.clone(),
        )),
        layout,
        config.clone(),
        Arc::new(SecurityService::new(config.clone())),
    ));
    let executions = ExecutionService::new(
        agents.clone(),
        personalities,
        registry,
        workspaces.clone(),
        audit,
    )
    .with_worktrees(worktrees);
    Stack {
        _dir: dir,
        project,
        config,
        fake,
        executions,
        workspaces,
        agents,
    }
}

fn agent(stack: &Stack, runtime: &str, model: &str) -> String {
    stack
        .agents
        .create(CreateAgentRequest {
            name: runtime.to_owned(),
            personality_id: "architect".to_owned(),
            runtime_id: runtime.to_owned(),
            model_id: model.to_owned(),
            instructions: String::new(),
            worktree_isolation: Some(false),
            result_contract: None,
        })
        .unwrap()
        .id
}

fn workspace(stack: &Stack) -> String {
    stack
        .workspaces
        .create(&WorkspaceInput {
            name: "W".to_owned(),
            project_path: stack.project.to_string_lossy().into_owned(),
            description: None,
        })
        .unwrap()
        .id
}

fn run(
    stack: &Stack,
    workspace_id: &str,
    agent_id: &str,
    observer: &Collector,
) -> crate::domain::execution::ExecutionRecord {
    run_as("exec-e2e", stack, workspace_id, agent_id, observer)
}

fn run_as(
    execution_id: &str,
    stack: &Stack,
    workspace_id: &str,
    agent_id: &str,
    observer: &Collector,
) -> crate::domain::execution::ExecutionRecord {
    stack
        .executions
        .run_with_id(
            execution_id.to_owned(),
            RunAgentRequest {
                task_id: "task-e2e".to_owned(),
                workspace_id: workspace_id.to_owned(),
                agent_id: agent_id.to_owned(),
                description: "Say hi".to_owned(),
            },
            observer,
        )
        .unwrap()
}

#[test]
fn claude_and_opencode_still_run_through_the_guard_and_the_launch_is_audited() {
    let s = stack();
    let ws = workspace(&s);
    for (runtime, model) in [("claude", "sonnet"), ("opencode", "opencode/m")] {
        let agent_id = agent(&s, runtime, model);

        let record = run(&s, &ws, &agent_id, &Collector::default());

        assert_eq!(
            record.execution.status,
            ExecutionStatus::Completed,
            "{runtime}"
        );
        assert_eq!(record.execution.result.as_deref(), Some("All good."));
        // The decision is on the execution record, tied to its ids.
        let events = &record.execution.permission_events;
        assert_eq!(events.len(), 1, "{runtime}");
        assert_eq!(events[0].action, PermissionAction::LaunchRuntime);
        assert_eq!(events[0].decision, PermissionOutcome::Allowed);
        assert_eq!(events[0].source, DecisionSource::Policy);
        assert_eq!(events[0].target.split(' ').next(), Some(runtime));
        assert_eq!(
            (
                &events[0].execution_id[..],
                &events[0].workspace_id[..],
                &events[0].agent_id[..]
            ),
            ("exec-e2e", &ws[..], &agent_id[..])
        );
        assert!(record
            .execution
            .logs
            .iter()
            .any(|l| l.starts_with("Permission Allowed")));
    }
    // Claude keeps its safe tool list; Atlas's guard did not change the arguments.
    let calls = s.fake.calls.lock().unwrap();
    let claude = calls
        .iter()
        .find(|c| c.program == "claude" && c.args[0] == "-p")
        .unwrap();
    let tools = claude.args.iter().position(|a| a == "--tools").unwrap();
    assert_eq!(claude.args[tools + 1], "Read,Grep,Glob");
    let opencode = calls
        .iter()
        .find(|c| c.program == "opencode" && c.args[0] == "run")
        .unwrap();
    assert_eq!(opencode.args[..3], ["run", "--agent", "plan"]);
    // Each run happened in the workspace's folder.
    assert!(calls
        .iter()
        .filter(|c| c.args[0] == "-p" || c.args[0] == "run")
        .all(|c| c.cwd.as_deref() == Some(s.project.as_path())));
}

#[test]
fn a_workspace_whose_folder_is_gone_cannot_start_a_runtime() {
    let s = stack();
    let ws = workspace(&s);
    let agent_id = agent(&s, "claude", "sonnet");
    fs::remove_dir_all(&s.project).unwrap();

    let observer = Collector::default();
    let record = run(&s, &ws, &agent_id, &observer);

    assert_eq!(record.execution.status, ExecutionStatus::Failed);
    let failure = record.execution.failure.unwrap();
    assert_eq!(failure.kind, FailureKind::PermissionDenied);
    assert_eq!(failure.details.as_deref(), Some("project_unavailable"));
    assert_eq!(record.execution.permission_events.len(), 1);
    assert_eq!(
        record.execution.permission_events[0].decision,
        PermissionOutcome::Denied
    );
    // Nothing was started.
    assert!(s.fake.calls.lock().unwrap().is_empty());
    let kinds: Vec<_> = observer.0.lock().unwrap().iter().map(|e| e.kind).collect();
    assert_eq!(kinds.last(), Some(&ExecutionEventKind::Failed));
}

#[test]
fn switching_workspace_changes_the_policy_the_same_agent_runs_under() {
    let s = stack();
    let a = workspace(&s);
    let b = s
        .workspaces
        .create(&WorkspaceInput {
            name: "B".to_owned(),
            project_path: s.project.to_string_lossy().into_owned(),
            description: None,
        })
        .unwrap()
        .id;
    // Workspace B is locked down by hand-editing its stored policy.
    s.config
        .modify(|c| {
            let ws = c.workspaces.iter_mut().find(|w| w.id == b).unwrap();
            ws.security = crate::domain::security::SecurityPolicy::read_only();
            Ok(())
        })
        .unwrap();
    let agent_id = agent(&s, "claude", "sonnet");
    s.agents
        .set_permission_profile(&agent_id, "developer")
        .unwrap();

    let overview = crate::application::security::SecurityOverview::new(
        Arc::new(SecurityService::new(s.config.clone())),
        s.config.clone(),
        Arc::new(RuntimeRegistry::new(vec![])),
    );
    let in_a = overview.workspace(&a).unwrap();
    let in_b = overview.workspace(&b).unwrap();

    assert_ne!(in_a.policy, in_b.policy);
    assert_eq!(
        in_b.label,
        crate::application::security::service::SecurityLabel::Secure
    );
    assert_eq!(
        in_a.label,
        crate::application::security::service::SecurityLabel::Developer
    );
    // And both still run (launching a read-only runtime is allowed everywhere).
    assert_eq!(
        run(&s, &a, &agent_id, &Collector::default())
            .execution
            .status,
        ExecutionStatus::Completed
    );
}

#[test]
fn an_isolated_run_is_launched_audited_and_held_to_its_worktree_not_the_checkout() {
    let s = stack_in(true);
    let ws = workspace(&s);
    let agent_id = s
        .agents
        .create(CreateAgentRequest {
            name: "isolated".to_owned(),
            personality_id: "architect".to_owned(),
            runtime_id: "claude".to_owned(),
            model_id: "sonnet".to_owned(),
            instructions: String::new(),
            worktree_isolation: None,
            result_contract: None,
        })
        .unwrap();
    assert!(
        agent_id.worktree_isolation,
        "isolated unless told otherwise"
    );

    let record = run_as("exec-1", &s, &ws, &agent_id.id, &Collector::default());

    assert_eq!(
        record.execution.status,
        ExecutionStatus::Completed,
        "{:?}",
        record.execution.failure
    );
    let calls = s.fake.calls.lock().unwrap();
    let launch = calls
        .iter()
        .find(|c| c.program == "claude" && c.args[0] == "-p")
        .unwrap();
    let cwd = launch.cwd.clone().unwrap();
    // The runtime ran in the worktree, which is outside the project and named for the run.
    assert_ne!(cwd, s.project);
    assert!(!cwd.starts_with(&s.project));
    assert!(cwd.ends_with("exec-000001") || cwd.to_string_lossy().contains("exec-0000"));
    // The guard let it start there, and the audit trail says where.
    let events = &record.execution.permission_events;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].decision, PermissionOutcome::Allowed);
    assert_eq!(events[0].cwd.as_deref(), Some(&*cwd.to_string_lossy()));
    // The model was told where it works, and it is not the checkout.
    assert!(record.execution.prompt.contains(&*cwd.to_string_lossy()));
    assert!(!record
        .execution
        .prompt
        .contains(&*s.project.to_string_lossy()));
    // Claude here cannot change files, so nothing is left to keep.
    let worktree = s.config.snapshot().worktrees.pop().unwrap();
    assert_eq!(
        worktree.merge_status,
        crate::domain::worktree::MergeStatus::NothingToMerge
    );
}
