use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::handoff::AgentHandoff;
use super::integration::WorkflowIntegration;
use super::workflow::Workflow;
use crate::domain::interaction::PendingInteraction;
use crate::domain::orchestration::SharedExecutionState;
use crate::domain::worktree::ChangeSet;

/// Steps that may run at once in one workflow run.
pub const DEFAULT_MAX_PARALLEL_STEPS: u32 = 4;
/// Events kept on a run; the oldest go first.
pub const MAX_EVENTS: usize = 400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowExecutionStatus {
    Running,
    /// No new step starts. A step already running is left to finish.
    Paused,
    /// A step asked a person something and the run stands still until it is answered. Steps
    /// already running are left to finish; nothing new starts.
    WaitingForInput,
    Completed,
    Failed,
    Cancelled,
    /// The app closed while the run was going. Nothing is assumed about how it would have ended:
    /// the user resumes, restarts or cancels it.
    Interrupted,
}

impl WorkflowExecutionStatus {
    pub fn is_final(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    /// Whether the run still holds on to its steps (running, or paused with steps in flight).
    pub fn is_active(self) -> bool {
        matches!(self, Self::Running | Self::Paused | Self::WaitingForInput)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    Pending,
    Ready,
    Running,
    WaitingApproval,
    /// The agent asked a person something. The step has not finished and has no outcome; it
    /// still holds its place (and its lock on the shared worktree) until the answer comes.
    WaitingForInput,
    Completed,
    Failed,
    /// A dependency can no longer be satisfied.
    Blocked,
    Cancelled,
    /// A condition decided it does not run.
    Skipped,
}

impl NodeStatus {
    pub fn is_in_flight(self) -> bool {
        matches!(
            self,
            Self::Running | Self::WaitingApproval | Self::WaitingForInput
        )
    }

    /// Will not change again unless a loop sends the workflow back through it.
    pub fn is_settled(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Blocked | Self::Cancelled | Self::Skipped
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
    /// The attempt ended by asking a person. It is neither a success nor a failure; the answer
    /// starts the next attempt.
    WaitingForInput,
}

/// One execution of a node: its own execution id, so retries and loop iterations can be told
/// apart and each can be opened in the Execution Inspector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeAttempt {
    /// 1-based, within the run of the node (counting every attempt).
    pub attempt: u32,
    /// Which pass through the node this belongs to (loops start a new one).
    pub iteration: u32,
    pub execution_id: String,
    pub status: AttemptStatus,
    pub started_at: u64,
    #[serde(default)]
    pub completed_at: Option<u64>,
    #[serde(default)]
    pub summary: Option<String>,
    /// The outcome this attempt's result declared (`pass`, `fail`…), if its agent's contract
    /// asked for one. Belongs to this attempt alone: a retry has its own.
    #[serde(default)]
    pub outcome: Option<String>,
    #[serde(default)]
    pub failure: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeState {
    pub status: NodeStatus,
    /// Edges (and failure routes) taken towards this node since it last started.
    #[serde(default)]
    pub activated_edges: BTreeSet<String>,
    /// How many times the node has started a pass (loop iterations).
    #[serde(default)]
    pub iterations: u32,
    /// Failed attempts in the current pass; interruptions do not count.
    #[serde(default)]
    pub failed_attempts: u32,
    #[serde(default)]
    pub attempts: Vec<NodeAttempt>,
    /// The facts the node's last result gave conditions to test.
    #[serde(default)]
    pub facts: BTreeMap<String, String>,
    /// Why the node is blocked, skipped or failed (a stable code).
    #[serde(default)]
    pub reason: Option<String>,
    /// The node's failure was handed to a failure route.
    #[serde(default)]
    pub routed: bool,
    /// The node completed but none of its edges matched its result.
    #[serde(default)]
    pub unrouted: bool,
    /// The next start continues the current pass (a retry, or a step resumed after an
    /// interruption) instead of beginning a new iteration.
    #[serde(default)]
    pub continuing: bool,
}

impl NodeState {
    pub fn pending() -> Self {
        Self {
            status: NodeStatus::Pending,
            activated_edges: BTreeSet::new(),
            iterations: 0,
            failed_attempts: 0,
            attempts: Vec::new(),
            facts: BTreeMap::new(),
            reason: None,
            routed: false,
            unrouted: false,
            continuing: false,
        }
    }

    pub fn last_attempt(&self) -> Option<&NodeAttempt> {
        self.attempts.last()
    }
}

/// Why a run ended badly. `code` is stable; the UI words it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowFailure {
    pub code: FailureCode,
    pub node_id: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCode {
    /// A loop would have started a node more often than its limit.
    MaxIterationsReached,
    /// A step failed and nothing handles it.
    NodeFailed,
    /// The run went as far as it could without reaching an end that means success.
    NoPathToCompletion,
    /// A step finished, but none of its edges matched what it reported.
    NoRouteMatched,
    /// An End node that means failure was reached.
    EndedInFailure,
    /// The definition did not pass validation when the run was about to start.
    InvalidWorkflow,
    InternalError,
    /// The run could not pick its shared worktree up again.
    WorktreeUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowEventKind {
    Started,
    Paused,
    Resumed,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
    NodeReady,
    NodeStarted,
    NodeWaitingApproval,
    /// The user answered (or the request lapsed): the step is running again.
    NodeApprovalResolved,
    /// The step asked a person something and waits (`execution.paused`).
    NodeWaitingForInput,
    /// The question was answered: the step starts again (`execution.resumed`).
    NodeInputResolved,
    /// A step asked a person (`interaction.detected`); `metadata` has `interactionId`, `kind`,
    /// `source`.
    InteractionDetected,
    /// The person answered (`interaction.answered`).
    InteractionAnswered,
    /// An answer was refused: not pending, not an option… (`interaction.rejected`).
    InteractionRejected,
    /// The question lapsed because the run was cancelled (`interaction.cancelled`).
    InteractionCancelled,
    NodeCompleted,
    NodeFailed,
    NodeBlocked,
    NodeSkipped,
    NodeRetrying,
    ArtifactCreated,
    DecisionCreated,
    OverlapDetected,
    /// A step handed its result to the next one.
    HandoffCreated,
    /// Where the run's code stands changed (changes available, applied, kept…).
    IntegrationChanged,
}

impl WorkflowEventKind {
    /// The name of the event on the wire: `workflow:node_started`.
    pub fn wire_name(self) -> String {
        let kind = serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        format!("workflow:{kind}")
    }
}

/// Something that happened in a run. Carries ids, not state: a listener reads the run again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowEvent {
    pub kind: WorkflowEventKind,
    pub workflow_id: String,
    pub execution_id: String,
    pub workspace_id: String,
    #[serde(default)]
    pub node_id: Option<String>,
    pub message: String,
    pub timestamp: u64,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

/// One run of a workflow. Definition and run are kept apart: the run holds the snapshot of the
/// definition it started from, so editing the definition never changes a run in progress.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowExecution {
    pub id: String,
    pub workflow_id: String,
    pub workspace_id: String,
    pub workflow_version: u32,
    pub workflow: Workflow,
    pub task: String,
    pub status: WorkflowExecutionStatus,
    #[serde(default)]
    pub failure: Option<WorkflowFailure>,
    /// A cancel was asked for: pending nodes are cancelled and the run ends when the steps in
    /// flight have.
    #[serde(default)]
    pub cancel_requested: bool,
    pub max_parallel_steps: u32,
    pub nodes: BTreeMap<String, NodeState>,
    pub state: SharedExecutionState,
    /// What each step handed to the next, in order. Kept with the run.
    #[serde(default)]
    pub handoffs: Vec<AgentHandoff>,
    /// What the run changed in the code, by Git (not by what the agents said), once it ended.
    #[serde(default)]
    pub changes: Option<ChangeSet>,
    /// Where the code stands. A run can be `completed` with its code not integrated.
    #[serde(default)]
    pub integration: WorkflowIntegration,
    /// Every question the run's steps asked a person, answered or not: the audit trail.
    #[serde(default)]
    pub interactions: Vec<PendingInteraction>,
    #[serde(default)]
    pub events: Vec<WorkflowEvent>,
    pub started_at: u64,
    pub updated_at: u64,
    #[serde(default)]
    pub completed_at: Option<u64>,
}

impl WorkflowExecution {
    pub fn new(id: String, workflow: Workflow, task: String, at: u64) -> Self {
        let nodes = workflow
            .nodes
            .iter()
            .map(|node| (node.id.clone(), NodeState::pending()))
            .collect();
        let state = SharedExecutionState {
            task: task.clone(),
            workflow: workflow.name.clone(),
            ..SharedExecutionState::default()
        };
        Self {
            id,
            workflow_id: workflow.id.clone(),
            workspace_id: workflow.workspace_id.clone(),
            workflow_version: workflow.version,
            workflow,
            task,
            status: WorkflowExecutionStatus::Running,
            failure: None,
            cancel_requested: false,
            max_parallel_steps: DEFAULT_MAX_PARALLEL_STEPS,
            nodes,
            state,
            handoffs: Vec::new(),
            changes: None,
            integration: WorkflowIntegration::default(),
            interactions: Vec::new(),
            events: Vec::new(),
            started_at: at,
            updated_at: at,
            completed_at: None,
        }
    }

    /// The question a step is waiting on, if it is.
    pub fn pending_interaction(&self, node_id: &str) -> Option<&PendingInteraction> {
        self.interactions
            .iter()
            .find(|i| i.step_id == node_id && i.is_pending())
    }

    pub fn node_state(&self, id: &str) -> Option<&NodeState> {
        self.nodes.get(id)
    }

    /// Records an event on the run (keeping the newest [`MAX_EVENTS`]) and returns it.
    pub fn record(
        &mut self,
        kind: WorkflowEventKind,
        node_id: Option<&str>,
        message: impl Into<String>,
        metadata: BTreeMap<String, String>,
        at: u64,
    ) -> WorkflowEvent {
        let event = WorkflowEvent {
            kind,
            workflow_id: self.workflow_id.clone(),
            execution_id: self.id.clone(),
            workspace_id: self.workspace_id.clone(),
            node_id: node_id.map(str::to_owned),
            message: message.into(),
            timestamp: at,
            metadata,
        };
        self.events.push(event.clone());
        if self.events.len() > MAX_EVENTS {
            let excess = self.events.len() - MAX_EVENTS;
            self.events.drain(..excess);
        }
        self.updated_at = at;
        event
    }

    pub fn count(&self, status: NodeStatus) -> usize {
        self.nodes.values().filter(|n| n.status == status).count()
    }

    pub fn in_flight(&self) -> usize {
        self.nodes
            .values()
            .filter(|n| n.status.is_in_flight())
            .count()
    }
}
