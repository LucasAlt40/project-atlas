//! The Orchestrator drives a workflow run: it asks the engine what may start, runs those steps
//! through a [`StepRunner`] (in parallel, up to the engine's limits), tells the engine how each
//! ended, keeps the shared state, persists the run at every transition and announces what
//! happens. It decides nothing about the graph (the engine does) and nothing about how a step
//! runs (the runner does: execution service, worktree, permission guard and runtime).

use crate::application::support::LockExt;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::panic::AssertUnwindSafe;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::engine::{StepCompletion, WorkflowEngine};
use super::integration::integration_from_close;
use super::runner::{
    Refusal, RunEnd, StepOutcome, StepRequest, StepRunner, StepStatus, WorkflowObserver,
    WorkspaceRequest,
};
use super::service::WorkflowService;
use super::worktree_lock::WorktreeLock;
use crate::application::errors::{AppError, ErrorCode};
use crate::application::interaction::detect_unfinished_question;
use crate::application::optimization::OptimizationFlags;
use crate::application::orchestration::brief;
use crate::application::orchestration::handoff::{
    build_handoff, record_handoff, record_result, redact_secrets_in, result_kept, HandoffSource,
    StepReport,
};
use crate::application::orchestration::result_parser::{parse_with_contract, OutcomeProblem};
use crate::application::support::now_ms;
use crate::domain::execution::WorkflowLink;
use crate::domain::interaction::{
    validate_answer, AnswerProblem, InteractionAnswer, InteractionDetection, InteractionStatus,
    PendingInteraction,
};
use crate::domain::orchestration::ArtifactType;
use crate::domain::result_contract::ResultContract;
use crate::domain::workflow::{
    FailureCode, IntegrationStatus, RecoveryKind, RecoveryPlan, RecoveryProblem, WorkflowEvent,
    WorkflowEventKind, WorkflowExecution, WorkflowExecutionStatus, WorkflowFailure,
    WorkflowIntegration,
};

/// What the user can ask of a run in progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Pause,
    Resume,
    Cancel,
}

// A step's outcome is moved once through a channel per step; boxing it would only add noise.
#[allow(clippy::large_enum_variant)]
enum Message {
    Finished {
        node_id: String,
        outcome: StepOutcome,
    },
    Control(Control),
    /// The person answered a question a step asked.
    Answer {
        interaction_id: String,
        answer: InteractionAnswer,
    },
}

/// Where an answer went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnswerDelivery {
    /// The run is being driven here and has the answer.
    Delivered,
    /// Nobody drives the run (the app was closed since it asked): the answer is recorded, and
    /// the caller picks the run up again to go on from it.
    Recorded,
}

pub struct Orchestrator {
    workflows: Arc<WorkflowService>,
    runner: Arc<dyn StepRunner>,
    /// The channel into each run being driven in this process.
    live: Mutex<HashMap<String, Sender<Message>>>,
    /// How often a run looks at things nothing reports to it (an approval being asked for, an
    /// agent becoming free).
    poll: Duration,
    /// Whether guardrails (`optimization.guardrails.enabled`) apply to what a step hands on.
    guardrails: Option<Arc<dyn OptimizationFlags>>,
}

impl Orchestrator {
    pub fn new(workflows: Arc<WorkflowService>, runner: Arc<dyn StepRunner>) -> Self {
        Self {
            workflows,
            runner,
            live: Mutex::new(HashMap::new()),
            poll: Duration::from_millis(150),
            guardrails: None,
        }
    }

    /// Applies the guardrails to what steps hand on (secrets are taken out of a result) while
    /// the flag says so.
    #[must_use]
    pub fn with_guardrails(mut self, flags: Arc<dyn OptimizationFlags>) -> Self {
        self.guardrails = Some(flags);
        self
    }

    fn guardrails_on(&self) -> bool {
        self.guardrails
            .as_ref()
            .is_some_and(|flags| flags.guardrails_enabled())
    }

    #[cfg(test)]
    #[must_use]
    pub fn with_poll(mut self, poll: Duration) -> Self {
        self.poll = poll;
        self
    }

    /// Whether the run is being driven by this process right now.
    #[cfg(test)]
    pub fn is_live(&self, execution_id: &str) -> bool {
        self.live.lock_or_recover().contains_key(execution_id)
    }

    /// Pauses, resumes or cancels a run. A run nobody is driving (interrupted by a shutdown)
    /// can only be cancelled: that is recorded directly.
    ///
    /// # Errors
    ///
    /// Fails if the run is unknown or not in a state that allows it.
    pub fn control(
        &self,
        execution_id: &str,
        control: Control,
        observer: &dyn WorkflowObserver,
    ) -> Result<(), AppError> {
        let sender = self.live.lock_or_recover().get(execution_id).cloned();
        if let Some(sender) = sender {
            return sender
                .send(Message::Control(control))
                .map_err(|_| AppError::new(ErrorCode::WorkflowStateInvalid));
        }
        let mut exec = self
            .workflows
            .execution(execution_id)
            .ok_or_else(|| AppError::new(ErrorCode::WorkflowExecutionNotFound))?;
        if control != Control::Cancel || exec.status != WorkflowExecutionStatus::Interrupted {
            return Err(AppError::new(ErrorCode::WorkflowStateInvalid));
        }
        let workflow = exec.workflow.clone();
        let mut events = WorkflowEngine::new(&workflow).request_cancel(&mut exec, now_ms());
        events.extend(self.conclude_workspace(&mut exec));
        self.workflows.save_execution(&exec)?;
        announce(observer, &events);
        Ok(())
    }

    /// Hands a person's answer to the step that asked. The answer is checked against the
    /// question (it must still be pending and be one of its options); it never changes a policy,
    /// a permission or the workflow, it is only given back to the agent as text.
    ///
    /// # Errors
    ///
    /// Fails if the run or the question is unknown, the question is no longer pending, or the
    /// answer is not one it accepts.
    pub fn answer_interaction(
        &self,
        execution_id: &str,
        interaction_id: &str,
        answer: InteractionAnswer,
        observer: &dyn WorkflowObserver,
    ) -> Result<AnswerDelivery, AppError> {
        let mut exec = self
            .workflows
            .execution(execution_id)
            .ok_or_else(|| AppError::new(ErrorCode::WorkflowExecutionNotFound))?;
        let problem = exec
            .interactions
            .iter()
            .find(|i| i.id == interaction_id)
            .map_or(Some(AnswerProblem::NotFound), |i| {
                validate_answer(i, &answer).err()
            });
        let workflow = exec.workflow.clone();
        let engine = WorkflowEngine::new(&workflow);
        if let Some(problem) = problem {
            // Refused answers are part of the trail.
            let events = engine.reject_answer(&mut exec, interaction_id, &problem, now_ms());
            self.workflows.save_execution(&exec)?;
            announce(observer, &events);
            return Err(answer_error(&problem));
        }
        let sender = self.live.lock_or_recover().get(execution_id).cloned();
        if let Some(sender) = sender {
            sender
                .send(Message::Answer {
                    interaction_id: interaction_id.to_owned(),
                    answer,
                })
                .map_err(|_| AppError::new(ErrorCode::WorkflowStateInvalid))?;
            return Ok(AnswerDelivery::Delivered);
        }
        if exec.status != WorkflowExecutionStatus::Interrupted {
            return Err(AppError::new(ErrorCode::WorkflowStateInvalid));
        }
        let events = engine
            .answer_input(&mut exec, interaction_id, &answer, now_ms())
            .map_err(|problem| answer_error(&problem))?;
        self.workflows.save_execution(&exec)?;
        announce(observer, &events);
        Ok(AnswerDelivery::Recorded)
    }

    /// Picks an interrupted run up again (the steps that were cut short start over) and drives
    /// it to its end. Blocks.
    ///
    /// # Errors
    ///
    /// Fails if the run is unknown or not interrupted.
    pub fn resume_interrupted(
        &self,
        execution_id: &str,
        observer: Arc<dyn WorkflowObserver>,
    ) -> Result<(), AppError> {
        let mut exec = self
            .workflows
            .execution(execution_id)
            .ok_or_else(|| AppError::new(ErrorCode::WorkflowExecutionNotFound))?;
        if exec.status != WorkflowExecutionStatus::Interrupted {
            return Err(AppError::new(ErrorCode::WorkflowStateInvalid));
        }
        let workflow = exec.workflow.clone();
        let events = WorkflowEngine::new(&workflow).resume_interrupted(&mut exec, now_ms());
        self.workflows.save_execution(&exec)?;
        announce(observer.as_ref(), &events);
        self.run(execution_id, observer)
    }

    /// Where a failed run would go on from, for the UI to say before the user asks: the step it
    /// stopped at, the last one that completed, the Recovery Point and what is kept. Looks only;
    /// changes nothing. `None` for a run that did not fail.
    pub fn recovery_plan(&self, execution_id: &str) -> Option<RecoveryPlan> {
        let mut exec = self.workflows.execution(execution_id)?;
        if exec.status != WorkflowExecutionStatus::Failed {
            return None;
        }
        self.workflows.adopt_current_definition(&mut exec);
        let workflow = exec.workflow.clone();
        let mut plan = WorkflowEngine::new(&workflow).recovery_plan(&exec)?;
        if plan.problem.is_none() {
            plan.problem = self.code_problem(&exec);
        }
        Some(plan)
    }

    /// Whether what the earlier steps wrote is still there to go on from.
    fn code_problem(&self, exec: &WorkflowExecution) -> Option<RecoveryProblem> {
        let primary = exec.integration.worktree_execution_id.as_deref()?;
        match exec.integration.status {
            // A run that changed nothing has nothing to lose: it gets a new worktree.
            IntegrationStatus::NotApplicable | IntegrationStatus::NoChanges => None,
            IntegrationStatus::Integrated | IntegrationStatus::Discarded => {
                Some(RecoveryProblem::CodeSettled)
            }
            _ if self.runner.workspace_usable(primary) => None,
            _ => Some(RecoveryProblem::RecoveryRequired),
        }
    }

    /// Picks a failed run up again where the graph says it should go on, in the same run and the
    /// same worktree: steps that completed are not run again. Blocks until the run ends.
    ///
    /// Nothing is applied to the project: the run's code stays in its worktree and is looked at
    /// again when the run ends, as for any run.
    ///
    /// # Errors
    ///
    /// Fails if the run is unknown, did not fail, another run of its workflow is going, or it
    /// cannot go on (the error says why).
    pub fn resume_failed(
        &self,
        execution_id: &str,
        observer: Arc<dyn WorkflowObserver>,
    ) -> Result<(), AppError> {
        let mut exec = self
            .workflows
            .execution(execution_id)
            .ok_or_else(|| AppError::new(ErrorCode::WorkflowExecutionNotFound))?;
        if exec.status != WorkflowExecutionStatus::Failed {
            return Err(AppError::new(ErrorCode::WorkflowStateInvalid));
        }
        if self.workflows.active_execution(&exec.workflow_id).is_some() {
            return Err(AppError::new(ErrorCode::WorkflowRunning));
        }
        self.workflows.adopt_current_definition(&mut exec);
        let workflow = exec.workflow.clone();
        let engine = WorkflowEngine::new(&workflow);
        let not_recoverable = |problem: RecoveryProblem| {
            let reason = serde_json::to_value(problem)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default();
            AppError::new(ErrorCode::WorkflowNotRecoverable).with("reason", reason)
        };
        let plan = engine
            .recovery_plan(&exec)
            .ok_or_else(|| not_recoverable(RecoveryProblem::NotRecoverable))?;
        if let Some(problem) = plan.problem.or_else(|| self.code_problem(&exec)) {
            return Err(not_recoverable(problem));
        }
        // The run goes on in the worktree it had; one that held nothing is simply given again.
        match exec.integration.status {
            IntegrationStatus::NoChanges => {
                exec.integration = WorkflowIntegration::default();
            }
            IntegrationStatus::NotApplicable => {}
            _ => {
                let primary = exec
                    .integration
                    .worktree_execution_id
                    .clone()
                    .unwrap_or_default();
                if !self.runner.reopen_workspace(&primary) {
                    return Err(not_recoverable(RecoveryProblem::RecoveryRequired));
                }
                exec.integration.status = IntegrationStatus::InProgress;
                exec.integration.can_apply = false;
                exec.integration.block_reason = None;
                exec.integration.conflicts.clear();
                exec.integration.message = None;
                exec.integration.updated_at = now_ms();
            }
        }
        let at = now_ms();
        let (plan, mut events) = engine
            .resume_failed(&mut exec, at)
            .map_err(not_recoverable)?;
        // A step whose result found no route could not hand anything over then; the route it
        // takes now carries what it concluded (its findings first of all).
        if let Some(node_id) = plan
            .failure_node_id
            .as_deref()
            .filter(|_| plan.kind == RecoveryKind::Resume)
        {
            let attempt = exec
                .node_state(node_id)
                .and_then(|s| s.last_attempt())
                .cloned();
            let result = result_kept(&exec, node_id);
            for link in engine.taken_links(&exec, node_id) {
                let handoff = build_handoff(
                    &exec,
                    &HandoffSource {
                        link: &link,
                        execution_id: attempt.as_ref().map_or("", |a| a.execution_id.as_str()),
                        iteration: attempt.as_ref().map_or(1, |a| a.iteration),
                        result: Some(&result),
                        delta: None,
                        failure: None,
                    },
                    at,
                );
                events.push(record_handoff(&mut exec, handoff, at));
            }
        }
        self.workflows.save_execution(&exec)?;
        announce(observer.as_ref(), &events);
        // The route it takes now may lead straight to an End: the run is then over before any
        // step starts, and only its code is left to be looked at, as for any run that ends.
        if exec.status.is_final() {
            let events = self.conclude_workspace(&mut exec);
            return self.persist(&exec, observer.as_ref(), &events);
        }
        self.run(execution_id, observer)
    }

    /// Drives a run until it ends, or (when paused with nothing in flight) until it is resumed
    /// and ends. Blocks, so call it from a blocking thread.
    ///
    /// # Errors
    ///
    /// Fails if the run is unknown or cannot be saved.
    #[allow(clippy::needless_pass_by_value)] // the run owns its observer: its step threads share it
    pub fn run(
        &self,
        execution_id: &str,
        observer: Arc<dyn WorkflowObserver>,
    ) -> Result<(), AppError> {
        let mut exec = self
            .workflows
            .execution(execution_id)
            .ok_or_else(|| AppError::new(ErrorCode::WorkflowExecutionNotFound))?;
        if exec.status != WorkflowExecutionStatus::Running {
            return Err(AppError::new(ErrorCode::WorkflowStateInvalid));
        }
        let (tx, rx) = mpsc::channel::<Message>();
        {
            let mut live = self.live.lock_or_recover();
            if live.contains_key(execution_id) {
                return Err(AppError::new(ErrorCode::WorkflowStateInvalid));
            }
            live.insert(execution_id.to_owned(), tx.clone());
        }
        let result = self.drive(&mut exec, &tx, &rx, &observer);
        // Whatever ended the run, its code is looked at once, here: with changes it waits for the
        // user's decision, never for an automatic merge.
        let concluded = if result.is_ok() && exec.status.is_final() {
            let events = self.conclude_workspace(&mut exec);
            self.persist(&exec, observer.as_ref(), &events)
        } else {
            Ok(())
        };
        self.live.lock_or_recover().remove(execution_id);
        result.and(concluded)
    }

    /// Gives the run its worktree (or picks it up again after an interruption), so that its
    /// steps share one.
    fn ensure_workspace(
        &self,
        exec: &mut WorkflowExecution,
        engine: &WorkflowEngine<'_>,
    ) -> Vec<WorkflowEvent> {
        let at = now_ms();
        if let Some(primary) = exec.integration.worktree_execution_id.clone() {
            // A run being resumed: its worktree was kept; it goes on in the same one.
            if exec.integration.status == IntegrationStatus::InProgress
                && !self.runner.reopen_workspace(&primary)
            {
                let failure = WorkflowFailure {
                    code: FailureCode::WorktreeUnavailable,
                    node_id: None,
                    detail: Some(primary),
                };
                return engine.fail(exec, failure, at);
            }
            return Vec::new();
        }
        let agents = self.workflows.isolated_agents(&exec.workflow);
        if agents.is_empty() {
            return Vec::new();
        }
        let request = WorkspaceRequest {
            workspace_id: exec.workspace_id.clone(),
            run_id: exec.id.clone(),
            agent_ids: agents,
        };
        // No worktree to give (not a Git repository…): the steps meet that themselves, each as
        // it always did, and the run has no code to integrate.
        let Some(handle) = self.runner.open_workspace(&request) else {
            return Vec::new();
        };
        exec.integration.status = IntegrationStatus::InProgress;
        exec.integration.worktree_execution_id = Some(handle.primary_execution_id);
        exec.integration.branch = Some(handle.branch);
        exec.integration.base_branch = Some(handle.base_branch);
        exec.integration.base_revision = Some(handle.base_revision);
        exec.integration.updated_at = at;
        vec![exec.record(
            WorkflowEventKind::IntegrationChanged,
            None,
            "The run has its own worktree",
            std::collections::BTreeMap::new(),
            at,
        )]
    }

    /// The run ended: decides what can be done with its code.
    fn conclude_workspace(&self, exec: &mut WorkflowExecution) -> Vec<WorkflowEvent> {
        let Some(primary) = exec.integration.worktree_execution_id.clone() else {
            return Vec::new();
        };
        if exec.integration.status != IntegrationStatus::InProgress {
            return Vec::new();
        }
        let end = if exec.status == WorkflowExecutionStatus::Completed {
            RunEnd::Completed
        } else {
            RunEnd::Ended
        };
        let at = now_ms();
        let next = if let Some(close) = self.runner.close_workspace(&primary, end) {
            exec.changes.clone_from(&close.changes);
            integration_from_close(&exec.integration, &close, at)
        } else {
            let mut failed = exec.integration.clone();
            failed.status = IntegrationStatus::Failed;
            failed.message = Some("The run's worktree could not be examined".to_owned());
            failed.updated_at = at;
            failed
        };
        exec.integration = next;
        vec![exec.record(
            WorkflowEventKind::IntegrationChanged,
            None,
            "Where the run's code stands changed",
            std::collections::BTreeMap::from([(
                "status".to_owned(),
                serde_json::to_value(exec.integration.status)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default(),
            )]),
            at,
        )]
    }

    fn drive(
        &self,
        exec: &mut WorkflowExecution,
        tx: &Sender<Message>,
        rx: &mpsc::Receiver<Message>,
        observer: &Arc<dyn WorkflowObserver>,
    ) -> Result<(), AppError> {
        let workflow = exec.workflow.clone();
        let engine = WorkflowEngine::new(&workflow);
        let mut cancelled_sent: BTreeSet<String> = BTreeSet::new();
        // The run's shared worktree is used by one writing step at a time.
        let mut lock = WorktreeLock::default();

        let fresh = exec.events.is_empty();
        let mut events = self.ensure_workspace(exec, &engine);
        if fresh && !exec.status.is_final() {
            events.extend(engine.start(exec, now_ms()));
        }
        // Saved even with nothing to announce: the run now says which worktree it works in.
        self.workflows.save_execution(exec)?;
        announce(observer.as_ref(), &events);

        loop {
            let events = self.dispatch(exec, &engine, tx, observer, &mut lock);
            self.persist(exec, observer.as_ref(), &events)?;

            // A run that must stop (cancelled, or failed by a loop limit) stops what is running.
            if exec.cancel_requested {
                self.stop_in_flight(exec, &mut cancelled_sent);
            }
            if exec.status.is_final() {
                return Ok(());
            }
            if exec.status == WorkflowExecutionStatus::Interrupted {
                return Ok(());
            }

            let first = match rx.recv_timeout(self.poll) {
                Ok(message) => Some(message),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => {
                    let failure = WorkflowFailure {
                        code: FailureCode::InternalError,
                        node_id: None,
                        detail: None,
                    };
                    let events = engine.fail(exec, failure, now_ms());
                    self.persist(exec, observer.as_ref(), &events)?;
                    return Ok(());
                }
            };
            let mut events = Vec::new();
            for message in first.into_iter().chain(rx.try_iter()) {
                events.extend(self.handle(exec, &engine, message, &mut lock));
            }
            events.extend(self.watch_approvals(exec, &engine));
            self.persist(exec, observer.as_ref(), &events)?;
            if exec.status.is_final() {
                return Ok(());
            }
        }
    }

    /// Starts every step the engine offers whose agent is free.
    fn dispatch(
        &self,
        exec: &mut WorkflowExecution,
        engine: &WorkflowEngine<'_>,
        tx: &Sender<Message>,
        observer: &Arc<dyn WorkflowObserver>,
        lock: &mut WorktreeLock,
    ) -> Vec<WorkflowEvent> {
        let mut events = Vec::new();
        let in_flight: BTreeSet<String> = exec
            .nodes
            .iter()
            .filter(|(_, s)| s.status.is_in_flight())
            .map(|(id, _)| id.clone())
            .collect();
        // A step that is about to continue after an answer keeps its place at the worktree: no
        // other writer slips in between the question and the answer.
        let resuming = exec
            .nodes
            .iter()
            .filter(|(_, s)| {
                s.status == crate::domain::workflow::NodeStatus::Ready
                    && s.last_attempt().is_some_and(|a| {
                        a.status == crate::domain::workflow::AttemptStatus::WaitingForInput
                    })
            })
            .map(|(id, _)| id.clone());
        let held: BTreeSet<String> = in_flight.iter().cloned().chain(resuming).collect();
        lock.begin_round(&held);
        for candidate in engine.dispatch_candidates(exec) {
            let at = now_ms();
            let Some(node) = engine.graph().workflow.node(&candidate.node_id) else {
                continue;
            };
            if !can_begin(exec, node) {
                // The loop limit: the engine records it and fails the run.
                events.extend(engine.begin_step(exec, &candidate.node_id, "", at).events);
                continue;
            }
            let shared_worktree = exec
                .integration
                .worktree_execution_id
                .clone()
                .filter(|_| exec.integration.status == IntegrationStatus::InProgress);
            // A step that works in the shared worktree waits for its turn at it.
            let access = shared_worktree.as_ref().and_then(|_| {
                self.runner
                    .shared_access(&exec.workspace_id, &candidate.agent_id)
            });
            if let Some(access) = access {
                if !lock.try_acquire(&candidate.node_id, access) {
                    continue; // stays ready; tried again when the worktree is free
                }
            }
            let contract = self.workflows.contract_of(&candidate.agent_id);
            let brief = brief::build(exec, engine.graph(), &candidate.node_id, &contract);
            let state = exec.node_state(&candidate.node_id);
            let (attempt, iteration) = state.map_or((1, 1), |s| {
                (
                    u32::try_from(s.attempts.len()).unwrap_or(u32::MAX) + 1,
                    if s.continuing {
                        s.iterations
                    } else {
                        s.iterations + 1
                    },
                )
            });
            let request = StepRequest {
                workspace_id: exec.workspace_id.clone(),
                agent_id: candidate.agent_id.clone(),
                instruction: brief.description,
                context_query: brief.context_query,
                brief_parts: brief.parts,
                review: brief.review,
                display_task: brief.display_task,
                shared_worktree,
                link: WorkflowLink {
                    workflow_id: exec.workflow_id.clone(),
                    workflow_name: exec.workflow.name.clone(),
                    workflow_execution_id: exec.id.clone(),
                    node_id: candidate.node_id.clone(),
                    node_label: node.label().to_owned(),
                    attempt,
                    iteration,
                },
            };
            match self.runner.prepare(request) {
                Err(Refusal::Busy) => lock.release(&candidate.node_id), // stays ready; tried again
                Err(Refusal::Invalid(reason)) => {
                    lock.release(&candidate.node_id);
                    let id = format!("{}-{}-{attempt}", exec.id, candidate.node_id);
                    events.extend(engine.begin_step(exec, &candidate.node_id, &id, at).events);
                    events.extend(engine.complete_step(
                        exec,
                        &candidate.node_id,
                        StepCompletion::Failed { message: reason },
                        now_ms(),
                    ));
                }
                Ok(prepared) => {
                    let begun =
                        engine.begin_step(exec, &candidate.node_id, prepared.execution_id(), at);
                    events.extend(begun.events);
                    if begun.started {
                        spawn_step(prepared, candidate.node_id, tx, observer.clone());
                    } else {
                        lock.release(&candidate.node_id);
                    }
                }
            }
        }
        events
    }

    #[allow(clippy::too_many_lines)]
    fn handle(
        &self,
        exec: &mut WorkflowExecution,
        engine: &WorkflowEngine<'_>,
        message: Message,
        lock: &mut WorktreeLock,
    ) -> Vec<WorkflowEvent> {
        let at = now_ms();
        match message {
            Message::Control(Control::Pause) => engine.pause(exec, at),
            Message::Control(Control::Resume) => engine.resume(exec, at),
            Message::Control(Control::Cancel) => engine.request_cancel(exec, at),
            Message::Answer {
                interaction_id,
                answer,
            } => match engine.answer_input(exec, &interaction_id, &answer, at) {
                Ok(events) => events,
                Err(problem) => engine.reject_answer(exec, &interaction_id, &problem, at),
            },
            Message::Finished {
                node_id,
                outcome:
                    StepOutcome {
                        status: StepStatus::WaitingForInput,
                        interaction: Some(interaction),
                        ..
                    },
            } if !exec.cancel_requested => {
                // The step stopped to ask a person. It has not finished: nothing is parsed,
                // nothing is handed off and no route is taken. It keeps its lock on the shared
                // worktree, because the answer continues it on top of what it already wrote.
                Self::ask(exec, engine, &node_id, interaction, at)
            }
            Message::Finished { node_id, outcome } => {
                // A step that ended without a valid result, on a question, is asking the
                // person. Failing it for want of an outcome would hide the question; it keeps
                // its place at the worktree like any step that asks.
                if outcome.status == StepStatus::Completed && !exec.cancel_requested {
                    if let Some(detection) =
                        self.unfinished_question(engine, &node_id, &outcome.text)
                    {
                        return Self::ask(exec, engine, &node_id, detection, at);
                    }
                }
                // Whatever way it ended, the step is done with the shared worktree.
                lock.release(&node_id);
                let mut events = Vec::new();
                let attempt = exec
                    .node_state(&node_id)
                    .and_then(|s| s.last_attempt())
                    .cloned();
                let execution_id = attempt
                    .as_ref()
                    .map(|a| a.execution_id.clone())
                    .unwrap_or_default();
                let iteration = attempt.as_ref().map_or(1, |a| a.iteration);
                let delta = outcome.delta.clone();
                let mut parsed = None;
                let mut failure_message = None;
                let completion = match outcome.status {
                    StepStatus::Completed => {
                        let agent_id = engine
                            .graph()
                            .workflow
                            .node(&node_id)
                            .and_then(|n| n.agent_id())
                            .map(str::to_owned);
                        let contract = agent_id
                            .as_deref()
                            .map(|id| self.workflows.contract_of(id))
                            .unwrap_or_default();
                        let read = parse_with_contract(&outcome.text, &contract);
                        let (mut result, problem) = (read.result, read.problem);
                        // What the next agent receives is this result: secrets do not travel in it.
                        if self.guardrails_on() {
                            let lines = redact_secrets_in(&mut result);
                            if lines > 0 {
                                events.push(exec.record(
                                    WorkflowEventKind::GuardrailTransformed,
                                    Some(&node_id),
                                    "Secrets were taken out of a step's result before it was handed on"
                                        .to_owned(),
                                    BTreeMap::from([
                                        ("stage".to_owned(), "before_handoff".to_owned()),
                                        ("rule".to_owned(), "secrets.redact".to_owned()),
                                        ("lines".to_owned(), lines.to_string()),
                                        ("executionId".to_owned(), execution_id.clone()),
                                    ]),
                                    at,
                                ));
                            }
                        }
                        let label = engine
                            .graph()
                            .workflow
                            .node(&node_id)
                            .map_or_else(String::new, |n| n.label().to_owned());
                        let personality = engine
                            .graph()
                            .workflow
                            .node(&node_id)
                            .and_then(|n| n.agent_id())
                            .and_then(|agent| self.workflows.personality_of(agent));
                        // In a shared worktree the files a step changed are what Git measured,
                        // not what the agent said it touched.
                        let measured = delta.as_ref().map(|d| {
                            let mut files: Vec<String> =
                                d.files.iter().map(|f| f.path.clone()).collect();
                            files.extend(d.uncommitted.iter().cloned());
                            files.sort();
                            files.dedup();
                            files
                        });
                        if let Some(problem) = problem {
                            // The agent's contract asks for an outcome and it gave none that
                            // is declared. Nothing is assumed (not a pass, not a success): the
                            // step fails, deterministically, and goes the way any failed step
                            // goes (retry, failure route, or the run failing). What it did
                            // say still travels with the failure.
                            let message = no_valid_outcome(&contract, &problem);
                            failure_message = Some(message.clone());
                            parsed = Some(result);
                            StepCompletion::Failed { message }
                        } else {
                            events.extend(record_result(
                                exec,
                                engine.graph(),
                                &StepReport {
                                    node_id: &node_id,
                                    node_label: &label,
                                    execution_id: &execution_id,
                                    default_artifact: default_artifact(personality.as_deref()),
                                    result: &result,
                                    measured_files: measured,
                                },
                                at,
                            ));
                            let completion = StepCompletion::Completed {
                                facts: result.facts(),
                                summary: Some(result.summary.clone()).filter(|s| !s.is_empty()),
                            };
                            parsed = Some(result);
                            completion
                        }
                    }
                    StepStatus::WaitingForInput => {
                        // Waiting for a person with nothing to ask, or the run is being
                        // cancelled: there is no one to answer, so the step does not wait.
                        if exec.cancel_requested {
                            StepCompletion::Cancelled
                        } else {
                            let message =
                                "The step asked for input but gave no question".to_owned();
                            failure_message = Some(message.clone());
                            StepCompletion::Failed { message }
                        }
                    }
                    StepStatus::Failed => {
                        let message = outcome
                            .failure
                            .unwrap_or_else(|| "The step failed".to_owned());
                        failure_message = Some(message.clone());
                        StepCompletion::Failed { message }
                    }
                    StepStatus::Cancelled => StepCompletion::Cancelled,
                };
                events.extend(engine.complete_step(exec, &node_id, completion, now_ms()));
                // Every transition the ending sent the run along carries what the step produced.
                for link in engine.taken_links(exec, &node_id) {
                    let handoff = build_handoff(
                        exec,
                        &HandoffSource {
                            link: &link,
                            execution_id: &execution_id,
                            iteration,
                            result: parsed.as_ref(),
                            delta: delta.as_ref(),
                            failure: failure_message.as_deref(),
                        },
                        now_ms(),
                    );
                    events.push(record_handoff(exec, handoff, now_ms()));
                }
                events
            }
        }
    }

    /// The question a step ended on, when it gave no valid outcome (so it cannot be routed).
    fn unfinished_question(
        &self,
        engine: &WorkflowEngine<'_>,
        node_id: &str,
        text: &str,
    ) -> Option<InteractionDetection> {
        let agent = engine.graph().workflow.node(node_id)?.agent_id()?;
        let contract = self.workflows.contract_of(agent);
        let read = parse_with_contract(text, &contract);
        if !matches!(read.problem, Some(OutcomeProblem::Missing)) {
            return None;
        }
        detect_unfinished_question(text)
    }

    /// The step is waiting for a person: records the question (persisted with the run) and
    /// pauses the run.
    fn ask(
        exec: &mut WorkflowExecution,
        engine: &WorkflowEngine<'_>,
        node_id: &str,
        detection: InteractionDetection,
        at: u64,
    ) -> Vec<WorkflowEvent> {
        let Some(kind) = detection.kind else {
            return engine.complete_step(
                exec,
                node_id,
                StepCompletion::Failed {
                    message: "The step asked for input but gave no question".to_owned(),
                },
                at,
            );
        };
        let node = engine.graph().workflow.node(node_id);
        let attempt = exec
            .node_state(node_id)
            .and_then(|s| s.last_attempt())
            .cloned();
        let interaction = PendingInteraction {
            workflow_id: exec.workflow_id.clone(),
            id: format!("{}-{node_id}-q{}", exec.id, exec.interactions.len() + 1),
            execution_id: attempt
                .as_ref()
                .map(|a| a.execution_id.clone())
                .unwrap_or_default(),
            workflow_execution_id: exec.id.clone(),
            workspace_id: exec.workspace_id.clone(),
            step_id: node_id.to_owned(),
            step_label: node.map_or_else(String::new, |n| n.label().to_owned()),
            agent_id: node
                .and_then(|n| n.agent_id())
                .map(str::to_owned)
                .unwrap_or_default(),
            iteration: attempt.as_ref().map_or(1, |a| a.iteration),
            kind,
            question: detection.question,
            context: detection.context,
            document: detection.document,
            options: detection.options,
            source: detection.source,
            confidence: detection.confidence,
            status: InteractionStatus::Pending,
            created_at: at,
            answered_at: None,
            choice: None,
            answer: None,
            evaluation: detection.evaluation,
        };
        engine.wait_for_input(exec, node_id, interaction, at)
    }

    /// Reflects in the run which steps are waiting for the user. The answer comes from the real
    /// approval mechanism, never from anything an agent said.
    fn watch_approvals(
        &self,
        exec: &mut WorkflowExecution,
        engine: &WorkflowEngine<'_>,
    ) -> Vec<WorkflowEvent> {
        let mut events = Vec::new();
        let in_flight: Vec<(String, String)> = exec
            .nodes
            .iter()
            .filter(|(_, s)| s.status.is_in_flight())
            .filter_map(|(id, s)| {
                s.last_attempt()
                    .map(|a| (id.clone(), a.execution_id.clone()))
            })
            .collect();
        for (node_id, execution_id) in in_flight {
            let waiting = self.runner.awaiting_approval(&execution_id);
            events.extend(engine.set_waiting_approval(exec, &node_id, waiting, now_ms()));
        }
        events
    }

    fn stop_in_flight(&self, exec: &WorkflowExecution, sent: &mut BTreeSet<String>) {
        for node in &exec.workflow.nodes {
            let Some(state) = exec.node_state(&node.id) else {
                continue;
            };
            if !state.status.is_in_flight() {
                continue;
            }
            let (Some(agent), Some(attempt)) = (node.agent_id(), state.last_attempt()) else {
                continue;
            };
            if sent.insert(attempt.execution_id.clone()) {
                self.runner
                    .cancel(&exec.workspace_id, agent, &attempt.execution_id);
            }
        }
    }

    fn persist(
        &self,
        exec: &WorkflowExecution,
        observer: &dyn WorkflowObserver,
        events: &[WorkflowEvent],
    ) -> Result<(), AppError> {
        if events.is_empty() {
            return Ok(());
        }
        self.workflows.save_execution(exec)?;
        announce(observer, events);
        Ok(())
    }
}

fn answer_error(problem: &AnswerProblem) -> AppError {
    AppError::new(match problem {
        AnswerProblem::NotFound => ErrorCode::InteractionNotFound,
        AnswerProblem::NotPending => ErrorCode::InteractionNotPending,
        AnswerProblem::Invalid => ErrorCode::InteractionAnswerInvalid,
    })
}

fn announce(observer: &dyn WorkflowObserver, events: &[WorkflowEvent]) {
    for event in events {
        observer.on_workflow_event(event);
    }
}

/// Whether the node may start without going past its loop limit.
fn can_begin(exec: &WorkflowExecution, node: &crate::domain::workflow::WorkflowNode) -> bool {
    let Some(state) = exec.node_state(&node.id) else {
        return false;
    };
    match &node.loop_policy {
        Some(policy) if !state.continuing => state.iterations < policy.max_iterations,
        _ => true,
    }
}

/// Why a step with a result contract failed for want of a valid outcome.
fn no_valid_outcome(contract: &ResultContract, problem: &OutcomeProblem) -> String {
    let allowed: Vec<&str> = contract.outcomes.iter().map(|o| o.id.as_str()).collect();
    let allowed = allowed.join(", ");
    match problem {
        OutcomeProblem::Missing => {
            format!("No valid outcome: the agent did not declare one of [{allowed}]")
        }
        OutcomeProblem::Undeclared(said) => {
            format!("No valid outcome: \"{said}\" is not one of [{allowed}]")
        }
    }
}

/// What a step produces when its agent describes no artifact: by what kind of agent it was.
fn default_artifact(personality: Option<&str>) -> ArtifactType {
    match personality {
        Some("architect") => ArtifactType::ArchitectureDocument,
        Some("qa") => ArtifactType::TestReport,
        Some("architecture-validator") => ArtifactType::ValidationReport,
        _ => ArtifactType::ImplementationSummary,
    }
}

fn spawn_step(
    prepared: Box<dyn super::runner::PreparedStep>,
    node_id: String,
    tx: &Sender<Message>,
    observer: Arc<dyn WorkflowObserver>,
) {
    let name = format!("workflow-step-{node_id}");
    let spawned = std::thread::Builder::new().name(name).spawn({
        let tx = tx.clone();
        let node_id = node_id.clone();
        move || {
            let outcome =
                std::panic::catch_unwind(AssertUnwindSafe(|| prepared.run(observer.as_ref())))
                    .unwrap_or_else(|_| StepOutcome {
                        status: StepStatus::Failed,
                        text: String::new(),
                        failure: Some("The step stopped unexpectedly".to_owned()),
                        interaction: None,
                        delta: None,
                    });
            let _ = tx.send(Message::Finished { node_id, outcome });
        }
    });
    if spawned.is_err() {
        let _ = tx.send(Message::Finished {
            node_id,
            outcome: StepOutcome {
                status: StepStatus::Failed,
                text: String::new(),
                failure: Some("The step could not be started".to_owned()),
                interaction: None,
                delta: None,
            },
        });
    }
}
