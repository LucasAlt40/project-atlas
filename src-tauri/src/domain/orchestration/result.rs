use serde::{Deserialize, Serialize};

use crate::domain::workflow::Facts;

/// How a step says it went. Edges route on it, so it is the one thing an agent says that
/// matters to the graph, and even so only through conditions the workflow itself defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ResultStatus {
    Pass,
    Fail,
    Warning,
    Success,
    /// The agent gave no structured result. Never read as a pass.
    #[default]
    Unknown,
}

impl ResultStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Warning => "warning",
            Self::Success => "success",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(text: &str) -> Self {
        match text.trim().to_ascii_lowercase().as_str() {
            "pass" | "passed" => Self::Pass,
            "fail" | "failed" => Self::Fail,
            "warning" | "warn" => Self::Warning,
            "success" | "ok" | "done" => Self::Success,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub title: String,
    /// A path inside the project, when the finding is about one file.
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub line: Option<u32>,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub evidence: String,
    #[serde(default)]
    pub recommendation: String,
}

/// An artifact as an agent describes it, before Atlas gives it an id and a producer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactDraft {
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DecisionDraft {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub decision: String,
    #[serde(default)]
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ValidationSummary {
    #[serde(default)]
    pub status: String,
}

/// What a step reports, structured. Runtimes are not required to produce it: without it Atlas
/// keeps a textual summary and the status is `unknown`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AgentResult {
    #[serde(default)]
    pub status: ResultStatus,
    /// One of the outcomes the agent's contract declares: what a workflow routes on. `None`
    /// when the agent declared none, or said something its contract does not allow. Not the
    /// execution's status: an execution that completed can carry the outcome `fail`.
    #[serde(default)]
    pub outcome: Option<String>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub artifacts: Vec<ArtifactDraft>,
    #[serde(default)]
    pub decisions: Vec<DecisionDraft>,
    #[serde(default)]
    pub findings: Vec<Finding>,
    #[serde(default)]
    pub touched_files: Vec<String>,
    #[serde(default)]
    pub touched_areas: Vec<String>,
    #[serde(default)]
    pub validation: Option<ValidationSummary>,
    /// What the agent thinks should happen next. Information for the user: the workflow decides.
    #[serde(default)]
    pub next_action: Option<String>,
    /// The result came from the agent's structured block, not from the fallback summary.
    #[serde(default)]
    pub structured: bool,
}

impl AgentResult {
    /// The facts conditions can test.
    pub fn facts(&self) -> Facts {
        let mut facts = Facts::new();
        facts.insert("result.status".to_owned(), self.status.as_str().to_owned());
        if let Some(outcome) = self.outcome.as_ref().filter(|o| !o.is_empty()) {
            facts.insert("result.outcome".to_owned(), outcome.clone());
        }
        if !self.summary.is_empty() {
            facts.insert("result.summary".to_owned(), self.summary.clone());
        }
        if let Some(next) = self.next_action.as_ref().filter(|n| !n.is_empty()) {
            facts.insert("result.next_action".to_owned(), next.clone());
        }
        if !self.findings.is_empty() {
            facts.insert(
                "result.findings".to_owned(),
                self.findings.len().to_string(),
            );
        }
        if let Some(validation) = self.validation.as_ref().filter(|v| !v.status.is_empty()) {
            facts.insert(
                "validation.status".to_owned(),
                validation.status.to_ascii_lowercase(),
            );
        }
        facts
    }
}
