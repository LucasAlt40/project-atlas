//! The phone's way into the existing system: reads the runs the Orchestrator keeps and hands
//! answers and controls back to it. It decides nothing about workflows — the engine still
//! validates every answer, and a race with the desktop is settled there (ADR 0034 §1).

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::policy::{self, rule_for};
use super::view::{self, Names, RunDetail, RunSummary};
use crate::application::agents::AgentService;
use crate::application::errors::{AppError, ErrorCode};
use crate::application::settings::SettingsService;
use crate::application::support::now_ms;
use crate::application::workflow::orchestrator::{AnswerDelivery, Control, Orchestrator};
use crate::application::workflow::runner::WorkflowObserver;
use crate::application::workflow::service::WorkflowService;
use crate::application::workspace::WorkspaceService;
use crate::domain::interaction::{InteractionAnswer, InteractionStatus, PendingInteraction};
use crate::domain::workflow::{WorkflowExecution, WorkflowExecutionStatus};

/// Finished runs are listed for this long.
const RECENT_MS: u64 = 7 * 24 * 60 * 60 * 1000;
/// Runs in one listing.
const MAX_RUNS: usize = 60;
/// How long a request waits to see what the run did with it.
const SETTLE: Duration = Duration::from_secs(5);
const SETTLE_STEP: Duration = Duration::from_millis(25);

/// Why the phone's request did not go through. `code` is stable and worded by the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteError {
    NotFound,
    /// This is for the computer (see `policy`).
    DesktopOnly,
    /// Someone else answered first, or it was answered differently before.
    AlreadyResolved,
    /// The answer is not one the question accepts.
    InvalidAnswer,
    /// The run is not being driven (the app was closed since): it must be resumed at the computer.
    NotLive,
    /// A cancel was asked for without saying it was meant.
    ConfirmationRequired,
    /// The run is not in a state that allows this.
    StateInvalid,
    Internal,
}

impl RemoteError {
    pub fn code(self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::DesktopOnly => "desktop_only",
            Self::AlreadyResolved => "already_resolved",
            Self::InvalidAnswer => "invalid_answer",
            Self::NotLive => "run_not_live",
            Self::ConfirmationRequired => "confirmation_required",
            Self::StateInvalid => "state_invalid",
            Self::Internal => "internal",
        }
    }

    pub fn status(self) -> u16 {
        match self {
            Self::NotFound => 404,
            Self::DesktopOnly => 403,
            Self::AlreadyResolved | Self::NotLive | Self::StateInvalid => 409,
            Self::InvalidAnswer | Self::ConfirmationRequired => 422,
            Self::Internal => 500,
        }
    }

    fn from_app(error: &AppError) -> Self {
        match error.code {
            ErrorCode::InteractionNotFound | ErrorCode::WorkflowExecutionNotFound => Self::NotFound,
            ErrorCode::InteractionNotPending => Self::AlreadyResolved,
            ErrorCode::InteractionAnswerInvalid => Self::InvalidAnswer,
            ErrorCode::WorkflowStateInvalid => Self::StateInvalid,
            _ => Self::Internal,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub server_time: u64,
    pub language: String,
    pub runs: Vec<RunSummary>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnswerRequest {
    pub choice: Option<String>,
    pub text: Option<String>,
}

/// What became of an answer, as the backend confirms it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Applied {
    /// This request answered the question.
    Now,
    /// The question had already been answered exactly like this (a retry after a lost reply).
    Already,
    /// The answer was accepted but the run has not shown it yet; read the run again.
    Pending,
    /// The run was not being driven: the answer is kept, and the run goes on once resumed at the
    /// computer.
    Recorded,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Answered {
    pub applied: Applied,
    pub run: RunDetail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlAction {
    Pause,
    Resume,
    Cancel,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlRequest {
    pub action: Option<ControlAction>,
    #[serde(default)]
    pub confirm: bool,
}

pub struct RemoteGateway {
    workflows: Arc<WorkflowService>,
    workspaces: Arc<WorkspaceService>,
    agents: Arc<AgentService>,
    settings: Arc<SettingsService>,
    orchestrator: Arc<Orchestrator>,
}

impl RemoteGateway {
    pub fn new(
        workflows: Arc<WorkflowService>,
        workspaces: Arc<WorkspaceService>,
        agents: Arc<AgentService>,
        settings: Arc<SettingsService>,
        orchestrator: Arc<Orchestrator>,
    ) -> Self {
        Self {
            workflows,
            workspaces,
            agents,
            settings,
            orchestrator,
        }
    }

    fn with_names<T>(&self, exec: &WorkflowExecution, f: impl FnOnce(&Names<'_>) -> T) -> T {
        let workspace = self
            .workspaces
            .get(&exec.workspace_id)
            .map_or_else(String::new, |w| w.name);
        let agent = |id: &str| self.agents.find(id).map(|a| a.name);
        f(&Names {
            workspace: &workspace,
            agent: &agent,
        })
    }

    fn detail_of(&self, exec: &WorkflowExecution) -> RunDetail {
        self.with_names(exec, |names| view::detail(exec, names))
    }

    /// The runs worth a glance: everything that has not ended, and what ended in the last week,
    /// those that need a person first.
    pub fn overview(&self) -> Overview {
        let now = now_ms();
        let mut runs: Vec<RunSummary> = self
            .workspaces
            .list()
            .iter()
            .flat_map(|workspace| {
                self.workflows.executions_since(
                    &workspace.id,
                    now.saturating_sub(RECENT_MS),
                    MAX_RUNS,
                )
            })
            .map(|exec| self.with_names(&exec, |names| view::summary(&exec, names)))
            .collect();
        runs.sort_by(|a, b| {
            let rank = |r: &RunSummary| match r.attention {
                view::Attention::NeedsAnswer => 0,
                view::Attention::NeedsComputer => 1,
                view::Attention::Failed => 2,
                view::Attention::None => 3,
            };
            rank(a)
                .cmp(&rank(b))
                .then(b.updated_at.cmp(&a.updated_at))
                .then(a.id.cmp(&b.id))
        });
        runs.truncate(MAX_RUNS);
        Overview {
            server_time: now,
            language: self.language(),
            runs,
        }
    }

    pub fn language(&self) -> String {
        self.settings.get().language
    }

    /// # Errors
    ///
    /// [`RemoteError::NotFound`] for a run Atlas does not have.
    pub fn run(&self, run_id: &str) -> Result<RunDetail, RemoteError> {
        self.workflows
            .execution(run_id)
            .map(|exec| self.detail_of(&exec))
            .ok_or(RemoteError::NotFound)
    }

    /// Answers a question a step asked. The phone names the run, the question and the answer;
    /// everything else is stored state. The result is read back from the store, so what the page
    /// shows is what happened, including when the desktop answered first.
    ///
    /// # Errors
    ///
    /// See [`RemoteError`].
    pub fn answer(
        &self,
        run_id: &str,
        interaction_id: &str,
        request: &AnswerRequest,
        observer: &dyn WorkflowObserver,
    ) -> Result<Answered, RemoteError> {
        let answer = InteractionAnswer {
            choice: request.choice.clone(),
            text: request.text.clone(),
        };
        let exec = self
            .workflows
            .execution(run_id)
            .ok_or(RemoteError::NotFound)?;
        let question = find(&exec, interaction_id).ok_or(RemoteError::NotFound)?;

        if !question.is_pending() {
            // A retry after a lost reply says the same thing again: that is not an error.
            return if says_the_same(question, &answer) {
                Ok(self.answered(run_id, Applied::Already))
            } else {
                Err(RemoteError::AlreadyResolved)
            };
        }
        if !policy::allows(question, &answer) {
            return Err(RemoteError::DesktopOnly);
        }
        if !exec.status.is_active() {
            return Err(RemoteError::NotLive);
        }

        let delivery = self
            .orchestrator
            .answer_interaction(run_id, interaction_id, answer.clone(), observer)
            .map_err(|e| RemoteError::from_app(&e))?;
        if delivery == AnswerDelivery::Recorded {
            return Ok(self.answered(run_id, Applied::Recorded));
        }

        // The answer went to the run's own loop. Wait for the loop to record it, then say what it
        // recorded: if the desktop got there first, that is the answer that counts.
        let settled = wait_until(|| {
            self.workflows
                .execution(run_id)
                .as_ref()
                .and_then(|e| find(e, interaction_id))
                .is_none_or(|q| !q.is_pending())
        });
        let current = self
            .workflows
            .execution(run_id)
            .ok_or(RemoteError::NotFound)?;
        let question = find(&current, interaction_id).ok_or(RemoteError::NotFound)?;
        if !settled {
            return Ok(self.answered(run_id, Applied::Pending));
        }
        if says_the_same(question, &answer) {
            Ok(self.answered(run_id, Applied::Now))
        } else {
            Err(RemoteError::AlreadyResolved)
        }
    }

    fn answered(&self, run_id: &str, applied: Applied) -> Answered {
        let run = self
            .workflows
            .execution(run_id)
            .map(|e| self.detail_of(&e))
            .expect("the run was read a moment ago");
        Answered { applied, run }
    }

    /// Pauses, resumes or cancels a run that is being driven. Cancelling must be confirmed in the
    /// request, so a stray call cannot do it.
    ///
    /// # Errors
    ///
    /// See [`RemoteError`].
    pub fn control(
        &self,
        run_id: &str,
        request: &ControlRequest,
        observer: &dyn WorkflowObserver,
    ) -> Result<RunDetail, RemoteError> {
        let action = request.action.ok_or(RemoteError::InvalidAnswer)?;
        if action == ControlAction::Cancel && !request.confirm {
            return Err(RemoteError::ConfirmationRequired);
        }
        let exec = self
            .workflows
            .execution(run_id)
            .ok_or(RemoteError::NotFound)?;
        let controls = self.detail_of(&exec).controls;
        let allowed = match action {
            ControlAction::Pause => controls.pause,
            ControlAction::Resume => controls.resume,
            ControlAction::Cancel => controls.cancel,
        };
        if !allowed {
            return Err(if exec.status == WorkflowExecutionStatus::Interrupted {
                RemoteError::NotLive
            } else {
                RemoteError::StateInvalid
            });
        }
        let control = match action {
            ControlAction::Pause => Control::Pause,
            ControlAction::Resume => Control::Resume,
            ControlAction::Cancel => Control::Cancel,
        };
        self.orchestrator
            .control(run_id, control, observer)
            .map_err(|e| RemoteError::from_app(&e))?;
        wait_until(|| {
            self.workflows
                .execution(run_id)
                .is_none_or(|e| match action {
                    ControlAction::Pause => e.status != WorkflowExecutionStatus::Running,
                    ControlAction::Resume => e.status != WorkflowExecutionStatus::Paused,
                    ControlAction::Cancel => e.cancel_requested || e.status.is_final(),
                })
        });
        self.run(run_id)
    }

    /// Whether any run asks for a person the phone can reach (for the desktop's status line).
    pub fn waiting_for_phone(&self) -> usize {
        self.workspaces
            .list()
            .iter()
            .flat_map(|w| self.workflows.executions(&w.id, None))
            .filter(|e| e.status.is_active())
            .flat_map(|e| e.interactions)
            .filter(|i| i.is_pending() && rule_for(i).can_answer)
            .count()
    }
}

fn find<'a>(exec: &'a WorkflowExecution, interaction_id: &str) -> Option<&'a PendingInteraction> {
    exec.interactions.iter().find(|i| i.id == interaction_id)
}

/// Whether a question was answered with exactly this. A pending one never was.
fn says_the_same(question: &PendingInteraction, answer: &InteractionAnswer) -> bool {
    let clean = |v: Option<&str>| {
        v.map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    question.status == InteractionStatus::Answered
        && !question.auto
        && question.choice == clean(answer.choice.as_deref())
        && question.answer == clean(answer.text.as_deref())
}

/// Polls `done` until it holds or [`SETTLE`] passes. Called on a blocking thread.
fn wait_until(mut done: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + SETTLE;
    loop {
        if done() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(SETTLE_STEP);
    }
}
