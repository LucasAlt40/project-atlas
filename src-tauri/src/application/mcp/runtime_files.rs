//! The port for the configuration files of the CLIs Atlas drives (`~/.codex/config.toml`,
//! `~/.gemini/settings.json`…). Atlas reads one to offer what is in it, and writes one only when a
//! person asks, adding servers and nothing else (see [`super::config_io::merge`]).

use super::config_io::ConfigDialect;

/// Why a file could not be read or written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFileError(pub String);

pub trait RuntimeConfigFiles: Send + Sync {
    /// Where `dialect`'s file is for a project, or `None` when there is no file Atlas may edit
    /// (Antigravity, or no home folder to look in).
    fn locate(&self, dialect: ConfigDialect, project_path: &str) -> Option<String>;

    /// A second file the CLI keeps servers in, read but never written (Claude Code's
    /// `~/.claude.json`, where `claude mcp add` puts them).
    fn user_file(&self, _dialect: ConfigDialect) -> Option<String> {
        None
    }

    /// The file's text; `None` when it does not exist yet.
    ///
    /// # Errors
    ///
    /// The file exists and cannot be read.
    fn read(&self, path: &str) -> Result<Option<String>, RuntimeFileError>;

    /// Replaces the file (creating its folder). When it existed, a copy is kept beside it first and
    /// its path returned.
    ///
    /// # Errors
    ///
    /// The copy or the write failed; the original is then as it was.
    fn write(&self, path: &str, text: &str) -> Result<Option<String>, RuntimeFileError>;
}

/// For a build that has no filesystem wired (and for tests that do not care).
pub struct NoRuntimeFiles;

impl RuntimeConfigFiles for NoRuntimeFiles {
    fn locate(&self, _dialect: ConfigDialect, _project_path: &str) -> Option<String> {
        None
    }

    fn read(&self, _path: &str) -> Result<Option<String>, RuntimeFileError> {
        Ok(None)
    }

    fn write(&self, _path: &str, _text: &str) -> Result<Option<String>, RuntimeFileError> {
        Err(RuntimeFileError("no filesystem".to_owned()))
    }
}

#[cfg(test)]
pub mod memory {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::{ConfigDialect, RuntimeConfigFiles, RuntimeFileError};

    /// Files in memory, one place per dialect.
    #[derive(Default)]
    pub struct MemoryRuntimeFiles {
        pub files: Mutex<HashMap<String, String>>,
    }

    impl RuntimeConfigFiles for MemoryRuntimeFiles {
        fn locate(&self, dialect: ConfigDialect, project_path: &str) -> Option<String> {
            (dialect != ConfigDialect::Antigravity).then(|| format!("{project_path}/{dialect:?}"))
        }

        fn read(&self, path: &str) -> Result<Option<String>, RuntimeFileError> {
            Ok(self.files.lock().unwrap().get(path).cloned())
        }

        fn write(&self, path: &str, text: &str) -> Result<Option<String>, RuntimeFileError> {
            let mut files = self.files.lock().unwrap();
            let backup = files.get(path).cloned().map(|old| {
                let at = format!("{path}.atlas-backup");
                files.insert(at.clone(), old);
                at
            });
            files.insert(path.to_owned(), text.to_owned());
            Ok(backup)
        }
    }
}
