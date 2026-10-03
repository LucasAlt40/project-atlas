use std::path::{Component, Path, PathBuf};

use super::WorktreeError;

/// Where Atlas keeps worktrees, and the only place a branch name or a worktree path is derived.
///
/// ```text
/// <app data>/worktrees/<workspace id>/exec-000042      branch: atlas/exec-000042
/// ```
///
/// Outside the project on purpose: nothing is added to the user's repository or its ignore
/// rules, the agent cannot reach the main checkout through a parent folder, and cleaning up is
/// removing one folder. The name carries the execution number, zero-padded, so it sorts and is
/// predictable; it is never reused (see `Collision`).
#[derive(Debug, Clone)]
pub struct WorktreeLayout {
    root: PathBuf,
}

const BRANCH_PREFIX: &str = "atlas/";
const MAX_ID_LEN: usize = 100;

impl WorktreeLayout {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The number in an id that Atlas generated (`exec-42`). Anything else is refused: ids from
    /// elsewhere never become a branch or a folder.
    ///
    /// # Errors
    ///
    /// [`WorktreeError::InvalidIdentifier`] unless the id is `exec-` and digits.
    pub fn execution_number(execution_id: &str) -> Result<u64, WorktreeError> {
        let digits = execution_id
            .strip_prefix("exec-")
            .filter(|d| !d.is_empty() && d.len() <= 12 && d.bytes().all(|b| b.is_ascii_digit()))
            .ok_or_else(|| WorktreeError::InvalidIdentifier(execution_id.to_owned()))?;
        digits
            .parse()
            .map_err(|_| WorktreeError::InvalidIdentifier(execution_id.to_owned()))
    }

    fn dir_name(execution_id: &str) -> Result<String, WorktreeError> {
        Ok(format!("exec-{:06}", Self::execution_number(execution_id)?))
    }

    /// `atlas/exec-000042`.
    ///
    /// # Errors
    ///
    /// See [`Self::execution_number`].
    pub fn branch_name(execution_id: &str) -> Result<String, WorktreeError> {
        Ok(format!("{BRANCH_PREFIX}{}", Self::dir_name(execution_id)?))
    }

    /// Whether `branch` is exactly a name [`Self::branch_name`] can produce. Used before a
    /// branch name reaches a Git command, whatever its source.
    pub fn is_execution_branch(branch: &str) -> bool {
        branch
            .strip_prefix(BRANCH_PREFIX)
            .and_then(|name| name.strip_prefix("exec-"))
            .is_some_and(|digits| digits.len() >= 6 && digits.bytes().all(|b| b.is_ascii_digit()))
    }

    fn workspace_component(workspace_id: &str) -> Result<&str, WorktreeError> {
        let valid = !workspace_id.is_empty()
            && workspace_id.len() <= MAX_ID_LEN
            && !workspace_id.starts_with('-')
            && workspace_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        if valid {
            Ok(workspace_id)
        } else {
            Err(WorktreeError::InvalidIdentifier(workspace_id.to_owned()))
        }
    }

    /// The worktree folder of an execution.
    ///
    /// # Errors
    ///
    /// [`WorktreeError::InvalidIdentifier`] if either id is not one Atlas generated.
    pub fn path_for(
        &self,
        workspace_id: &str,
        execution_id: &str,
    ) -> Result<PathBuf, WorktreeError> {
        Ok(self
            .root
            .join(Self::workspace_component(workspace_id)?)
            .join(Self::dir_name(execution_id)?))
    }

    /// Whether `path` is exactly where this layout puts the worktree of that execution: the
    /// check before anything is removed. Compared as written (no `..`, no symlink games: the
    /// folder itself must not be a link).
    pub fn is_worktree_of(&self, path: &Path, workspace_id: &str, execution_id: &str) -> bool {
        let Ok(expected) = self.path_for(workspace_id, execution_id) else {
            return false;
        };
        path == expected
            && !path.components().any(|c| matches!(c, Component::ParentDir))
            && path
                .symlink_metadata()
                .is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> WorktreeLayout {
        WorktreeLayout::new(PathBuf::from("/data/worktrees"))
    }

    #[test]
    fn names_are_derived_from_the_execution_number_alone() {
        assert_eq!(
            WorktreeLayout::branch_name("exec-42").unwrap(),
            "atlas/exec-000042"
        );
        assert_eq!(
            layout().path_for("ws-1-2", "exec-42").unwrap(),
            PathBuf::from("/data/worktrees/ws-1-2/exec-000042")
        );
        assert!(WorktreeLayout::is_execution_branch("atlas/exec-000042"));
    }

    #[test]
    fn ids_that_atlas_did_not_generate_never_become_names_or_paths() {
        for bad in [
            "",
            "exec-",
            "exec-4x",
            "exec-../../etc",
            "exec-1/../2",
            "EXEC-1",
            "exec--1",
            "exec-1\n",
            "exec-1 --force",
            "exec-1234567890123",
            "e",
        ] {
            assert!(WorktreeLayout::branch_name(bad).is_err(), "{bad:?}");
            assert!(layout().path_for("ws-1", bad).is_err(), "{bad:?}");
        }
        for bad in [
            "", "..", "../x", "a/b", "a\\b", "-rf", ".", "ws 1", "ws:1", "ws\0", "C:\\x",
        ] {
            assert!(layout().path_for(bad, "exec-1").is_err(), "{bad:?}");
        }
    }

    #[test]
    fn only_atlas_execution_branches_are_accepted() {
        for bad in [
            "main",
            "atlas/exec-1",
            "atlas/exec-000001/../x",
            "-atlas/exec-000001",
            "atlas/exec-00000a",
            "atlas/exec-000001 ",
            "refs/heads/atlas/exec-000001",
            "atlas/other",
        ] {
            assert!(!WorktreeLayout::is_execution_branch(bad), "{bad:?}");
        }
    }
}
