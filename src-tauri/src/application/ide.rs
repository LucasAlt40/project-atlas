//! Port: opening a folder in the user's editor. Atlas launches the editor itself, as a program
//! with a list of arguments (never a command line), choosing the program from a fixed list of
//! known editors and the folder from what Atlas stored. Nothing an agent wrote ever reaches it.

use std::fmt;
use std::path::Path;

/// An editor Atlas can open a folder in.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Ide {
    /// A stable id from the fixed list (`vscode`, `cursor`, `intellij`).
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdeError {
    /// Not one of the editors Atlas knows.
    Unknown(String),
    /// Known, but not installed here.
    NotInstalled(String),
    /// The folder is not an absolute path to an existing folder.
    NotAFolder,
    LaunchFailed(String),
}

impl fmt::Display for IdeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(id) => write!(f, "Unknown editor: {id}"),
            Self::NotInstalled(id) => write!(f, "{id} is not installed"),
            Self::NotAFolder => write!(f, "Not a folder that can be opened"),
            Self::LaunchFailed(why) => write!(f, "The editor could not be started: {why}"),
        }
    }
}

impl std::error::Error for IdeError {}

pub trait IdeLauncher: Send + Sync {
    /// The editors found on this machine.
    fn available(&self) -> Vec<Ide>;

    /// Opens `folder` in the editor.
    ///
    /// # Errors
    ///
    /// See [`IdeError`].
    fn open(&self, ide_id: &str, folder: &Path) -> Result<(), IdeError>;
}

#[cfg(test)]
pub mod fake {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use super::{Ide, IdeError, IdeLauncher};

    /// Records what it was asked to open.
    #[derive(Default)]
    pub struct FakeIde {
        pub opened: Mutex<Vec<(String, PathBuf)>>,
    }

    impl IdeLauncher for FakeIde {
        fn available(&self) -> Vec<Ide> {
            vec![Ide {
                id: "vscode".to_owned(),
                name: "Visual Studio Code".to_owned(),
            }]
        }

        fn open(&self, ide_id: &str, folder: &std::path::Path) -> Result<(), IdeError> {
            if ide_id != "vscode" {
                return Err(IdeError::Unknown(ide_id.to_owned()));
            }
            self.opened
                .lock()
                .unwrap()
                .push((ide_id.to_owned(), folder.to_path_buf()));
            Ok(())
        }
    }
}
