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

// The vocabulary of the module: not every name is used outside it yet.
#[allow(unused_imports)]
pub use condition::{Condition, ConditionError, ConditionOperator, Facts, KNOWN_FIELDS};
pub use edge::WorkflowEdge;
#[allow(unused_imports)]
pub use execution::{
    AttemptStatus, FailureCode, NodeAttempt, NodeState, NodeStatus, WorkflowEvent,
    WorkflowEventKind, WorkflowExecution, WorkflowExecutionStatus, WorkflowFailure,
    DEFAULT_MAX_PARALLEL_STEPS,
};
#[allow(unused_imports)]
pub use handoff::{AgentHandoff, HandoffArtifact, HandoffDecision, HandoffKind, HandoffValidation};
#[allow(unused_imports)]
pub use integration::{IntegrationStatus, WorkflowIntegration};
#[allow(unused_imports)]
pub use node::{
    AgentNode, ConditionNode, EndNode, EndOutcome, ExecutionPolicy, FailurePolicy, IsolationMode,
    LoopPolicy, NodeKind, Position, RetryPolicy, WorkflowNode,
};
pub use workflow::{Viewport, Workflow, WorkflowMode, WorkflowStatus};
