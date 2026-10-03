use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::application::errors::{AppError, ErrorCode};
use crate::application::harness::manifest::MAX_MANIFEST_BYTES;
use crate::application::harness::HarnessStore;
use crate::domain::harness::{HarnessFile, WriteReport};

const ATLAS_DIR: &str = ".atlas";
const MAX_CONTEXT_BYTES: u64 = 64 * 1024;
const MAX_KNOWLEDGE_READ: u64 = 1024 * 1024;

fn file_name(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, n)| n)
}

/// `.atlas/` on disk, inside the project's folder. Every path is checked to be a plain
/// relative path, and nothing is ever written through a symlink.
pub struct FsHarnessStore;

impl FsHarnessStore {
    fn atlas(project_path: &str) -> PathBuf {
        Path::new(project_path).join(ATLAS_DIR)
    }
}

fn failed(detail: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::HarnessGenerationFailed).with_detail(detail)
}

/// Reads a regular file (not a link) up to `max` bytes; `Ok(None)` if it does not exist.
fn read_regular(path: &Path, max: u64) -> Result<Option<String>, AppError> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(AppError::new(ErrorCode::HarnessInvalid).with_detail(e.to_string())),
    };
    if !meta.is_file() || meta.len() > max {
        return Err(
            AppError::new(ErrorCode::HarnessInvalid).with_detail(format!(
                "{} is not a regular file of a sane size",
                path.display()
            )),
        );
    }
    fs::read_to_string(path)
        .map(Some)
        .map_err(|e| AppError::new(ErrorCode::HarnessInvalid).with_detail(e.to_string()))
}

/// A relative path made only of normal names, so it cannot climb out of `.atlas/`.
fn safe_relative(path: &str) -> bool {
    !path.is_empty()
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}

/// Creates the folder chain under `base`, refusing to pass through a symlink.
fn ensure_dir(base: &Path, relative: &Path) -> Result<(), AppError> {
    let mut current = base.to_path_buf();
    for part in relative.components() {
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => return Err(failed(format!("{} is not a folder", current.display()))),
            Err(_) => fs::create_dir(&current).map_err(|e| failed(e.to_string()))?,
        }
    }
    Ok(())
}

impl HarnessStore for FsHarnessStore {
    fn read_manifest(&self, project_path: &str) -> Result<Option<String>, AppError> {
        read_regular(
            &Self::atlas(project_path).join("project.yaml"),
            MAX_MANIFEST_BYTES as u64,
        )
    }

    fn has_atlas_dir(&self, project_path: &str) -> bool {
        fs::symlink_metadata(Self::atlas(project_path)).is_ok_and(|m| m.is_dir())
    }

    fn read_context(&self, project_path: &str, name: &str) -> Option<String> {
        if !safe_relative(name) || name.contains('/') {
            return None;
        }
        let path = Self::atlas(project_path)
            .join("context")
            .join(format!("{name}.md"));
        read_regular(&path, MAX_CONTEXT_BYTES).ok().flatten()
    }

    fn read_file(&self, project_path: &str, path: &str) -> Result<Option<String>, AppError> {
        if !safe_relative(path) {
            return Err(AppError::new(ErrorCode::HarnessInvalid).with_detail("unsafe path"));
        }
        // No folder on the way may be a link either.
        let mut current = Self::atlas(project_path);
        if let Some(parent) = Path::new(path).parent() {
            for part in parent.components() {
                current.push(part);
                match fs::symlink_metadata(&current) {
                    Ok(m) if m.is_dir() => {}
                    Ok(_) => {
                        return Err(AppError::new(ErrorCode::HarnessInvalid)
                            .with_detail("not a plain folder"));
                    }
                    Err(_) => return Ok(None),
                }
            }
        }
        read_regular(&current.join(file_name(path)), MAX_KNOWLEDGE_READ)
    }

    fn write(&self, project_path: &str, files: &[HarnessFile]) -> Result<WriteReport, AppError> {
        let project = fs::canonicalize(project_path).map_err(|e| failed(e.to_string()))?;
        let atlas = project.join(ATLAS_DIR);
        ensure_dir(&project, Path::new(ATLAS_DIR))?;
        let backup_dir = format!("backups/{}", crate::application::support::now_ms());
        let mut report = WriteReport::default();

        for file in files {
            if !safe_relative(&file.path) {
                return Err(failed(format!("unsafe path {}", file.path)));
            }
            let target = atlas.join(&file.path);
            if let Some(parent) = Path::new(&file.path).parent() {
                ensure_dir(&atlas, parent)?;
            }
            match fs::symlink_metadata(&target) {
                Ok(meta) if !meta.is_file() => {
                    return Err(failed(format!("{} is not a regular file", file.path)));
                }
                Ok(_) if file.keep_if_exists => {
                    report.skipped.push(file.path.clone());
                    continue;
                }
                Ok(_) => {
                    let old = fs::read(&target).map_err(|e| failed(e.to_string()))?;
                    if old == file.content.as_bytes() {
                        continue;
                    }
                    let backup = format!("{backup_dir}/{}", file.path);
                    if let Some(parent) = Path::new(&backup).parent() {
                        ensure_dir(&atlas, parent)?;
                    }
                    fs::write(atlas.join(&backup), old).map_err(|e| failed(e.to_string()))?;
                    report.backed_up.push(file.path.clone());
                }
                Err(_) => {}
            }
            // Written beside the target and renamed, so a crash never leaves half a file.
            let temp = target.with_extension("atlas-tmp");
            fs::write(&temp, &file.content).map_err(|e| failed(e.to_string()))?;
            fs::rename(&temp, &target).map_err(|e| failed(e.to_string()))?;
            report.written.push(file.path.clone());
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("atlas-store-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::canonicalize(dir).unwrap()
    }

    fn file(path: &str, content: &str, keep: bool) -> HarnessFile {
        HarnessFile {
            path: path.to_owned(),
            content: content.to_owned(),
            keep_if_exists: keep,
        }
    }

    fn write(dir: &Path, files: &[HarnessFile]) -> Result<WriteReport, AppError> {
        FsHarnessStore.write(&dir.to_string_lossy(), files)
    }

    #[test]
    fn writes_files_under_atlas_and_reads_them_back() {
        let dir = project("write");
        let report = write(
            &dir,
            &[
                file("project.yaml", "version: 1\n", false),
                file("context/stack.md", "# Stack\n", false),
                file("memory/.gitkeep", "", true),
            ],
        )
        .unwrap();

        assert_eq!(report.written.len(), 3);
        assert!(dir.join(".atlas/memory/.gitkeep").is_file());
        let path = dir.to_string_lossy();
        assert!(FsHarnessStore.has_atlas_dir(&path));
        assert_eq!(
            FsHarnessStore.read_manifest(&path).unwrap().as_deref(),
            Some("version: 1\n")
        );
        assert_eq!(
            FsHarnessStore.read_context(&path, "stack").as_deref(),
            Some("# Stack\n")
        );
        assert_eq!(FsHarnessStore.read_context(&path, "../project"), None);
        // `.atlas/` is never added to .gitignore: it belongs to the project.
        assert!(!dir.join(".gitignore").exists());
    }

    #[test]
    fn replacing_a_file_keeps_the_old_one_as_a_backup_and_keep_if_exists_is_honoured() {
        let dir = project("backup");
        write(&dir, &[file("context/business.md", "mine", false)]).unwrap();

        let report = write(
            &dir,
            &[
                file("context/business.md", "placeholder", true),
                file("context/stack.md", "new", false),
            ],
        )
        .unwrap();
        assert_eq!(report.skipped, ["context/business.md"]);
        assert_eq!(
            fs::read_to_string(dir.join(".atlas/context/business.md")).unwrap(),
            "mine"
        );

        let report = write(&dir, &[file("context/business.md", "replacement", false)]).unwrap();
        assert_eq!(report.backed_up, ["context/business.md"]);
        let backups: Vec<_> = fs::read_dir(dir.join(".atlas/backups")).unwrap().collect();
        assert_eq!(backups.len(), 1);
        let backup = backups[0]
            .as_ref()
            .unwrap()
            .path()
            .join("context/business.md");
        assert_eq!(fs::read_to_string(backup).unwrap(), "mine");
        // Writing identical content again changes and backs up nothing.
        let report = write(&dir, &[file("context/business.md", "replacement", false)]).unwrap();
        assert!(report.written.is_empty() && report.backed_up.is_empty());
    }

    #[test]
    fn paths_that_climb_out_of_atlas_are_refused() {
        let dir = project("traversal");
        for bad in ["../evil.txt", "/etc/evil", "context/../../evil", ""] {
            let error = write(&dir, &[file(bad, "x", false)]).unwrap_err();
            assert_eq!(error.code, ErrorCode::HarnessGenerationFailed, "{bad}");
        }
        assert!(!dir.join("evil.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    fn nothing_is_written_through_a_symlink() {
        let dir = project("symlink");
        let outside = project("symlink-outside");
        fs::create_dir_all(dir.join(".atlas")).unwrap();
        std::os::unix::fs::symlink(&outside, dir.join(".atlas/context")).unwrap();

        let error = write(&dir, &[file("context/stack.md", "x", false)]).unwrap_err();

        assert_eq!(error.code, ErrorCode::HarnessGenerationFailed);
        assert!(!outside.join("stack.md").exists());

        // A manifest that is a link is not read either.
        let linked = project("symlink-manifest");
        fs::create_dir_all(linked.join(".atlas")).unwrap();
        fs::write(outside.join("real.yaml"), "version: 1").unwrap();
        std::os::unix::fs::symlink(
            outside.join("real.yaml"),
            linked.join(".atlas/project.yaml"),
        )
        .unwrap();
        assert!(FsHarnessStore
            .read_manifest(&linked.to_string_lossy())
            .is_err());
    }

    #[test]
    fn a_missing_atlas_folder_is_simply_not_initialized() {
        let dir = project("none");
        let path = dir.to_string_lossy();

        assert!(!FsHarnessStore.has_atlas_dir(&path));
        assert_eq!(FsHarnessStore.read_manifest(&path).unwrap(), None);
    }
}
