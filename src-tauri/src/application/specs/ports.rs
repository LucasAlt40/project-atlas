//! What the specification use cases need from the outside, as ports. The adapters are in
//! `infrastructure/` (the real disk) and in the tests (memory), so no use case touches a file.

use std::collections::BTreeMap;
#[cfg(test)]
use std::sync::Mutex;

use super::framework::ChangeFolder;
#[cfg(test)]
use crate::application::support::LockExt;
use crate::domain::spec::Spec;

/// Where specs are kept between runs of the app. One record per spec; Atlas's own data, never
/// the project's.
pub trait SpecStore: Send + Sync {
    /// Every spec that can be read. A record that cannot is left as it is and reported in the
    /// second part, never deleted.
    fn load_all(&self) -> (Vec<Spec>, Vec<String>);

    /// # Errors
    ///
    /// Says why the record could not be written. The previous one is untouched.
    fn save(&self, spec: &Spec) -> Result<(), String>;

    /// # Errors
    ///
    /// Says why the record could not be removed.
    fn remove(&self, id: &str) -> Result<(), String>;
}

/// Why something in the project could not be read or written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilesError {
    /// The file is already there.
    Exists,
    /// The path, or something on the way to it, is not what Atlas may write to (a symlink out of
    /// the project, a path with `..`).
    Unsafe(String),
    /// The project folder is gone or not a folder.
    NoProject,
    Io(String),
}

/// The project's `openspec/` folder, as Atlas reads and writes it. Every path is relative to the
/// project root and is checked: nothing leaves the root, nothing follows a symlink out of it.
pub trait ProjectFiles: Send + Sync {
    /// The main specs of the project: capability path → `spec.md`, read-only and bounded.
    fn main_specs(&self, root: &str) -> BTreeMap<String, String>;

    /// The ids of the changes in `<root>/openspec/changes/`, without `archive`.
    fn change_ids(&self, root: &str, changes_dir: &str) -> Vec<String>;

    fn change_exists(&self, root: &str, changes_dir: &str, id: &str) -> bool;

    /// The files of one change, read without changing anything.
    ///
    /// # Errors
    ///
    /// When the folder is not there or cannot be read.
    fn read_change(
        &self,
        root: &str,
        changes_dir: &str,
        id: &str,
    ) -> Result<ChangeFolder, FilesError>;

    /// The content of a file, if it is there.
    fn read(&self, root: &str, relative: &str) -> Option<String>;

    /// Writes a file that is not there yet, creating the folders. Never overwrites.
    ///
    /// # Errors
    ///
    /// [`FilesError::Exists`] when the file is there.
    fn create(&self, root: &str, relative: &str, content: &str) -> Result<(), FilesError>;

    /// Replaces the content of a file that is there, whole or not at all.
    ///
    /// # Errors
    ///
    /// When the file cannot be replaced.
    fn replace(&self, root: &str, relative: &str, content: &str) -> Result<(), FilesError>;
}

/// Specs in memory, for tests.
#[cfg(test)]
#[derive(Default)]
pub struct MemorySpecStore {
    specs: Mutex<BTreeMap<String, Spec>>,
    fail_saves: Mutex<bool>,
}

#[cfg(test)]
impl MemorySpecStore {
    /// Makes every save fail, to see what the service does when the disk does.
    pub fn fail_saves(&self, fail: bool) {
        *self.fail_saves.lock_or_recover() = fail;
    }

    pub fn stored(&self, id: &str) -> Option<Spec> {
        self.specs.lock_or_recover().get(id).cloned()
    }
}

#[cfg(test)]
impl SpecStore for MemorySpecStore {
    fn load_all(&self) -> (Vec<Spec>, Vec<String>) {
        (
            self.specs.lock_or_recover().values().cloned().collect(),
            Vec::new(),
        )
    }

    fn save(&self, spec: &Spec) -> Result<(), String> {
        if *self.fail_saves.lock_or_recover() {
            return Err("disk full".to_owned());
        }
        self.specs
            .lock_or_recover()
            .insert(spec.id.clone(), spec.clone());
        Ok(())
    }

    fn remove(&self, id: &str) -> Result<(), String> {
        self.specs.lock_or_recover().remove(id);
        Ok(())
    }
}

/// A project folder in memory, for tests: paths → contents.
#[cfg(test)]
#[derive(Default)]
pub struct MemoryProjectFiles {
    pub files: Mutex<BTreeMap<String, String>>,
    /// Paths whose creation fails, to see a publication left half way.
    pub fail_on: Mutex<Vec<String>>,
}

#[cfg(test)]
impl MemoryProjectFiles {
    pub fn put(&self, path: &str, content: &str) {
        self.files
            .lock_or_recover()
            .insert(path.to_owned(), content.to_owned());
    }

    pub fn get(&self, path: &str) -> Option<String> {
        self.files.lock_or_recover().get(path).cloned()
    }

    pub fn paths(&self) -> Vec<String> {
        self.files.lock_or_recover().keys().cloned().collect()
    }
}

#[cfg(test)]
impl ProjectFiles for MemoryProjectFiles {
    fn main_specs(&self, _root: &str) -> BTreeMap<String, String> {
        self.files
            .lock_or_recover()
            .iter()
            .filter_map(|(path, content)| {
                let inner = path
                    .strip_prefix("openspec/specs/")?
                    .strip_suffix("/spec.md")?;
                Some((inner.to_owned(), content.clone()))
            })
            .collect()
    }

    fn change_ids(&self, _root: &str, changes_dir: &str) -> Vec<String> {
        let prefix = format!("{changes_dir}/");
        let mut ids: Vec<String> = self
            .files
            .lock_or_recover()
            .keys()
            .filter_map(|p| p.strip_prefix(&prefix))
            .filter_map(|rest| rest.split('/').next())
            .filter(|id| *id != "archive")
            .map(str::to_owned)
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }

    fn change_exists(&self, _root: &str, changes_dir: &str, id: &str) -> bool {
        let prefix = format!("{changes_dir}/{id}/");
        self.files
            .lock_or_recover()
            .keys()
            .any(|p| p.starts_with(&prefix))
    }

    fn read_change(
        &self,
        _root: &str,
        changes_dir: &str,
        id: &str,
    ) -> Result<ChangeFolder, FilesError> {
        let prefix = format!("{changes_dir}/{id}/");
        let files: Vec<_> = self
            .files
            .lock_or_recover()
            .iter()
            .filter_map(|(p, c)| {
                p.strip_prefix(&prefix)
                    .map(|rel| crate::domain::spec::SpecFile {
                        path: rel.to_owned(),
                        content: c.clone(),
                    })
            })
            .collect();
        if files.is_empty() {
            return Err(FilesError::Io("not found".to_owned()));
        }
        Ok(ChangeFolder { files })
    }

    fn read(&self, _root: &str, relative: &str) -> Option<String> {
        self.get(relative)
    }

    fn create(&self, _root: &str, relative: &str, content: &str) -> Result<(), FilesError> {
        if self.fail_on.lock_or_recover().iter().any(|p| p == relative) {
            return Err(FilesError::Io("disk full".to_owned()));
        }
        let mut files = self.files.lock_or_recover();
        if files.contains_key(relative) {
            return Err(FilesError::Exists);
        }
        files.insert(relative.to_owned(), content.to_owned());
        Ok(())
    }

    fn replace(&self, _root: &str, relative: &str, content: &str) -> Result<(), FilesError> {
        self.files
            .lock_or_recover()
            .insert(relative.to_owned(), content.to_owned());
        Ok(())
    }
}
