use std::fs;
use std::path::PathBuf;

use super::atomic_file;
use crate::application::specs::ports::SpecStore;
use crate::domain::spec::Spec;

const FILE: &str = "spec.json";

/// Specs kept in Atlas's own data folder, one folder per spec (`<root>/<id>/spec.json`): never in
/// a project, and apart from `config.json`, which is rewritten whole on every change and would
/// carry every document of every revision with it.
pub struct FsSpecStore {
    root: PathBuf,
}

impl FsSpecStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// `None` for an id that is not a plain folder name: the id is data, not a path.
    fn folder(&self, id: &str) -> Option<PathBuf> {
        let plain = !id.is_empty()
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        plain.then(|| self.root.join(id))
    }
}

impl SpecStore for FsSpecStore {
    fn load_all(&self) -> (Vec<Spec>, Vec<String>) {
        let mut specs = Vec::new();
        let mut unreadable = Vec::new();
        let Ok(entries) = fs::read_dir(&self.root) else {
            return (specs, unreadable);
        };
        for entry in entries.flatten() {
            let path = entry.path().join(FILE);
            if !path.is_file() {
                continue;
            }
            let read = fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|text| serde_json::from_str::<Spec>(&text).map_err(|e| e.to_string()));
            match read {
                Ok(spec) => specs.push(spec),
                Err(error) => unreadable.push(format!("{}: {error}", path.display())),
            }
        }
        specs.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
        (specs, unreadable)
    }

    fn save(&self, spec: &Spec) -> Result<(), String> {
        let folder = self
            .folder(&spec.id)
            .ok_or_else(|| "the spec's id is not a plain name".to_owned())?;
        fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(spec).map_err(|e| e.to_string())?;
        // A crash never leaves half a record: the previous one stays until the new one is whole.
        atomic_file::write(&folder.join(FILE), json.as_bytes()).map_err(|e| e.to_string())
    }

    fn remove(&self, id: &str) -> Result<(), String> {
        let Some(folder) = self.folder(id) else {
            return Err("the spec's id is not a plain name".to_owned());
        };
        match fs::remove_dir_all(folder) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::assert_is_empty)]
    use super::*;
    use crate::domain::spec::rules::{create, NewSpec};
    use crate::domain::spec::{ArtifactKind, FrameworkKind, SpecOrigin};

    fn store(name: &str) -> FsSpecStore {
        let dir = std::env::temp_dir().join(format!("atlas-specs-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        FsSpecStore::new(dir)
    }

    fn spec(id: &str) -> Spec {
        create(
            NewSpec {
                id: id.to_owned(),
                title: "T".to_owned(),
                request: "R".to_owned(),
                context: String::new(),
                workspace_id: "w".to_owned(),
                project_id: "p".to_owned(),
                framework: FrameworkKind::Openspec,
                framework_schema: "spec-driven/1".to_owned(),
                change_id: "t".to_owned(),
                author_agent_id: "a".to_owned(),
                reviewer_agent_id: "a".to_owned(),
                origin: SpecOrigin::Created,
            },
            &[(ArtifactKind::Proposal, vec![])],
            1,
        )
    }

    #[test]
    fn a_spec_comes_back_as_it_was_saved() {
        let store = store("roundtrip");
        let saved = spec("spec-1");

        store.save(&saved).unwrap();
        let (loaded, unreadable) = store.load_all();

        assert_eq!(loaded, vec![saved]);
        assert!(unreadable.is_empty());
    }

    #[test]
    fn saving_again_replaces_the_record_whole() {
        let store = store("replace");
        let mut s = spec("spec-1");
        store.save(&s).unwrap();
        s.title = "Changed".to_owned();
        store.save(&s).unwrap();

        let (loaded, _) = store.load_all();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].title, "Changed");
        let files: Vec<_> = fs::read_dir(store.root.join("spec-1"))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(files.len(), 1, "no temporary file stays behind");
    }

    #[test]
    fn a_record_that_cannot_be_read_is_reported_and_left_alone() {
        let store = store("corrupt");
        store.save(&spec("good")).unwrap();
        let bad = store.root.join("bad");
        fs::create_dir_all(&bad).unwrap();
        fs::write(bad.join(FILE), "{ not json").unwrap();

        let (loaded, unreadable) = store.load_all();

        assert_eq!(loaded.len(), 1);
        assert_eq!(unreadable.len(), 1);
        assert!(bad.join(FILE).exists(), "nothing is deleted");
    }

    #[test]
    fn an_id_cannot_reach_outside_the_folder() {
        let store = store("escape");
        let mut s = spec("x");
        s.id = "../evil".to_owned();

        assert!(store.save(&s).is_err());
        assert!(store.remove("../evil").is_err());
    }

    #[test]
    fn removing_forgets_the_spec_and_removing_twice_is_fine() {
        let store = store("remove");
        store.save(&spec("spec-1")).unwrap();

        store.remove("spec-1").unwrap();
        store.remove("spec-1").unwrap();

        assert!(store.load_all().0.is_empty());
    }
}
