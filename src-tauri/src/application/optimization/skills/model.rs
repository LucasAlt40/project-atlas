use std::collections::BTreeMap;
use std::path::Path;

use crate::domain::optimization::TextSize;

/// Where a skill was found. A project's skill shadows a user skill of the same name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SkillScope {
    Project,
    User,
}

impl SkillScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::User => "user",
        }
    }
}

/// What a resource under a skill is, from its folder in the Agent Skills layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
    /// `references/`: documents the skill points at. May be loaded when the task needs them.
    Reference,
    /// `scripts/`: run, never read into a prompt.
    Script,
    /// `assets/` and anything else: templates, data, binaries.
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceEntry {
    /// Relative to the skill's folder, with `/` (`references/api.md`).
    pub path: String,
    pub kind: ResourceKind,
    pub bytes: u64,
}

/// The raw `SKILL.md` and a cheap fingerprint that changes when the file does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSkill {
    pub text: String,
    pub fingerprint: String,
}

/// Where skills are read from. A port: the filesystem adapter is in `infrastructure`, and tests
/// use an in-memory one. `base` is a folder that holds skill folders.
pub trait SkillStore: Send + Sync {
    /// The names of the folders in `base` that contain a `SKILL.md`.
    fn list(&self, base: &Path) -> Vec<String>;
    /// A skill's fingerprint without reading it (a stat), to tell whether a cached copy is stale.
    fn fingerprint(&self, base: &Path, name: &str) -> Option<String>;
    fn read(&self, base: &Path, name: &str) -> Option<RawSkill>;
    /// The files under the skill's folder other than `SKILL.md`, paths relative to it.
    fn resources(&self, base: &Path, name: &str) -> Vec<ResourceEntry>;
    /// One resource as text. `None` if it does not exist, is not text, is too big or its path
    /// would leave the skill's folder.
    fn read_resource(&self, base: &Path, name: &str, path: &str) -> Option<String>;
}

/// A parsed `SKILL.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSkill {
    /// The folder name.
    pub dir: String,
    /// The `name` field, when there is one.
    pub name: Option<String>,
    pub description: Option<String>,
    pub license: Option<String>,
    pub compatibility: Option<String>,
    pub allowed_tools: Vec<String>,
    /// The format's free-form `metadata` map, values as text.
    pub metadata: BTreeMap<String, String>,
    pub body: String,
    pub body_size: TextSize,
    pub body_lines: usize,
}

impl ParsedSkill {
    /// The name the skill goes by: its `name` field, else its folder.
    pub fn id(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.dir)
    }

    pub fn priority(&self) -> i32 {
        self.metadata
            .get("atlas-priority")
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0)
    }

    /// A comma-separated `atlas-*` hint as lowercase items.
    pub fn hint(&self, key: &str) -> Vec<String> {
        self.metadata
            .get(key)
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    NoFrontmatter,
    UnclosedFrontmatter,
    InvalidYaml(String),
    NotAMapping,
}

impl ParseError {
    pub fn message(&self) -> String {
        match self {
            Self::NoFrontmatter => "SKILL.md does not start with `---` frontmatter".to_owned(),
            Self::UnclosedFrontmatter => "the frontmatter is not closed with `---`".to_owned(),
            Self::InvalidYaml(e) => format!("the frontmatter is not valid YAML: {e}"),
            Self::NotAMapping => "the frontmatter is not a set of `key: value` fields".to_owned(),
        }
    }
}

fn scalar(value: &serde_norway::Value) -> Option<String> {
    match value {
        serde_norway::Value::String(s) => Some(s.trim().to_owned()),
        serde_norway::Value::Number(n) => Some(n.to_string()),
        serde_norway::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Splits `SKILL.md` into its frontmatter and its Markdown, and reads the fields of the Agent
/// Skills format. Unknown fields are ignored.
pub fn parse(dir: &str, text: &str) -> Result<ParsedSkill, ParseError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let rest = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))
        .ok_or(ParseError::NoFrontmatter)?;
    let mut front = String::new();
    let mut body_start = None;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        offset += line.len();
        if line.trim_end() == "---" {
            body_start = Some(offset);
            break;
        }
        front.push_str(line);
    }
    let body_start = body_start.ok_or(ParseError::UnclosedFrontmatter)?;
    let value: serde_norway::Value =
        serde_norway::from_str(&front).map_err(|e| ParseError::InvalidYaml(e.to_string()))?;
    let serde_norway::Value::Mapping(map) = value else {
        return Err(ParseError::NotAMapping);
    };
    let field = |key: &str| map.get(key).and_then(scalar).filter(|s| !s.is_empty());
    let allowed_tools = match map.get("allowed-tools") {
        Some(serde_norway::Value::String(s)) => s.split_whitespace().map(str::to_owned).collect(),
        Some(serde_norway::Value::Sequence(items)) => items.iter().filter_map(scalar).collect(),
        _ => Vec::new(),
    };
    let metadata = match map.get("metadata") {
        Some(serde_norway::Value::Mapping(m)) => m
            .iter()
            .filter_map(|(k, v)| Some((k.as_str()?.to_owned(), scalar(v)?)))
            .collect(),
        _ => BTreeMap::new(),
    };
    let body = rest[body_start..]
        .trim_start_matches(['\r', '\n'])
        .to_owned();
    Ok(ParsedSkill {
        dir: dir.to_owned(),
        name: field("name"),
        description: field("description"),
        license: field("license"),
        compatibility: field("compatibility"),
        allowed_tools,
        metadata,
        body_size: TextSize::of(&body),
        body_lines: body.lines().count(),
        body,
    })
}
