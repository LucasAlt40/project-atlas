//! These tests run the real `git`: what is under test is how Atlas drives it, and a fake Git
//! would only prove that Atlas agrees with itself.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use super::service::Prepared;
use super::*;
use crate::application::config::memory::MemoryStore;
use crate::application::config::ConfigRepository;
use crate::application::security::testutil::TempDir;
use crate::application::security::SecurityService;
use crate::domain::agent::Agent;
use crate::domain::security::{Permission, SecurityPolicy};
use crate::domain::workspace::{Workspace, WorkspaceLayout};
use crate::domain::worktree::{
    BlockReason, ExecutionWorktree, MergeStatus, Recommendation, Validation, WorktreeStatus,
};
use crate::infrastructure::GitWorktreeManager;

pub const WORKSPACE: &str = "ws-1-1";
pub const AGENT: &str = "agent-1";

/// Runs `git` in `dir`, panicking if it fails, as a person would.
pub fn git(dir: &Path, args: &[&str]) -> String {
    let output = git_raw(dir, args);
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// `git status --porcelain`, line by line, with the leading space of each code kept.
pub fn status_lines(dir: &Path) -> Vec<String> {
    let output = git_raw(dir, &["status", "--porcelain"]);
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect()
}

pub fn git_raw(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "protocol.file.allow=always",
        ])
        .args(args)
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .expect("git runs")
}

/// A repository with one commit on `branch`.
pub fn init_repo(dir: &Path, branch: &str) {
    git(dir, &["init", "--quiet", "-b", branch]);
    fs::write(dir.join("README.md"), "# Project\n").unwrap();
    fs::write(dir.join("shared.txt"), "line 1\nline 2\nline 3\n").unwrap();
    git(dir, &["add", "--all"]);
    git(dir, &["commit", "--quiet", "-m", "initial"]);
}

pub struct Env {
    pub data: TempDir,
    pub project: TempDir,
    pub store: Arc<MemoryStore>,
    pub config: Arc<ConfigRepository>,
    pub layout: WorktreeLayout,
    pub manager: Arc<GitWorktreeManager>,
    pub service: Arc<WorktreeService>,
}

impl Env {
    /// A repository on `main` in a folder whose name has a space in it, and an app data folder
    /// likewise: neither may matter.
    pub fn new(git_write: Permission) -> Self {
        let project = TempDir::new("my project");
        init_repo(project.path(), "main");
        Self::over(project, git_write)
    }

    /// Whatever is in `project`, a workspace over it, and a developer agent.
    pub fn over(project: TempDir, git_write: Permission) -> Self {
        let data = TempDir::new("atlas data");
        let store = Arc::new(MemoryStore::default());
        let config = Arc::new(ConfigRepository::load(Box::new(store.clone())));
        let mut security = SecurityPolicy::developer();
        security.git.write = git_write;
        config
            .modify(|c| {
                c.workspaces.push(Workspace {
                    id: WORKSPACE.to_owned(),
                    name: "tests".to_owned(),
                    project_path: project.path().to_string_lossy().into_owned(),
                    description: None,
                    created_at: 1,
                    updated_at: 1,
                    layout: WorkspaceLayout {
                        rows: 2,
                        columns: 2,
                        agent_placements: vec![],
                    },
                    security,
                });
                for id in [AGENT, "agent-2", "agent-3", "agent-4"] {
                    c.agents.push(Agent {
                        id: id.to_owned(),
                        name: id.to_owned(),
                        personality_id: "architect".to_owned(),
                        runtime_id: "x".to_owned(),
                        model_id: "m".to_owned(),
                        instructions: String::new(),
                        permission_profile_id: Some("developer".to_owned()),
                        worktree_isolation: true,
                        result_contract: crate::domain::result_contract::ResultContract::default(),
                        created_at: 1,
                    });
                }
                Ok(())
            })
            .unwrap();
        let layout = WorktreeLayout::new(data.path().join("worktrees"));
        let manager = Arc::new(GitWorktreeManager::new("git".into(), layout.clone()));
        let service = Arc::new(WorktreeService::new(
            manager.clone(),
            layout.clone(),
            config.clone(),
            Arc::new(SecurityService::new(config.clone())),
        ));
        Self {
            data,
            project,
            store,
            config,
            layout,
            manager,
            service,
        }
    }

    pub fn prepare(&self, id: &str) -> Prepared {
        self.prepare_for(AGENT, id)
    }

    pub fn prepare_for(&self, agent: &str, id: &str) -> Prepared {
        self.service
            .prepare(WORKSPACE, agent, id, self.project.path())
            .unwrap()
    }

    /// The service as a restarted app would build it: same stored data, nothing in memory.
    pub fn restarted(&self) -> WorktreeService {
        let config = Arc::new(ConfigRepository::load(Box::new(self.store.clone())));
        WorktreeService::new(
            self.manager.clone(),
            self.layout.clone(),
            config.clone(),
            Arc::new(SecurityService::new(config)),
        )
    }

    pub fn finalize(&self, id: &str) -> ExecutionWorktree {
        self.service
            .finalize(id, RunOutcome::Completed, Validation::NotRun)
            .unwrap()
    }

    pub fn main_branch_head(&self) -> String {
        git(self.project.path(), &["rev-parse", "HEAD"])
    }
}

fn write(dir: &Path, name: &str, text: &str) {
    if let Some(parent) = Path::new(name).parent() {
        fs::create_dir_all(dir.join(parent)).unwrap();
    }
    fs::write(dir.join(name), text).unwrap();
}

// ---- creation -----------------------------------------------------------------------------

#[test]
fn an_execution_gets_its_own_branch_and_worktree_and_the_checkout_is_not_the_working_directory() {
    let env = Env::new(Permission::Allowed);

    let prepared = env.prepare("exec-42");

    assert_ne!(prepared.working_dir, env.project.path());
    assert!(prepared.working_dir.starts_with(env.layout.root()));
    assert!(prepared.working_dir.ends_with("exec-000042"));
    assert!(prepared.working_dir.join("README.md").is_file());
    let worktree = &prepared.worktree;
    assert_eq!(worktree.branch_name, "atlas/exec-000042");
    assert_eq!(worktree.base_branch, "main");
    assert_eq!(worktree.base_commit, env.main_branch_head());
    assert_eq!(worktree.status, WorktreeStatus::Active);
    assert_eq!(worktree.merge_status, MergeStatus::NotEvaluated);
    assert_eq!(
        git(
            &prepared.working_dir,
            &["rev-parse", "--abbrev-ref", "HEAD"]
        ),
        "atlas/exec-000042"
    );
    // The project's checkout is where it was.
    assert_eq!(
        git(env.project.path(), &["rev-parse", "--abbrev-ref", "HEAD"]),
        "main"
    );
    assert_eq!(git(env.project.path(), &["status", "--porcelain"]), "");
    // Nothing was added to the user's repository: no folder, no ignore rule.
    assert!(!env.project.path().join(".atlas").exists());
    assert_eq!(env.service.get("exec-42"), Some(worktree.clone()));
}

#[test]
fn executions_do_not_share_a_working_tree_even_for_the_same_agent() {
    let env = Env::new(Permission::Allowed);

    let a = env.prepare("exec-101");
    let b = env.prepare("exec-102");
    write(&a.working_dir, "only-in-a.txt", "a\n");
    write(&b.working_dir, "only-in-b.txt", "b\n");

    assert_ne!(a.working_dir, b.working_dir);
    assert_ne!(a.worktree.branch_name, b.worktree.branch_name);
    assert!(!b.working_dir.join("only-in-a.txt").exists());
    assert!(!a.working_dir.join("only-in-b.txt").exists());
    assert!(!env.project.path().join("only-in-a.txt").exists());
    assert!(!env.project.path().join("only-in-b.txt").exists());
}

#[test]
fn the_base_is_whatever_branch_is_checked_out_not_main() {
    for name in ["master", "release/1.0", "develop"] {
        let project = TempDir::new("custom base");
        init_repo(project.path(), name);
        let env = Env::over(project, Permission::Allowed);

        let prepared = env.prepare("exec-1");

        assert_eq!(prepared.worktree.base_branch, name);
    }
}

#[test]
fn a_feature_branch_checked_out_in_the_project_is_the_base() {
    let env = Env::new(Permission::Allowed);
    git(
        env.project.path(),
        &["checkout", "--quiet", "-b", "feature/login"],
    );
    write(env.project.path(), "login.txt", "x\n");
    git(env.project.path(), &["add", "--all"]);
    git(env.project.path(), &["commit", "--quiet", "-m", "login"]);

    let prepared = env.prepare("exec-1");

    assert_eq!(prepared.worktree.base_branch, "feature/login");
    assert!(prepared.working_dir.join("login.txt").is_file());
}

#[test]
fn a_project_that_is_not_a_git_repository_fails_with_nothing_created() {
    let project = TempDir::new("plain");
    write(project.path(), "file.txt", "x\n");
    let env = Env::over(project, Permission::Allowed);

    let result = env
        .service
        .prepare(WORKSPACE, AGENT, "exec-1", env.project.path());

    assert_eq!(result.unwrap_err(), WorktreeError::GitRepositoryRequired);
    assert_eq!(env.service.list(None, None), []);
    assert!(!env.layout.path_for(WORKSPACE, "exec-1").unwrap().exists());
    // Atlas did not run `git init` for the user.
    assert!(!env.project.path().join(".git").exists());
}

#[test]
fn a_detached_head_or_an_empty_repository_has_no_base_to_start_from() {
    let detached = TempDir::new("detached");
    init_repo(detached.path(), "main");
    git(detached.path(), &["checkout", "--quiet", "--detach"]);
    let env = Env::over(detached, Permission::Allowed);
    assert!(matches!(
        env.service
            .prepare(WORKSPACE, AGENT, "exec-1", env.project.path()),
        Err(WorktreeError::BaseUnavailable(_))
    ));

    let empty = TempDir::new("empty");
    git(empty.path(), &["init", "--quiet", "-b", "main"]);
    let env = Env::over(empty, Permission::Allowed);
    assert!(matches!(
        env.service
            .prepare(WORKSPACE, AGENT, "exec-1", env.project.path()),
        Err(WorktreeError::BaseUnavailable(_))
    ));
    assert_eq!(env.service.list(None, None), []);
}

#[test]
fn uncommitted_work_in_the_checkout_is_neither_touched_nor_given_to_the_agent() {
    let env = Env::new(Permission::Allowed);
    write(env.project.path(), "README.md", "# edited by the user\n");
    write(env.project.path(), "notes.txt", "untracked\n");

    let prepared = env.prepare("exec-1");

    assert!(prepared.worktree.base_dirty_at_start);
    assert_eq!(
        fs::read_to_string(env.project.path().join("README.md")).unwrap(),
        "# edited by the user\n"
    );
    assert_eq!(
        fs::read_to_string(env.project.path().join("notes.txt")).unwrap(),
        "untracked\n"
    );
    // The agent starts from what is committed.
    assert_eq!(
        fs::read_to_string(prepared.working_dir.join("README.md")).unwrap(),
        "# Project\n"
    );
    assert!(!prepared.working_dir.join("notes.txt").exists());
    // Their change is still pending: nothing was stashed, reset or cleaned.
    assert_eq!(
        git(env.project.path(), &["status", "--porcelain"]),
        "M README.md\n?? notes.txt"
    );
    assert_eq!(git(env.project.path(), &["stash", "list"]), "");
}

#[test]
fn a_project_folder_inside_a_repository_gets_the_same_folder_in_the_worktree() {
    let root = TempDir::new("monorepo");
    init_repo(root.path(), "main");
    write(root.path(), "services/api/main.rs", "fn main() {}\n");
    git(root.path(), &["add", "--all"]);
    git(root.path(), &["commit", "--quiet", "-m", "api"]);
    let env = Env::over(root, Permission::Allowed);
    let project = env.project.path().join("services/api");

    let prepared = env
        .service
        .prepare(WORKSPACE, AGENT, "exec-1", &project)
        .unwrap();

    assert!(prepared.working_dir.ends_with("exec-000001/services/api"));
    assert!(prepared.working_dir.join("main.rs").is_file());
    assert_eq!(
        prepared.worktree.worktree_path,
        env.layout
            .path_for(WORKSPACE, "exec-1")
            .unwrap()
            .to_string_lossy()
    );
}

#[test]
fn an_execution_is_never_given_a_worktree_twice() {
    let env = Env::new(Permission::Allowed);
    env.prepare("exec-1");

    let again = env
        .service
        .prepare(WORKSPACE, AGENT, "exec-1", env.project.path());

    assert!(matches!(again, Err(WorktreeError::Collision(_))));
    assert_eq!(env.service.list(None, None).len(), 1);
}

#[test]
fn atlas_runs_git_without_the_repositorys_hooks() {
    let env = Env::new(Permission::Allowed);
    let marker = env.data.path().join("hook-ran");
    let hooks = env.project.path().join(".git/hooks");
    for name in [
        "pre-commit",
        "post-commit",
        "post-checkout",
        "pre-merge-commit",
        "post-merge",
    ] {
        let hook = hooks.join(name);
        fs::write(
            &hook,
            format!("#!/bin/sh\necho {name} >> '{}'\n", marker.display()),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    let prepared = env.prepare("exec-1");
    write(&prepared.working_dir, "new.txt", "x\n");

    let done = env.finalize("exec-1");

    assert_eq!(done.merge_status, MergeStatus::Merged);
    assert!(!marker.exists(), "a repository hook ran");
}

// ---- finishing: merge, conflict, policy ---------------------------------------------------

#[test]
fn a_clean_execution_is_merged_and_cleaned_up_when_the_policy_allows_it() {
    let env = Env::new(Permission::Allowed);
    let prepared = env.prepare("exec-7");
    write(&prepared.working_dir, "feature.txt", "done\n");
    write(&prepared.working_dir, "src/deep/file.txt", "deep\n");
    let before = env.main_branch_head();

    let done = env.finalize("exec-7");

    assert_eq!(done.merge_status, MergeStatus::Merged);
    assert_eq!(done.status, WorktreeStatus::Cleaned);
    assert_eq!(done.block_reason, None);
    assert_eq!(done.changes.as_ref().unwrap().files_changed, 2);
    assert_eq!(done.changes.as_ref().unwrap().commits_ahead, 1);
    assert!(env.project.path().join("feature.txt").is_file());
    assert!(env.project.path().join("src/deep/file.txt").is_file());
    assert_ne!(env.main_branch_head(), before);
    // A merge commit on the base branch, naming the execution.
    assert!(git(env.project.path(), &["log", "-1", "--format=%s"]).contains("atlas/exec-000007"));
    assert_eq!(git(env.project.path(), &["status", "--porcelain"]), "");
    // The worktree and its (merged) branch are gone.
    assert!(!prepared.working_dir.exists());
    assert_eq!(
        git(env.project.path(), &["branch", "--list", "atlas/*"]),
        ""
    );
    assert_eq!(
        git(env.project.path(), &["worktree", "list", "--porcelain"])
            .matches("worktree ")
            .count(),
        1
    );
}

#[test]
fn a_conflict_keeps_branch_and_worktree_and_leaves_the_base_exactly_as_it_was() {
    let env = Env::new(Permission::Allowed);
    let prepared = env.prepare("exec-1");
    write(
        &prepared.working_dir,
        "shared.txt",
        "line 1\nagent change\nline 3\n",
    );
    // Meanwhile the user committed a different change to the same line.
    write(
        env.project.path(),
        "shared.txt",
        "line 1\nuser change\nline 3\n",
    );
    git(env.project.path(), &["commit", "--quiet", "-am", "user"]);
    let before = env.main_branch_head();

    let done = env.finalize("exec-1");

    assert_eq!(done.merge_status, MergeStatus::Conflict);
    assert_eq!(done.block_reason, Some(BlockReason::Conflict));
    assert_eq!(done.recommendation, Some(Recommendation::ResolveConflicts));
    assert_eq!(done.changes.as_ref().unwrap().conflicts, ["shared.txt"]);
    assert_eq!(done.status, WorktreeStatus::Completed);
    assert!(prepared.working_dir.is_dir());
    assert!(git_raw(
        env.project.path(),
        &["rev-parse", "--verify", "refs/heads/atlas/exec-000001"]
    )
    .status
    .success());
    assert_eq!(env.main_branch_head(), before);
    assert_eq!(git(env.project.path(), &["status", "--porcelain"]), "");
    assert!(!env.project.path().join(".git/MERGE_HEAD").exists());
    assert_eq!(
        fs::read_to_string(env.project.path().join("shared.txt")).unwrap(),
        "line 1\nuser change\nline 3\n"
    );
}

#[test]
fn a_diverged_base_that_still_merges_cleanly_is_merged() {
    let env = Env::new(Permission::Allowed);
    let prepared = env.prepare("exec-1");
    write(&prepared.working_dir, "agent.txt", "a\n");
    write(env.project.path(), "user.txt", "u\n");
    git(env.project.path(), &["add", "--all"]);
    git(env.project.path(), &["commit", "--quiet", "-m", "user"]);

    let done = env.finalize("exec-1");

    assert_eq!(done.merge_status, MergeStatus::Merged);
    assert!(env.project.path().join("agent.txt").is_file());
    assert!(env.project.path().join("user.txt").is_file());
}

#[test]
fn when_the_policy_asks_for_approval_nothing_is_merged_until_the_user_says_so() {
    let env = Env::new(Permission::ApprovalRequired);
    let prepared = env.prepare("exec-1");
    write(&prepared.working_dir, "feature.txt", "done\n");
    let before = env.main_branch_head();

    let done = env.finalize("exec-1");

    assert_eq!(done.merge_status, MergeStatus::Pending);
    assert_eq!(done.recommendation, Some(Recommendation::Merge));
    assert_eq!(done.status, WorktreeStatus::Completed);
    assert_eq!(env.main_branch_head(), before);
    assert!(!env.project.path().join("feature.txt").exists());
    assert!(prepared.working_dir.is_dir());

    let merged = env.service.merge("exec-1").unwrap();

    assert_eq!(merged.merge_status, MergeStatus::Merged);
    assert_eq!(merged.status, WorktreeStatus::Cleaned);
    assert!(env.project.path().join("feature.txt").is_file());
}

#[test]
fn a_policy_that_denies_git_writes_blocks_the_merge_even_when_the_user_asks() {
    let env = Env::new(Permission::Denied);
    let prepared = env.prepare("exec-1");
    write(&prepared.working_dir, "feature.txt", "done\n");
    let before = env.main_branch_head();

    let done = env.finalize("exec-1");

    assert_eq!(done.merge_status, MergeStatus::Blocked);
    assert_eq!(done.block_reason, Some(BlockReason::UncommittedChanges));
    assert_eq!(
        env.service.merge("exec-1").unwrap_err(),
        WorktreeError::PolicyDenied
    );
    assert_eq!(env.main_branch_head(), before);
    assert!(prepared.working_dir.join("feature.txt").is_file());
}

#[test]
fn work_already_committed_under_a_denying_policy_is_blocked_by_policy() {
    let env = Env::new(Permission::Denied);
    let prepared = env.prepare("exec-1");
    write(&prepared.working_dir, "feature.txt", "done\n");
    git(&prepared.working_dir, &["add", "--all"]);
    git(&prepared.working_dir, &["commit", "--quiet", "-m", "agent"]);

    let done = env.finalize("exec-1");

    assert_eq!(done.merge_status, MergeStatus::Blocked);
    assert_eq!(done.block_reason, Some(BlockReason::PolicyDenied));
    assert_eq!(
        env.service.merge("exec-1").unwrap_err(),
        WorktreeError::PolicyDenied
    );
    assert!(!env.project.path().join("feature.txt").exists());
}

#[test]
fn a_failed_or_cancelled_execution_is_never_merged_and_keeps_its_work() {
    let env = Env::new(Permission::Allowed);
    let prepared = env.prepare("exec-1");
    write(&prepared.working_dir, "half-done.txt", "wip\n");
    let before = env.main_branch_head();

    let done = env
        .service
        .finalize("exec-1", RunOutcome::Ended, Validation::NotRun)
        .unwrap();

    assert_eq!(done.status, WorktreeStatus::Failed);
    assert_eq!(done.merge_status, MergeStatus::Blocked);
    assert_eq!(done.block_reason, Some(BlockReason::ExecutionNotCompleted));
    assert_eq!(done.recommendation, Some(Recommendation::Inspect));
    assert_eq!(env.main_branch_head(), before);
    assert!(!env.project.path().join("half-done.txt").exists());
    // The partial work is saved on the branch, so it can be recovered.
    assert!(prepared.working_dir.join("half-done.txt").is_file());
    assert_eq!(
        git(
            env.project.path(),
            &["show", "atlas/exec-000001:half-done.txt"]
        ),
        "wip"
    );
    // And not even the user's "merge" applies to an execution that did not complete.
    assert!(matches!(
        env.service.merge("exec-1"),
        Err(WorktreeError::InvalidState(_))
    ));
}

#[test]
fn failing_validation_blocks_the_merge() {
    let env = Env::new(Permission::Allowed);
    let prepared = env.prepare("exec-1");
    write(&prepared.working_dir, "feature.txt", "done\n");

    let done = env
        .service
        .finalize("exec-1", RunOutcome::Completed, Validation::Failed)
        .unwrap();

    assert_eq!(done.merge_status, MergeStatus::Blocked);
    assert_eq!(done.block_reason, Some(BlockReason::ValidationFailed));
    assert_eq!(done.validation, Validation::Failed);
    assert!(!env.project.path().join("feature.txt").exists());
    assert!(prepared.working_dir.is_dir());
}

#[test]
fn an_execution_that_changed_nothing_leaves_nothing_behind() {
    let env = Env::new(Permission::Allowed);
    let prepared = env.prepare("exec-1");

    let done = env.finalize("exec-1");

    assert_eq!(done.merge_status, MergeStatus::NothingToMerge);
    assert_eq!(done.status, WorktreeStatus::Cleaned);
    assert!(!prepared.working_dir.exists());
    assert_eq!(
        git(env.project.path(), &["branch", "--list", "atlas/*"]),
        ""
    );
}

#[test]
fn a_checkout_with_uncommitted_changes_is_not_merged_into() {
    let env = Env::new(Permission::Allowed);
    let prepared = env.prepare("exec-1");
    write(&prepared.working_dir, "feature.txt", "done\n");
    write(env.project.path(), "README.md", "# user is editing\n");

    let done = env.finalize("exec-1");

    assert_eq!(done.merge_status, MergeStatus::Blocked);
    assert_eq!(done.block_reason, Some(BlockReason::BaseDirty));
    assert!(!env.project.path().join("feature.txt").exists());
    assert_eq!(
        fs::read_to_string(env.project.path().join("README.md")).unwrap(),
        "# user is editing\n"
    );
    // Once the user has dealt with their change, the merge can be asked for.
    git(env.project.path(), &["commit", "--quiet", "-am", "mine"]);
    let merged = env.service.merge("exec-1").unwrap();
    assert_eq!(merged.merge_status, MergeStatus::Merged);
}

#[test]
fn a_checkout_that_left_the_base_branch_is_not_merged_into() {
    let env = Env::new(Permission::Allowed);
    let prepared = env.prepare("exec-1");
    write(&prepared.working_dir, "feature.txt", "done\n");
    git(
        env.project.path(),
        &["checkout", "--quiet", "-b", "elsewhere"],
    );

    let done = env.finalize("exec-1");

    assert_eq!(done.merge_status, MergeStatus::Blocked);
    assert_eq!(done.block_reason, Some(BlockReason::BaseBranchChanged));
    assert!(!env.project.path().join("feature.txt").exists());
}

#[test]
fn a_merged_execution_cannot_be_merged_again_and_an_unknown_one_cannot_be_merged_at_all() {
    let env = Env::new(Permission::ApprovalRequired);
    let prepared = env.prepare("exec-1");
    write(&prepared.working_dir, "feature.txt", "done\n");
    env.finalize("exec-1");
    env.service.merge("exec-1").unwrap();

    assert!(matches!(
        env.service.merge("exec-1"),
        Err(WorktreeError::InvalidState(_))
    ));
    assert!(matches!(
        env.service.merge("exec-404"),
        Err(WorktreeError::NotFound(_))
    ));
}

#[test]
fn the_diff_is_read_from_git_and_bounded() {
    let env = Env::new(Permission::ApprovalRequired);
    let prepared = env.prepare("exec-1");
    write(&prepared.working_dir, "feature.txt", &"line\n".repeat(1000));
    let done = env.finalize("exec-1");

    let diff = env
        .manager
        .get_diff(
            Path::new(&done.repository_path),
            &done.base_branch,
            &done.branch_name,
            200,
        )
        .unwrap();

    assert!(diff.starts_with("diff --git a/feature.txt"));
    assert_eq!(diff.len(), 200);
    assert_eq!(
        env.manager.get_branch(&prepared.working_dir).unwrap(),
        "atlas/exec-000001"
    );
}

// ---- security -----------------------------------------------------------------------------

#[test]
fn nothing_but_an_atlas_branch_name_reaches_a_git_command() {
    let env = Env::new(Permission::Allowed);
    let base = env.manager.inspect_base(env.project.path()).unwrap();
    let path = env.layout.path_for(WORKSPACE, "exec-1").unwrap();

    for branch in [
        "--force",
        "-b",
        "main",
        "atlas/exec-1",
        "atlas/exec-000001; rm -rf /",
        "atlas/exec-000001 --orphan",
        "$(touch pwned)",
        "refs/heads/atlas/exec-000001",
        "",
    ] {
        let result = env.manager.create(&NewWorktree {
            toplevel: &base.toplevel,
            path: &path,
            branch,
            base_commit: &base.commit,
        });
        assert!(
            matches!(result, Err(WorktreeError::InvalidIdentifier(_))),
            "{branch:?}: {result:?}"
        );
        assert!(env
            .manager
            .merge(&base.toplevel, "main", branch, "m")
            .is_err());
        assert!(env.manager.assess(&base.toplevel, "main", branch).is_err());
        assert!(env
            .manager
            .delete_merged_branch(&base.toplevel, branch)
            .is_err());
    }
    for commit in [
        "--help",
        "HEAD",
        "main",
        &format!("{} --orphan", base.commit),
    ] {
        assert!(matches!(
            env.manager.create(&NewWorktree {
                toplevel: &base.toplevel,
                path: &path,
                branch: "atlas/exec-000001",
                base_commit: commit,
            }),
            Err(WorktreeError::InvalidIdentifier(_))
        ));
    }
    for base_branch in ["--abort", "-X", "no such branch", "a..b", "x~1", "x\ny"] {
        assert!(env
            .manager
            .assess(&base.toplevel, base_branch, "atlas/exec-000001")
            .is_err());
    }
    assert!(!env.data.path().join("pwned").exists());
    assert!(!path.exists());
    assert_eq!(git(env.project.path(), &["branch", "--list"]), "* main");
}

#[test]
fn a_worktree_can_only_be_made_or_removed_inside_the_worktree_folder() {
    let env = Env::new(Permission::Allowed);
    let base = env.manager.inspect_base(env.project.path()).unwrap();
    let outside = env.data.path().join("elsewhere");
    let traversal = env
        .layout
        .root()
        .join("ws-1")
        .join("..")
        .join("..")
        .join("escaped");
    let victim = TempDir::new("victim");
    fs::write(victim.path().join("precious.txt"), "keep\n").unwrap();

    for path in [&outside, &traversal, env.project.path(), victim.path()] {
        assert!(matches!(
            env.manager.create(&NewWorktree {
                toplevel: &base.toplevel,
                path,
                branch: "atlas/exec-000001",
                base_commit: &base.commit,
            }),
            Err(WorktreeError::OutsideRoot(_))
        ));
        assert!(matches!(
            env.manager.remove(&base.toplevel, path),
            Err(WorktreeError::OutsideRoot(_))
        ));
        assert!(matches!(
            env.manager.commit_all(path, "x"),
            Err(WorktreeError::OutsideRoot(_))
        ));
    }
    assert!(env.project.path().join("README.md").is_file());
    assert!(victim.path().join("precious.txt").is_file());
    assert!(!outside.exists());
}

#[test]
fn ids_that_are_not_generated_by_atlas_never_become_a_path_or_a_branch() {
    let env = Env::new(Permission::Allowed);

    for id in [
        "../escape",
        "exec-1/../../x",
        "exec-1; id",
        "",
        "main",
        "exec-",
    ] {
        assert!(matches!(
            env.service
                .prepare(WORKSPACE, AGENT, id, env.project.path()),
            Err(WorktreeError::InvalidIdentifier(_))
        ));
    }
    for workspace in ["../ws", "ws/1", "..", "", "-rf"] {
        assert!(matches!(
            env.service
                .prepare(workspace, AGENT, "exec-1", env.project.path()),
            Err(WorktreeError::InvalidIdentifier(_))
        ));
    }
    assert!(!env.layout.path_for(WORKSPACE, "exec-1").unwrap().exists());
}

#[test]
fn an_existing_folder_or_branch_is_never_overwritten_or_reused() {
    let env = Env::new(Permission::Allowed);
    let path = env.layout.path_for(WORKSPACE, "exec-1").unwrap();
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join("mine.txt"), "important\n").unwrap();

    let result = env
        .service
        .prepare(WORKSPACE, AGENT, "exec-1", env.project.path());

    assert!(matches!(result, Err(WorktreeError::Collision(_))));
    assert_eq!(
        fs::read_to_string(path.join("mine.txt")).unwrap(),
        "important\n"
    );
    // A failed attempt leaves no record that would make the folder look like Atlas's.
    assert_eq!(env.service.get("exec-1"), None);

    fs::remove_dir_all(&path).unwrap();
    git(env.project.path(), &["branch", "atlas/exec-000001"]);
    let result = env
        .service
        .prepare(WORKSPACE, AGENT, "exec-1", env.project.path());
    assert!(matches!(result, Err(WorktreeError::Collision(_))));
    assert!(!path.exists());
}

#[cfg(unix)]
#[test]
fn a_symlink_standing_in_for_a_worktree_is_not_followed_or_removed() {
    let env = Env::new(Permission::Allowed);
    let base = env.manager.inspect_base(env.project.path()).unwrap();
    let victim = TempDir::new("victim");
    fs::write(victim.path().join("precious.txt"), "keep\n").unwrap();
    let link = env.layout.path_for(WORKSPACE, "exec-1").unwrap();
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(victim.path(), &link).unwrap();

    assert!(!env.layout.is_worktree_of(&link, WORKSPACE, "exec-1"));
    assert!(matches!(
        env.manager.remove(&base.toplevel, &link),
        Err(WorktreeError::OutsideRoot(_))
    ));
    assert!(matches!(
        env.service
            .prepare(WORKSPACE, AGENT, "exec-1", env.project.path()),
        Err(WorktreeError::Collision(_))
    ));
    assert!(victim.path().join("precious.txt").is_file());
}

#[test]
fn a_record_that_points_outside_the_worktree_folder_cannot_make_atlas_remove_anything() {
    let env = Env::new(Permission::Allowed);
    env.prepare("exec-1");
    let victim = TempDir::new("victim");
    fs::write(victim.path().join("precious.txt"), "keep\n").unwrap();
    // A hand-edited config.json aims the record at somebody else's folder.
    env.config
        .modify(|c| {
            c.worktrees[0].worktree_path = victim.path().to_string_lossy().into_owned();
            c.worktrees[0].working_dir = victim.path().to_string_lossy().into_owned();
            Ok(())
        })
        .unwrap();

    let done = env.finalize("exec-1");

    assert!(victim.path().join("precious.txt").is_file());
    assert_ne!(done.merge_status, MergeStatus::Merged);
    assert_ne!(done.status, WorktreeStatus::Cleaned);
}

#[test]
fn removal_is_never_forced_so_a_worktree_with_untracked_files_stays() {
    let env = Env::new(Permission::Allowed);
    let prepared = env.prepare("exec-1");
    let base = env.manager.inspect_base(env.project.path()).unwrap();
    write(&prepared.working_dir, "untracked.txt", "x\n");

    let result = env.manager.remove(&base.toplevel, &prepared.working_dir);

    assert!(matches!(result, Err(WorktreeError::Git(_))));
    assert!(prepared.working_dir.join("untracked.txt").is_file());
}

// ---- isolation between workspaces and agents ---------------------------------------------

#[test]
fn worktrees_of_other_workspaces_and_agents_stay_apart() {
    let env = Env::new(Permission::Allowed);
    env.prepare_for(AGENT, "exec-1");
    env.prepare_for("agent-2", "exec-2");

    assert_eq!(env.service.list(Some(WORKSPACE), Some(AGENT)).len(), 1);
    assert_eq!(env.service.list(Some(WORKSPACE), Some("agent-2")).len(), 1);
    assert_eq!(env.service.list(Some("ws-other"), None), []);
    assert_eq!(env.service.list(None, None).len(), 2);
    let a = env.service.get("exec-1").unwrap();
    let b = env.service.get("exec-2").unwrap();
    assert_ne!(a.worktree_path, b.worktree_path);
    assert_eq!(
        (a.agent_id.as_str(), b.agent_id.as_str()),
        (AGENT, "agent-2")
    );
}

#[test]
fn four_executions_work_and_finish_at_the_same_time_in_the_same_project() {
    let env = Env::new(Permission::Allowed);
    let agents = [AGENT, "agent-2", "agent-3", "agent-4"];

    std::thread::scope(|scope| {
        let handles: Vec<_> = agents
            .iter()
            .enumerate()
            .map(|(n, agent)| {
                let env = &env;
                scope.spawn(move || {
                    let id = format!("exec-{}", n + 1);
                    let prepared = env.prepare_for(agent, &id);
                    write(&prepared.working_dir, &format!("from-{n}.txt"), "x\n");
                    let done = env.finalize(&id);
                    (prepared.working_dir, done)
                })
            })
            .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        let mut paths: Vec<_> = results.iter().map(|(p, _)| p.clone()).collect();
        paths.sort();
        paths.dedup();
        assert_eq!(paths.len(), 4);
        for (_, done) in &results {
            assert_eq!(done.merge_status, MergeStatus::Merged, "{done:?}");
            assert_eq!(done.status, WorktreeStatus::Cleaned);
        }
    });

    for n in 0..4 {
        assert!(env.project.path().join(format!("from-{n}.txt")).is_file());
    }
    assert_eq!(git(env.project.path(), &["status", "--porcelain"]), "");
    assert_eq!(
        git(env.project.path(), &["branch", "--list", "atlas/*"]),
        ""
    );
    assert_eq!(
        git(env.project.path(), &["worktree", "list", "--porcelain"])
            .matches("worktree ")
            .count(),
        1
    );
}

#[test]
fn four_concurrent_executions_that_collide_leave_one_merged_and_the_rest_kept() {
    let env = Env::new(Permission::Allowed);
    let agents = [AGENT, "agent-2", "agent-3", "agent-4"];
    let prepared: Vec<_> = agents
        .iter()
        .enumerate()
        .map(|(n, agent)| {
            let p = env.prepare_for(agent, &format!("exec-{}", n + 1));
            write(
                &p.working_dir,
                "shared.txt",
                &format!("line 1\nagent {n}\nline 3\n"),
            );
            p
        })
        .collect();

    let done: Vec<_> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|n| {
                let env = &env;
                scope.spawn(move || env.finalize(&format!("exec-{}", n + 1)))
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    let merged = done
        .iter()
        .filter(|d| d.merge_status == MergeStatus::Merged)
        .count();
    let conflicts: Vec<_> = done
        .iter()
        .filter(|d| d.merge_status == MergeStatus::Conflict)
        .collect();
    assert_eq!(merged, 1);
    assert_eq!(conflicts.len(), 3);
    for conflict in conflicts {
        assert!(Path::new(&conflict.worktree_path).is_dir());
    }
    assert_eq!(git(env.project.path(), &["status", "--porcelain"]), "");
    drop(prepared);
}

// ---- persistence and recovery ------------------------------------------------------------

#[test]
fn the_worktree_state_survives_a_restart() {
    let env = Env::new(Permission::ApprovalRequired);
    let prepared = env.prepare("exec-9");
    write(&prepared.working_dir, "feature.txt", "x\n");
    let done = env.finalize("exec-9");

    let restarted = env.restarted();

    assert_eq!(restarted.get("exec-9"), Some(done.clone()));
    assert_eq!(
        restarted.get("exec-9").unwrap().merge_status,
        MergeStatus::Pending
    );
    assert_eq!(restarted.next_execution_number(1), 10);
    // And the user can still decide, after the restart.
    let merged = restarted.merge("exec-9").unwrap();
    assert_eq!(merged.merge_status, MergeStatus::Merged);
}

#[test]
fn an_execution_cut_short_by_closing_atlas_keeps_its_worktree_and_is_marked_so() {
    let env = Env::new(Permission::Allowed);
    let prepared = env.prepare("exec-5");
    write(&prepared.working_dir, "wip.txt", "x\n");
    assert_eq!(
        env.service.get("exec-5").unwrap().status,
        WorktreeStatus::Active
    );

    let restarted = env.restarted();
    restarted.recover_interrupted();

    let recovered = restarted.get("exec-5").unwrap();
    assert_eq!(recovered.status, WorktreeStatus::Failed);
    assert_eq!(recovered.merge_status, MergeStatus::Blocked);
    assert_eq!(
        recovered.block_reason,
        Some(BlockReason::ExecutionNotCompleted)
    );
    assert!(prepared.working_dir.join("wip.txt").is_file());
    // It is not an execution that could be merged.
    assert!(restarted.merge("exec-5").is_err());
}

#[test]
fn what_is_stored_is_metadata_only() {
    let env = Env::new(Permission::ApprovalRequired);
    let prepared = env.prepare("exec-1");
    write(
        &prepared.working_dir,
        "big.txt",
        &"secret contents\n".repeat(5000),
    );
    env.finalize("exec-1");

    let stored = serde_json::to_string(&env.config.snapshot().worktrees).unwrap();

    assert!(!stored.contains("secret contents"));
    assert!(stored.len() < 2000, "{} bytes", stored.len());
    for key in [
        "executionId",
        "branchName",
        "baseBranch",
        "worktreePath",
        "status",
        "mergeStatus",
    ] {
        assert!(stored.contains(key), "{key}");
    }
}

#[test]
fn a_merged_worktree_whose_folder_could_not_be_removed_is_cleaned_later() {
    let env = Env::new(Permission::Allowed);
    let prepared = env.prepare("exec-2");
    write(&prepared.working_dir, "feature2.txt", "x\n");
    env.manager
        .commit_all(&prepared.working_dir, "work")
        .unwrap();
    git(
        env.project.path(),
        &["merge", "--quiet", "--no-edit", "atlas/exec-000002"],
    );
    // The merge happened; then something untracked appeared, so Git refuses to remove the folder.
    write(&prepared.working_dir, "late.txt", "x\n");
    env.config
        .modify(|c| {
            c.worktrees[0].status = WorktreeStatus::CleanupPending;
            c.worktrees[0].merge_status = MergeStatus::Merged;
            Ok(())
        })
        .unwrap();

    env.service.cleanup_pending();

    assert_eq!(
        env.service.get("exec-2").unwrap().status,
        WorktreeStatus::CleanupPending
    );
    assert!(prepared.working_dir.join("late.txt").is_file());

    fs::remove_file(prepared.working_dir.join("late.txt")).unwrap();
    env.service.cleanup_pending();

    assert_eq!(
        env.service.get("exec-2").unwrap().status,
        WorktreeStatus::Cleaned
    );
    assert!(!prepared.working_dir.exists());
}

// ---- the security layer ------------------------------------------------------------------

#[test]
fn the_guard_holds_an_isolated_execution_to_its_worktree_not_the_checkout() {
    use std::time::Duration;

    use crate::application::process::fake::{ok, FakeProcessRunner};
    use crate::application::process::{
        ExecutionScope, ProcessContext, ProcessError, ProcessRunner, ProcessSpec,
    };
    use crate::application::security::{ApprovalBroker, AuditLog, GuardedProcessRunner, NoSandbox};
    use crate::domain::security::{Reason, ToolAccess};

    let env = Env::new(Permission::Allowed);
    let prepared = env.prepare("exec-1");
    let guard = GuardedProcessRunner::new(
        Arc::new(FakeProcessRunner::new(&["git", "npm"], |_| ok("ran"))),
        Arc::new(SecurityService::new(env.config.clone())),
        Arc::new(ApprovalBroker::new()),
        Arc::new(AuditLog::default()),
        Arc::new(NoSandbox),
        vec!["claude".to_owned()],
    );
    let scope = |isolated: bool, agent: &str| ExecutionScope {
        workspace_id: WORKSPACE.to_owned(),
        agent_id: agent.to_owned(),
        execution_id: "exec-1".to_owned(),
        task_id: "t".to_owned(),
        runtime_access: ToolAccess {
            filesystem_write: true,
            process_execution: true,
            network: false,
        },
        isolated,
    };
    let request = |scope: ExecutionScope, cwd: &Path, args: &[&str]| ProcessSpec {
        program: "git".to_owned(),
        args: args.iter().map(|a| (*a).to_owned()).collect(),
        stdin: None,
        cwd: Some(cwd.to_path_buf()),
        env: Vec::new(),
        timeout: Duration::from_secs(5),
        context: ProcessContext::AgentRequested(scope),
        terminal: None,
    };
    let run = |spec: &ProcessSpec| guard.run(spec, &|_| {});
    let main = env.project.path();
    let isolated = scope(true, AGENT);

    // Inside its worktree: allowed.
    assert!(run(&request(
        isolated.clone(),
        &prepared.working_dir,
        &["status"]
    ))
    .is_ok());
    // The main checkout is outside what this execution may touch, as a working directory or as
    // something a command names (here, spelled as the path an agent could have been told).
    assert_eq!(
        run(&request(isolated.clone(), main, &["status"])),
        Err(ProcessError::PermissionDenied(Reason::OutsideProject))
    );
    let main_file = main.join("README.md");
    assert_eq!(
        run(&request(
            isolated.clone(),
            &prepared.working_dir,
            &["diff", main_file.to_str().unwrap()]
        )),
        Err(ProcessError::PermissionDenied(Reason::OutsideProject))
    );
    assert_eq!(
        run(&request(
            isolated.clone(),
            &prepared.working_dir,
            &["diff", "../../.."]
        )),
        Err(ProcessError::PermissionDenied(Reason::OutsideProject))
    );
    // Not isolated, the same execution would be held to the checkout instead.
    assert!(run(&request(scope(false, AGENT), main, &["status"])).is_ok());
    // An isolated scope for an agent that is not the one the worktree was made for: refused.
    assert_eq!(
        run(&request(
            scope(true, "agent-2"),
            &prepared.working_dir,
            &["status"]
        )),
        Err(ProcessError::PermissionDenied(Reason::UnknownScope))
    );
    // Once the execution is over its worktree is no longer a place a process may start in.
    env.finalize("exec-1");
    assert_eq!(
        run(&request(isolated, &prepared.working_dir, &["status"])),
        Err(ProcessError::PermissionDenied(Reason::UnknownScope))
    );
}

#[test]
fn a_worktree_whose_branch_was_switched_is_left_alone() {
    let env = Env::new(Permission::Allowed);
    let prepared = env.prepare("exec-1");
    git(
        &prepared.working_dir,
        &["checkout", "--quiet", "-b", "agent/own-branch"],
    );
    write(&prepared.working_dir, "feature.txt", "x\n");
    let before = env.main_branch_head();

    let done = env.finalize("exec-1");

    assert_eq!(done.merge_status, MergeStatus::Blocked);
    assert_eq!(done.block_reason, Some(BlockReason::WorktreeInconsistent));
    assert_eq!(done.recommendation, Some(Recommendation::Inspect));
    assert_eq!(env.main_branch_head(), before);
    // Nothing was committed on the agent's branch on its behalf, nothing was removed.
    assert!(prepared.working_dir.join("feature.txt").is_file());
    assert_eq!(
        git(&prepared.working_dir, &["status", "--porcelain"]),
        "?? feature.txt"
    );
    assert_ne!(done.status, WorktreeStatus::Cleaned);
}

#[test]
fn new_execution_ids_continue_after_every_worktree_still_around_even_without_history() {
    let env = Env::new(Permission::ApprovalRequired);
    let a = env.prepare("exec-7");
    write(&a.working_dir, "a.txt", "x\n");
    env.finalize("exec-7");

    // The conversation history was cleared (or trimmed): it says nothing about exec-7 any more.
    let restarted = env.restarted();

    assert_eq!(restarted.next_execution_number(1), 8);
    assert_eq!(restarted.next_execution_number(20), 20);
    // And a worktree-less app starts where the history says.
    assert_eq!(
        Env::new(Permission::Allowed)
            .service
            .next_execution_number(5),
        5
    );
}

// ---- a workflow's shared worktree ---------------------------------------------------------------

mod shared {
    #![allow(clippy::assert_is_empty)]
    use super::*;
    use crate::domain::worktree::FileChangeStatus;

    /// The primary worktree of a workflow run, as the orchestrator opens it.
    fn open(env: &Env, id: &str) -> Prepared {
        let prepared = env.prepare(id);
        env.service.mark_workflow(id, "wfx-1").unwrap();
        prepared
    }

    fn step(env: &Env, agent: &str, id: &str, primary: &str) -> Prepared {
        env.service.attach(WORKSPACE, agent, id, primary).unwrap()
    }

    #[test]
    fn a_later_step_sees_what_an_earlier_one_wrote_because_they_share_one_worktree() {
        let env = Env::new(Permission::Allowed);
        let primary = open(&env, "exec-1");

        let developer = step(&env, "agent-2", "exec-2", "exec-1");
        write(
            &developer.working_dir,
            "src/foo.ts",
            "export const x = 1;\n",
        );
        env.service
            .finish_step("exec-2", RunOutcome::Completed)
            .unwrap();

        let validator = step(&env, "agent-3", "exec-3", "exec-1");
        assert_eq!(validator.working_dir, developer.working_dir);
        assert_eq!(validator.working_dir, primary.working_dir);
        assert_eq!(
            fs::read_to_string(validator.working_dir.join("src/foo.ts")).unwrap(),
            "export const x = 1;\n",
            "the second agent reads the file the first one wrote"
        );
        env.service
            .finish_step("exec-3", RunOutcome::Completed)
            .unwrap();

        // The first step's delta names the file; the read-only step changed nothing.
        let first = env.service.step_delta("exec-2").unwrap();
        assert_eq!(first.files.len(), 1);
        assert_eq!(first.files[0].path, "src/foo.ts");
        assert_eq!(first.files[0].status, FileChangeStatus::Added);
        assert_eq!(first.files[0].additions, Some(1));
        assert!(first.uncommitted.is_empty());
        assert!(env.service.step_delta("exec-3").unwrap().files.is_empty());
        // And the project's own checkout has not been touched.
        assert!(!env.project.path().join("src/foo.ts").exists());
    }

    #[test]
    fn every_step_gets_a_lease_of_its_own_on_the_same_folder_and_branch() {
        let env = Env::new(Permission::Allowed);
        let primary = open(&env, "exec-1");

        let lease = step(&env, "agent-2", "exec-2", "exec-1").worktree;

        assert_eq!(lease.worktree_path, primary.worktree.worktree_path);
        assert_eq!(lease.branch_name, primary.worktree.branch_name);
        assert_eq!(lease.shared_with.as_deref(), Some("exec-1"));
        assert_eq!(lease.workflow_execution_id.as_deref(), Some("wfx-1"));
        assert_eq!(lease.status, WorktreeStatus::Active);
        // A lease is never merged, so a step cannot deliver work on its own.
        env.service
            .finish_step("exec-2", RunOutcome::Completed)
            .unwrap();
        assert!(matches!(
            env.service.merge("exec-2"),
            Err(WorktreeError::InvalidState(_))
        ));
        // Nor can it be attached to something that is not a workflow's worktree.
        let plain = env.prepare("exec-9");
        assert!(matches!(
            env.service.attach(
                WORKSPACE,
                "agent-2",
                "exec-10",
                &plain.worktree.execution_id
            ),
            Err(WorktreeError::InvalidState(_))
        ));
        assert!(matches!(
            env.service.attach(WORKSPACE, "agent-2", "exec-2", "exec-1"),
            Err(WorktreeError::Collision(_))
        ));
        assert!(matches!(
            env.service
                .attach("other-workspace", "agent-2", "exec-11", "exec-1"),
            Err(WorktreeError::InvalidState(_))
        ));
    }

    #[test]
    fn the_end_of_a_run_waits_for_the_user_even_when_the_policy_would_allow_a_merge() {
        let env = Env::new(Permission::Allowed);
        open(&env, "exec-1");
        let developer = step(&env, "agent-2", "exec-2", "exec-1");
        write(&developer.working_dir, "feature.txt", "done\n");
        env.service
            .finish_step("exec-2", RunOutcome::Completed)
            .unwrap();
        let before = env.main_branch_head();

        let closed = env
            .service
            .close_shared("exec-1", RunOutcome::Completed, Validation::NotRun)
            .unwrap();

        assert_eq!(closed.merge_status, MergeStatus::Pending);
        assert_eq!(closed.status, WorktreeStatus::Completed);
        assert_eq!(env.main_branch_head(), before, "nothing was merged");
        assert!(!env.project.path().join("feature.txt").exists());
        assert!(
            Path::new(&closed.worktree_path).is_dir(),
            "the worktree is kept"
        );

        // The user's explicit decision applies it.
        let applied = env.service.merge("exec-1").unwrap();
        assert_eq!(applied.merge_status, MergeStatus::Applied);
        assert_eq!(
            fs::read_to_string(env.project.path().join("feature.txt")).unwrap(),
            "done\n"
        );
        // ...and it is the working tree that has it, not the history.
        assert_eq!(env.main_branch_head(), before, "HEAD did not move");
        assert_eq!(
            git(env.project.path(), &["status", "--porcelain"]),
            "?? feature.txt"
        );
    }

    #[test]
    fn a_run_that_changed_nothing_leaves_nothing_behind() {
        let env = Env::new(Permission::Allowed);
        let primary = open(&env, "exec-1");
        let reader = step(&env, "agent-2", "exec-2", "exec-1");
        env.service
            .finish_step("exec-2", RunOutcome::Completed)
            .unwrap();

        let closed = env
            .service
            .close_shared("exec-1", RunOutcome::Completed, Validation::NotRun)
            .unwrap();

        assert_eq!(closed.merge_status, MergeStatus::NothingToMerge);
        assert_eq!(closed.status, WorktreeStatus::Cleaned);
        assert!(!primary.working_dir.exists());
        drop(reader);
    }

    #[test]
    fn a_cancelled_or_failed_run_keeps_its_work_and_can_never_be_applied() {
        let env = Env::new(Permission::Allowed);
        open(&env, "exec-1");
        let developer = step(&env, "agent-2", "exec-2", "exec-1");
        write(&developer.working_dir, "half.txt", "partial\n");
        env.service
            .finish_step("exec-2", RunOutcome::Ended)
            .unwrap();

        let closed = env
            .service
            .close_shared("exec-1", RunOutcome::Ended, Validation::NotRun)
            .unwrap();

        assert_eq!(closed.status, WorktreeStatus::Failed);
        assert_eq!(closed.merge_status, MergeStatus::Blocked);
        assert_eq!(
            closed.block_reason,
            Some(BlockReason::ExecutionNotCompleted)
        );
        assert!(Path::new(&closed.worktree_path).join("half.txt").is_file());
        assert!(matches!(
            env.service.merge("exec-1"),
            Err(WorktreeError::InvalidState(_))
        ));
        // It can still be looked at.
        let changes = env.service.change_set("exec-1").unwrap();
        assert_eq!(changes.files_changed, 1);
        assert!(env
            .service
            .diff("exec-1", None, 10_000)
            .unwrap()
            .contains("+partial"));
    }

    /// A finished workflow run that wrote `files` (name, text) in its worktree.
    fn finished_run(env: &Env, files: &[(&str, &str)]) {
        open(env, "exec-1");
        let developer = step(env, "agent-2", "exec-2", "exec-1");
        for (name, text) in files {
            write(&developer.working_dir, name, text);
        }
        env.service
            .finish_step("exec-2", RunOutcome::Completed)
            .unwrap();
        let closed = env
            .service
            .close_shared("exec-1", RunOutcome::Completed, Validation::NotRun)
            .unwrap();
        assert_eq!(closed.merge_status, MergeStatus::Pending);
    }

    /// Everything the project's repository holds that applying must not change.
    fn history(env: &Env) -> (String, String, String, String) {
        let project = env.project.path();
        (
            git(project, &["rev-parse", "HEAD"]),
            git(project, &["reflog", "show", "HEAD"]),
            git(
                project,
                &["for-each-ref", "--format=%(refname) %(objectname)"],
            ),
            git(project, &["diff", "--cached", "--name-only"]),
        )
    }

    #[test]
    fn applying_on_a_clean_checkout_modifies_the_working_tree_and_creates_nothing() {
        let env = Env::new(Permission::Allowed);
        // A bare remote: applying must never push to it.
        let remote = TempDir::new("remote");
        git(remote.path(), &["init", "--quiet", "--bare", "-b", "main"]);
        git(
            env.project.path(),
            &["remote", "add", "origin", &remote.path().to_string_lossy()],
        );
        finished_run(
            &env,
            &[("feature.txt", "new\n"), ("README.md", "# Project\nmore\n")],
        );
        let before = history(&env);
        let branches = git(env.project.path(), &["branch", "--format=%(refname:short)"]);

        let applied = env.service.merge("exec-1").unwrap();

        assert_eq!(applied.merge_status, MergeStatus::Applied);
        assert_eq!(applied.status, WorktreeStatus::Cleaned);
        // Working tree changed...
        assert_eq!(
            status_lines(env.project.path()),
            [" M README.md", "?? feature.txt"]
        );
        assert_eq!(
            fs::read_to_string(env.project.path().join("README.md")).unwrap(),
            "# Project\nmore\n"
        );
        // ...and nothing else: same HEAD, same reflog (no commit, merge, rebase or checkout),
        // same refs except the run's own branch, nothing staged, no merge in progress.
        let after = history(&env);
        assert_eq!(after.0, before.0, "HEAD unchanged");
        assert_eq!(after.1, before.1, "no commit, merge or rebase was recorded");
        assert_eq!(after.2, before.2, "no ref moved");
        assert_eq!(after.3, "", "nothing staged");
        assert!(!env.project.path().join(".git/MERGE_HEAD").exists());
        assert_eq!(
            git(env.project.path(), &["branch", "--format=%(refname:short)"]),
            branches
        );
        // The run's branch is kept as the record of what was applied.
        assert!(!git(env.project.path(), &["branch", "--list", "atlas/*"]).is_empty());
        // No push: the remote has no ref at all.
        assert_eq!(git(remote.path(), &["for-each-ref"]), "");
        // The user's `git add` and `git commit` are then theirs to do.
        git(env.project.path(), &["add", "--all"]);
        git(env.project.path(), &["commit", "--quiet", "-m", "mine"]);
        assert_eq!(git(env.project.path(), &["status", "--porcelain"]), "");
    }

    #[test]
    fn applying_carries_added_modified_and_deleted_files() {
        let env = Env::new(Permission::Allowed);
        open(&env, "exec-1");
        let developer = step(&env, "agent-2", "exec-2", "exec-1");
        write(&developer.working_dir, "new/dir/file.txt", "added\n");
        write(
            &developer.working_dir,
            "shared.txt",
            "line 1\nline 2 changed\nline 3\n",
        );
        fs::remove_file(developer.working_dir.join("README.md")).unwrap();
        env.service
            .finish_step("exec-2", RunOutcome::Completed)
            .unwrap();
        env.service
            .close_shared("exec-1", RunOutcome::Completed, Validation::NotRun)
            .unwrap();
        let head = env.main_branch_head();

        let applied = env.service.merge("exec-1").unwrap();

        assert_eq!(applied.merge_status, MergeStatus::Applied);
        let root = env.project.path();
        assert_eq!(
            fs::read_to_string(root.join("new/dir/file.txt")).unwrap(),
            "added\n"
        );
        assert!(!root.join("README.md").exists());
        assert_eq!(
            fs::read_to_string(root.join("shared.txt")).unwrap(),
            "line 1\nline 2 changed\nline 3\n"
        );
        assert_eq!(
            status_lines(root),
            [" D README.md", " M shared.txt", "?? new/"]
        );
        assert_eq!(env.main_branch_head(), head);
    }

    #[test]
    fn local_changes_in_other_files_do_not_block_applying_and_survive_it() {
        let env = Env::new(Permission::Allowed);
        finished_run(&env, &[("feature.txt", "x\n")]);
        write(
            env.project.path(),
            "README.md",
            "# edited by the user, not committed\n",
        );
        write(env.project.path(), "notes.txt", "untracked, the user's\n");
        let head = env.main_branch_head();

        let applied = env.service.merge("exec-1").unwrap();

        assert_eq!(applied.merge_status, MergeStatus::Applied);
        assert_eq!(
            fs::read_to_string(env.project.path().join("README.md")).unwrap(),
            "# edited by the user, not committed\n",
            "the user's work is untouched"
        );
        assert_eq!(
            fs::read_to_string(env.project.path().join("notes.txt")).unwrap(),
            "untracked, the user's\n"
        );
        assert_eq!(
            fs::read_to_string(env.project.path().join("feature.txt")).unwrap(),
            "x\n"
        );
        assert_eq!(env.main_branch_head(), head);
    }

    #[test]
    fn a_file_the_user_changed_and_the_run_changed_too_blocks_applying_and_writes_nothing() {
        let env = Env::new(Permission::Allowed);
        finished_run(
            &env,
            &[
                ("README.md", "# changed by the run\n"),
                ("other.txt", "from the run\n"),
            ],
        );
        write(env.project.path(), "README.md", "# changed by the user\n");
        let before = history(&env);

        let blocked = env.service.merge("exec-1").unwrap();

        assert_eq!(blocked.merge_status, MergeStatus::Conflict);
        assert_eq!(blocked.block_reason, Some(BlockReason::Conflict));
        assert_eq!(blocked.changes.as_ref().unwrap().conflicts, ["README.md"]);
        assert_eq!(
            fs::read_to_string(env.project.path().join("README.md")).unwrap(),
            "# changed by the user\n"
        );
        assert!(
            !env.project.path().join("other.txt").exists(),
            "all or nothing: not even the file that did not conflict was written"
        );
        assert_eq!(history(&env), before);
        // The worktree is kept, so it can be tried again once the user has decided.
        assert!(Path::new(&blocked.worktree_path).is_dir());
        git(
            env.project.path(),
            &["checkout", "--quiet", "--", "README.md"],
        );
        assert_eq!(
            env.service.merge("exec-1").unwrap().merge_status,
            MergeStatus::Applied
        );
    }

    #[test]
    fn an_untracked_file_the_run_also_created_is_a_conflict_not_an_overwrite() {
        let env = Env::new(Permission::Allowed);
        finished_run(&env, &[("feature.txt", "from the run\n")]);
        write(env.project.path(), "feature.txt", "the user's own\n");

        let blocked = env.service.merge("exec-1").unwrap();

        assert_eq!(blocked.merge_status, MergeStatus::Conflict);
        assert_eq!(
            fs::read_to_string(env.project.path().join("feature.txt")).unwrap(),
            "the user's own\n"
        );
    }

    #[test]
    fn applying_is_refused_when_the_checkout_has_left_the_base_branch() {
        let env = Env::new(Permission::Allowed);
        finished_run(&env, &[("feature.txt", "x\n")]);
        git(
            env.project.path(),
            &["checkout", "--quiet", "-b", "elsewhere"],
        );

        let blocked = env.service.merge("exec-1").unwrap();

        assert_eq!(blocked.merge_status, MergeStatus::Blocked);
        assert_eq!(blocked.block_reason, Some(BlockReason::BaseBranchChanged));
        assert!(!env.project.path().join("feature.txt").exists());
    }

    #[test]
    fn a_base_that_moved_on_still_receives_the_changes_without_a_new_commit() {
        let env = Env::new(Permission::Allowed);
        finished_run(&env, &[("feature.txt", "x\n")]);
        // The user committed something unrelated meanwhile.
        write(env.project.path(), "later.txt", "later\n");
        git(env.project.path(), &["add", "--all"]);
        git(env.project.path(), &["commit", "--quiet", "-m", "later"]);
        let head = env.main_branch_head();

        let applied = env.service.merge("exec-1").unwrap();

        assert_eq!(applied.merge_status, MergeStatus::Applied);
        assert_eq!(env.main_branch_head(), head);
        assert_eq!(
            git(env.project.path(), &["status", "--porcelain"]),
            "?? feature.txt",
            "only the run's change is pending, not what the base did meanwhile"
        );
    }

    // ---- taking applied changes back out ("keep isolated" after apply) --------------------------

    use crate::application::worktree::UnapplyOutcome;

    #[test]
    fn applied_changes_can_be_taken_back_out_leaving_the_project_as_it_was_and_the_work_isolated() {
        let env = Env::new(Permission::Allowed);
        finished_run(
            &env,
            &[("feature.txt", "new\n"), ("README.md", "# Project\nmore\n")],
        );
        let head = env.main_branch_head();
        let before = history(&env);
        env.service.merge("exec-1").unwrap();
        assert_eq!(status_lines(env.project.path()).len(), 2);

        let outcome = env.service.unapply("exec-1", &head).unwrap();

        let UnapplyOutcome::Reverted(worktree) = outcome else {
            panic!("expected the changes to be taken back, got {outcome:?}");
        };
        assert_eq!(worktree.merge_status, MergeStatus::Pending);
        assert_eq!(worktree.status, WorktreeStatus::Completed);
        assert_eq!(status_lines(env.project.path()), Vec::<String>::new());
        assert!(!env.project.path().join("feature.txt").exists());
        assert_eq!(
            fs::read_to_string(env.project.path().join("README.md")).unwrap(),
            "# Project\n"
        );
        assert_eq!(
            history(&env),
            before,
            "HEAD, reflog, refs, index: nothing moved"
        );
        // The work is isolated again: the folder is back, on its branch, with its changes.
        assert_eq!(
            fs::read_to_string(Path::new(&worktree.worktree_path).join("feature.txt")).unwrap(),
            "new\n"
        );
        // ...and can be applied again.
        assert_eq!(
            env.service.merge("exec-1").unwrap().merge_status,
            MergeStatus::Applied
        );
        assert!(env.project.path().join("feature.txt").is_file());
        assert_eq!(env.main_branch_head(), head);
    }

    #[test]
    fn taking_changes_back_leaves_the_users_own_changes_alone() {
        let env = Env::new(Permission::Allowed);
        finished_run(&env, &[("feature.txt", "new\n")]);
        let head = env.main_branch_head();
        write(env.project.path(), "README.md", "# my own edit\n");
        env.service.merge("exec-1").unwrap();
        write(env.project.path(), "notes.txt", "added after applying\n");

        let outcome = env.service.unapply("exec-1", &head).unwrap();

        assert!(matches!(outcome, UnapplyOutcome::Reverted(_)));
        assert!(!env.project.path().join("feature.txt").exists());
        assert_eq!(
            fs::read_to_string(env.project.path().join("README.md")).unwrap(),
            "# my own edit\n"
        );
        assert_eq!(
            fs::read_to_string(env.project.path().join("notes.txt")).unwrap(),
            "added after applying\n"
        );
    }

    #[test]
    fn an_applied_file_the_user_changed_since_blocks_taking_back_and_nothing_is_touched() {
        let env = Env::new(Permission::Allowed);
        finished_run(
            &env,
            &[("feature.txt", "new\n"), ("other.txt", "from the run\n")],
        );
        let head = env.main_branch_head();
        env.service.merge("exec-1").unwrap();
        write(
            env.project.path(),
            "feature.txt",
            "new\nand the user kept typing\n",
        );
        let status = status_lines(env.project.path());

        let outcome = env.service.unapply("exec-1", &head).unwrap();

        assert_eq!(
            outcome,
            UnapplyOutcome::Conflict(vec!["feature.txt".to_owned()])
        );
        assert_eq!(
            fs::read_to_string(env.project.path().join("feature.txt")).unwrap(),
            "new\nand the user kept typing\n"
        );
        assert!(
            env.project.path().join("other.txt").is_file(),
            "all or nothing: the other applied file stays too"
        );
        assert_eq!(status_lines(env.project.path()), status);
        let record = env.service.get("exec-1").unwrap();
        assert_eq!(record.merge_status, MergeStatus::Applied);
        assert!(
            !Path::new(&record.worktree_path).exists(),
            "the folder that was made to try is taken away again"
        );
    }

    #[test]
    fn once_the_project_has_a_new_commit_the_applied_changes_are_not_taken_back() {
        let env = Env::new(Permission::Allowed);
        finished_run(&env, &[("feature.txt", "new\n")]);
        let applied_at = env.main_branch_head();
        env.service.merge("exec-1").unwrap();
        // The user commits the applied changes (and so they are part of the history now).
        git(env.project.path(), &["add", "--all"]);
        git(env.project.path(), &["commit", "--quiet", "-m", "mine"]);
        let head = env.main_branch_head();

        let outcome = env.service.unapply("exec-1", &applied_at).unwrap();

        assert_eq!(outcome, UnapplyOutcome::ProjectMoved);
        assert_eq!(env.main_branch_head(), head);
        assert!(env.project.path().join("feature.txt").is_file());
        assert_eq!(status_lines(env.project.path()), Vec::<String>::new());
    }

    #[test]
    fn only_changes_that_were_applied_can_be_taken_back() {
        let env = Env::new(Permission::Allowed);
        finished_run(&env, &[("feature.txt", "new\n")]);

        assert!(matches!(
            env.service.unapply("exec-1", &env.main_branch_head()),
            Err(WorktreeError::InvalidState(_))
        ));
        assert!(!env.project.path().join("feature.txt").exists());
    }

    #[test]
    fn a_conflict_is_reported_and_the_checkout_is_left_as_it_was() {
        let env = Env::new(Permission::Allowed);
        open(&env, "exec-1");
        let developer = step(&env, "agent-2", "exec-2", "exec-1");
        write(
            &developer.working_dir,
            "shared.txt",
            "line 1\nCHANGED BY WORKFLOW\nline 3\n",
        );
        env.service
            .finish_step("exec-2", RunOutcome::Completed)
            .unwrap();
        // Meanwhile the user committed a different change to the same line.
        write(
            env.project.path(),
            "shared.txt",
            "line 1\nCHANGED BY USER\nline 3\n",
        );
        git(
            env.project.path(),
            &["commit", "--quiet", "-am", "user edit"],
        );
        let head = env.main_branch_head();

        let closed = env
            .service
            .close_shared("exec-1", RunOutcome::Completed, Validation::NotRun)
            .unwrap();

        assert_eq!(closed.merge_status, MergeStatus::Conflict);
        assert_eq!(closed.changes.as_ref().unwrap().conflicts, ["shared.txt"]);
        assert_eq!(env.main_branch_head(), head);
        assert!(git(env.project.path(), &["status", "--porcelain"]).is_empty());
        // Applying says the same and does not force anything.
        assert_eq!(
            env.service.merge("exec-1").unwrap().merge_status,
            MergeStatus::Conflict
        );
    }

    #[test]
    fn a_policy_that_denies_the_agents_git_writes_still_lets_the_user_apply_to_the_working_tree() {
        let env = Env::new(Permission::Denied);
        open(&env, "exec-1");
        let developer = step(&env, "agent-2", "exec-2", "exec-1");
        write(&developer.working_dir, "wip.txt", "x\n");
        env.service
            .finish_step("exec-2", RunOutcome::Completed)
            .unwrap();
        let head = env.main_branch_head();

        let closed = env
            .service
            .close_shared("exec-1", RunOutcome::Completed, Validation::NotRun)
            .unwrap();

        // The work is saved on the run's own isolated branch and waits for the user.
        assert_eq!(closed.merge_status, MergeStatus::Pending);
        let applied = env.service.merge("exec-1").unwrap();
        assert_eq!(applied.merge_status, MergeStatus::Applied);
        assert_eq!(
            fs::read_to_string(env.project.path().join("wip.txt")).unwrap(),
            "x\n"
        );
        assert_eq!(env.main_branch_head(), head);
        assert_eq!(status_lines(env.project.path()), ["?? wip.txt"]);
    }

    #[test]
    fn the_change_set_comes_from_git_with_renames_deletions_and_counts() {
        let env = Env::new(Permission::Allowed);
        open(&env, "exec-1");
        let developer = step(&env, "agent-2", "exec-2", "exec-1");
        let dir = &developer.working_dir;
        fs::rename(dir.join("README.md"), dir.join("docs.md")).unwrap();
        fs::remove_file(dir.join("shared.txt")).unwrap();
        write(dir, "src/new.ts", "a\nb\nc\n");
        fs::write(dir.join("logo.bin"), [0u8, 159, 146, 150, 0, 1, 2]).unwrap();
        env.service
            .finish_step("exec-2", RunOutcome::Completed)
            .unwrap();

        let set = env.service.change_set("exec-1").unwrap();

        let by = |path: &str| set.files.iter().find(|f| f.path == path).unwrap();
        assert_eq!(set.files_changed, 4);
        assert_eq!(by("docs.md").status, FileChangeStatus::Renamed);
        assert_eq!(by("docs.md").old_path.as_deref(), Some("README.md"));
        assert_eq!(by("shared.txt").status, FileChangeStatus::Deleted);
        assert_eq!(by("shared.txt").deletions, Some(3));
        assert_eq!(by("src/new.ts").status, FileChangeStatus::Added);
        assert_eq!(by("src/new.ts").additions, Some(3));
        assert!(by("logo.bin").binary);
        assert_eq!((set.additions, set.deletions), (3, 3));
        assert_eq!(set.base_revision, env.main_branch_head());
        assert_ne!(set.current_revision, set.base_revision);
        assert!(set.uncommitted.is_empty());
    }

    #[test]
    fn only_plain_relative_paths_can_be_asked_for_a_diff() {
        let env = Env::new(Permission::Allowed);
        open(&env, "exec-1");
        let developer = step(&env, "agent-2", "exec-2", "exec-1");
        write(&developer.working_dir, "a.txt", "x\n");
        env.service
            .finish_step("exec-2", RunOutcome::Completed)
            .unwrap();

        assert!(env
            .service
            .diff("exec-1", Some("a.txt"), 1000)
            .unwrap()
            .contains("+x"));
        for bad in [
            "../outside",
            "/etc/passwd",
            "--output=/tmp/pwned",
            "-p",
            ":(top)a.txt",
            "a/../../b",
            "*.txt",
            "a\nb",
            "",
        ] {
            assert!(
                env.service.diff("exec-1", Some(bad), 1000).is_err(),
                "{bad:?} must be refused"
            );
        }
        assert!(!Path::new("/tmp/pwned").exists());
    }

    #[test]
    fn a_workflow_worktree_survives_a_restart_and_can_be_picked_up_again() {
        let env = Env::new(Permission::Allowed);
        let primary = open(&env, "exec-1");
        let developer = step(&env, "agent-2", "exec-2", "exec-1");
        write(&developer.working_dir, "kept.txt", "work in progress\n");

        // The app closes with the run going: the worktree is kept, recorded as cut short.
        let restarted = env.restarted();
        restarted.recover_interrupted();
        assert_eq!(
            restarted.get("exec-1").unwrap().status,
            WorktreeStatus::Failed
        );
        assert!(primary.working_dir.join("kept.txt").is_file());
        assert_eq!(
            restarted.get("exec-2").unwrap().status,
            WorktreeStatus::Failed
        );

        // Resuming the run reuses it: no second worktree.
        let reopened = restarted.reopen("exec-1").unwrap();
        assert_eq!(reopened.status, WorktreeStatus::Active);
        assert_eq!(reopened.worktree_path, primary.worktree.worktree_path);
        let again = restarted
            .attach(WORKSPACE, "agent-2", "exec-3", "exec-1")
            .unwrap();
        assert!(again.working_dir.join("kept.txt").is_file());
        assert_eq!(restarted.list(None, None).len(), 3);
    }

    #[test]
    fn reopening_refuses_a_worktree_that_is_not_the_workflows_own() {
        let env = Env::new(Permission::Allowed);
        env.prepare("exec-1"); // not marked as a workflow's
        env.restarted().recover_interrupted();
        assert!(matches!(
            env.restarted().reopen("exec-1"),
            Err(WorktreeError::InvalidState(_))
        ));
    }

    #[test]
    fn discarding_removes_the_folder_but_keeps_the_branch_so_nothing_is_lost() {
        let env = Env::new(Permission::Allowed);
        open(&env, "exec-1");
        let developer = step(&env, "agent-2", "exec-2", "exec-1");
        write(&developer.working_dir, "feature.txt", "x\n");
        env.service
            .finish_step("exec-2", RunOutcome::Completed)
            .unwrap();
        let closed = env
            .service
            .close_shared("exec-1", RunOutcome::Completed, Validation::NotRun)
            .unwrap();

        let discarded = env.service.discard_shared("exec-1").unwrap();

        assert_eq!(discarded.status, WorktreeStatus::Cleaned);
        assert!(!Path::new(&closed.worktree_path).exists());
        let branches = git(env.project.path(), &["branch", "--list", "atlas/*"]);
        assert!(branches.contains("atlas/exec-000001"), "{branches}");
        assert!(env.service.editor_folder("exec-1").is_none());
        assert!(!env.project.path().join("feature.txt").exists());
        // And a worktree that is still in use, or a plain execution's, cannot be discarded.
        open(&env, "exec-5");
        assert!(env.service.discard_shared("exec-5").is_err());
        env.prepare("exec-7");
        assert!(env.service.discard_shared("exec-7").is_err());
    }

    #[test]
    fn the_editor_folder_is_only_ever_a_folder_atlas_made() {
        let env = Env::new(Permission::Allowed);
        let primary = open(&env, "exec-1");
        assert_eq!(
            env.service.editor_folder("exec-1"),
            Some(primary.working_dir)
        );
        assert!(env.service.editor_folder("exec-404").is_none());
    }
}
