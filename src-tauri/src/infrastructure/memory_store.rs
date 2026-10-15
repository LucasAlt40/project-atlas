use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::application::memory::{MemoryStore, ProjectProbe};
use crate::domain::memory::Memory;

/// Memories kept in Atlas's own data folder, one file each (`<root>/<id>.json`): never in a
/// project, and apart from `config.json`, which is rewritten whole on every change.
pub struct FsMemoryStore {
    root: PathBuf,
}

impl FsMemoryStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// `None` for an id that is not a plain file name: the id is data, not a path.
    fn file(&self, id: &str) -> Option<PathBuf> {
        let plain = !id.is_empty()
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        plain.then(|| self.root.join(format!("{id}.json")))
    }
}

impl MemoryStore for FsMemoryStore {
    fn load_all(&self) -> (Vec<Memory>, Vec<String>) {
        let mut memories = Vec::new();
        let mut unreadable = Vec::new();
        let Ok(entries) = fs::read_dir(&self.root) else {
            return (memories, unreadable);
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let read = fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|text| serde_json::from_str::<Memory>(&text).map_err(|e| e.to_string()));
            match read {
                Ok(memory) => memories.push(memory),
                Err(error) => unreadable.push(format!("{}: {error}", path.display())),
            }
        }
        memories.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
        (memories, unreadable)
    }

    fn save(&self, memory: &Memory) -> Result<(), String> {
        let file = self
            .file(&memory.id)
            .ok_or_else(|| "the memory's id is not a plain name".to_owned())?;
        fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(memory).map_err(|e| e.to_string())?;
        // Beside the target and renamed, so a crash never leaves half a record: the previous one
        // stays until the new one is whole.
        let temp = file.with_extension("json.tmp");
        fs::write(&temp, json).map_err(|e| e.to_string())?;
        fs::rename(&temp, file).map_err(|e| e.to_string())
    }

    fn remove(&self, id: &str) -> Result<(), String> {
        let Some(file) = self.file(id) else {
            return Err("the memory's id is not a plain name".to_owned());
        };
        match fs::remove_file(file) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// Reads the project to check what a lesson depends on. It only looks: it never writes, runs or
/// follows a path out of the project.
pub struct FsProjectProbe;

/// The path as a relative path made only of plain names; `None` for `..`, an absolute path or a
/// prefix, so a lesson can never make Atlas look outside the project.
fn plain_relative(path: &str) -> Option<PathBuf> {
    let relative = Path::new(path);
    let plain = !path.trim().is_empty()
        && relative
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    plain.then(|| relative.to_path_buf())
}

impl ProjectProbe for FsProjectProbe {
    fn path_exists(&self, root: &str, path: &str) -> bool {
        plain_relative(path).is_some_and(|relative| Path::new(root).join(relative).exists())
    }

    fn npm_scripts(&self, root: &str) -> Option<BTreeSet<String>> {
        let text = fs::read_to_string(Path::new(root).join("package.json")).ok()?;
        let json: serde_json::Value = serde_json::from_str(&text).ok()?;
        let scripts = json.get("scripts").and_then(|s| s.as_object());
        Some(scripts.map_or_else(BTreeSet::new, |s| s.keys().cloned().collect()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::memory::tests::memory;
    use crate::domain::memory::MemoryScope;

    fn dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("atlas-memory-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_memory_comes_back_as_it_was_saved_and_a_bad_file_is_reported_not_deleted() {
        let root = dir("roundtrip");
        let store = FsMemoryStore::new(root.join("memories"));
        let saved = memory("memory-1", MemoryScope::User, None, "run the check");

        store.save(&saved).unwrap();
        fs::write(root.join("memories").join("broken.json"), "{not json").unwrap();
        let (loaded, unreadable) = store.load_all();

        assert_eq!(loaded, [saved]);
        assert_eq!(unreadable.len(), 1);
        assert!(root.join("memories").join("broken.json").exists());
    }

    #[test]
    fn an_id_that_is_not_a_plain_name_is_refused_so_it_cannot_become_a_path() {
        let store = FsMemoryStore::new(dir("ids"));
        let mut bad = memory("x", MemoryScope::User, None, "c");
        bad.id = "../escape".to_owned();

        assert!(store.save(&bad).is_err());
        assert!(store.remove("../escape").is_err());
    }

    #[test]
    fn removing_a_memory_that_is_gone_is_not_an_error() {
        let store = FsMemoryStore::new(dir("remove"));
        let saved = memory("memory-1", MemoryScope::User, None, "c");
        store.save(&saved).unwrap();

        store.remove("memory-1").unwrap();
        store.remove("memory-1").unwrap();

        assert_eq!(store.load_all().0, []);
    }

    #[test]
    fn the_probe_looks_inside_the_project_only() {
        let root = dir("probe");
        fs::create_dir_all(root.join("scripts")).unwrap();
        fs::write(root.join("scripts/check.sh"), "").unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"check":"x","build":"y"}}"#,
        )
        .unwrap();
        let outside = root.parent().unwrap().to_string_lossy().into_owned();
        let root_str = root.to_string_lossy().into_owned();
        let probe = FsProjectProbe;

        assert!(probe.path_exists(&root_str, "scripts/check.sh"));
        assert!(!probe.path_exists(&root_str, "scripts/gone.sh"));
        assert!(!probe.path_exists(&root_str, "../"));
        assert!(!probe.path_exists(&root_str, &outside));
        assert_eq!(
            probe.npm_scripts(&root_str),
            Some(["build".to_owned(), "check".to_owned()].into())
        );
        assert_eq!(probe.npm_scripts(&outside), None);
    }
}
