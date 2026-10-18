//! `OpenSpec` as a [`SpecFramework`]: the `spec-driven` schema (proposal → specs → design → tasks),
//! the delta format of its spec files, the checks of `openspec validate`, and the layout of
//! `openspec/changes/<id>/`. Everything format-specific is here.
//!
//! Atlas does not call `OpenSpec`'s CLI to do any of this: the rules are Atlas's own reading of the
//! format, so a machine without Node.js writes and checks the same change (ADR 0035). The CLI is
//! an extra check, in `cli.rs`.

pub mod markdown;
mod text;
pub mod validate;

use std::collections::BTreeMap;
use std::fmt::Write;

use super::edits::{self, EDIT_CLOSE, EDIT_OPEN};
use super::framework::{
    ArtifactInstructions, ChangeFolder, ChangeIdProblem, Documents, FileOutline, ImportProblem,
    ImportedChange, OutputProblem, ProjectSpecs, PublishFile, RequirementOutline, ScenarioOutline,
    SpecFramework, StepOutline, TaskItem,
};
use crate::domain::spec::{
    ArtifactKind, FrameworkKind, SpecFile, ValidationIssue, MAX_FILE, MAX_FILES,
};

const DOC_OPEN: &str = "<<<ATLAS-DOCUMENT path=\"";
const DOC_OPEN_END: &str = "\">>>";
pub const DOC_CLOSE: &str = "<<<END-ATLAS-DOCUMENT>>>";
const SCHEMA: &str = "spec-driven";
const MAX_CHANGE_ID: usize = 64;

pub struct OpenSpecAdapter;

/// How a document is wrapped in an agent's answer, so Atlas reads it with no guessing and a
/// document that has code fences inside it is no problem.
pub fn document_protocol(kind: ArtifactKind) -> String {
    let example = match kind {
        ArtifactKind::Proposal => "proposal.md",
        ArtifactKind::Design => "design.md",
        ArtifactKind::Tasks => "tasks.md",
        ArtifactKind::Specs => "specs/<capability-path>/spec.md",
    };
    let many = if kind == ArtifactKind::Specs {
        " Write one such block per file."
    } else {
        ""
    };
    format!(
        "{DOC_OPEN}{example}{DOC_OPEN_END}\n<the complete Markdown of the file>\n{DOC_CLOSE}\n\
         Put each file between those two marker lines, exactly as shown, with the file's path in the \
         first one.{many} Nothing is read from outside the markers, so any note for the person \
         goes before the first block. Do not wrap the document in a code fence."
    )
}

/// Whether a file of this name can be part of the artifact.
fn path_belongs(kind: ArtifactKind, path: &str) -> bool {
    match kind {
        ArtifactKind::Proposal => path == "proposal.md",
        ArtifactKind::Design => path == "design.md",
        ArtifactKind::Tasks => path == "tasks.md",
        ArtifactKind::Specs => validate::capability_of(path).is_some(),
    }
}

/// How an agent may hand back only the sections that change.
fn edit_protocol(kind: ArtifactKind) -> String {
    let example = match kind {
        ArtifactKind::Proposal => "proposal.md",
        ArtifactKind::Design => "design.md",
        ArtifactKind::Tasks => "tasks.md",
        ArtifactKind::Specs => "specs/<capability-path>/spec.md",
    };
    format!(
        "Give back ONLY the sections that change, not the whole document. One block per section:\n\
         {EDIT_OPEN} path=\"{example}\" op=\"replace\" heading=\"## The heading, exactly as written\">>>\n\
         <the complete new text of that section, starting with its heading line>\n\
         {EDIT_CLOSE}\n\
         - `op=\"replace\"`: the section (the heading and everything under it, subsections included) \
         becomes the text you give. It starts with a heading of the same level and holds no heading of \
         the same or a higher level.\n\
         - `op=\"insert_after\"`: the text is a new section, placed after the whole section named.\n\
         - `op=\"delete\"`: the section is removed; the block has no text.\n\
         `heading` is the heading line of an existing section, exactly as it is in the document and \
         unique in it. Everything you do not name stays as it is, so do not repeat it. If the change \
         reaches the whole document, or you cannot say which sections change, write the COMPLETE \
         document instead, in the format above. Any note for the person goes before the first block."
    )
}

fn is_safe_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains(['\\', '\0', ':'])
        && path
            .split('/')
            .all(|s| !s.is_empty() && s != "." && s != "..")
}

/// `(path, content)` of every complete document block in the answer, and the path of one that was
/// opened and never closed.
fn read_blocks(output: &str) -> (Vec<(String, String)>, Option<String>) {
    let mut blocks = Vec::new();
    let mut open: Option<(String, Vec<&str>)> = None;
    for line in output.lines() {
        let trimmed = line.trim();
        match &mut open {
            None => {
                if let Some(rest) = trimmed.strip_prefix(DOC_OPEN) {
                    if let Some(path) = marker_end(rest, "\"") {
                        open = Some((path.trim().to_owned(), Vec::new()));
                    }
                }
            }
            Some((path, body)) => {
                if is_close(trimmed) {
                    blocks.push((path.clone(), clean_document(&body.join("\n"))));
                    open = None;
                } else {
                    body.push(line.trim_end_matches('\r'));
                }
            }
        }
    }
    (blocks, open.map(|(path, _)| path))
}

/// What comes before the end of a marker: `before` then one to three `>`. Models write the
/// documented `">>>` but small ones also drop a `>` (measured: `path="tasks.md">>`); the marker
/// still fills its line and its path is still quoted, so it cannot be mistaken for text.
fn marker_end<'a>(rest: &'a str, before: &str) -> Option<&'a str> {
    let without = rest.trim_end_matches('>');
    let arrows = rest.len() - without.len();
    (1..=DOC_OPEN_END.len() - 1)
        .contains(&arrows)
        .then(|| without.strip_suffix(before))
        .flatten()
}

/// Whether `line` closes a document: [`DOC_CLOSE`], or the same with one or two `>` dropped.
fn is_close(line: &str) -> bool {
    line == DOC_CLOSE
        || line
            .strip_prefix(DOC_CLOSE.trim_end_matches('>'))
            .is_some_and(|rest| marker_end(rest, "").is_some())
}

/// The document without blank lines around it and without a code fence an agent wrapped it in
/// despite being told not to.
fn clean_document(text: &str) -> String {
    let trimmed = text.trim();
    for opener in ["```markdown", "```md", "```"] {
        if let Some(rest) = trimmed.strip_prefix(opener) {
            if let Some(inner) = rest.trim_start_matches([' ', '\t']).strip_prefix('\n') {
                if let Some(inner) = inner.trim_end().strip_suffix("```") {
                    // Only when the fence encloses the whole document.
                    if !inner.contains("\n```") || inner.matches("```").count() % 2 == 0 {
                        return inner.trim().to_owned();
                    }
                }
            }
        }
    }
    trimmed.to_owned()
}

/// `YYYY-MM-DD` of a time in milliseconds since the Unix epoch (UTC), for `.openspec.yaml`.
pub fn iso_date(ms: u64) -> String {
    // Days since 1970-01-01 → civil date (Howard Hinnant's algorithm).
    let days = i64::try_from(ms / 86_400_000).unwrap_or(0) + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = if month <= 2 { year + 1 } else { year };
    format!("{year:04}-{month:02}-{day:02}")
}

fn ascii_letter(c: char) -> Option<char> {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => Some('a'),
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => Some('e'),
        'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => Some('i'),
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => Some('o'),
        'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => Some('u'),
        'ç' | 'Ç' => Some('c'),
        'ñ' | 'Ñ' => Some('n'),
        _ => None,
    }
}

fn slug(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars() {
        let c = ascii_letter(c).unwrap_or(c);
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-');
    // Short enough to read in a list, cut at a hyphen when it can be.
    if trimmed.len() <= 48 {
        return trimmed.to_owned();
    }
    let cut = &trimmed[..48];
    cut.rsplit_once('-')
        .map_or(cut, |(head, _)| head)
        .trim_matches('-')
        .to_owned()
}

fn clip(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn section_text(content: &str, level: usize, title: &str) -> String {
    let clean = validate::without_comments(content);
    let all = markdown::sections(&clean, level);
    markdown::find_section(&all, title)
        .map(|s| s.full_text.trim().to_owned())
        .unwrap_or_default()
}

impl SpecFramework for OpenSpecAdapter {
    fn kind(&self) -> FrameworkKind {
        FrameworkKind::Openspec
    }

    fn schema(&self) -> &'static str {
        "spec-driven/1"
    }

    fn validator(&self) -> &'static str {
        "atlas-openspec/1"
    }

    /// `OpenSpec` lets `design` be written beside `specs`; Atlas puts it after them, so the design
    /// can cite requirements the person already approved. The files are the same either way.
    fn graph(&self) -> Vec<(ArtifactKind, Vec<ArtifactKind>)> {
        vec![
            (ArtifactKind::Proposal, vec![]),
            (ArtifactKind::Specs, vec![ArtifactKind::Proposal]),
            (
                ArtifactKind::Design,
                vec![ArtifactKind::Proposal, ArtifactKind::Specs],
            ),
            (
                ArtifactKind::Tasks,
                vec![ArtifactKind::Specs, ArtifactKind::Design],
            ),
        ]
    }

    fn changes_root(&self) -> &'static str {
        "openspec/changes"
    }

    fn suggest_change_id(&self, title: &str) -> String {
        let id = slug(title);
        if id.is_empty() {
            "change".to_owned()
        } else {
            id
        }
    }

    fn check_change_id(&self, id: &str) -> Result<(), ChangeIdProblem> {
        if id.is_empty() {
            return Err(ChangeIdProblem::Empty);
        }
        if id.len() > MAX_CHANGE_ID {
            return Err(ChangeIdProblem::TooLong);
        }
        let kebab = id.split('-').all(|part| {
            !part.is_empty() && part.chars().all(|c| matches!(c, 'a'..='z' | '0'..='9'))
        });
        if kebab {
            Ok(())
        } else {
            Err(ChangeIdProblem::Format)
        }
    }

    fn instructions(&self, kind: ArtifactKind) -> ArtifactInstructions {
        let (instruction, template) = match kind {
            ArtifactKind::Proposal => (text::PROPOSAL_INSTRUCTION, text::PROPOSAL_TEMPLATE),
            ArtifactKind::Specs => (text::SPECS_INSTRUCTION, text::SPECS_TEMPLATE),
            ArtifactKind::Design => (text::DESIGN_INSTRUCTION, text::DESIGN_TEMPLATE),
            ArtifactKind::Tasks => (text::TASKS_INSTRUCTION, text::TASKS_TEMPLATE),
        };
        ArtifactInstructions {
            instruction: instruction.to_owned(),
            template: template.to_owned(),
        }
    }

    fn output_protocol(&self, kind: ArtifactKind) -> String {
        document_protocol(kind)
    }

    fn review_instruction(&self) -> String {
        format!("{}\n\n{}", text::REVIEW_INSTRUCTION, text::REVIEW_FORMAT)
    }

    fn edit_protocol(&self, kind: ArtifactKind) -> Option<String> {
        Some(edit_protocol(kind))
    }

    fn parse_revision(
        &self,
        kind: ArtifactKind,
        output: &str,
        current: &[SpecFile],
    ) -> Result<Vec<SpecFile>, OutputProblem> {
        let (edits, problem) = edits::read_edits(output);
        if edits.is_empty() && problem.is_none() {
            return self.parse_output(kind, output);
        }
        if let Some(problem) = problem {
            return Err(OutputProblem::Edit(problem));
        }
        let mut files = edits::apply(current, &edits).map_err(OutputProblem::Edit)?;
        // Whole files in the same answer replace the ones they name (a new capability, say).
        let (blocks, _) = read_blocks(output);
        for (path, content) in blocks {
            if !is_safe_relative(&path) {
                return Err(OutputProblem::UnsafePath(path));
            }
            if !path_belongs(kind, &path) {
                return Err(OutputProblem::UnexpectedPath(path));
            }
            if content.trim().is_empty() {
                return Err(OutputProblem::EmptyDocument(path));
            }
            match files.iter_mut().find(|f| f.path == path) {
                Some(file) => file.content = content,
                None => files.push(SpecFile { path, content }),
            }
        }
        if files.len() > MAX_FILES {
            return Err(OutputProblem::TooManyFiles);
        }
        if let Some(big) = files.iter().find(|f| f.content.len() > MAX_FILE) {
            return Err(OutputProblem::TooLarge(big.path.clone()));
        }
        if files.iter().any(|f| f.content.trim().is_empty()) {
            let empty = files.iter().find(|f| f.content.trim().is_empty());
            return Err(OutputProblem::EmptyDocument(
                empty.map(|f| f.path.clone()).unwrap_or_default(),
            ));
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }

    fn parse_output(
        &self,
        kind: ArtifactKind,
        output: &str,
    ) -> Result<Vec<SpecFile>, OutputProblem> {
        let (blocks, unterminated) = read_blocks(output);
        if blocks.is_empty() {
            return Err(unterminated.map_or(OutputProblem::NoDocument, OutputProblem::Unterminated));
        }
        if blocks.len() > MAX_FILES {
            return Err(OutputProblem::TooManyFiles);
        }
        let mut files: Vec<SpecFile> = Vec::new();
        for (path, content) in blocks {
            if !is_safe_relative(&path) {
                return Err(OutputProblem::UnsafePath(path));
            }
            if !path_belongs(kind, &path) {
                return Err(OutputProblem::UnexpectedPath(path));
            }
            if files.iter().any(|f| f.path == path) {
                return Err(OutputProblem::DuplicatePath(path));
            }
            if content.trim().is_empty() {
                return Err(OutputProblem::EmptyDocument(path));
            }
            if content.len() > MAX_FILE {
                return Err(OutputProblem::TooLarge(path));
            }
            files.push(SpecFile { path, content });
        }
        if kind != ArtifactKind::Specs {
            let expected = match kind {
                ArtifactKind::Proposal => "proposal.md",
                ArtifactKind::Design => "design.md",
                _ => "tasks.md",
            };
            if !files.iter().any(|f| f.path == expected) {
                return Err(OutputProblem::MissingDocument(expected.to_owned()));
            }
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }

    fn validate_artifact(
        &self,
        kind: ArtifactKind,
        files: &[SpecFile],
        project: &ProjectSpecs,
    ) -> Vec<ValidationIssue> {
        validate::validate_artifact(kind, files, project)
    }

    fn validate_consistency(&self, documents: &Documents) -> Vec<ValidationIssue> {
        validate::validate_consistency(documents)
    }

    fn publish_files(
        &self,
        documents: &Documents,
        created_at_ms: u64,
    ) -> Result<Vec<PublishFile>, ArtifactKind> {
        let mut out = vec![PublishFile {
            path: ".openspec.yaml".to_owned(),
            content: format!("schema: {SCHEMA}\ncreated: {}\n", iso_date(created_at_ms)),
        }];
        for kind in [
            ArtifactKind::Proposal,
            ArtifactKind::Specs,
            ArtifactKind::Design,
            ArtifactKind::Tasks,
        ] {
            let files = documents
                .get(kind.id())
                .filter(|f| !f.is_empty())
                .ok_or(kind)?;
            for file in files {
                let content = if file.content.ends_with('\n') {
                    file.content.clone()
                } else {
                    format!("{}\n", file.content)
                };
                out.push(PublishFile {
                    path: file.path.clone(),
                    content,
                });
            }
        }
        Ok(out)
    }

    fn import(&self, folder: &ChangeFolder) -> ImportedChange {
        let mut artifacts: BTreeMap<ArtifactKind, Vec<SpecFile>> = BTreeMap::new();
        let mut problems = Vec::new();
        for file in &folder.files {
            if file.path == ".openspec.yaml" {
                if let Some(schema) = yaml_schema(&file.content) {
                    if schema != SCHEMA {
                        problems.push(ImportProblem::SchemaUnsupported(schema));
                    }
                }
                continue;
            }
            if file.content.len() > MAX_FILE {
                problems.push(ImportProblem::TooLarge(file.path.clone()));
                continue;
            }
            let kind = match file.path.as_str() {
                "proposal.md" => Some(ArtifactKind::Proposal),
                "design.md" => Some(ArtifactKind::Design),
                "tasks.md" => Some(ArtifactKind::Tasks),
                other if validate::capability_of(other).is_some() => Some(ArtifactKind::Specs),
                _ => None,
            };
            match kind {
                Some(kind) => artifacts.entry(kind).or_default().push(file.clone()),
                None => problems.push(ImportProblem::Unrecognized(file.path.clone())),
            }
        }
        for files in artifacts.values_mut() {
            files.sort_by(|a, b| a.path.cmp(&b.path));
        }
        for kind in ArtifactKind::ALL {
            if !artifacts.contains_key(&kind) {
                problems.push(ImportProblem::Missing(kind));
            }
        }
        ImportedChange {
            artifacts,
            problems,
        }
    }

    fn outline(&self, files: &[SpecFile]) -> Vec<FileOutline> {
        files
            .iter()
            .filter(|f| validate::capability_of(&f.path).is_some())
            .map(|file| {
                let doc = markdown::parse_spec_file(&validate::without_comments(&file.content));
                let requirements = doc
                    .deltas
                    .iter()
                    .filter(|d| d.line > 0)
                    .flat_map(|delta| {
                        delta.requirements.iter().map(move |r| RequirementOutline {
                            operation: delta.op.name().to_owned(),
                            name: r.name.trim().to_owned(),
                            line: r.line,
                            text: r.text.trim().to_owned(),
                            scenarios: r
                                .scenarios
                                .iter()
                                .map(|s| ScenarioOutline {
                                    name: s.name.trim().to_owned(),
                                    line: s.line,
                                    steps: markdown::scenario_step_list(&s.body)
                                        .into_iter()
                                        .map(|(keyword, text)| StepOutline { keyword, text })
                                        .collect(),
                                })
                                .collect(),
                        })
                    })
                    .collect();
                FileOutline {
                    path: file.path.clone(),
                    purpose: doc.purpose,
                    requirements,
                }
            })
            .collect()
    }

    fn tasks(&self, files: &[SpecFile]) -> Vec<TaskItem> {
        files
            .iter()
            .find(|f| f.path == "tasks.md")
            .map(|f| {
                markdown::parse_tasks(&validate::without_comments(&f.content))
                    .into_iter()
                    .map(|t| TaskItem {
                        id: t.id,
                        text: t.text,
                        checked: t.done,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn digest(&self, documents: &Documents, max_chars: usize) -> String {
        let content = |id: &str, path: &str| {
            documents
                .get(id)
                .and_then(|files| files.iter().find(|f| f.path == path))
                .map(|f| f.content.as_str())
                .unwrap_or_default()
        };
        // The parts, each with what it needs, so that a spec too long for the budget loses a bit
        // of every part and says what it lost, instead of losing its tail without a word.
        let mut head = String::new();
        let proposal = content("proposal", "proposal.md");
        let why = section_text(proposal, 2, "Why");
        if !why.is_empty() {
            let _ = writeln!(head, "Why: {}", clip(&why.replace('\n', " "), 1_200));
        }
        let what = section_text(proposal, 2, "What Changes");
        if !what.is_empty() {
            let _ = write!(head, "What changes:\n{}\n", clip(&what, 2_000));
        }
        let mut requirements = String::new();
        for file in documents.get("specs").into_iter().flatten() {
            let Some(capability) = validate::capability_of(&file.path) else {
                continue;
            };
            let _ = writeln!(requirements, "{capability} ({}):", file.path);
            let parsed = markdown::parse_spec_file(&validate::without_comments(&file.content));
            for delta in &parsed.deltas {
                for requirement in &delta.requirements {
                    let scenarios: Vec<&str> = requirement
                        .scenarios
                        .iter()
                        .map(|s| s.name.trim())
                        .collect();
                    let _ = write!(
                        requirements,
                        "- {} {}: {}",
                        delta.op.name(),
                        requirement.name.trim(),
                        clip(&requirement.text.replace('\n', " "), 400)
                    );
                    if !scenarios.is_empty() {
                        let _ = write!(
                            requirements,
                            " [scenarios: {}]",
                            clip(&scenarios.join("; "), 300)
                        );
                    }
                    requirements.push('\n');
                }
            }
        }
        let design_doc = content("design", "design.md");
        let mut design = String::new();
        for title in ["Decisions", "Risks / Trade-offs", "Migration Plan"] {
            let text = section_text(design_doc, 2, title);
            if !text.is_empty() {
                let _ = write!(design, "## {title}\n{text}\n\n");
            }
        }
        let mut tasks = String::new();
        for task in
            &markdown::parse_tasks(&validate::without_comments(content("tasks", "tasks.md")))
        {
            let _ = writeln!(tasks, "- {} {}", task.id, clip(&task.text, 1_200));
        }
        let parts = [
            ("", head, 10),
            (
                "Requirements by capability (what the system SHALL do)",
                requirements,
                25,
            ),
            ("Design decisions and risks", design, 25),
            (
                "Tasks (follow this list, in order, and verify each as it says)",
                tasks,
                40,
            ),
        ];
        fit_parts(&parts, max_chars)
    }
}

/// The parts laid out within `max` characters. Each gets what it needs when everything fits;
/// otherwise each gets its share of the budget (by weight), a part that needs less than its share
/// hands the rest to those that were cut, and a cut part says how much of it is not shown.
fn fit_parts(parts: &[(&str, String, usize)], max: usize) -> String {
    let needs: Vec<usize> = parts
        .iter()
        .map(|(_, text, _)| text.chars().count())
        .collect();
    let titles: usize = parts
        .iter()
        .filter(|(title, text, _)| !title.is_empty() && !text.is_empty())
        .map(|(title, _, _)| title.chars().count() + 4)
        .sum();
    let room = max.saturating_sub(titles);
    let weight_total: usize = parts.iter().map(|(_, _, w)| *w).sum();
    let mut given: Vec<usize> = parts
        .iter()
        .zip(&needs)
        .map(|((_, _, w), need)| (*need).min(room * w / weight_total.max(1)))
        .collect();
    // What the parts that need less than their share leave goes to the ones still cut.
    let mut spare = room.saturating_sub(given.iter().sum());
    while spare > 0 {
        let hungry: Vec<usize> = (0..parts.len()).filter(|i| given[*i] < needs[*i]).collect();
        if hungry.is_empty() {
            break;
        }
        let each = (spare / hungry.len()).max(1);
        for i in hungry {
            let add = each.min(needs[i] - given[i]).min(spare);
            given[i] += add;
            spare -= add;
        }
    }
    let mut out = String::new();
    for (((title, text, _), need), give) in parts.iter().zip(&needs).zip(&given) {
        if text.is_empty() {
            continue;
        }
        if !title.is_empty() {
            let _ = writeln!(out, "{title}:");
        }
        if give >= need {
            out.push_str(text.trim_end());
        } else {
            out.push_str(&clip_block(text, *give));
            let _ = write!(
                out,
                "\n[cut: {} of {} characters of this part are not shown here]",
                need - give,
                need
            );
        }
        out.push_str("\n\n");
    }
    out.trim_end().to_owned()
}

/// Cut at a line end so a block never ends mid-word.
fn clip_block(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.trim_end().to_owned();
    }
    let cut: String = text.chars().take(max).collect();
    let end = cut.rfind('\n').unwrap_or(cut.len());
    format!("{}\n…", cut[..end].trim_end())
}

fn yaml_schema(content: &str) -> Option<String> {
    let value: serde_norway::Value = serde_norway::from_str(content).ok()?;
    value.get("schema")?.as_str().map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::validate::tests_support::{DESIGN, PROPOSAL, SPEC, TASKS};
    use super::*;

    fn wrap(path: &str, content: &str) -> String {
        format!("{DOC_OPEN}{path}{DOC_OPEN_END}\n{content}\n{DOC_CLOSE}\n")
    }

    fn adapter() -> OpenSpecAdapter {
        OpenSpecAdapter
    }

    #[test]
    fn a_document_in_the_agreed_shape_is_read_with_its_code_fences_inside() {
        let doc = "# Proposal\n\n```rust\nfn main() {}\n```\n";
        let answer = format!("A note first.\n{}Thanks!", wrap("proposal.md", doc));

        let files = adapter()
            .parse_output(ArtifactKind::Proposal, &answer)
            .unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "proposal.md");
        assert!(files[0].content.contains("fn main()"));
        assert!(!files[0].content.contains("Thanks"));
    }

    #[test]
    fn several_spec_files_come_back_in_order() {
        let answer = format!(
            "{}{}",
            wrap("specs/b-cap/spec.md", SPEC),
            wrap("specs/a-cap/spec.md", SPEC)
        );

        let files = adapter()
            .parse_output(ArtifactKind::Specs, &answer)
            .unwrap();

        assert_eq!(
            files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(),
            vec!["specs/a-cap/spec.md", "specs/b-cap/spec.md"]
        );
    }

    /// Measured with a real model (`opencode/big-pickle`, a tasks document): the opening marker
    /// came back with two `>`, and the whole answer was refused as having no document.
    #[test]
    fn a_marker_that_dropped_an_arrow_still_opens_and_closes_a_document() {
        let answer = format!(
            "The design states no open questions.\n\n<<<ATLAS-DOCUMENT path=\"tasks.md\">>\n{TASKS}\n<<<END-ATLAS-DOCUMENT>>\n"
        );

        let files = adapter()
            .parse_output(ArtifactKind::Tasks, &answer)
            .unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "tasks.md");
        assert_eq!(files[0].content, TASKS.trim());
    }

    #[test]
    fn a_marker_with_no_arrow_or_text_after_it_is_not_a_marker() {
        for opener in [
            "<<<ATLAS-DOCUMENT path=\"tasks.md\"",
            "<<<ATLAS-DOCUMENT path=\"tasks.md\">>>> extra",
            "<<<ATLAS-DOCUMENT path=\"tasks.md\">>>>",
        ] {
            let answer = format!("{opener}\n{TASKS}\n{DOC_CLOSE}\n");
            assert_eq!(
                adapter().parse_output(ArtifactKind::Tasks, &answer),
                Err(OutputProblem::NoDocument),
                "{opener}"
            );
        }
    }

    #[test]
    fn an_answer_without_documents_is_refused() {
        let result = adapter().parse_output(ArtifactKind::Proposal, "Here is my proposal: ...");

        assert_eq!(result, Err(OutputProblem::NoDocument));
    }

    #[test]
    fn a_document_that_was_never_closed_is_refused_as_cut_off() {
        let answer = format!("{DOC_OPEN}proposal.md{DOC_OPEN_END}\n# Proposal\nhalf");

        assert_eq!(
            adapter().parse_output(ArtifactKind::Proposal, &answer),
            Err(OutputProblem::Unterminated("proposal.md".to_owned()))
        );
    }

    #[test]
    fn a_file_the_artifact_cannot_have_is_refused() {
        let answer = wrap("design.md", "# Design");

        assert_eq!(
            adapter().parse_output(ArtifactKind::Proposal, &answer),
            Err(OutputProblem::UnexpectedPath("design.md".to_owned()))
        );
    }

    #[test]
    fn a_path_that_leaves_the_change_is_refused() {
        for path in [
            "../proposal.md",
            "/etc/passwd",
            "specs/../../x/spec.md",
            "a\\b.md",
        ] {
            let result = adapter().parse_output(ArtifactKind::Specs, &wrap(path, SPEC));
            assert_eq!(
                result,
                Err(OutputProblem::UnsafePath(path.to_owned())),
                "{path}"
            );
        }
    }

    #[test]
    fn the_same_file_twice_and_an_empty_file_are_refused() {
        let twice = format!(
            "{}{}",
            wrap("specs/a/spec.md", SPEC),
            wrap("specs/a/spec.md", SPEC)
        );
        assert_eq!(
            adapter().parse_output(ArtifactKind::Specs, &twice),
            Err(OutputProblem::DuplicatePath("specs/a/spec.md".to_owned()))
        );
        assert_eq!(
            adapter().parse_output(ArtifactKind::Specs, &wrap("specs/a/spec.md", "  ")),
            Err(OutputProblem::EmptyDocument("specs/a/spec.md".to_owned()))
        );
    }

    #[test]
    fn a_document_an_agent_wrapped_in_a_fence_anyway_is_unwrapped() {
        let answer = wrap("design.md", "```markdown\n# Design\n\n## Context\nx\n```");

        let files = adapter()
            .parse_output(ArtifactKind::Design, &answer)
            .unwrap();

        assert_eq!(files[0].content, "# Design\n\n## Context\nx");
    }

    #[test]
    fn a_title_becomes_a_valid_change_id() {
        let a = adapter();
        assert_eq!(a.suggest_change_id("Add dark mode!"), "add-dark-mode");
        assert_eq!(
            a.suggest_change_id("Autenticação de usuários"),
            "autenticacao-de-usuarios"
        );
        assert_eq!(a.suggest_change_id("  ---  "), "change");
        let long = a.suggest_change_id(&"palavra ".repeat(20));
        assert!(
            long.len() <= 48 && a.check_change_id(&long).is_ok(),
            "{long}"
        );
        for title in ["Add dark mode", "Ñandú & CIA", "123 go"] {
            assert!(
                a.check_change_id(&a.suggest_change_id(title)).is_ok(),
                "{title}"
            );
        }
    }

    #[test]
    fn only_kebab_case_names_a_change() {
        let a = adapter();
        assert_eq!(a.check_change_id(""), Err(ChangeIdProblem::Empty));
        assert_eq!(a.check_change_id("Add-Dark"), Err(ChangeIdProblem::Format));
        assert_eq!(a.check_change_id("add--dark"), Err(ChangeIdProblem::Format));
        assert_eq!(a.check_change_id("../x"), Err(ChangeIdProblem::Format));
        assert_eq!(
            a.check_change_id(&"a".repeat(65)),
            Err(ChangeIdProblem::TooLong)
        );
        assert!(a.check_change_id("add-dark-mode-2").is_ok());
    }

    #[test]
    fn dates_are_written_as_openspec_writes_them() {
        assert_eq!(iso_date(0), "1970-01-01");
        assert_eq!(iso_date(951_782_400_000), "2000-02-29");
        assert_eq!(iso_date(1_791_849_600_000), "2026-10-13");
    }

    fn documents() -> Documents {
        [
            (
                "proposal".to_owned(),
                vec![SpecFile {
                    path: "proposal.md".to_owned(),
                    content: PROPOSAL.to_owned(),
                }],
            ),
            (
                "specs".to_owned(),
                vec![SpecFile {
                    path: "specs/dark-mode/spec.md".to_owned(),
                    content: SPEC.to_owned(),
                }],
            ),
            (
                "design".to_owned(),
                vec![SpecFile {
                    path: "design.md".to_owned(),
                    content: DESIGN.to_owned(),
                }],
            ),
            (
                "tasks".to_owned(),
                vec![SpecFile {
                    path: "tasks.md".to_owned(),
                    content: TASKS.to_owned(),
                }],
            ),
        ]
        .into()
    }

    #[test]
    fn publishing_lists_the_metadata_and_every_document() {
        let files = adapter()
            .publish_files(&documents(), 1_791_849_600_000)
            .unwrap();

        let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths,
            vec![
                ".openspec.yaml",
                "proposal.md",
                "specs/dark-mode/spec.md",
                "design.md",
                "tasks.md"
            ]
        );
        assert_eq!(
            files[0].content,
            "schema: spec-driven\ncreated: 2026-10-13\n"
        );
        assert!(files.iter().all(|f| f.content.ends_with('\n')));
    }

    #[test]
    fn publishing_names_the_document_that_is_missing() {
        let mut docs = documents();
        docs.remove("design");

        assert_eq!(adapter().publish_files(&docs, 0), Err(ArtifactKind::Design));
    }

    #[test]
    fn a_change_folder_is_read_back_into_artifacts_and_says_what_it_lacks() {
        let folder = ChangeFolder {
            files: vec![
                SpecFile {
                    path: ".openspec.yaml".to_owned(),
                    content: "schema: spec-driven\ncreated: 2026-01-01\n".to_owned(),
                },
                SpecFile {
                    path: "proposal.md".to_owned(),
                    content: PROPOSAL.to_owned(),
                },
                SpecFile {
                    path: "specs/dark-mode/spec.md".to_owned(),
                    content: SPEC.to_owned(),
                },
                SpecFile {
                    path: "notes.txt".to_owned(),
                    content: "x".to_owned(),
                },
            ],
        };

        let imported = adapter().import(&folder);

        assert_eq!(imported.artifacts.len(), 2);
        assert!(imported
            .problems
            .contains(&ImportProblem::Missing(ArtifactKind::Design)));
        assert!(imported
            .problems
            .contains(&ImportProblem::Missing(ArtifactKind::Tasks)));
        assert!(imported
            .problems
            .contains(&ImportProblem::Unrecognized("notes.txt".to_owned())));
    }

    #[test]
    fn a_custom_schema_is_reported_not_read_as_if_it_were_ours() {
        let folder = ChangeFolder {
            files: vec![SpecFile {
                path: ".openspec.yaml".to_owned(),
                content: "schema: tdd-driven\n".to_owned(),
            }],
        };

        let imported = adapter().import(&folder);

        assert!(imported
            .problems
            .contains(&ImportProblem::SchemaUnsupported("tdd-driven".to_owned())));
    }

    #[test]
    fn a_spec_file_is_laid_out_for_reading_with_the_line_of_each_requirement() {
        let outline = adapter().outline(&[SpecFile {
            path: "specs/dark-mode/spec.md".to_owned(),
            content: SPEC.to_owned(),
        }]);

        assert_eq!(outline.len(), 1);
        assert!(outline[0]
            .purpose
            .as_deref()
            .unwrap()
            .starts_with("Lets people"));
        let requirement = &outline[0].requirements[0];
        assert_eq!(requirement.operation, "ADDED");
        assert_eq!(requirement.name, "User can switch the theme");
        assert!(requirement.line > 0);
        assert_eq!(requirement.scenarios[0].name, "Switch to dark");
        assert_eq!(requirement.scenarios[0].steps[0].keyword, "WHEN");
        assert_eq!(requirement.scenarios[0].steps[1].keyword, "THEN");
    }

    #[test]
    fn tasks_are_listed_with_what_the_document_says_about_them() {
        let tasks = adapter().tasks(&[SpecFile {
            path: "tasks.md".to_owned(),
            content: "## 1. A\n- [x] 1.1 Done\n- [ ] 1.2 Todo\n".to_owned(),
        }]);

        assert_eq!(tasks.len(), 2);
        assert!(tasks[0].checked && !tasks[1].checked);
        assert_eq!(tasks[1].id, "1.2");
    }

    #[test]
    fn the_digest_states_the_documents_within_its_budget() {
        let digest = adapter().digest(&documents(), 6_000);

        assert!(digest.contains("Why: Users working at night"));
        assert!(digest.contains("dark-mode"));
        assert!(digest.contains("ADDED User can switch the theme"));
        assert!(digest.contains("1.1"));
        assert!(!digest.contains("[cut:"), "it all fits, so nothing is cut");
    }

    fn long_tasks(count: usize) -> Documents {
        let mut docs = documents();
        let mut tasks = String::from("## 1. Work\n");
        for n in 1..=count {
            let _ = writeln!(
                tasks,
                "- [ ] 1.{n} Task number {n} with enough words in it to take some room. Verificação: a test passes."
            );
        }
        docs.insert(
            "tasks".to_owned(),
            vec![SpecFile {
                path: "tasks.md".to_owned(),
                content: tasks,
            }],
        );
        docs
    }

    #[test]
    fn a_long_task_list_is_given_whole_when_the_budget_allows_it() {
        let digest = adapter().digest(&long_tasks(120), 24_000);

        assert!(digest.contains("1.120 Task number 120"));
        assert!(digest.contains("Verificação: a test passes."));
        assert!(!digest.contains("[cut:"));
    }

    #[test]
    fn a_spec_too_long_for_the_budget_loses_a_bit_of_every_part_and_says_so() {
        let digest = adapter().digest(&long_tasks(120), 3_000);

        // Every part is still there, the tasks are cut, and the cut is stated.
        assert!(digest.contains("Why: Users working at night"));
        assert!(digest.contains("ADDED User can switch the theme"));
        assert!(digest.contains("Design decisions and risks"));
        assert!(digest.contains("1.1 Task number 1 "));
        assert!(digest.contains("[cut:"));
        assert!(digest.chars().count() < 3_300, "{}", digest.chars().count());
    }

    #[test]
    fn the_graph_serializes_design_after_the_specs() {
        let graph = adapter().graph();
        let design = graph
            .iter()
            .find(|(k, _)| *k == ArtifactKind::Design)
            .unwrap();

        assert!(design.1.contains(&ArtifactKind::Specs));
    }
}
