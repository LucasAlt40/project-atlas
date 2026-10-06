//! The review of a run's `ChangeSet` before it can be applied to the project.
//!
//! A workflow's code is written in an isolated worktree; Apply puts it in the project's working
//! tree. This is the last place Atlas can look at *what* is about to enter. The review is
//! deterministic and looks at data only: file names, where each path really leads, and the lines
//! the run added (with the Harness's secret scanner, not a second one). It never reads a file as
//! an instruction, and nothing in a file can change its result.
//!
//! | Finding                                                    | Health        | Answer |
//! | ---------------------------------------------------------- | ------------- | ------ |
//! | `.git` anywhere in a path                                  | `Invalid`     | `DENY` |
//! | absolute path, `../`, a path that really leads outside     | `Invalid`     | `DENY` |
//! | `.atlas/`, `.env`, credentials, private keys, secret lines | `NeedsReview` | `ASK`  |
//! | nothing                                                    | `Healthy`     | `ALLOW`|
//!
//! `.atlas/` is `NeedsReview` and not `Invalid` because the harness is a project file a workflow
//! may legitimately update; a person looks first. Atlas has no per-path policy to defer to, and
//! this adds none: the rule is fixed here, in one place.
//!
//! `ALLOW` does not apply anything. Apply is a person's act in every case, and Apply never
//! commits: the changes arrive uncommitted in the working tree.

use std::fmt::Write as _;
use std::path::Path;

use super::paths::{is_inside_physical, NormalizedPath, PathError, PathFlavor};
use crate::application::harness::fingerprint::digest_text;
use crate::application::harness::secrets::redact_secrets;
use crate::domain::guardrail::{ChangeIssue, ChangeIssueCode, ChangeSetHealth, ChangeSetReview};
use crate::domain::worktree::ChangeSet;

/// Reads what the run added to one file (the added lines of its diff), `None` if it cannot.
pub type AddedLines<'a> = &'a dyn Fn(&str) -> Option<String>;

/// Every path the change set touches: where a file is, and where a renamed one was.
fn touched(changes: &ChangeSet) -> Vec<(&str, bool)> {
    let mut paths: Vec<(&str, bool)> = Vec::new();
    for file in &changes.files {
        paths.push((file.path.as_str(), file.binary));
        if let Some(old) = &file.old_path {
            paths.push((old.as_str(), file.binary));
        }
    }
    for path in &changes.uncommitted {
        paths.push((path.as_str(), true));
    }
    paths
}

/// Reviews `changes`, made in the worktree at `root`.
pub fn review(root: &Path, changes: &ChangeSet, added: AddedLines<'_>) -> ChangeSetReview {
    let mut issues: Vec<ChangeIssue> = Vec::new();
    let mut flag = |code: ChangeIssueCode, path: &str| {
        issues.push(ChangeIssue {
            code,
            path: path.to_owned(),
        });
    };
    let paths = touched(changes);
    for &(path, binary) in &paths {
        // Where the path leads, by its name alone.
        if escapes(path) {
            flag(ChangeIssueCode::EscapesProject, path);
            // Nothing else about a path that leaves the project is worth reading.
            continue;
        }
        let components = components_of(path);
        if components.iter().any(|c| c == ".git") {
            flag(ChangeIssueCode::ProtectedGit, path);
        }
        if components.first().is_some_and(|c| c == ".atlas") {
            flag(ChangeIssueCode::ProtectedAtlas, path);
        }
        if is_sensitive(&components) {
            flag(ChangeIssueCode::SensitiveFile, path);
        }
        // Where it really is, following links: a link (or a linked folder) that leaves the
        // project is as bad as a `../` in the name.
        match physical(root, path) {
            Physical::Inside | Physical::Absent => {}
            Physical::Outside => flag(ChangeIssueCode::SymlinkEscape, path),
            Physical::Unknown => flag(ChangeIssueCode::Unverifiable, path),
        }
        if !binary {
            match added(path) {
                Some(text) => {
                    if redact_secrets(&text).1 > 0 {
                        flag(ChangeIssueCode::SecretInContent, path);
                    }
                }
                None => flag(ChangeIssueCode::Unverifiable, path),
            }
        }
    }
    issues.sort();
    issues.dedup();
    let health = issues
        .iter()
        .map(|i| i.code.health())
        .max()
        .unwrap_or(ChangeSetHealth::Healthy);
    let fingerprint = fingerprint_of(changes, &issues);
    ChangeSetReview {
        health,
        issues,
        files_reviewed: u32::try_from(changes.files.len()).unwrap_or(u32::MAX),
        fingerprint,
    }
}

fn fingerprint_of(changes: &ChangeSet, issues: &[ChangeIssue]) -> String {
    let mut canonical = format!(
        "atlas.changeset.review.v1\nbase={}\ncurrent={}\n",
        changes.base_revision, changes.current_revision
    );
    for file in &changes.files {
        let _ = writeln!(
            canonical,
            "file={}|{:?}|{}",
            file.path,
            file.status,
            file.old_path.as_deref().unwrap_or("")
        );
    }
    for path in &changes.uncommitted {
        let _ = writeln!(canonical, "uncommitted={path}");
    }
    for issue in issues {
        let _ = writeln!(canonical, "issue={}|{}", issue.code.as_str(), issue.path);
    }
    digest_text(&canonical)
}

/// A path as names, either separator, lowercase (Git's own folder is `.git` on every system, and
/// a case-insensitive file system answers to `.GIT` too).
fn components_of(path: &str) -> Vec<String> {
    path.replace('\\', "/")
        .split('/')
        .filter(|c| !c.is_empty() && *c != ".")
        .map(str::to_lowercase)
        .collect()
}

/// Absolute (either flavour, drive letters, UNC), or climbing out of the project by `..`.
fn escapes(path: &str) -> bool {
    let slashed = path.replace('\\', "/");
    let drive = slashed
        .as_bytes()
        .get(..2)
        .is_some_and(|b| b[0].is_ascii_alphabetic() && b[1] == b':');
    if path.is_empty() || path.contains('\0') || slashed.starts_with('/') || drive {
        return true;
    }
    let root = NormalizedPath::parse("/project", PathFlavor::Posix);
    !NormalizedPath::parse(&format!("/project/{slashed}"), PathFlavor::Posix).starts_with(&root)
}

const SENSITIVE_NAMES: &[&str] = &[
    "credentials",
    "credentials.json",
    ".credentials",
    ".netrc",
    "id_rsa",
    "id_dsa",
    "id_ecdsa",
    "id_ed25519",
];
const SENSITIVE_EXTENSIONS: &[&str] = &["pem", "key", "p12", "pfx", "ppk"];
/// `.env.example` and its kind are documentation of the variables, not their values.
const ENV_TEMPLATES: &[&str] = &["example", "sample", "template", "dist", "defaults"];

fn is_sensitive(components: &[String]) -> bool {
    let Some(name) = components.last() else {
        return false;
    };
    if components.iter().any(|c| c == ".ssh") {
        return true;
    }
    if components
        .windows(2)
        .any(|w| w[0] == ".aws" && w[1] == "credentials")
    {
        return true;
    }
    if name == ".env" {
        return true;
    }
    if let Some(suffix) = name.strip_prefix(".env.") {
        return !ENV_TEMPLATES.contains(&suffix);
    }
    if SENSITIVE_NAMES.contains(&name.as_str()) {
        return true;
    }
    name.rsplit_once('.')
        .is_some_and(|(_, ext)| SENSITIVE_EXTENSIONS.contains(&ext))
}

enum Physical {
    Inside,
    /// Not on disk (a deleted file): nothing there to lead anywhere.
    Absent,
    Outside,
    Unknown,
}

fn physical(root: &Path, path: &str) -> Physical {
    let full = root.join(path);
    match full.symlink_metadata() {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // The file is gone, but a linked folder above it may still lead outside.
            match parent_physical(root, &full) {
                Some(inside) => inside,
                None => Physical::Absent,
            }
        }
        Err(_) => Physical::Unknown,
        Ok(_) => match is_inside_physical(root, &full, root) {
            Ok(true) => Physical::Inside,
            Ok(false) => Physical::Outside,
            Err(PathError::BrokenLink | PathError::Io) => dangling(root, &full),
        },
    }
}

/// A link that leads nowhere (yet): where it points is still known by its name, and a file that
/// is created later there would be outside the project.
fn dangling(root: &Path, full: &Path) -> Physical {
    let Ok(target) = std::fs::read_link(full) else {
        return Physical::Unknown;
    };
    let flavor = PathFlavor::host();
    let root = NormalizedPath::parse(&root.to_string_lossy(), flavor);
    let resolved = if target.is_absolute() {
        target
    } else {
        full.parent().unwrap_or(full).join(target)
    };
    if NormalizedPath::parse(&resolved.to_string_lossy(), flavor).starts_with(&root) {
        Physical::Unknown
    } else {
        Physical::Outside
    }
}

/// Where the nearest folder that still exists above `full` really is.
fn parent_physical(root: &Path, full: &Path) -> Option<Physical> {
    let existing = full
        .ancestors()
        .skip(1)
        .find(|p| p.symlink_metadata().is_ok())?;
    Some(match is_inside_physical(root, existing, root) {
        Ok(true) => Physical::Inside,
        Ok(false) => Physical::Outside,
        Err(_) => Physical::Unknown,
    })
}

/// The added lines of a unified diff: what a run wrote, without the context around it.
pub fn added_lines(diff: &str) -> String {
    diff.lines()
        .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
        .map(|l| &l[1..])
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests;
