//! The configuration file a person exports to, or imports from, on their disk. The path always
//! comes from a native file dialog the person used, never from the webview; what is read is bounded
//! and what is written replaces the file only once it is complete.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::application::errors::{AppError, ErrorCode};

fn io(action: &str, error: &std::io::Error) -> AppError {
    AppError::new(ErrorCode::ConfigFileIo).with_detail(format!("{action}: {error}"))
}

/// Reads a text file of at most `max` bytes.
///
/// # Errors
///
/// Fails if the file cannot be read, is larger than `max` or is not UTF-8 text.
pub fn read_limited(path: &Path, max: usize) -> Result<String, AppError> {
    let file = File::open(path).map_err(|e| io("open", &e))?;
    let mut bytes = Vec::new();
    // One byte past the limit is enough to know the file is over it, without reading the rest.
    file.take(u64::try_from(max).unwrap_or(u64::MAX).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| io("read", &e))?;
    if bytes.len() > max {
        return Err(AppError::new(ErrorCode::ImportFileTooLarge)
            .with("max", (max / (1024 * 1024)).to_string()));
    }
    String::from_utf8(bytes).map_err(|_| AppError::new(ErrorCode::ImportNotAtlasFile))
}

/// Writes `text` to `path` through a temporary file next to it, so an interrupted write leaves
/// the file that was there (or nothing), never half of the new one.
///
/// # Errors
///
/// Fails if the folder is not writable or the file cannot be replaced.
pub fn write_atomic(path: &Path, text: &str) -> Result<(), AppError> {
    super::atomic_file::write(path, text.as_bytes()).map_err(|e| io("write", &e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn folder(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("atlas-config-file-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn what_is_written_is_what_is_read_and_replaces_what_was_there() {
        let dir = folder("roundtrip");
        let path = dir.join("a.atlas.json");

        write_atomic(&path, "first").unwrap();
        write_atomic(&path, "second ✓").unwrap();

        assert_eq!(read_limited(&path, 100).unwrap(), "second ✓");
        let leftovers: Vec<_> = fs::read_dir(&dir).unwrap().flatten().collect();
        assert_eq!(leftovers.len(), 1, "no temporary file stays behind");
    }

    #[test]
    fn a_file_over_the_limit_is_refused_and_a_binary_one_is_not_text() {
        let dir = folder("limits");
        let big = dir.join("big.json");
        fs::write(&big, vec![b'x'; 11]).unwrap();
        let binary = dir.join("bin.json");
        fs::write(&binary, [0xff, 0xfe, 0x00]).unwrap();

        assert!(read_limited(&big, 10)
            .unwrap_err()
            .is(ErrorCode::ImportFileTooLarge));
        assert!(read_limited(&big, 11).is_ok());
        assert!(read_limited(&binary, 10)
            .unwrap_err()
            .is(ErrorCode::ImportNotAtlasFile));
        assert!(read_limited(&dir.join("missing.json"), 10)
            .unwrap_err()
            .is(ErrorCode::ConfigFileIo));
    }

    #[test]
    fn writing_into_a_missing_folder_fails_cleanly() {
        let dir = folder("missing");

        let error = write_atomic(&dir.join("nope").join("a.json"), "x").unwrap_err();

        assert!(error.is(ErrorCode::ConfigFileIo));
    }
}
