use serde::{Deserialize, Serialize};

use super::edge::WorkflowEdge;
use super::node::{NodeKind, WorkflowNode};

/// Who made the graph. The model is the same: only the author differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowMode {
    Automatic,
    Custom,
}

/// Whether the definition passed validation when it was last saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStatus {
    #[default]
    Draft,
    Ready,
}

/// The canvas view. Presentation only.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Viewport {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}

/// The declarative definition of a process. It says nothing about any run of it: a run is a
/// [`super::execution::WorkflowExecution`], which carries a snapshot of the version it started from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workflow {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub mode: WorkflowMode,
    /// Starts at 1 and grows with every structural change.
    pub version: u32,
    #[serde(default)]
    pub status: WorkflowStatus,
    /// The template it came from, when it came from one.
    #[serde(default)]
    pub template_id: Option<String>,
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
    #[serde(default)]
    pub viewport: Option<Viewport>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl Workflow {
    pub fn node(&self, id: &str) -> Option<&WorkflowNode> {
        self.nodes.iter().find(|node| node.id == id)
    }

    /// Whether `other` describes the same process: same nodes, edges and policies. Where nodes
    /// sit on the canvas, the viewport and the name are not part of the process.
    pub fn same_structure(&self, other: &Self) -> bool {
        let strip = |workflow: &Self| {
            let mut nodes = workflow.nodes.clone();
            for node in &mut nodes {
                node.position = None;
            }
            (nodes, workflow.edges.clone())
        };
        strip(self) == strip(other)
    }

    /// The agents the workflow uses, in node order, without repeats.
    pub fn agent_ids(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = Vec::new();
        for node in &self.nodes {
            if let NodeKind::Agent(agent) = &node.kind {
                if !ids.contains(&agent.agent_id.as_str()) {
                    ids.push(&agent.agent_id);
                }
            }
        }
        ids
    }
}
