use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use crate::application::specs::framework::ChangeFolder;
use crate::application::specs::ports::{FilesError, ProjectFiles};
use crate::domain::spec::SpecFile;

/// Most a single file is read for, and how much of a project's specs is read at all.
const MAX_FILE_BYTES: u64 = 512 * 1024;
const MAX_MAIN_SPECS_BYTES: u64 = 2 * 1024 * 1024;
const MAX_CHANGE_FILES: usize = 200;
const MAX_DEPTH: usize = 6;

/// The project's `openspec/` folder on disk. Every path is a plain relative path under the
/// project root; nothing is written through a symlink; a file is created, never overwritten, and
/// replaced only through a temporary file in the same folder.
pub struct FsProjectFiles;

fn plain_relative(relative: &str) -> Result<PathBuf, FilesError> {
    let path = Path::new(relative);
    let plain = !relative.is_empty()
        && path.components().all(|c| matches!(c, Component::Normal(_)))
        && !relative.contains('\\');
    if plain {
        Ok(path.to_path_buf())
    } else {
        Err(FilesError::Unsafe(relative.to_owned()))
    }
}

fn root_of(root: &str) -> Result<PathBuf, FilesError> {
    let path = PathBuf::from(root);
    if path.is_absolute() && path.is_dir() {
        Ok(path)
    } else {
        Err(FilesError::NoProject)
    }
}

/// Creates the folders down to the file's parent, refusing to pass through a symlink.
fn ensure_parent(root: &Path, relative: &Path) -> Result<PathBuf, FilesError> {
    let mut current = root.to_path_buf();
    let parent = relative.parent().unwrap_or_else(|| Path::new(""));
    for part in parent.components() {
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(FilesError::Unsafe(current.display().to_string()));
            }
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => return Err(FilesError::Unsafe(current.display().to_string())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current).map_err(|e| FilesError::Io(e.to_string()))?;
            }
            Err(e) => return Err(FilesError::Io(e.to_string())),
        }
    }
    Ok(root.join(relative))
}

fn read_regular(path: &Path) -> Option<String> {
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
        return None;
    }
    fs::read_to_string(path).ok()
}

/// Every regular file under `dir`, as paths relative to it, without following symlinks.
fn walk(dir: &Path, prefix: &str, depth: usize, out: &mut Vec<(String, PathBuf)>) {
    if depth > MAX_DEPTH || out.len() >= MAX_CHANGE_FILES {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(meta) = fs::symlink_metadata(entry.path()) else {
            continue;
        };
        let relative = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        if meta.is_dir() {
            walk(&entry.path(), &relative, depth + 1, out);
        } else if meta.is_file() {
            out.push((relative, entry.path()));
        }
    }
}

impl ProjectFiles for FsProjectFiles {
    fn main_specs(&self, root: &str) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        let Ok(root) = root_of(root) else {
            return out;
        };
        let base = root.join("openspec").join("specs");
        let mut files = Vec::new();
        walk(&base, "", 0, &mut files);
        let mut total = 0u64;
        for (relative, path) in files {
            let Some(capability) = relative.strip_suffix("/spec.md") else {
                continue;
            };
            let Some(content) = read_regular(&path) else {
                continue;
            };
            total += content.len() as u64;
            if total > MAX_MAIN_SPECS_BYTES {
                break;
            }
            out.insert(capability.to_owned(), content);
        }
        out
    }

    fn change_ids(&self, root: &str, changes_dir: &str) -> Vec<String> {
        let Ok(root) = root_of(root) else {
            return Vec::new();
        };
        let Ok(relative) = plain_relative(changes_dir) else {
            return Vec::new();
        };
        let Ok(entries) = fs::read_dir(root.join(relative)) else {
            return Vec::new();
        };
        let mut ids: Vec<String> = entries
            .flatten()
            .filter(|e| fs::symlink_metadata(e.path()).is_ok_and(|m| m.is_dir() && !m.is_symlink()))
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| name != "archive" && !name.starts_with('.'))
            .collect();
        ids.sort();
        ids
    }

    fn change_exists(&self, root: &str, changes_dir: &str, id: &str) -> bool {
        let (Ok(root), Ok(relative)) = (
            root_of(root),
            plain_relative(&format!("{changes_dir}/{id}")),
        ) else {
            return false;
        };
        fs::symlink_metadata(root.join(relative)).is_ok()
    }

    fn read_change(
        &self,
        root: &str,
        changes_dir: &str,
        id: &str,
    ) -> Result<ChangeFolder, FilesError> {
        let root = root_of(root)?;
        let relative = plain_relative(&format!("{changes_dir}/{id}"))?;
        let base = root.join(relative);
        let meta = fs::symlink_metadata(&base).map_err(|e| FilesError::Io(e.to_string()))?;
        if !meta.is_dir() || meta.is_symlink() {
            return Err(FilesError::Unsafe(base.display().to_string()));
        }
        let mut found = Vec::new();
        walk(&base, "", 0, &mut found);
        let files = found
            .into_iter()
            .filter(|(relative, _)| {
                Path::new(relative)
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("md"))
                    || relative == ".openspec.yaml"
            })
            .filter_map(|(relative, path)| {
                read_regular(&path).map(|content| SpecFile {
                    path: relative,
                    content,
                })
            })
            .collect();
        Ok(ChangeFolder { files })
    }

    fn read(&self, root: &str, relative: &str) -> Option<String> {
        let root = root_of(root).ok()?;
        let relative = plain_relative(relative).ok()?;
        read_regular(&root.join(relative))
    }

    fn create(&self, root: &str, relative: &str, content: &str) -> Result<(), FilesError> {
        let root = root_of(root)?;
        let relative = plain_relative(relative)?;
        let target = ensure_parent(&root, &relative)?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::AlreadyExists => FilesError::Exists,
                _ => FilesError::Io(e.to_string()),
            })?;
        file.write_all(content.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|e| FilesError::Io(e.to_string()))
    }

    fn replace(&self, root: &str, relative: &str, content: &str) -> Result<(), FilesError> {
        let root = root_of(root)?;
        let relative = plain_relative(relative)?;
        let target = ensure_parent(&root, &relative)?;
        if fs::symlink_metadata(&target).is_ok_and(|m| m.is_symlink()) {
            return Err(FilesError::Unsafe(target.display().to_string()));
        }
        let temp = target.with_extension("atlas-tmp");
        fs::write(&temp, content).map_err(|e| FilesError::Io(e.to_string()))?;
        fs::rename(&temp, &target).map_err(|e| {
            let _ = fs::remove_file(&temp);
            FilesError::Io(e.to_string())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(name: &str) -> String {
        let dir = std::env::temp_dir().join(format!("atlas-pf-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.to_string_lossy().into_owned()
    }

    #[test]
    fn a_file_is_created_with_its_folders_and_never_overwritten() {
        let root = project("create");
        let files = FsProjectFiles;

        files
            .create(&root, "openspec/changes/a/proposal.md", "first")
            .unwrap();

        assert_eq!(
            files.create(&root, "openspec/changes/a/proposal.md", "second"),
            Err(FilesError::Exists)
        );
        assert_eq!(
            files
                .read(&root, "openspec/changes/a/proposal.md")
                .as_deref(),
            Some("first")
        );
    }

    #[test]
    fn a_path_that_climbs_out_or_is_absolute_is_refused() {
        let root = project("unsafe");
        let files = FsProjectFiles;

        for path in ["../x.md", "/etc/x.md", "a/../../x.md", "a\\b.md", ""] {
            assert!(
                matches!(files.create(&root, path, "x"), Err(FilesError::Unsafe(_))),
                "{path}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn nothing_is_written_through_a_symlink() {
        let root = project("symlink");
        let outside = project("symlink-outside");
        fs::create_dir_all(Path::new(&root).join("openspec")).unwrap();
        std::os::unix::fs::symlink(&outside, Path::new(&root).join("openspec/changes")).unwrap();
        let files = FsProjectFiles;

        let result = files.create(&root, "openspec/changes/a/proposal.md", "x");

        assert!(matches!(result, Err(FilesError::Unsafe(_))));
        assert!(fs::read_dir(&outside).unwrap().next().is_none());
    }

    #[test]
    fn a_missing_project_folder_is_said_so() {
        let files = FsProjectFiles;

        assert_eq!(
            files.create("/definitely/not/here", "a.md", "x"),
            Err(FilesError::NoProject)
        );
    }

    #[test]
    fn replacing_a_file_is_whole_or_not_at_all() {
        let root = project("replace");
        let files = FsProjectFiles;
        files.create(&root, "a/b.md", "old").unwrap();

        files.replace(&root, "a/b.md", "new").unwrap();

        assert_eq!(files.read(&root, "a/b.md").as_deref(), Some("new"));
        assert!(!Path::new(&root).join("a/b.atlas-tmp").exists());
    }

    #[test]
    fn changes_are_listed_without_the_archive_and_read_back() {
        let root = project("changes");
        let files = FsProjectFiles;
        files
            .create(&root, "openspec/changes/add-x/proposal.md", "# P")
            .unwrap();
        files
            .create(
                &root,
                "openspec/changes/add-x/.openspec.yaml",
                "schema: spec-driven\n",
            )
            .unwrap();
        files
            .create(&root, "openspec/changes/add-x/specs/cap/spec.md", "# S")
            .unwrap();
        files
            .create(&root, "openspec/changes/add-x/image.png", "bin")
            .unwrap();
        files
            .create(
                &root,
                "openspec/changes/archive/2026-01-01-old/proposal.md",
                "# O",
            )
            .unwrap();

        assert_eq!(
            files.change_ids(&root, "openspec/changes"),
            vec!["add-x".to_owned()]
        );
        assert!(files.change_exists(&root, "openspec/changes", "add-x"));
        assert!(!files.change_exists(&root, "openspec/changes", "other"));
        let folder = files
            .read_change(&root, "openspec/changes", "add-x")
            .unwrap();
        let mut paths: Vec<&str> = folder.files.iter().map(|f| f.path.as_str()).collect();
        paths.sort_unstable();
        assert_eq!(
            paths,
            vec![".openspec.yaml", "proposal.md", "specs/cap/spec.md"]
        );
    }

    #[test]
    fn the_main_specs_are_read_by_capability() {
        let root = project("main");
        let files = FsProjectFiles;
        files
            .create(&root, "openspec/specs/auth/spec.md", "## Requirements")
            .unwrap();
        files
            .create(
                &root,
                "openspec/specs/identity/sso/spec.md",
                "## Requirements",
            )
            .unwrap();

        let specs = files.main_specs(&root);

        assert_eq!(
            specs.keys().cloned().collect::<Vec<_>>(),
            vec!["auth".to_owned(), "identity/sso".to_owned()]
        );
    }
}
