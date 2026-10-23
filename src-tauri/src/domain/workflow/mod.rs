//! The declarative side of workflows: graph, nodes, edges, conditions, and the record of a run.
//! Nothing here knows how a graph is drawn or how a step is executed.

mod chain;
mod condition;
mod edge;
mod execution;
mod handoff;
mod integration;
mod node;
mod template;
#[allow(clippy::module_inception)]
mod workflow;

pub use chain::{
    ChainNote, ChainProject, ChainStep, ChainStepSummary, ProjectChain, MAX_CHAIN_DIFF_CHARS,
    MAX_CHAIN_FILES, MAX_CHAIN_STEPS, MAX_CHAIN_SUMMARY_CHARS,
};
pub use condition::{Condition, ConditionError, ConditionOperator, Facts};
pub use edge::WorkflowEdge;
pub use execution::{
    AttemptStatus, FailureCode, NodeAttempt, NodeState, NodeStatus, RecoveryKind, RecoveryPlan,
    RecoveryProblem, RecoveryRecord, SpecIntegrationRef, SpecLink, SpecTaskRef, WorkflowEvent,
    WorkflowEventKind, WorkflowExecution, WorkflowExecutionStatus, WorkflowFailure,
};
pub use handoff::{AgentHandoff, HandoffArtifact, HandoffDecision, HandoffKind, HandoffValidation};
pub use integration::{IntegrationStatus, WorkflowIntegration};
pub use node::{
    AgentNode, EndNode, EndOutcome, ExecutionPolicy, FailurePolicy, LoopPolicy, NodeKind,
    RetryPolicy, WorkflowNode,
};
pub use node::{ConditionNode, Position};
pub use template::{CustomTemplate, SuggestionTarget, ToolCapability, ToolSuggestion};
pub use workflow::{RouteRepair, Viewport, Workflow, WorkflowMode, WorkflowStatus};
