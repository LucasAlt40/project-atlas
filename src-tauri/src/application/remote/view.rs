//! What the phone is shown of a run: a projection built for reading on a small screen, not the
//! stored `WorkflowExecution`.
//!
//! The projection is where privacy is decided (ADR 0034 §9). It carries names, never folders;
//! text that agents wrote passes through the same secret redaction the Harness uses; raw terminal
//! output, diffs and worktree paths are not here at all. Progress is only what the engine knows:
//! how many steps completed out of how many the graph has, never a percentage.

use serde::Serialize;

use super::policy::{rule_for, RemoteRule};
use crate::application::harness::secrets::redact_secrets;
use crate::domain::interaction::{
    InteractionKind, InteractionOption, InteractionStatus, PendingInteraction,
};
use crate::domain::workflow::{
    AttemptStatus, FailureCode, IntegrationStatus, NodeKind, NodeStatus, WorkflowEvent,
    WorkflowExecution, WorkflowExecutionStatus,
};

const TASK_MAX: usize = 400;
const SUMMARY_MAX: usize = 1_200;
const EVENT_MAX: usize = 300;
const DOCUMENT_MAX: usize = 20_000;
const EVENTS_SHOWN: usize = 80;
const ARTIFACTS_SHOWN: usize = 20;
const DECISIONS_SHOWN: usize = 10;

/// Names the projection needs that the run does not carry.
pub struct Names<'a> {
    pub workspace: &'a str,
    pub agent: &'a dyn Fn(&str) -> Option<String>,
}

/// Why a run is asking for a person, in the order that matters to them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Attention {
    None,
    /// A question the phone can answer.
    NeedsAnswer,
    /// Something waits that only the computer can do (a command approval, a guardrail question,
    /// a grant, an interrupted run).
    NeedsComputer,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepBrief {
    pub node_id: String,
    pub label: String,
    pub status: NodeStatus,
    pub agent: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FailureView {
    pub code: FailureCode,
    pub step: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub id: String,
    pub workflow_name: String,
    pub workspace_id: String,
    pub workspace_name: String,
    pub task: String,
    pub status: WorkflowExecutionStatus,
    pub attention: Attention,
    /// Questions waiting, of any kind.
    pub pending: u32,
    /// The steps being worked on (or waiting) now.
    pub current: Vec<StepBrief>,
    /// Steps of the graph that are agents, and how many completed. A count, not a percentage: a
    /// loop or a branch not taken makes "how far along" unknowable.
    pub steps_total: u32,
    pub steps_done: u32,
    pub started_at: u64,
    pub updated_at: u64,
    pub completed_at: Option<u64>,
    pub failure: Option<FailureView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptView {
    pub attempt: u32,
    pub iteration: u32,
    pub status: AttemptStatus,
    pub started_at: u64,
    pub completed_at: Option<u64>,
    pub summary: Option<String>,
    pub outcome: Option<String>,
    pub failure: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepView {
    pub node_id: String,
    /// `agent`, `condition` or `end`.
    pub kind: &'static str,
    pub label: String,
    pub status: NodeStatus,
    pub agent: Option<String>,
    pub iterations: u32,
    pub reason: Option<String>,
    pub started_at: Option<u64>,
    pub completed_at: Option<u64>,
    pub attempts: Vec<AttemptView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeView {
    pub from: String,
    pub to: String,
    pub label: String,
    /// The edge is taken only for some results.
    pub conditional: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionView {
    pub id: String,
    pub node_id: String,
    pub step: String,
    pub agent: Option<String>,
    pub kind: InteractionKind,
    pub status: InteractionStatus,
    pub question: String,
    pub context: String,
    /// The plan or analysis the question is about, in Markdown.
    pub document: String,
    pub options: Vec<InteractionOption>,
    pub created_at: u64,
    pub answered_at: Option<u64>,
    pub choice: Option<String>,
    pub answer: Option<String>,
    /// Atlas answered it because the person had asked for plans to be approved automatically.
    pub auto: bool,
    /// What the phone may do about it.
    pub remote: RemoteRule,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventView {
    pub kind: String,
    pub step: Option<String>,
    pub message: String,
    pub at: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactView {
    pub name: String,
    pub kind: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionView {
    pub title: String,
    pub decision: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultView {
    /// What the last step to finish said.
    pub summary: Option<String>,
    pub artifacts: Vec<ArtifactView>,
    pub decisions: Vec<DecisionView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeView {
    pub status: IntegrationStatus,
    pub files_changed: u32,
    pub additions: u32,
    pub deletions: u32,
}

/// What the phone may ask of the run as a whole.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Controls {
    pub pause: bool,
    pub resume: bool,
    pub cancel: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunDetail {
    #[serde(flatten)]
    pub summary: RunSummary,
    pub steps: Vec<StepView>,
    pub edges: Vec<EdgeView>,
    pub interactions: Vec<InteractionView>,
    pub events: Vec<EventView>,
    pub result: ResultView,
    pub code: CodeView,
    pub auto_approve_plans: bool,
    pub controls: Controls,
}

/// Text from an agent, without what looks like a secret, and bounded.
pub fn safe(text: &str, max: usize) -> String {
    let (redacted, _) = redact_secrets(text);
    clip(&redacted, max)
}

fn clip(text: &str, max: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max {
        return trimmed.to_owned();
    }
    let mut cut: String = trimmed.chars().take(max).collect();
    cut.push('…');
    cut
}

fn kind_label(node: &NodeKind) -> &'static str {
    match node {
        NodeKind::Agent(_) => "agent",
        NodeKind::Condition(_) => "condition",
        NodeKind::End(_) => "end",
    }
}

pub fn summary(exec: &WorkflowExecution, names: &Names<'_>) -> RunSummary {
    let step_label = |node_id: &str| {
        exec.workflow
            .nodes
            .iter()
            .find(|n| n.id == node_id)
            .map(|n| safe(n.label(), 120))
    };
    let agents: Vec<_> = exec
        .workflow
        .nodes
        .iter()
        .filter(|n| n.agent().is_some())
        .collect();
    let done = agents
        .iter()
        .filter(|n| {
            exec.node_state(&n.id)
                .is_some_and(|s| s.status == NodeStatus::Completed)
        })
        .count();
    let current = exec
        .workflow
        .nodes
        .iter()
        .filter_map(|node| {
            let state = exec.node_state(&node.id)?;
            let live = state.status.is_in_flight()
                || (state.status == NodeStatus::Ready && exec.status.is_active());
            live.then(|| StepBrief {
                node_id: node.id.clone(),
                label: safe(node.label(), 120),
                status: state.status,
                agent: node
                    .agent_id()
                    .and_then(|id| (names.agent)(id))
                    .map(|n| safe(&n, 80)),
            })
        })
        .collect();
    let pending: Vec<&PendingInteraction> = exec
        .interactions
        .iter()
        .filter(|i| i.is_pending())
        .collect();
    let waiting_command_approval = exec
        .nodes
        .values()
        .any(|s| s.status == NodeStatus::WaitingApproval);
    let attention = if pending.iter().any(|i| rule_for(i).can_answer) {
        Attention::NeedsAnswer
    } else if !pending.is_empty()
        || waiting_command_approval
        || exec.status == WorkflowExecutionStatus::Interrupted
    {
        Attention::NeedsComputer
    } else if exec.status == WorkflowExecutionStatus::Failed {
        Attention::Failed
    } else {
        Attention::None
    };
    RunSummary {
        id: exec.id.clone(),
        workflow_name: safe(&exec.workflow.name, 120),
        workspace_id: exec.workspace_id.clone(),
        workspace_name: safe(names.workspace, 120),
        task: safe(&exec.task, TASK_MAX),
        status: exec.status,
        attention,
        pending: u32::try_from(pending.len()).unwrap_or(u32::MAX),
        current,
        steps_total: u32::try_from(agents.len()).unwrap_or(u32::MAX),
        steps_done: u32::try_from(done).unwrap_or(u32::MAX),
        started_at: exec.started_at,
        updated_at: exec.updated_at,
        completed_at: exec.completed_at,
        failure: exec.failure.as_ref().map(|f| FailureView {
            code: f.code,
            step: f.node_id.as_deref().and_then(step_label),
            detail: f.detail.as_deref().map(|d| safe(d, EVENT_MAX)),
        }),
    }
}

fn interaction_view(i: &PendingInteraction, names: &Names<'_>) -> InteractionView {
    InteractionView {
        id: i.id.clone(),
        node_id: i.step_id.clone(),
        step: safe(&i.step_label, 120),
        agent: (names.agent)(&i.agent_id).map(|n| safe(&n, 80)),
        kind: i.kind,
        status: i.status,
        question: safe(&i.question, 600),
        context: safe(&i.context, 800),
        document: safe(&i.document, DOCUMENT_MAX),
        options: i.options.clone(),
        created_at: i.created_at,
        answered_at: i.answered_at,
        choice: i.choice.clone(),
        answer: i.answer.as_deref().map(|a| safe(a, 2_000)),
        auto: i.auto,
        remote: rule_for(i),
    }
}

fn event_view(exec: &WorkflowExecution, event: &WorkflowEvent) -> EventView {
    EventView {
        kind: event
            .kind
            .wire_name()
            .trim_start_matches("workflow:")
            .to_owned(),
        step: event.node_id.as_deref().and_then(|id| {
            exec.workflow
                .nodes
                .iter()
                .find(|n| n.id == id)
                .map(|n| safe(n.label(), 120))
        }),
        message: safe(&event.message, EVENT_MAX),
        at: event.timestamp,
    }
}

#[allow(clippy::too_many_lines)] // one projection, field by field
pub fn detail(exec: &WorkflowExecution, names: &Names<'_>) -> RunDetail {
    let mut steps: Vec<StepView> = exec
        .workflow
        .nodes
        .iter()
        .filter_map(|node| {
            let state = exec.node_state(&node.id)?;
            let attempts: Vec<AttemptView> = state
                .attempts
                .iter()
                .map(|a| AttemptView {
                    attempt: a.attempt,
                    iteration: a.iteration,
                    status: a.status,
                    started_at: a.started_at,
                    completed_at: a.completed_at,
                    summary: a.summary.as_deref().map(|s| safe(s, SUMMARY_MAX)),
                    outcome: a.outcome.clone(),
                    failure: a.failure.as_deref().map(|f| safe(f, EVENT_MAX)),
                })
                .collect();
            Some(StepView {
                node_id: node.id.clone(),
                kind: kind_label(&node.kind),
                label: safe(node.label(), 120),
                status: state.status,
                agent: node
                    .agent_id()
                    .and_then(|id| (names.agent)(id))
                    .map(|n| safe(&n, 80)),
                iterations: state.iterations,
                reason: state.reason.clone(),
                started_at: attempts.first().map(|a| a.started_at),
                completed_at: attempts.last().and_then(|a| a.completed_at),
                attempts,
            })
        })
        .collect();
    // What began, in the order it began; what has not, in the order the workflow was drawn.
    // `sort_by_key` is stable, so the drawn order breaks ties between steps that began together.
    steps.sort_by_key(|s| s.started_at.unwrap_or(u64::MAX));

    let edges = exec
        .workflow
        .edges
        .iter()
        .map(|e| EdgeView {
            from: e.source_node_id.clone(),
            to: e.target_node_id.clone(),
            label: safe(&e.label, 80),
            conditional: e.condition.is_some(),
        })
        .collect();

    let last_summary = exec
        .nodes
        .values()
        .flat_map(|s| s.attempts.iter())
        .filter(|a| a.status == AttemptStatus::Completed)
        .max_by_key(|a| a.completed_at.unwrap_or(0))
        .and_then(|a| a.summary.as_deref())
        .map(|s| safe(s, SUMMARY_MAX))
        .filter(|s| !s.is_empty());

    let result = ResultView {
        summary: last_summary,
        artifacts: exec
            .state
            .artifacts
            .iter()
            .take(ARTIFACTS_SHOWN)
            .map(|a| ArtifactView {
                name: safe(&a.name, 120),
                kind: serde_json::to_value(a.kind)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default(),
                summary: safe(&a.summary, 400),
            })
            .collect(),
        decisions: exec
            .state
            .decisions
            .iter()
            .take(DECISIONS_SHOWN)
            .map(|d| DecisionView {
                title: safe(&d.title, 120),
                decision: safe(&d.decision, 400),
            })
            .collect(),
    };

    let skip = exec.events.len().saturating_sub(EVENTS_SHOWN);
    RunDetail {
        summary: summary(exec, names),
        steps,
        edges,
        interactions: exec
            .interactions
            .iter()
            .map(|i| interaction_view(i, names))
            .collect(),
        events: exec
            .events
            .iter()
            .skip(skip)
            .map(|e| event_view(exec, e))
            .collect(),
        result,
        code: CodeView {
            status: exec.integration.status,
            files_changed: exec.changes.as_ref().map_or(0, |c| c.files_changed),
            additions: exec.changes.as_ref().map_or(0, |c| c.additions),
            deletions: exec.changes.as_ref().map_or(0, |c| c.deletions),
        },
        auto_approve_plans: exec.auto_approve_plans,
        controls: Controls {
            pause: exec.status == WorkflowExecutionStatus::Running,
            resume: exec.status == WorkflowExecutionStatus::Paused,
            cancel: exec.status.is_active(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_from_an_agent_loses_what_looks_like_a_secret() {
        let shown = safe(
            "read .env\nAPI_KEY=sk-abcdefghijklmnopqrstuv\nall good",
            500,
        );
        assert!(!shown.contains("sk-abcdef"));
        assert!(shown.contains("all good"));
    }

    #[test]
    fn long_text_is_cut_with_a_mark() {
        let shown = safe(&"x".repeat(100), 10);
        assert_eq!(shown.chars().count(), 11);
        assert!(shown.ends_with('…'));
    }
}
