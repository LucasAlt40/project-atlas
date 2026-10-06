//! The ports between the orchestrator and whatever really runs a step. The orchestrator knows
//! nothing about runtimes, terminals or Git: it asks a [`StepRunner`] to prepare a step, runs it,
//! and hears how it went. The real runner (`chat_runner`) sits on `ChatService` and so on
//! `ExecutionService`, the worktrees and the permission guard; tests use a scripted one.

use crate::application::chat::ChatObserver;
use crate::application::worktree::StepDelta;
use crate::domain::execution::WorkflowLink;
use crate::domain::guardrail::ReviewAnswer;
use crate::domain::interaction::InteractionDetection;
use crate::domain::optimization::BriefParts;
use crate::domain::workflow::WorkflowEvent;
use crate::domain::worktree::{ChangeSet, ExecutionWorktree};

/// Everything a step needs, decided by the orchestrator.
#[derive(Debug, Clone)]
pub struct StepRequest {
    pub workspace_id: String,
    pub agent_id: String,
    /// The instruction the agent is sent.
    pub instruction: String,
    /// What the Task Context is chosen for.
    pub context_query: String,
    /// The parts of the instruction that follow the node's own instructions and the task.
    pub brief_parts: BriefParts,
    /// What a person answered when a guardrail asked about this step's context, if it did.
    pub review: Option<ReviewAnswer>,
    pub display_task: String,
    pub link: WorkflowLink,
    /// The run's shared worktree, when it has one: the step works in it (if its agent is
    /// isolated) instead of in a worktree of its own.
    pub shared_worktree: Option<String>,
}

/// Why a step could not be prepared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The agent is working on something else. Not a failure: the step waits.
    Busy,
    /// The step can never run as asked (unknown agent…). A failure of the step.
    Invalid(String),
}

/// How a step uses the run's shared worktree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepAccess {
    /// It only reads what is there: steps like it may run side by side.
    Read,
    /// It may change files, so it needs the worktree to itself.
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Completed,
    /// The agent stopped to ask a person. Not a result: the step is paused, not finished.
    WaitingForInput,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepOutcome {
    pub status: StepStatus,
    /// The agent's answer, when it completed.
    pub text: String,
    pub failure: Option<String>,
    /// What the agent asked, when the step is waiting for a person.
    pub interaction: Option<InteractionDetection>,
    /// What the step changed in the run's shared worktree, measured by Git.
    pub delta: Option<StepDelta>,
}

/// The run's code workspace, asked for when the run begins.
#[derive(Debug, Clone)]
pub struct WorkspaceRequest {
    pub workspace_id: String,
    pub run_id: String,
    /// The agents of the workflow that work in an isolated worktree.
    pub agent_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceHandle {
    /// The execution id of the primary worktree; steps are given this to share it.
    pub primary_execution_id: String,
    pub branch: String,
    pub base_branch: String,
    pub base_revision: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunEnd {
    Completed,
    /// Failed or cancelled: the work is kept and never applied.
    Ended,
}

/// What became of the run's worktree when the run ended.
#[derive(Debug, Clone)]
pub struct WorkspaceClose {
    pub worktree: ExecutionWorktree,
    /// By Git; `None` when the worktree is gone (nothing to apply, so it was removed).
    pub changes: Option<ChangeSet>,
}

/// A step that has an execution reserved and an agent held for it. Running it blocks until the
/// execution ends.
pub trait PreparedStep: Send {
    fn execution_id(&self) -> &str;
    fn run(self: Box<Self>, observer: &dyn WorkflowObserver) -> StepOutcome;
}

pub trait StepRunner: Send + Sync {
    /// Reserves an execution and the agent for a step.
    ///
    /// # Errors
    ///
    /// [`Refusal::Busy`] if the agent is occupied, [`Refusal::Invalid`] if the step cannot run.
    fn prepare(&self, request: StepRequest) -> Result<Box<dyn PreparedStep>, Refusal>;

    /// How a step of this agent uses the run's shared worktree: `None` when it does not work in
    /// it. A step that may write is one whose runtime can edit files, whose policy allows it and
    /// which works in an isolated worktree: never a matter of which agent it is.
    fn shared_access(&self, _workspace_id: &str, _agent_id: &str) -> Option<StepAccess> {
        Some(StepAccess::Read)
    }

    /// Asks the step's process to stop. The step then ends as cancelled.
    fn cancel(&self, workspace_id: &str, agent_id: &str, execution_id: &str);

    /// Whether the execution is waiting for the user to approve something. Answers only from
    /// the real approval mechanism.
    fn awaiting_approval(&self, execution_id: &str) -> bool;

    /// Gives the run one worktree for all its steps, so what one step writes is what the next
    /// one reads. `None` when the run has no isolated agent or the project cannot have one.
    fn open_workspace(&self, _request: &WorkspaceRequest) -> Option<WorkspaceHandle> {
        None
    }

    /// Picks the worktree of a run that was cut short up again. `false` if it cannot be.
    fn reopen_workspace(&self, _primary_execution_id: &str) -> bool {
        false
    }

    /// Whether the worktree of a finished run is still there and still what Atlas made, so a
    /// run that failed can go on in it. Looks only; changes nothing.
    fn workspace_usable(&self, _primary_execution_id: &str) -> bool {
        false
    }

    /// The run is over: decides what can be done with its code. Never applies it.
    fn close_workspace(&self, _primary_execution_id: &str, _end: RunEnd) -> Option<WorkspaceClose> {
        None
    }
}

/// Port: what the UI hears about a workflow run. The step executions' own progress and messages
/// go through the same observer, as for any execution.
pub trait WorkflowObserver: ChatObserver {
    fn on_workflow_event(&self, event: &WorkflowEvent);
}
