//! Real `git`, as in the worktree tests: what matters is what is left on disk and in Git.

use std::fs;
use std::sync::Arc;

use super::*;
use crate::application::agents::AgentService;
use crate::application::personalities::PersonalityService;
use crate::application::runtimes::RuntimeRegistry;
use crate::application::workspace::WorkspaceService;
use crate::application::worktree::tests::{git, Env};
use crate::application::worktree::RunOutcome;
use crate::domain::security::{Permission, SandboxSettings};
use crate::domain::worktree::{MergeStatus, Validation, WorktreeStatus};

struct World {
    env: Env,
    storage: StorageService,
    sandbox_dir: PathBuf,
}

fn world() -> World {
    // Approval required: a finished execution's work waits for the person, folder kept.
    let env = Env::new(Permission::ApprovalRequired);
    let config = env.config.clone();
    let personalities = Arc::new(PersonalityService::new(config.clone()));
    let runtimes = Arc::new(RuntimeRegistry::new(vec![]));
    let agents = Arc::new(AgentService::new(config.clone(), personalities, runtimes));
    let workspaces = Arc::new(WorkspaceService::new(
        config.clone(),
        agents.clone(),
        Arc::new(crate::application::projects::fake::FakeInspector::with(&[])),
    ));
    let workflows = Arc::new(WorkflowService::new(config, agents, workspaces.clone()));
    let integration = Arc::new(IntegrationService::new(
        workflows.clone(),
        env.service.clone(),
        workspaces,
        Arc::new(crate::application::ide::fake::FakeIde::default()),
    ));
    let sandbox_dir = env.data.path().join("sandbox");
    let sandbox = Arc::new(MacosSandbox::new(
        Arc::new(SandboxSettings::default),
        sandbox_dir.clone(),
        crate::application::runtimes::confinement::default_confinements(),
    ));
    let storage = StorageService::new(
        env.data.path().to_path_buf(),
        env.service.clone(),
        workflows,
        integration,
        Arc::new(SessionRegistry::default()),
        sandbox,
    );
    World {
        env,
        storage,
        sandbox_dir,
    }
}

/// An execution that changed a file and ended, waiting for the person.
fn finished(world: &World, id: &str) -> PathBuf {
    let prepared = world.env.prepare(id);
    fs::write(prepared.working_dir.join("work.txt"), "done\n").unwrap();
    let worktree = world
        .env
        .service
        .finalize(id, RunOutcome::Completed, Validation::NotRun)
        .unwrap();
    assert_eq!(worktree.merge_status, MergeStatus::Pending);
    prepared.working_dir
}

const ALL: CleanupChoice = CleanupChoice {
    worktrees: true,
    sandbox_temp: true,
    branches: false,
};

#[test]
fn measures_the_data_folder_and_what_can_be_cleaned() {
    let world = world();
    let folder = finished(&world, "exec-1");
    fs::create_dir_all(world.sandbox_dir.join("tmp/exec-1")).unwrap();
    fs::write(
        world.sandbox_dir.join("tmp/exec-1/cache.bin"),
        vec![0u8; 4096],
    )
    .unwrap();

    let usage = world.storage.usage();

    assert!(!usage.running);
    assert_eq!(usage.leftovers.worktrees.count, 1);
    assert!(usage.leftovers.worktrees.bytes >= size_of(&folder.join("work.txt")));
    assert_eq!(usage.leftovers.sandbox_temp.count, 1);
    assert_eq!(usage.leftovers.sandbox_temp.bytes, 4096);
    // The folder is still in use: its branch is not a leftover.
    assert_eq!(usage.leftovers.branches, 0);
    let worktrees = usage
        .parts
        .iter()
        .find(|p| p.kind == StoragePartKind::Worktrees)
        .unwrap();
    assert!(worktrees.bytes > 0);
    assert_eq!(
        usage.total_bytes,
        usage.parts.iter().map(|p| p.bytes).sum::<u64>()
    );
}

#[test]
fn removing_a_finished_worktree_keeps_its_work_on_the_branch() {
    let world = world();
    let folder = finished(&world, "exec-2");
    // Something the agent left after the run, never committed.
    fs::write(folder.join("late.txt"), "late\n").unwrap();

    let report = world.storage.clean(ALL).unwrap();

    assert_eq!(report.worktrees_removed, 1);
    assert_eq!(report.worktrees_kept, 0);
    assert!(report.freed_bytes > 0);
    assert!(!folder.exists());
    let record = world.env.service.get("exec-2").unwrap();
    assert_eq!(record.status, WorktreeStatus::Cleaned);
    assert_eq!(record.merge_status, MergeStatus::Blocked);
    // The branch has everything, the late file included.
    let project = world.env.project.path();
    let files = git(project, &["ls-tree", "--name-only", "atlas/exec-000002"]);
    assert!(
        files.contains("work.txt") && files.contains("late.txt"),
        "{files}"
    );
    // The project's checkout was not touched.
    assert_eq!(git(project, &["status", "--porcelain"]), "");
    assert_eq!(world.storage.usage().leftovers.worktrees.count, 0);
}

#[test]
fn a_folder_no_execution_owns_is_removed_too() {
    let world = world();
    let folder = finished(&world, "exec-3");
    // Its record is gone, as when its workspace was deleted.
    world
        .env
        .config
        .modify(|c| {
            c.worktrees.clear();
            Ok(())
        })
        .unwrap();
    let stray = world.env.layout.root().join("ws-gone/exec-000099");
    fs::create_dir_all(&stray).unwrap();
    fs::write(stray.join("junk.txt"), "x").unwrap();

    assert_eq!(world.storage.usage().leftovers.worktrees.count, 2);
    let report = world.storage.clean(ALL).unwrap();

    assert_eq!(report.worktrees_removed, 2);
    assert!(!folder.exists());
    assert!(!stray.exists());
    assert!(!world.env.layout.root().join("ws-gone").exists());
    // Git forgot the worktree; the branch, with the work, stays.
    let project = world.env.project.path();
    assert!(!git(project, &["worktree", "list"]).contains("exec-000003"));
    assert!(git(project, &["ls-tree", "--name-only", "atlas/exec-000003"]).contains("work.txt"));
}

#[test]
fn branches_go_only_when_the_person_asks() {
    let world = world();
    finished(&world, "exec-4");
    world.storage.clean(ALL).unwrap();
    let project = world.env.project.path();
    assert_eq!(world.storage.usage().leftovers.branches, 1);

    let report = world
        .storage
        .clean(CleanupChoice {
            branches: true,
            ..CleanupChoice::default()
        })
        .unwrap();

    assert_eq!(report.branches_removed, 1);
    assert_eq!(git(project, &["branch", "--list", "atlas/*"]), "");
    assert_eq!(world.storage.usage().leftovers.branches, 0);
}

#[test]
fn nothing_is_cleaned_while_an_execution_runs() {
    let world = world();
    let finished_folder = finished(&world, "exec-5");
    let running = world.env.prepare("exec-6");
    fs::create_dir_all(world.sandbox_dir.join("tmp/exec-6")).unwrap();

    let usage = world.storage.usage();
    assert!(usage.running);
    // The running one is never offered.
    assert_eq!(usage.leftovers.worktrees.count, 1);

    let error = world.storage.clean(ALL).unwrap_err();

    assert_eq!(error.code, ErrorCode::StorageBusy);
    assert!(finished_folder.exists());
    assert!(running.working_dir.exists());
    assert!(world.sandbox_dir.join("tmp/exec-6").exists());
}

#[test]
fn sandbox_leftovers_go_and_nothing_else_is_touched_when_only_they_are_chosen() {
    let world = world();
    let folder = finished(&world, "exec-7");
    fs::create_dir_all(world.sandbox_dir.join("tmp/exec-7")).unwrap();
    fs::create_dir_all(world.sandbox_dir.join("profiles")).unwrap();
    fs::write(
        world.sandbox_dir.join("profiles/exec-7-1.sb"),
        "(version 1)",
    )
    .unwrap();

    let report = world
        .storage
        .clean(CleanupChoice {
            sandbox_temp: true,
            ..CleanupChoice::default()
        })
        .unwrap();

    assert_eq!(report.sandbox_temp_removed, 2);
    assert_eq!(world.storage.usage().leftovers.sandbox_temp.count, 0);
    assert!(folder.exists());
    assert_eq!(report.worktrees_removed, 0);
}
