//! Skill quality: a long skill is not a good skill. Short discovery, focused activation, deep
//! resources on demand. Every check is deterministic and says what to fix.

use std::collections::HashSet;

use super::model::ParsedSkill;
use crate::application::optimization::context::text::{normalized, words};

/// Estimated tokens above which a skill's instructions are too big to load whole.
pub const MAX_BODY_TOKENS: u64 = 5_000;
/// Lines above which the format itself recommends moving detail into `references/`.
pub const MAX_BODY_LINES: usize = 500;
pub const MAX_DESCRIPTION_CHARS: usize = 1_024;
pub const MAX_NAME_CHARS: usize = 64;
/// More tools than this and the skill is asking for the whole toolbox.
pub const MAX_ALLOWED_TOOLS: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Worth fixing; the skill still works.
    Warning,
    /// The skill cannot be used (Atlas never selects it).
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IssueCode {
    Malformed,
    MissingName,
    InvalidName,
    NameMismatch,
    MissingDescription,
    DescriptionTooLong,
    TooLarge,
    MissingTriggers,
    OverlyGeneric,
    DuplicatedInstructions,
    ConflictingInstructions,
    UnusedSections,
    ToolExplosion,
    /// Another skill, from a folder that wins, has this name.
    Shadowed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillIssue {
    pub code: IssueCode,
    pub severity: Severity,
    pub message: String,
}

fn issue(code: IssueCode, severity: Severity, message: impl Into<String>) -> SkillIssue {
    SkillIssue {
        code,
        severity,
        message: message.into(),
    }
}

/// A valid name: lowercase letters, digits and single hyphens, not starting or ending with one.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= MAX_NAME_CHARS
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

const TRIGGER_CUES: [&str; 8] = [
    "use when",
    "use this",
    "use for",
    "when the",
    "when you",
    "quando",
    "usar quando",
    "use para",
];

pub fn validate(skill: &ParsedSkill) -> Vec<SkillIssue> {
    let mut issues = Vec::new();
    match skill.name.as_deref() {
        None => issues.push(issue(
            IssueCode::MissingName,
            Severity::Error,
            "the frontmatter has no `name`",
        )),
        Some(name) if !valid_name(name) => issues.push(issue(
            IssueCode::InvalidName,
            Severity::Error,
            format!("`{name}` must be lowercase letters, digits and single hyphens (at most 64)"),
        )),
        Some(name) if name != skill.dir => issues.push(issue(
            IssueCode::NameMismatch,
            Severity::Warning,
            format!("`name: {name}` does not match its folder `{}`", skill.dir),
        )),
        Some(_) => {}
    }

    match skill.description.as_deref() {
        None => issues.push(issue(
            IssueCode::MissingDescription,
            Severity::Error,
            "no `description`: nothing tells Atlas when to use this skill",
        )),
        Some(description) => {
            if description.chars().count() > MAX_DESCRIPTION_CHARS {
                issues.push(issue(
                    IssueCode::DescriptionTooLong,
                    Severity::Warning,
                    format!("the description is over {MAX_DESCRIPTION_CHARS} characters"),
                ));
            }
            let lower = description.to_lowercase();
            if !TRIGGER_CUES.iter().any(|cue| lower.contains(cue)) {
                issues.push(issue(
                    IssueCode::MissingTriggers,
                    Severity::Warning,
                    "the description does not say when to use the skill (\"Use when …\")",
                ));
            }
            let distinct = words(description).len();
            if distinct < 6 {
                issues.push(issue(
                    IssueCode::OverlyGeneric,
                    Severity::Warning,
                    "the description is too short to tell this skill from any other",
                ));
            }
        }
    }

    if skill.body_size.estimated_tokens() > MAX_BODY_TOKENS || skill.body_lines > MAX_BODY_LINES {
        issues.push(issue(
            IssueCode::TooLarge,
            Severity::Warning,
            format!(
                "{} lines, about {} tokens: move detail into `references/` and keep SKILL.md focused",
                skill.body_lines,
                skill.body_size.estimated_tokens()
            ),
        ));
    }
    if skill.allowed_tools.len() > MAX_ALLOWED_TOOLS {
        issues.push(issue(
            IssueCode::ToolExplosion,
            Severity::Warning,
            format!(
                "`allowed-tools` lists {} tools; a skill should ask only for what it uses",
                skill.allowed_tools.len()
            ),
        ));
    }
    if let Some(line) = repeated_line(&skill.body) {
        issues.push(issue(
            IssueCode::DuplicatedInstructions,
            Severity::Warning,
            format!("an instruction is said more than once: \"{line}\""),
        ));
    }
    if let Some((positive, negative)) = conflict(&skill.body) {
        issues.push(issue(
            IssueCode::ConflictingInstructions,
            Severity::Warning,
            format!("\"{positive}\" and \"{negative}\" say opposite things"),
        ));
    }
    if let Some(heading) = empty_section(&skill.body) {
        issues.push(issue(
            IssueCode::UnusedSections,
            Severity::Warning,
            format!("the section \"{heading}\" has no content"),
        ));
    }
    issues
}

/// The body's non-heading lines of at least four words, outside code fences.
fn instruction_lines(body: &str) -> Vec<&str> {
    let mut inside = false;
    let mut lines = Vec::new();
    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            inside = !inside;
            continue;
        }
        if inside || line.trim_start().starts_with('#') {
            continue;
        }
        if words(line).len() >= 4 {
            lines.push(line.trim());
        }
    }
    lines
}

fn repeated_line(body: &str) -> Option<String> {
    let mut seen = HashSet::new();
    instruction_lines(body)
        .into_iter()
        .find(|line| !seen.insert(normalized(line)))
        .map(|line| line.chars().take(80).collect())
}

const POSITIVE: [&str; 3] = ["always ", "you must ", "must "];
const NEGATIVE: [&str; 4] = ["never ", "do not ", "don't ", "must not "];

/// The words of an instruction after its modal ("always", "never"…), and whether it forbids.
fn directive(line: &str) -> Option<(HashSet<String>, bool)> {
    let lower = line.to_lowercase();
    for cue in NEGATIVE {
        if let Some(at) = lower.find(cue) {
            return Some((words(&lower[at + cue.len()..]), true));
        }
    }
    for cue in POSITIVE {
        if let Some(at) = lower.find(cue) {
            return Some((words(&lower[at + cue.len()..]), false));
        }
    }
    None
}

/// Two lines that tell the same thing to be done and not to be done.
fn conflict(body: &str) -> Option<(String, String)> {
    let directives: Vec<(&str, HashSet<String>, bool)> = instruction_lines(body)
        .into_iter()
        .filter_map(|line| directive(line).map(|(w, neg)| (line, w, neg)))
        .filter(|(_, w, _)| w.len() >= 3)
        .collect();
    for (i, (a, wa, na)) in directives.iter().enumerate() {
        for (b, wb, nb) in directives.iter().skip(i + 1) {
            if na == nb {
                continue;
            }
            let shared = wa.intersection(wb).count();
            let smaller = wa.len().min(wb.len());
            // Nearly all of the shorter instruction is in the other one.
            if shared * 5 >= smaller * 4 {
                let cut = |s: &str| s.chars().take(80).collect::<String>();
                let (positive, negative) = if *na { (b, a) } else { (a, b) };
                return Some((cut(positive), cut(negative)));
            }
        }
    }
    None
}

/// A heading followed directly by another heading or by the end.
fn empty_section(body: &str) -> Option<String> {
    let mut inside = false;
    let mut pending: Option<&str> = None;
    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            inside = !inside;
        }
        if !inside && line.trim_start().starts_with('#') {
            if let Some(heading) = pending {
                return Some(heading.trim_start_matches('#').trim().to_owned());
            }
            pending = Some(line);
        } else if !line.trim().is_empty() {
            pending = None;
        }
    }
    pending.map(|h| h.trim_start_matches('#').trim().to_owned())
}

/// Whether the skill has an issue that makes it unusable.
pub fn blocking(issues: &[SkillIssue]) -> bool {
    issues.iter().any(|i| i.severity == Severity::Error)
}

/// Instructions two skills share word for word (three or more lines): one of them repeats the
/// other and the repetition is paid for twice when both load.
pub fn shared_instructions(a: &ParsedSkill, b: &ParsedSkill) -> usize {
    let lines_a: HashSet<String> = instruction_lines(&a.body)
        .into_iter()
        .map(normalized)
        .collect();
    instruction_lines(&b.body)
        .into_iter()
        .filter(|l| lines_a.contains(&normalized(l)))
        .count()
}
