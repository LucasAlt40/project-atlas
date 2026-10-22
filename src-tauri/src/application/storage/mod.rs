//! How much of the disk Atlas uses, and the person's cleanup of what executions leave behind
//! (Settings › Storage).
//!
//! What can be cleaned:
//! - **worktrees**: folders of isolated executions that are no longer running, and folders no
//!   execution owns any more. A workflow run still waiting for a decision is discarded the way
//!   the person's own "Discard" does it. Uncommitted work is committed to the execution's branch
//!   first, so the work can still be found with Git;
//! - **sandbox temporary files**: profiles and temporary folders of confined executions;
//! - **branches** (`atlas/exec-…`) whose folder is gone. Deleting one deletes its commits, so it
//!   is a separate choice, off unless the person asks.
//!
//! Nothing is cleaned while an agent is running. Settings, history, memory and specs are only
//! measured, never touched.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::application::errors::{AppError, ErrorCode};
use crate::application::security::MacosSandbox;
use crate::application::sessions::SessionRegistry;
use crate::application::workflow::integration::IntegrationService;
use crate::application::workflow::service::WorkflowService;
use crate::application::worktree::WorktreeService;
use crate::domain::workflow::IntegrationStatus;

/// What a part of Atlas's data folder holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StoragePartKind {
    Worktrees,
    SandboxTemp,
    /// `config.json`: settings, workspaces, agents and the history of runs.
    Settings,
    Memory,
    Specs,
    WorkspaceContext,
    Other,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoragePart {
    pub kind: StoragePartKind,
    pub bytes: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Amount {
    pub count: u32,
    pub bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Leftovers {
    pub worktrees: Amount,
    pub sandbox_temp: Amount,
    /// Execution branches whose folder is gone (their size lives in the project's repository).
    pub branches: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsage {
    /// Atlas's data folder.
    pub folder: String,
    pub total_bytes: u64,
    /// Largest first.
    pub parts: Vec<StoragePart>,
    pub leftovers: Leftovers,
    /// An agent is running: cleaning has to wait.
    pub running: bool,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupChoice {
    #[serde(default)]
    pub worktrees: bool,
    #[serde(default)]
    pub sandbox_temp: bool,
    #[serde(default)]
    pub branches: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupReport {
    pub freed_bytes: u64,
    pub worktrees_removed: u32,
    /// Folders Git refused to remove (something in them it would not commit).
    pub worktrees_kept: u32,
    pub sandbox_temp_removed: u32,
    pub branches_removed: u32,
    /// Branches Git refused to delete (checked out somewhere).
    pub branches_kept: u32,
}

/// What to do with one worktree folder.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Removal {
    /// A workflow run waiting for a decision: discarded as the person's "Discard" does.
    Discard {
        run_id: String,
        execution_id: String,
    },
    /// An execution that ended: its folder goes, its record says so.
    Finished { execution_id: String },
    /// A folder no execution owns.
    Orphan,
}

pub struct StorageService {
    data_dir: PathBuf,
    worktrees: Arc<WorktreeService>,
    workflows: Arc<WorkflowService>,
    integration: Arc<IntegrationService>,
    sessions: Arc<SessionRegistry>,
    sandbox: Arc<MacosSandbox>,
}

/// Bytes in a folder or file, links not followed.
pub fn size_of(path: &Path) -> u64 {
    let Ok(metadata) = path.symlink_metadata() else {
        return 0;
    };
    if !metadata.is_dir() {
        return metadata.len();
    }
    let mut total = 0;
    let mut pending = vec![path.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(metadata) = entry.path().symlink_metadata() else {
                continue;
            };
            if metadata.is_dir() {
                pending.push(entry.path());
            } else {
                total += metadata.len();
            }
        }
    }
    total
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

impl StorageService {
    pub fn new(
        data_dir: PathBuf,
        worktrees: Arc<WorktreeService>,
        workflows: Arc<WorkflowService>,
        integration: Arc<IntegrationService>,
        sessions: Arc<SessionRegistry>,
        sandbox: Arc<MacosSandbox>,
    ) -> Self {
        Self {
            data_dir,
            worktrees,
            workflows,
            integration,
            sessions,
            sandbox,
        }
    }

    /// An agent's process is alive or an isolated execution is being prepared or running.
    fn running(&self) -> bool {
        !self.sessions.live_executions().is_empty() || self.worktrees.any_running()
    }

    /// What would become of each worktree folder; folders of running work are left out.
    fn removals(&self) -> Vec<(PathBuf, Removal)> {
        self.worktrees
            .folders()
            .into_iter()
            .filter_map(|path| {
                let Some(worktree) = self.worktrees.record_at(&path) else {
                    return Some((path, Removal::Orphan));
                };
                if matches!(
                    worktree.status,
                    crate::domain::worktree::WorktreeStatus::Creating
                        | crate::domain::worktree::WorktreeStatus::Active
                ) {
                    return None;
                }
                let run = worktree
                    .workflow_execution_id
                    .as_deref()
                    .and_then(|id| self.workflows.execution(id));
                let removal = match run {
                    Some(run) if !run.status.is_final() => return None,
                    Some(run)
                        if run.integration.worktree_execution_id.as_deref()
                            == Some(worktree.execution_id.as_str())
                            && matches!(
                                run.integration.status,
                                IntegrationStatus::ChangesAvailable
                                    | IntegrationStatus::Blocked
                                    | IntegrationStatus::Conflicts
                                    | IntegrationStatus::Failed
                                    | IntegrationStatus::KeptIsolated
                            ) =>
                    {
                        Removal::Discard {
                            run_id: run.id,
                            execution_id: worktree.execution_id,
                        }
                    }
                    _ => Removal::Finished {
                        execution_id: worktree.execution_id,
                    },
                };
                Some((path, removal))
            })
            .collect()
    }

    /// Executions whose branch is still needed: runs that are going on, and changes applied to
    /// a project that can still be taken back out (that needs the branch).
    fn kept_branches(&self) -> HashSet<String> {
        self.worktrees
            .list(None, None)
            .into_iter()
            .filter(|w| {
                w.workflow_execution_id
                    .as_deref()
                    .and_then(|id| self.workflows.execution(id))
                    .is_some_and(|run| {
                        !run.status.is_final()
                            || (run.integration.status == IntegrationStatus::Integrated
                                && run.integration.can_undo)
                    })
            })
            .map(|w| w.execution_id)
            .collect()
    }

    /// The size of Atlas's data folder by part, and what can be cleaned. Reads only.
    pub fn usage(&self) -> StorageUsage {
        let mut parts: Vec<StoragePart> = Vec::new();
        let mut add = |kind: StoragePartKind, bytes: u64| {
            if let Some(part) = parts.iter_mut().find(|p| p.kind == kind) {
                part.bytes += bytes;
            } else {
                parts.push(StoragePart { kind, bytes });
            }
        };
        if let Ok(entries) = fs::read_dir(&self.data_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let kind = match name.as_str() {
                    "worktrees" => StoragePartKind::Worktrees,
                    "sandbox" => StoragePartKind::SandboxTemp,
                    "memory" => StoragePartKind::Memory,
                    "specs" => StoragePartKind::Specs,
                    "workspace-context" => StoragePartKind::WorkspaceContext,
                    n if n.starts_with("config.json") => StoragePartKind::Settings,
                    _ => StoragePartKind::Other,
                };
                add(kind, size_of(&entry.path()));
            }
        }
        parts.retain(|p| p.bytes > 0);
        parts.sort_by_key(|p| std::cmp::Reverse(p.bytes));

        let removals = self.removals();
        let temp = self.sandbox.leftovers();
        StorageUsage {
            folder: self.data_dir.display().to_string(),
            total_bytes: parts.iter().map(|p| p.bytes).sum(),
            parts,
            leftovers: Leftovers {
                worktrees: Amount {
                    count: count(removals.len()),
                    bytes: removals.iter().map(|(path, _)| size_of(path)).sum(),
                },
                sandbox_temp: Amount {
                    count: count(temp.len()),
                    bytes: temp.iter().map(|path| size_of(path)).sum(),
                },
                branches: count(
                    self.worktrees
                        .leftover_branches(&self.kept_branches())
                        .len(),
                ),
            },
            running: self.running(),
        }
    }

    /// Cleans what the person chose. Worktrees go before branches, so the branch of a folder
    /// removed now can go in the same cleanup.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::StorageBusy`] while an agent is running; nothing is touched then.
    pub fn clean(&self, choice: CleanupChoice) -> Result<CleanupReport, AppError> {
        if self.running() {
            return Err(AppError::new(ErrorCode::StorageBusy));
        }
        let mut report = CleanupReport::default();
        if choice.worktrees {
            for (path, removal) in self.removals() {
                let bytes = size_of(&path);
                let removed = match removal {
                    Removal::Discard {
                        run_id,
                        execution_id,
                    } => {
                        self.worktrees.save_leftover_work(&execution_id);
                        self.integration.discard(&run_id).is_ok()
                    }
                    Removal::Finished { execution_id } => {
                        self.worktrees.remove_finished(&execution_id).is_ok()
                    }
                    Removal::Orphan => self.worktrees.remove_orphan(&path).is_ok(),
                };
                if removed && !path.exists() {
                    report.worktrees_removed += 1;
                    report.freed_bytes += bytes;
                } else {
                    report.worktrees_kept += 1;
                }
            }
            self.worktrees.tidy();
        }
        if choice.sandbox_temp {
            let bytes: u64 = self.sandbox.leftovers().iter().map(|p| size_of(p)).sum();
            report.sandbox_temp_removed = count(self.sandbox.clear_leftovers());
            let left: u64 = self.sandbox.leftovers().iter().map(|p| size_of(p)).sum();
            report.freed_bytes += bytes.saturating_sub(left);
        }
        if choice.branches {
            for (repo, branch) in self.worktrees.leftover_branches(&self.kept_branches()) {
                if self
                    .worktrees
                    .delete_leftover_branch(&repo, &branch)
                    .is_ok()
                {
                    report.branches_removed += 1;
                } else {
                    report.branches_kept += 1;
                }
            }
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests;
