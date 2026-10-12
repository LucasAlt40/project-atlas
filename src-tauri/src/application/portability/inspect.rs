//! Looking at a file against this installation, changing nothing: what each item would be,
//! what it matches, what it needs, and what is wrong with it. This is what the person reads
//! before deciding.

use std::cell::{OnceCell, RefCell};
use std::collections::BTreeMap;

use serde::Serialize;

use super::format::{Item, Kind, Package, Source};
use super::kinds::agent::ModelAssessment;
use super::{category, Decision, Existing, Issue, Scope, Severity};
use crate::application::config::UserConfig;
use crate::application::runtimes::RuntimeRegistry;
use crate::domain::runtime::RuntimeStatus;

/// One fact about an item, for the person: a code the UI words and its value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Fact {
    pub label: &'static str,
    pub value: String,
}

impl Fact {
    pub fn new(label: &'static str, value: impl Into<String>) -> Self {
        Self {
            label,
            value: value.into(),
        }
    }
}

/// A long text of an item (instructions), shown so the person reads what an agent will be told.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Excerpt {
    pub value: String,
    pub truncated: bool,
}

const EXCERPT_CHARS: usize = 6_000;

impl Excerpt {
    pub fn of(text: &str) -> Self {
        let truncated = text.chars().count() > EXCERPT_CHARS;
        Self {
            value: text.chars().take(EXCERPT_CHARS).collect(),
            truncated,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepPreview {
    pub label: String,
    /// `agent`, `condition` or `end`.
    pub kind: &'static str,
    /// The `ref` of the agent that runs it.
    pub agent: Option<String>,
    pub instructions: Option<Excerpt>,
    pub read_only: bool,
}

/// What a category found out about an item. The flow adds what is common to all kinds.
#[derive(Default)]
pub(crate) struct Inspection {
    pub facts: Vec<Fact>,
    pub text: Option<Excerpt>,
    pub steps: Vec<StepPreview>,
    /// Its content equals what the installation already has under that name.
    pub identical: bool,
    pub model: Option<ModelAssessment>,
    pub issues: Vec<Issue>,
}

/// The installation as a category sees it while inspecting.
pub(crate) struct Inspector<'a> {
    pub config: &'a UserConfig,
    pub package: &'a Package,
    pub workspace_id: Option<&'a str>,
    runtimes: &'a RuntimeRegistry,
    statuses: OnceCell<Vec<RuntimeStatus>>,
    /// The local configuration each dependency would be, were the file imported as it stands: set
    /// for the items that would reuse what is here.
    reused: RefCell<BTreeMap<String, String>>,
}

impl<'a> Inspector<'a> {
    pub fn scope(&self) -> Scope<'a> {
        Scope {
            config: self.config,
            workspace_id: self.workspace_id,
        }
    }

    /// What every runtime offers here. Asked once, and only when something needs it: each
    /// runtime is a process to start.
    pub fn runtime_statuses(&self) -> &[RuntimeStatus] {
        self.statuses.get_or_init(|| self.runtimes.inspect_all())
    }

    /// The local id a dependency of the file stands for, when it is one that exists already.
    pub fn local_for(&self, key: &str) -> Option<String> {
        self.reused.borrow().get(key).cloned()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemPreview {
    #[serde(rename = "ref")]
    pub key: String,
    pub kind: Kind,
    pub name: String,
    pub description: String,
    /// Chosen by whoever exported it, as opposed to carried because something needs it.
    pub root: bool,
    pub facts: Vec<Fact>,
    pub text: Option<Excerpt>,
    pub steps: Vec<StepPreview>,
    /// What it matches here, by name.
    pub existing: Option<Existing>,
    pub identical: bool,
    pub default_decision: Decision,
    pub allowed: Vec<Decision>,
    /// The `ref`s it needs.
    pub depends_on: Vec<String>,
    pub model: Option<ModelAssessment>,
    pub issues: Vec<Issue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub id: String,
    pub name: String,
}

/// Something the file needs and does not carry: the person picks which of theirs it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalPreview {
    #[serde(rename = "ref")]
    pub key: String,
    pub kind: Kind,
    pub name: String,
    pub description: String,
    /// The one with the same name here. Offered, never applied on its own.
    pub suggested: Option<String>,
    pub candidates: Vec<Candidate>,
    pub used_by: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub schema_version: u32,
    pub exported_at: Option<u64>,
    pub source: Source,
    pub roots: Vec<String>,
    pub items: Vec<ItemPreview>,
    pub external: Vec<ExternalPreview>,
    /// Workflows belong to a workspace: one has to be chosen.
    pub needs_workspace: bool,
    /// Some agent asks for the permission to edit files.
    pub grants_editing: bool,
}

pub(super) fn preview(
    package: &Package,
    config: &UserConfig,
    runtimes: &RuntimeRegistry,
    workspace_id: Option<&str>,
) -> ImportPreview {
    let inspector = Inspector {
        config,
        package,
        workspace_id,
        runtimes,
        statuses: OnceCell::new(),
        reused: RefCell::new(BTreeMap::new()),
    };
    let scope = inspector.scope();
    let mut order: Vec<&Item> = package.items.iter().collect();
    order.sort_by_key(|item| item.kind);
    let mut previews: BTreeMap<&str, ItemPreview> = BTreeMap::new();
    for item in order {
        let cat = category(item.kind);
        let mut inspection = cat.inspect(&inspector, item);
        if super::holds_a_secret(item) {
            inspection.issues.push(Issue::warning("possible_secret"));
        }
        let existing = cat.existing(&scope, item);
        let blocked = inspection
            .issues
            .iter()
            .any(|issue| issue.severity == Severity::Error);
        let identical = existing.is_some() && inspection.identical;
        let default_decision = if identical {
            Decision::UseExisting
        } else if blocked {
            Decision::Skip
        } else {
            Decision::Create
        };
        if let (Decision::UseExisting, Some(existing)) = (default_decision, &existing) {
            inspector
                .reused
                .borrow_mut()
                .insert(item.key.clone(), existing.id.clone());
        }
        let mut allowed = vec![Decision::Skip];
        if !blocked {
            allowed.push(Decision::Create);
        }
        if existing.is_some() {
            allowed.push(Decision::UseExisting);
            if !blocked {
                allowed.push(Decision::Replace);
            }
        }
        let depends_on = cat
            .check(item, "")
            .map(|deps| deps.into_iter().map(|d| d.key).collect())
            .unwrap_or_default();
        previews.insert(
            &item.key,
            ItemPreview {
                key: item.key.clone(),
                kind: item.kind,
                name: item.name.clone(),
                description: item.description.clone(),
                root: package.roots.contains(&item.key),
                facts: inspection.facts,
                text: inspection.text,
                steps: inspection.steps,
                existing,
                identical,
                default_decision,
                allowed,
                depends_on,
                model: inspection.model,
                issues: inspection.issues,
            },
        );
    }
    let items: Vec<ItemPreview> = package
        .items
        .iter()
        .filter_map(|item| previews.remove(item.key.as_str()))
        .collect();
    let external = externals(package, &scope, &items);
    ImportPreview {
        schema_version: package.schema_version,
        exported_at: package.exported_at,
        source: package.source.clone(),
        roots: package.roots.clone(),
        needs_workspace: items.iter().any(|item| item.kind == Kind::Workflow),
        grants_editing: items
            .iter()
            .any(|item| item.issues.iter().any(|i| i.code == "grants_editing")),
        items,
        external,
    }
}

/// What the file needs and does not carry, with the local candidates for each.
fn externals(package: &Package, scope: &Scope<'_>, items: &[ItemPreview]) -> Vec<ExternalPreview> {
    package
        .external
        .iter()
        .map(|external| {
            let candidates: Vec<Candidate> = category(external.kind)
                .list(scope)
                .into_iter()
                .map(|c| Candidate {
                    id: c.id,
                    name: c.name,
                })
                .collect();
            let wanted = external.name.trim().to_lowercase();
            let suggested = candidates
                .iter()
                .find(|c| external.builtin_id.as_deref() == Some(c.id.as_str()))
                .or_else(|| {
                    candidates
                        .iter()
                        .find(|c| c.name.trim().to_lowercase() == wanted)
                })
                .map(|c| c.id.clone());
            ExternalPreview {
                key: external.key.clone(),
                kind: external.kind,
                name: external.name.clone(),
                description: external.description.clone(),
                suggested,
                candidates,
                used_by: items
                    .iter()
                    .filter(|item| item.depends_on.contains(&external.key))
                    .map(|item| item.key.clone())
                    .collect(),
            }
        })
        .collect()
}
