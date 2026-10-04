use serde::{Deserialize, Serialize};

use super::condition::Condition;

/// A transition. It is taken when its source completes and its condition, if any, holds for the
/// source's result. An edge without a condition is always taken.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowEdge {
    pub id: String,
    pub source_node_id: String,
    pub target_node_id: String,
    #[serde(default)]
    pub condition: Option<Condition>,
    #[serde(default)]
    pub label: String,
}
