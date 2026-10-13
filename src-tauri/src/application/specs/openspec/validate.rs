//! The checks Atlas runs on `OpenSpec` documents. Deterministic: the same files give the same issues,
//! whatever any agent said about them. The rules follow `OpenSpec`'s own (its `validate` command and
//! the `spec-driven` schema): a `## Why` of 50 characters or more, requirements that say SHALL or
//! MUST and have `#### Scenario:` blocks, delta sections, checkbox tasks. Where Atlas asks for
//! more (a capability in the proposal has a spec, a task cites the requirement it covers) the issue
//! says so in its code, and the ones that only suggest are not errors.

use std::collections::{BTreeMap, BTreeSet};

use super::markdown::{
    self, find_section, parse_spec_file, parse_tasks, scenario_steps, sections, DeltaOp, TaskLine,
};
use crate::application::specs::framework::{Documents, ProjectSpecs};
use crate::domain::spec::{ArtifactKind, IssueSeverity, SpecFile, ValidationIssue};

pub const MIN_WHY: usize = 50;
pub const MAX_WHY: usize = 1_000;
pub const MIN_PURPOSE: usize = 50;
pub const MAX_REQUIREMENT_TEXT: usize = 500;
pub const MAX_DELTAS: usize = 10;

fn issue(
    code: &str,
    severity: IssueSeverity,
    file: Option<&str>,
    line: Option<u32>,
    params: &[(&str, &str)],
) -> ValidationIssue {
    ValidationIssue {
        code: code.to_owned(),
        severity,
        file: file.map(str::to_owned),
        line,
        message: code.replace('_', " "),
        params: params
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
    }
}

fn error(code: &str, file: &str, line: Option<u32>, params: &[(&str, &str)]) -> ValidationIssue {
    issue(code, IssueSeverity::Error, Some(file), line, params)
}

fn warning(code: &str, file: &str, line: Option<u32>, params: &[(&str, &str)]) -> ValidationIssue {
    issue(code, IssueSeverity::Warning, Some(file), line, params)
}

fn suggestion(
    code: &str,
    file: &str,
    line: Option<u32>,
    params: &[(&str, &str)],
) -> ValidationIssue {
    issue(code, IssueSeverity::Suggestion, Some(file), line, params)
}

/// The text with its HTML comments blanked (newlines kept, so line numbers still match): a
/// template's `<!-- … -->` hints are not content.
pub fn without_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        // An unclosed comment runs to the end of the text.
        let end = rest[start..]
            .find("-->")
            .map_or(rest.len(), |end| start + end + 3);
        for c in rest[start..end].chars() {
            out.push(if c == '\n' { '\n' } else { ' ' });
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

fn normalize(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn is_kebab(segment: &str) -> bool {
    !segment.is_empty()
        && segment.split('-').all(|part| {
            !part.is_empty() && part.chars().all(|c| matches!(c, 'a'..='z' | '0'..='9'))
        })
}

/// A capability path: one to three segments of letters, digits, `-`, `_` and `.`, none of them
/// `.` or `..`. A path Atlas will write under `specs/`, so it must stay inside it.
pub fn is_capability_path(path: &str) -> bool {
    let segments: Vec<&str> = path.split('/').collect();
    (1..=3).contains(&segments.len())
        && segments.iter().all(|s| {
            !s.is_empty()
                && *s != "."
                && *s != ".."
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        })
}

/// `cap` from `specs/<cap>/spec.md`.
pub fn capability_of(path: &str) -> Option<&str> {
    let inner = path.strip_prefix("specs/")?.strip_suffix("/spec.md")?;
    is_capability_path(inner).then_some(inner)
}

// ---- pending markers ---------------------------------------------------------------------------------

/// Lines that still carry a marker for something undecided: `TODO`, `TBD`,
/// `[NEEDS CLARIFICATION …]`. Outside code fences and comments.
fn pending_markers(content: &str) -> Vec<(u32, &'static str)> {
    let clean = without_comments(content);
    let mut found = Vec::new();
    for line in markdown::lines(&clean) {
        if line.in_fence {
            continue;
        }
        if line.text.contains("[NEEDS CLARIFICATION") {
            found.push((line.number, "NEEDS CLARIFICATION"));
            continue;
        }
        let has_word = |word: &str| {
            line.text
                .split(|c: char| !c.is_ascii_alphanumeric())
                .any(|w| w == word)
        };
        if has_word("TODO") {
            found.push((line.number, "TODO"));
        } else if has_word("TBD") {
            found.push((line.number, "TBD"));
        }
    }
    found
}

fn pending_issues(file: &str, content: &str, severity: IssueSeverity) -> Vec<ValidationIssue> {
    pending_markers(content)
        .into_iter()
        .map(|(line, marker)| {
            issue(
                "pending_marker",
                severity,
                Some(file),
                Some(line),
                &[("marker", marker)],
            )
        })
        .collect()
}

// ---- proposal ------------------------------------------------------------------------------------------

/// The capabilities a proposal lists: `(new, modified)`, from the bullets under `### New
/// Capabilities` and `### Modified Capabilities` that name one in backticks.
pub fn proposal_capabilities(content: &str) -> (Vec<String>, Vec<String>) {
    let clean = without_comments(content);
    let top = sections(&clean, 2);
    let Some(capabilities) = find_section(&top, "Capabilities") else {
        return (Vec::new(), Vec::new());
    };
    let sub = sections(&capabilities.full_text, 3);
    let read = |title: &str| -> Vec<String> {
        find_section(&sub, title)
            .map(|s| bullet_names(&s.own_text))
            .unwrap_or_default()
    };
    (read("New Capabilities"), read("Modified Capabilities"))
}

/// The names in backticks at the start of each bullet: ``- `user-auth`: what it covers``.
fn bullet_names(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let item = line
                .trim_start()
                .strip_prefix(['-', '*', '+'])?
                .trim_start();
            let rest = item.strip_prefix('`')?;
            let name = &rest[..rest.find('`')?];
            let name = name.trim();
            (!name.is_empty() && !name.starts_with('<')).then(|| name.to_owned())
        })
        .collect()
}

fn validate_proposal(file: &str, content: &str) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    if content.trim().is_empty() {
        return vec![error("file_empty", file, None, &[])];
    }
    let clean = without_comments(content);
    let top = sections(&clean, 2);
    match find_section(&top, "Why") {
        None => issues.push(error("why_missing", file, None, &[])),
        Some(why) => {
            let length = why.full_text.trim().chars().count();
            if length < MIN_WHY {
                issues.push(error(
                    "why_too_short",
                    file,
                    Some(why.line),
                    &[("min", &MIN_WHY.to_string())],
                ));
            } else if length > MAX_WHY {
                issues.push(warning(
                    "why_too_long",
                    file,
                    Some(why.line),
                    &[("max", &MAX_WHY.to_string())],
                ));
            }
        }
    }
    match find_section(&top, "What Changes") {
        None => issues.push(error("what_changes_missing", file, None, &[])),
        Some(what) if what.full_text.trim().is_empty() => {
            issues.push(error("what_changes_empty", file, Some(what.line), &[]));
        }
        Some(_) => {}
    }
    if find_section(&top, "Capabilities").is_none() {
        issues.push(error("capabilities_missing", file, None, &[]));
    } else {
        let (new, modified) = proposal_capabilities(content);
        if new.is_empty() && modified.is_empty() {
            issues.push(error("no_capabilities", file, None, &[]));
        }
        for name in &new {
            if !is_capability_path(name) {
                issues.push(error(
                    "capability_path_invalid",
                    file,
                    None,
                    &[("capability", name)],
                ));
            } else if !name.split('/').all(is_kebab) {
                issues.push(error(
                    "capability_not_kebab",
                    file,
                    None,
                    &[("capability", name)],
                ));
            }
        }
        for name in &modified {
            if !is_capability_path(name) {
                issues.push(error(
                    "capability_path_invalid",
                    file,
                    None,
                    &[("capability", name)],
                ));
            }
        }
    }
    if find_section(&top, "Impact").is_none() {
        issues.push(suggestion("impact_missing", file, None, &[]));
    }
    issues.extend(pending_issues(file, content, IssueSeverity::Warning));
    issues
}

// ---- specs ---------------------------------------------------------------------------------------------

fn requirement_names(content: &str) -> Vec<String> {
    parse_spec_file(content)
        .deltas
        .iter()
        .flat_map(|d| d.requirements.iter().map(|r| normalize(&r.name)))
        .collect()
}

#[allow(clippy::too_many_lines)] // one rule after another, in the order of the document
fn validate_spec_file(file: &SpecFile, project: &ProjectSpecs) -> Vec<ValidationIssue> {
    let path = file.path.as_str();
    let mut issues = Vec::new();
    let Some(capability) = capability_of(path) else {
        return vec![error("spec_path_invalid", path, None, &[])];
    };
    if file.content.trim().is_empty() {
        return vec![error("file_empty", path, None, &[])];
    }
    let clean = without_comments(&file.content);
    let doc = parse_spec_file(&clean);
    let deltas: Vec<_> = doc.deltas.iter().filter(|d| d.line > 0).collect();
    if deltas.is_empty() {
        issues.push(error("no_deltas", path, None, &[]));
    }
    for (line, name) in &doc.stray_requirements {
        issues.push(warning(
            "requirement_outside_delta",
            path,
            Some(*line),
            &[("requirement", name)],
        ));
    }
    let existing = project.capabilities.get(capability);
    let existing_names: BTreeSet<String> = existing
        .map(|content| requirement_names(content).into_iter().collect())
        .unwrap_or_default();
    let is_new_capability = existing.is_none();
    if is_new_capability {
        match &doc.purpose {
            None => issues.push(warning("purpose_missing", path, None, &[])),
            Some(purpose) => {
                if purpose.chars().count() < MIN_PURPOSE {
                    issues.push(warning(
                        "purpose_too_brief",
                        path,
                        None,
                        &[("min", &MIN_PURPOSE.to_string())],
                    ));
                }
                if purpose.contains("TBD") || purpose.contains("TODO") {
                    issues.push(warning("purpose_placeholder", path, None, &[]));
                }
            }
        }
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for delta in &deltas {
        let op = delta.op;
        if is_new_capability && op != DeltaOp::Added {
            issues.push(error(
                "capability_not_in_project",
                path,
                Some(delta.line),
                &[("capability", capability), ("operation", op.name())],
            ));
        }
        if op == DeltaOp::Renamed {
            let upper = delta.text.to_ascii_uppercase();
            if !(upper.contains("FROM:") && upper.contains("TO:")) {
                issues.push(error("rename_malformed", path, Some(delta.line), &[]));
            }
            continue;
        }
        if delta.requirements.is_empty() {
            issues.push(error(
                "delta_without_requirements",
                path,
                Some(delta.line),
                &[("operation", op.name())],
            ));
        }
        for requirement in &delta.requirements {
            let name = requirement.name.trim();
            let at = Some(requirement.line);
            if name.is_empty() {
                issues.push(error("requirement_name_empty", path, at, &[]));
                continue;
            }
            let key = normalize(name);
            if !seen.insert(key.clone()) {
                issues.push(error(
                    "requirement_duplicate",
                    path,
                    at,
                    &[("requirement", name)],
                ));
            }
            match op {
                DeltaOp::Added if existing_names.contains(&key) => issues.push(error(
                    "added_requirement_exists",
                    path,
                    at,
                    &[("requirement", name)],
                )),
                DeltaOp::Modified | DeltaOp::Removed
                    if !is_new_capability && !existing_names.contains(&key) =>
                {
                    issues.push(error(
                        "requirement_not_in_project",
                        path,
                        at,
                        &[("requirement", name), ("operation", op.name())],
                    ));
                }
                _ => {}
            }
            if op == DeltaOp::Removed {
                let text = requirement.text.to_ascii_lowercase();
                if !text.contains("**reason**") || !text.contains("**migration**") {
                    issues.push(warning(
                        "removed_without_reason",
                        path,
                        at,
                        &[("requirement", name)],
                    ));
                }
                continue;
            }
            let text = requirement.text.trim();
            if text.is_empty() {
                issues.push(error(
                    "requirement_empty",
                    path,
                    at,
                    &[("requirement", name)],
                ));
            } else {
                if !has_normative_word(text) {
                    issues.push(error(
                        "requirement_no_normative",
                        path,
                        at,
                        &[("requirement", name)],
                    ));
                }
                if text.chars().count() > MAX_REQUIREMENT_TEXT {
                    issues.push(warning(
                        "requirement_too_long",
                        path,
                        at,
                        &[
                            ("requirement", name),
                            ("max", &MAX_REQUIREMENT_TEXT.to_string()),
                        ],
                    ));
                }
            }
            for wrong in &requirement.wrong_level_scenarios {
                issues.push(error(
                    "scenario_wrong_level",
                    path,
                    Some(*wrong),
                    &[("requirement", name)],
                ));
            }
            if requirement.scenarios.is_empty() {
                issues.push(error(
                    "requirement_no_scenario",
                    path,
                    at,
                    &[("requirement", name)],
                ));
            }
            let mut scenario_names = BTreeSet::new();
            for scenario in &requirement.scenarios {
                let at = Some(scenario.line);
                if scenario.body.trim().is_empty() {
                    issues.push(error(
                        "scenario_empty",
                        path,
                        at,
                        &[("scenario", &scenario.name)],
                    ));
                    continue;
                }
                let (when, then) = scenario_steps(&scenario.body);
                if !(when && then) {
                    issues.push(error(
                        "scenario_not_when_then",
                        path,
                        at,
                        &[("scenario", &scenario.name)],
                    ));
                }
                if !scenario_names.insert(normalize(&scenario.name)) {
                    issues.push(warning(
                        "scenario_duplicate",
                        path,
                        at,
                        &[("scenario", &scenario.name)],
                    ));
                }
            }
        }
    }
    issues.extend(pending_issues(path, &file.content, IssueSeverity::Warning));
    issues
}

/// SHALL or MUST as a word, in capitals, as `OpenSpec` asks for normative requirements.
fn has_normative_word(text: &str) -> bool {
    text.split(|c: char| !c.is_ascii_alphabetic())
        .any(|w| w == "SHALL" || w == "MUST")
}

fn validate_specs(files: &[SpecFile], project: &ProjectSpecs) -> Vec<ValidationIssue> {
    if files.is_empty() {
        return vec![issue(
            "no_spec_files",
            IssueSeverity::Error,
            None,
            None,
            &[],
        )];
    }
    let mut issues: Vec<ValidationIssue> = files
        .iter()
        .flat_map(|f| validate_spec_file(f, project))
        .collect();
    let deltas: usize = files
        .iter()
        .map(|f| {
            parse_spec_file(&without_comments(&f.content))
                .deltas
                .iter()
                .filter(|d| d.line > 0)
                .map(|d| d.requirements.len().max(1))
                .sum::<usize>()
        })
        .sum();
    if deltas > MAX_DELTAS {
        issues.push(issue(
            "too_many_deltas",
            IssueSeverity::Suggestion,
            None,
            None,
            &[("max", &MAX_DELTAS.to_string())],
        ));
    }
    issues
}

// ---- design --------------------------------------------------------------------------------------------

fn validate_design(file: &str, content: &str) -> Vec<ValidationIssue> {
    if content.trim().is_empty() {
        return vec![error("file_empty", file, None, &[])];
    }
    let clean = without_comments(content);
    let top = sections(&clean, 2);
    let mut issues = Vec::new();
    for section in ["Context", "Decisions"] {
        if find_section(&top, section).is_none() {
            issues.push(warning(
                "design_section_missing",
                file,
                None,
                &[("section", section)],
            ));
        }
    }
    for section in ["Goals / Non-Goals", "Risks / Trade-offs"] {
        if find_section(&top, section).is_none() {
            issues.push(suggestion(
                "design_section_missing",
                file,
                None,
                &[("section", section)],
            ));
        }
    }
    issues.extend(pending_issues(file, content, IssueSeverity::Warning));
    issues
}

// ---- tasks ---------------------------------------------------------------------------------------------

fn dependency_ids(text: &str) -> Vec<String> {
    let lower = text.to_ascii_lowercase();
    let mut out = Vec::new();
    for marker in [
        "depends on ",
        "after task ",
        "requires task ",
        "blocked by ",
    ] {
        let mut rest = lower.as_str();
        while let Some(at) = rest.find(marker) {
            rest = &rest[at + marker.len()..];
            for token in rest.split([',', ' ', ';', ')']).take(4) {
                let token = token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.');
                let numbered =
                    token.contains('.') && token.chars().next().is_some_and(|c| c.is_ascii_digit());
                if numbered {
                    out.push(token.to_owned());
                } else if !token.is_empty() && token != "and" && token != "task" {
                    break;
                }
            }
        }
    }
    out
}

fn has_cycle(edges: &BTreeMap<String, Vec<String>>) -> Option<String> {
    fn visit(
        node: &str,
        edges: &BTreeMap<String, Vec<String>>,
        stack: &mut Vec<String>,
        done: &mut BTreeSet<String>,
    ) -> Option<String> {
        if stack.iter().any(|s| s == node) {
            return Some(node.to_owned());
        }
        if done.contains(node) {
            return None;
        }
        stack.push(node.to_owned());
        for next in edges.get(node).into_iter().flatten() {
            if let Some(found) = visit(next, edges, stack, done) {
                return Some(found);
            }
        }
        stack.pop();
        done.insert(node.to_owned());
        None
    }
    let mut done = BTreeSet::new();
    for node in edges.keys() {
        if let Some(found) = visit(node, edges, &mut Vec::new(), &mut done) {
            return Some(found);
        }
    }
    None
}

fn mentions_verification(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "verify", "test", "check", "validate", "confirm", "ensure", "assert", "run ", "passes",
    ]
    .iter()
    .any(|word| lower.contains(word))
}

fn numbering_issues(file: &str, tasks: &[TaskLine]) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let mut first: BTreeMap<&str, u32> = BTreeMap::new();
    for task in tasks.iter().filter(|t| t.numbered) {
        if let (Some(group), Some(lead)) = (&task.group, task.id.split('.').next()) {
            if group.trim_start_matches('0') != lead.trim_start_matches('0') {
                issues.push(warning(
                    "task_numbering",
                    file,
                    Some(task.line),
                    &[("task", &task.id), ("group", group)],
                ));
            }
        }
        if first.insert(task.id.as_str(), task.line).is_some() {
            issues.push(warning(
                "task_id_duplicate",
                file,
                Some(task.line),
                &[("task", &task.id)],
            ));
        }
    }
    issues
}

fn validate_tasks(file: &str, content: &str) -> Vec<ValidationIssue> {
    if content.trim().is_empty() {
        return vec![error("file_empty", file, None, &[])];
    }
    let clean = without_comments(content);
    let tasks = parse_tasks(&clean);
    if tasks.is_empty() {
        return vec![error("no_tasks", file, None, &[])];
    }
    let mut issues = numbering_issues(file, &tasks);
    let ids: BTreeSet<&str> = tasks.iter().map(|t| t.id.as_str()).collect();
    let mut edges: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut unverified = 0usize;
    let mut unnumbered = 0usize;
    for task in &tasks {
        let at = Some(task.line);
        if task.text.trim().is_empty() {
            issues.push(error("task_empty", file, at, &[("task", &task.id)]));
            continue;
        }
        if task.done {
            issues.push(warning("task_prechecked", file, at, &[("task", &task.id)]));
        }
        if !task.numbered {
            unnumbered += 1;
        }
        if !mentions_verification(&task.text) {
            unverified += 1;
        }
        for dep in dependency_ids(&task.text) {
            if dep == task.id {
                issues.push(error(
                    "task_dependency_cycle",
                    file,
                    at,
                    &[("task", &task.id)],
                ));
            } else if !ids.contains(dep.as_str()) {
                issues.push(error(
                    "task_dependency_unknown",
                    file,
                    at,
                    &[("task", &task.id), ("dependency", &dep)],
                ));
            } else {
                edges.entry(task.id.clone()).or_default().push(dep);
            }
        }
    }
    if let Some(node) = has_cycle(&edges) {
        issues.push(error(
            "task_dependency_cycle",
            file,
            None,
            &[("task", &node)],
        ));
    }
    if unnumbered > 0 {
        issues.push(suggestion(
            "tasks_unnumbered",
            file,
            None,
            &[("count", &unnumbered.to_string())],
        ));
    }
    if unverified > 0 {
        issues.push(suggestion(
            "task_without_verification",
            file,
            None,
            &[("count", &unverified.to_string())],
        ));
    }
    issues.extend(pending_issues(file, content, IssueSeverity::Warning));
    issues
}

// ---- the entry points ----------------------------------------------------------------------------------

fn single<'a>(files: &'a [SpecFile], expected: &str) -> Result<&'a SpecFile, Vec<ValidationIssue>> {
    let mut unexpected: Vec<ValidationIssue> = files
        .iter()
        .filter(|f| f.path != expected)
        .map(|f| error("unexpected_file", &f.path, None, &[("expected", expected)]))
        .collect();
    match files.iter().find(|f| f.path == expected) {
        Some(file) if unexpected.is_empty() => Ok(file),
        Some(_) => Err(unexpected),
        None => {
            unexpected.push(error("file_missing", expected, None, &[]));
            Err(unexpected)
        }
    }
}

pub fn validate_artifact(
    kind: ArtifactKind,
    files: &[SpecFile],
    project: &ProjectSpecs,
) -> Vec<ValidationIssue> {
    let check =
        |expected: &str, f: fn(&str, &str) -> Vec<ValidationIssue>| match single(files, expected) {
            Ok(file) => f(&file.path, &file.content),
            Err(issues) => issues,
        };
    match kind {
        ArtifactKind::Proposal => check("proposal.md", validate_proposal),
        ArtifactKind::Design => check("design.md", validate_design),
        ArtifactKind::Tasks => check("tasks.md", validate_tasks),
        ArtifactKind::Specs => validate_specs(files, project),
    }
}

/// The requirements the spec files add or modify: `(capability, name)`.
fn spec_requirements(files: &[SpecFile]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for file in files {
        let Some(capability) = capability_of(&file.path) else {
            continue;
        };
        for delta in parse_spec_file(&without_comments(&file.content)).deltas {
            if matches!(delta.op, DeltaOp::Added | DeltaOp::Modified) {
                for requirement in delta.requirements {
                    out.push((capability.to_owned(), requirement.name.trim().to_owned()));
                }
            }
        }
    }
    out
}

/// The requirement names a task cites: the text after `Requirement:` up to `)`, `]`, `;` or the end
/// of the line.
fn cited_requirements(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let lower = text.to_ascii_lowercase();
    let mut from = 0;
    while let Some(at) = lower[from..].find("requirement:") {
        let start = from + at + "requirement:".len();
        let rest = &text[start..];
        let end = rest.find([')', ']', ';', '\n']).unwrap_or(rest.len());
        let cited = rest[..end].trim().trim_matches(['`', '*', '"']).trim();
        if !cited.is_empty() {
            out.push(cited.to_owned());
        }
        from = start + end;
    }
    out
}

pub fn validate_consistency(documents: &Documents) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let content_of = |id: &str, path: &str| -> Option<String> {
        documents
            .get(id)?
            .iter()
            .find(|f| f.path == path)
            .map(|f| f.content.clone())
    };
    let spec_files: Vec<SpecFile> = documents.get("specs").cloned().unwrap_or_default();
    if let Some(proposal) = content_of("proposal", "proposal.md") {
        let (new, modified) = proposal_capabilities(&proposal);
        let in_files: BTreeSet<&str> = spec_files
            .iter()
            .filter_map(|f| capability_of(&f.path))
            .collect();
        for name in new.iter().chain(modified.iter()) {
            if !in_files.contains(name.as_str()) {
                issues.push(error(
                    "capability_without_spec",
                    "proposal.md",
                    None,
                    &[("capability", name)],
                ));
            }
        }
        for name in &in_files {
            if !new.iter().chain(modified.iter()).any(|c| c == name) {
                issues.push(warning(
                    "spec_not_in_proposal",
                    &format!("specs/{name}/spec.md"),
                    None,
                    &[("capability", name)],
                ));
            }
        }
    }
    if let Some(tasks_text) = content_of("tasks", "tasks.md") {
        let requirements = spec_requirements(&spec_files);
        let names: Vec<String> = requirements.iter().map(|(_, n)| normalize(n)).collect();
        let tasks = parse_tasks(&without_comments(&tasks_text));
        let all_text = normalize(
            &tasks
                .iter()
                .map(|t| t.text.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        );
        let mut cited_any = false;
        for task in &tasks {
            for cited in cited_requirements(&task.text) {
                cited_any = true;
                let key = normalize(&cited);
                if !names.iter().any(|n| *n == key || key.contains(n.as_str())) {
                    issues.push(error(
                        "task_reference_broken",
                        "tasks.md",
                        Some(task.line),
                        &[("task", &task.id), ("requirement", &cited)],
                    ));
                }
            }
        }
        if !requirements.is_empty() && !cited_any {
            issues.push(warning(
                "tasks_without_requirement_links",
                "tasks.md",
                None,
                &[],
            ));
        } else {
            for (capability, name) in &requirements {
                if !all_text.contains(&normalize(name)) {
                    issues.push(warning(
                        "requirement_not_covered",
                        "tasks.md",
                        None,
                        &[("capability", capability), ("requirement", name)],
                    ));
                }
            }
        }
    }
    // A marker left in the text is a decision nobody made: fine in a draft, not in the final.
    for (id, files) in documents {
        let _ = id;
        for file in files {
            issues.extend(pending_issues(
                &file.path,
                &file.content,
                IssueSeverity::Error,
            ));
        }
    }
    issues
}

#[cfg(test)]
#[cfg(test)]
pub(crate) mod tests_support {
    pub const PROPOSAL: &str = "# Proposal\n\n## Why\n\nUsers working at night find the bright interface tiring and ask for a dark theme.\n\n## What Changes\n\n- Add a dark theme switch.\n\n## Capabilities\n\n### New Capabilities\n- `dark-mode`: theme switching\n\n### Modified Capabilities\n\n## Impact\n\nThe settings screen.\n";

    pub const SPEC: &str = "# Spec Delta\n\n## Purpose\n\nLets people switch the interface between a light and a dark theme.\n\n## ADDED Requirements\n\n### Requirement: User can switch the theme\nThe system SHALL let the user pick light or dark.\n\n#### Scenario: Switch to dark\n- **WHEN** the user picks dark\n- **THEN** the interface uses the dark theme\n";

    pub const DESIGN: &str = "# Design\n\n## Context\n\nA settings screen exists.\n\n## Goals / Non-Goals\n\nGoals: a switch.\n\n## Decisions\n\nUse CSS variables.\n\n## Risks / Trade-offs\n\nContrast.\n";

    pub const TASKS: &str = "# Tasks\n\n## 1. Theme\n\n- [ ] 1.1 Add the theme switch (Requirement: User can switch the theme) and verify it with a component test\n- [ ] 1.2 Persist the choice, depends on 1.1, and verify after a restart\n";
}

#[cfg(test)]
mod tests {
    use super::tests_support::{DESIGN, PROPOSAL, SPEC, TASKS};
    use super::*;

    fn file(path: &str, content: &str) -> SpecFile {
        SpecFile {
            path: path.to_owned(),
            content: content.to_owned(),
        }
    }

    fn has(issues: &[ValidationIssue], code: &str, severity: IssueSeverity) -> bool {
        issues
            .iter()
            .any(|i| i.code == code && i.severity == severity)
    }

    #[test]
    fn a_complete_proposal_has_no_errors() {
        let issues = validate_artifact(
            ArtifactKind::Proposal,
            &[file("proposal.md", PROPOSAL)],
            &ProjectSpecs::default(),
        );

        assert!(
            issues.iter().all(|i| i.severity != IssueSeverity::Error),
            "{issues:?}"
        );
    }

    #[test]
    fn a_proposal_needs_a_why_of_substance_and_what_changes() {
        let issues = validate_artifact(
            ArtifactKind::Proposal,
            &[file(
                "proposal.md",
                "# P\n\n## Why\n\nBecause.\n\n## Capabilities\n\n### New Capabilities\n- `a`: b\n",
            )],
            &ProjectSpecs::default(),
        );

        assert!(has(&issues, "why_too_short", IssueSeverity::Error));
        assert!(has(&issues, "what_changes_missing", IssueSeverity::Error));
    }

    #[test]
    fn a_proposal_that_names_no_capability_is_an_error() {
        let issues = validate_artifact(
            ArtifactKind::Proposal,
            &[file(
                "proposal.md",
                "## Why\n\nA long enough reason to want this change to exist at all.\n\n## What Changes\n\n- x\n\n## Capabilities\n\n### New Capabilities\n<!-- nothing -->\n- `<capability-path>`: template left as it was\n",
            )],
            &ProjectSpecs::default(),
        );

        assert!(has(&issues, "no_capabilities", IssueSeverity::Error));
    }

    #[test]
    fn a_new_capability_must_be_kebab_case() {
        let issues = validate_artifact(
            ArtifactKind::Proposal,
            &[file(
                "proposal.md",
                &PROPOSAL.replace("`dark-mode`", "`Dark_Mode`"),
            )],
            &ProjectSpecs::default(),
        );

        assert!(has(&issues, "capability_not_kebab", IssueSeverity::Error));
    }

    #[test]
    fn the_wrong_file_name_is_refused() {
        let issues = validate_artifact(
            ArtifactKind::Proposal,
            &[file("PROPOSAL.md", PROPOSAL)],
            &ProjectSpecs::default(),
        );

        assert!(has(&issues, "file_missing", IssueSeverity::Error));
        assert!(has(&issues, "unexpected_file", IssueSeverity::Error));
    }

    #[test]
    fn a_complete_spec_file_has_no_errors() {
        let issues = validate_artifact(
            ArtifactKind::Specs,
            &[file("specs/dark-mode/spec.md", SPEC)],
            &ProjectSpecs::default(),
        );

        assert!(
            issues.iter().all(|i| i.severity != IssueSeverity::Error),
            "{issues:?}"
        );
    }

    #[test]
    fn a_requirement_needs_a_normative_word_and_a_scenario() {
        let spec = "## ADDED Requirements\n\n### Requirement: Weak\nThe system should do it.\n";
        let issues = validate_artifact(
            ArtifactKind::Specs,
            &[file("specs/weak/spec.md", spec)],
            &ProjectSpecs::default(),
        );

        assert!(has(
            &issues,
            "requirement_no_normative",
            IssueSeverity::Error
        ));
        assert!(has(
            &issues,
            "requirement_no_scenario",
            IssueSeverity::Error
        ));
    }

    #[test]
    fn a_scenario_needs_when_and_then() {
        let spec = "## ADDED Requirements\n\n### Requirement: R\nThe system MUST x.\n\n#### Scenario: S\n- the user does something\n";
        let issues = validate_artifact(
            ArtifactKind::Specs,
            &[file("specs/r/spec.md", spec)],
            &ProjectSpecs::default(),
        );

        assert!(has(&issues, "scenario_not_when_then", IssueSeverity::Error));
    }

    #[test]
    fn scenarios_under_three_hashes_are_an_error_not_silently_ignored() {
        let spec = "## ADDED Requirements\n\n### Requirement: R\nThe system MUST x.\n\n### Scenario: S\n- **WHEN** a\n- **THEN** b\n";
        let issues = validate_artifact(
            ArtifactKind::Specs,
            &[file("specs/r/spec.md", spec)],
            &ProjectSpecs::default(),
        );

        assert!(has(&issues, "scenario_wrong_level", IssueSeverity::Error));
        assert!(has(
            &issues,
            "requirement_no_scenario",
            IssueSeverity::Error
        ));
    }

    #[test]
    fn a_spec_file_without_delta_sections_is_an_error() {
        let issues = validate_artifact(
            ArtifactKind::Specs,
            &[file("specs/r/spec.md", "# Spec\n\nJust prose.\n")],
            &ProjectSpecs::default(),
        );

        assert!(has(&issues, "no_deltas", IssueSeverity::Error));
    }

    #[test]
    fn a_long_requirement_is_a_warning_and_a_missing_purpose_too() {
        let long = "x ".repeat(300);
        let spec = format!(
            "## ADDED Requirements\n\n### Requirement: R\nThe system SHALL {long}\n\n#### Scenario: S\n- **WHEN** a\n- **THEN** b\n"
        );
        let issues = validate_artifact(
            ArtifactKind::Specs,
            &[file("specs/r/spec.md", &spec)],
            &ProjectSpecs::default(),
        );

        assert!(has(&issues, "requirement_too_long", IssueSeverity::Warning));
        assert!(has(&issues, "purpose_missing", IssueSeverity::Warning));
        assert!(issues.iter().all(|i| i.severity != IssueSeverity::Error));
    }

    #[test]
    fn modifying_a_capability_the_project_does_not_have_is_an_error() {
        let spec = "## MODIFIED Requirements\n\n### Requirement: R\nThe system SHALL x.\n\n#### Scenario: S\n- **WHEN** a\n- **THEN** b\n";
        let issues = validate_artifact(
            ArtifactKind::Specs,
            &[file("specs/ghost/spec.md", spec)],
            &ProjectSpecs::default(),
        );

        assert!(has(
            &issues,
            "capability_not_in_project",
            IssueSeverity::Error
        ));
    }

    #[test]
    fn a_modified_requirement_must_exist_in_the_main_spec() {
        let main = "## Purpose\nx\n\n## Requirements\n### Requirement: Login\nThe system SHALL log in.\n#### Scenario: s\n- **WHEN** a\n- **THEN** b\n";
        let project = ProjectSpecs {
            capabilities: [("auth".to_owned(), main.to_owned())].into(),
        };
        let spec = "## MODIFIED Requirements\n\n### Requirement: Logout\nThe system SHALL x.\n\n#### Scenario: S\n- **WHEN** a\n- **THEN** b\n\n### Requirement: login\nThe system SHALL y.\n\n#### Scenario: S\n- **WHEN** a\n- **THEN** b\n";

        let issues = validate_artifact(
            ArtifactKind::Specs,
            &[file("specs/auth/spec.md", spec)],
            &project,
        );

        let missing: Vec<_> = issues
            .iter()
            .filter(|i| i.code == "requirement_not_in_project")
            .collect();
        assert_eq!(missing.len(), 1, "{issues:?}");
        assert_eq!(missing[0].params["requirement"], "Logout");
    }

    #[test]
    fn adding_a_requirement_the_main_spec_already_has_is_an_error() {
        let main = "## Requirements\n### Requirement: Login\nThe system SHALL log in.\n#### Scenario: s\n- **WHEN** a\n- **THEN** b\n";
        let project = ProjectSpecs {
            capabilities: [("auth".to_owned(), main.to_owned())].into(),
        };
        let spec = "## ADDED Requirements\n\n### Requirement: Login\nThe system SHALL x.\n\n#### Scenario: S\n- **WHEN** a\n- **THEN** b\n";

        let issues = validate_artifact(
            ArtifactKind::Specs,
            &[file("specs/auth/spec.md", spec)],
            &project,
        );

        assert!(has(
            &issues,
            "added_requirement_exists",
            IssueSeverity::Error
        ));
    }

    #[test]
    fn a_spec_path_that_leaves_the_specs_folder_is_refused() {
        for path in [
            "specs/../evil/spec.md",
            "specs//spec.md",
            "other/spec.md",
            "specs/a/b/c/d/spec.md",
        ] {
            let issues = validate_artifact(
                ArtifactKind::Specs,
                &[file(path, SPEC)],
                &ProjectSpecs::default(),
            );
            assert!(
                has(&issues, "spec_path_invalid", IssueSeverity::Error),
                "{path}"
            );
        }
    }

    #[test]
    fn an_empty_file_is_an_error_in_every_artifact() {
        for (kind, path) in [
            (ArtifactKind::Proposal, "proposal.md"),
            (ArtifactKind::Design, "design.md"),
            (ArtifactKind::Tasks, "tasks.md"),
            (ArtifactKind::Specs, "specs/a/spec.md"),
        ] {
            let issues = validate_artifact(kind, &[file(path, "  \n")], &ProjectSpecs::default());
            assert!(has(&issues, "file_empty", IssueSeverity::Error), "{path}");
        }
    }

    #[test]
    fn a_design_without_context_or_decisions_is_warned_not_blocked() {
        let issues = validate_artifact(
            ArtifactKind::Design,
            &[file("design.md", "# Design\n\nSome thoughts.\n")],
            &ProjectSpecs::default(),
        );

        assert!(has(
            &issues,
            "design_section_missing",
            IssueSeverity::Warning
        ));
        assert!(issues.iter().all(|i| i.severity != IssueSeverity::Error));
    }

    #[test]
    fn complete_tasks_have_no_errors() {
        let issues = validate_artifact(
            ArtifactKind::Tasks,
            &[file("tasks.md", TASKS)],
            &ProjectSpecs::default(),
        );

        assert!(
            issues.iter().all(|i| i.severity != IssueSeverity::Error),
            "{issues:?}"
        );
    }

    #[test]
    fn tasks_without_checkboxes_are_an_error() {
        let issues = validate_artifact(
            ArtifactKind::Tasks,
            &[file("tasks.md", "# Tasks\n\n- do this\n- do that\n")],
            &ProjectSpecs::default(),
        );

        assert!(has(&issues, "no_tasks", IssueSeverity::Error));
    }

    #[test]
    fn an_empty_task_and_a_precompleted_task_are_found() {
        let issues = validate_artifact(
            ArtifactKind::Tasks,
            &[file(
                "tasks.md",
                "## 1. A\n\n- [ ] 1.1\n- [x] 1.2 Done already\n",
            )],
            &ProjectSpecs::default(),
        );

        assert!(has(&issues, "task_empty", IssueSeverity::Error));
        assert!(has(&issues, "task_prechecked", IssueSeverity::Warning));
    }

    #[test]
    fn a_task_that_depends_on_a_task_that_does_not_exist_is_an_error() {
        let issues = validate_artifact(
            ArtifactKind::Tasks,
            &[file(
                "tasks.md",
                "## 1. A\n\n- [ ] 1.1 First, depends on 9.9, and test it\n",
            )],
            &ProjectSpecs::default(),
        );

        assert!(has(
            &issues,
            "task_dependency_unknown",
            IssueSeverity::Error
        ));
    }

    #[test]
    fn tasks_that_depend_on_each_other_in_a_circle_are_an_error() {
        let issues = validate_artifact(
            ArtifactKind::Tasks,
            &[file(
                "tasks.md",
                "## 1. A\n\n- [ ] 1.1 First depends on 1.2 test\n- [ ] 1.2 Second depends on 1.1 test\n",
            )],
            &ProjectSpecs::default(),
        );

        assert!(has(&issues, "task_dependency_cycle", IssueSeverity::Error));
    }

    #[test]
    fn numbering_mistakes_are_warnings() {
        let issues = validate_artifact(
            ArtifactKind::Tasks,
            &[file(
                "tasks.md",
                "## 1. A\n\n- [ ] 1.1 One test\n- [ ] 2.1 Wrong group test\n- [ ] 1.1 Duplicate test\n",
            )],
            &ProjectSpecs::default(),
        );

        assert!(has(&issues, "task_numbering", IssueSeverity::Warning));
        assert!(has(&issues, "task_id_duplicate", IssueSeverity::Warning));
    }

    #[test]
    fn a_pending_marker_is_a_warning_in_a_draft_and_ignored_in_code_and_comments() {
        let issues = validate_artifact(
            ArtifactKind::Design,
            &[file(
                "design.md",
                "## Context\nx\n## Decisions\nTODO decide the store\n```\nTODO in code\n```\n<!-- TODO in a comment -->\n",
            )],
            &ProjectSpecs::default(),
        );

        let markers: Vec<_> = issues
            .iter()
            .filter(|i| i.code == "pending_marker")
            .collect();
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].severity, IssueSeverity::Warning);
        assert_eq!(markers[0].line, Some(4));
    }

    fn documents() -> Documents {
        [
            ("proposal".to_owned(), vec![file("proposal.md", PROPOSAL)]),
            (
                "specs".to_owned(),
                vec![file("specs/dark-mode/spec.md", SPEC)],
            ),
            ("design".to_owned(), vec![file("design.md", DESIGN)]),
            ("tasks".to_owned(), vec![file("tasks.md", TASKS)]),
        ]
        .into()
    }

    #[test]
    fn documents_that_agree_have_no_consistency_errors() {
        let issues = validate_consistency(&documents());

        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn a_capability_in_the_proposal_with_no_spec_file_is_an_error() {
        let mut docs = documents();
        docs.insert("specs".to_owned(), vec![file("specs/other/spec.md", SPEC)]);

        let issues = validate_consistency(&docs);

        assert!(has(
            &issues,
            "capability_without_spec",
            IssueSeverity::Error
        ));
        assert!(has(&issues, "spec_not_in_proposal", IssueSeverity::Warning));
    }

    #[test]
    fn a_task_that_cites_a_requirement_that_is_not_there_is_an_error() {
        let mut docs = documents();
        docs.insert(
            "tasks".to_owned(),
            vec![file(
                "tasks.md",
                "## 1. A\n\n- [ ] 1.1 Do (Requirement: Something invented) and test\n",
            )],
        );

        let issues = validate_consistency(&docs);

        assert!(has(&issues, "task_reference_broken", IssueSeverity::Error));
    }

    #[test]
    fn a_requirement_no_task_covers_is_a_warning() {
        let mut docs = documents();
        docs.insert(
            "tasks".to_owned(),
            vec![file(
                "tasks.md",
                "## 1. A\n\n- [ ] 1.1 Something unrelated and test it\n",
            )],
        );

        let issues = validate_consistency(&docs);

        assert!(has(
            &issues,
            "tasks_without_requirement_links",
            IssueSeverity::Warning
        ));
        assert!(issues.iter().all(|i| i.severity != IssueSeverity::Error));
    }

    #[test]
    fn a_marker_left_in_any_document_is_an_error_in_the_final_check() {
        let mut docs = documents();
        docs.insert(
            "design".to_owned(),
            vec![file(
                "design.md",
                "## Context\nx\n## Decisions\n[NEEDS CLARIFICATION: which store?]\n",
            )],
        );

        let issues = validate_consistency(&docs);

        assert!(has(&issues, "pending_marker", IssueSeverity::Error));
    }

    #[test]
    fn the_same_files_always_give_the_same_issues() {
        let files = [file("specs/dark-mode/spec.md", SPEC)];
        let a = validate_artifact(ArtifactKind::Specs, &files, &ProjectSpecs::default());
        let b = validate_artifact(ArtifactKind::Specs, &files, &ProjectSpecs::default());

        assert_eq!(a, b);
    }
}
