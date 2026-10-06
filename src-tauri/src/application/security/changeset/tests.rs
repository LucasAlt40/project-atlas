use super::*;
use crate::application::security::testutil::TempDir;
use crate::domain::worktree::{FileChange, FileChangeStatus};

fn change(path: &str) -> FileChange {
    FileChange {
        path: path.to_owned(),
        old_path: None,
        status: FileChangeStatus::Modified,
        additions: Some(1),
        deletions: Some(0),
        binary: false,
    }
}

fn set(files: &[&str]) -> ChangeSet {
    ChangeSet {
        base_revision: "base".to_owned(),
        current_revision: "head".to_owned(),
        files: files.iter().map(|p| change(p)).collect(),
        files_changed: u32::try_from(files.len()).unwrap(),
        additions: 0,
        deletions: 0,
        uncommitted: Vec::new(),
        captured_at: 0,
    }
}

// The shape the review reads a file's added lines through: `None` is "could not read".
#[allow(clippy::unnecessary_wraps)]
fn nothing_added(_: &str) -> Option<String> {
    Some(String::new())
}

fn reviewed(root: &TempDir, files: &[&str]) -> ChangeSetReview {
    review(root.path(), &set(files), &nothing_added)
}

fn codes(review: &ChangeSetReview) -> Vec<&'static str> {
    review.issues.iter().map(|i| i.code.as_str()).collect()
}

#[test]
fn a_normal_change_is_healthy() {
    let root = TempDir::new("changes");
    let r = reviewed(&root, &["src/foo.ts", "docs/readme.md", "Cargo.toml"]);

    assert_eq!(r.health, ChangeSetHealth::Healthy);
    assert_eq!(r.issues.len(), 0);
    assert_eq!(r.files_reviewed, 3);
    assert_eq!(
        r.health.decision(),
        crate::domain::guardrail::GuardrailDecision::Allow
    );
}

#[test]
fn env_files_and_keys_need_a_person_but_their_templates_do_not() {
    let root = TempDir::new("changes");
    for sensitive in [
        ".env",
        "app/.env",
        ".env.local",
        ".env.production",
        "config/credentials.json",
        "deploy/id_rsa",
        "keys/server.pem",
        "tls/site.key",
        ".aws/credentials",
        ".ssh/config",
    ] {
        let r = reviewed(&root, &[sensitive]);
        assert_eq!(r.health, ChangeSetHealth::NeedsReview, "{sensitive}");
        assert_eq!(codes(&r), ["sensitive_file"], "{sensitive}");
        assert_eq!(
            r.health.decision(),
            crate::domain::guardrail::GuardrailDecision::Ask
        );
    }
    for harmless in [
        ".env.example",
        ".env.sample",
        "keys/README.md",
        "src/keyboard.ts",
    ] {
        assert_eq!(
            reviewed(&root, &[harmless]).health,
            ChangeSetHealth::Healthy,
            "{harmless}"
        );
    }
}

#[test]
fn git_s_own_files_are_never_let_in() {
    let root = TempDir::new("changes");
    for path in [
        ".git/config",
        ".git/hooks/pre-commit",
        "sub/.git/HEAD",
        ".GIT/config",
        ".git\\config",
    ] {
        let r = reviewed(&root, &[path]);
        assert_eq!(r.health, ChangeSetHealth::Invalid, "{path}");
        assert!(codes(&r).contains(&"protected_git"), "{path}");
        assert_eq!(
            r.health.decision(),
            crate::domain::guardrail::GuardrailDecision::Deny
        );
    }
    // A name that only contains the letters is not Git's folder.
    assert_eq!(
        reviewed(&root, &[".github/workflows/ci.yml", "src/.gitignore"]).health,
        ChangeSetHealth::Healthy
    );
}

#[test]
fn atlas_s_folder_needs_a_person_to_look() {
    let root = TempDir::new("changes");
    let r = reviewed(&root, &[".atlas/harness/context.md"]);

    assert_eq!(r.health, ChangeSetHealth::NeedsReview);
    assert_eq!(codes(&r), ["protected_atlas"]);
    // Only the project's own `.atlas`, at its root.
    assert_eq!(
        reviewed(&root, &["docs/.atlas/note.md"]).health,
        ChangeSetHealth::Healthy
    );
}

#[test]
fn a_path_that_leaves_the_project_is_invalid_by_its_name() {
    let root = TempDir::new("changes");
    for path in [
        "/etc/passwd",
        "\\Windows\\system32\\x",
        "C:/Users/x/y",
        "../outside.txt",
        "src/../../outside.txt",
        "a/b/../../../c",
        "",
    ] {
        let r = reviewed(&root, &[path]);
        assert_eq!(r.health, ChangeSetHealth::Invalid, "{path:?}");
        assert_eq!(codes(&r), ["escapes_project"], "{path:?}");
    }
    // `..` that stays inside is only a long way of writing a normal path.
    assert_eq!(
        reviewed(&root, &["src/../docs/a.md"]).health,
        ChangeSetHealth::Healthy
    );
}

#[cfg(unix)]
#[test]
fn a_link_that_leads_outside_the_project_is_invalid_and_one_that_stays_inside_is_not() {
    let root = TempDir::new("changes");
    let outside = TempDir::new("changes");
    std::fs::create_dir_all(root.path().join("src")).unwrap();
    std::fs::write(root.path().join("src/real.ts"), "x").unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("escape")).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("secret.txt"),
        root.path().join("src/steal.ts"),
    )
    .unwrap();
    std::os::unix::fs::symlink("real.ts", root.path().join("src/alias.ts")).unwrap();

    // A link to a file outside, a linked folder (and so everything under it), one that stays.
    let file = reviewed(&root, &["src/steal.ts"]);
    assert_eq!(file.health, ChangeSetHealth::Invalid);
    assert_eq!(codes(&file), ["symlink_escape"]);
    let folder = reviewed(&root, &["escape/new.txt"]);
    assert_eq!(folder.health, ChangeSetHealth::Invalid);
    assert_eq!(codes(&folder), ["symlink_escape"]);
    assert_eq!(
        reviewed(&root, &["src/alias.ts"]).health,
        ChangeSetHealth::Healthy
    );
}

#[test]
fn every_file_is_evaluated_and_the_worst_one_decides() {
    let root = TempDir::new("changes");
    let r = reviewed(
        &root,
        &[
            "src/a.ts",
            ".env",
            "src/b.ts",
            ".git/hooks/post-commit",
            ".atlas/x.md",
        ],
    );

    assert_eq!(r.files_reviewed, 5);
    assert_eq!(r.health, ChangeSetHealth::Invalid);
    let found: Vec<(&str, &str)> = r
        .issues
        .iter()
        .map(|i| (i.code.as_str(), i.path.as_str()))
        .collect();
    assert!(found.contains(&("sensitive_file", ".env")));
    assert!(found.contains(&("protected_git", ".git/hooks/post-commit")));
    assert!(found.contains(&("protected_atlas", ".atlas/x.md")));
    assert!(r
        .summary()
        .contains("protected_git: .git/hooks/post-commit"));
}

#[test]
fn a_renamed_file_is_judged_by_where_it_was_too() {
    let root = TempDir::new("changes");
    let mut changes = set(&["src/moved.ts"]);
    changes.files[0].status = FileChangeStatus::Renamed;
    changes.files[0].old_path = Some(".git/hooks/pre-commit".to_owned());

    let r = review(root.path(), &changes, &nothing_added);

    assert_eq!(r.health, ChangeSetHealth::Invalid);
}

#[test]
fn a_secret_in_what_was_added_needs_a_person_and_the_review_never_carries_it() {
    let root = TempDir::new("changes");
    let added = |path: &str| {
        Some(if path == "src/client.ts" {
            "const api_key = \"sk-abcdefghijklmnopqrstuvwxyz0123\";\nlet a = 1;".to_owned()
        } else {
            "let b = 2;".to_owned()
        })
    };

    let r = review(
        root.path(),
        &set(&["src/client.ts", "src/other.ts"]),
        &added,
    );

    assert_eq!(r.health, ChangeSetHealth::NeedsReview);
    assert_eq!(codes(&r), ["secret_in_content"]);
    assert!(!format!("{r:?}").contains("sk-abc"));
    // A file that cannot be read is not assumed clean.
    let unreadable = review(root.path(), &set(&["src/x.ts"]), &|_| None);
    assert_eq!(codes(&unreadable), ["unverifiable"]);
}

#[test]
fn only_added_lines_are_scanned() {
    let diff = "--- a/f\n+++ b/f\n@@ -1,2 +1,2 @@\n-api_key = \"sk-abcdefghijklmnopqrstuvwxyz0123\"\n+let a = 1;\n context\n";
    assert_eq!(added_lines(diff), "let a = 1;");
}

#[test]
fn the_fingerprint_follows_the_revisions_the_files_and_the_findings() {
    let root = TempDir::new("changes");
    let one = reviewed(&root, &[".env"]);
    let same = reviewed(&root, &[".env"]);
    let other_file = reviewed(&root, &[".env.local"]);
    let mut moved = set(&[".env"]);
    moved.current_revision = "newer".to_owned();

    assert_eq!(one.fingerprint, same.fingerprint);
    assert_ne!(one.fingerprint, other_file.fingerprint);
    assert_ne!(
        one.fingerprint,
        review(root.path(), &moved, &nothing_added).fingerprint
    );
}
