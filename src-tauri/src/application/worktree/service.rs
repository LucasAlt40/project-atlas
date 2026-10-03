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
    BlockReason, ExecutionWorktree, MergeStatus, Recommendation, Validation, WorktreeChanges,
    WorktreeStatus,
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
struct Facts<'a> {
    outcome: RunOutcome,
    validation: Validation,
    git_write: Permission,
    /// The user is explicitly asking for the merge: that answers "approval required".
    approved_by_user: bool,
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
    if facts.git_write == Permission::ApprovalRequired && !facts.approved_by_user {
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
        let scope = ExecutionScope {
            workspace_id: worktree.workspace_id.clone(),
            agent_id: worktree.agent_id.clone(),
            execution_id: worktree.execution_id.clone(),
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
        let _busy = self.begin(execution_id).ok()?;
        let worktree = self.get(execution_id)?;
        let evaluated = self.evaluate(&worktree, outcome, validation, false);
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
        let evaluated = self.evaluate(&worktree, RunOutcome::Completed, worktree.validation, true);
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
