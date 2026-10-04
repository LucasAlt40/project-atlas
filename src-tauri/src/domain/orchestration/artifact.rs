use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactType {
    ArchitectureDocument,
    ApiContract,
    DatabaseMigration,
    ImplementationSummary,
    TestReport,
    ValidationReport,
    BugReport,
    Other,
}

impl ArtifactType {
    /// Reads the wire name an agent used; anything unknown is `Other`.
    pub fn parse(name: &str) -> Self {
        match name
            .trim()
            .to_ascii_lowercase()
            .replace(['-', ' '], "_")
            .as_str()
        {
            "architecture_document" => Self::ArchitectureDocument,
            "api_contract" => Self::ApiContract,
            "database_migration" => Self::DatabaseMigration,
            "implementation_summary" => Self::ImplementationSummary,
            "test_report" => Self::TestReport,
            "validation_report" => Self::ValidationReport,
            "bug_report" => Self::BugReport,
            _ => Self::Other,
        }
    }
}

/// A shareable output of a step. It points at the work (a file, an execution) and summarises it;
/// the full content is not copied into the workflow's state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: ArtifactType,
    pub name: String,
    pub producer_node_id: String,
    pub execution_id: String,
    /// A path inside the project or worktree, relative. Never read by Atlas itself.
    #[serde(default)]
    pub path: Option<String>,
    pub summary: String,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
    pub created_at: u64,
}
