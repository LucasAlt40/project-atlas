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
        // A link is neither written through nor replaced. In a project it can be planted by the
        // repository (`.mcp.json` pointing at a private file elsewhere), and the backup and the
        // merged file would carry that file's contents into the repository; in the home folder it
        // is usually a person's dotfiles, which replacing it would silently detach.
        if std::fs::symlink_metadata(target).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(RuntimeFileError(format!(
                "{} is a link; Atlas does not write through or replace it",
                target.display()
            )));
        }
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
        // Keeps the file's permissions: these files can hold tokens and are often private.
        super::atomic_file::write(target, text.as_bytes())
            .map_err(|e| error("replacing", target, &e))?;
        Ok(backup.and_then(|b| b.to_str().map(str::to_owned)))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;

    fn folder(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("atlas-runtime-files-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_file_is_replaced_and_the_old_one_is_kept_as_a_backup() {
        let dir = folder("backup");
        let path = dir.join(".mcp.json");
        fs::write(&path, "{\"old\":true}").unwrap();

        let backup = FsRuntimeConfigFiles
            .write(path.to_str().unwrap(), "{\"new\":true}")
            .unwrap()
            .unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "{\"new\":true}");
        assert_eq!(fs::read_to_string(backup).unwrap(), "{\"old\":true}");
    }

    #[test]
    fn a_link_planted_in_a_project_is_not_written_and_its_target_is_not_copied() {
        let project = folder("planted");
        let private = folder("planted-private").join("config.json");
        fs::write(&private, "{\"auths\":{\"registry\":\"secret\"}}").unwrap();
        let path = project.join(".mcp.json");
        std::os::unix::fs::symlink(&private, &path).unwrap();

        let refused = FsRuntimeConfigFiles.write(path.to_str().unwrap(), "{}");

        assert!(refused.unwrap_err().0.contains("is a link"));
        assert!(fs::symlink_metadata(&path).unwrap().is_symlink());
        assert_eq!(
            fs::read_to_string(&private).unwrap(),
            "{\"auths\":{\"registry\":\"secret\"}}"
        );
        let names: Vec<_> = fs::read_dir(&project).unwrap().flatten().collect();
        assert_eq!(names.len(), 1, "no backup of the private file was made");
    }
}
