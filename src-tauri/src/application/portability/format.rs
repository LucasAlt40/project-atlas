//! The interchange file: an envelope Atlas can recognise, version and read without trusting it.
//!
//! JSON, because it is what the rest of Atlas speaks (serde, the webview), it has no implicit
//! typing or aliasing to be surprised by, and a person can still read and edit it. The file is
//! its own contract: nothing in it is a stored record, so the database may change without
//! breaking a file somebody exported last year.
//!
//! Compatibility rules, in one place:
//! - `format` names the file, `schemaVersion` the contract. A newer schema is refused (with the
//!   version said), an older one is brought forward by [`UPGRADES`] before it is read.
//! - Adding an optional field does not change the version; unknown fields are ignored.
//!   Renaming, removing or re-meaning a field does, and comes with an upgrade step.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::application::errors::{AppError, ErrorCode};

pub const FORMAT: &str = "atlas-config";
pub const SCHEMA_VERSION: u32 = 1;
/// Larger files are refused before they are parsed.
pub const MAX_FILE_BYTES: usize = 2 * 1024 * 1024;
const MAX_ITEMS: usize = 200;
const MAX_REF_LEN: usize = 64;
pub const MAX_NAME_LEN: usize = 80;
const MAX_DESCRIPTION_LEN: usize = 4_000;

/// The kinds of configuration a file can carry. Declared in the order dependencies are written:
/// an item may only depend on kinds that come before its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Personality,
    Agent,
    Workflow,
    WorkflowTemplate,
}

impl Kind {
    pub const ALL: [Self; 4] = [
        Self::Personality,
        Self::Agent,
        Self::Workflow,
        Self::WorkflowTemplate,
    ];

    pub fn code(self) -> &'static str {
        match self {
            Self::Personality => "personality",
            Self::Agent => "agent",
            Self::Workflow => "workflow",
            Self::WorkflowTemplate => "workflow_template",
        }
    }
}

/// What wrote the file. Information for the person reading it; nothing depends on it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub app: String,
    pub app_version: String,
}

/// One configuration in the file. `key` is only meaningful inside the file: it is how items name
/// each other, and it is never an id of the installation that exported it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    #[serde(rename = "ref")]
    pub key: String,
    pub kind: Kind,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// What the kind needs to rebuild it; read by that kind's category (see `spec.rs`).
    pub spec: Value,
}

/// A configuration an item needs that the file does not carry (exported without its
/// dependencies). The importer asks which of its own to use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct External {
    #[serde(rename = "ref")]
    pub key: String,
    pub kind: Kind,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// The id of a built-in personality (these ship with Atlas, so the id means the same on
    /// every installation).
    #[serde(default)]
    pub builtin_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    pub format: String,
    pub schema_version: u32,
    /// Milliseconds since the Unix epoch.
    #[serde(default)]
    pub exported_at: Option<u64>,
    #[serde(default)]
    pub source: Source,
    /// The items the person chose to export; the others are there because those need them.
    pub roots: Vec<String>,
    pub items: Vec<Item>,
    #[serde(default)]
    pub external: Vec<External>,
}

impl Package {
    pub fn item(&self, key: &str) -> Option<&Item> {
        self.items.iter().find(|item| item.key == key)
    }

    pub fn external(&self, key: &str) -> Option<&External> {
        self.external.iter().find(|e| e.key == key)
    }

    /// The kind of whatever `key` names, in the file or declared external.
    pub fn kind_of(&self, key: &str) -> Option<Kind> {
        self.item(key)
            .map(|i| i.kind)
            .or_else(|| self.external(key).map(|e| e.kind))
    }

    pub fn to_text(&self) -> String {
        // A structure of strings, numbers and options cannot fail to serialize.
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

/// Why a file cannot be read. Never carries the file's content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    TooLarge,
    NotJson(String),
    NotAtlas,
    Unsupported { found: u32 },
    Invalid { reason: &'static str, at: String },
}

impl FormatError {
    pub fn invalid(reason: &'static str, at: impl Into<String>) -> Self {
        Self::Invalid {
            reason,
            at: at.into(),
        }
    }
}

impl From<FormatError> for AppError {
    fn from(error: FormatError) -> Self {
        match error {
            FormatError::TooLarge => Self::new(ErrorCode::ImportFileTooLarge)
                .with("max", (MAX_FILE_BYTES / (1024 * 1024)).to_string()),
            FormatError::NotJson(detail) => {
                Self::new(ErrorCode::ImportNotAtlasFile).with_detail(detail)
            }
            FormatError::NotAtlas => Self::new(ErrorCode::ImportNotAtlasFile),
            FormatError::Unsupported { found } => Self::new(ErrorCode::ImportUnsupportedVersion)
                .with("found", found.to_string())
                .with("supported", SCHEMA_VERSION.to_string()),
            FormatError::Invalid { reason, at } => Self::new(ErrorCode::ImportInvalid)
                .with("reason", reason)
                .with("at", at),
        }
    }
}

/// One step that rewrites a file of schema `n + 1`'s predecessor into schema `n + 1`. Empty while
/// the schema is at 1: the day it becomes 2, the rewrite from 1 goes here, and old files go on
/// working.
type Upgrade = fn(Value) -> Value;
const UPGRADES: &[Upgrade] = &[];

/// Reads the envelope of a file: size, JSON, identity, version, shape and the references between
/// items. What each item's `spec` holds is checked by its category, after this.
///
/// # Errors
///
/// See [`FormatError`].
pub fn parse(text: &str) -> Result<Package, FormatError> {
    if text.len() > MAX_FILE_BYTES {
        return Err(FormatError::TooLarge);
    }
    let value: Value =
        serde_json::from_str(text).map_err(|error| FormatError::NotJson(error.to_string()))?;
    if value.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Err(FormatError::NotAtlas);
    }
    let version = value
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .filter(|v| *v >= 1)
        .ok_or_else(|| FormatError::invalid("schema_version", "schemaVersion"))?;
    let value = upgrade(value, version, UPGRADES)?;
    check_kinds(&value)?;
    let package: Package = serde_json::from_value(value)
        .map_err(|error| FormatError::invalid("structure", error.to_string()))?;
    check_package(&package)?;
    Ok(package)
}

/// Brings a file of schema `from` to the current one with `steps` (`steps[0]` turns 1 into 2).
fn upgrade(mut value: Value, from: u32, steps: &[Upgrade]) -> Result<Value, FormatError> {
    let current = u32::try_from(steps.len()).unwrap_or(u32::MAX) + 1;
    if from > current {
        return Err(FormatError::Unsupported { found: from });
    }
    for step in &steps[(from - 1) as usize..] {
        value = step(value);
    }
    if let Some(version) = value.get_mut("schemaVersion") {
        *version = Value::from(current);
    }
    Ok(value)
}

/// A kind this Atlas does not know is named as such, instead of as a serde message.
fn check_kinds(value: &Value) -> Result<(), FormatError> {
    for (list, name) in [("items", "items"), ("external", "external")] {
        let Some(entries) = value.get(list).and_then(Value::as_array) else {
            continue;
        };
        for (index, entry) in entries.iter().enumerate() {
            let known = entry
                .get("kind")
                .and_then(Value::as_str)
                .is_some_and(|kind| Kind::ALL.iter().any(|k| k.code() == kind));
            if !known {
                return Err(FormatError::invalid(
                    "unsupported_kind",
                    format!("{name}[{index}].kind"),
                ));
            }
        }
    }
    Ok(())
}

fn check_package(package: &Package) -> Result<(), FormatError> {
    if package.items.is_empty() {
        return Err(FormatError::invalid("empty", "items"));
    }
    if package.items.len() > MAX_ITEMS {
        return Err(FormatError::invalid("too_many_items", "items"));
    }
    let mut keys = BTreeSet::new();
    for (index, item) in package.items.iter().enumerate() {
        let at = format!("items[{index}]");
        check_key(&item.key, &at)?;
        if !keys.insert(item.key.as_str()) {
            return Err(FormatError::invalid("duplicate_ref", at));
        }
        check_text(&item.name, &item.description, &at)?;
    }
    for (index, external) in package.external.iter().enumerate() {
        let at = format!("external[{index}]");
        check_key(&external.key, &at)?;
        if !keys.insert(external.key.as_str()) {
            return Err(FormatError::invalid("duplicate_ref", at));
        }
        check_text(&external.name, &external.description, &at)?;
    }
    if package.roots.is_empty() {
        return Err(FormatError::invalid("no_roots", "roots"));
    }
    for root in &package.roots {
        if package.item(root).is_none() {
            return Err(FormatError::invalid("unknown_ref", format!("roots.{root}")));
        }
    }
    Ok(())
}

fn check_key(key: &str, at: &str) -> Result<(), FormatError> {
    let valid = !key.is_empty()
        && key.len() <= MAX_REF_LEN
        && key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if valid {
        Ok(())
    } else {
        Err(FormatError::invalid("invalid_ref", at))
    }
}

/// Names and descriptions are shown to a person before anything else: no control characters, and
/// a size a screen can hold.
fn check_text(name: &str, description: &str, at: &str) -> Result<(), FormatError> {
    if name.trim().is_empty() {
        return Err(FormatError::invalid("name_required", at));
    }
    let printable = |text: &str| text.chars().all(|c| !c.is_control() || c == '\n');
    if name.chars().count() > MAX_NAME_LEN || !printable(name) || name.contains('\n') {
        return Err(FormatError::invalid("invalid_name", at));
    }
    if description.chars().count() > MAX_DESCRIPTION_LEN || !printable(description) {
        return Err(FormatError::invalid("invalid_description", at));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn file() -> Value {
        json!({
            "format": FORMAT,
            "schemaVersion": 1,
            "roots": ["personality-1"],
            "items": [{
                "ref": "personality-1",
                "kind": "personality",
                "name": "Reviewer",
                "spec": {"instructions": "Review."}
            }]
        })
    }

    fn parse_value(value: &Value) -> Result<Package, FormatError> {
        parse(&value.to_string())
    }

    #[test]
    fn reads_a_minimal_file_and_defaults_what_is_optional() {
        let package = parse_value(&file()).unwrap();

        assert_eq!(package.schema_version, 1);
        assert_eq!(package.items[0].description, "");
        assert_eq!(package.exported_at, None);
        assert_eq!(package.external, Vec::new());
    }

    #[test]
    fn a_file_that_is_not_json_or_not_ours_is_said_so() {
        assert!(matches!(parse("{ nope"), Err(FormatError::NotJson(_))));
        assert_eq!(parse("[]"), Err(FormatError::NotAtlas));
        assert_eq!(
            parse(&json!({"format": "something-else", "schemaVersion": 1}).to_string()),
            Err(FormatError::NotAtlas)
        );
    }

    #[test]
    fn a_newer_schema_is_refused_with_its_version() {
        let mut value = file();
        value["schemaVersion"] = json!(2);

        assert_eq!(
            parse_value(&value),
            Err(FormatError::Unsupported { found: 2 })
        );
    }

    #[test]
    fn a_missing_or_nonsense_version_is_invalid() {
        for version in [json!(null), json!(0), json!(-1), json!("1"), json!(1.5)] {
            let mut value = file();
            value["schemaVersion"] = version;
            assert!(
                matches!(
                    parse_value(&value),
                    Err(FormatError::Invalid {
                        reason: "schema_version",
                        ..
                    })
                ),
                "{value}"
            );
        }
    }

    #[test]
    fn a_file_over_the_limit_is_refused_before_it_is_parsed() {
        let big = format!(
            "{{\"format\":\"{FORMAT}\",\"pad\":\"{}\"}}",
            "x".repeat(MAX_FILE_BYTES)
        );

        assert_eq!(parse(&big), Err(FormatError::TooLarge));
    }

    #[test]
    fn unknown_fields_are_ignored_but_unknown_kinds_are_not() {
        let mut value = file();
        value["somethingNew"] = json!(true);
        value["items"][0]["alsoNew"] = json!(1);
        assert!(parse_value(&value).is_ok());

        value["items"][0]["kind"] = json!("robot");
        assert_eq!(
            parse_value(&value),
            Err(FormatError::invalid("unsupported_kind", "items[0].kind"))
        );
    }

    #[test]
    fn refs_must_be_well_formed_unique_and_roots_must_exist() {
        let mut value = file();
        value["items"][0]["ref"] = json!("Has Space");
        assert!(matches!(
            parse_value(&value),
            Err(FormatError::Invalid {
                reason: "invalid_ref",
                ..
            })
        ));

        let mut value = file();
        let again = value["items"][0].clone();
        value["items"].as_array_mut().unwrap().push(again);
        assert!(matches!(
            parse_value(&value),
            Err(FormatError::Invalid {
                reason: "duplicate_ref",
                ..
            })
        ));

        let mut value = file();
        value["roots"] = json!(["ghost"]);
        assert!(matches!(
            parse_value(&value),
            Err(FormatError::Invalid {
                reason: "unknown_ref",
                ..
            })
        ));

        let mut value = file();
        value["roots"] = json!([]);
        assert!(matches!(
            parse_value(&value),
            Err(FormatError::Invalid {
                reason: "no_roots",
                ..
            })
        ));
    }

    #[test]
    fn names_are_text_a_person_can_read() {
        for name in [
            "",
            "   ",
            "two\nlines",
            "bell\u{7}",
            &"x".repeat(MAX_NAME_LEN + 1),
        ] {
            let mut value = file();
            value["items"][0]["name"] = json!(name);
            assert!(parse_value(&value).is_err(), "{name:?}");
        }
    }

    #[test]
    fn a_missing_required_field_is_reported_where_it_is() {
        let mut value = file();
        value["items"][0].as_object_mut().unwrap().remove("spec");

        let Err(FormatError::Invalid { reason, at }) = parse_value(&value) else {
            panic!("should be invalid");
        };
        assert_eq!(reason, "structure");
        assert!(at.contains("spec"), "{at}");
    }

    #[test]
    fn an_older_schema_is_brought_forward_step_by_step() {
        fn rename(mut value: Value) -> Value {
            value["roots"] = value["entries"].take();
            value
        }
        let old = json!({"format": FORMAT, "schemaVersion": 1, "entries": ["a"]});

        let upgraded = upgrade(old, 1, &[rename]).unwrap();

        assert_eq!(upgraded["roots"], json!(["a"]));
        assert_eq!(upgraded["schemaVersion"], json!(2));
        assert_eq!(
            upgrade(json!({"schemaVersion": 3}), 3, &[rename]),
            Err(FormatError::Unsupported { found: 3 })
        );
    }

    #[test]
    fn errors_map_to_codes_the_ui_can_word() {
        assert!(AppError::from(FormatError::TooLarge).is(ErrorCode::ImportFileTooLarge));
        assert!(AppError::from(FormatError::NotAtlas).is(ErrorCode::ImportNotAtlasFile));
        let unsupported = AppError::from(FormatError::Unsupported { found: 9 });
        assert!(unsupported.is(ErrorCode::ImportUnsupportedVersion));
        assert_eq!(unsupported.params["found"], "9");
    }
}
