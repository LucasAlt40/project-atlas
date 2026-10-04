use serde::{Deserialize, Serialize};

use super::condition::Condition;

/// Where a node sits on the canvas. Presentation only: it never affects how the graph runs.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

/// A retry is explicit and bounded. `max_retries = 2` means up to three attempts per iteration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RetryPolicy {
    pub max_retries: u32,
}

/// What happens when a step still fails after its retries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FailurePolicy {
    /// The failure is final: whatever depends on the node is blocked and the workflow fails once
    /// nothing else can run.
    #[default]
    StopWorkflow,
    /// The workflow carries on at another node, as a failure route.
    #[serde(rename_all = "camelCase")]
    RouteToNode { node_id: String },
}

/// How a step runs. The agent stays the source of truth: a node can only inherit the agent's
/// isolation, never weaken it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IsolationMode {
    #[default]
    FromAgent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPolicy {
    #[serde(default)]
    pub isolation: IsolationMode,
}

/// Bounds a loop: the node may start at most `max_iterations` times in one workflow run. Every
/// cycle in a graph must pass through at least one node that has one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopPolicy {
    pub loop_id: String,
    pub max_iterations: u32,
}

/// One run of an existing Agent. Personality, runtime and model are the agent's; the node only
/// adds what is specific to this step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentNode {
    pub agent_id: String,
    pub label: String,
    #[serde(default)]
    pub instructions: String,
    #[serde(default)]
    pub retry_policy: RetryPolicy,
    #[serde(default)]
    pub failure_policy: FailurePolicy,
    #[serde(default)]
    pub execution_policy: ExecutionPolicy,
}

/// Tests the result of the step that led here and records the answer (`result.matched`), which
/// the edges leaving it can route on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConditionNode {
    pub label: String,
    pub condition: Condition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EndOutcome {
    #[default]
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndNode {
    pub label: String,
    #[serde(default)]
    pub outcome: EndOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeKind {
    Agent(AgentNode),
    Condition(ConditionNode),
    End(EndNode),
}

/// A unit of work in the graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowNode {
    pub id: String,
    #[serde(flatten)]
    pub kind: NodeKind,
    /// Higher goes first when several nodes are ready at once.
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub loop_policy: Option<LoopPolicy>,
    #[serde(default)]
    pub position: Option<Position>,
}

impl WorkflowNode {
    pub fn label(&self) -> &str {
        match &self.kind {
            NodeKind::Agent(node) => &node.label,
            NodeKind::Condition(node) => &node.label,
            NodeKind::End(node) => &node.label,
        }
    }

    pub fn agent(&self) -> Option<&AgentNode> {
        match &self.kind {
            NodeKind::Agent(node) => Some(node),
            _ => None,
        }
    }

    pub fn agent_id(&self) -> Option<&str> {
        self.agent().map(|node| node.agent_id.as_str())
    }

    pub fn failure_route(&self) -> Option<&str> {
        match self.agent().map(|node| &node.failure_policy) {
            Some(FailurePolicy::RouteToNode { node_id }) => Some(node_id.as_str()),
            _ => None,
        }
    }
}
