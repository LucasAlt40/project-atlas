use serde::{Deserialize, Serialize};

use crate::domain::orchestration::{ArtifactType, Finding, ResultStatus};
use crate::domain::worktree::FileChange;

/// How the step that produced a handoff ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandoffKind {
    /// The step completed and its result routed the workflow here.
    Result,
    /// The step failed and its failure route sent the workflow here.
    Failure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HandoffDecision {
    pub title: String,
    pub decision: String,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HandoffArtifact {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: ArtifactType,
    pub path: Option<String>,
    pub summary: String,
}

/// What a validating step concluded. `status` is `unknown` when the step said nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HandoffValidation {
    pub status: ResultStatus,
    #[serde(default)]
    pub outcome: Option<String>,
    pub summary: String,
    pub findings: Vec<Finding>,
}

/// What one step hands to the next, over one transition of the workflow. The orchestrator
/// builds it from the step's structured result and from Git; the next step is told it, as
/// context (it grants nothing). Anything the step did not say is empty, never invented.
///
/// Where the code is concerned, `changed_files` is what Git measured in the shared worktree
/// during the step: the source of truth. `reported_files` is only what the agent claimed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentHandoff {
    pub id: String,
    pub workflow_execution_id: String,
    /// The edge (or `failure:<node>` route) this travelled over.
    pub link_id: String,
    pub from_node_id: String,
    pub to_node_id: String,
    pub from_execution_id: String,
    /// Which pass through the sending node.
    pub iteration: u32,
    pub kind: HandoffKind,
    pub created_at: u64,
    pub status: ResultStatus,
    /// The outcome the step declared (`pass`, `fail`, `approved`…), when its agent has a result
    /// contract. Not the status of the execution: a step that completed can say `fail`.
    #[serde(default)]
    pub outcome: Option<String>,
    pub summary: String,
    /// What the agent suggested should happen next. Information: the workflow decides.
    pub instructions: Option<String>,
    pub decisions: Vec<HandoffDecision>,
    pub artifacts: Vec<HandoffArtifact>,
    pub changed_files: Vec<FileChange>,
    /// Changes in the worktree that are not in a commit yet.
    pub uncommitted_files: Vec<String>,
    pub reported_files: Vec<String>,
    pub validation: Option<HandoffValidation>,
    /// For a failure route: why the step failed.
    pub failure: Option<String>,
}
