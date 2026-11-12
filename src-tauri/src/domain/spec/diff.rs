//! What changed between two revisions of a document (ADR 0035, 0036): line by line, per file,
//! with a little context around each change. Pure; the same answer for the same two revisions.
//!
//! The comparison is a longest-common-subsequence over lines, after the lines both sides share at
//! the start and at the end are set aside (a revision usually touches a part of a document). When
//! what is left is still too large to compare, the file says so instead of using unbounded memory.

use serde::Serialize;

use super::model::{Revision, SpecFile};

/// Lines of unchanged text kept around each change.
pub const CONTEXT_LINES: usize = 3;
/// The largest comparison done (lines of one side × lines of the other, after the shared start and
/// end are set aside). Past it, a file is reported as too large to compare.
pub const MAX_CELLS: usize = 4_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DiffLine {
    Same {
        text: String,
    },
    Added {
        text: String,
    },
    Removed {
        text: String,
    },
    /// Unchanged lines left out between two changes.
    Gap {
        lines: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChange {
    Added,
    Removed,
    Changed,
    Unchanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub path: String,
    pub change: FileChange,
    pub added: usize,
    pub removed: usize,
    /// The comparison was larger than [`MAX_CELLS`]: no lines, only that the file changed.
    pub too_large: bool,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionDiff {
    pub from: u32,
    pub to: u32,
    pub files: Vec<FileDiff>,
}

/// What changed from `from` to `to`, file by file in the order of `to` (files only in `from` last).
pub fn diff_revisions(from: &Revision, to: &Revision) -> RevisionDiff {
    let mut files: Vec<FileDiff> = to
        .files
        .iter()
        .map(|new| {
            let old = from.files.iter().find(|f| f.path == new.path);
            diff_file(old, Some(new))
        })
        .collect();
    files.extend(
        from.files
            .iter()
            .filter(|old| !to.files.iter().any(|f| f.path == old.path))
            .map(|old| diff_file(Some(old), None)),
    );
    RevisionDiff {
        from: from.number,
        to: to.number,
        files,
    }
}

fn diff_file(old: Option<&SpecFile>, new: Option<&SpecFile>) -> FileDiff {
    let path = new.or(old).map(|f| f.path.clone()).unwrap_or_default();
    let a: Vec<&str> = old.map(|f| f.content.lines().collect()).unwrap_or_default();
    let b: Vec<&str> = new.map(|f| f.content.lines().collect()).unwrap_or_default();
    let change = match (old, new) {
        (None, _) => FileChange::Added,
        (_, None) => FileChange::Removed,
        _ if a == b => FileChange::Unchanged,
        _ => FileChange::Changed,
    };
    let Some(ops) = compare(&a, &b) else {
        return FileDiff {
            path,
            change,
            added: b.len(),
            removed: a.len(),
            too_large: true,
            lines: Vec::new(),
        };
    };
    let added = ops.iter().filter(|op| matches!(op, Op::Added(_))).count();
    let removed = ops.iter().filter(|op| matches!(op, Op::Removed(_))).count();
    FileDiff {
        path,
        change,
        added,
        removed,
        too_large: false,
        lines: with_context(&ops, &a, &b),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    /// Index into the old lines (and the new ones: they are the same text).
    Same(usize, usize),
    Removed(usize),
    Added(usize),
}

/// The edit from `a` to `b`, or `None` when it is too large to work out.
fn compare(a: &[&str], b: &[&str]) -> Option<Vec<Op>> {
    let prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (a_mid, b_mid) = (&a[prefix..a.len() - suffix], &b[prefix..b.len() - suffix]);
    if a_mid.len().saturating_mul(b_mid.len()) > MAX_CELLS {
        return None;
    }
    let mut ops: Vec<Op> = (0..prefix).map(|i| Op::Same(i, i)).collect();
    // lengths[i][j]: the longest common subsequence of a_mid[i..] and b_mid[j..].
    let (n, m) = (a_mid.len(), b_mid.len());
    let mut lengths = vec![0u32; (n + 1) * (m + 1)];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lengths[i * (m + 1) + j] = if a_mid[i] == b_mid[j] {
                lengths[(i + 1) * (m + 1) + j + 1] + 1
            } else {
                lengths[(i + 1) * (m + 1) + j].max(lengths[i * (m + 1) + j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && a_mid[i] == b_mid[j] {
            ops.push(Op::Same(prefix + i, prefix + j));
            i += 1;
            j += 1;
        } else if j < m
            && (i == n || lengths[i * (m + 1) + j + 1] >= lengths[(i + 1) * (m + 1) + j])
        {
            ops.push(Op::Added(prefix + j));
            j += 1;
        } else {
            ops.push(Op::Removed(prefix + i));
            i += 1;
        }
    }
    ops.extend((0..suffix).map(|k| Op::Same(a.len() - suffix + k, b.len() - suffix + k)));
    Some(ops)
}

/// The changes, each with [`CONTEXT_LINES`] of unchanged text around it; the rest becomes gaps.
fn with_context(ops: &[Op], a: &[&str], b: &[&str]) -> Vec<DiffLine> {
    let changed: Vec<usize> = ops
        .iter()
        .enumerate()
        .filter(|(_, op)| !matches!(op, Op::Same(..)))
        .map(|(i, _)| i)
        .collect();
    let near = |i: usize| changed.iter().any(|&c| c.abs_diff(i) <= CONTEXT_LINES);
    let mut lines = Vec::new();
    let mut skipped = 0;
    for (index, op) in ops.iter().enumerate() {
        let line = match *op {
            Op::Same(old, _) if near(index) => DiffLine::Same {
                text: a[old].to_owned(),
            },
            Op::Same(..) => {
                skipped += 1;
                continue;
            }
            Op::Removed(old) => DiffLine::Removed {
                text: a[old].to_owned(),
            },
            Op::Added(new) => DiffLine::Added {
                text: b[new].to_owned(),
            },
        };
        if skipped > 0 {
            lines.push(DiffLine::Gap { lines: skipped });
            skipped = 0;
        }
        lines.push(line);
    }
    if skipped > 0 && !lines.is_empty() {
        lines.push(DiffLine::Gap { lines: skipped });
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::spec::RevisionOrigin;
    use std::collections::BTreeMap;

    fn revision(number: u32, files: &[(&str, &str)]) -> Revision {
        Revision {
            number,
            files: files
                .iter()
                .map(|(path, content)| SpecFile {
                    path: (*path).to_owned(),
                    content: (*content).to_owned(),
                })
                .collect(),
            origin: RevisionOrigin::Generated,
            run_id: None,
            agent_id: None,
            based_on: BTreeMap::new(),
            created_at: 0,
        }
    }

    fn text(lines: std::ops::Range<usize>) -> String {
        lines.map(|i| format!("line {i}\n")).collect()
    }

    #[test]
    fn a_changed_line_shows_what_left_and_what_came_with_context_and_gaps() {
        let old = text(0..20);
        let new = old.replace("line 10\n", "line ten\n");

        let diff = diff_revisions(
            &revision(1, &[("design.md", &old)]),
            &revision(2, &[("design.md", &new)]),
        );

        let file = &diff.files[0];
        assert_eq!((diff.from, diff.to), (1, 2));
        assert_eq!(file.change, FileChange::Changed);
        assert_eq!((file.added, file.removed), (1, 1));
        assert_eq!(file.lines[0], DiffLine::Gap { lines: 7 });
        assert!(file.lines.contains(&DiffLine::Removed {
            text: "line 10".to_owned()
        }));
        assert!(file.lines.contains(&DiffLine::Added {
            text: "line ten".to_owned()
        }));
        assert_eq!(file.lines.last(), Some(&DiffLine::Gap { lines: 6 }));
        let shown = file
            .lines
            .iter()
            .filter(|l| matches!(l, DiffLine::Same { .. }))
            .count();
        assert_eq!(shown, 2 * CONTEXT_LINES);
    }

    #[test]
    fn files_that_appear_or_go_are_whole_and_the_same_file_is_unchanged() {
        let diff = diff_revisions(
            &revision(1, &[("specs/a/spec.md", "a\n"), ("specs/b/spec.md", "b\n")]),
            &revision(
                2,
                &[("specs/a/spec.md", "a\n"), ("specs/c/spec.md", "c\nd\n")],
            ),
        );

        let changes: Vec<_> = diff
            .files
            .iter()
            .map(|f| (f.path.as_str(), f.change))
            .collect();
        assert_eq!(
            changes,
            [
                ("specs/a/spec.md", FileChange::Unchanged),
                ("specs/c/spec.md", FileChange::Added),
                ("specs/b/spec.md", FileChange::Removed),
            ]
        );
        assert!(diff.files[0].lines.is_empty());
        assert_eq!(diff.files[1].added, 2);
        assert_eq!(diff.files[2].removed, 1);
    }

    #[test]
    fn insertions_and_deletions_in_the_middle_keep_the_rest_in_place() {
        let old = "a\nb\nc\nd\ne\n";
        let new = "a\nc\nX\nd\ne\nf\n";

        let diff = diff_revisions(
            &revision(1, &[("p.md", old)]),
            &revision(2, &[("p.md", new)]),
        );

        let file = &diff.files[0];
        assert_eq!((file.added, file.removed), (2, 1));
        // Replaying the diff on the old text gives the new one.
        let rebuilt: Vec<&str> = file
            .lines
            .iter()
            .filter_map(|l| match l {
                DiffLine::Same { text } | DiffLine::Added { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(rebuilt, new.lines().collect::<Vec<_>>());
    }

    #[test]
    fn a_comparison_too_large_says_so_instead_of_using_unbounded_memory() {
        let old = text(0..3_000);
        let new: String = (0..3_000).map(|i| format!("other {i}\n")).collect();

        let diff = diff_revisions(
            &revision(1, &[("t.md", &old)]),
            &revision(2, &[("t.md", &new)]),
        );

        let file = &diff.files[0];
        assert!(file.too_large);
        assert!(file.lines.is_empty());
        assert_eq!((file.added, file.removed), (3_000, 3_000));
        // A small edit in a long document is cheap: the shared start and end are set aside.
        let long = text(0..20_000);
        let edited = long.replace("line 9999\n", "changed\n");
        let small = diff_revisions(
            &revision(1, &[("t.md", &long)]),
            &revision(2, &[("t.md", &edited)]),
        );
        assert!(!small.files[0].too_large);
    }
}
