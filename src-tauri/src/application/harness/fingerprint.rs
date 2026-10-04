//! The Project Fingerprint: a cheap, deterministic digest of what Harness knowledge depends on,
//! and the comparison that tells whether the Harness may be out of date.
//!
//! The question is not "did any byte change?" but "did the project change in a way that could
//! make what Atlas knows wrong?". So only these *relevant* parts count:
//!
//! - **manifests and lockfiles** (`package.json`, `Cargo.toml`, `Cargo.lock`, `go.mod`, ...);
//! - **configuration** (`tsconfig*.json`, `vite.config.*`, `angular.json`, `tauri.conf.json`,
//!   linters, `rust-toolchain*`, ...);
//! - **CI and containers** (`.github/workflows/*`, `.gitlab-ci.yml`, `Dockerfile`, compose);
//! - **documentation** (`README*`);
//! - **structure**: the existence of folders up to [`STRUCTURE_DEPTH`] levels deep.
//!
//! Editing an ordinary source file (`src/components/Button.tsx`) is *not* relevant on its own:
//! it is not a part. Adding or removing a folder is, because architecture and modules are
//! concluded from the folder layout.
//!
//! Each part is digested on its own, so a later check can name what changed. Nothing here reads
//! secrets (`.env`, keys), follows links or runs anything: it works on a [`ScanSnapshot`].

use std::collections::BTreeMap;
use std::fmt::Write;

use super::snapshot::{depth, file_name, ScanSnapshot};
use crate::domain::harness::{
    AnalysisInfo, ChangeKind, ProjectFingerprint, RelevantChange, Staleness, MAX_LISTED_CHANGES,
};

/// Folders count down to this depth (`src/features/harness` is 2).
pub const STRUCTURE_DEPTH: usize = 2;
/// Files count down to this depth.
const FILE_DEPTH: usize = 3;
/// Marks a folder part, so it cannot collide with a file path.
const DIR_PREFIX: &str = "dir:";
/// Only the first bytes of a large file are digested.
pub const MAX_HASHED_BYTES: usize = 4 * 1024 * 1024;

const EXACT_NAMES: &[&str] = &[
    "package.json",
    "package-lock.json",
    "npm-shrinkwrap.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "bun.lockb",
    "cargo.toml",
    "cargo.lock",
    "rust-toolchain",
    "rust-toolchain.toml",
    "pyproject.toml",
    "requirements.txt",
    "poetry.lock",
    "go.mod",
    "go.sum",
    "pom.xml",
    "global.json",
    "build.gradle",
    "build.gradle.kts",
    "settings.gradle",
    "settings.gradle.kts",
    "angular.json",
    "nx.json",
    "turbo.json",
    "tauri.conf.json",
    "docker-compose.yml",
    "docker-compose.yaml",
    "compose.yml",
    "compose.yaml",
    "dockerfile",
    ".gitlab-ci.yml",
    "azure-pipelines.yml",
    ".editorconfig",
    "directory.build.props",
    "readme",
    "readme.md",
];
const PREFIXES: &[&str] = &[
    "tsconfig",
    "jsconfig",
    "vite.config.",
    "vitest.config.",
    "jest.config.",
    "webpack.config.",
    "next.config.",
    "nuxt.config.",
    "eslint.config.",
    ".eslintrc",
    "prettier.config.",
    ".prettierrc",
    "readme.",
    "dockerfile.",
];
const EXTENSIONS: &[&str] = &["csproj", "fsproj", "sln"];

/// Whether a file is a part of the fingerprint. Never true for anything secret.
pub fn is_relevant_file(path: &str) -> bool {
    if depth(path) > FILE_DEPTH || super::sampler::looks_secret_path(path) {
        return false;
    }
    let name = file_name(path).to_ascii_lowercase();
    if path.starts_with(".github/workflows/") || path.starts_with("docker/") {
        return true;
    }
    EXACT_NAMES.contains(&name.as_str())
        || PREFIXES.iter().any(|p| name.starts_with(p))
        || name
            .rsplit_once('.')
            .is_some_and(|(_, ext)| EXTENSIONS.contains(&ext))
}

/// FNV-1a, 64 bits: stable across platforms and Rust versions (unlike `DefaultHasher`), cheap,
/// and enough to notice change. It is not a security boundary.
pub fn digest(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Line endings do not make a different project: `\r\n` and `\n` digest the same.
pub fn digest_text(text: &str) -> String {
    digest(text.replace("\r\n", "\n").as_bytes())
}

/// The fingerprint of what a scan saw.
pub fn compute(snapshot: &ScanSnapshot) -> ProjectFingerprint {
    let mut parts = BTreeMap::new();
    for entry in &snapshot.entries {
        let path = entry.path.as_str();
        if entry.is_dir {
            // `.git` and friends are listed but never relevant; generated folders are not
            // entered, and are not structure either.
            if depth(path) < STRUCTURE_DEPTH
                && !is_ignored_dir(file_name(path))
                && !path.split('/').any(is_ignored_dir)
            {
                parts.insert(format!("{DIR_PREFIX}{path}"), "dir".to_owned());
            }
        } else if is_relevant_file(path) {
            let value = snapshot
                .files
                .get(path)
                .map(|text| digest_text(text))
                .or_else(|| snapshot.hashes.get(path).cloned())
                .unwrap_or_else(|| "present".to_owned());
            parts.insert(path.to_owned(), value);
        }
    }
    let mut joined = String::new();
    for (part, value) in &parts {
        let _ = writeln!(joined, "{part}\t{value}");
    }
    ProjectFingerprint {
        digest: digest(joined.as_bytes()),
        parts,
    }
}

fn is_ignored_dir(name: &str) -> bool {
    super::snapshot::IGNORED_DIRS.contains(&name)
}

/// Whether the Harness still describes the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Drift {
    /// The project could not be read now (or nothing was asked): nothing is claimed.
    Unchecked,
    /// The knowledge predates fingerprints: nothing says it still holds.
    Unknown,
    /// Nothing relevant changed.
    Current,
    Changed(Staleness),
}

/// Compares the knowledge's fingerprint with the project's as it is now.
pub fn drift(analysis: &AnalysisInfo, current: Option<&ProjectFingerprint>) -> Drift {
    let Some(stored) = &analysis.fingerprint else {
        return Drift::Unknown;
    };
    let Some(current) = current else {
        return Drift::Unchecked;
    };
    detect(stored, current, analysis.analyzed_at).map_or(Drift::Current, Drift::Changed)
}

/// What changed in relevant parts between the Harness's fingerprint and the project now.
/// `None` when nothing relevant changed.
pub fn detect(
    stored: &ProjectFingerprint,
    current: &ProjectFingerprint,
    analyzed_at: u64,
) -> Option<Staleness> {
    if stored.digest == current.digest {
        return None;
    }
    let mut changes = Vec::new();
    for (part, value) in &current.parts {
        match stored.parts.get(part) {
            None => changes.push(change(part, ChangeKind::Added)),
            Some(old) if old != value => changes.push(change(part, ChangeKind::Modified)),
            Some(_) => {}
        }
    }
    for part in stored.parts.keys() {
        if !current.parts.contains_key(part) {
            changes.push(change(part, ChangeKind::Removed));
        }
    }
    if changes.is_empty() {
        return None;
    }
    changes.sort_by(|a, b| a.path.cmp(&b.path));
    let total_changes = changes.len();
    changes.truncate(MAX_LISTED_CHANGES);
    Some(Staleness {
        analyzed_at,
        changes,
        total_changes,
    })
}

fn change(part: &str, kind: ChangeKind) -> RelevantChange {
    RelevantChange {
        path: part.strip_prefix(DIR_PREFIX).unwrap_or(part).to_owned(),
        kind,
    }
}

/// Whether a finding's evidence lives where the project changed: the same path, or a folder
/// that contains it or is contained by it.
pub fn touches(evidence_source: &str, staleness: &Staleness) -> bool {
    let source = evidence_source.trim_end_matches('/');
    staleness.changes.iter().any(|c| {
        c.path == source
            || c.path
                .strip_prefix(source)
                .is_some_and(|r| r.starts_with('/'))
            || source
                .strip_prefix(c.path.as_str())
                .is_some_and(|r| r.starts_with('/'))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> ScanSnapshot {
        let mut s = ScanSnapshot::default();
        s.dir(".git");
        s.dir("src");
        s.dir("src/domain");
        s.dir("node_modules");
        s.file("package.json", Some(r#"{"name":"x"}"#));
        s.file("README.md", Some("# x"));
        s.file("src/components/Button.tsx", Some("export {}"));
        s.file(".env", Some("TOKEN=secret"));
        s
    }

    #[test]
    fn the_same_project_has_the_same_fingerprint() {
        assert_eq!(compute(&project()), compute(&project()));
    }

    #[test]
    fn line_endings_do_not_change_it() {
        let mut crlf = project();
        crlf.files
            .insert("package.json".into(), "{\r\n\"name\":\"x\"}".into());
        let mut lf = project();
        lf.files
            .insert("package.json".into(), "{\n\"name\":\"x\"}".into());

        assert_eq!(compute(&crlf), compute(&lf));
    }

    #[test]
    fn editing_an_ordinary_source_file_is_not_relevant() {
        let mut edited = project();
        edited.files.insert(
            "src/components/Button.tsx".into(),
            "export const a = 1".into(),
        );
        edited.file("src/components/Other.tsx", Some("x"));

        assert_eq!(compute(&project()).digest, compute(&edited).digest);
    }

    #[test]
    fn a_manifest_change_a_new_folder_and_a_removed_config_are_relevant() {
        let before = compute(&project());
        let mut changed = project();
        changed.files.insert(
            "package.json".into(),
            r#"{"name":"x","dependencies":{"a":"1"}}"#.into(),
        );
        changed.dir("src/infrastructure");
        changed.file("tsconfig.json", None);

        let stale = detect(&before, &compute(&changed), 7).unwrap();

        let listed: Vec<(&str, ChangeKind)> = stale
            .changes
            .iter()
            .map(|c| (c.path.as_str(), c.kind))
            .collect();
        assert_eq!(
            listed,
            [
                ("package.json", ChangeKind::Modified),
                ("src/infrastructure", ChangeKind::Added),
                ("tsconfig.json", ChangeKind::Added),
            ]
        );
        assert_eq!(stale.analyzed_at, 7);
        assert_eq!(detect(&before, &before, 7), None);
    }

    #[test]
    fn generated_folders_and_secrets_never_enter_the_fingerprint() {
        let fingerprint = compute(&project());

        assert!(fingerprint
            .parts
            .keys()
            .all(|p| !p.contains("node_modules") && !p.contains(".env") && !p.contains(".git")));
        let mut with_secret = project();
        with_secret
            .files
            .insert(".env".into(), "TOKEN=other".into());
        assert_eq!(fingerprint, compute(&with_secret));
    }

    #[test]
    fn paths_use_forward_slashes_so_every_platform_agrees() {
        let mut s = ScanSnapshot::default();
        s.dir(".github");
        s.dir(".github/workflows");
        s.file(".github/workflows/ci.yml", Some("on: push"));

        let fingerprint = compute(&s);

        assert!(fingerprint.parts.contains_key(".github/workflows/ci.yml"));
        assert!(fingerprint.parts.keys().all(|p| !p.contains('\\')));
    }

    #[test]
    fn a_long_list_of_changes_is_capped_but_counted() {
        let before = ProjectFingerprint::default();
        let mut after = ProjectFingerprint::default();
        for i in 0..30 {
            after.parts.insert(format!("dir:src/m{i:02}"), "dir".into());
        }
        after.digest = "x".into();

        let stale = detect(&before, &after, 1).unwrap();

        assert_eq!(stale.changes.len(), MAX_LISTED_CHANGES);
        assert_eq!(stale.total_changes, 30);
    }

    #[test]
    fn evidence_is_touched_by_its_own_path_or_a_related_folder() {
        let stale = Staleness {
            analyzed_at: 1,
            changes: vec![
                RelevantChange {
                    path: "package.json".into(),
                    kind: ChangeKind::Modified,
                },
                RelevantChange {
                    path: "src/domain".into(),
                    kind: ChangeKind::Added,
                },
            ],
            total_changes: 2,
        };

        assert!(touches("package.json", &stale));
        assert!(touches("src", &stale));
        assert!(touches("src/domain/", &stale));
        assert!(!touches("Cargo.toml", &stale));
        assert!(!touches("package.json.bak", &stale));
    }
}
