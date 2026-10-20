//! What Atlas has published to a memory server on this machine (ADR 0040), kept in Atlas's own data
//! folder so a page whose note was deleted while the server was off can still be found and retired.
//! Apart from the memory folder, whose every `*.json` is read as a note.

use std::collections::BTreeSet;
use std::path::PathBuf;

use super::atomic_file;
use crate::application::memory::external::{Published, PublishedLedger};

pub struct FsPublishedLedger {
    file: PathBuf,
}

impl FsPublishedLedger {
    pub fn new(file: PathBuf) -> Self {
        Self { file }
    }
}

impl PublishedLedger for FsPublishedLedger {
    fn load(&self) -> BTreeSet<Published> {
        // A ledger that cannot be read is an empty one: the next sync publishes again, and the
        // pages it cannot know about stay where they are.
        std::fs::read_to_string(&self.file)
            .ok()
            .and_then(|text| serde_json::from_str::<BTreeSet<Published>>(&text).ok())
            .unwrap_or_default()
    }

    fn save(&self, entries: &BTreeSet<Published>) -> Result<(), String> {
        if let Some(folder) = self.file.parent() {
            std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(entries).map_err(|e| e.to_string())?;
        atomic_file::write(&self.file, json.as_bytes()).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str) -> Published {
        Published {
            workspace: "default".to_owned(),
            project: "billing".to_owned(),
            path: path.to_owned(),
        }
    }

    #[test]
    fn what_was_published_survives_a_restart_and_a_broken_file_is_an_empty_ledger() {
        let dir = std::env::temp_dir().join(format!("atlas-ledger-{}", std::process::id()));
        let ledger = FsPublishedLedger::new(dir.join("published.json"));
        assert!(ledger.load().is_empty());

        ledger
            .save(&BTreeSet::from([entry("atlas/a.md"), entry("atlas/b.md")]))
            .unwrap();
        assert_eq!(ledger.load().len(), 2);

        std::fs::write(dir.join("published.json"), "{ not json").unwrap();
        assert!(ledger.load().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
