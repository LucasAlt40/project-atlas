//! The contract between Atlas and a specification format. Everything the rest of Atlas knows
//! about `OpenSpec` is behind this trait, so the services, the screens and the stored records never
//! ask "which framework is this?": they ask the framework.
//!
//! A framework does not run agents, read the project or touch the disk. It turns documents into
//! checks, instructions and file lists; Atlas does the rest.

use std::collections::BTreeMap;

use crate::domain::spec::{ArtifactKind, FrameworkKind, SpecFile, ValidationIssue};

/// Why a name cannot be a change id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeIdProblem {
    Empty,
    /// Not kebab-case (lowercase letters, digits and single hyphens).
    Format,
    TooLong,
}

/// What an agent's answer could not be turned into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputProblem {
    /// No document in the agreed shape.
    NoDocument,
    /// A document was opened and never closed (`path`): the answer was cut off.
    Unterminated(String),
    /// A document names a file this artifact cannot have (`path`).
    UnexpectedPath(String),
    /// A file name that is not a safe relative path inside the change (`path`).
    UnsafePath(String),
    /// The same file twice.
    DuplicatePath(String),
    TooManyFiles,
    TooLarge(String),
    /// A file with nothing in it.
    EmptyDocument(String),
    /// A document the artifact needs and the answer left out (`path`).
    MissingDocument(String),
    /// The edits of a revision could not be applied (see [`super::edits::EditProblem`]).
    Edit(super::edits::EditProblem),
}

/// What the checks of one artifact know of the project: the specs already in `openspec/specs/`,
/// by capability. Empty when the project has none (or Atlas could not read them).
#[derive(Debug, Clone, Default)]
pub struct ProjectSpecs {
    pub capabilities: BTreeMap<String, String>,
}

/// The documents of a spec at some revision, by artifact id.
pub type Documents = BTreeMap<String, Vec<SpecFile>>;

/// The parts of one artifact's prompt the framework owns.
pub struct ArtifactInstructions {
    /// What the document is for and the rules of its format.
    pub instruction: String,
    /// The skeleton the document follows.
    pub template: String,
}

/// A file of a change that Atlas will write to the project, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishFile {
    /// Relative to the change's folder.
    pub path: String,
    pub content: String,
}

/// What a change folder of the project holds, as read from disk by Atlas.
#[derive(Debug, Clone, Default)]
pub struct ChangeFolder {
    pub files: Vec<SpecFile>,
}

/// A change read back from the project, mapped onto artifacts.
#[derive(Debug, Clone)]
pub struct ImportedChange {
    pub artifacts: BTreeMap<ArtifactKind, Vec<SpecFile>>,
    /// What the folder lacks or has wrong, for the person to read; nothing is invented.
    pub problems: Vec<ImportProblem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportProblem {
    Missing(ArtifactKind),
    /// A file the format does not know (`path`); kept out of the record, left where it is.
    Unrecognized(String),
    /// `.openspec.yaml` says a schema Atlas does not read.
    SchemaUnsupported(String),
    TooLarge(String),
}

/// A task of the task list, as a framework reads it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskItem {
    pub id: String,
    pub text: String,
    /// What the document itself says: a checked box.
    pub checked: bool,
}

/// A spec file read for display: what it says, requirement by requirement, with the line each
/// starts on so the screen can point at it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileOutline {
    pub path: String,
    pub purpose: Option<String>,
    pub requirements: Vec<RequirementOutline>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequirementOutline {
    /// ADDED, MODIFIED, REMOVED.
    pub operation: String,
    pub name: String,
    pub line: u32,
    pub text: String,
    pub scenarios: Vec<ScenarioOutline>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioOutline {
    pub name: String,
    pub line: u32,
    pub steps: Vec<StepOutline>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StepOutline {
    pub keyword: String,
    pub text: String,
}

pub trait SpecFramework: Send + Sync {
    fn kind(&self) -> FrameworkKind;

    /// The format and its version (`spec-driven/1`), stored with every spec.
    fn schema(&self) -> &'static str;

    /// Which rules produced a check (`atlas-openspec/1`). A result from other rules is run again.
    fn validator(&self) -> &'static str;

    /// The artifacts, and for each the ones it is written from. Atlas takes this as the order:
    /// an artifact can be written when the ones it needs are approved.
    fn graph(&self) -> Vec<(ArtifactKind, Vec<ArtifactKind>)>;

    /// The folder (relative to the project) that changes are published into.
    fn changes_root(&self) -> &'static str;

    /// A change id from a title: valid, readable, stable.
    fn suggest_change_id(&self, title: &str) -> String;

    /// # Errors
    ///
    /// Says why `id` cannot name a change.
    fn check_change_id(&self, id: &str) -> Result<(), ChangeIdProblem>;

    fn instructions(&self, kind: ArtifactKind) -> ArtifactInstructions;

    /// How the agent hands a document back, so Atlas reads it with no guessing.
    fn output_protocol(&self, kind: ArtifactKind) -> String;

    /// What the reviewing agent is asked to look for, in this format's terms.
    fn review_instruction(&self) -> String;

    /// How the agent may answer a request for changes with the sections that change instead of
    /// the whole document, when the format can take that. `None`: it cannot, and the whole
    /// document is always asked for.
    fn edit_protocol(&self, _kind: ArtifactKind) -> Option<String> {
        None
    }

    /// The documents of a revision: an answer made of edits is applied to `current`, one made of
    /// whole documents is read as [`Self::parse_output`] does.
    ///
    /// # Errors
    ///
    /// Says what is wrong with the answer, or which edit cannot be applied.
    fn parse_revision(
        &self,
        kind: ArtifactKind,
        output: &str,
        _current: &[SpecFile],
    ) -> Result<Vec<SpecFile>, OutputProblem> {
        self.parse_output(kind, output)
    }

    /// The documents in an agent's answer for this artifact.
    ///
    /// # Errors
    ///
    /// Says what is wrong with the answer.
    fn parse_output(
        &self,
        kind: ArtifactKind,
        output: &str,
    ) -> Result<Vec<SpecFile>, OutputProblem>;

    /// What the format's rules find in the artifact's files. Pure: the same files and the same
    /// project specs always give the same issues.
    fn validate_artifact(
        &self,
        kind: ArtifactKind,
        files: &[SpecFile],
        project: &ProjectSpecs,
    ) -> Vec<ValidationIssue>;

    /// What the rules find across the documents (a requirement no task covers, a task that points
    /// at a requirement that is not there, a marker left in the text).
    fn validate_consistency(&self, documents: &Documents) -> Vec<ValidationIssue>;

    /// The files to write to the project, change-relative. Fails while a document is missing.
    ///
    /// # Errors
    ///
    /// Names the artifact that has no document.
    fn publish_files(
        &self,
        documents: &Documents,
        created_at_ms: u64,
    ) -> Result<Vec<PublishFile>, ArtifactKind>;

    /// A change folder read back from the project.
    fn import(&self, folder: &ChangeFolder) -> ImportedChange;

    /// The spec files laid out for reading.
    fn outline(&self, files: &[SpecFile]) -> Vec<FileOutline>;

    /// The tasks of the task list.
    fn tasks(&self, files: &[SpecFile]) -> Vec<TaskItem>;

    /// What the spec asks, for an implementing agent that cannot read the documents: made of what
    /// they say, never of what an agent claims, within `max_chars`. When it does not fit, every part
    /// loses some and says how much, so that nothing is cut without a word.
    fn digest(&self, documents: &Documents, max_chars: usize) -> String;
}
