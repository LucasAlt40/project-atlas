//! Changing a document by sections instead of writing it again (ADR 0036). When a person asks for
//! a change to a long document, the agent answers with the sections that change and Atlas splices
//! them into the document it already has. Everything outside those sections is the same bytes as
//! before, by construction, and a change that reaches outside the section it names is refused.
//!
//! The agent never touches a file: it names a heading and gives the text of the section. A
//! heading that is not there, or is there twice, or an answer that cannot be applied cleanly, is a
//! problem Atlas reports; the next try is a whole document, which always works.

use crate::domain::spec::SpecFile;

/// The line that opens an edit: `<<<ATLAS-EDIT path="design.md" op="replace" heading="## Risks">>>`.
pub const EDIT_OPEN: &str = "<<<ATLAS-EDIT";
const EDIT_OPEN_END: &str = ">>>";
/// The line that closes it.
pub const EDIT_CLOSE: &str = "<<<END-ATLAS-EDIT>>>";
/// More than this and the document is being rewritten in pieces: say so with a whole document.
const MAX_EDITS: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditOp {
    /// The section (its heading and everything under it) becomes the text given.
    Replace,
    /// The text given is a new section, placed after the whole section named.
    InsertAfter,
    /// The section is removed.
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub path: String,
    pub op: EditOp,
    /// The heading line of the section, as written in the document.
    pub heading: String,
    pub body: String,
}

/// Why an answer's edits could not be applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditProblem {
    /// An edit was opened and never closed: the answer was cut off.
    Unterminated,
    /// The opening line is not in the agreed shape, or an edit has no text where it needs it.
    Malformed(String),
    TooMany,
    /// The edit names a file the document does not have.
    UnknownFile(String),
    HeadingNotFound(String),
    /// Two sections have the heading named, so Atlas cannot tell which was meant.
    HeadingAmbiguous(String),
    /// The text reaches outside the section it replaces, or removes something that is not a
    /// section (`path: heading`).
    OutOfScope(String),
}

impl EditProblem {
    /// The code the screens word, and what it is about.
    pub fn code(&self) -> (&'static str, String) {
        match self {
            Self::Unterminated => ("output_edit_cut_off", String::new()),
            Self::Malformed(what) => ("output_edit_malformed", what.clone()),
            Self::TooMany => ("output_edit_too_many", String::new()),
            Self::UnknownFile(path) => ("output_edit_unknown_file", path.clone()),
            Self::HeadingNotFound(heading) => ("output_edit_heading_missing", heading.clone()),
            Self::HeadingAmbiguous(heading) => ("output_edit_heading_ambiguous", heading.clone()),
            Self::OutOfScope(what) => ("output_edit_out_of_scope", what.clone()),
        }
    }
}

fn attribute(line: &str, name: &str) -> Option<String> {
    let key = format!("{name}=\"");
    let start = line.find(&key)? + key.len();
    let rest = &line[start..];
    // A heading can hold a quote; the attribute ends at the quote that is followed by another
    // attribute or by the end of the line.
    let mut end = None;
    for (index, _) in rest.match_indices('"') {
        let after = rest[index + 1..].trim_start();
        if after.is_empty()
            || after.starts_with("op=")
            || after.starts_with("heading=")
            || after.starts_with("path=")
        {
            end = Some(index);
            break;
        }
    }
    end.map(|e| rest[..e].to_owned())
}

/// What an opening line says (`path`, `op`, `heading`), or the line itself when it says nothing usable.
type Opening = Result<(String, EditOp, String), String>;

/// The edits in an answer, and a problem if the answer ends in the middle of one.
pub fn read_edits(output: &str) -> (Vec<Edit>, Option<EditProblem>) {
    let mut edits = Vec::new();
    let mut open: Option<(Opening, Vec<&str>)> = None;
    for line in output.lines() {
        let trimmed = line.trim();
        match &mut open {
            None => {
                let Some(rest) = trimmed.strip_prefix(EDIT_OPEN) else {
                    continue;
                };
                let Some(attributes) = rest.strip_suffix(EDIT_OPEN_END) else {
                    continue;
                };
                let head = (|| {
                    let path = attribute(attributes, "path")?;
                    let op = match attribute(attributes, "op")?.as_str() {
                        "replace" => EditOp::Replace,
                        "insert_after" => EditOp::InsertAfter,
                        "delete" => EditOp::Delete,
                        _ => return None,
                    };
                    let heading = attribute(attributes, "heading")?;
                    Some((path, op, heading))
                })()
                .ok_or_else(|| attributes.trim().chars().take(120).collect::<String>());
                open = Some((head, Vec::new()));
            }
            Some((head, body)) => {
                if trimmed == EDIT_CLOSE {
                    match head {
                        Ok((path, op, heading)) => edits.push(Edit {
                            path: path.clone(),
                            op: *op,
                            heading: heading.clone(),
                            body: body.join("\n").trim().to_owned(),
                        }),
                        Err(shown) => {
                            return (edits, Some(EditProblem::Malformed(shown.clone())));
                        }
                    }
                    open = None;
                } else {
                    body.push(line.trim_end_matches('\r'));
                }
            }
        }
    }
    (edits, open.map(|_| EditProblem::Unterminated))
}

/// The level of a Markdown heading line outside a code fence (`## x` is 2), and its text.
fn heading_of(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_end();
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &trimmed[hashes..];
    (rest.starts_with(' ') && !rest.trim().is_empty()).then_some((hashes, rest.trim()))
}

/// Per line: whether it is a heading (level) outside any code fence.
fn headings(lines: &[String]) -> Vec<Option<usize>> {
    let mut fenced = false;
    lines
        .iter()
        .map(|line| {
            let t = line.trim_start();
            if t.starts_with("```") || t.starts_with("~~~") {
                fenced = !fenced;
                return None;
            }
            if fenced {
                return None;
            }
            heading_of(line).map(|(level, _)| level)
        })
        .collect()
}

fn same_heading(line: &str, wanted: &str) -> bool {
    let squash = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    squash(line) == squash(wanted)
}

/// `[start, end)` of the section whose heading line is `heading`: up to the next heading of the
/// same or a higher level.
fn find_section(lines: &[String], heading: &str) -> Result<(usize, usize, usize), EditProblem> {
    let levels = headings(lines);
    let wanted = heading.trim();
    let hits: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(i, line)| levels[*i].is_some() && same_heading(line, wanted))
        .map(|(i, _)| i)
        .collect();
    let start = match hits.as_slice() {
        [] => return Err(EditProblem::HeadingNotFound(heading.to_owned())),
        [one] => *one,
        _ => return Err(EditProblem::HeadingAmbiguous(heading.to_owned())),
    };
    let level = levels[start].unwrap_or(1);
    let end = (start + 1..lines.len())
        .find(|i| levels[*i].is_some_and(|l| l <= level))
        .unwrap_or(lines.len());
    Ok((start, end, level))
}

fn text_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.trim_end_matches('\r').to_owned())
        .collect()
}

/// The lines of a new section: trimmed of blank lines, starting at a heading.
fn section_body(edit: &Edit, scope: &str) -> Result<Vec<String>, EditProblem> {
    let lines = text_lines(edit.body.trim());
    let levels = headings(&lines);
    match levels.first() {
        Some(Some(_)) => Ok(lines),
        _ => Err(EditProblem::Malformed(format!(
            "{scope}: the text must start with the section's heading"
        ))),
    }
}

fn apply_one(file: &mut Vec<String>, edit: &Edit) -> Result<(), EditProblem> {
    let scope = format!("{}: {}", edit.path, edit.heading);
    let (start, end, level) = find_section(file, &edit.heading)?;
    match edit.op {
        EditOp::Delete => {
            if level == 1 {
                return Err(EditProblem::OutOfScope(scope));
            }
            file.drain(start..end);
        }
        EditOp::Replace => {
            let body = section_body(edit, &scope)?;
            let levels = headings(&body);
            // The new text is this section: its heading is of the same level, and nothing in it
            // is a section of the same or a higher level, which would be someone else's.
            if levels[0] != Some(level) || levels.iter().skip(1).flatten().any(|l| *l <= level) {
                return Err(EditProblem::OutOfScope(scope));
            }
            // The blank lines that set the section off from the next one stay as they were.
            let blanks = file[start..end]
                .iter()
                .rev()
                .take_while(|l| l.trim().is_empty())
                .count();
            let mut replacement = body;
            replacement.extend(std::iter::repeat_n(String::new(), blanks));
            file.splice(start..end, replacement);
        }
        EditOp::InsertAfter => {
            let body = section_body(edit, &scope)?;
            // After the section's last line of text: the blank lines that set it off from what
            // follows stay where they are, now after the new section.
            let mut at = end;
            while at > start + 1 && file[at - 1].trim().is_empty() {
                at -= 1;
            }
            let mut inserted = vec![String::new()];
            inserted.extend(body);
            file.splice(at..at, inserted);
        }
    }
    Ok(())
}

/// The files with the edits applied, in order. A file the edits do not name is returned as it is.
///
/// # Errors
///
/// The first edit that cannot be applied cleanly; nothing is returned of the others.
pub fn apply(files: &[SpecFile], edits: &[Edit]) -> Result<Vec<SpecFile>, EditProblem> {
    if edits.len() > MAX_EDITS {
        return Err(EditProblem::TooMany);
    }
    let mut work: Vec<(String, Vec<String>)> = files
        .iter()
        .map(|f| (f.path.clone(), text_lines(&f.content)))
        .collect();
    for edit in edits {
        let Some((_, lines)) = work.iter_mut().find(|(path, _)| *path == edit.path) else {
            return Err(EditProblem::UnknownFile(edit.path.clone()));
        };
        apply_one(lines, edit)?;
    }
    Ok(work
        .into_iter()
        .zip(files)
        .map(|((path, lines), original)| {
            let content = lines.join("\n");
            // Only what changed is rewritten: the rest keeps its exact bytes, ending included.
            if content.trim() == text_lines(&original.content).join("\n").trim() {
                SpecFile {
                    path,
                    content: original.content.clone(),
                }
            } else {
                SpecFile {
                    path,
                    content: content.trim().to_owned(),
                }
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "# Design\n\n## Context\nold context\n\n## Decisions\n### Decision 1\none\n\n### Decision 2\ntwo\n\n## Risks\nrisk\n";

    fn file(content: &str) -> Vec<SpecFile> {
        vec![SpecFile {
            path: "design.md".to_owned(),
            content: content.to_owned(),
        }]
    }

    fn edit(op: EditOp, heading: &str, body: &str) -> Edit {
        Edit {
            path: "design.md".to_owned(),
            op,
            heading: heading.to_owned(),
            body: body.to_owned(),
        }
    }

    fn applied(edits: &[Edit]) -> Result<String, EditProblem> {
        apply(&file(DOC), edits).map(|files| files[0].content.clone())
    }

    #[test]
    fn edits_are_read_from_the_answer_with_their_text() {
        let answer = "note\n<<<ATLAS-EDIT path=\"design.md\" op=\"replace\" heading=\"## Context\">>>\n## Context\nnew\n<<<END-ATLAS-EDIT>>>\n<<<ATLAS-EDIT path=\"design.md\" op=\"delete\" heading=\"## Risks\">>>\n<<<END-ATLAS-EDIT>>>\n";

        let (edits, problem) = read_edits(answer);

        assert_eq!(problem, None);
        assert_eq!(
            edits,
            vec![
                edit(EditOp::Replace, "## Context", "## Context\nnew"),
                edit(EditOp::Delete, "## Risks", ""),
            ]
        );
    }

    #[test]
    fn a_heading_with_a_quote_in_it_is_still_read() {
        let (edits, _) = read_edits(
            "<<<ATLAS-EDIT path=\"a.md\" op=\"replace\" heading=\"## The \"fast\" path\">>>\n## The \"fast\" path\nx\n<<<END-ATLAS-EDIT>>>",
        );

        assert_eq!(edits[0].heading, "## The \"fast\" path");
    }

    #[test]
    fn an_answer_cut_off_inside_an_edit_is_a_problem_and_a_bad_opening_line_is_too() {
        let (edits, problem) =
            read_edits("<<<ATLAS-EDIT path=\"d.md\" op=\"replace\" heading=\"## A\">>>\n## A\ncut");
        assert_eq!(edits, Vec::<Edit>::new());
        assert_eq!(problem, Some(EditProblem::Unterminated));

        let (_, problem) = read_edits("<<<ATLAS-EDIT path=\"d.md\" op=\"rewrite\" heading=\"## A\">>>\nx\n<<<END-ATLAS-EDIT>>>");
        assert!(matches!(problem, Some(EditProblem::Malformed(_))));
    }

    #[test]
    fn replacing_a_section_changes_that_section_and_nothing_else() {
        let result = applied(&[edit(
            EditOp::Replace,
            "## Context",
            "## Context\nnew context\nmore",
        )])
        .unwrap();

        assert_eq!(
            result,
            "# Design\n\n## Context\nnew context\nmore\n\n## Decisions\n### Decision 1\none\n\n### Decision 2\ntwo\n\n## Risks\nrisk"
        );
    }

    #[test]
    fn a_section_holds_its_subsections() {
        let result = applied(&[edit(
            EditOp::Replace,
            "## Decisions",
            "## Decisions\n### Only decision\nthe one",
        )])
        .unwrap();

        assert!(result.contains("### Only decision"));
        assert!(!result.contains("Decision 1"));
        assert!(result.ends_with("## Risks\nrisk"));
    }

    #[test]
    fn a_subsection_can_be_changed_alone() {
        let result = applied(&[edit(
            EditOp::Replace,
            "### Decision 2",
            "### Decision 2\nchanged",
        )])
        .unwrap();

        assert!(result.contains("### Decision 1\none"));
        assert!(result.contains("### Decision 2\nchanged"));
        assert!(result.contains("## Risks"));
    }

    #[test]
    fn text_that_reaches_a_section_of_the_same_level_is_out_of_scope() {
        let result = applied(&[edit(
            EditOp::Replace,
            "## Context",
            "## Context\nnew\n\n## Risks\nsneaked in",
        )]);

        assert!(matches!(result, Err(EditProblem::OutOfScope(_))));
    }

    #[test]
    fn the_new_text_must_start_with_a_heading_of_the_same_level() {
        assert!(matches!(
            applied(&[edit(EditOp::Replace, "## Context", "no heading")]),
            Err(EditProblem::Malformed(_))
        ));
        assert!(matches!(
            applied(&[edit(EditOp::Replace, "## Context", "### Context\nx")]),
            Err(EditProblem::OutOfScope(_))
        ));
    }

    #[test]
    fn a_section_can_be_added_after_another_and_removed() {
        let added = applied(&[edit(
            EditOp::InsertAfter,
            "## Context",
            "## Constraints\nnone",
        )])
        .unwrap();
        assert!(added.contains("old context\n\n## Constraints\nnone\n\n## Decisions"));

        let removed = applied(&[edit(EditOp::Delete, "## Risks", "")]).unwrap();
        assert!(!removed.contains("Risks"));
        assert!(removed.contains("### Decision 2\ntwo"));
    }

    #[test]
    fn the_title_cannot_be_deleted() {
        assert!(matches!(
            applied(&[edit(EditOp::Delete, "# Design", "")]),
            Err(EditProblem::OutOfScope(_))
        ));
    }

    #[test]
    fn an_unknown_heading_a_repeated_heading_and_an_unknown_file_are_problems() {
        assert_eq!(
            applied(&[edit(EditOp::Replace, "## Nowhere", "## Nowhere\nx")]),
            Err(EditProblem::HeadingNotFound("## Nowhere".to_owned()))
        );
        let twice = apply(
            &file("# T\n\n## A\nx\n\n## A\ny\n"),
            &[edit(EditOp::Replace, "## A", "## A\nz")],
        );
        assert_eq!(twice, Err(EditProblem::HeadingAmbiguous("## A".to_owned())));
        let mut elsewhere = edit(EditOp::Delete, "## Risks", "");
        elsewhere.path = "tasks.md".to_owned();
        assert_eq!(
            apply(&file(DOC), &[elsewhere]),
            Err(EditProblem::UnknownFile("tasks.md".to_owned()))
        );
    }

    #[test]
    fn a_heading_inside_a_code_fence_is_not_a_section() {
        let doc = "# T\n\n## A\n```\n## A\n```\ntext\n\n## B\nb\n";
        let result = apply(
            &file(doc),
            &[edit(EditOp::Replace, "## A", "## A\nreplaced")],
        )
        .unwrap();

        assert_eq!(result[0].content, "# T\n\n## A\nreplaced\n\n## B\nb");
    }

    #[test]
    fn edits_apply_in_order_and_a_failed_one_returns_nothing() {
        let both = applied(&[
            edit(EditOp::Replace, "## Context", "## Context\nfirst"),
            edit(EditOp::Replace, "## Risks", "## Risks\nsecond"),
        ])
        .unwrap();
        assert!(both.contains("first") && both.contains("second"));

        let failed = applied(&[
            edit(EditOp::Replace, "## Context", "## Context\nfirst"),
            edit(EditOp::Replace, "## Nowhere", "## Nowhere\nx"),
        ]);
        assert!(failed.is_err());
    }

    #[test]
    fn a_file_that_no_edit_names_keeps_every_byte() {
        let files = vec![
            SpecFile {
                path: "a.md".to_owned(),
                content: "# A\n\n## S\nx\n".to_owned(),
            },
            SpecFile {
                path: "b.md".to_owned(),
                content: "# B  \r\n\nkeep\n".to_owned(),
            },
        ];
        let mut e = edit(EditOp::Replace, "## S", "## S\ny");
        e.path = "a.md".to_owned();

        let out = apply(&files, &[e]).unwrap();

        assert_eq!(out[1], files[1]);
        assert_eq!(out[0].content, "# A\n\n## S\ny");
    }
}
