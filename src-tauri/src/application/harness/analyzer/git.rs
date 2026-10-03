use super::SnapshotAnalyzer;
use crate::application::harness::snapshot::ScanSnapshot;
use crate::domain::harness::{Confidence, Finding, FindingCategory, Origin};

pub struct GitAnalyzer;

impl SnapshotAnalyzer for GitAnalyzer {
    fn analyze(&self, s: &ScanSnapshot) -> Vec<Finding> {
        let mut out = Vec::new();
        let fact = |key: &str, value: &str, source: &str, evidence: &str| {
            Finding::new(
                FindingCategory::Repository,
                key,
                value,
                Confidence::High,
                Origin::Fact,
                source,
                evidence,
            )
        };
        if s.has_dir(".git") {
            out.push(fact("git", "true", ".git", "Git repository folder"));
            if let Some(head) = &s.git_head {
                out.push(fact("current_branch", &branch_of(head), ".git/HEAD", head));
            }
        } else if s.has_path(".git") {
            out.push(Finding::new(
                FindingCategory::Repository,
                "git",
                "true",
                Confidence::Medium,
                Origin::Fact,
                ".git",
                ".git is a file (a linked worktree or a submodule)",
            ));
        }
        if s.has_path(".gitignore") {
            out.push(fact(
                "gitignore",
                "true",
                ".gitignore",
                ".gitignore present",
            ));
        }
        if s.has_path(".gitmodules") {
            out.push(fact(
                "submodules",
                "true",
                ".gitmodules",
                ".gitmodules present",
            ));
        }
        out
    }
}

/// `ref: refs/heads/main` -> `main`; a bare commit id means a detached HEAD.
fn branch_of(head: &str) -> String {
    head.trim()
        .strip_prefix("ref: refs/heads/")
        .map_or_else(|| "(detached)".to_owned(), str::to_owned)
}
