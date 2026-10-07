//! The declarative side of workflows: graph, nodes, edges, conditions, and the record of a run.
//! Nothing here knows how a graph is drawn or how a step is executed.

mod condition;
mod edge;
mod execution;
mod handoff;
mod integration;
mod node;
#[allow(clippy::module_inception)]
mod workflow;

pub use condition::{Condition, ConditionError, ConditionOperator, Facts};
pub use edge::WorkflowEdge;
pub use execution::{
    AttemptStatus, FailureCode, NodeAttempt, NodeState, NodeStatus, RecoveryKind, RecoveryPlan,
    RecoveryProblem, RecoveryRecord, WorkflowEvent, WorkflowEventKind, WorkflowExecution,
    WorkflowExecutionStatus, WorkflowFailure,
};
pub use handoff::{AgentHandoff, HandoffArtifact, HandoffDecision, HandoffKind, HandoffValidation};
pub use integration::{IntegrationStatus, WorkflowIntegration};
pub use node::{
    AgentNode, EndNode, EndOutcome, ExecutionPolicy, FailurePolicy, LoopPolicy, NodeKind,
    RetryPolicy, WorkflowNode,
};
#[cfg(test)]
pub use node::{ConditionNode, Position};
pub use workflow::{RouteRepair, Viewport, Workflow, WorkflowMode, WorkflowStatus};
