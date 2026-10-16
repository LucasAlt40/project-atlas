//! The configuration files of the CLIs, on disk. Reading is plain; writing goes through a
//! temporary file and a rename after a copy of the original was kept, so a failure leaves the
//! file as it was.

use std::path::{Path, PathBuf};

use crate::application::mcp::config_io::ConfigDialect;
use crate::application::mcp::{RuntimeConfigFiles, RuntimeFileError};

pub struct FsRuntimeConfigFiles;

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
}

fn error(action: &str, path: &Path, e: &std::io::Error) -> RuntimeFileError {
    RuntimeFileError(format!("{action} {}: {e}", path.display()))
}

impl RuntimeConfigFiles for FsRuntimeConfigFiles {
    fn locate(&self, dialect: ConfigDialect, project_path: &str) -> Option<String> {
        let path = match dialect {
            // The project scope: it travels with the repository and leaves `~/.claude.json`, which
            // holds the CLI's state as well, alone.
            ConfigDialect::Claude | ConfigDialect::Mcp => {
                let project = PathBuf::from(project_path);
                project.is_absolute().then(|| project.join(".mcp.json"))?
            }
            ConfigDialect::Gemini => home()?.join(".gemini").join("settings.json"),
            ConfigDialect::OpenCode => home()?
                .join(".config")
                .join("opencode")
                .join("opencode.json"),
            ConfigDialect::Codex => home()?.join(".codex").join("config.toml"),
            // `COPILOT_HOME` moves the CLI's folder, as it does for the CLI itself.
            ConfigDialect::Copilot => std::env::var_os("COPILOT_HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .or_else(|| home().map(|h| h.join(".copilot")))?
                .join("mcp-config.json"),
            ConfigDialect::Antigravity => return None,
        };
        path.to_str().map(str::to_owned)
    }

    fn user_file(&self, dialect: ConfigDialect) -> Option<String> {
        (dialect == ConfigDialect::Claude)
            .then(|| home().map(|h| h.join(".claude.json")))
            .flatten()
            .and_then(|p| p.to_str().map(str::to_owned))
    }

    fn read(&self, path: &str) -> Result<Option<String>, RuntimeFileError> {
        match std::fs::read_to_string(path) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(error("reading", Path::new(path), &e)),
        }
    }

    fn write(&self, path: &str, text: &str) -> Result<Option<String>, RuntimeFileError> {
        let target = Path::new(path);
        if let Some(folder) = target.parent() {
            std::fs::create_dir_all(folder).map_err(|e| error("creating", folder, &e))?;
        }
        let backup = if target.exists() {
            let mut name = target.as_os_str().to_owned();
            name.push(format!(
                ".atlas-backup-{}",
                crate::application::support::now_ms()
            ));
            let backup = PathBuf::from(name);
            std::fs::copy(target, &backup).map_err(|e| error("copying", target, &e))?;
            Some(backup)
        } else {
            None
        };
        let mut staging = target.as_os_str().to_owned();
        staging.push(".atlas-tmp");
        let staging = PathBuf::from(staging);
        std::fs::write(&staging, text).map_err(|e| error("writing", &staging, &e))?;
        if let Ok(meta) = std::fs::metadata(target) {
            let _ = std::fs::set_permissions(&staging, meta.permissions());
        }
        std::fs::rename(&staging, target).map_err(|e| {
            let _ = std::fs::remove_file(&staging);
            error("replacing", target, &e)
        })?;
        Ok(backup.and_then(|b| b.to_str().map(str::to_owned)))
    }
}
