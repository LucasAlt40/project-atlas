//! What stays in Atlas's worktree folder after the work in it was decided, set aside or
//! forgotten, and the person's explicit cleanup of it (Settings › Storage).
//!
//! Removing a folder never loses committed work: what is left uncommitted is committed to the
//! execution's branch first, and Git removes the folder unforced. Only deleting a branch loses
//! commits, and that is a separate choice the person makes.

use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use super::WorktreeService;
use crate::application::worktree::WorktreeError;
use crate::domain::worktree::{ExecutionWorktree, MergeStatus, WorktreeStatus};

const SAVE_MESSAGE: &str = "Atlas: work saved before its worktree was cleaned up";

/// A folder, not a link to one.
fn is_real_dir(path: &Path) -> bool {
    path.symlink_metadata()
        .is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
}

/// The repository a worktree folder belongs to, read from its `.git` file
/// (`gitdir: <repo>/.git/worktrees/<name>`). `None` when the folder is not a worktree Git still
/// knows.
fn repository_of(path: &Path) -> Option<PathBuf> {
    let text = fs::read_to_string(path.join(".git")).ok()?;
    let admin = PathBuf::from(text.trim().strip_prefix("gitdir:")?.trim());
    if !admin.is_absolute() || !admin.is_dir() {
        return None;
    }
    let common = admin.parent()?.parent()?;
    let toplevel = if common.file_name().is_some_and(|n| n == ".git") {
        common.parent()?
    } else {
        common
    };
    toplevel.is_dir().then(|| toplevel.to_path_buf())
}

impl WorktreeService {
    /// Whether an isolated execution is being prepared or is running.
    pub fn any_running(&self) -> bool {
        self.config.read(|config| {
            config
                .worktrees
                .iter()
                .any(|w| matches!(w.status, WorktreeStatus::Creating | WorktreeStatus::Active))
        })
    }

    /// The worktree folders in Atlas's folder (`<root>/<workspace>/<exec-…>`), links left out.
    pub fn folders(&self) -> Vec<PathBuf> {
        let mut found = Vec::new();
        let Ok(workspaces) = fs::read_dir(self.layout.root()) else {
            return found;
        };
        for workspace in workspaces.flatten() {
            if !is_real_dir(&workspace.path()) {
                continue;
            }
            let Ok(entries) = fs::read_dir(workspace.path()) else {
                continue;
            };
            found.extend(
                entries
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|path| is_real_dir(path)),
            );
        }
        found.sort();
        found
    }

    /// The execution whose own folder this is (a step's lease of a workflow folder is not).
    pub fn record_at(&self, path: &Path) -> Option<ExecutionWorktree> {
        self.config.read(|config| {
            config
                .worktrees
                .iter()
                .find(|w| w.shared_with.is_none() && Path::new(&w.worktree_path) == path)
                .cloned()
        })
    }

    /// Commits what is left uncommitted in an execution's folder to its branch, so removing the
    /// folder loses nothing. Best effort: a folder Git will not commit stays when it is removed.
    pub fn save_leftover_work(&self, execution_id: &str) {
        let Some(worktree) = self.get(execution_id) else {
            return;
        };
        let path = Path::new(&worktree.worktree_path);
        if self
            .layout
            .is_worktree_of(path, &worktree.workspace_id, &worktree.execution_id)
        {
            let _ = self.manager.commit_all(path, SAVE_MESSAGE);
        }
    }

    /// Removes the folder of an execution that is not running: its uncommitted work is committed
    /// to its branch first, Git removes the folder unforced, and the record says it is gone. The
    /// branch stays.
    ///
    /// # Errors
    ///
    /// Fails if the execution is unknown, a lease, still running, its folder is not one Atlas
    /// made, or Git refuses to remove it.
    pub fn remove_finished(&self, execution_id: &str) -> Result<(), WorktreeError> {
        let _busy = self.begin(execution_id)?;
        let worktree = self
            .get(execution_id)
            .ok_or_else(|| WorktreeError::NotFound(execution_id.to_owned()))?;
        if worktree.shared_with.is_some()
            || matches!(
                worktree.status,
                WorktreeStatus::Creating | WorktreeStatus::Active
            )
        {
            return Err(WorktreeError::InvalidState(format!(
                "{:?}",
                worktree.status
            )));
        }
        let path = Path::new(&worktree.worktree_path);
        if path.exists() {
            if !self
                .layout
                .is_worktree_of(path, &worktree.workspace_id, &worktree.execution_id)
            {
                return Err(WorktreeError::OutsideRoot(path.display().to_string()));
            }
            let _ = self.manager.commit_all(path, SAVE_MESSAGE);
            self.manager
                .remove(Path::new(&worktree.repository_path), path)?;
        }
        self.update(execution_id, |w| {
            w.status = WorktreeStatus::Cleaned;
            if !matches!(
                w.merge_status,
                MergeStatus::Merged | MergeStatus::Applied | MergeStatus::NothingToMerge
            ) {
                w.merge_status = MergeStatus::Blocked;
            }
            w.block_reason = None;
            w.recommendation = None;
        })?;
        Ok(())
    }

    /// Removes a folder in Atlas's folder that no execution owns (its workspace or run was
    /// deleted, or Atlas stopped while making it). A worktree Git knows has its uncommitted work
    /// committed to its branch and is removed by Git, unforced; a folder Git does not know is
    /// deleted.
    ///
    /// # Errors
    ///
    /// Fails if the path is not a folder of Atlas's layout, an execution owns it, or Git refuses.
    pub fn remove_orphan(&self, path: &Path) -> Result<(), WorktreeError> {
        let inside = path.parent().and_then(Path::parent) == Some(self.layout.root())
            && !path.components().any(|c| matches!(c, Component::ParentDir))
            && is_real_dir(path);
        if !inside {
            return Err(WorktreeError::OutsideRoot(path.display().to_string()));
        }
        if self.record_at(path).is_some() {
            return Err(WorktreeError::InvalidState(
                "owned by an execution".to_owned(),
            ));
        }
        match repository_of(path) {
            Some(toplevel) => {
                let _ = self.manager.commit_all(path, SAVE_MESSAGE);
                self.manager.remove(&toplevel, path)?;
            }
            None => fs::remove_dir_all(path).map_err(|e| WorktreeError::Git(e.to_string()))?,
        }
        Ok(())
    }

    /// The repositories Atlas made worktrees in.
    fn repositories(&self) -> BTreeSet<PathBuf> {
        self.list(None, None)
            .into_iter()
            .map(|w| PathBuf::from(w.repository_path))
            .filter(|path| path.is_dir())
            .collect()
    }

    /// Execution branches nothing uses any more: their folder is gone and they are not among
    /// `keep` (execution ids whose branch is still needed, such as changes that can be undone).
    pub fn leftover_branches(&self, keep: &HashSet<String>) -> Vec<(PathBuf, String)> {
        let records = self.list(None, None);
        let held: HashSet<&str> = records
            .iter()
            .filter(|w| {
                keep.contains(&w.execution_id)
                    || Path::new(&w.worktree_path).exists()
                    || matches!(w.status, WorktreeStatus::Creating | WorktreeStatus::Active)
            })
            .map(|w| w.branch_name.as_str())
            .collect();
        self.repositories()
            .into_iter()
            .flat_map(|repo| {
                self.manager
                    .execution_branches(&repo)
                    .into_iter()
                    .filter(|branch| !held.contains(branch.as_str()))
                    .map(move |branch| (repo.clone(), branch))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// Deletes an execution branch, merged or not: the person's explicit choice.
    ///
    /// # Errors
    ///
    /// Fails if it is not an execution branch or Git refuses (it is checked out somewhere).
    pub fn delete_leftover_branch(&self, repo: &Path, branch: &str) -> Result<(), WorktreeError> {
        self.manager.delete_branch(repo, branch)
    }

    /// After a cleanup: Git forgets worktrees whose folder is gone, and empty workspace folders
    /// go.
    pub fn tidy(&self) {
        for repo in self.repositories() {
            self.manager.prune(&repo);
        }
        if let Ok(workspaces) = fs::read_dir(self.layout.root()) {
            for workspace in workspaces.flatten() {
                if is_real_dir(&workspace.path()) {
                    // Only an empty folder is removed.
                    let _ = fs::remove_dir(workspace.path());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_repository_of_a_worktree_from_its_git_file() {
        let base = std::env::temp_dir().join(format!("atlas-leftover-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let admin = base.join("repo/.git/worktrees/exec-000001");
        fs::create_dir_all(&admin).unwrap();
        let folder = base.join("worktrees/ws/exec-000001");
        fs::create_dir_all(&folder).unwrap();
        fs::write(
            folder.join(".git"),
            format!("gitdir: {}\n", admin.display()),
        )
        .unwrap();

        assert_eq!(repository_of(&folder), Some(base.join("repo")));

        fs::remove_dir_all(&admin).unwrap();
        assert_eq!(repository_of(&folder), None, "Git no longer knows it");
        fs::write(folder.join(".git"), "nonsense").unwrap();
        assert_eq!(repository_of(&folder), None);
        let _ = fs::remove_dir_all(&base);
    }
}
