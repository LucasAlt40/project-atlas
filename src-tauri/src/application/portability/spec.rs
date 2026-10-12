//! What each kind of configuration looks like in a file, and how it becomes (and comes from) the
//! domain's own types. These are the file's own types, on purpose: a field of the stored model
//! can be renamed, split or dropped without touching what a file written last year says. The
//! mapping is where that is absorbed, and an exhaustive `match` here is what notices a new
//! variant of a domain enum.
//!
//! Nothing here holds a local id: references to other configurations are the file's `ref`s.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::format::FormatError;
use crate::application::errors::{AppError, ErrorCode};
use crate::domain::mcp::ToolSelection;
use crate::domain::result_contract::{ContractKind, Outcome, ResultContract};
use crate::domain::workflow::{
    AgentNode, Condition, ConditionNode, ConditionOperator, EndNode, EndOutcome, ExecutionPolicy,
    FailurePolicy, LoopPolicy, NodeKind, Position, RetryPolicy, SuggestionTarget, ToolCapability,
    ToolSuggestion, WorkflowEdge, WorkflowMode, WorkflowNode,
};

const MAX_NODES: usize = 200;
const MAX_EDGES: usize = 400;
const MAX_TOOLS: usize = 400;

fn yes() -> bool {
    true
}

// ---- result contracts ------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractKindSpec {
    #[default]
    General,
    Validation,
    Review,
    Implementation,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutcomeSpec {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractSpec {
    #[serde(default)]
    pub kind: ContractKindSpec,
    #[serde(default)]
    pub outcomes: Vec<OutcomeSpec>,
}

impl From<&ResultContract> for ContractSpec {
    fn from(contract: &ResultContract) -> Self {
        Self {
            kind: match contract.kind {
                ContractKind::General => ContractKindSpec::General,
                ContractKind::Validation => ContractKindSpec::Validation,
                ContractKind::Review => ContractKindSpec::Review,
                ContractKind::Implementation => ContractKindSpec::Implementation,
                ContractKind::Custom => ContractKindSpec::Custom,
            },
            outcomes: contract
                .outcomes
                .iter()
                .map(|o| OutcomeSpec {
                    id: o.id.clone(),
                    label: o.label.clone(),
                    description: o.description.clone(),
                })
                .collect(),
        }
    }
}

impl ContractSpec {
    /// The contract, held to the same rules as one typed into the editor.
    ///
    /// # Errors
    ///
    /// Fails with `ResultContractInvalid` and the reason, like the editor does.
    pub fn to_domain(&self) -> Result<ResultContract, AppError> {
        let contract = ResultContract {
            kind: match self.kind {
                ContractKindSpec::General => ContractKind::General,
                ContractKindSpec::Validation => ContractKind::Validation,
                ContractKindSpec::Review => ContractKind::Review,
                ContractKindSpec::Implementation => ContractKind::Implementation,
                ContractKindSpec::Custom => ContractKind::Custom,
            },
            outcomes: self
                .outcomes
                .iter()
                .map(|o| Outcome {
                    id: o.id.clone(),
                    label: o.label.clone(),
                    description: o.description.clone(),
                })
                .collect(),
        };
        crate::application::agents::AgentService::checked_contract(contract)
    }
}

// ---- personality -----------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalitySpec {
    /// The system instructions: how the agent behaves.
    pub instructions: String,
    #[serde(default)]
    pub behavior: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub suggested_contract: ContractSpec,
    #[serde(default = "developer")]
    pub suggested_permission_profile: String,
    /// Set when the personality is one of those that ship with Atlas (possibly edited): its id
    /// means the same on every installation.
    #[serde(default)]
    pub builtin_id: Option<String>,
}

fn developer() -> String {
    "developer".to_owned()
}

// ---- agent -----------------------------------------------------------------------------------

/// The runtime an agent used when it was exported, with the names a person recognises it by.
/// What the destination does with it is decided there, by asking its own runtimes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRef {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRef {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpec {
    /// The `ref` of the personality.
    pub personality: String,
    pub runtime: RuntimeRef,
    pub model: ModelRef,
    pub instructions: String,
    #[serde(default)]
    pub permission_profile: Option<String>,
    #[serde(default = "yes")]
    pub worktree_isolation: bool,
    #[serde(default)]
    pub result_contract: ContractSpec,
}

// ---- workflow graphs -------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModeSpec {
    Automatic,
    #[default]
    Custom,
}

impl From<WorkflowMode> for ModeSpec {
    fn from(mode: WorkflowMode) -> Self {
        match mode {
            WorkflowMode::Automatic => Self::Automatic,
            WorkflowMode::Custom => Self::Custom,
        }
    }
}

impl From<ModeSpec> for WorkflowMode {
    fn from(mode: ModeSpec) -> Self {
        match mode {
            ModeSpec::Automatic => Self::Automatic,
            ModeSpec::Custom => Self::Custom,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorSpec {
    Equals,
    NotEquals,
    Exists,
    NotExists,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConditionSpec {
    pub field: String,
    pub operator: OperatorSpec,
    #[serde(default)]
    pub value: Option<String>,
}

impl From<&Condition> for ConditionSpec {
    fn from(condition: &Condition) -> Self {
        Self {
            field: condition.field.clone(),
            operator: match condition.operator {
                ConditionOperator::Equals => OperatorSpec::Equals,
                ConditionOperator::NotEquals => OperatorSpec::NotEquals,
                ConditionOperator::Exists => OperatorSpec::Exists,
                ConditionOperator::NotExists => OperatorSpec::NotExists,
            },
            value: condition.value.clone(),
        }
    }
}

impl From<&ConditionSpec> for Condition {
    fn from(spec: &ConditionSpec) -> Self {
        Self {
            field: spec.field.clone(),
            operator: match spec.operator {
                OperatorSpec::Equals => ConditionOperator::Equals,
                OperatorSpec::NotEquals => ConditionOperator::NotEquals,
                OperatorSpec::Exists => ConditionOperator::Exists,
                OperatorSpec::NotExists => ConditionOperator::NotExists,
            },
            value: spec.value.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FailureSpec {
    #[default]
    Stop,
    /// The workflow carries on at this step of the same file.
    RouteToNode { node: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStepSpec {
    /// The `ref` of the agent that runs the step; none when the agent it had no longer exists.
    #[serde(default)]
    pub agent: Option<String>,
    pub label: String,
    #[serde(default)]
    pub instructions: String,
    #[serde(default)]
    pub completion_criteria: String,
    #[serde(default)]
    pub retries: u32,
    #[serde(default)]
    pub on_failure: FailureSpec,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default = "yes")]
    pub announce_integrations: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConditionStepSpec {
    pub label: String,
    pub condition: ConditionSpec,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndOutcomeSpec {
    #[default]
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndStepSpec {
    pub label: String,
    #[serde(default)]
    pub outcome: EndOutcomeSpec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StepSpec {
    Agent(AgentStepSpec),
    Condition(ConditionStepSpec),
    End(EndStepSpec),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopSpec {
    pub id: String,
    pub max_iterations: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PositionSpec {
    pub x: f64,
    pub y: f64,
}

/// A step. Its `id` only has to be unique in the workflow: edges and failure routes name it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeSpec {
    pub id: String,
    #[serde(flatten)]
    pub step: StepSpec,
    #[serde(default)]
    pub priority: i32,
    #[serde(default, rename = "loop")]
    pub loop_limit: Option<LoopSpec>,
    #[serde(default)]
    pub position: Option<PositionSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeSpec {
    pub id: String,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub condition: Option<ConditionSpec>,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowSpec {
    #[serde(default)]
    pub mode: ModeSpec,
    pub nodes: Vec<NodeSpec>,
    #[serde(default)]
    pub edges: Vec<EdgeSpec>,
}

// ---- saved templates -------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitySpec {
    Browser,
    Issues,
    Docs,
    Repo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolsSpec {
    Server,
    Only { tools: Vec<String> },
}

/// What a template suggests a step be given. A suggestion only: it names a kind of tool or a
/// connection by name, never one of the destination's connections, and grants nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolTargetSpec {
    Capability { capability: CapabilitySpec },
    Connection { name: String, tools: ToolsSpec },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub node: String,
    #[serde(flatten)]
    pub target: ToolTargetSpec,
    #[serde(default)]
    pub purpose: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateSpec {
    /// The built-in template it started from (an id that ships with Atlas).
    #[serde(default)]
    pub based_on: Option<String>,
    pub nodes: Vec<NodeSpec>,
    #[serde(default)]
    pub edges: Vec<EdgeSpec>,
    #[serde(default)]
    pub tools: Vec<ToolSpec>,
}

impl From<&ToolSuggestion> for ToolSpec {
    fn from(suggestion: &ToolSuggestion) -> Self {
        Self {
            node: suggestion.node_id.clone(),
            target: match &suggestion.target {
                SuggestionTarget::Capability { capability } => ToolTargetSpec::Capability {
                    capability: match capability {
                        ToolCapability::Browser => CapabilitySpec::Browser,
                        ToolCapability::Issues => CapabilitySpec::Issues,
                        ToolCapability::Docs => CapabilitySpec::Docs,
                        ToolCapability::Repo => CapabilitySpec::Repo,
                    },
                },
                SuggestionTarget::Connection { name, tools } => ToolTargetSpec::Connection {
                    name: name.clone(),
                    tools: match tools {
                        ToolSelection::Server => ToolsSpec::Server,
                        ToolSelection::Only { tools } => ToolsSpec::Only {
                            tools: tools.clone(),
                        },
                    },
                },
            },
            purpose: suggestion.purpose.clone(),
        }
    }
}

impl From<&ToolSpec> for ToolSuggestion {
    fn from(spec: &ToolSpec) -> Self {
        Self {
            node_id: spec.node.clone(),
            target: match &spec.target {
                ToolTargetSpec::Capability { capability } => SuggestionTarget::Capability {
                    capability: match capability {
                        CapabilitySpec::Browser => ToolCapability::Browser,
                        CapabilitySpec::Issues => ToolCapability::Issues,
                        CapabilitySpec::Docs => ToolCapability::Docs,
                        CapabilitySpec::Repo => ToolCapability::Repo,
                    },
                },
                ToolTargetSpec::Connection { name, tools } => SuggestionTarget::Connection {
                    name: name.clone(),
                    tools: match tools {
                        ToolsSpec::Server => ToolSelection::Server,
                        ToolsSpec::Only { tools } => ToolSelection::Only {
                            tools: tools.clone(),
                        },
                    },
                },
            },
            purpose: spec.purpose.clone(),
        }
    }
}

// ---- the graph, both ways --------------------------------------------------------------------

/// Writes a graph in the file's words. `agent_ref` names the agent behind a local agent id (and
/// is where the exporter decides whether it is included or only declared); `None` for an agent
/// that is gone.
pub fn graph_to_spec(
    nodes: &[WorkflowNode],
    edges: &[WorkflowEdge],
    agent_ref: &mut dyn FnMut(&str) -> Option<String>,
) -> (Vec<NodeSpec>, Vec<EdgeSpec>) {
    let nodes = nodes
        .iter()
        .map(|node| NodeSpec {
            id: node.id.clone(),
            step: match &node.kind {
                NodeKind::Agent(step) => StepSpec::Agent(AgentStepSpec {
                    agent: agent_ref(&step.agent_id),
                    label: step.label.clone(),
                    instructions: step.instructions.clone(),
                    completion_criteria: step.completion_criteria.clone(),
                    retries: step.retry_policy.max_retries,
                    on_failure: match &step.failure_policy {
                        FailurePolicy::StopWorkflow => FailureSpec::Stop,
                        FailurePolicy::RouteToNode { node_id } => FailureSpec::RouteToNode {
                            node: node_id.clone(),
                        },
                    },
                    read_only: step.execution_policy.read_only,
                    announce_integrations: step.execution_policy.announce_integrations,
                }),
                NodeKind::Condition(step) => StepSpec::Condition(ConditionStepSpec {
                    label: step.label.clone(),
                    condition: (&step.condition).into(),
                }),
                NodeKind::End(step) => StepSpec::End(EndStepSpec {
                    label: step.label.clone(),
                    outcome: match step.outcome {
                        EndOutcome::Done => EndOutcomeSpec::Done,
                        EndOutcome::Failed => EndOutcomeSpec::Failed,
                        EndOutcome::Cancelled => EndOutcomeSpec::Cancelled,
                    },
                }),
            },
            priority: node.priority,
            loop_limit: node.loop_policy.as_ref().map(|l| LoopSpec {
                id: l.loop_id.clone(),
                max_iterations: l.max_iterations,
            }),
            position: node.position.map(|p| PositionSpec { x: p.x, y: p.y }),
        })
        .collect();
    let edges = edges
        .iter()
        .map(|edge| EdgeSpec {
            id: edge.id.clone(),
            from: edge.source_node_id.clone(),
            to: edge.target_node_id.clone(),
            condition: edge.condition.as_ref().map(Into::into),
            label: edge.label.clone(),
        })
        .collect();
    (nodes, edges)
}

/// Builds the domain graph. `agent_id` turns a `ref` into the id the destination uses.
pub fn graph_to_domain(
    nodes: &[NodeSpec],
    edges: &[EdgeSpec],
    agent_id: &dyn Fn(&str) -> String,
) -> (Vec<WorkflowNode>, Vec<WorkflowEdge>) {
    let nodes = nodes
        .iter()
        .map(|node| WorkflowNode {
            id: node.id.clone(),
            kind: match &node.step {
                StepSpec::Agent(step) => NodeKind::Agent(AgentNode {
                    agent_id: step.agent.as_deref().map(agent_id).unwrap_or_default(),
                    label: step.label.clone(),
                    instructions: step.instructions.clone(),
                    completion_criteria: step.completion_criteria.clone(),
                    retry_policy: RetryPolicy {
                        max_retries: step.retries,
                    },
                    failure_policy: match &step.on_failure {
                        FailureSpec::Stop => FailurePolicy::StopWorkflow,
                        FailureSpec::RouteToNode { node } => FailurePolicy::RouteToNode {
                            node_id: node.clone(),
                        },
                    },
                    execution_policy: ExecutionPolicy {
                        read_only: step.read_only,
                        announce_integrations: step.announce_integrations,
                        ..ExecutionPolicy::default()
                    },
                }),
                StepSpec::Condition(step) => NodeKind::Condition(ConditionNode {
                    label: step.label.clone(),
                    condition: (&step.condition).into(),
                }),
                StepSpec::End(step) => NodeKind::End(EndNode {
                    label: step.label.clone(),
                    outcome: match step.outcome {
                        EndOutcomeSpec::Done => EndOutcome::Done,
                        EndOutcomeSpec::Failed => EndOutcome::Failed,
                        EndOutcomeSpec::Cancelled => EndOutcome::Cancelled,
                    },
                }),
            },
            priority: node.priority,
            loop_policy: node.loop_limit.as_ref().map(|l| LoopPolicy {
                loop_id: l.id.clone(),
                max_iterations: l.max_iterations,
            }),
            position: node.position.map(|p| Position { x: p.x, y: p.y }),
        })
        .collect();
    let edges = edges
        .iter()
        .map(|edge| WorkflowEdge {
            id: edge.id.clone(),
            source_node_id: edge.from.clone(),
            target_node_id: edge.to.clone(),
            condition: edge.condition.as_ref().map(Into::into),
            label: edge.label.clone(),
        })
        .collect();
    (nodes, edges)
}

/// What a graph must be for Atlas to hold it at all (as opposed to hold it as a draft that
/// cannot run yet): steps and routes that name each other consistently. Returns the `ref`s of
/// the agents the steps use.
///
/// # Errors
///
/// Fails when a step or route cannot even be placed in a graph.
pub fn check_graph(
    nodes: &[NodeSpec],
    edges: &[EdgeSpec],
    at: &str,
) -> Result<Vec<String>, FormatError> {
    if nodes.len() > MAX_NODES || edges.len() > MAX_EDGES {
        return Err(FormatError::invalid("too_large_graph", at));
    }
    let mut ids = BTreeSet::new();
    for node in nodes {
        if node.id.trim().is_empty() || !ids.insert(node.id.as_str()) {
            return Err(FormatError::invalid("duplicate_node", at));
        }
    }
    let mut edge_ids = BTreeSet::new();
    for edge in edges {
        if edge.id.trim().is_empty() || !edge_ids.insert(edge.id.as_str()) {
            return Err(FormatError::invalid("duplicate_edge", at));
        }
        if !ids.contains(edge.from.as_str()) || !ids.contains(edge.to.as_str()) {
            return Err(FormatError::invalid("edge_unknown_node", at));
        }
    }
    let mut agents: Vec<String> = Vec::new();
    for node in nodes {
        if let StepSpec::Agent(step) = &node.step {
            if let FailureSpec::RouteToNode { node } = &step.on_failure {
                if !ids.contains(node.as_str()) {
                    return Err(FormatError::invalid("failure_route_unknown_node", at));
                }
            }
            if let Some(agent) = &step.agent {
                if !agents.contains(agent) {
                    agents.push(agent.clone());
                }
            }
        }
    }
    Ok(agents)
}

/// The steps a template's tool suggestions name must exist.
///
/// # Errors
///
/// Fails when a suggestion names a step the template does not have.
pub fn check_tools(tools: &[ToolSpec], nodes: &[NodeSpec], at: &str) -> Result<(), FormatError> {
    if tools.len() > MAX_TOOLS {
        return Err(FormatError::invalid("too_many_tools", at));
    }
    if tools
        .iter()
        .any(|tool| !nodes.iter().any(|node| node.id == tool.node))
    {
        return Err(FormatError::invalid("tool_unknown_node", at));
    }
    Ok(())
}

/// A decoding failure of an item's `spec`, in the words of the format.
pub fn spec_error(error: &serde_json::Error, at: &str) -> FormatError {
    FormatError::invalid("spec", format!("{at}.spec: {error}"))
}

/// The reason code the preview and the report give for a domain rule an item breaks.
pub fn reason_of(error: &AppError) -> &'static str {
    match error.code {
        ErrorCode::NameRequired => "name_required",
        ErrorCode::NameTooLong => "name_too_long",
        ErrorCode::InstructionsRequired => "instructions_required",
        ErrorCode::RuntimeRequired => "runtime_unknown",
        ErrorCode::RuntimeNotSupported => "runtime_unsupported",
        ErrorCode::ModelRequired => "model_required",
        ErrorCode::ModelInvalid => "model_invalid",
        ErrorCode::PermissionProfileInvalid => "permission_profile_invalid",
        ErrorCode::ResultContractInvalid => "contract_invalid",
        _ => "invalid",
    }
}
