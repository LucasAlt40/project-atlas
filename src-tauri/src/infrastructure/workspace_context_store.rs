use std::fs;
use std::path::PathBuf;

use super::atomic_file;
use crate::application::errors::{AppError, ErrorCode};
use crate::application::harness::workspace_context::{ContextFile, WorkspaceContextStore};

/// Kept in Atlas's own data folder, one folder per workspace: never inside a project, so the
/// context of a workspace does not belong to any one repository and is not committed with one.
pub struct FsWorkspaceContextStore {
    root: PathBuf,
}

impl FsWorkspaceContextStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// `None` for an id that is not a plain folder name: the id is data, not a path.
    fn folder(&self, workspace_id: &str) -> Option<PathBuf> {
        let plain = !workspace_id.is_empty()
            && workspace_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        plain.then(|| self.root.join(workspace_id))
    }

    fn file(&self, workspace_id: &str, file: ContextFile) -> Option<PathBuf> {
        let name = match file {
            ContextFile::Generated => "context.md",
            ContextFile::Notes => "notes.md",
        };
        self.folder(workspace_id).map(|dir| dir.join(name))
    }
}

impl WorkspaceContextStore for FsWorkspaceContextStore {
    fn read(&self, workspace_id: &str, file: ContextFile) -> Option<String> {
        let path = self.file(workspace_id, file)?;
        let meta = fs::symlink_metadata(&path).ok()?;
        meta.is_file().then(|| fs::read_to_string(path).ok())?
    }

    fn write(&self, workspace_id: &str, file: ContextFile, text: &str) -> Result<(), AppError> {
        let failed =
            |error: String| AppError::new(ErrorCode::HarnessGenerationFailed).with_detail(error);
        let path = self
            .file(workspace_id, file)
            .ok_or_else(|| failed("invalid workspace id".to_owned()))?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| failed(e.to_string()))?;
        }
        // A crash never leaves half a file.
        atomic_file::write(&path, text.as_bytes()).map_err(|e| failed(e.to_string()))
    }

    fn remove(&self, workspace_id: &str) {
        if let Some(dir) = self.folder(workspace_id) {
            let _ = fs::remove_dir_all(dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(name: &str) -> FsWorkspaceContextStore {
        let dir = std::env::temp_dir().join(format!("atlas-wsctx-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        FsWorkspaceContextStore::new(dir)
    }

    #[test]
    fn the_two_parts_are_kept_apart_and_come_back() {
        let store = store("roundtrip");

        store
            .write("ws-1", ContextFile::Generated, "generated")
            .unwrap();
        store.write("ws-1", ContextFile::Notes, "mine").unwrap();

        assert_eq!(
            store.read("ws-1", ContextFile::Generated).as_deref(),
            Some("generated")
        );
        assert_eq!(
            store.read("ws-1", ContextFile::Notes).as_deref(),
            Some("mine")
        );
        assert_eq!(store.read("ws-2", ContextFile::Notes), None);
    }

    #[test]
    fn a_workspace_id_cannot_reach_outside_the_folder() {
        let store = store("escape");

        assert!(store.write("../evil", ContextFile::Notes, "x").is_err());
        assert_eq!(store.read("../evil", ContextFile::Notes), None);
    }

    #[test]
    fn removing_a_workspace_forgets_its_context() {
        let store = store("remove");
        store.write("ws-1", ContextFile::Notes, "mine").unwrap();

        store.remove("ws-1");

        assert_eq!(store.read("ws-1", ContextFile::Notes), None);
    }
}
