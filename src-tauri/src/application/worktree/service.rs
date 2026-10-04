use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::{
    Assessment, BaseReadiness, MergeOutcome, NewWorktree, WorktreeError, WorktreeLayout,
    WorktreeManager,
};
use crate::application::config::ConfigRepository;
use crate::application::process::ExecutionScope;
use crate::application::security::PolicyResolver;
use crate::application::support::now_ms;
use crate::domain::execution::ExecutionStatus;
use crate::domain::security::{Permission, ToolAccess};
use crate::domain::worktree::{
    BlockReason, ChangeSet, ExecutionWorktree, FileChange, MergeStatus, Recommendation, Validation,
    WorktreeChanges, WorktreeStatus, MAX_CHANGESET_FILES,
};

/// A worktree ready for a runtime.
#[derive(Debug, Clone)]
pub struct Prepared {
    /// What the runtime is given as its working directory.
    pub working_dir: PathBuf,
    pub worktree: ExecutionWorktree,
}

/// Where the execution stands when its runtime returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOutcome {
    Completed,
    /// Failed or cancelled: the work is kept, never merged.
    Ended,
}

impl From<ExecutionStatus> for RunOutcome {
    fn from(status: ExecutionStatus) -> Self {
        if status == ExecutionStatus::Completed {
            Self::Completed
        } else {
            Self::Ended
        }
    }
}

/// What a step changed in a workflow's worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepDelta {
    /// Where the worktree was when the step began.
    pub from: String,
    /// Where it was when the step ended.
    pub to: Option<String>,
    pub files: Vec<FileChange>,
    /// Still not in a commit.
    pub uncommitted: Vec<String>,
}

/// What to do with the work once it has been measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Decision {
    NothingToMerge,
    Blocked(BlockReason, Recommendation),
    Conflict,
    /// Mergeable, but only the user may do it.
    AwaitApproval(Recommendation),
    Merge,
}

/// Everything the decision is based on. Plain facts, so the rules can be read (and tested) in
/// one place.
#[allow(clippy::struct_excessive_bools)] // plain facts, read in one place
struct Facts<'a> {
    outcome: RunOutcome,
    validation: Validation,
    git_write: Permission,
    /// The user is explicitly asking for the merge: that answers "approval required".
    approved_by_user: bool,
    /// Atlas may merge on its own when the policy allows it. A workflow's worktree never is:
    /// the user decides what enters the project.
    automatic: bool,
    /// The worktree is the folder Atlas made for this execution and is on its branch.
    consistent: bool,
    uncommitted: bool,
    assessment: Option<&'a Assessment>,
    readiness: Option<BaseReadiness>,
}

fn decide(facts: &Facts<'_>) -> Decision {
    // Whatever else is true, a worktree that is not what Atlas made is looked at, not acted on.
    if !facts.consistent {
        return Decision::Blocked(BlockReason::WorktreeInconsistent, Recommendation::Inspect);
    }
    let Some(assessment) = facts.assessment else {
        return Decision::Blocked(BlockReason::Undetermined, Recommendation::Inspect);
    };
    let nothing = assessment.files_changed == 0 && assessment.commits_ahead == 0;
    if nothing && !facts.uncommitted {
        return Decision::NothingToMerge;
    }
    if facts.outcome != RunOutcome::Completed {
        return Decision::Blocked(BlockReason::ExecutionNotCompleted, Recommendation::Inspect);
    }
    if facts.uncommitted {
        return Decision::Blocked(BlockReason::UncommittedChanges, Recommendation::Inspect);
    }
    if facts.validation == Validation::Failed {
        return Decision::Blocked(BlockReason::ValidationFailed, Recommendation::Review);
    }
    match assessment.mergeable {
        Some(false) => return Decision::Conflict,
        None => return Decision::Blocked(BlockReason::Undetermined, Recommendation::Review),
        Some(true) => {}
    }
    if facts.git_write == Permission::Denied {
        return Decision::Blocked(BlockReason::PolicyDenied, Recommendation::Review);
    }
    match facts.readiness {
        Some(BaseReadiness::Ready) => {}
        Some(BaseReadiness::Dirty) => {
            return Decision::Blocked(BlockReason::BaseDirty, Recommendation::Review)
        }
        Some(BaseReadiness::BranchChanged) => {
            return Decision::Blocked(BlockReason::BaseBranchChanged, Recommendation::Review)
        }
        None => return Decision::Blocked(BlockReason::Undetermined, Recommendation::Review),
    }
    if (facts.git_write == Permission::ApprovalRequired || !facts.automatic)
        && !facts.approved_by_user
    {
        let recommendation = if assessment.commits_behind > 0 {
            Recommendation::Review
        } else {
            Recommendation::Merge
        };
        return Decision::AwaitApproval(recommendation);
    }
    Decision::Merge
}

/// Use case: the life of an execution's worktree. See the module documentation.
pub struct WorktreeService {
    manager: Arc<dyn WorktreeManager>,
    layout: WorktreeLayout,
    config: Arc<ConfigRepository>,
    /// Where the agent's Git permission is read from: the same policy that bounds the agent's
    /// own commands.
    policies: Arc<dyn PolicyResolver>,
    /// Executions with a merge or cleanup in progress, so the same worktree is never worked on
    /// twice at once.
    busy: Mutex<HashSet<String>>,
}

struct BusyGuard<'a> {
    service: &'a WorktreeService,
    id: String,
}

impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut busy) = self.service.busy.lock() {
            busy.remove(&self.id);
        }
    }
}

impl WorktreeService {
    pub fn new(
        manager: Arc<dyn WorktreeManager>,
        layout: WorktreeLayout,
        config: Arc<ConfigRepository>,
        policies: Arc<dyn PolicyResolver>,
    ) -> Self {
        Self {
            manager,
            layout,
            config,
            policies,
            busy: Mutex::default(),
        }
    }

    fn begin(&self, execution_id: &str) -> Result<BusyGuard<'_>, WorktreeError> {
        let mut busy = self.busy.lock().expect("worktree busy lock poisoned");
        if !busy.insert(execution_id.to_owned()) {
            return Err(WorktreeError::Busy);
        }
        Ok(BusyGuard {
            service: self,
            id: execution_id.to_owned(),
        })
    }

    /// The worktree of an execution, if it ran isolated.
    pub fn get(&self, execution_id: &str) -> Option<ExecutionWorktree> {
        self.config.read(|config| {
            config
                .worktrees
                .iter()
                .find(|w| w.execution_id == execution_id)
                .cloned()
        })
    }

    /// Worktrees, oldest first, optionally narrowed to a workspace and/or an agent.
    pub fn list(
        &self,
        workspace_id: Option<&str>,
        agent_id: Option<&str>,
    ) -> Vec<ExecutionWorktree> {
        self.config.read(|config| {
            config
                .worktrees
                .iter()
                .filter(|w| workspace_id.is_none_or(|id| w.workspace_id == id))
                .filter(|w| agent_id.is_none_or(|id| w.agent_id == id))
                .cloned()
                .collect()
        })
    }

    /// The number new executions start from: after everything the history knows (`from_history`)
    /// and after every worktree still around, so a branch or a folder is never reused.
    pub fn next_execution_number(&self, from_history: u64) -> u64 {
        self.highest_execution_number()
            .map_or(from_history, |highest| from_history.max(highest + 1))
    }

    /// The highest execution number that has a worktree.
    fn highest_execution_number(&self) -> Option<u64> {
        self.config.read(|config| {
            config
                .worktrees
                .iter()
                .filter_map(|w| WorktreeLayout::execution_number(&w.execution_id).ok())
                .max()
        })
    }

    fn update<T>(
        &self,
        execution_id: &str,
        change: impl FnOnce(&mut ExecutionWorktree) -> T,
    ) -> Result<T, WorktreeError> {
        Ok(self.config.modify(|config| {
            let worktree = config
                .worktrees
                .iter_mut()
                .find(|w| w.execution_id == execution_id)
                .ok_or_else(|| {
                    crate::application::errors::AppError::new(
                        crate::application::errors::ErrorCode::ExecutionNotFound,
                    )
                })?;
            Ok(change(worktree))
        })?)
    }

    /// Creates the branch and worktree of an execution, before its runtime starts. The record
    /// is saved first, so a crash at any point leaves something that can be found.
    ///
    /// # Errors
    ///
    /// [`WorktreeError::GitRepositoryRequired`] if the project is not a Git repository (there is
    /// no fallback to the checkout); no base branch; ids that Atlas did not generate; an
    /// existing branch or folder; Git failing.
    pub fn prepare(
        &self,
        workspace_id: &str,
        agent_id: &str,
        execution_id: &str,
        project: &Path,
    ) -> Result<Prepared, WorktreeError> {
        let branch = WorktreeLayout::branch_name(execution_id)?;
        let path = self.layout.path_for(workspace_id, execution_id)?;
        if self.get(execution_id).is_some() {
            return Err(WorktreeError::Collision(execution_id.to_owned()));
        }
        let base = self.manager.inspect_base(project)?;
        let working_dir = path.join(&base.subdir);
        let record = ExecutionWorktree {
            execution_id: execution_id.to_owned(),
            workspace_id: workspace_id.to_owned(),
            agent_id: agent_id.to_owned(),
            base_branch: base.branch.clone(),
            base_commit: base.commit.clone(),
            branch_name: branch.clone(),
            worktree_path: path.to_string_lossy().into_owned(),
            working_dir: working_dir.to_string_lossy().into_owned(),
            repository_path: base.toplevel.to_string_lossy().into_owned(),
            status: WorktreeStatus::Creating,
            merge_status: MergeStatus::NotEvaluated,
            block_reason: None,
            base_dirty_at_start: base.dirty,
            created_at: now_ms(),
            changes: None,
            validation: Validation::NotRun,
            recommendation: None,
            workflow_execution_id: None,
            shared_with: None,
            end_commit: None,
        };
        self.config.modify(|config| {
            config.worktrees.push(record.clone());
            Ok(())
        })?;

        let created = self.manager.create(&NewWorktree {
            toplevel: &base.toplevel,
            path: &path,
            branch: &branch,
            base_commit: &base.commit,
        });
        let created = created.and_then(|()| {
            // A project folder that exists on disk but not in the commit is not in the worktree.
            if working_dir.is_dir() {
                Ok(())
            } else {
                let _ = self.manager.remove(&base.toplevel, &path);
                let _ = self.manager.delete_merged_branch(&base.toplevel, &branch);
                Err(WorktreeError::BaseUnavailable(
                    "the project folder is not part of the repository's history".to_owned(),
                ))
            }
        });
        if let Err(error) = created {
            // Nothing was made (Git cleans up after itself), so there is nothing to keep.
            let _ = self.config.modify(|config| {
                config.worktrees.retain(|w| w.execution_id != execution_id);
                Ok(())
            });
            return Err(error);
        }
        let worktree = self.update(execution_id, |w| {
            w.status = WorktreeStatus::Active;
            w.clone()
        })?;
        Ok(Prepared {
            working_dir,
            worktree,
        })
    }

    /// The Git permission the agent's policy gives. Anything unknown is `Denied`.
    fn git_write(&self, worktree: &ExecutionWorktree) -> Permission {
        self.git_write_of(
            &worktree.workspace_id,
            &worktree.agent_id,
            &worktree.execution_id,
        )
    }

    fn git_write_of(&self, workspace_id: &str, agent_id: &str, execution_id: &str) -> Permission {
        let scope = ExecutionScope {
            workspace_id: workspace_id.to_owned(),
            agent_id: agent_id.to_owned(),
            execution_id: execution_id.to_owned(),
            task_id: String::new(),
            runtime_access: ToolAccess::NONE,
            isolated: false,
        };
        self.policies
            .resolve(&scope)
            .map_or(Permission::Denied, |resolved| {
                resolved.agent_policy.git.write
            })
    }

    /// Looks at the work when the runtime has returned and decides what happens to it: nothing
    /// to merge (and the worktree goes), merge now (policy allows it), merge when the user says
    /// so, or keep it with an explanation. Never fails: whatever Git says, the record ends
    /// up describing what is on disk. A failed or cancelled execution is never merged.
    pub fn finalize(
        &self,
        execution_id: &str,
        outcome: RunOutcome,
        validation: Validation,
    ) -> Option<ExecutionWorktree> {
        self.finalize_with(execution_id, outcome, validation, true)
    }

    /// [`Self::finalize`], saying whether Atlas may merge on its own.
    pub fn finalize_with(
        &self,
        execution_id: &str,
        outcome: RunOutcome,
        validation: Validation,
        automatic: bool,
    ) -> Option<ExecutionWorktree> {
        let _busy = self.begin(execution_id).ok()?;
        let worktree = self.get(execution_id)?;
        let evaluated = self.evaluate(&worktree, outcome, validation, false, automatic);
        self.apply(execution_id, evaluated, outcome).ok()
    }

    /// The user's explicit "merge it". Counts as the approval a policy may ask for; it does
    /// not override a policy that denies, nor any check that the merge is clean.
    ///
    /// # Errors
    ///
    /// Fails if there is no such worktree, it is not a completed execution's, it is already
    /// merged, the policy denies Git writes, or another operation is running. A merge that
    /// Git cannot do is not an error: the returned worktree says why (conflict, blocked).
    pub fn merge(&self, execution_id: &str) -> Result<ExecutionWorktree, WorktreeError> {
        let _busy = self.begin(execution_id)?;
        let worktree = self
            .get(execution_id)
            .ok_or_else(|| WorktreeError::NotFound(execution_id.to_owned()))?;
        let waiting = matches!(
            worktree.merge_status,
            MergeStatus::Pending | MergeStatus::Conflict | MergeStatus::Blocked
        );
        if worktree.status != WorktreeStatus::Completed || !waiting {
            return Err(WorktreeError::InvalidState(format!(
                "{:?} / {:?}",
                worktree.status, worktree.merge_status
            )));
        }
        if self.git_write(&worktree) == Permission::Denied {
            return Err(WorktreeError::PolicyDenied);
        }
        let evaluated = self.evaluate(
            &worktree,
            RunOutcome::Completed,
            worktree.validation,
            true,
            true,
        );
        self.apply(execution_id, evaluated, RunOutcome::Completed)
    }

    /// Whether the worktree has changes that are not in a commit (or cannot be told).
    fn has_uncommitted(&self, path: &Path) -> bool {
        self.manager
            .get_status(path)
            .map_or(true, |files| !files.is_empty())
    }

    /// Commits what the runtime left, measures the branch and reaches a decision.
    fn evaluate(
        &self,
        worktree: &ExecutionWorktree,
        outcome: RunOutcome,
        validation: Validation,
        approved_by_user: bool,
        automatic: bool,
    ) -> Evaluated {
        let path = Path::new(&worktree.worktree_path);
        let toplevel = Path::new(&worktree.repository_path);
        let git_write = self.git_write(worktree);
        let consistent =
            self.layout
                .is_worktree_of(path, &worktree.workspace_id, &worktree.execution_id)
                && self.manager.get_branch(path).as_deref() == Ok(worktree.branch_name.as_str());
        // Work is saved on the execution's own branch whenever Git writes are not denied: it
        // is what makes the work recoverable. Nothing else is touched.
        let uncommitted = if !consistent {
            false
        } else if git_write == Permission::Denied {
            self.has_uncommitted(path)
        } else {
            let message = format!("Atlas: work of {}", worktree.execution_id);
            self.manager.commit_all(path, &message).is_err() || self.has_uncommitted(path)
        };
        let assessment = self
            .manager
            .assess(toplevel, &worktree.base_branch, &worktree.branch_name)
            .ok();
        let readiness = self
            .manager
            .base_ready(toplevel, &worktree.base_branch)
            .ok();
        let decision = decide(&Facts {
            outcome,
            validation,
            git_write,
            approved_by_user,
            automatic,
            consistent,
            uncommitted,
            assessment: assessment.as_ref(),
            readiness,
        });
        Evaluated {
            decision,
            assessment,
            validation,
        }
    }

    /// Carries the decision out and saves the result.
    fn apply(
        &self,
        execution_id: &str,
        evaluated: Evaluated,
        outcome: RunOutcome,
    ) -> Result<ExecutionWorktree, WorktreeError> {
        let worktree = self
            .get(execution_id)
            .ok_or_else(|| WorktreeError::NotFound(execution_id.to_owned()))?;
        let Evaluated {
            decision,
            assessment,
            validation,
        } = evaluated;
        let mut merge = Merge {
            status: MergeStatus::NotEvaluated,
            block: None,
            recommendation: None,
            conflicts: Vec::new(),
            cleaned: None,
        };
        match decision {
            Decision::NothingToMerge => {
                merge.status = MergeStatus::NothingToMerge;
                merge.recommendation = Some(Recommendation::Discard);
                // Nothing to lose: the empty worktree and its branch go.
                merge.cleaned = Some(self.cleanup_inner(&worktree, false));
            }
            Decision::Blocked(reason, recommendation) => {
                merge.status = MergeStatus::Blocked;
                merge.block = Some(reason);
                merge.recommendation = Some(recommendation);
            }
            Decision::Conflict => {
                merge.status = MergeStatus::Conflict;
                merge.block = Some(BlockReason::Conflict);
                merge.recommendation = Some(Recommendation::ResolveConflicts);
                merge.conflicts = assessment
                    .as_ref()
                    .map(|a| a.conflicts.clone())
                    .unwrap_or_default();
            }
            Decision::AwaitApproval(recommendation) => {
                merge.status = MergeStatus::Pending;
                merge.recommendation = Some(recommendation);
            }
            Decision::Merge => self.attempt_merge(&worktree, &mut merge),
        }
        self.update(execution_id, |w| {
            w.status = match (outcome, merge.cleaned) {
                (_, Some(WorktreeStatus::Cleaned)) => WorktreeStatus::Cleaned,
                (_, Some(pending)) => pending,
                (RunOutcome::Completed, None) => WorktreeStatus::Completed,
                (RunOutcome::Ended, None) => WorktreeStatus::Failed,
            };
            w.merge_status = merge.status;
            w.block_reason = merge.block;
            w.recommendation = merge.recommendation;
            w.validation = validation;
            w.changes = assessment.map(|a| {
                let mut files = a.files;
                files.truncate(crate::domain::worktree::MAX_LISTED_FILES);
                WorktreeChanges {
                    files_changed: a.files_changed,
                    commits_ahead: a.commits_ahead,
                    commits_behind: a.commits_behind,
                    files,
                    conflicts: if merge.conflicts.is_empty() {
                        a.conflicts
                    } else {
                        merge.conflicts.clone()
                    },
                    mergeable: a.mergeable,
                }
            });
            w.clone()
        })
    }

    fn attempt_merge(&self, worktree: &ExecutionWorktree, merge: &mut Merge) {
        let message = format!("Merge {} ({})", worktree.branch_name, worktree.execution_id);
        let outcome = self.manager.merge(
            Path::new(&worktree.repository_path),
            &worktree.base_branch,
            &worktree.branch_name,
            &message,
        );
        match outcome {
            Ok(MergeOutcome::Merged { .. }) => {
                merge.status = MergeStatus::Merged;
                merge.cleaned = Some(self.cleanup_inner(worktree, true));
            }
            Ok(MergeOutcome::Conflict { files }) => {
                merge.status = MergeStatus::Conflict;
                merge.block = Some(BlockReason::Conflict);
                merge.recommendation = Some(Recommendation::ResolveConflicts);
                merge.conflicts = files;
            }
            Err(_) => {
                merge.status = MergeStatus::Blocked;
                merge.block = Some(BlockReason::Undetermined);
                merge.recommendation = Some(Recommendation::Review);
            }
        }
    }

    /// Removes the worktree folder and (if `branch_merged`) its branch. Only the folder this
    /// layout would have created for this execution, and only unforced: anything Git
    /// refuses to remove stays, as `CleanupPending`.
    fn cleanup_inner(&self, worktree: &ExecutionWorktree, branch_merged: bool) -> WorktreeStatus {
        let path = Path::new(&worktree.worktree_path);
        let toplevel = Path::new(&worktree.repository_path);
        let folder_gone = if path.exists() {
            self.layout
                .is_worktree_of(path, &worktree.workspace_id, &worktree.execution_id)
                && self.manager.remove(toplevel, path).is_ok()
        } else {
            true
        };
        if !folder_gone {
            return WorktreeStatus::CleanupPending;
        }
        // An empty branch is trivially merged; `-d` checks it either way.
        let _ = branch_merged;
        if self
            .manager
            .delete_merged_branch(toplevel, &worktree.branch_name)
            .is_ok()
        {
            WorktreeStatus::Cleaned
        } else {
            WorktreeStatus::CleanupPending
        }
    }

    /// Tries again for worktrees whose merge succeeded but whose removal did not.
    pub fn cleanup_pending(&self) {
        for worktree in self
            .list(None, None)
            .into_iter()
            .filter(|w| w.status == WorktreeStatus::CleanupPending)
            .filter(|w| {
                w.merge_status == MergeStatus::Merged
                    || w.merge_status == MergeStatus::NothingToMerge
            })
        {
            let Ok(_busy) = self.begin(&worktree.execution_id) else {
                continue;
            };
            let status = self.cleanup_inner(&worktree, true);
            let _ = self.update(&worktree.execution_id, |w| w.status = status);
        }
    }

    /// Worktrees whose execution was cut short by Atlas closing are not running any more:
    /// record that, keep them. Call once at startup, before anything runs.
    pub fn recover_interrupted(&self) {
        let result = self.config.modify(|config| {
            for worktree in &mut config.worktrees {
                if matches!(
                    worktree.status,
                    WorktreeStatus::Creating | WorktreeStatus::Active
                ) {
                    worktree.status = WorktreeStatus::Failed;
                    worktree.merge_status = MergeStatus::Blocked;
                    worktree.block_reason = Some(BlockReason::ExecutionNotCompleted);
                    worktree.recommendation = Some(Recommendation::Inspect);
                }
            }
            Ok(())
        });
        if let Err(error) = result {
            eprintln!("could not save recovered worktrees: {error:?}");
        }
    }

    // ---- a workflow's shared worktree -------------------------------------------------------
    //
    // One workflow run has one worktree for all its steps, so that what the Developer writes is
    // what the Validator and QA read. It is an ordinary worktree (the *primary*, made by
    // `prepare`) plus one *lease* per step: a record with the step's own execution id that
    // points at the same folder, so the security layer still knows which agent is working in it.
    // Leases are never merged or removed; the primary is decided once, when the run ends, and
    // never merged without the user.

    /// Of these agents, the one whose Git policy is the strictest (denied before approval
    /// required before allowed; the first among equals).
    pub fn strictest_git_agent(&self, workspace_id: &str, agent_ids: &[String]) -> Option<String> {
        let rank = |permission: Permission| match permission {
            Permission::Denied => 0,
            Permission::ApprovalRequired => 1,
            Permission::Allowed => 2,
        };
        agent_ids
            .iter()
            .min_by_key(|agent| rank(self.git_write_of(workspace_id, agent, "")))
            .cloned()
    }

    /// Marks a worktree as the primary one of a workflow run.
    ///
    /// # Errors
    ///
    /// Fails if there is no such worktree.
    pub fn mark_workflow(&self, execution_id: &str, run_id: &str) -> Result<(), WorktreeError> {
        self.update(execution_id, |w| {
            w.workflow_execution_id = Some(run_id.to_owned());
        })
    }

    /// Lets a step work in the workflow's worktree: a lease with the step's execution id.
    ///
    /// # Errors
    ///
    /// Fails if the primary is not an active workflow worktree of the same workspace, or the
    /// step already has a record.
    pub fn attach(
        &self,
        workspace_id: &str,
        agent_id: &str,
        step_execution_id: &str,
        primary_id: &str,
    ) -> Result<Prepared, WorktreeError> {
        WorktreeLayout::execution_number(step_execution_id)?;
        let primary = self
            .get(primary_id)
            .ok_or_else(|| WorktreeError::NotFound(primary_id.to_owned()))?;
        let usable = primary.workspace_id == workspace_id
            && primary.shared_with.is_none()
            && primary.workflow_execution_id.is_some()
            && primary.status == WorktreeStatus::Active
            && self.layout.is_worktree_of(
                Path::new(&primary.worktree_path),
                &primary.workspace_id,
                &primary.execution_id,
            );
        if !usable {
            return Err(WorktreeError::InvalidState(
                "not an active workflow worktree".to_owned(),
            ));
        }
        if self.get(step_execution_id).is_some() {
            return Err(WorktreeError::Collision(step_execution_id.to_owned()));
        }
        let working_dir = PathBuf::from(&primary.working_dir);
        let here = self
            .manager
            .head_commit(Path::new(&primary.worktree_path))
            .unwrap_or_else(|_| primary.base_commit.clone());
        let lease = ExecutionWorktree {
            execution_id: step_execution_id.to_owned(),
            agent_id: agent_id.to_owned(),
            base_commit: here,
            status: WorktreeStatus::Active,
            merge_status: MergeStatus::NotEvaluated,
            block_reason: None,
            changes: None,
            recommendation: None,
            shared_with: Some(primary_id.to_owned()),
            end_commit: None,
            created_at: now_ms(),
            ..primary
        };
        self.config.modify(|config| {
            config.worktrees.push(lease.clone());
            Ok(())
        })?;
        Ok(Prepared {
            working_dir,
            worktree: lease,
        })
    }

    /// A step ended: what it left in the worktree is committed (so the next step starts from a
    /// clean tree and the work is recoverable), and the lease records where the worktree got to.
    /// Never merges; the primary decides that, at the end of the run.
    pub fn finish_step(
        &self,
        step_execution_id: &str,
        outcome: RunOutcome,
    ) -> Option<ExecutionWorktree> {
        let lease = self.get(step_execution_id)?;
        let path = Path::new(&lease.worktree_path);
        if self.git_write(&lease) != Permission::Denied
            && self
                .layout
                .is_worktree_of(path, &lease.workspace_id, lease.shared_with.as_deref()?)
        {
            let message = format!("Atlas: step {step_execution_id}");
            let _ = self.manager.commit_all(path, &message);
        }
        let end = self.manager.head_commit(path).ok();
        self.update(step_execution_id, |w| {
            w.status = match outcome {
                RunOutcome::Completed => WorktreeStatus::Completed,
                RunOutcome::Ended => WorktreeStatus::Failed,
            };
            w.end_commit = end;
            w.clone()
        })
        .ok()
    }

    /// What a step changed in the shared worktree, by Git: the files between where the worktree
    /// was when the step began and where it was when it ended, and anything still uncommitted.
    pub fn step_delta(&self, step_execution_id: &str) -> Option<StepDelta> {
        let lease = self.get(step_execution_id)?;
        lease.shared_with.as_ref()?;
        let path = Path::new(&lease.worktree_path);
        let files = lease
            .end_commit
            .as_ref()
            .and_then(|end| {
                self.manager
                    .changed_files(path, &lease.base_commit, end)
                    .ok()
            })
            .unwrap_or_default();
        let uncommitted = self.manager.get_status(path).unwrap_or_default();
        Some(StepDelta {
            from: lease.base_commit,
            to: lease.end_commit,
            files,
            uncommitted,
        })
    }

    /// Makes a workflow worktree usable again after the app was closed in the middle of a run:
    /// it was recorded as cut short (kept, never merged); the run now goes on in it.
    ///
    /// # Errors
    ///
    /// Fails if it is not a workflow's primary worktree, or its folder or branch is not what
    /// Atlas made.
    pub fn reopen(&self, primary_id: &str) -> Result<ExecutionWorktree, WorktreeError> {
        let _busy = self.begin(primary_id)?;
        let primary = self
            .get(primary_id)
            .ok_or_else(|| WorktreeError::NotFound(primary_id.to_owned()))?;
        let path = Path::new(&primary.worktree_path);
        let consistent = primary.shared_with.is_none()
            && primary.workflow_execution_id.is_some()
            && path.is_dir()
            && self
                .layout
                .is_worktree_of(path, &primary.workspace_id, &primary.execution_id)
            && self.manager.get_branch(path).as_deref() == Ok(primary.branch_name.as_str());
        if !consistent {
            return Err(WorktreeError::InvalidState(
                "the workflow worktree is no longer what Atlas made".to_owned(),
            ));
        }
        self.update(primary_id, |w| {
            w.status = WorktreeStatus::Active;
            w.merge_status = MergeStatus::NotEvaluated;
            w.block_reason = None;
            w.recommendation = None;
            w.clone()
        })
    }

    /// The run is over: look at what the worktree holds and decide what can be done with it. Never
    /// merges by itself: with changes, the answer is "waiting for the user" (or blocked, with the
    /// reason); with none, the empty worktree goes.
    pub fn close_shared(
        &self,
        primary_id: &str,
        outcome: RunOutcome,
        validation: Validation,
    ) -> Option<ExecutionWorktree> {
        self.finalize_with(primary_id, outcome, validation, false)
    }

    /// Everything the worktree holds compared with where the workflow started, from Git.
    ///
    /// # Errors
    ///
    /// Fails if there is no such worktree or Git cannot read it.
    pub fn change_set(&self, primary_id: &str) -> Result<ChangeSet, WorktreeError> {
        let primary = self
            .get(primary_id)
            .ok_or_else(|| WorktreeError::NotFound(primary_id.to_owned()))?;
        let path = Path::new(&primary.worktree_path);
        if !path.is_dir() {
            // Merged and removed, or discarded: the branch (if it is still there) is the record.
            return Err(WorktreeError::InvalidState(
                "the worktree is gone".to_owned(),
            ));
        }
        let head = self.manager.head_commit(path)?;
        let files: Vec<FileChange> =
            self.manager
                .changed_files(path, &primary.base_commit, &head)?;
        let all = files.len();
        let additions = files.iter().filter_map(|f| f.additions).sum();
        let deletions = files.iter().filter_map(|f| f.deletions).sum();
        let mut listed = files;
        listed.truncate(MAX_CHANGESET_FILES);
        Ok(ChangeSet {
            base_revision: primary.base_commit,
            current_revision: head,
            files_changed: u32::try_from(all).unwrap_or(u32::MAX),
            files: listed,
            additions,
            deletions,
            uncommitted: self.manager.get_status(path).unwrap_or_default(),
            captured_at: now_ms(),
        })
    }

    /// The real diff of the workflow's worktree, of one file when `file` is given.
    ///
    /// # Errors
    ///
    /// As [`Self::change_set`]; and if `file` is not a plain relative path.
    pub fn diff(
        &self,
        primary_id: &str,
        file: Option<&str>,
        max_bytes: usize,
    ) -> Result<String, WorktreeError> {
        let primary = self
            .get(primary_id)
            .ok_or_else(|| WorktreeError::NotFound(primary_id.to_owned()))?;
        let path = Path::new(&primary.worktree_path);
        let head = self.manager.head_commit(path)?;
        self.manager
            .diff_between(path, &primary.base_commit, &head, file, max_bytes)
    }

    /// The diff between two commits of the run, read in the project's repository: for changes
    /// that were applied and whose worktree is gone.
    ///
    /// # Errors
    ///
    /// Fails if there is no such worktree record, or Git cannot produce the diff.
    pub fn diff_revisions(
        &self,
        primary_id: &str,
        from: &str,
        to: &str,
        file: Option<&str>,
        max_bytes: usize,
    ) -> Result<String, WorktreeError> {
        let primary = self
            .get(primary_id)
            .ok_or_else(|| WorktreeError::NotFound(primary_id.to_owned()))?;
        self.manager.diff_between(
            Path::new(&primary.repository_path),
            from,
            to,
            file,
            max_bytes,
        )
    }

    /// The user's explicit "discard": the worktree folder goes (never forced, so anything Git
    /// considers unclean stays) and the work is no longer offered. The *branch is kept*: nothing
    /// Atlas does deletes commits that were not merged, so the work can still be recovered
    /// with Git.
    ///
    /// # Errors
    ///
    /// Fails if it is not a finished workflow primary worktree, it was merged, or Git refuses
    /// to remove the folder.
    pub fn discard_shared(&self, primary_id: &str) -> Result<ExecutionWorktree, WorktreeError> {
        let _busy = self.begin(primary_id)?;
        let primary = self
            .get(primary_id)
            .ok_or_else(|| WorktreeError::NotFound(primary_id.to_owned()))?;
        let removable = primary.shared_with.is_none()
            && primary.workflow_execution_id.is_some()
            && !matches!(
                primary.status,
                WorktreeStatus::Active | WorktreeStatus::Creating | WorktreeStatus::Cleaned
            )
            && primary.merge_status != MergeStatus::Merged;
        if !removable {
            return Err(WorktreeError::InvalidState(format!(
                "{:?} / {:?}",
                primary.status, primary.merge_status
            )));
        }
        let path = Path::new(&primary.worktree_path);
        if path.exists() {
            if !self
                .layout
                .is_worktree_of(path, &primary.workspace_id, &primary.execution_id)
            {
                return Err(WorktreeError::OutsideRoot(path.display().to_string()));
            }
            self.manager
                .remove(Path::new(&primary.repository_path), path)?;
        }
        self.update(primary_id, |w| {
            w.status = WorktreeStatus::Cleaned;
            w.merge_status = MergeStatus::Blocked;
            w.block_reason = None;
            w.recommendation = None;
            w.clone()
        })
    }

    /// The folder to open in an editor for this worktree: the project's folder inside it. Only
    /// a folder Atlas made, and only while it exists.
    pub fn editor_folder(&self, primary_id: &str) -> Option<PathBuf> {
        let primary = self.get(primary_id)?;
        let path = Path::new(&primary.worktree_path);
        let folder = PathBuf::from(&primary.working_dir);
        (path.is_dir()
            && folder.is_dir()
            && self
                .layout
                .is_worktree_of(path, &primary.workspace_id, &primary.execution_id))
        .then_some(folder)
    }

    /// The facts about a worktree as the strings an execution event carries.
    pub fn event_metadata(worktree: &ExecutionWorktree) -> BTreeMap<String, String> {
        fn wire(value: &impl serde::Serialize) -> String {
            serde_json::to_value(value)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default()
        }
        let mut metadata = BTreeMap::from([
            ("baseBranch".to_owned(), worktree.base_branch.clone()),
            ("branch".to_owned(), worktree.branch_name.clone()),
            ("worktreePath".to_owned(), worktree.worktree_path.clone()),
            ("worktreeStatus".to_owned(), wire(&worktree.status)),
            ("mergeStatus".to_owned(), wire(&worktree.merge_status)),
        ]);
        if let Some(reason) = worktree.block_reason {
            metadata.insert("blockReason".to_owned(), wire(&reason));
        }
        if let Some(recommendation) = worktree.recommendation {
            metadata.insert("recommendation".to_owned(), wire(&recommendation));
        }
        if let Some(changes) = &worktree.changes {
            metadata.insert("filesChanged".to_owned(), changes.files_changed.to_string());
            metadata.insert("commitsAhead".to_owned(), changes.commits_ahead.to_string());
        }
        metadata.insert("validation".to_owned(), wire(&worktree.validation));
        metadata
    }
}

struct Evaluated {
    decision: Decision,
    assessment: Option<Assessment>,
    validation: Validation,
}

struct Merge {
    status: MergeStatus,
    block: Option<BlockReason>,
    recommendation: Option<Recommendation>,
    conflicts: Vec<String>,
    cleaned: Option<WorktreeStatus>,
}
