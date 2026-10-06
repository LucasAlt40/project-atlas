//! The real [`StepRunner`]: a workflow step is an execution like any other. It goes through
//! `ChatService` (which holds the agent, records the execution, its usage and its answer) and so
//! through `ExecutionService`: the agent's own worktree policy, the task context, the runtime
//! registry and the permission guard. Nothing here starts a process, reads a file or approves
//! anything.

use std::sync::Arc;

use super::runner::{
    PreparedStep, Refusal, RunEnd, StepAccess, StepOutcome, StepRequest, StepRunner, StepStatus,
    WorkflowObserver, WorkspaceClose, WorkspaceHandle, WorkspaceRequest,
};
use crate::application::chat::{ChatError, ChatService, PendingRun, WorkflowStepRequest};
use crate::application::executions::ExecutionService;
use crate::application::live_workspace::LiveWorkspaceService;
use crate::application::security::ApprovalBroker;
use crate::application::sessions::{SessionRegistry, SessionTarget};
use crate::application::workspace::WorkspaceService;
use crate::application::worktree::{RunOutcome, WorktreeService};
use crate::domain::execution::ExecutionStatus;
use crate::domain::live_workspace::LivePhase;
use crate::domain::worktree::Validation;

pub struct ChatStepRunner {
    chat: ChatService,
    sessions: Arc<SessionRegistry>,
    approvals: Arc<ApprovalBroker>,
    /// Where a run's shared worktree is made. Without it steps work as they always did.
    worktrees: Option<Arc<WorktreeService>>,
    workspaces: Option<Arc<WorkspaceService>>,
    executions: Option<Arc<ExecutionService>>,
    /// Told what the run is doing, so its worktree is observed while it works.
    live: Option<Arc<LiveWorkspaceService>>,
}

impl ChatStepRunner {
    pub fn new(
        chat: ChatService,
        sessions: Arc<SessionRegistry>,
        approvals: Arc<ApprovalBroker>,
    ) -> Self {
        Self {
            chat,
            sessions,
            approvals,
            worktrees: None,
            workspaces: None,
            executions: None,
            live: None,
        }
    }

    /// Observes the run's worktree while its steps work in it.
    #[must_use]
    pub fn with_live(mut self, live: Arc<LiveWorkspaceService>) -> Self {
        self.live = Some(live);
        self
    }

    /// Gives runs a worktree of their own, shared by their steps.
    #[must_use]
    pub fn with_shared_worktrees(
        mut self,
        worktrees: Arc<WorktreeService>,
        workspaces: Arc<WorkspaceService>,
        executions: Arc<ExecutionService>,
    ) -> Self {
        self.worktrees = Some(worktrees);
        self.workspaces = Some(workspaces);
        self.executions = Some(executions);
        self
    }
}

struct Prepared {
    execution_id: String,
    run: PendingRun,
    worktrees: Option<Arc<WorktreeService>>,
    live: Option<Arc<LiveWorkspaceService>>,
    /// The run's shared worktree this step works in, when it does.
    shared_worktree: Option<String>,
}

impl PreparedStep for Prepared {
    fn execution_id(&self) -> &str {
        &self.execution_id
    }

    fn run(self: Box<Self>, observer: &dyn WorkflowObserver) -> StepOutcome {
        let tell_live = |phase: LivePhase| {
            if let (Some(live), Some(primary)) = (&self.live, &self.shared_worktree) {
                live.set_phase(primary, phase);
            }
        };
        tell_live(LivePhase::Running);
        let summary = self.run.run(observer);
        // Whatever way it ended, the agent is no longer writing: the phase says so, and the
        // observer reads the whole worktree (what is there is what the step left).
        tell_live(match summary.status {
            ExecutionStatus::WaitingForInput => LivePhase::WaitingForInput,
            ExecutionStatus::Cancelled => LivePhase::Cancelled,
            _ => LivePhase::Idle,
        });
        // What the step changed, by Git, once its work has been saved in the shared worktree.
        let delta = self
            .worktrees
            .as_ref()
            .and_then(|worktrees| worktrees.step_delta(&self.execution_id));
        match summary.status {
            ExecutionStatus::Completed => StepOutcome {
                status: StepStatus::Completed,
                text: summary.result.unwrap_or_default(),
                failure: None,
                interaction: None,
                delta,
            },
            ExecutionStatus::WaitingForInput => StepOutcome {
                status: StepStatus::WaitingForInput,
                text: summary.result.unwrap_or_default(),
                failure: None,
                interaction: summary.interaction,
                delta,
            },
            ExecutionStatus::Cancelled => StepOutcome {
                status: StepStatus::Cancelled,
                text: String::new(),
                failure: None,
                interaction: None,
                delta,
            },
            ExecutionStatus::Failed | ExecutionStatus::Running => StepOutcome {
                status: StepStatus::Failed,
                text: String::new(),
                failure: summary.failure,
                interaction: None,
                delta,
            },
        }
    }
}

impl StepRunner for ChatStepRunner {
    fn prepare(&self, request: StepRequest) -> Result<Box<dyn PreparedStep>, Refusal> {
        let step = WorkflowStepRequest {
            workspace_id: request.workspace_id,
            agent_id: request.agent_id,
            display_task: request.display_task,
            instruction: request.instruction,
            context_query: request.context_query,
            brief_parts: request.brief_parts,
            review: request.review,
            link: request.link,
            shared_worktree: request.shared_worktree.clone(),
        };
        match self.chat.send_workflow_step(step) {
            Ok((sent, run)) => Ok(Box::new(Prepared {
                execution_id: sent.execution_id,
                run,
                worktrees: self.worktrees.clone(),
                live: self.live.clone(),
                shared_worktree: request.shared_worktree,
            })),
            Err(ChatError::AgentBusy(_)) => Err(Refusal::Busy),
            Err(error) => Err(Refusal::Invalid(error.to_string())),
        }
    }

    fn shared_access(&self, workspace_id: &str, agent_id: &str) -> Option<StepAccess> {
        let Some(executions) = &self.executions else {
            return Some(StepAccess::Read);
        };
        if !executions.is_isolated(agent_id) {
            return None;
        }
        Some(if executions.would_write(workspace_id, agent_id) {
            StepAccess::Write
        } else {
            StepAccess::Read
        })
    }

    fn cancel(&self, workspace_id: &str, agent_id: &str, execution_id: &str) {
        // The same control the user has in the terminal: the process is ended, and the
        // execution records that the user (here, through the workflow) stopped it. A step with
        // no process yet simply finishes on its own.
        let _ = self.sessions.terminate(&SessionTarget {
            execution_id: execution_id.to_owned(),
            workspace_id: workspace_id.to_owned(),
            agent_id: agent_id.to_owned(),
        });
    }

    fn awaiting_approval(&self, execution_id: &str) -> bool {
        self.approvals
            .pending(None)
            .iter()
            .any(|approval| approval.execution_id == execution_id)
    }

    fn open_workspace(&self, request: &WorkspaceRequest) -> Option<WorkspaceHandle> {
        let (worktrees, workspaces, executions) = (
            self.worktrees.as_ref()?,
            self.workspaces.as_ref()?,
            self.executions.as_ref()?,
        );
        let project = workspaces.get(&request.workspace_id)?.project_path;
        // The agent whose policy governs the run's code is the most restricted of its agents:
        // applying the changes asks no more of anyone than the strictest of them allows.
        let agent_id = worktrees.strictest_git_agent(&request.workspace_id, &request.agent_ids)?;
        let id = executions.next_execution_id();
        let prepared = worktrees
            .prepare(
                &request.workspace_id,
                &agent_id,
                &id,
                std::path::Path::new(&project),
            )
            .ok()?;
        worktrees.mark_workflow(&id, &request.run_id).ok()?;
        // From here the worktree is observed; nothing has run in it yet.
        if let Some(live) = &self.live {
            let _ = live.start(&id);
        }
        Some(WorkspaceHandle {
            primary_execution_id: id,
            branch: prepared.worktree.branch_name,
            base_branch: prepared.worktree.base_branch,
            base_revision: prepared.worktree.base_commit,
        })
    }

    fn reopen_workspace(&self, primary_execution_id: &str) -> bool {
        let reopened = self
            .worktrees
            .as_ref()
            .is_some_and(|worktrees| worktrees.reopen(primary_execution_id).is_ok());
        // Same worktree, same baseline: what it already holds is read first.
        if let (true, Some(live)) = (reopened, &self.live) {
            let _ = live.start(primary_execution_id);
        }
        reopened
    }

    fn workspace_usable(&self, primary_execution_id: &str) -> bool {
        self.worktrees
            .as_ref()
            .is_some_and(|worktrees| worktrees.can_reopen(primary_execution_id))
    }

    fn close_workspace(&self, primary_execution_id: &str, end: RunEnd) -> Option<WorkspaceClose> {
        let worktrees = self.worktrees.as_ref()?;
        let outcome = match end {
            RunEnd::Completed => RunOutcome::Completed,
            RunEnd::Ended => RunOutcome::Ended,
        };
        // Observed one last time while the worktree is certainly there (closing may remove an
        // empty one): this is the state the review starts from.
        if let Some(live) = &self.live {
            let phase = match end {
                RunEnd::Completed => LivePhase::Ended,
                RunEnd::Ended => LivePhase::Stopped,
            };
            live.finish(primary_execution_id, phase);
        }
        let worktree = worktrees.close_shared(primary_execution_id, outcome, Validation::NotRun)?;
        let changes = worktrees.change_set(primary_execution_id).ok();
        Some(WorkspaceClose { worktree, changes })
    }
}
