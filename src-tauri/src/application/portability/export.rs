//! Writing the file. What goes in is decided by each category's `export`; what is shared by all
//! of them is here: which items a file needs, how they name each other, and what is warned about.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::format::{External, Item, Kind, Package, Source, FORMAT, SCHEMA_VERSION};
use super::{category, holds_a_secret};
use crate::application::config::UserConfig;
use crate::application::errors::{AppError, ErrorCode};
use crate::application::runtimes::RuntimeRegistry;
use crate::application::support::now_ms;

/// A configuration to export, by the id this installation knows it by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportTarget {
    pub kind: Kind,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedEntry {
    pub kind: Kind,
    pub name: String,
    /// Chosen by the person, as opposed to carried because something chosen needs it.
    pub chosen: bool,
}

/// Something about the file the person should know before sharing it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportWarning {
    /// `possible_secret`: a text looks like a credential. `agent_missing`: a step names an agent
    /// that no longer exists.
    pub code: &'static str,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedFile {
    /// A name for the file, safe on every system.
    pub file_name: String,
    pub text: String,
    pub items: Vec<ExportedEntry>,
    /// Needed but not carried (exported without dependencies).
    pub external: Vec<ExportedEntry>,
    pub warnings: Vec<ExportWarning>,
}

/// The file being built. Categories ask it for the `ref` of what they depend on; it decides
/// whether that is carried in the file or only named.
pub(crate) struct Exporter<'a> {
    pub config: &'a UserConfig,
    pub runtimes: &'a RuntimeRegistry,
    include_dependencies: bool,
    items: Vec<Item>,
    external: Vec<External>,
    keys: BTreeMap<(Kind, String), String>,
    pub warnings: Vec<ExportWarning>,
}

impl<'a> Exporter<'a> {
    fn new(
        config: &'a UserConfig,
        runtimes: &'a RuntimeRegistry,
        include_dependencies: bool,
    ) -> Self {
        Self {
            config,
            runtimes,
            include_dependencies,
            items: Vec::new(),
            external: Vec::new(),
            keys: BTreeMap::new(),
            warnings: Vec::new(),
        }
    }

    fn next_key(&self, kind: Kind) -> String {
        format!("{}-{}", kind.code().replace('_', "-"), self.keys.len() + 1)
    }

    /// Puts the configuration in the file and returns its `ref`.
    fn include(&mut self, kind: Kind, id: &str) -> Result<String, AppError> {
        if let Some(key) = self.keys.get(&(kind, id.to_owned())) {
            return Ok(key.clone());
        }
        let key = self.next_key(kind);
        self.keys.insert((kind, id.to_owned()), key.clone());
        let mut item = category(kind).export(self, id)?;
        item.key.clone_from(&key);
        if holds_a_secret(&item) {
            self.warnings.push(ExportWarning {
                code: "possible_secret",
                name: item.name.clone(),
            });
        }
        self.items.push(item);
        Ok(key)
    }

    /// The `ref` of a configuration an item depends on: carried in the file, or declared as
    /// needed. `None` when it no longer exists.
    ///
    /// # Errors
    ///
    /// Fails if a carried dependency cannot be exported.
    pub fn reference(&mut self, kind: Kind, id: &str) -> Result<Option<String>, AppError> {
        if self.include_dependencies {
            return if category(kind).describe(self.config, id).is_some() {
                self.include(kind, id).map(Some)
            } else {
                Ok(None)
            };
        }
        if let Some(key) = self.keys.get(&(kind, id.to_owned())) {
            return Ok(Some(key.clone()));
        }
        let Some(described) = category(kind).describe(self.config, id) else {
            return Ok(None);
        };
        let key = self.next_key(kind);
        self.keys.insert((kind, id.to_owned()), key.clone());
        self.external.push(External {
            key: key.clone(),
            kind,
            name: described.name,
            description: described.description,
            builtin_id: described.builtin_id,
        });
        Ok(Some(key))
    }
}

/// # Errors
///
/// Fails if a target does not exist or nothing was chosen.
pub(super) fn export(
    config: &UserConfig,
    runtimes: &RuntimeRegistry,
    targets: &[ExportTarget],
    include_dependencies: bool,
) -> Result<ExportedFile, AppError> {
    let mut exporter = Exporter::new(config, runtimes, include_dependencies);
    let mut roots: Vec<String> = Vec::new();
    for target in targets {
        if category(target.kind).describe(config, &target.id).is_none() {
            return Err(AppError::new(ErrorCode::ExportTargetNotFound)
                .with("kind", target.kind.code())
                .with("id", target.id.clone()));
        }
        let key = exporter.include(target.kind, &target.id)?;
        if !roots.contains(&key) {
            roots.push(key);
        }
    }
    if roots.is_empty() {
        return Err(AppError::new(ErrorCode::ExportTargetNotFound));
    }
    let entry = |kind: Kind, name: &str, chosen: bool| ExportedEntry {
        kind,
        name: name.to_owned(),
        chosen,
    };
    let items = exporter
        .items
        .iter()
        .map(|i| entry(i.kind, &i.name, roots.contains(&i.key)))
        .collect();
    let external = exporter
        .external
        .iter()
        .map(|e| entry(e.kind, &e.name, false))
        .collect();
    let file_name = file_name(&exporter.items, &roots);
    let package = Package {
        format: FORMAT.to_owned(),
        schema_version: SCHEMA_VERSION,
        exported_at: Some(now_ms()),
        source: Source {
            app: "atlas".to_owned(),
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
        },
        roots,
        items: exporter.items,
        external: exporter.external,
    };
    Ok(ExportedFile {
        file_name,
        text: package.to_text(),
        items,
        external,
        warnings: exporter.warnings,
    })
}

/// `reviewer.atlas.json` for one configuration, `atlas-configs.atlas.json` for several: letters,
/// digits and dashes only, so every system accepts it.
fn file_name(items: &[Item], roots: &[String]) -> String {
    let stem = match roots {
        [only] => items
            .iter()
            .find(|i| &i.key == only)
            .map(|i| slug(&i.name))
            .unwrap_or_default(),
        _ => "atlas-configs".to_owned(),
    };
    let stem = if stem.is_empty() {
        "config".to_owned()
    } else {
        stem
    };
    format!("{stem}.atlas.json")
}

fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').chars().take(60).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_becomes_a_file_name_every_system_accepts() {
        assert_eq!(slug("Arquiteto Sênior / v2"), "arquiteto-s-nior-v2");
        assert_eq!(slug("  ..  "), "");
        assert_eq!(slug(&"a".repeat(100)).len(), 60);
        let item = |key: &str, name: &str| Item {
            key: key.to_owned(),
            kind: Kind::Agent,
            name: name.to_owned(),
            description: String::new(),
            spec: serde_json::Value::Null,
        };
        assert_eq!(
            file_name(&[item("a", "!!!")], &["a".to_owned()]),
            "config.atlas.json"
        );
        assert_eq!(
            file_name(
                &[item("a", "x"), item("b", "y")],
                &["a".to_owned(), "b".to_owned()]
            ),
            "atlas-configs.atlas.json"
        );
    }
}
