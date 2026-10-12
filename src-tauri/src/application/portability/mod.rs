//! Use case: take configurations out of Atlas as a file, and bring such a file in again, here or
//! on another installation. See ADR 0032.
//!
//! The parts, each with one job:
//! - `format`: the file's envelope, its version and the checks that need no knowledge of Atlas.
//! - `spec`: what each kind looks like in a file, and the mapping to the domain's types.
//! - `kinds`: one [`Category`] per kind (personality, agent, workflow, saved template). A new
//!   kind is a new category and a new variant of [`Kind`]; the flow below does not change.
//! - `export`, `inspect`, `apply`: the three steps. Importing is never "deserialize and save":
//!   the file is read (`parse`), looked at against what is here (`inspect`, which changes
//!   nothing) and, once the person has decided, written in one atomic change (`apply`).
//!
//! The file is untrusted input. It is size-limited and checked before anything is looked at, it
//! carries no code Atlas runs, its personalities, agents and steps are held to the same rules as
//! the ones typed in the editor, and importing grants nothing: no tool, no permission wider than
//! what the person agrees to in the preview.

mod apply;
mod export;
pub mod format;
mod inspect;
mod kinds;
pub mod spec;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use apply::{ImportReport, ImportRequest};
pub use export::{ExportTarget, ExportedFile};
pub use inspect::ImportPreview;

use self::format::{FormatError, Item, Kind, Package};
use super::config::{ConfigRepository, UserConfig};
use super::errors::AppError;
use super::runtimes::RuntimeRegistry;
use super::workspace::WorkspaceService;

/// What the person decides for each item of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// Add it as a new configuration, with a new local id (and a new name if the name is taken).
    Create,
    /// Use the one that is already here; the file's is not imported.
    UseExisting,
    /// Overwrite the one that is already here. Only ever on the person's explicit choice.
    Replace,
    /// Leave it out.
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// Something about an item the person should know before, or after, importing it. A code the UI
/// words, and the values for its message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Issue {
    pub code: &'static str,
    pub severity: Severity,
    pub params: BTreeMap<String, String>,
}

impl Issue {
    pub fn new(code: &'static str, severity: Severity) -> Self {
        Self {
            code,
            severity,
            params: BTreeMap::new(),
        }
    }

    pub fn error(code: &'static str) -> Self {
        Self::new(code, Severity::Error)
    }

    pub fn warning(code: &'static str) -> Self {
        Self::new(code, Severity::Warning)
    }

    #[must_use]
    pub fn with(mut self, key: &str, value: impl Into<String>) -> Self {
        self.params.insert(key.to_owned(), value.into());
        self
    }
}

/// A configuration of the installation that a file's item matches, by name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Existing {
    pub id: String,
    pub name: String,
}

/// Where an item is going to, as far as `apply` is concerned: a local scope.
pub(crate) struct Scope<'a> {
    pub config: &'a UserConfig,
    /// The workspace workflows are imported into.
    pub workspace_id: Option<&'a str>,
}

/// A configuration an item names that its file does not carry, or carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Dependency {
    pub key: String,
    pub kind: Kind,
}

/// What a category tells about a local configuration, to declare it in a file without carrying it.
pub(crate) struct Described {
    pub name: String,
    pub description: String,
    pub builtin_id: Option<String>,
}

/// One kind of configuration, end to end. The only place that knows what a personality, an
/// agent, a workflow or a template is made of.
pub(crate) trait Category: Sync {
    /// Reads the item's spec without looking at anything local; names what it depends on.
    fn check(&self, item: &Item, at: &str) -> Result<Vec<Dependency>, FormatError>;

    /// The configurations of this kind the installation has (where the import is going).
    fn list(&self, scope: &Scope<'_>) -> Vec<Existing>;

    /// The configuration of this kind that `item` is the same as by name, if there is one.
    fn existing(&self, scope: &Scope<'_>, item: &Item) -> Option<Existing> {
        by_name(self.list(scope), &item.name)
    }

    fn describe(&self, config: &UserConfig, id: &str) -> Option<Described>;

    /// Builds the file's item for a local configuration; the exporter gives it its `ref`.
    fn export(&self, cx: &mut export::Exporter<'_>, id: &str) -> Result<Item, AppError>;

    /// What importing the item would do, without doing it.
    fn inspect(&self, cx: &inspect::Inspector<'_>, item: &Item) -> inspect::Inspection;

    /// Writes the item into `config`, as a new configuration or over `replacing`.
    fn write(
        &self,
        cx: &mut apply::Applier<'_>,
        config: &mut UserConfig,
        item: &Item,
        replacing: Option<&Existing>,
    ) -> Result<apply::Written, AppError>;
}

/// The one whose name is `name`, ignoring case and surrounding space.
pub(super) fn by_name(list: Vec<Existing>, name: &str) -> Option<Existing> {
    let wanted = name.trim().to_lowercase();
    list.into_iter()
        .find(|existing| existing.name.trim().to_lowercase() == wanted)
}

/// Every kind there is. Adding one is adding it here.
pub(crate) fn category(kind: Kind) -> &'static dyn Category {
    match kind {
        Kind::Personality => &kinds::personality::PersonalityKind,
        Kind::Agent => &kinds::agent::AgentKind,
        Kind::Workflow => &kinds::workflow::WorkflowKind,
        Kind::WorkflowTemplate => &kinds::template::TemplateKind,
    }
}

pub(super) fn decode<T: serde::de::DeserializeOwned>(
    item: &Item,
    at: &str,
) -> Result<T, FormatError> {
    serde_json::from_value(item.spec.clone()).map_err(|error| spec::spec_error(&error, at))
}

/// Whether any text of the item looks like a credential. Atlas never writes one into a file on
/// purpose; this catches the one a person typed into an instruction.
pub(super) fn holds_a_secret(item: &Item) -> bool {
    fn any_secret(value: &Value) -> bool {
        match value {
            Value::String(text) => super::harness::secrets::redact_secrets(text).1 > 0,
            Value::Array(values) => values.iter().any(any_secret),
            Value::Object(map) => map.values().any(any_secret),
            _ => false,
        }
    }
    any_secret(&item.spec) || super::harness::secrets::redact_secrets(&item.description).1 > 0
}

/// Reads a file completely: envelope, then every item's spec and the references between items.
///
/// # Errors
///
/// Fails with the first thing wrong, in terms the UI can word.
pub fn load(text: &str) -> Result<Package, AppError> {
    let package = format::parse(text)?;
    for (index, item) in package.items.iter().enumerate() {
        let at = format!("items[{index}]");
        for dependency in category(item.kind).check(item, &at)? {
            if package.kind_of(&dependency.key) != Some(dependency.kind) {
                return Err(FormatError::invalid(
                    "unknown_ref",
                    format!("{at}.{}", dependency.key),
                )
                .into());
            }
        }
    }
    Ok(package)
}

/// Export, preview and import of configurations.
pub struct PortabilityService {
    config: Arc<ConfigRepository>,
    runtimes: Arc<RuntimeRegistry>,
    workspaces: Arc<WorkspaceService>,
}

impl PortabilityService {
    pub fn new(
        config: Arc<ConfigRepository>,
        runtimes: Arc<RuntimeRegistry>,
        workspaces: Arc<WorkspaceService>,
    ) -> Self {
        Self {
            config,
            runtimes,
            workspaces,
        }
    }

    /// Writes the chosen configurations as a file. With `include_dependencies`, whatever they
    /// need (an agent's personality, a workflow's agents) travels in the same file; without,
    /// the file only names what is needed and the importer picks its own.
    ///
    /// # Errors
    ///
    /// Fails if a chosen configuration does not exist.
    pub fn export(
        &self,
        targets: &[ExportTarget],
        include_dependencies: bool,
    ) -> Result<ExportedFile, AppError> {
        export::export(
            &self.config.snapshot(),
            &self.runtimes,
            targets,
            include_dependencies,
        )
    }

    /// What importing the file would do here: every item, what it matches, what it needs, what is
    /// wrong with it. Changes nothing. May take a moment when the file has agents: their runtimes
    /// are asked which models they offer.
    ///
    /// # Errors
    ///
    /// Fails if the file cannot be read as an Atlas file.
    pub fn preview(
        &self,
        text: &str,
        workspace_id: Option<&str>,
    ) -> Result<ImportPreview, AppError> {
        let package = load(text)?;
        Ok(inspect::preview(
            &package,
            &self.config.snapshot(),
            &self.runtimes,
            workspace_id.filter(|id| self.workspaces.get(id).is_some()),
        ))
    }

    /// Imports the file as the person decided in the preview. All or nothing: the file is
    /// looked at again against what is here now, every decision is checked, and only then is
    /// everything written in one change; any failure leaves the installation as it was.
    ///
    /// # Errors
    ///
    /// Fails if the file is unreadable, a decision cannot be honoured or saving fails.
    pub fn import(&self, text: &str, request: &ImportRequest) -> Result<ImportReport, AppError> {
        let package = load(text)?;
        let workspace_id = request
            .workspace_id
            .as_deref()
            .filter(|id| self.workspaces.get(id).is_some());
        apply::import(
            &self.config,
            &self.runtimes,
            &package,
            request,
            workspace_id,
        )
    }
}
