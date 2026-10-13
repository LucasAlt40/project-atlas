//! Reading the parts of `OpenSpec`'s Markdown that the checks need: headings (outside code
//! fences), the requirements and scenarios of a spec file, and the checkbox tasks of `tasks.md`.
//! A reader, not a Markdown parser: it knows the shapes `OpenSpec` documents and nothing else.

/// A line of the document and whether it sits inside a fenced code block (where a `## Heading`
/// is an example, not a heading).
pub struct Line<'a> {
    /// 1-based.
    pub number: u32,
    pub text: &'a str,
    pub in_fence: bool,
}

pub fn lines(content: &str) -> Vec<Line<'_>> {
    let mut out = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    for (index, raw) in content.lines().enumerate() {
        let text = raw.trim_end_matches('\r');
        let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
        let marker = fence_marker(text);
        let in_fence = match (fence, marker) {
            (None, Some(open)) => {
                fence = Some(open);
                true
            }
            (Some((ch, len)), Some((c, l))) if c == ch && l >= len && is_bare_fence(text) => {
                fence = None;
                true
            }
            (Some(_), _) => true,
            (None, None) => false,
        };
        out.push(Line {
            number,
            text,
            in_fence,
        });
    }
    out
}

/// The character and length of a code fence that opens (or closes) this line, if any: three or
/// more backticks or tildes, indented by at most three spaces.
fn fence_marker(line: &str) -> Option<(char, usize)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = line.trim_start_matches(' ');
    let ch = rest.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let len = rest.chars().take_while(|c| *c == ch).count();
    (len >= 3).then_some((ch, len))
}

fn is_bare_fence(line: &str) -> bool {
    let rest = line.trim();
    let ch = rest.chars().next().unwrap_or(' ');
    rest.chars().all(|c| c == ch)
}

/// `(level, title)` of an ATX heading line (`## Title`), indented by at most three spaces.
pub fn heading(line: &str) -> Option<(usize, &str)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = line.trim_start_matches(' ');
    let level = rest.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let after = &rest[level..];
    if after.is_empty() {
        return Some((level, ""));
    }
    after
        .starts_with([' ', '\t'])
        .then(|| (level, after.trim().trim_end_matches('#').trim()))
}

/// A section of the document: a heading and what follows it up to the next heading of the same or
/// a higher level.
pub struct Section {
    pub title: String,
    pub line: u32,
    /// The text below the heading, up to the next heading of any level.
    pub own_text: String,
    /// The text below the heading, including its subsections.
    pub full_text: String,
}

/// The sections of a document with a heading of exactly `level`.
pub fn sections(content: &str, level: usize) -> Vec<Section> {
    let all = lines(content);
    let mut out = Vec::new();
    for (i, line) in all.iter().enumerate() {
        if line.in_fence {
            continue;
        }
        let Some((l, title)) = heading(line.text) else {
            continue;
        };
        if l != level {
            continue;
        }
        let mut own = String::new();
        let mut full = String::new();
        let mut own_done = false;
        for next in &all[i + 1..] {
            if !next.in_fence {
                if let Some((nl, _)) = heading(next.text) {
                    if nl <= level {
                        break;
                    }
                    own_done = true;
                }
            }
            full.push_str(next.text);
            full.push('\n');
            if !own_done {
                own.push_str(next.text);
                own.push('\n');
            }
        }
        out.push(Section {
            title: title.to_owned(),
            line: line.number,
            own_text: own,
            full_text: full,
        });
    }
    out
}

pub fn find_section<'a>(sections: &'a [Section], title: &str) -> Option<&'a Section> {
    sections
        .iter()
        .find(|s| s.title.eq_ignore_ascii_case(title))
}

// ---- spec files --------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeltaOp {
    Added,
    Modified,
    Removed,
    Renamed,
}

impl DeltaOp {
    fn parse(title: &str) -> Option<Self> {
        let lower = title.to_ascii_lowercase();
        let word = lower.strip_suffix("requirements")?.trim();
        match word {
            "added" => Some(Self::Added),
            "modified" => Some(Self::Modified),
            "removed" => Some(Self::Removed),
            "renamed" => Some(Self::Renamed),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Added => "ADDED",
            Self::Modified => "MODIFIED",
            Self::Removed => "REMOVED",
            Self::Renamed => "RENAMED",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Scenario {
    pub name: String,
    pub line: u32,
    pub body: String,
}

#[derive(Debug, Clone, Default)]
pub struct Requirement {
    pub name: String,
    pub line: u32,
    /// The text between the heading and the first scenario.
    pub text: String,
    pub scenarios: Vec<Scenario>,
    /// Lines of `### Scenario:` headings, which `OpenSpec` silently ignores (scenarios are `####`).
    pub wrong_level_scenarios: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct Delta {
    pub op: DeltaOp,
    pub line: u32,
    pub requirements: Vec<Requirement>,
    /// Everything under the heading, for the shapes that are not requirements (RENAMED).
    pub text: String,
    /// Still taking lines: a heading of its own level ends it.
    open: bool,
}

impl Delta {
    fn new(op: DeltaOp, line: u32) -> Self {
        Self {
            op,
            line,
            requirements: Vec::new(),
            text: String::new(),
            open: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SpecFileDoc {
    pub purpose: Option<String>,
    pub deltas: Vec<Delta>,
    /// Requirements written outside any delta section (`(line, name)`).
    pub stray_requirements: Vec<(u32, String)>,
}

fn after_label<'a>(text: &'a str, label: &str) -> Option<&'a str> {
    let lower = text.to_ascii_lowercase();
    lower
        .starts_with(&label.to_ascii_lowercase())
        .then(|| text[label.len()..].trim())
}

/// Reads a spec file of a change (the delta format) or of `openspec/specs/` (requirements under
/// `## Requirements`, which are read as added).
pub fn parse_spec_file(content: &str) -> SpecFileDoc {
    let mut doc = SpecFileDoc::default();
    let mut in_purpose = false;
    let mut purpose = String::new();
    for line in lines(content) {
        if line.in_fence {
            absorb(&mut doc, line.text);
            continue;
        }
        let Some((level, title)) = heading(line.text) else {
            if in_purpose {
                purpose.push_str(line.text);
                purpose.push('\n');
            }
            absorb(&mut doc, line.text);
            continue;
        };
        match level {
            2 => {
                in_purpose = false;
                end_delta(&mut doc);
                if title.eq_ignore_ascii_case("purpose") {
                    in_purpose = true;
                } else if title.eq_ignore_ascii_case("requirements") {
                    // A main spec: its requirements are the capability's, as if added.
                    doc.deltas.push(Delta::new(DeltaOp::Added, line.number));
                } else if let Some(op) = DeltaOp::parse(title) {
                    doc.deltas.push(Delta::new(op, line.number));
                }
            }
            3 => {
                if let Some(name) = after_label(title, "Requirement:") {
                    open_requirement(&mut doc, line.number, name);
                } else if after_label(title, "Scenario:").is_some() {
                    if let Some(req) = current_requirement(&mut doc) {
                        req.wrong_level_scenarios.push(line.number);
                    }
                }
            }
            4 => {
                if let Some(name) = after_label(title, "Scenario:") {
                    if let Some(req) = current_requirement(&mut doc) {
                        req.scenarios.push(Scenario {
                            name: name.to_owned(),
                            line: line.number,
                            body: String::new(),
                        });
                    }
                }
            }
            _ => {}
        }
        absorb(&mut doc, line.text);
    }
    let purpose = purpose.trim().to_owned();
    doc.purpose = (!purpose.is_empty()).then_some(purpose);
    doc
}

/// After a `## Something else` heading no delta is current any more.
fn end_delta(doc: &mut SpecFileDoc) {
    if let Some(last) = doc.deltas.last_mut() {
        last.open = false;
    }
}

fn current_delta(doc: &mut SpecFileDoc) -> Option<&mut Delta> {
    doc.deltas.last_mut().filter(|d| d.open)
}

fn current_requirement(doc: &mut SpecFileDoc) -> Option<&mut Requirement> {
    current_delta(doc).and_then(|d| d.requirements.last_mut())
}

fn open_requirement(doc: &mut SpecFileDoc, line: u32, name: &str) {
    let requirement = Requirement {
        name: name.to_owned(),
        line,
        ..Requirement::default()
    };
    match current_delta(doc) {
        Some(delta) => delta.requirements.push(requirement),
        None => doc.stray_requirements.push((line, name.to_owned())),
    }
}

/// Adds a line of the body to whatever is open: the scenario, else the requirement's text.
fn absorb(doc: &mut SpecFileDoc, text: &str) {
    let Some(delta) = current_delta(doc) else {
        return;
    };
    let is_requirement_heading =
        heading(text).is_some_and(|(l, t)| l == 3 && after_label(t, "Requirement:").is_some());
    if is_requirement_heading {
        return;
    }
    delta.text.push_str(text);
    delta.text.push('\n');
    let Some(requirement) = delta.requirements.last_mut() else {
        return;
    };
    let is_heading = heading(text).is_some();
    if let Some(scenario) = requirement.scenarios.last_mut() {
        if !(is_heading && text.trim_start().starts_with("####")) {
            scenario.body.push_str(text);
            scenario.body.push('\n');
        }
    } else if !is_heading {
        requirement.text.push_str(text);
        requirement.text.push('\n');
    }
}

/// The content of the scenario's WHEN and THEN lines: `- **WHEN** x`, `- WHEN x`, `**WHEN** x`.
pub fn scenario_steps(body: &str) -> (bool, bool) {
    let mut when = false;
    let mut then = false;
    for line in body.lines() {
        let word = line
            .trim()
            .trim_start_matches(['-', '*', '+', ' '])
            .trim_start_matches('*')
            .trim_start();
        let upper: String = word
            .chars()
            .take_while(char::is_ascii_alphabetic)
            .collect::<String>()
            .to_ascii_uppercase();
        match upper.as_str() {
            "WHEN" | "GIVEN" => when = true,
            "THEN" => then = true,
            _ => {}
        }
    }
    (when, then)
}

/// The steps of a scenario as written: `(KEYWORD, text)` for each `WHEN`, `GIVEN`, `THEN`, `AND` or
/// `BUT` line. Lines that are not steps are left out.
pub fn scenario_step_list(body: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in body.lines() {
        let word = line
            .trim()
            .trim_start_matches(['-', '*', '+', ' '])
            .trim_start_matches('*')
            .trim_start();
        let keyword: String = word
            .chars()
            .take_while(char::is_ascii_alphabetic)
            .collect::<String>()
            .to_ascii_uppercase();
        if !matches!(keyword.as_str(), "WHEN" | "GIVEN" | "THEN" | "AND" | "BUT") {
            continue;
        }
        let rest = word[keyword.len()..]
            .trim_start_matches('*')
            .trim_start()
            .trim();
        out.push((keyword, rest.to_owned()));
    }
    out
}

// ---- tasks ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskLine {
    pub line: u32,
    /// `1.2` when the description starts with a number, else `t<n>` by position.
    pub id: String,
    /// Whether the id is the task's own number or a positional one.
    pub numbered: bool,
    pub done: bool,
    /// The description, without the checkbox and the number.
    pub text: String,
    /// The `## N.` group the task sits under, if any.
    pub group: Option<String>,
}

/// The checkbox tasks of `tasks.md`: `- [ ] 1.1 Description`. A box holding only `x` is done.
pub fn parse_tasks(content: &str) -> Vec<TaskLine> {
    let mut out: Vec<TaskLine> = Vec::new();
    let mut group: Option<String> = None;
    for line in lines(content) {
        if line.in_fence {
            continue;
        }
        if let Some((2, title)) = heading(line.text) {
            group = title
                .split('.')
                .next()
                .filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
                .map(str::to_owned);
            continue;
        }
        let trimmed = line.text.trim_start();
        let Some(rest) = trimmed
            .strip_prefix("- [")
            .or_else(|| trimmed.strip_prefix("* ["))
        else {
            continue;
        };
        let Some(close) = rest.find(']') else {
            continue;
        };
        let (mark, after) = (&rest[..close], &rest[close + 1..]);
        if !after.starts_with(char::is_whitespace) && !after.is_empty() {
            continue;
        }
        let done = mark.trim().eq_ignore_ascii_case("x");
        if !done && !mark.trim().is_empty() {
            // `- [~]`, `- [-]`: not a box OpenSpec counts, and not one that is checked.
            continue;
        }
        let description = after.trim();
        let (id, numbered, text) = split_task_id(description, out.len() + 1);
        out.push(TaskLine {
            line: line.number,
            id,
            numbered,
            done,
            text,
            group: group.clone(),
        });
    }
    out
}

fn split_task_id(description: &str, position: usize) -> (String, bool, String) {
    let first: String = description
        .chars()
        .take_while(|c| !c.is_whitespace())
        .collect();
    let token = first.trim_end_matches(['.', ':', ')']);
    let numbered = token.contains('.')
        && token
            .split('.')
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric()))
        && token.chars().next().is_some_and(|c| c.is_ascii_digit());
    if numbered {
        (
            token.to_owned(),
            true,
            description[first.len()..].trim().to_owned(),
        )
    } else {
        (format!("t{position}"), false, description.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: &str = "# Spec Delta\n\n## Purpose\n\nLets users take their data out of the product.\n\n## ADDED Requirements\n\n### Requirement: User can export data\nThe system SHALL allow export.\n\n#### Scenario: Successful export\n- **WHEN** the user clicks Export\n- **THEN** a CSV downloads\n\n## REMOVED Requirements\n\n### Requirement: Legacy export\n**Reason**: replaced\n**Migration**: use v2\n";

    #[test]
    fn headings_inside_a_code_fence_are_not_headings() {
        let content = "# A\n```\n## Not a heading\n```\n## B\n";

        let found: Vec<(u32, bool)> = lines(content)
            .iter()
            .map(|l| (l.number, l.in_fence))
            .collect();

        assert_eq!(
            found,
            vec![(1, false), (2, true), (3, true), (4, true), (5, false)]
        );
        let titles: Vec<String> = sections(content, 2).into_iter().map(|s| s.title).collect();
        assert_eq!(titles, vec!["B".to_owned()]);
    }

    #[test]
    fn a_spec_file_is_read_into_deltas_requirements_and_scenarios() {
        let doc = parse_spec_file(SPEC);

        assert!(doc.purpose.as_deref().unwrap().starts_with("Lets users"));
        assert_eq!(doc.deltas.len(), 2);
        let added = &doc.deltas[0];
        assert_eq!(added.op, DeltaOp::Added);
        assert_eq!(added.requirements[0].name, "User can export data");
        assert!(added.requirements[0].text.contains("SHALL"));
        assert_eq!(added.requirements[0].scenarios.len(), 1);
        assert_eq!(
            scenario_steps(&added.requirements[0].scenarios[0].body),
            (true, true)
        );
        assert_eq!(doc.deltas[1].op, DeltaOp::Removed);
        assert!(doc.deltas[1].requirements[0].text.contains("**Reason**"));
    }

    #[test]
    fn a_scenario_with_three_hashes_is_reported_not_counted() {
        let doc = parse_spec_file(
            "## ADDED Requirements\n### Requirement: X\nThe system SHALL x.\n### Scenario: wrong\n- **WHEN** a\n- **THEN** b\n",
        );

        let req = &doc.deltas[0].requirements[0];
        assert!(req.scenarios.is_empty());
        assert_eq!(req.wrong_level_scenarios, vec![4]);
    }

    #[test]
    fn a_requirement_outside_a_delta_is_reported() {
        let doc = parse_spec_file("# Spec\n### Requirement: Loose\nThe system SHALL x.\n");

        assert_eq!(doc.stray_requirements, vec![(2, "Loose".to_owned())]);
    }

    #[test]
    fn a_main_spec_reads_its_requirements_as_added() {
        let doc = parse_spec_file(
            "## Purpose\nx\n\n## Requirements\n### Requirement: A\nThe system SHALL a.\n#### Scenario: s\n- **WHEN** a\n- **THEN** b\n",
        );

        assert_eq!(doc.deltas[0].requirements[0].name, "A");
    }

    #[test]
    fn a_heading_after_a_delta_ends_it() {
        let doc = parse_spec_file(
            "## ADDED Requirements\n### Requirement: A\nThe system SHALL a.\n\n## Notes\n### Requirement: B\nThe system SHALL b.\n",
        );

        assert_eq!(doc.deltas[0].requirements.len(), 1);
        assert_eq!(doc.stray_requirements.len(), 1);
    }

    #[test]
    fn scenario_steps_accept_the_bullet_shapes_openspec_documents() {
        assert_eq!(scenario_steps("- **WHEN** a\n- **THEN** b\n"), (true, true));
        assert_eq!(scenario_steps("- WHEN a\n- THEN b\n- AND c"), (true, true));
        assert_eq!(scenario_steps("**WHEN** a\n**THEN** b"), (true, true));
        assert_eq!(
            scenario_steps("- the user clicks\n- it works"),
            (false, false)
        );
        assert_eq!(scenario_steps("- **WHEN** a only"), (true, false));
    }

    #[test]
    fn the_steps_of_a_scenario_are_listed_with_their_keywords() {
        let steps = scenario_step_list(
            "- **WHEN** the user clicks\n- **THEN** a file downloads\nnoise\n- AND it is a CSV",
        );

        assert_eq!(
            steps,
            vec![
                ("WHEN".to_owned(), "the user clicks".to_owned()),
                ("THEN".to_owned(), "a file downloads".to_owned()),
                ("AND".to_owned(), "it is a CSV".to_owned()),
            ]
        );
    }

    #[test]
    fn tasks_are_the_checkbox_lines_and_only_an_x_is_done() {
        let tasks = parse_tasks(
            "# Tasks\n\n## 1. Setup\n\n- [ ] 1.1 Create module\n- [x] 1.2 Add deps\n- [ x] 1.3 Spaced\n- [~] 1.4 Odd\n- not a task\n\n## 2. Core\n\n- [ ] 2.1 Do it (Requirement: A)\n",
        );

        let ids: Vec<&str> = tasks.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["1.1", "1.2", "1.3", "2.1"]);
        assert_eq!(
            tasks.iter().map(|t| t.done).collect::<Vec<_>>(),
            vec![false, true, true, false]
        );
        assert_eq!(tasks[0].group.as_deref(), Some("1"));
        assert_eq!(tasks[3].group.as_deref(), Some("2"));
        assert_eq!(tasks[0].text, "Create module");
    }

    #[test]
    fn an_unnumbered_task_gets_a_positional_id() {
        let tasks = parse_tasks("- [ ] Write the thing\n- [ ] 2.1 Numbered\n");

        assert_eq!(tasks[0].id, "t1");
        assert!(!tasks[0].numbered);
        assert_eq!(tasks[1].id, "2.1");
        assert!(tasks[1].numbered);
    }

    #[test]
    fn checkboxes_in_a_code_fence_are_not_tasks() {
        let tasks = parse_tasks("```\n- [ ] 1.1 example\n```\n- [ ] 1.1 real\n");

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].text, "real");
    }
}
