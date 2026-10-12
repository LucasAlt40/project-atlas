//! Writing what the person decided. Everything happens inside one `ConfigRepository::modify`,
//! so the installation either has all of it or none of it: a decision that cannot be honoured,
//! a rule an item breaks, a failed save, each leaves things exactly as they were.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::format::{Item, Kind, Package};
use super::{category, Decision, Issue, Scope};
use crate::application::config::{ConfigRepository, UserConfig};
use crate::application::errors::{AppError, ErrorCode};
use crate::application::runtimes::RuntimeRegistry;
use crate::application::support::now_ms;

/// The runtime and model an agent is imported with, when not the ones it was exported with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelChoice {
    pub runtime_id: String,
    pub model_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRequest {
    /// Where workflows go. Needed when the file has any.
    #[serde(default)]
    pub workspace_id: Option<String>,
    /// What to do with each item, by its `ref`. An item that matches something here must have one.
    #[serde(default)]
    pub decisions: BTreeMap<String, Decision>,
    /// Another runtime and model for an agent, by its `ref`. Without one, the original is kept.
    #[serde(default)]
    pub models: BTreeMap<String, ModelChoice>,
    /// For what the file needs and does not carry: which local configuration it is, by the
    /// `ref` the file gave it.
    #[serde(default)]
    pub bindings: BTreeMap<String, String>,
    /// Keep the permission to edit files that agents were exported with. Off, agents are
    /// imported able only to read, whatever the file says: the person turns editing on, or
    /// not, in the preview.
    #[serde(default)]
    pub keep_permission_profiles: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Created,
    Replaced,
    Reused,
    Skipped,
}

/// What became of an agent's model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelOutcome {
    pub original_runtime: String,
    pub original_model: String,
    pub runtime: String,
    pub model: String,
}

/// What a category reports after writing an item.
pub(crate) struct Written {
    pub id: String,
    pub name: String,
    pub issues: Vec<Issue>,
    pub model: Option<ModelOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedItem {
    #[serde(rename = "ref")]
    pub key: String,
    pub kind: Kind,
    /// The name in the file.
    pub name: String,
    /// The id here; none for what was skipped.
    pub id: Option<String>,
    pub result: Action,
    /// The name it got when the file's was taken.
    pub renamed_to: Option<String>,
    pub issues: Vec<Issue>,
    pub model: Option<ModelOutcome>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub items: Vec<ImportedItem>,
}

/// The state of one import while it is being written.
pub(crate) struct Applier<'a> {
    pub package: &'a Package,
    pub request: &'a ImportRequest,
    pub runtimes: &'a RuntimeRegistry,
    pub workspace_id: Option<&'a str>,
    pub now: u64,
    /// The local id of each configuration the file names: what was written or reused, and what
    /// the person bound the external ones to.
    resolved: BTreeMap<String, String>,
}

impl Applier<'_> {
    /// The local id behind a `ref` an item depends on.
    ///
    /// # Errors
    ///
    /// Fails if it was skipped or never given: nothing that depends on it may be written.
    pub fn local(&self, key: &str, item: &Item) -> Result<String, AppError> {
        self.resolved
            .get(key)
            .cloned()
            .ok_or_else(|| reject("dependency_unresolved", &item.name).with("dependency", key))
    }

    /// The name for a new configuration: the file's, or a numbered variant when that is taken.
    pub fn free_name(taken: impl Fn(&str) -> bool, wanted: &str) -> (String, bool) {
        if !taken(wanted) {
            return (wanted.to_owned(), false);
        }
        for n in 2.. {
            let suffix = format!(" ({n})");
            let room = super::format::MAX_NAME_LEN - suffix.chars().count();
            let base: String = wanted.chars().take(room).collect();
            let candidate = format!("{}{suffix}", base.trim_end());
            if !taken(&candidate) {
                return (candidate, true);
            }
        }
        unreachable!("an unbounded range always yields a free name")
    }
}

pub(super) fn reject(reason: &'static str, item: &str) -> AppError {
    AppError::new(ErrorCode::ImportRejected)
        .with("reason", reason)
        .with("item", item)
}

pub(super) fn import(
    config: &ConfigRepository,
    runtimes: &RuntimeRegistry,
    package: &Package,
    request: &ImportRequest,
    workspace_id: Option<&str>,
) -> Result<ImportReport, AppError> {
    check_request(package, request)?;
    config.modify(|cfg| {
        let mut applier = Applier {
            package,
            request,
            runtimes,
            workspace_id,
            now: now_ms(),
            resolved: BTreeMap::new(),
        };
        bind_external(&mut applier, cfg)?;
        let mut order: Vec<&Item> = package.items.iter().collect();
        order.sort_by_key(|item| item.kind);
        let mut done: BTreeMap<&str, ImportedItem> = BTreeMap::new();
        for item in order {
            let imported = apply_item(&mut applier, cfg, item)?;
            done.insert(&item.key, imported);
        }
        Ok(ImportReport {
            items: package
                .items
                .iter()
                .filter_map(|item| done.remove(item.key.as_str()))
                .collect(),
        })
    })
}

/// A request about things the file does not have is a mistake, not something to ignore.
fn check_request(package: &Package, request: &ImportRequest) -> Result<(), AppError> {
    let items: BTreeSet<&str> = package.items.iter().map(|i| i.key.as_str()).collect();
    for key in request.decisions.keys().chain(request.models.keys()) {
        if !items.contains(key.as_str()) {
            return Err(reject("unknown_item", key));
        }
    }
    for key in request.bindings.keys() {
        if package.external(key).is_none() {
            return Err(reject("unknown_item", key));
        }
    }
    Ok(())
}

/// What the person said the external dependencies are, checked to be configurations of the
/// right kind that exist here.
fn bind_external(applier: &mut Applier<'_>, cfg: &UserConfig) -> Result<(), AppError> {
    for (key, id) in &applier.request.bindings {
        let Some(external) = applier.package.external(key) else {
            continue;
        };
        let scope = Scope {
            config: cfg,
            workspace_id: applier.workspace_id,
        };
        if !category(external.kind)
            .list(&scope)
            .iter()
            .any(|existing| &existing.id == id)
        {
            return Err(reject("binding_unknown", &external.name));
        }
        applier.resolved.insert(key.clone(), id.clone());
    }
    Ok(())
}

fn apply_item(
    applier: &mut Applier<'_>,
    cfg: &mut UserConfig,
    item: &Item,
) -> Result<ImportedItem, AppError> {
    let cat = category(item.kind);
    let existing = cat.existing(
        &Scope {
            config: cfg,
            workspace_id: applier.workspace_id,
        },
        item,
    );
    let decision = match (applier.request.decisions.get(&item.key), &existing) {
        (Some(decision), _) => *decision,
        (None, None) => Decision::Create,
        (None, Some(_)) => return Err(reject("decision_required", &item.name)),
    };
    let mut imported = ImportedItem {
        key: item.key.clone(),
        kind: item.kind,
        name: item.name.clone(),
        id: None,
        result: Action::Skipped,
        renamed_to: None,
        issues: Vec::new(),
        model: None,
    };
    match decision {
        Decision::Skip => {}
        Decision::UseExisting => {
            let existing = existing.ok_or_else(|| reject("no_existing", &item.name))?;
            applier
                .resolved
                .insert(item.key.clone(), existing.id.clone());
            imported.id = Some(existing.id);
            imported.result = Action::Reused;
        }
        Decision::Create | Decision::Replace => {
            let replacing = if decision == Decision::Replace {
                Some(existing.ok_or_else(|| reject("no_existing", &item.name))?)
            } else {
                None
            };
            let written = cat.write(applier, cfg, item, replacing.as_ref())?;
            applier
                .resolved
                .insert(item.key.clone(), written.id.clone());
            imported.result = if replacing.is_some() {
                Action::Replaced
            } else {
                Action::Created
            };
            if written.name != item.name.trim() {
                imported.renamed_to = Some(written.name);
            }
            imported.id = Some(written.id);
            imported.issues = written.issues;
            imported.model = written.model;
        }
    }
    Ok(imported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_taken_name_gets_the_first_free_number_and_stays_within_the_limit() {
        let taken = |name: &str| name == "Reviewer" || name == "Reviewer (2)";
        assert_eq!(
            Applier::free_name(taken, "Reviewer"),
            ("Reviewer (3)".to_owned(), true)
        );
        assert_eq!(
            Applier::free_name(taken, "Other"),
            ("Other".to_owned(), false)
        );
        let long = "x".repeat(super::super::format::MAX_NAME_LEN);
        let long_taken = |name: &str| name == long;
        let (name, renamed) = Applier::free_name(long_taken, &long);
        assert!(renamed);
        assert_eq!(name.chars().count(), super::super::format::MAX_NAME_LEN);
        assert!(name.ends_with("(2)"));
    }
}
