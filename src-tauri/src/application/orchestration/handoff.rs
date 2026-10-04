//! Turns what a step reported into the workflow's shared state: artifacts, decisions, validation
//! results and where the step touched the project, flagging steps that could run side by side
//! but touched the same place. Detects and reports; it never tries to resolve a conflict.

use std::collections::{BTreeMap, BTreeSet};

use crate::application::workflow::graph::{Graph, Link};
use crate::application::worktree::StepDelta;
use crate::domain::orchestration::{
    AgentResult, Artifact, ArtifactType, Decision, OverlapWarning, ResultStatus, ValidationEntry,
};
use crate::domain::workflow::{
    AgentHandoff, HandoffArtifact, HandoffDecision, HandoffKind, HandoffValidation, WorkflowEvent,
    WorkflowEventKind, WorkflowExecution,
};

/// What the step was, to put its output in the right place.
pub struct StepReport<'a> {
    pub node_id: &'a str,
    pub node_label: &'a str,
    pub execution_id: &'a str,
    /// The kind of artifact to record when the agent described none.
    pub default_artifact: ArtifactType,
    pub result: &'a AgentResult,
    /// The files Git measured the step changing in the shared worktree, when it has one. They
    /// replace what the agent claimed: for the code, Git is the source of truth.
    pub measured_files: Option<Vec<String>>,
}

pub fn record_result(
    exec: &mut WorkflowExecution,
    graph: &Graph<'_>,
    report: &StepReport<'_>,
    at: u64,
) -> Vec<WorkflowEvent> {
    let mut events = Vec::new();
    let result = report.result;

    for draft in &result.artifacts {
        let artifact = Artifact {
            id: format!("artifact-{}", exec.state.artifacts.len() + 1),
            kind: ArtifactType::parse(&draft.kind),
            name: draft.name.clone(),
            producer_node_id: report.node_id.to_owned(),
            execution_id: report.execution_id.to_owned(),
            path: draft.path.clone(),
            summary: draft.summary.clone(),
            metadata: BTreeMap::new(),
            created_at: at,
        };
        events.push(artifact_event(exec, &artifact, at));
        exec.state.artifacts.push(artifact);
    }
    if result.artifacts.is_empty() && !result.summary.is_empty() {
        let artifact = Artifact {
            id: format!("artifact-{}", exec.state.artifacts.len() + 1),
            kind: report.default_artifact,
            name: report.node_label.to_owned(),
            producer_node_id: report.node_id.to_owned(),
            execution_id: report.execution_id.to_owned(),
            path: None,
            summary: result.summary.clone(),
            metadata: BTreeMap::from([("source".to_owned(), "summary".to_owned())]),
            created_at: at,
        };
        events.push(artifact_event(exec, &artifact, at));
        exec.state.artifacts.push(artifact);
    }

    for draft in &result.decisions {
        let decision = Decision {
            id: format!("decision-{}", exec.state.decisions.len() + 1),
            title: draft.title.clone(),
            decision: draft.decision.clone(),
            rationale: draft.rationale.clone(),
            source_node_id: report.node_id.to_owned(),
            created_at: at,
        };
        events.push(exec.record(
            WorkflowEventKind::DecisionCreated,
            Some(report.node_id),
            format!("Decision: {}", decision.title),
            BTreeMap::from([("decisionId".to_owned(), decision.id.clone())]),
            at,
        ));
        exec.state.decisions.push(decision);
    }

    let validating = matches!(
        result.status,
        ResultStatus::Pass | ResultStatus::Fail | ResultStatus::Warning
    ) || !result.findings.is_empty();
    if validating {
        // Only the latest verdict of a node matters to whoever has to act on it.
        exec.state
            .validation_results
            .retain(|entry| entry.node_id != report.node_id);
        exec.state.validation_results.push(ValidationEntry {
            node_id: report.node_id.to_owned(),
            execution_id: report.execution_id.to_owned(),
            status: result.status,
            outcome: result.outcome.clone(),
            summary: result.summary.clone(),
            findings: result.findings.clone(),
        });
    }

    exec.state.touched_files.insert(
        report.node_id.to_owned(),
        report
            .measured_files
            .as_ref()
            .unwrap_or(&result.touched_files)
            .iter()
            .cloned()
            .collect(),
    );
    exec.state.touched_areas.insert(
        report.node_id.to_owned(),
        result.touched_areas.iter().cloned().collect(),
    );
    events.extend(detect_overlaps(exec, graph, report.node_id, at));
    events
}

fn artifact_event(exec: &mut WorkflowExecution, artifact: &Artifact, at: u64) -> WorkflowEvent {
    exec.record(
        WorkflowEventKind::ArtifactCreated,
        Some(&artifact.producer_node_id),
        format!("Artifact: {}", artifact.name),
        BTreeMap::from([
            ("artifactId".to_owned(), artifact.id.clone()),
            ("executionId".to_owned(), artifact.execution_id.clone()),
        ]),
        at,
    )
}

fn normal(path: &str) -> String {
    path.trim()
        .trim_end_matches("/**")
        .trim_end_matches("/*")
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

/// Whether two touched paths are the same place, or one contains the other.
fn overlaps(a: &str, b: &str) -> bool {
    let (a, b) = (normal(a), normal(b));
    !a.is_empty()
        && !b.is_empty()
        && (a == b || a.starts_with(&format!("{b}/")) || b.starts_with(&format!("{a}/")))
}

fn shared_places(a: &BTreeSet<String>, b: &BTreeSet<String>) -> Vec<String> {
    let mut shared: BTreeSet<String> = BTreeSet::new();
    for x in a {
        for y in b {
            if overlaps(x, y) {
                // Name the more specific of the two.
                shared.insert(
                    if normal(x).len() >= normal(y).len() {
                        x
                    } else {
                        y
                    }
                    .clone(),
                );
            }
        }
    }
    shared.into_iter().collect()
}

fn detect_overlaps(
    exec: &mut WorkflowExecution,
    graph: &Graph<'_>,
    node_id: &str,
    at: u64,
) -> Vec<WorkflowEvent> {
    let mut events = Vec::new();
    let empty = BTreeSet::new();
    let mine_files = exec
        .state
        .touched_files
        .get(node_id)
        .unwrap_or(&empty)
        .clone();
    let mine_areas = exec
        .state
        .touched_areas
        .get(node_id)
        .unwrap_or(&empty)
        .clone();
    if mine_files.is_empty() && mine_areas.is_empty() {
        return events;
    }
    let others: Vec<String> = exec
        .state
        .touched_files
        .keys()
        .chain(exec.state.touched_areas.keys())
        .filter(|id| id.as_str() != node_id)
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    for other in others {
        // Steps one after the other are expected to build on each other's files.
        if !graph.independent(node_id, &other) {
            continue;
        }
        let mut paths = shared_places(
            &mine_files,
            exec.state.touched_files.get(&other).unwrap_or(&empty),
        );
        paths.extend(shared_places(
            &mine_areas,
            exec.state.touched_areas.get(&other).unwrap_or(&empty),
        ));
        if paths.is_empty() {
            continue;
        }
        let mut pair = [node_id.to_owned(), other.clone()];
        pair.sort();
        if exec
            .state
            .warnings
            .iter()
            .any(|w| w.node_ids == pair && w.paths == paths)
        {
            continue;
        }
        events.push(exec.record(
            WorkflowEventKind::OverlapDetected,
            Some(node_id),
            "Potential overlap detected",
            BTreeMap::from([
                ("with".to_owned(), other.clone()),
                ("paths".to_owned(), paths.join(",")),
            ]),
            at,
        ));
        exec.state.warnings.push(OverlapWarning {
            node_ids: pair,
            paths,
        });
    }
    events
}

/// What the sending step produced, to be handed over along `link`.
pub struct HandoffSource<'a> {
    pub link: &'a Link,
    pub execution_id: &'a str,
    pub iteration: u32,
    /// The step's structured result (a failure has none).
    pub result: Option<&'a AgentResult>,
    pub delta: Option<&'a StepDelta>,
    pub failure: Option<&'a str>,
}

/// Builds the handoff of one transition from the step's result and from Git. Nothing is made up:
/// what the step did not say stays empty and its status stays `unknown`.
pub fn build_handoff(
    exec: &WorkflowExecution,
    source: &HandoffSource<'_>,
    at: u64,
) -> AgentHandoff {
    let result = source.result;
    let from = &source.link.source;
    let artifacts = exec
        .state
        .artifacts
        .iter()
        .filter(|a| a.execution_id == source.execution_id)
        .map(|a| HandoffArtifact {
            id: a.id.clone(),
            name: a.name.clone(),
            kind: a.kind,
            path: a.path.clone(),
            summary: a.summary.clone(),
        })
        .collect();
    let validation = exec
        .state
        .validation_results
        .iter()
        .find(|v| v.execution_id == source.execution_id)
        .map(|v| HandoffValidation {
            status: v.status,
            outcome: v.outcome.clone(),
            summary: v.summary.clone(),
            findings: v.findings.clone(),
        });
    AgentHandoff {
        id: format!("handoff-{}", exec.handoffs.len() + 1),
        workflow_execution_id: exec.id.clone(),
        link_id: source.link.id.clone(),
        from_node_id: from.clone(),
        to_node_id: source.link.target.clone(),
        from_execution_id: source.execution_id.to_owned(),
        iteration: source.iteration,
        kind: if source.link.failure_route {
            HandoffKind::Failure
        } else {
            HandoffKind::Result
        },
        created_at: at,
        status: result.map_or(ResultStatus::Fail, |r| r.status),
        outcome: result.and_then(|r| r.outcome.clone()),
        summary: result.map(|r| r.summary.clone()).unwrap_or_default(),
        instructions: result.and_then(|r| r.next_action.clone()),
        decisions: result
            .map(|r| {
                r.decisions
                    .iter()
                    .map(|d| HandoffDecision {
                        title: d.title.clone(),
                        decision: d.decision.clone(),
                        rationale: d.rationale.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        artifacts,
        changed_files: source.delta.map(|d| d.files.clone()).unwrap_or_default(),
        uncommitted_files: source
            .delta
            .map(|d| d.uncommitted.clone())
            .unwrap_or_default(),
        reported_files: result.map(|r| r.touched_files.clone()).unwrap_or_default(),
        validation,
        failure: source.failure.map(str::to_owned),
    }
}

/// Records the handoff on the run and says so.
pub fn record_handoff(
    exec: &mut WorkflowExecution,
    handoff: AgentHandoff,
    at: u64,
) -> WorkflowEvent {
    let event = exec.record(
        WorkflowEventKind::HandoffCreated,
        Some(&handoff.from_node_id),
        format!("Handoff to {}", handoff.to_node_id),
        BTreeMap::from([
            ("handoffId".to_owned(), handoff.id.clone()),
            ("to".to_owned(), handoff.to_node_id.clone()),
            ("linkId".to_owned(), handoff.link_id.clone()),
        ]),
        at,
    );
    exec.handoffs.push(handoff);
    event
}
