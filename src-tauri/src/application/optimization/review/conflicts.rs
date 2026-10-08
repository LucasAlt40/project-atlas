//! Two sources that assert incompatible things about the same known choice.
//!
//! Deterministic and narrow on purpose. Atlas knows a few groups of mutually exclusive choices (a
//! project's database, its front-end framework, its package manager, its test runner). A conflict
//! is reported only when one source says "use A" and another says "use B" about the same group,
//! the sources differ, and neither also mentions the other's choice (which would make it a
//! comparison or a migration, not an instruction). Everything else is not reported: when a
//! conflict cannot be shown, saying nothing is more honest than guessing.

use std::collections::HashSet;

use super::super::context::text::{fenced_lines, words};
use super::super::context::ContextItem;
use super::{excerpt, issue};
use crate::domain::guardrail::{IssueCode, IssueSeverity, ReviewIssue, SourceTrust};
use crate::domain::optimization::SectionKind;

type Group = (
    &'static str,
    &'static [(&'static str, &'static [&'static str])],
);

const GROUPS: &[Group] = &[
    (
        "database",
        &[
            ("PostgreSQL", &["postgresql", "postgres"]),
            ("MySQL", &["mysql"]),
            ("MariaDB", &["mariadb"]),
            ("MongoDB", &["mongodb", "mongo"]),
            ("SQLite", &["sqlite"]),
            ("DynamoDB", &["dynamodb"]),
        ],
    ),
    (
        "front-end framework",
        &[
            ("Angular", &["angular"]),
            ("React", &["react"]),
            ("Vue", &["vue"]),
            ("Svelte", &["svelte"]),
        ],
    ),
    (
        "package manager",
        &[
            ("npm", &["npm"]),
            ("pnpm", &["pnpm"]),
            ("yarn", &["yarn"]),
            ("bun", &["bun"]),
        ],
    ),
    (
        "test runner",
        &[
            ("Jest", &["jest"]),
            ("Vitest", &["vitest"]),
            ("Mocha", &["mocha"]),
        ],
    ),
];

/// A line that tells what to use, in English or Portuguese.
const CUES: &[&str] = &[
    "use ",
    "uses ",
    "using ",
    "must use",
    "should use",
    "stack:",
    "stack ",
    "database:",
    "framework:",
    "built with",
    "built on",
    "usar ",
    "usa ",
    "usando ",
    "deve usar",
    "banco de dados:",
    "banco:",
];

/// A line that is about leaving, comparing or forbidding a choice, not about making one.
const NOT_AN_INSTRUCTION: &[&str] = &[
    " not ",
    "never",
    "don't",
    "do not",
    "avoid",
    "instead of",
    "migrat",
    "replac",
    "deprecat",
    "legacy",
    "→",
    "->",
    "não ",
    "nunca",
    "evite",
    "em vez de",
    "substitu",
    "legado",
];

struct Assertion<'a> {
    group: usize,
    tech: usize,
    source: SectionKind,
    /// Which one of a section's items: each rule is a source of its own, so two rules can
    /// disagree with each other (empty for every other section).
    owner: &'a str,
    /// Where the source first appears, to order a pair the same way every time.
    position: usize,
    line: &'a str,
}

fn assertions(items: &[ContextItem]) -> Vec<Assertion<'_>> {
    let mut found = Vec::new();
    for (position, item) in items.iter().enumerate() {
        if SourceTrust::of(item.source) == SourceTrust::Atlas {
            continue;
        }
        let fenced = fenced_lines(&item.content);
        for (line, in_code) in item.content.lines().zip(fenced) {
            let lower = line.to_lowercase();
            if in_code
                || !CUES.iter().any(|c| lower.contains(c))
                || NOT_AN_INSTRUCTION.iter().any(|n| lower.contains(n))
            {
                continue;
            }
            let tokens = words(line);
            for (g, (_, techs)) in GROUPS.iter().enumerate() {
                let present: Vec<usize> = techs
                    .iter()
                    .enumerate()
                    .filter(|(_, (_, aliases))| aliases.iter().any(|a| tokens.contains(*a)))
                    .map(|(t, _)| t)
                    .collect();
                // A line naming two choices of one group compares them: not an instruction.
                if let [tech] = present.as_slice() {
                    found.push(Assertion {
                        group: g,
                        tech: *tech,
                        source: item.source,
                        owner: if item.source == SectionKind::Rules {
                            item.id.as_str()
                        } else {
                            ""
                        },
                        position,
                        line,
                    });
                }
            }
        }
    }
    found
}

pub fn find(items: &[ContextItem]) -> Vec<ReviewIssue> {
    let all = assertions(items);
    let mut reported: HashSet<(usize, usize, usize, usize, usize)> = HashSet::new();
    let mut issues = Vec::new();
    for (i, a) in all.iter().enumerate() {
        for b in all.iter().skip(i + 1) {
            if a.group != b.group
                || a.tech == b.tech
                || (a.source == b.source && a.owner == b.owner)
            {
                continue;
            }
            // If either side also asserts the other's choice, it is not taking one side.
            let also = |x: &Assertion<'_>, tech: usize| {
                all.iter().any(|o| {
                    o.group == x.group
                        && o.source == x.source
                        && o.owner == x.owner
                        && o.tech == tech
                })
            };
            if also(a, b.tech) || also(b, a.tech) {
                continue;
            }
            let (first, second) = if a.position <= b.position {
                (a, b)
            } else {
                (b, a)
            };
            let key = (
                first.group,
                first.tech.min(second.tech),
                first.tech.max(second.tech),
                first.position,
                second.position,
            );
            if !reported.insert(key) {
                continue;
            }
            let (label, techs) = GROUPS[first.group];
            let mut conflict = issue(
                IssueCode::ConflictingInstructions,
                IssueSeverity::Error,
                first.source,
                format!(
                    "two sources disagree on the {label}: {} vs {}",
                    techs[first.tech].0, techs[second.tech].0
                ),
            );
            conflict.other_source = Some(second.source);
            conflict.excerpt = excerpt(first.line);
            issues.push(conflict);
        }
    }
    issues
}
