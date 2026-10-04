use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::artifact::Artifact;
use super::decision::Decision;
use super::result::{Finding, ResultStatus};

/// What a validating step (a validator, QA) concluded, kept so the step that has to fix it
/// receives the failure, not a transcript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationEntry {
    pub node_id: String,
    pub execution_id: String,
    pub status: ResultStatus,
    #[serde(default)]
    pub outcome: Option<String>,
    pub summary: String,
    pub findings: Vec<Finding>,
}

/// Two steps that could run side by side touched the same place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlapWarning {
    pub node_ids: [String; 2],
    pub paths: Vec<String>,
}

/// The structured state the steps of one workflow run share. Steps do not talk to each other:
/// they leave artifacts, decisions and results here, and the orchestrator hands each step what
/// it needs. Every collection is ordered, so the state serialises the same way every time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SharedExecutionState {
    pub task: String,
    pub workflow: String,
    pub current_nodes: Vec<String>,
    pub completed_nodes: Vec<String>,
    pub failed_nodes: Vec<String>,
    pub artifacts: Vec<Artifact>,
    pub decisions: Vec<Decision>,
    pub active_agents: BTreeSet<String>,
    pub completed_agents: BTreeSet<String>,
    /// Areas and files each node reported touching, by node id.
    pub touched_areas: BTreeMap<String, BTreeSet<String>>,
    pub touched_files: BTreeMap<String, BTreeSet<String>>,
    pub validation_results: Vec<ValidationEntry>,
    pub warnings: Vec<OverlapWarning>,
    /// How many times each loop has gone round, by loop id.
    pub iteration_count: BTreeMap<String, u32>,
}
