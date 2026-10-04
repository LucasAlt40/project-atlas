//! The Workflow Engine: reads the graph and decides what may run. It is a pure state machine
//! over a [`WorkflowExecution`]: it never runs a step, reads a clock or touches storage. The
//! orchestrator tells it what happened (a step started, finished, failed) and asks what to do
//! next, so the same definition and the same history always give the same answers.
//!
//! Semantics, in short:
//!
//! * A **node** is a unit of work, an **edge** a transition, a **condition** decides whether an
//!   edge is taken, an **artifact** is a step's shareable output.
//! * A node becomes *ready* when it is *activated*. Edges from outside a cycle are either
//!   requirements (unconditional ones: all must have fired) or alternatives (conditional ones
//!   and failure routes: any one will do), and either group is enough. Edges inside a cycle are
//!   always alternatives: any one re-activates the node. That is what lets a join
//!   wait for two parallel branches while a `QA -> Bug Fixer -> QA` loop does not deadlock.
//! * A step that fails is retried up to its limit, then follows its failure route, or stops the
//!   workflow. A step that finishes follows the edges whose conditions match its result.
//! * A node whose activation can no longer happen is *blocked* (a dependency failed) or
//!   *skipped* (the conditions routed around it).
//! * A node with a loop limit may start at most that many times: the next start fails the run.
//! * The run ends when nothing is in flight and nothing is ready.

use std::collections::{BTreeMap, BTreeSet};

use super::graph::{Graph, Link};
use crate::domain::interaction::{
    validate_answer, AnswerProblem, InteractionAnswer, InteractionStatus, PendingInteraction,
};
use crate::domain::workflow::Workflow;
use crate::domain::workflow::{
    AttemptStatus, EndOutcome, Facts, FailureCode, NodeAttempt, NodeKind, NodeStatus, RecoveryKind,
    RecoveryPlan, RecoveryProblem, RecoveryRecord, WorkflowEvent, WorkflowEventKind,
    WorkflowExecution, WorkflowExecutionStatus, WorkflowFailure,
};

/// A node that is ready and could start now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub node_id: String,
    pub agent_id: String,
}

/// What a step came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepCompletion {
    Completed {
        facts: Facts,
        summary: Option<String>,
    },
    Failed {
        message: String,
    },
    Cancelled,
}

/// Whether a step was allowed to start.
#[derive(Debug)]
pub struct Begun {
    pub started: bool,
    pub events: Vec<WorkflowEvent>,
}

pub struct WorkflowEngine<'a> {
    graph: Graph<'a>,
}

type Events = Vec<WorkflowEvent>;

fn meta(pairs: &[(&str, String)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.clone()))
        .collect()
}

impl<'a> WorkflowEngine<'a> {
    pub fn new(workflow: &'a Workflow) -> Self {
        Self {
            graph: Graph::new(workflow),
        }
    }

    pub fn graph(&self) -> &Graph<'a> {
        &self.graph
    }

    /// Makes the start nodes ready.
    pub fn start(&self, exec: &mut WorkflowExecution, at: u64) -> Events {
        let mut events = vec![exec.record(
            WorkflowEventKind::Started,
            None,
            "Workflow started",
            BTreeMap::new(),
            at,
        )];
        for id in self.graph.starts() {
            self.make_ready(exec, id, at, &mut events);
        }
        self.settle_into(exec, at, &mut events);
        events
    }

    /// The ready agent steps that may start now, best first: highest priority, then creation
    /// order, then id. At most `max_parallel_steps` run at once and an agent never runs two
    /// steps at the same time, so a second step of a busy agent waits.
    pub fn dispatch_candidates(&self, exec: &WorkflowExecution) -> Vec<Candidate> {
        if exec.status != WorkflowExecutionStatus::Running || exec.cancel_requested {
            return vec![];
        }
        let capacity = (exec.max_parallel_steps as usize).saturating_sub(exec.in_flight());
        let mut busy: BTreeSet<&str> = self
            .graph
            .workflow
            .nodes
            .iter()
            .filter(|n| {
                exec.node_state(&n.id)
                    .is_some_and(|s| s.status.is_in_flight())
            })
            .filter_map(|n| n.agent_id())
            .collect();
        let mut out = Vec::new();
        for node in self.ready_sorted(exec) {
            if out.len() >= capacity {
                break;
            }
            if let Some(agent_id) = node.agent_id() {
                if busy.insert(agent_id) {
                    out.push(Candidate {
                        node_id: node.id.clone(),
                        agent_id: agent_id.to_owned(),
                    });
                }
            }
        }
        out
    }

    /// The step starts: a new attempt with its own execution id. Refuses (and fails the run) if
    /// a loop would go past its limit.
    pub fn begin_step(
        &self,
        exec: &mut WorkflowExecution,
        node_id: &str,
        execution_id: &str,
        at: u64,
    ) -> Begun {
        let mut events = Vec::new();
        let Some(node) = self.graph.workflow.node(node_id) else {
            return Begun {
                started: false,
                events,
            };
        };
        let Some(state) = exec.nodes.get(node_id) else {
            return Begun {
                started: false,
                events,
            };
        };
        if state.status != NodeStatus::Ready {
            return Begun {
                started: false,
                events,
            };
        }
        if !self.enter(exec, node_id, at, &mut events) {
            self.settle_into(exec, at, &mut events);
            return Begun {
                started: false,
                events,
            };
        }
        let state = exec.nodes.get_mut(node_id).expect("node state exists");
        let attempt = u32::try_from(state.attempts.len()).unwrap_or(u32::MAX) + 1;
        let iteration = state.iterations;
        state.status = NodeStatus::Running;
        state.reason = None;
        state.attempts.push(NodeAttempt {
            attempt,
            iteration,
            execution_id: execution_id.to_owned(),
            status: AttemptStatus::Running,
            started_at: at,
            completed_at: None,
            summary: None,
            outcome: None,
            failure: None,
        });
        let mut details = vec![
            ("executionId", execution_id.to_owned()),
            ("attempt", attempt.to_string()),
            ("iteration", iteration.to_string()),
        ];
        if let Some(policy) = &node.loop_policy {
            details.push(("loopId", policy.loop_id.clone()));
            details.push(("max", policy.max_iterations.to_string()));
        }
        events.push(exec.record(
            WorkflowEventKind::NodeStarted,
            Some(node_id),
            format!("{} started", node.label()),
            meta(&details),
            at,
        ));
        self.refresh_shared(exec);
        Begun {
            started: true,
            events,
        }
    }

    /// The step is waiting for the user (or no longer is). Only the real approval mechanism
    /// reports this; nothing an agent says reaches here.
    pub fn set_waiting_approval(
        &self,
        exec: &mut WorkflowExecution,
        node_id: &str,
        waiting: bool,
        at: u64,
    ) -> Events {
        let Some(state) = exec.nodes.get_mut(node_id) else {
            return vec![];
        };
        let to = match (state.status, waiting) {
            (NodeStatus::Running, true) => NodeStatus::WaitingApproval,
            (NodeStatus::WaitingApproval, false) => NodeStatus::Running,
            _ => return vec![],
        };
        state.status = to;
        let label = self
            .graph
            .workflow
            .node(node_id)
            .map_or_else(String::new, |n| n.label().to_owned());
        let (kind, text) = if waiting {
            (
                WorkflowEventKind::NodeWaitingApproval,
                format!("{label} is waiting for approval"),
            )
        } else {
            (
                WorkflowEventKind::NodeApprovalResolved,
                format!("{label} is no longer waiting for approval"),
            )
        };
        vec![exec.record(kind, Some(node_id), text, BTreeMap::new(), at)]
    }

    /// The step asked a person something. It has not finished: no result, no outcome, no
    /// handoff, no route taken. The run stands still (no new step starts) until the question is
    /// answered. At most one question is pending per step.
    pub fn wait_for_input(
        &self,
        exec: &mut WorkflowExecution,
        node_id: &str,
        interaction: PendingInteraction,
        at: u64,
    ) -> Events {
        if exec.pending_interaction(node_id).is_some() {
            return vec![];
        }
        let label = self
            .graph
            .workflow
            .node(node_id)
            .map_or_else(String::new, |n| n.label().to_owned());
        let Some(state) = exec.nodes.get_mut(node_id) else {
            return vec![];
        };
        if !state.status.is_in_flight() {
            return vec![];
        }
        state.status = NodeStatus::WaitingForInput;
        if let Some(attempt) = state.attempts.last_mut() {
            attempt.status = AttemptStatus::WaitingForInput;
            attempt.completed_at = Some(at);
        }
        if exec.status == WorkflowExecutionStatus::Running {
            exec.status = WorkflowExecutionStatus::WaitingForInput;
        }
        let details = meta(&[
            ("interactionId", interaction.id.clone()),
            ("kind", interaction.kind.as_str().to_owned()),
            (
                "source",
                format!("{:?}", interaction.source).to_ascii_lowercase(),
            ),
            ("executionId", interaction.execution_id.clone()),
        ]);
        let question = interaction.question.clone();
        exec.interactions.push(interaction);
        let events = vec![
            exec.record(
                WorkflowEventKind::InteractionDetected,
                Some(node_id),
                question,
                details.clone(),
                at,
            ),
            exec.record(
                WorkflowEventKind::NodeWaitingForInput,
                Some(node_id),
                format!("{label} is waiting for your input"),
                details,
                at,
            ),
        ];
        self.refresh_shared(exec);
        events
    }

    /// The person answered. The step is ready to run again (as a continuation of the same pass,
    /// in the same worktree) and the run goes back to running when nothing else waits. The
    /// answer is checked against the question first; a refused answer changes nothing.
    ///
    /// # Errors
    ///
    /// The question is unknown, no longer pending, or the answer is not one it accepts.
    pub fn answer_input(
        &self,
        exec: &mut WorkflowExecution,
        interaction_id: &str,
        answer: &InteractionAnswer,
        at: u64,
    ) -> Result<Events, AnswerProblem> {
        let index = exec
            .interactions
            .iter()
            .position(|i| i.id == interaction_id)
            .ok_or(AnswerProblem::NotFound)?;
        let (choice, text) = validate_answer(&exec.interactions[index], answer)?;
        let node_id = exec.interactions[index].step_id.clone();
        if exec
            .node_state(&node_id)
            .is_none_or(|s| s.status != NodeStatus::WaitingForInput)
        {
            return Err(AnswerProblem::NotPending);
        }
        let label = exec.interactions[index].step_label.clone();
        let interaction = &mut exec.interactions[index];
        interaction.status = InteractionStatus::Answered;
        interaction.answered_at = Some(at);
        interaction.choice = choice;
        interaction.answer = text;
        let details = meta(&[
            ("interactionId", interaction_id.to_owned()),
            ("kind", interaction.kind.as_str().to_owned()),
            ("declined", interaction.was_declined().to_string()),
        ]);
        if let Some(state) = exec.nodes.get_mut(&node_id) {
            state.status = NodeStatus::Ready;
            state.continuing = true;
        }
        let mut events = vec![
            exec.record(
                WorkflowEventKind::InteractionAnswered,
                Some(&node_id),
                format!("{label} was answered"),
                details.clone(),
                at,
            ),
            exec.record(
                WorkflowEventKind::NodeInputResolved,
                Some(&node_id),
                format!("{label} will continue"),
                details,
                at,
            ),
        ];
        self.leave_waiting(exec);
        self.refresh_shared(exec);
        self.settle_into(exec, at, &mut events);
        Ok(events)
    }

    /// An answer that was refused is part of the audit trail too.
    #[allow(clippy::unused_self)]
    pub fn reject_answer(
        &self,
        exec: &mut WorkflowExecution,
        interaction_id: &str,
        problem: &AnswerProblem,
        at: u64,
    ) -> Events {
        let node_id = exec
            .interactions
            .iter()
            .find(|i| i.id == interaction_id)
            .map(|i| i.step_id.clone());
        let reason = match problem {
            AnswerProblem::NotFound => "not_found",
            AnswerProblem::NotPending => "not_pending",
            AnswerProblem::Invalid => "invalid",
        };
        vec![exec.record(
            WorkflowEventKind::InteractionRejected,
            node_id.as_deref(),
            "An answer was refused",
            meta(&[
                ("interactionId", interaction_id.to_owned()),
                ("reason", reason.to_owned()),
            ]),
            at,
        )]
    }

    /// The run was waiting only because of steps that are no longer waiting: it runs again.
    #[allow(clippy::unused_self)]
    fn leave_waiting(&self, exec: &mut WorkflowExecution) {
        if exec.status == WorkflowExecutionStatus::WaitingForInput
            && exec.count(NodeStatus::WaitingForInput) == 0
        {
            exec.status = WorkflowExecutionStatus::Running;
        }
    }

    /// A run that comes back (resumed, unpaused) while a step still waits goes on waiting.
    #[allow(clippy::unused_self)]
    fn restore_waiting(&self, exec: &mut WorkflowExecution) {
        if exec.status == WorkflowExecutionStatus::Running
            && exec.count(NodeStatus::WaitingForInput) > 0
        {
            exec.status = WorkflowExecutionStatus::WaitingForInput;
        }
    }

    /// Records how the step ended and moves the run on: routes along the matching edges, retries,
    /// follows the failure route or leaves the failure standing.
    pub fn complete_step(
        &self,
        exec: &mut WorkflowExecution,
        node_id: &str,
        completion: StepCompletion,
        at: u64,
    ) -> Events {
        let mut events = Vec::new();
        let Some(node) = self.graph.workflow.node(node_id) else {
            return events;
        };
        let Some(state) = exec.nodes.get_mut(node_id) else {
            return events;
        };
        if !state.status.is_in_flight() {
            return events;
        }
        let label = node.label().to_owned();
        match completion {
            StepCompletion::Completed { facts, summary } => {
                if let Some(attempt) = state.attempts.last_mut() {
                    attempt.status = AttemptStatus::Completed;
                    attempt.completed_at = Some(at);
                    attempt.summary = summary;
                    attempt.outcome = facts.get("result.outcome").cloned();
                }
                state.status = NodeStatus::Completed;
                state.facts = facts.clone();
                state.failed_attempts = 0;
                state.routed = false;
                state.unrouted = false;
                state.reason = None;
                events.push(exec.record(
                    WorkflowEventKind::NodeCompleted,
                    Some(node_id),
                    format!("{label} completed"),
                    BTreeMap::new(),
                    at,
                ));
                if !exec.cancel_requested {
                    self.route(exec, node_id, &facts, at, &mut events);
                }
            }
            StepCompletion::Failed { message } => {
                self.step_failed(exec, node_id, &message, at, &mut events);
            }
            StepCompletion::Cancelled => {
                if let Some(attempt) = state.attempts.last_mut() {
                    attempt.status = AttemptStatus::Cancelled;
                    attempt.completed_at = Some(at);
                }
                state.status = NodeStatus::Cancelled;
            }
        }
        // A node that was activated again while it ran goes round once more.
        self.rearm(exec, node_id, at, &mut events);
        self.refresh_shared(exec);
        self.settle_into(exec, at, &mut events);
        events
    }

    /// The links a node's last ending sent the run along: the edges whose conditions its result
    /// met, or its failure route when it failed and was handed to it. This is what the
    /// orchestrator turns into handoffs.
    pub fn taken_links(&self, exec: &WorkflowExecution, node_id: &str) -> Vec<Link> {
        let Some(state) = exec.node_state(node_id) else {
            return vec![];
        };
        if exec.cancel_requested {
            return vec![];
        }
        match state.status {
            NodeStatus::Completed => self
                .graph
                .out_links(node_id)
                .filter(|l| !l.failure_route)
                .filter(|l| {
                    l.condition
                        .as_ref()
                        .is_none_or(|c| c.evaluate(&state.facts))
                })
                .cloned()
                .collect(),
            NodeStatus::Failed if state.routed => self
                .graph
                .out_links(node_id)
                .filter(|l| l.failure_route)
                .cloned()
                .collect(),
            _ => vec![],
        }
    }

    /// No new step starts; steps in flight are left to finish.
    #[allow(clippy::unused_self)] // part of the engine's one vocabulary of transitions
    pub fn pause(&self, exec: &mut WorkflowExecution, at: u64) -> Events {
        if exec.status != WorkflowExecutionStatus::Running {
            return vec![];
        }
        exec.status = WorkflowExecutionStatus::Paused;
        vec![exec.record(
            WorkflowEventKind::Paused,
            None,
            "Workflow paused",
            BTreeMap::new(),
            at,
        )]
    }

    pub fn resume(&self, exec: &mut WorkflowExecution, at: u64) -> Events {
        if exec.status != WorkflowExecutionStatus::Paused {
            return vec![];
        }
        exec.status = WorkflowExecutionStatus::Running;
        self.restore_waiting(exec);
        let mut events = vec![exec.record(
            WorkflowEventKind::Resumed,
            None,
            "Workflow resumed",
            BTreeMap::new(),
            at,
        )];
        self.settle_into(exec, at, &mut events);
        events
    }

    /// Cancels what has not started and asks for what is running to stop. The run ends as
    /// cancelled once the steps in flight have ended. Completed steps are kept as they are.
    pub fn request_cancel(&self, exec: &mut WorkflowExecution, at: u64) -> Events {
        if exec.status.is_final() {
            return vec![];
        }
        exec.cancel_requested = true;
        let mut events = Vec::new();
        // A step waiting for a person has no process to stop: the question simply lapses.
        let waiting: Vec<String> = exec
            .nodes
            .iter()
            .filter(|(_, s)| s.status == NodeStatus::WaitingForInput)
            .map(|(id, _)| id.clone())
            .collect();
        for id in waiting {
            if let Some(state) = exec.nodes.get_mut(&id) {
                state.status = NodeStatus::Cancelled;
                if let Some(attempt) = state.attempts.last_mut() {
                    attempt.status = AttemptStatus::Cancelled;
                    attempt.completed_at = Some(at);
                }
            }
            let mut lapsed = Vec::new();
            for interaction in &mut exec.interactions {
                if interaction.step_id == id && interaction.is_pending() {
                    interaction.status = InteractionStatus::Cancelled;
                    lapsed.push(interaction.id.clone());
                }
            }
            for interaction_id in lapsed {
                events.push(exec.record(
                    WorkflowEventKind::InteractionCancelled,
                    Some(&id),
                    "The question was cancelled with the run",
                    meta(&[("interactionId", interaction_id)]),
                    at,
                ));
            }
        }
        for state in exec.nodes.values_mut() {
            let interrupted = state.reason.as_deref() == Some(INTERRUPTED);
            if matches!(state.status, NodeStatus::Pending | NodeStatus::Ready)
                || (state.status == NodeStatus::Failed && interrupted)
            {
                state.status = NodeStatus::Cancelled;
            }
        }
        self.settle_into(exec, at, &mut events);
        events
    }

    /// Makes the run say what it is: the app closed while it was going. Steps that were in flight
    /// are recorded as interrupted; nothing is treated as finished.
    pub fn interrupt(&self, exec: &mut WorkflowExecution, at: u64) -> Events {
        if !exec.status.is_active() {
            return vec![];
        }
        for state in exec.nodes.values_mut() {
            // A step waiting for a person was not cut short: its question is still there.
            if state.status.is_in_flight() && state.status != NodeStatus::WaitingForInput {
                if let Some(attempt) = state.attempts.last_mut() {
                    attempt.status = AttemptStatus::Interrupted;
                    attempt.completed_at = Some(at);
                }
                state.status = NodeStatus::Failed;
                state.reason = Some(INTERRUPTED.to_owned());
                state.continuing = true;
            }
        }
        exec.status = WorkflowExecutionStatus::Interrupted;
        self.refresh_shared(exec);
        vec![exec.record(
            WorkflowEventKind::Interrupted,
            None,
            "Workflow interrupted by application shutdown",
            BTreeMap::new(),
            at,
        )]
    }

    /// Picks an interrupted run up again: the steps that were cut short are ready to start again.
    pub fn resume_interrupted(&self, exec: &mut WorkflowExecution, at: u64) -> Events {
        if exec.status != WorkflowExecutionStatus::Interrupted {
            return vec![];
        }
        exec.status = WorkflowExecutionStatus::Running;
        self.restore_waiting(exec);
        exec.cancel_requested = false;
        let mut events = vec![exec.record(
            WorkflowEventKind::Resumed,
            None,
            "Workflow resumed after an interruption",
            BTreeMap::new(),
            at,
        )];
        let cut_short: Vec<String> = exec
            .nodes
            .iter()
            .filter(|(_, s)| s.reason.as_deref() == Some(INTERRUPTED))
            .map(|(id, _)| id.clone())
            .collect();
        for id in cut_short {
            if let Some(state) = exec.nodes.get_mut(&id) {
                state.status = NodeStatus::Ready;
                state.reason = None;
                state.continuing = true;
            }
            events.push(exec.record(
                WorkflowEventKind::NodeReady,
                Some(&id),
                "Ready to run again",
                BTreeMap::new(),
                at,
            ));
        }
        self.refresh_shared(exec);
        self.settle_into(exec, at, &mut events);
        events
    }

    /// Where a failed run would go on from, worked out from the graph and what the steps
    /// reported; `None` for a run that did not fail. Nothing here knows a role: a step whose
    /// result has a route goes along it, whoever the step is.
    pub fn recovery_plan(&self, exec: &WorkflowExecution) -> Option<RecoveryPlan> {
        if exec.status != WorkflowExecutionStatus::Failed {
            return None;
        }
        let failure = exec.failure.as_ref()?;
        let order: Vec<&str> = self
            .graph
            .workflow
            .nodes
            .iter()
            .map(|n| n.id.as_str())
            .collect();
        let completed_at = |id: &str| {
            exec.node_state(id)
                .and_then(|s| s.last_attempt())
                .and_then(|a| a.completed_at)
        };
        let reused: Vec<String> = order
            .iter()
            .filter(|id| {
                exec.node_state(id)
                    .is_some_and(|s| s.status == NodeStatus::Completed && !s.attempts.is_empty())
            })
            .map(|id| (*id).to_owned())
            .collect();
        let last_completed = reused
            .iter()
            .max_by_key(|id| completed_at(id).unwrap_or(0))
            .cloned();
        let mut plan = RecoveryPlan {
            kind: RecoveryKind::Resume,
            failure_node_id: failure.node_id.clone(),
            last_completed_node_id: last_completed,
            restart_node_ids: Vec::new(),
            reused_node_ids: reused,
            problem: None,
        };
        match failure.code {
            FailureCode::NodeFailed => {
                plan.kind = RecoveryKind::Retry;
                plan.restart_node_ids = order
                    .iter()
                    .filter(|id| {
                        exec.node_state(id)
                            .is_some_and(|s| s.status == NodeStatus::Failed && !s.routed)
                    })
                    .map(|id| (*id).to_owned())
                    .collect();
            }
            FailureCode::NoRouteMatched => {
                if let Some(id) = failure.node_id.as_deref() {
                    let facts = exec.node_state(id).map(|s| s.facts.clone());
                    plan.restart_node_ids = facts.map_or_else(Vec::new, |facts| {
                        self.graph
                            .out_links(id)
                            .filter(|l| !l.failure_route)
                            .filter(|l| l.condition.as_ref().is_none_or(|c| c.evaluate(&facts)))
                            .map(|l| l.target.clone())
                            .collect()
                    });
                    if plan.restart_node_ids.is_empty() {
                        plan.problem = Some(RecoveryProblem::NoRoute);
                    }
                }
            }
            FailureCode::MaxIterationsReached => {
                let room = failure.node_id.as_deref().is_some_and(|id| {
                    let used = exec.node_state(id).map_or(0, |s| s.iterations);
                    self.graph
                        .workflow
                        .node(id)
                        .and_then(|n| n.loop_policy.as_ref())
                        .is_none_or(|p| used < p.max_iterations)
                });
                if room {
                    plan.restart_node_ids = failure.node_id.iter().cloned().collect();
                } else {
                    plan.problem = Some(RecoveryProblem::LoopLimit);
                }
            }
            _ => {}
        }
        if plan.restart_node_ids.is_empty() && plan.problem.is_none() {
            plan.problem = Some(RecoveryProblem::NotRecoverable);
        }
        Some(plan)
    }

    /// Picks a failed run up again at its Recovery Point. Steps that completed stay completed and
    /// do not run again; the steps the failure left blocked wait for their turn again. A retry
    /// runs the failed step itself again; a resume goes along the route the last result takes.
    ///
    /// # Errors
    ///
    /// Says why the run cannot go on (see [`RecoveryProblem`]); the run is left as it was.
    pub fn resume_failed(
        &self,
        exec: &mut WorkflowExecution,
        at: u64,
    ) -> Result<(RecoveryPlan, Events), RecoveryProblem> {
        let plan = self
            .recovery_plan(exec)
            .ok_or(RecoveryProblem::NotRecoverable)?;
        if let Some(problem) = plan.problem {
            return Err(problem);
        }
        let failure = exec.failure.take();
        exec.recoveries.push(RecoveryRecord {
            at,
            kind: plan.kind,
            failure: failure.clone(),
            restarted_node_ids: plan.restart_node_ids.clone(),
            reused_node_ids: plan.reused_node_ids.clone(),
            workflow_version: exec.workflow_version,
        });
        exec.status = WorkflowExecutionStatus::Running;
        exec.cancel_requested = false;
        exec.completed_at = None;
        // What the failure left unreached waits for its turn again.
        for state in exec.nodes.values_mut() {
            if matches!(state.status, NodeStatus::Blocked | NodeStatus::Cancelled)
                && (state.attempts.is_empty() || state.reason.as_deref() == Some(MAX_ITERATIONS))
            {
                state.status = NodeStatus::Pending;
                state.reason = None;
            }
        }
        let mut events = vec![exec.record(
            WorkflowEventKind::Resumed,
            None,
            "Workflow picked up again after a failure",
            meta(&[
                (
                    "kind",
                    match plan.kind {
                        RecoveryKind::Retry => "retry",
                        RecoveryKind::Resume => "resume",
                    }
                    .to_owned(),
                ),
                ("from", plan.restart_node_ids.join(",")),
            ]),
            at,
        )];
        match (plan.kind, failure.as_ref().map(|f| f.code)) {
            (RecoveryKind::Retry, _) => {
                for id in &plan.restart_node_ids {
                    if let Some(state) = exec.nodes.get_mut(id) {
                        state.status = NodeStatus::Ready;
                        state.reason = None;
                        state.failed_attempts = 0;
                        state.continuing = true;
                    }
                    events.push(exec.record(
                        WorkflowEventKind::NodeReady,
                        Some(id),
                        "Ready to run again",
                        BTreeMap::new(),
                        at,
                    ));
                }
            }
            (RecoveryKind::Resume, Some(FailureCode::NoRouteMatched)) => {
                if let Some(id) = plan.failure_node_id.as_deref() {
                    let facts = exec
                        .nodes
                        .get_mut(id)
                        .map(|s| {
                            s.unrouted = false;
                            s.facts.clone()
                        })
                        .unwrap_or_default();
                    self.route(exec, id, &facts, at, &mut events);
                }
            }
            (RecoveryKind::Resume, _) => {
                for id in &plan.restart_node_ids {
                    let activated = exec
                        .node_state(id)
                        .map(|s| s.activated_edges.clone())
                        .unwrap_or_default();
                    if !activated.is_empty() && self.satisfied(exec, id, &activated) {
                        self.make_ready(exec, id, at, &mut events);
                    }
                }
            }
        }
        self.refresh_shared(exec);
        self.settle_into(exec, at, &mut events);
        Ok((plan, events))
    }

    /// Ends the run as failed for a reason outside the graph (an invalid definition, an
    /// internal error). Steps in flight are cancelled by the caller.
    pub fn fail(&self, exec: &mut WorkflowExecution, failure: WorkflowFailure, at: u64) -> Events {
        if exec.status.is_final() {
            return vec![];
        }
        exec.failure.get_or_insert(failure);
        exec.cancel_requested = true;
        for state in exec.nodes.values_mut() {
            if matches!(state.status, NodeStatus::Pending | NodeStatus::Ready) {
                state.status = NodeStatus::Cancelled;
            }
        }
        let mut events = Vec::new();
        self.settle_into(exec, at, &mut events);
        events
    }

    // ---- internals -------------------------------------------------------------------------

    fn ready_sorted(
        &self,
        exec: &WorkflowExecution,
    ) -> Vec<&'a crate::domain::workflow::WorkflowNode> {
        let mut ready: Vec<&crate::domain::workflow::WorkflowNode> = self
            .graph
            .workflow
            .nodes
            .iter()
            .filter(|n| {
                exec.node_state(&n.id)
                    .is_some_and(|s| s.status == NodeStatus::Ready)
            })
            .collect();
        ready.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| self.graph.index_of(&a.id).cmp(&self.graph.index_of(&b.id)))
                .then_with(|| a.id.cmp(&b.id))
        });
        ready
    }

    fn make_ready(&self, exec: &mut WorkflowExecution, id: &str, at: u64, events: &mut Events) {
        let Some(state) = exec.nodes.get_mut(id) else {
            return;
        };
        if state.status.is_in_flight() || state.status == NodeStatus::Ready {
            return;
        }
        state.status = NodeStatus::Ready;
        state.reason = None;
        let label = self
            .graph
            .workflow
            .node(id)
            .map_or_else(String::new, |n| n.label().to_owned());
        events.push(exec.record(
            WorkflowEventKind::NodeReady,
            Some(id),
            format!("{label} is ready"),
            BTreeMap::new(),
            at,
        ));
    }

    /// A link out of a step that failed and was handed to its failure route never fires: the
    /// route took over, so a join does not wait for it.
    fn handed_to_failure_route(exec: &WorkflowExecution, link: &Link) -> bool {
        !link.failure_route && exec.node_state(&link.source).is_some_and(|s| s.routed)
    }

    /// Whether the links that have fired towards `node` activate it.
    fn satisfied(
        &self,
        exec: &WorkflowExecution,
        node: &str,
        activated: &BTreeSet<String>,
    ) -> bool {
        let ins: Vec<&Link> = self.graph.in_links(node).collect();
        if ins
            .iter()
            .any(|l| self.graph.is_internal(l) && activated.contains(&l.id))
        {
            return true;
        }
        let entry: Vec<&&Link> = ins.iter().filter(|l| !self.graph.is_internal(l)).collect();
        if entry.is_empty() {
            return false;
        }
        let (alternatives, required): (Vec<&&&Link>, Vec<&&&Link>) =
            entry.iter().partition(|l| l.is_alternative());
        // Unconditional edges are requirements (all of them); conditional ones and failure
        // routes are alternatives (any of them). Either group is enough on its own.
        let required_met = !required.is_empty()
            && required
                .iter()
                .all(|l| activated.contains(&l.id) || Self::handed_to_failure_route(exec, l))
            && required.iter().any(|l| activated.contains(&l.id));
        let alternative_met = alternatives.iter().any(|l| activated.contains(&l.id));
        required_met || alternative_met
    }

    /// The link fires: the target is activated, and ready if that is enough.
    fn fire(&self, exec: &mut WorkflowExecution, link: &Link, at: u64, events: &mut Events) {
        let Some(state) = exec.nodes.get_mut(&link.target) else {
            return;
        };
        state.activated_edges.insert(link.id.clone());
        let activated = state.activated_edges.clone();
        let status = state.status;
        if !status.is_in_flight()
            && status != NodeStatus::Ready
            && self.satisfied(exec, &link.target, &activated)
        {
            self.make_ready(exec, &link.target, at, events);
        }
    }

    fn rearm(&self, exec: &mut WorkflowExecution, id: &str, at: u64, events: &mut Events) {
        let Some(state) = exec.nodes.get(id) else {
            return;
        };
        if state.status.is_settled()
            && !state.activated_edges.is_empty()
            && self.satisfied(exec, id, &state.activated_edges)
        {
            self.make_ready(exec, id, at, events);
        }
    }

    fn route(
        &self,
        exec: &mut WorkflowExecution,
        node_id: &str,
        facts: &Facts,
        at: u64,
        events: &mut Events,
    ) {
        let links: Vec<Link> = self
            .graph
            .out_links(node_id)
            .filter(|l| !l.failure_route)
            .cloned()
            .collect();
        let mut fired = false;
        for link in &links {
            if link.condition.as_ref().is_none_or(|c| c.evaluate(facts)) {
                fired = true;
                self.fire(exec, link, at, events);
            }
        }
        if !links.is_empty() && !fired {
            if let Some(state) = exec.nodes.get_mut(node_id) {
                state.unrouted = true;
            }
        }
    }

    fn step_failed(
        &self,
        exec: &mut WorkflowExecution,
        node_id: &str,
        message: &str,
        at: u64,
        events: &mut Events,
    ) {
        let Some(node) = self.graph.workflow.node(node_id) else {
            return;
        };
        let max_retries = node.agent().map_or(0, |a| a.retry_policy.max_retries);
        let label = node.label().to_owned();
        let cancelling = exec.cancel_requested;
        let state = exec.nodes.get_mut(node_id).expect("node state exists");
        if let Some(attempt) = state.attempts.last_mut() {
            attempt.status = AttemptStatus::Failed;
            attempt.completed_at = Some(at);
            attempt.failure = Some(message.to_owned());
        }
        state.failed_attempts += 1;
        if !cancelling && state.failed_attempts <= max_retries {
            state.status = NodeStatus::Ready;
            state.continuing = true;
            let next = state.attempts.len() + 1;
            events.push(exec.record(
                WorkflowEventKind::NodeRetrying,
                Some(node_id),
                format!("{label} failed and will be retried"),
                meta(&[("attempt", next.to_string()), ("error", message.to_owned())]),
                at,
            ));
            return;
        }
        state.status = NodeStatus::Failed;
        state.failed_attempts = 0;
        state.reason = Some(message.to_owned());
        events.push(exec.record(
            WorkflowEventKind::NodeFailed,
            Some(node_id),
            format!("{label} failed"),
            meta(&[("error", message.to_owned())]),
            at,
        ));
        if cancelling {
            return;
        }
        let route = self
            .graph
            .out_links(node_id)
            .find(|l| l.failure_route)
            .cloned();
        if let Some(link) = route {
            if let Some(state) = exec.nodes.get_mut(node_id) {
                state.routed = true;
            }
            self.fire(exec, &link, at, events);
        }
    }

    /// A node is about to start a pass. Counts the iteration and checks the loop limit; a limit
    /// reached fails the run.
    fn enter(
        &self,
        exec: &mut WorkflowExecution,
        node_id: &str,
        at: u64,
        events: &mut Events,
    ) -> bool {
        let node = self.graph.workflow.node(node_id).expect("node exists");
        let state = exec.nodes.get_mut(node_id).expect("node state exists");
        if state.continuing {
            state.continuing = false;
            return true;
        }
        if let Some(policy) = &node.loop_policy {
            if state.iterations >= policy.max_iterations {
                state.status = NodeStatus::Blocked;
                state.reason = Some(MAX_ITERATIONS.to_owned());
                events.push(exec.record(
                    WorkflowEventKind::NodeBlocked,
                    Some(node_id),
                    format!("{} reached its loop limit", node.label()),
                    meta(&[
                        ("loopId", policy.loop_id.clone()),
                        ("max", policy.max_iterations.to_string()),
                    ]),
                    at,
                ));
                exec.failure.get_or_insert(WorkflowFailure {
                    code: FailureCode::MaxIterationsReached,
                    node_id: Some(node_id.to_owned()),
                    detail: Some(policy.loop_id.clone()),
                });
                exec.cancel_requested = true;
                for other in exec.nodes.values_mut() {
                    if matches!(other.status, NodeStatus::Pending | NodeStatus::Ready) {
                        other.status = NodeStatus::Cancelled;
                    }
                }
                return false;
            }
        }
        let state = exec.nodes.get_mut(node_id).expect("node state exists");
        state.iterations += 1;
        state.activated_edges.clear();
        if let Some(policy) = &node.loop_policy {
            let count = state.iterations;
            exec.state
                .iteration_count
                .insert(policy.loop_id.clone(), count);
        }
        true
    }

    fn refresh_shared(&self, exec: &mut WorkflowExecution) {
        let nodes = &self.graph.workflow.nodes;
        let order = |a: &String| nodes.iter().position(|n| &n.id == a).unwrap_or(usize::MAX);
        let mut current: Vec<String> = Vec::new();
        let mut completed: Vec<String> = Vec::new();
        let mut failed: Vec<String> = Vec::new();
        let mut active = BTreeSet::new();
        let mut done_agents = BTreeSet::new();
        for node in nodes {
            let Some(state) = exec.nodes.get(&node.id) else {
                continue;
            };
            match state.status {
                s if s.is_in_flight() => {
                    current.push(node.id.clone());
                    if let Some(agent) = node.agent_id() {
                        active.insert(agent.to_owned());
                    }
                }
                NodeStatus::Completed => {
                    completed.push(node.id.clone());
                    if let Some(agent) = node.agent_id() {
                        done_agents.insert(agent.to_owned());
                    }
                }
                NodeStatus::Failed => failed.push(node.id.clone()),
                _ => {}
            }
        }
        current.sort_by_key(order);
        exec.state.current_nodes = current;
        exec.state.completed_nodes = completed;
        exec.state.failed_nodes = failed;
        exec.state.active_agents = active;
        exec.state.completed_agents = done_agents;
    }

    fn can_run_again(&self, exec: &WorkflowExecution, node: &str) -> bool {
        self.graph.cycle_of(node).iter().any(|member| {
            exec.node_state(member).is_some_and(|s| {
                matches!(
                    s.status,
                    NodeStatus::Pending
                        | NodeStatus::Ready
                        | NodeStatus::Running
                        | NodeStatus::WaitingApproval
                        | NodeStatus::WaitingForInput
                )
            })
        })
    }

    /// The link has fired, or may still fire.
    fn link_alive(
        &self,
        exec: &WorkflowExecution,
        link: &Link,
        target_activated: &BTreeSet<String>,
    ) -> bool {
        if target_activated.contains(&link.id) || Self::handed_to_failure_route(exec, link) {
            return true;
        }
        let Some(source) = exec.node_state(&link.source) else {
            return false;
        };
        !source.status.is_settled() || self.can_run_again(exec, &link.source)
    }

    /// Whether a pending node can still be activated.
    fn can_still_activate(&self, exec: &WorkflowExecution, node: &str) -> bool {
        let activated = exec
            .node_state(node)
            .map(|s| s.activated_edges.clone())
            .unwrap_or_default();
        let ins: Vec<&Link> = self.graph.in_links(node).collect();
        if ins
            .iter()
            .any(|l| self.graph.is_internal(l) && self.link_alive(exec, l, &activated))
        {
            return true;
        }
        let entry: Vec<&&Link> = ins.iter().filter(|l| !self.graph.is_internal(l)).collect();
        if entry.is_empty() {
            return false;
        }
        let (alternatives, required): (Vec<&&&Link>, Vec<&&&Link>) =
            entry.iter().partition(|l| l.is_alternative());
        let required_alive = !required.is_empty()
            && required
                .iter()
                .all(|l| self.link_alive(exec, l, &activated));
        let alternative_alive = alternatives
            .iter()
            .any(|l| self.link_alive(exec, l, &activated));
        required_alive || alternative_alive
    }

    /// Blocks or skips pending nodes whose activation can no longer happen, to a fixpoint.
    fn propagate_blocking(
        &self,
        exec: &mut WorkflowExecution,
        at: u64,
        events: &mut Events,
    ) -> bool {
        let mut changed = false;
        loop {
            let mut progress = false;
            for node in &self.graph.workflow.nodes {
                let Some(state) = exec.node_state(&node.id) else {
                    continue;
                };
                if state.status != NodeStatus::Pending
                    || self.graph.in_links(&node.id).next().is_none()
                    || self.can_still_activate(exec, &node.id)
                {
                    continue;
                }
                let failed_dependency = self.graph.in_links(&node.id).any(|l| {
                    exec.node_state(&l.source).is_some_and(|s| {
                        matches!(
                            s.status,
                            NodeStatus::Failed | NodeStatus::Blocked | NodeStatus::Cancelled
                        )
                    })
                });
                let (status, kind, reason) = if failed_dependency {
                    (
                        NodeStatus::Blocked,
                        WorkflowEventKind::NodeBlocked,
                        DEPENDENCY_FAILED,
                    )
                } else {
                    (
                        NodeStatus::Skipped,
                        WorkflowEventKind::NodeSkipped,
                        SKIPPED_BY_ROUTE,
                    )
                };
                let id = node.id.clone();
                if let Some(state) = exec.nodes.get_mut(&id) {
                    state.status = status;
                    state.reason = Some(reason.to_owned());
                }
                events.push(exec.record(
                    kind,
                    Some(&id),
                    format!(
                        "{} {}",
                        node.label(),
                        if failed_dependency {
                            "is blocked"
                        } else {
                            "was skipped"
                        }
                    ),
                    meta(&[("reason", reason.to_owned())]),
                    at,
                ));
                progress = true;
            }
            if !progress {
                break;
            }
            changed = true;
        }
        changed
    }

    /// Runs the nodes that need no agent, in the order the scheduler would pick them.
    fn resolve_instant(&self, exec: &mut WorkflowExecution, at: u64, events: &mut Events) -> bool {
        let mut changed = false;
        loop {
            let next = self
                .ready_sorted(exec)
                .into_iter()
                .find(|n| !matches!(n.kind, NodeKind::Agent(_)))
                .map(|n| n.id.clone());
            let Some(id) = next else { break };
            if exec.status != WorkflowExecutionStatus::Running || exec.cancel_requested {
                break;
            }
            changed = true;
            self.run_instant(exec, &id, at, events);
        }
        changed
    }

    fn run_instant(&self, exec: &mut WorkflowExecution, id: &str, at: u64, events: &mut Events) {
        let node = self.graph.workflow.node(id).expect("node exists");
        let activated = exec
            .node_state(id)
            .map(|s| s.activated_edges.clone())
            .unwrap_or_default();
        if !self.enter(exec, id, at, events) {
            return;
        }
        match &node.kind {
            NodeKind::Condition(condition) => {
                // The facts of the step that led here: the first activating link, in link order.
                let upstream = self
                    .graph
                    .in_links(id)
                    .find(|l| activated.contains(&l.id))
                    .and_then(|l| exec.node_state(&l.source))
                    .map(|s| s.facts.clone())
                    .unwrap_or_default();
                let matched = condition.condition.evaluate(&upstream);
                let mut facts = Facts::new();
                facts.insert("result.matched".to_owned(), matched.to_string());
                if let Some(state) = exec.nodes.get_mut(id) {
                    state.status = NodeStatus::Completed;
                    state.facts = facts.clone();
                }
                events.push(exec.record(
                    WorkflowEventKind::NodeCompleted,
                    Some(id),
                    format!("{} evaluated to {matched}", condition.label),
                    meta(&[("matched", matched.to_string())]),
                    at,
                ));
                self.route(exec, id, &facts, at, events);
            }
            NodeKind::End(end) => {
                if let Some(state) = exec.nodes.get_mut(id) {
                    state.status = NodeStatus::Completed;
                    state.reason = Some(
                        match end.outcome {
                            EndOutcome::Done => END_DONE,
                            EndOutcome::Failed => END_FAILED,
                            EndOutcome::Cancelled => END_CANCELLED,
                        }
                        .to_owned(),
                    );
                }
                events.push(exec.record(
                    WorkflowEventKind::NodeCompleted,
                    Some(id),
                    format!("{} reached", end.label),
                    BTreeMap::new(),
                    at,
                ));
            }
            NodeKind::Agent(_) => {}
        }
        self.refresh_shared(exec);
    }

    fn settle_into(&self, exec: &mut WorkflowExecution, at: u64, events: &mut Events) {
        loop {
            let resolved = self.resolve_instant(exec, at, events);
            let blocked = self.propagate_blocking(exec, at, events);
            if !resolved && !blocked {
                break;
            }
        }
        self.finalize(exec, at, events);
    }

    fn finalize(&self, exec: &mut WorkflowExecution, at: u64, events: &mut Events) {
        let finishable = exec.status.is_active()
            || (exec.status == WorkflowExecutionStatus::Interrupted && exec.cancel_requested);
        if !finishable || exec.in_flight() > 0 {
            return;
        }
        if exec.cancel_requested {
            for state in exec.nodes.values_mut() {
                if matches!(state.status, NodeStatus::Pending | NodeStatus::Ready) {
                    state.status = NodeStatus::Cancelled;
                }
            }
            if exec.failure.is_some() {
                self.end(exec, WorkflowExecutionStatus::Failed, at, events);
            } else {
                self.end(exec, WorkflowExecutionStatus::Cancelled, at, events);
            }
            return;
        }
        if exec.status == WorkflowExecutionStatus::Paused || exec.count(NodeStatus::Ready) > 0 {
            return;
        }
        let ended = |reason: &str| {
            exec.nodes
                .values()
                .any(|s| s.status == NodeStatus::Completed && s.reason.as_deref() == Some(reason))
        };
        let failed_node = self.graph.workflow.nodes.iter().find(|n| {
            exec.node_state(&n.id)
                .is_some_and(|s| s.status == NodeStatus::Failed && !s.routed)
        });
        let unrouted_node = self.graph.workflow.nodes.iter().find(|n| {
            exec.node_state(&n.id)
                .is_some_and(|s| s.status == NodeStatus::Completed && s.unrouted)
        });
        let (status, failure) = if let Some(node) = failed_node {
            (
                WorkflowExecutionStatus::Failed,
                Some(WorkflowFailure {
                    code: FailureCode::NodeFailed,
                    node_id: Some(node.id.clone()),
                    detail: exec.node_state(&node.id).and_then(|s| s.reason.clone()),
                }),
            )
        } else if let Some(node) = unrouted_node {
            (
                WorkflowExecutionStatus::Failed,
                Some(WorkflowFailure {
                    code: FailureCode::NoRouteMatched,
                    node_id: Some(node.id.clone()),
                    // The outcome nothing routes, so the message can name it.
                    detail: exec
                        .node_state(&node.id)
                        .and_then(|s| s.facts.get("result.outcome").cloned()),
                }),
            )
        } else if ended(END_FAILED) {
            (
                WorkflowExecutionStatus::Failed,
                Some(WorkflowFailure {
                    code: FailureCode::EndedInFailure,
                    node_id: None,
                    detail: None,
                }),
            )
        } else if ended(END_DONE) {
            (WorkflowExecutionStatus::Completed, None)
        } else if ended(END_CANCELLED) {
            (WorkflowExecutionStatus::Cancelled, None)
        } else {
            (
                WorkflowExecutionStatus::Failed,
                Some(WorkflowFailure {
                    code: FailureCode::NoPathToCompletion,
                    node_id: None,
                    detail: None,
                }),
            )
        };
        // What was never reached is skipped in a run that succeeded and blocked in one that did
        // not: it never ran, but only the second explains why.
        let leftover = if status == WorkflowExecutionStatus::Completed {
            NodeStatus::Skipped
        } else {
            NodeStatus::Blocked
        };
        for state in exec.nodes.values_mut() {
            if state.status == NodeStatus::Pending {
                state.status = leftover;
                state.reason = Some(NOT_REACHED.to_owned());
            }
        }
        if exec.failure.is_none() {
            exec.failure = failure;
        }
        self.end(exec, status, at, events);
    }

    fn end(
        &self,
        exec: &mut WorkflowExecution,
        status: WorkflowExecutionStatus,
        at: u64,
        events: &mut Events,
    ) {
        exec.status = status;
        exec.completed_at = Some(at);
        self.refresh_shared(exec);
        let (kind, text) = match status {
            WorkflowExecutionStatus::Completed => {
                (WorkflowEventKind::Completed, "Workflow completed")
            }
            WorkflowExecutionStatus::Cancelled => {
                (WorkflowEventKind::Cancelled, "Workflow cancelled")
            }
            _ => (WorkflowEventKind::Failed, "Workflow failed"),
        };
        let mut details = BTreeMap::new();
        if let Some(failure) = &exec.failure {
            details.insert(
                "code".to_owned(),
                serde_json::to_value(failure.code)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default(),
            );
        }
        events.push(exec.record(kind, None, text, details, at));
    }
}

/// Reasons recorded on nodes. Stable codes the UI words.
pub const INTERRUPTED: &str = "interrupted";
pub const MAX_ITERATIONS: &str = "max_iterations";
pub const DEPENDENCY_FAILED: &str = "dependency_failed";
pub const SKIPPED_BY_ROUTE: &str = "skipped_by_route";
pub const NOT_REACHED: &str = "not_reached";
pub const END_DONE: &str = "end_done";
pub const END_FAILED: &str = "end_failed";
pub const END_CANCELLED: &str = "end_cancelled";
