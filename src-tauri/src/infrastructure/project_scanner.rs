use std::collections::VecDeque;
use std::fs;
use std::path::{Component, Path};

use crate::application::errors::{AppError, ErrorCode};
use crate::application::harness::fingerprint::{digest_text, is_relevant_file, MAX_HASHED_BYTES};
use crate::application::harness::sampler::looks_secret_path;
use crate::application::harness::snapshot::{ScanEntry, ScanSnapshot, IGNORED_DIRS};
use crate::application::harness::ProjectScanner;

const MAX_HEAD_BYTES: u64 = 1024;

/// The scan's bounds. They are internal knobs, not user settings; hitting `max_entries` makes
/// the analysis partial.
#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_field_names)]
pub struct ScanLimits {
    pub max_depth: usize,
    pub max_entries: usize,
    pub max_file_bytes: u64,
    pub max_files_read: usize,
}

impl ScanLimits {
    pub const DEFAULT: Self = Self {
        max_depth: 4,
        max_entries: 5_000,
        max_file_bytes: 256 * 1024,
        max_files_read: 60,
    };
}

impl Default for ScanLimits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Manifests whose text the analyzer may look at. Exact names (case-insensitive) or extensions.
const READABLE_NAMES: &[&str] = &[
    "package.json",
    "cargo.toml",
    "pyproject.toml",
    "requirements.txt",
    "go.mod",
    "pom.xml",
    "global.json",
    "build.gradle",
    "build.gradle.kts",
    "settings.gradle",
    "settings.gradle.kts",
    "docker-compose.yml",
    "docker-compose.yaml",
    "compose.yml",
    "compose.yaml",
];
const READABLE_EXTENSIONS: &[&str] = &["csproj", "fsproj"];
/// How deep a manifest's text is read: root, and the folders one and two levels down.
const READ_DEPTH: usize = 3;

/// Reads names and a few manifests. It never follows a symlink (so it cannot leave the project
/// root), never reads `.env` or anything not listed above, and never runs, builds or installs
/// anything. The depth limit is by design (only structure near the root matters), so it is not
/// "partial"; running out of entries is, because then whole folders were not seen.
#[derive(Default)]
pub struct FsProjectScanner {
    limits: ScanLimits,
}

impl FsProjectScanner {}

impl ProjectScanner for FsProjectScanner {
    fn scan(&self, project_path: &str) -> Result<ScanSnapshot, AppError> {
        let path = Path::new(project_path);
        if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
            return Err(AppError::new(ErrorCode::UnsafeProjectPath).with("path", project_path));
        }
        let root = match fs::canonicalize(path) {
            Ok(root) => root,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(AppError::new(ErrorCode::ProjectNotFound).with("path", project_path));
            }
            Err(e) => {
                return Err(AppError::new(ErrorCode::ProjectAnalysisFailed)
                    .with("path", project_path)
                    .with_detail(e.to_string()));
            }
        };
        if !root.is_dir() {
            return Err(AppError::new(ErrorCode::ProjectNotDirectory).with("path", project_path));
        }

        let limits = self.limits;
        let mut snapshot = ScanSnapshot {
            root_name: root.file_name().map_or_else(
                || project_path.to_owned(),
                |n| n.to_string_lossy().into_owned(),
            ),
            ..ScanSnapshot::default()
        };
        let mut read = 0;
        let mut queue: VecDeque<(String, usize)> = VecDeque::from([(String::new(), 0)]);
        while let Some((relative, depth)) = queue.pop_front() {
            let dir = if relative.is_empty() {
                root.clone()
            } else {
                root.join(&relative)
            };
            let Ok(read_dir) = fs::read_dir(&dir) else {
                continue;
            };
            let mut children: Vec<_> = read_dir.filter_map(Result::ok).collect();
            children.sort_by_key(fs::DirEntry::file_name);
            for child in children {
                if snapshot.entries.len() >= limits.max_entries {
                    snapshot.truncated = true;
                    return Ok(snapshot);
                }
                let name = child.file_name().to_string_lossy().into_owned();
                let rel = if relative.is_empty() {
                    name.clone()
                } else {
                    format!("{relative}/{name}")
                };
                // `file_type` does not follow symlinks: a link is neither file nor folder here.
                let Ok(kind) = child.file_type() else {
                    continue;
                };
                if kind.is_symlink() {
                    continue;
                }
                if kind.is_dir() {
                    snapshot.entries.push(ScanEntry {
                        path: rel.clone(),
                        is_dir: true,
                    });
                    if IGNORED_DIRS.contains(&name.as_str()) {
                        if name == ".git" && depth == 0 {
                            snapshot.git_head = read_head(&child.path());
                        }
                    } else if depth + 1 < limits.max_depth {
                        queue.push_back((rel, depth + 1));
                    }
                } else if kind.is_file() {
                    let content =
                        (depth < READ_DEPTH && read < limits.max_files_read && readable(&name))
                            .then(|| read_small(&child.path(), limits.max_file_bytes))
                            .flatten();
                    if content.is_some() {
                        read += 1;
                    } else if is_relevant_file(&rel) {
                        // Lockfiles, configuration, CI and docs are not analysed, only digested
                        // so a later check can tell whether they changed.
                        if let Some(hash) = hash_file(&child.path()) {
                            snapshot.hashes.insert(rel.clone(), hash);
                        }
                    }
                    snapshot.entries.push(ScanEntry {
                        path: rel.clone(),
                        is_dir: false,
                    });
                    if let Some(content) = content {
                        snapshot.files.insert(rel, content);
                    }
                }
            }
        }
        Ok(snapshot)
    }

    fn exists(&self, project_path: &str, relative: &str) -> bool {
        let relative = relative.trim_end_matches('/');
        let Ok(root) = fs::canonicalize(project_path) else {
            return false;
        };
        let safe = !relative.is_empty()
            && Path::new(relative)
                .components()
                .all(|c| matches!(c, Component::Normal(_)));
        safe && !ancestors_are_links(&root, relative)
            && fs::symlink_metadata(root.join(relative)).is_ok_and(|m| m.is_file() || m.is_dir())
    }

    fn read_files(
        &self,
        project_path: &str,
        paths: &[String],
        max_bytes: usize,
    ) -> Vec<(String, String)> {
        let Ok(root) = fs::canonicalize(project_path) else {
            return Vec::new();
        };
        paths
            .iter()
            .filter(|p| {
                !p.is_empty()
                    && !looks_secret_path(p)
                    && Path::new(p.as_str())
                        .components()
                        .all(|c| matches!(c, Component::Normal(_)))
            })
            .filter_map(|relative| {
                let path = root.join(relative);
                // Regular files only, never through a link, and only the first bytes.
                let meta = fs::symlink_metadata(&path).ok()?;
                if !meta.is_file() || ancestors_are_links(&root, relative) {
                    return None;
                }
                let mut bytes =
                    vec![0; max_bytes.min(usize::try_from(meta.len()).unwrap_or(usize::MAX))];
                std::io::Read::read_exact(&mut fs::File::open(&path).ok()?, &mut bytes).ok()?;
                Some((
                    relative.clone(),
                    String::from_utf8_lossy(&bytes).into_owned(),
                ))
            })
            .collect()
    }
}

/// Is any folder on the way to `relative` a symlink? (The file itself was checked already.)
fn ancestors_are_links(root: &Path, relative: &str) -> bool {
    let mut current = root.to_path_buf();
    let parts: Vec<&str> = relative.split('/').collect();
    for part in &parts[..parts.len().saturating_sub(1)] {
        current.push(part);
        if fs::symlink_metadata(&current).map_or(true, |m| m.file_type().is_symlink()) {
            return true;
        }
    }
    false
}

fn readable(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    READABLE_NAMES.contains(&lower.as_str())
        || lower
            .rsplit_once('.')
            .is_some_and(|(_, ext)| READABLE_EXTENSIONS.contains(&ext))
}

/// Only regular files (checked without following links) up to a size cap, as UTF-8 text.
fn read_small(path: &Path, max_bytes: u64) -> Option<String> {
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.is_file() || meta.len() > max_bytes {
        return None;
    }
    fs::read_to_string(path).ok()
}

/// A digest of the first bytes of a regular file (checked without following links).
fn hash_file(path: &Path) -> Option<String> {
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(
        &mut std::io::Read::take(fs::File::open(path).ok()?, MAX_HASHED_BYTES as u64),
        &mut bytes,
    )
    .ok()?;
    Some(digest_text(&String::from_utf8_lossy(&bytes)))
}

/// The first line of `.git/HEAD`. `.git` was listed as a real folder, not a link.
fn read_head(git_dir: &Path) -> Option<String> {
    let head = git_dir.join("HEAD");
    let meta = fs::symlink_metadata(&head).ok()?;
    if !meta.is_file() || meta.len() > MAX_HEAD_BYTES {
        return None;
    }
    fs::read_to_string(head)
        .ok()
        .and_then(|t| t.lines().next().map(str::to_owned))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::harness::analyzer::DeterministicAnalyzer;

    fn project(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("atlas-scan-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for (file, content) in files {
            let path = dir.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
        fs::canonicalize(dir).unwrap()
    }

    fn scan(dir: &Path) -> ScanSnapshot {
        FsProjectScanner::default()
            .scan(&dir.to_string_lossy())
            .unwrap()
    }

    #[test]
    fn reads_structure_and_manifests_and_finds_the_stack() {
        let dir = project(
            "angular",
            &[
                (".git/HEAD", "ref: refs/heads/main\n"),
                (
                    "package.json",
                    r#"{"dependencies":{"@angular/core":"^21.0.0"}}"#,
                ),
                ("angular.json", "{}"),
                (".github/workflows/ci.yml", "on: push"),
                ("src/app/app.ts", "export {}"),
            ],
        );

        let snapshot = scan(&dir);
        let findings = DeterministicAnalyzer::default().analyze(&snapshot);

        assert_eq!(snapshot.git_head.as_deref(), Some("ref: refs/heads/main"));
        assert!(!snapshot.truncated);
        for id in [
            "framework:angular",
            "repository:git",
            "repository:current_branch",
            "ci:github_actions",
        ] {
            assert!(findings.iter().any(|f| f.id == id), "missing {id}");
        }
        // Source files are listed by name but never read.
        assert!(!snapshot.files.contains_key("src/app/app.ts"));
    }

    #[test]
    fn generated_folders_are_noted_but_never_entered() {
        let dir = project(
            "ignored",
            &[
                ("package.json", "{}"),
                ("node_modules/pkg/package.json", "{}"),
                ("target/debug/Cargo.toml", ""),
                ("dist/app.js", ""),
                (".atlas/project.yaml", "x"),
            ],
        );

        let snapshot = scan(&dir);

        assert!(snapshot.has_dir("node_modules"));
        assert!(!snapshot.has_path("node_modules/pkg"));
        assert!(!snapshot.has_path("target/debug"));
        assert!(!snapshot.has_path("dist/app.js"));
        assert!(!snapshot.has_path(".atlas/project.yaml"));
    }

    #[test]
    fn env_files_and_unlisted_files_are_never_read() {
        let dir = project(
            "env",
            &[
                (".env", "API_KEY=super-secret-value"),
                (".env.local", "TOKEN=another-secret"),
                ("config/secrets.json", r#"{"password":"x"}"#),
                ("package.json", "{}"),
            ],
        );

        let snapshot = scan(&dir);
        let findings = DeterministicAnalyzer::default().analyze(&snapshot);

        assert!(snapshot.has_path(".env"));
        assert!(snapshot.files.keys().all(|k| k == "package.json"));
        let dump = format!("{snapshot:?}{findings:?}");
        assert!(!dump.contains("super-secret-value") && !dump.contains("another-secret"));
        assert!(findings.iter().any(|f| f.id == "environment:env_file"));
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_not_followed_so_the_scan_cannot_leave_the_root() {
        let outside = project(
            "outside",
            &[("package.json", r#"{"dependencies":{"react":"1"}}"#)],
        );
        let dir = project("links", &[("a.txt", "")]);
        std::os::unix::fs::symlink(&outside, dir.join("escape")).unwrap();
        std::os::unix::fs::symlink(outside.join("package.json"), dir.join("package.json")).unwrap();

        let snapshot = scan(&dir);

        assert!(!snapshot.has_path("escape"));
        assert!(!snapshot.has_path("escape/package.json"));
        assert!(!snapshot.has_path("package.json"));
        assert!(snapshot.files.is_empty());
        let findings = DeterministicAnalyzer::default().analyze(&snapshot);
        assert!(findings.iter().all(|f| f.id != "framework:react"));
    }

    #[test]
    fn unsafe_missing_and_non_directory_paths_are_refused_with_specific_errors() {
        let dir = project("paths", &[("file.txt", "")]);
        let code = |path: &str| FsProjectScanner::default().scan(path).unwrap_err().code;

        assert_eq!(code("relative/dir"), ErrorCode::UnsafeProjectPath);
        assert_eq!(
            code(&format!("{}/../etc", dir.display())),
            ErrorCode::UnsafeProjectPath
        );
        assert_eq!(code("/definitely/not/here"), ErrorCode::ProjectNotFound);
        assert_eq!(
            code(&dir.join("file.txt").to_string_lossy()),
            ErrorCode::ProjectNotDirectory
        );
    }

    #[test]
    fn huge_folders_are_reported_as_partial_but_a_deep_tree_alone_is_not() {
        let deep = project("deep", &[("a/b/c/d/e/package.json", "{}")]);
        let snapshot = scan(&deep);
        assert!(!snapshot.truncated);
        assert!(!snapshot.has_path("a/b/c/d/e"));

        let many: Vec<(String, &str)> = (0..ScanLimits::DEFAULT.max_entries + 10)
            .map(|i| (format!("f/{i}.txt"), ""))
            .collect();
        let refs: Vec<(&str, &str)> = many.iter().map(|(p, c)| (p.as_str(), *c)).collect();
        let wide = project("wide", &refs);
        let snapshot = scan(&wide);
        assert!(snapshot.truncated);
        assert!(snapshot.entries.len() <= ScanLimits::DEFAULT.max_entries);
    }

    #[test]
    fn an_empty_or_unknown_project_still_analyses_without_inventing() {
        let dir = project("unknown", &[("notes.txt", "hello")]);

        let findings = DeterministicAnalyzer::default().analyze(&scan(&dir));

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].id, "architecture:unknown");
    }

    #[test]
    fn the_fingerprint_digests_lockfiles_and_ignores_secrets_generated_folders_and_links() {
        use crate::application::harness::fingerprint::compute;
        let base = [
            ("package.json", r#"{"name":"x"}"#),
            ("package-lock.json", "{\"lock\":1}"),
            ("src/domain/a.ts", "export {}"),
        ];
        let dir = project("fp-a", &base);
        let before = compute(&scan(&dir));
        assert!(before.parts.contains_key("package-lock.json"));

        // Secrets, generated folders and ordinary source edits are not part of it.
        fs::write(dir.join(".env"), "TOKEN=1").unwrap();
        fs::create_dir_all(dir.join("node_modules/x")).unwrap();
        fs::write(dir.join("node_modules/x/package.json"), "{}").unwrap();
        fs::write(dir.join("src/domain/a.ts"), "export const a = 1").unwrap();
        assert_eq!(before, compute(&scan(&dir)));

        // A link, even to a manifest outside the project, is not read and changes nothing.
        #[cfg(unix)]
        {
            let outside = project("fp-out", &[("Cargo.toml", "[package]")]);
            std::os::unix::fs::symlink(outside.join("Cargo.toml"), dir.join("Cargo.toml")).unwrap();
            assert_eq!(before, compute(&scan(&dir)));
        }

        // A lockfile change is.
        fs::write(dir.join("package-lock.json"), "{\"lock\":2}").unwrap();
        assert_ne!(before.digest, compute(&scan(&dir)).digest);
    }
}

#[cfg(test)]
mod read_tests {
    use super::*;

    fn project(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("atlas-read-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for (file, content) in files {
            let path = dir.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
        fs::canonicalize(dir).unwrap()
    }

    fn read(dir: &Path, paths: &[&str], max: usize) -> Vec<(String, String)> {
        let paths: Vec<String> = paths.iter().map(|p| (*p).to_owned()).collect();
        FsProjectScanner::default().read_files(&dir.to_string_lossy(), &paths, max)
    }

    #[test]
    fn reads_the_first_bytes_of_regular_files_only() {
        let dir = project(
            "regular",
            &[("README.md", "0123456789"), ("src/a.ts", "abc")],
        );

        let got = read(&dir, &["README.md", "src/a.ts", "missing.md"], 4);

        assert_eq!(
            got,
            [
                ("README.md".to_owned(), "0123".to_owned()),
                ("src/a.ts".to_owned(), "abc".to_owned())
            ]
        );
    }

    #[test]
    fn secrets_traversal_absolute_paths_and_directories_are_never_read() {
        let dir = project(
            "unsafe",
            &[(".env", "TOKEN=abc"), ("key.pem", "x"), ("a/b.txt", "ok")],
        );
        fs::write(dir.parent().unwrap().join("outside-secret.txt"), "outside").unwrap();

        let got = read(
            &dir,
            &[
                ".env",
                "key.pem",
                "../outside-secret.txt",
                "/etc/passwd",
                "a",
                "",
            ],
            100,
        );

        assert_eq!(got.len(), 0);
        assert_eq!(read(&dir, &["a/b.txt"], 100).len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_files_and_folders_are_not_followed() {
        let outside = project(
            "read-outside",
            &[("secret.txt", "outside"), ("d/x.txt", "outside")],
        );
        let dir = project("read-links", &[("real.txt", "ok")]);
        std::os::unix::fs::symlink(outside.join("secret.txt"), dir.join("link.txt")).unwrap();
        std::os::unix::fs::symlink(outside.join("d"), dir.join("linked")).unwrap();

        let got = read(&dir, &["link.txt", "linked/x.txt", "real.txt"], 100);

        assert_eq!(got, [("real.txt".to_owned(), "ok".to_owned())]);
    }
}
