#![allow(clippy::assert_is_empty)]
//! A workspace with several projects, through the real execution path: where the process runs,
//! which folders it is given, what the prompt says of the other projects, and that a workspace
//! with one project is exactly what it was.

use std::sync::{Arc, Mutex};

use super::*;
use crate::application::agents::CreateAgentRequest;
use crate::application::harness::context::HarnessContextBuilder;
use crate::application::harness::workspace_tests::{shop_with, Shop, BACK, FRONT, ORDERS};
use crate::application::process::fake::{ok, FakeProcessRunner};
use crate::application::process::{ProcessContext, ProcessSpec};
use crate::application::runtimes::fake::FakeRuntime;
use crate::application::runtimes::ClaudeRuntime;
use crate::application::security::testutil::TempDir;
use crate::application::workspace::{ProjectInput, WorkspaceInput};
use crate::domain::execution::ExecutionStatus;
use crate::domain::scope::ScopeRequest;
use crate::domain::worktree::{MergeStatus, WorktreeStatus};

#[derive(Default)]
struct Quiet(Mutex<Vec<ExecutionEvent>>);

impl ExecutionObserver for Quiet {
    fn on_event(&self, event: &ExecutionEvent) {
        self.0.lock().unwrap().push(event.clone());
    }
}

const CLAUDE_ANSWER: &str =
    r#"{"type":"result","subtype":"success","is_error":false,"result":"done\n"}"#;

struct Stack {
    shop: Shop,
    service: ExecutionService,
    spy: Arc<FakeProcessRunner>,
    agent_id: String,
}

/// The real Claude adapter over a spy process port, in the three-project shop, with the Harness
/// of every project in place and the workspace context built.
fn claude_stack(with_harness: bool) -> Stack {
    let spy = Arc::new(FakeProcessRunner::new(&["claude"], |_| ok(CLAUDE_ANSWER)));
    let registry = Arc::new(RuntimeRegistry::new(vec![
        Arc::new(ClaudeRuntime::new(spy.clone())) as Arc<_>,
    ]));
    let shop = shop_with(registry.clone());
    let agent_id = shop
        .agents
        .create(CreateAgentRequest {
            permission_profile_id: None,
            name: "architect".to_owned(),
            personality_id: "architect".to_owned(),
            runtime_id: "claude".to_owned(),
            model_id: "sonnet".to_owned(),
            instructions: String::new(),
            worktree_isolation: Some(false),
            result_contract: None,
        })
        .unwrap()
        .id;
    let mut service = ExecutionService::new(
        shop.agents.clone(),
        shop.personalities.clone(),
        registry,
        shop.workspaces.clone(),
        Arc::new(crate::application::security::AuditLog::default()),
    );
    if with_harness {
        shop.service
            .initialize_workspace(&shop.workspace_id, None, true, &|_| {})
            .unwrap();
        service = service
            .with_harness(Arc::new(HarnessContextBuilder::new(shop.harnesses.clone())))
            .with_workspace_context(shop.context.clone());
    }
    Stack {
        shop,
        service,
        spy,
        agent_id,
    }
}

fn scope(shop: &Shop, working: usize, read: &[usize]) -> ScopeRequest {
    ScopeRequest {
        working_project_id: Some(shop.ids[working].clone()),
        read_project_ids: read.iter().map(|i| shop.ids[*i].clone()).collect(),
    }
}

fn ask(stack: &Stack, scope: Option<ScopeRequest>) -> Result<ExecutionRecord, ExecutionError> {
    stack.service.run_with_id(
        stack.service.next_execution_id(),
        RunAgentRequest {
            task_id: "task-1".to_owned(),
            workspace_id: stack.shop.workspace_id.clone(),
            agent_id: stack.agent_id.clone(),
            description: "check the checkout contract".to_owned(),
            scope,
        },
        &Quiet::default(),
    )
}

fn launched(stack: &Stack) -> ProcessSpec {
    let calls = stack.spy.calls.lock().unwrap();
    assert_eq!(calls.len(), 1, "one launch");
    calls[0].clone()
}

fn add_dirs(spec: &ProcessSpec) -> Vec<&str> {
    spec.args
        .windows(2)
        .filter(|pair| pair[0] == "--add-dir")
        .map(|pair| pair[1].as_str())
        .collect()
}

fn runtime_scope(spec: &ProcessSpec) -> crate::application::process::ExecutionScope {
    match &spec.context {
        ProcessContext::Runtime(scope) => scope.clone(),
        other => panic!("not a runtime launch: {other:?}"),
    }
}

#[test]
fn without_a_scope_the_execution_is_the_primary_project_alone_as_it_always_was() {
    let stack = claude_stack(false);

    let record = ask(&stack, None).unwrap();

    let spec = launched(&stack);
    assert_eq!(record.execution.status, ExecutionStatus::Completed);
    assert_eq!(spec.cwd.as_deref(), Some(std::path::Path::new(FRONT)));
    assert!(add_dirs(&spec).is_empty());
    assert!(record
        .execution
        .prompt
        .contains("Project: front\nPath: /shop/front\nDetected technologies:\n- Angular"));
    assert!(!record.execution.prompt.contains("other projects"));
    assert_eq!(runtime_scope(&spec).project_id, None);
}

#[test]
fn the_working_project_is_where_the_process_runs_and_what_the_prompt_names_first() {
    let stack = claude_stack(false);

    let record = ask(&stack, Some(scope(&stack.shop, 1, &[]))).unwrap();

    let spec = launched(&stack);
    assert_eq!(spec.cwd.as_deref(), Some(std::path::Path::new(BACK)));
    assert!(record
        .execution
        .prompt
        .contains("Project: API\nPath: /shop/back"));
    // Only a project that is not the primary one is named to the security layer.
    assert_eq!(
        runtime_scope(&spec).project_id.as_deref(),
        Some(stack.shop.ids[1].as_str())
    );
}

#[test]
fn a_reading_agent_is_given_the_other_folders_and_told_it_may_not_change_them() {
    let stack = claude_stack(false);

    let record = ask(&stack, Some(scope(&stack.shop, 0, &[1, 2]))).unwrap();

    let spec = launched(&stack);
    assert_eq!(spec.cwd.as_deref(), Some(std::path::Path::new(FRONT)));
    assert_eq!(add_dirs(&spec), [BACK, ORDERS]);
    let prompt = &record.execution.prompt;
    assert!(prompt.contains("You work in front (the working project)"));
    assert!(prompt
        .contains("- API (backend) at /shop/back — read-only: consult it, never change it; Maven"));
    assert!(prompt.contains("- Orders (service) at /shop/orders — read-only"));
    assert!(prompt.contains("Change files only in the working project"));
    assert!(!prompt.contains("not opened for you"));
}

#[test]
fn a_project_outside_the_workspace_stops_the_execution_before_anything_starts() {
    let stack = claude_stack(false);

    let unknown = ask(
        &stack,
        Some(ScopeRequest {
            read_project_ids: vec!["ghost".to_owned()],
            ..ScopeRequest::default()
        }),
    )
    .unwrap_err();

    assert_eq!(unknown, ExecutionError::InvalidScope("ghost".to_owned()));
    assert!(stack.spy.calls.lock().unwrap().is_empty());
    assert!(AppError::from(&unknown).is(ErrorCode::ScopeInvalid));
}

#[test]
fn the_prompt_carries_the_workspace_context_and_a_fragment_of_each_project_in_scope_only() {
    let stack = claude_stack(true);

    let record = ask(&stack, Some(scope(&stack.shop, 0, &[1]))).unwrap();

    let prompt = &record.execution.prompt;
    assert!(prompt.contains("WORKSPACE CONTEXT — Shop"));
    assert!(prompt.contains("PROJECT — front (working project)"));
    assert!(prompt.contains("PROJECT — API (backend, read-only)"));
    // What is known of Orders is not told: it is not in this execution's scope.
    assert!(!prompt.contains("PROJECT — Orders"));
    assert!(!prompt.contains("orders-service"));
    // And it says so in the log of the run.
    let logs = record.execution.logs.join("\n");
    assert!(logs.contains("Scope: working in front, consulting API"));
    assert!(logs.contains("Project Harness of API (read-only)"));
    assert!(record.execution.context.is_some());
}

#[test]
fn a_single_project_scope_in_a_multi_project_workspace_still_gets_the_workspace_context() {
    let stack = claude_stack(true);

    let record = ask(&stack, Some(scope(&stack.shop, 2, &[]))).unwrap();

    let prompt = &record.execution.prompt;
    assert!(prompt.contains("WORKSPACE CONTEXT — Shop"));
    assert!(prompt.contains("PROJECT — Orders (service, working project)"));
    assert!(!prompt.contains("PROJECT — front"));
}

#[test]
fn a_workspace_with_one_project_gets_the_same_prompt_as_before_workspaces_had_several() {
    let stack = claude_stack(true);
    for id in &stack.shop.ids[1..] {
        stack
            .shop
            .workspaces
            .remove_project(&stack.shop.workspace_id, id)
            .unwrap();
    }
    // The context of the workspace made when it had three projects is gone with them.
    stack
        .shop
        .service
        .forget_workspace(&stack.shop.workspace_id);

    let record = ask(&stack, None).unwrap();

    let prompt = &record.execution.prompt;
    assert!(!prompt.contains("WORKSPACE CONTEXT"));
    assert!(!prompt.contains("PROJECT —"));
    assert!(prompt.contains("Project: front"));
}

// ---- worktrees and editing --------------------------------------------------------------

use crate::application::agents::AgentService;
use crate::application::config::memory::MemoryStore;
use crate::application::config::ConfigRepository;
use crate::application::personalities::PersonalityService;
use crate::application::projects::fake::FakeInspector;
use crate::application::security::SecurityService;
use crate::application::workspace::WorkspaceService;
use crate::application::worktree::tests::init_repo;
use crate::application::worktree::{WorktreeLayout, WorktreeService};
use crate::infrastructure::GitWorktreeManager;

struct Repos {
    _data: TempDir,
    front: TempDir,
    back: TempDir,
    service: ExecutionService,
    worktrees: Arc<WorktreeService>,
    runtime: Arc<FakeRuntime>,
    workspace_id: String,
    back_id: String,
    agents: Arc<AgentService>,
}

/// Two independent Git repositories in one workspace, and a runtime that edits by writing a
/// file in the directory it is given.
fn two_repositories(extra_dirs: bool) -> Repos {
    let data = TempDir::new("scope data");
    let front = TempDir::new("scope front");
    let back = TempDir::new("scope back");
    init_repo(front.path(), "main");
    init_repo(back.path(), "main");
    let config = Arc::new(ConfigRepository::load(Box::<MemoryStore>::default()));
    let personalities = Arc::new(PersonalityService::new(config.clone()));
    let mut fake = FakeRuntime::new("rt-a", Ok("done"))
        .with_file_edit()
        .with_work(|request| {
            std::fs::write(request.working_dir.join("feature.txt"), "x\n").unwrap();
        });
    if extra_dirs {
        fake = fake.with_extra_dirs();
    }
    let runtime = Arc::new(fake);
    let registry = Arc::new(RuntimeRegistry::new(vec![runtime.clone() as Arc<_>]));
    let agents = Arc::new(AgentService::new(
        config.clone(),
        personalities.clone(),
        registry.clone(),
    ));
    let (front_path, back_path) = (
        front.path().to_string_lossy().into_owned(),
        back.path().to_string_lossy().into_owned(),
    );
    let workspaces = Arc::new(WorkspaceService::new(
        config.clone(),
        agents.clone(),
        Arc::new(FakeInspector::with(&[
            (front_path.as_str(), &["Angular"]),
            (back_path.as_str(), &["Maven"]),
        ])),
    ));
    let workspace = workspaces
        .create(&WorkspaceInput {
            name: "Shop".to_owned(),
            project_path: front_path,
            description: None,
        })
        .unwrap();
    let with_back = workspaces
        .add_project(
            &workspace.id,
            &ProjectInput {
                path: back_path,
                name: Some("API".to_owned()),
                role: None,
                depends_on: Vec::new(),
            },
        )
        .unwrap();
    let layout = WorktreeLayout::new(data.path().join("worktrees"));
    let security = Arc::new(SecurityService::new(config.clone()));
    let worktrees = Arc::new(WorktreeService::new(
        Arc::new(GitWorktreeManager::new("git".into(), layout.clone())),
        layout,
        config,
        security.clone(),
    ));
    let service = ExecutionService::new(
        agents.clone(),
        personalities,
        registry,
        workspaces,
        Arc::new(crate::application::security::AuditLog::default()),
    )
    .with_worktrees(worktrees.clone())
    .with_policies(security);
    Repos {
        _data: data,
        front,
        back,
        service,
        worktrees,
        runtime,
        back_id: with_back.projects[1].id.clone(),
        workspace_id: workspace.id,
        agents,
    }
}

fn developer(repos: &Repos, isolated: bool) -> String {
    let agent = repos
        .agents
        .create(CreateAgentRequest {
            permission_profile_id: None,
            name: "Dev".to_owned(),
            personality_id: "architect".to_owned(),
            runtime_id: "rt-a".to_owned(),
            model_id: "m1".to_owned(),
            instructions: String::new(),
            worktree_isolation: Some(isolated),
            result_contract: None,
        })
        .unwrap();
    repos
        .agents
        .set_permission_profile(&agent.id, "developer")
        .unwrap();
    agent.id
}

fn run_in(repos: &Repos, agent: &str, scope: Option<ScopeRequest>) -> ExecutionRecord {
    repos
        .service
        .run_with_id(
            repos.service.next_execution_id(),
            RunAgentRequest {
                task_id: "task-1".to_owned(),
                workspace_id: repos.workspace_id.clone(),
                agent_id: agent.to_owned(),
                description: "add the field".to_owned(),
                scope,
            },
            &Quiet::default(),
        )
        .unwrap()
}

#[test]
fn an_isolated_agent_edits_a_worktree_of_the_repository_of_the_project_it_works_in() {
    let repos = two_repositories(false);
    let agent = developer(&repos, true);

    let record = run_in(
        &repos,
        &agent,
        Some(ScopeRequest {
            working_project_id: Some(repos.back_id.clone()),
            ..ScopeRequest::default()
        }),
    );

    let sent = repos.runtime.requests.lock().unwrap()[0].clone();
    assert_eq!(record.execution.status, ExecutionStatus::Completed);
    // The worktree is the second repository's, and the run is held to it.
    assert!(sent.allow_edits);
    assert!(sent.scope.isolated);
    assert_eq!(
        sent.scope.project_id.as_deref(),
        Some(repos.back_id.as_str())
    );
    let worktree = repos.worktrees.get(&record.execution.id).unwrap();
    let back = dunce::canonicalize(repos.back.path()).unwrap();
    assert_eq!(std::path::Path::new(&worktree.repository_path), back);
    // The change went to that repository, and the other project was never touched.
    assert_eq!(worktree.merge_status, MergeStatus::Merged);
    assert_eq!(worktree.status, WorktreeStatus::Cleaned);
    assert!(repos.back.path().join("feature.txt").is_file());
    assert!(!repos.front.path().join("feature.txt").exists());
}

#[test]
fn an_agent_that_edits_is_not_handed_the_folders_it_only_reads() {
    let repos = two_repositories(true);
    let agent = developer(&repos, true);

    let record = run_in(
        &repos,
        &agent,
        Some(ScopeRequest {
            read_project_ids: vec![repos.back_id.clone()],
            ..ScopeRequest::default()
        }),
    );

    let sent = repos.runtime.requests.lock().unwrap()[0].clone();
    // The runtime can take more folders, but a run that edits could edit them too: not given.
    assert!(sent.allow_edits);
    assert!(sent.extra_dirs.is_empty());
    let prompt = &record.execution.prompt;
    assert!(prompt.contains("not opened for you in this run"));
    assert!(record
        .execution
        .logs
        .iter()
        .any(|l| l.contains("not given folders it could edit")));
    // It works in its own worktree and writes nothing in the read-only project.
    assert!(repos.front.path().join("feature.txt").is_file());
    assert!(!repos.back.path().join("feature.txt").exists());
}

#[test]
fn a_run_without_edit_tools_is_given_the_folders_when_its_runtime_can_take_them() {
    let repos = two_repositories(true);
    let agent = developer(&repos, false);

    let record = run_in(
        &repos,
        &agent,
        Some(ScopeRequest {
            read_project_ids: vec![repos.back_id.clone()],
            ..ScopeRequest::default()
        }),
    );

    let sent = repos.runtime.requests.lock().unwrap()[0].clone();
    assert!(!sent.allow_edits);
    assert_eq!(sent.extra_dirs, [repos.back.path().to_path_buf()]);
    assert!(!record.execution.prompt.contains("not opened for you"));
}

#[test]
fn a_runtime_that_cannot_take_other_folders_is_not_pretended_to() {
    let repos = two_repositories(false);
    let agent = developer(&repos, false);

    let record = run_in(
        &repos,
        &agent,
        Some(ScopeRequest {
            read_project_ids: vec![repos.back_id.clone()],
            ..ScopeRequest::default()
        }),
    );

    let sent = repos.runtime.requests.lock().unwrap()[0].clone();
    assert!(sent.extra_dirs.is_empty());
    assert!(record
        .execution
        .prompt
        .contains("not opened for you in this run"));
    assert!(record
        .execution
        .logs
        .iter()
        .any(|l| l.contains("cannot be given other project folders")));
}
