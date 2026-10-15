//! What Memory needs from the outside. The adapters are in `infrastructure/` (the real disk) and
//! in the tests (memory), so no use case touches a file.

#[cfg(test)]
use std::collections::BTreeMap;
use std::collections::BTreeSet;
#[cfg(test)]
use std::sync::Mutex;

#[cfg(test)]
use crate::application::support::LockExt;
use crate::domain::memory::Memory;

/// Where memories are kept between runs of the app: Atlas's own data, never a project and never
/// `config.json`.
pub trait MemoryStore: Send + Sync {
    /// Every memory that can be read. A record that cannot is left as it is and reported in the
    /// second part, never deleted.
    fn load_all(&self) -> (Vec<Memory>, Vec<String>);

    /// # Errors
    ///
    /// Says why the record could not be written. The previous one is untouched.
    fn save(&self, memory: &Memory) -> Result<(), String>;

    /// # Errors
    ///
    /// Says why the record could not be removed.
    fn remove(&self, id: &str) -> Result<(), String>;
}

/// What a lesson may depend on in the project, checked without a model.
pub trait ProjectProbe: Send + Sync {
    /// Whether the project-relative `path` is there. A path that leaves the project is not.
    fn path_exists(&self, root: &str, path: &str) -> bool;

    /// The names of the `package.json` scripts, or `None` when the project has no `package.json`
    /// (a lesson about a script then has nothing to be checked against, and is kept).
    fn npm_scripts(&self, root: &str) -> Option<BTreeSet<String>>;
}

/// Memories in memory, for tests.
#[cfg(test)]
#[derive(Default)]
pub struct MemoryMemoryStore {
    items: Mutex<BTreeMap<String, Memory>>,
    fail_saves: Mutex<bool>,
}

#[cfg(test)]
impl MemoryMemoryStore {
    pub fn fail_saves(&self, fail: bool) {
        *self.fail_saves.lock_or_recover() = fail;
    }

    pub fn stored(&self, id: &str) -> Option<Memory> {
        self.items.lock_or_recover().get(id).cloned()
    }
}

#[cfg(test)]
impl MemoryStore for MemoryMemoryStore {
    fn load_all(&self) -> (Vec<Memory>, Vec<String>) {
        (
            self.items.lock_or_recover().values().cloned().collect(),
            Vec::new(),
        )
    }

    fn save(&self, memory: &Memory) -> Result<(), String> {
        if *self.fail_saves.lock_or_recover() {
            return Err("disk full".to_owned());
        }
        self.items
            .lock_or_recover()
            .insert(memory.id.clone(), memory.clone());
        Ok(())
    }

    fn remove(&self, id: &str) -> Result<(), String> {
        self.items.lock_or_recover().remove(id);
        Ok(())
    }
}

/// A project in memory, for tests.
#[cfg(test)]
#[derive(Default)]
pub struct FakeProbe {
    pub paths: Mutex<BTreeSet<String>>,
    pub scripts: Mutex<Option<BTreeSet<String>>>,
}

#[cfg(test)]
impl ProjectProbe for FakeProbe {
    fn path_exists(&self, _root: &str, path: &str) -> bool {
        self.paths.lock_or_recover().contains(path)
    }

    fn npm_scripts(&self, _root: &str) -> Option<BTreeSet<String>> {
        self.scripts.lock_or_recover().clone()
    }
}
