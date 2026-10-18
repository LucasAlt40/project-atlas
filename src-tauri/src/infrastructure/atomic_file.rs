//! Replacing a file whole. Every store that rewrites a file goes through [`write`], so they all
//! give the same guarantees instead of each one getting a slightly different subset of them.

use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// The temporary file beside `path`: hidden, and named after the whole file name, so two files
/// of one folder (`stack.md`, `stack.yaml`) never share one.
fn temporary(path: &Path) -> io::Result<PathBuf> {
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the path has no file name"))?;
    let mut temp = OsString::new();
    if !name.to_string_lossy().starts_with('.') {
        temp.push(".");
    }
    temp.push(name);
    temp.push(".atlas-tmp");
    Ok(path.with_file_name(temp))
}

/// Writes `bytes` to `path` so that a reader, or a crash, finds the old file or the new one and
/// never half of one.
///
/// - The temporary file is always created new: a symlink at its name (a cloned repository can
///   carry one) is removed, never written through, so nothing outside the folder is touched.
/// - `path` itself is replaced, not written through: a symlink there is replaced by the file.
/// - The file keeps the permissions of the one it replaces (an executable stays executable, a
///   private file stays private), set before any of the new contents is written.
/// - The contents reach the disk before the rename; the folder is synced after it where the
///   platform allows, so the rename survives a power cut too.
/// - On failure the temporary file is removed and `path` is left as it was.
///
/// # Errors
///
/// Any I/O error on the way; `path` is then unchanged.
pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temp = temporary(path)?;
    // Left by a crash, or planted. Removing a link never touches what it points to.
    match fs::remove_file(&temp) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let written = write_new(&temp, path, bytes).and_then(|()| fs::rename(&temp, path));
    if let Err(error) = written {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    sync_folder(path);
    Ok(())
}

fn write_new(temp: &Path, path: &Path, bytes: &[u8]) -> io::Result<()> {
    // `create_new` fails on anything already there, a symlink included, instead of following it.
    let mut file = OpenOptions::new().write(true).create_new(true).open(temp)?;
    keep_permissions(&file, path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(unix)]
fn keep_permissions(file: &fs::File, path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() => file.set_permissions(meta.permissions()),
        _ => Ok(()),
    }
}

/// Windows only has the read-only flag, and a read-only target cannot be replaced anyway.
#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)] // the same signature as on Unix
fn keep_permissions(_file: &fs::File, _path: &Path) -> io::Result<()> {
    Ok(())
}

/// Best effort: a folder cannot be opened this way on every platform.
fn sync_folder(path: &Path) {
    #[cfg(unix)]
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        if let Ok(dir) = fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }
    #[cfg(not(unix))]
    let _ = path;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("atlas-atomic-file-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_file_is_created_then_replaced_and_no_temporary_file_stays() {
        let dir = folder("replace");
        let path = dir.join("a.json");

        write(&path, b"first").unwrap();
        write(&path, b"second").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"second");
        assert_eq!(names(&dir), vec!["a.json"]);
    }

    #[test]
    fn two_files_with_the_same_stem_do_not_share_a_temporary_file() {
        let a = temporary(Path::new("/p/stack.md")).unwrap();
        let b = temporary(Path::new("/p/stack.yaml")).unwrap();
        assert_ne!(a, b);
        assert_eq!(
            temporary(Path::new("/p/.gitignore")).unwrap(),
            Path::new("/p/.gitignore.atlas-tmp")
        );
    }

    #[test]
    fn a_failed_write_leaves_nothing_and_reports_the_error() {
        let dir = folder("missing");
        assert!(write(&dir.join("nope").join("a.json"), b"x").is_err());
        assert_eq!(names(&dir), Vec::<String>::new());
    }

    #[test]
    fn a_leftover_temporary_file_from_a_crash_does_not_block_the_next_write() {
        let dir = folder("leftover");
        let path = dir.join("a.json");
        fs::write(temporary(&path).unwrap(), b"half").unwrap();

        write(&path, b"whole").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"whole");
        assert_eq!(names(&dir), vec!["a.json"]);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_planted_at_the_temporary_name_is_never_written_through() {
        let dir = folder("planted");
        let outside = folder("planted-outside").join("victim");
        fs::write(&outside, b"precious").unwrap();
        let path = dir.join("stack.md");
        std::os::unix::fs::symlink(&outside, temporary(&path).unwrap()).unwrap();

        write(&path, b"generated").unwrap();

        assert_eq!(fs::read(&outside).unwrap(), b"precious");
        assert_eq!(fs::read(&path).unwrap(), b"generated");
        assert!(!fs::symlink_metadata(&path).unwrap().is_symlink());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_at_the_target_is_replaced_not_followed() {
        let dir = folder("target-link");
        let outside = folder("target-link-outside").join("victim");
        fs::write(&outside, b"precious").unwrap();
        let path = dir.join("a.json");
        std::os::unix::fs::symlink(&outside, &path).unwrap();

        write(&path, b"new").unwrap();

        assert_eq!(fs::read(&outside).unwrap(), b"precious");
        assert!(!fs::symlink_metadata(&path).unwrap().is_symlink());
    }

    #[cfg(unix)]
    #[test]
    fn the_replaced_file_keeps_its_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = folder("permissions");
        let script = dir.join("run.sh");
        let secret = dir.join("secret.json");
        fs::write(&script, b"#!/bin/sh").unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(&secret, b"{}").unwrap();
        fs::set_permissions(&secret, fs::Permissions::from_mode(0o600)).unwrap();

        write(&script, b"#!/bin/sh\necho hi").unwrap();
        write(&secret, b"{\"token\":\"t\"}").unwrap();

        let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&script), 0o755);
        assert_eq!(mode(&secret), 0o600);
    }
}
