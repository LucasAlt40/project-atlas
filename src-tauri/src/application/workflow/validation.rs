//! Checks a workflow before it runs. Nothing starts from an invalid definition: the user gets
//! the list of what is wrong, each item with a stable code and the ids it is about.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::graph::Graph;
use crate::domain::workflow::{Condition, ConditionError, NodeKind, Workflow, WorkflowEdge};

/// Retries are bounded; this is the most a node may ask for.
pub const MAX_RETRIES_LIMIT: u32 = 5;
pub const MAX_ITERATIONS_LIMIT: u32 = 20;

/// Port: which agents exist. Keeps validation independent of how agents are stored.
pub trait AgentCatalog {
    fn agent_exists(&self, id: &str) -> bool;

    /// The outcome ids the agent's result contract declares (none for a general agent).
    fn declared_outcomes(&self, _id: &str) -> Vec<String> {
        Vec::new()
    }

    /// The agent's name, for messages.
    fn agent_name(&self, id: &str) -> String {
        id.to_owned()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueCode {
    EmptyWorkflow,
    DuplicateNodeId,
    DuplicateEdgeId,
    EdgeUnknownNode,
    FailureRouteUnknownNode,
    EndHasOutgoing,
    NoStartNode,
    NoEndNode,
    IsolatedNode,
    UnreachableNode,
    DeadEnd,
    CycleWithoutLimit,
    InvalidLoopPolicy,
    DuplicateLoopId,
    InvalidCondition,
    /// An edge routes on an outcome its source agent does not declare.
    UndeclaredOutcome,
    MissingAgent,
    UnknownAgent,
    WorkspaceMismatch,
    TooManyRetries,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    pub code: IssueCode,
    pub node_id: Option<String>,
    pub edge_id: Option<String>,
    pub params: BTreeMap<String, String>,
}

impl ValidationIssue {
    fn new(code: IssueCode) -> Self {
        Self {
            code,
            node_id: None,
            edge_id: None,
            params: BTreeMap::new(),
        }
    }

    fn node(mut self, id: &str) -> Self {
        self.node_id = Some(id.to_owned());
        self
    }

    fn edge(mut self, id: &str) -> Self {
        self.edge_id = Some(id.to_owned());
        self
    }

    fn with(mut self, key: &str, value: impl Into<String>) -> Self {
        self.params.insert(key.to_owned(), value.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub valid: bool,
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    #[cfg(test)]
    pub fn has(&self, code: IssueCode) -> bool {
        self.issues.iter().any(|issue| issue.code == code)
    }
}

/// Validates the structure of the workflow, its policies, its agents and (when given) that it
/// belongs to `workspace_id`.
pub fn validate(
    workflow: &Workflow,
    workspace_id: Option<&str>,
    agents: &dyn AgentCatalog,
) -> ValidationReport {
    let mut issues = Vec::new();
    if let Some(expected) = workspace_id {
        if workflow.workspace_id != expected {
            issues.push(ValidationIssue::new(IssueCode::WorkspaceMismatch));
        }
    }
    if workflow.nodes.is_empty() {
        issues.push(ValidationIssue::new(IssueCode::EmptyWorkflow));
        return report(issues);
    }
    check_identity(workflow, &mut issues);
    check_nodes(workflow, agents, &mut issues);
    check_edges(workflow, agents, &mut issues);
    check_shape(workflow, &mut issues);
    report(issues)
}

fn report(issues: Vec<ValidationIssue>) -> ValidationReport {
    ValidationReport {
        valid: issues.is_empty(),
        issues,
    }
}

fn check_identity(workflow: &Workflow, issues: &mut Vec<ValidationIssue>) {
    let mut nodes = BTreeSet::new();
    for node in &workflow.nodes {
        if !nodes.insert(node.id.as_str()) {
            issues.push(ValidationIssue::new(IssueCode::DuplicateNodeId).node(&node.id));
        }
    }
    let mut edges = BTreeSet::new();
    for edge in &workflow.edges {
        if !edges.insert(edge.id.as_str()) {
            issues.push(ValidationIssue::new(IssueCode::DuplicateEdgeId).edge(&edge.id));
        }
    }
}

fn check_nodes(workflow: &Workflow, agents: &dyn AgentCatalog, issues: &mut Vec<ValidationIssue>) {
    let ids: BTreeSet<&str> = workflow.nodes.iter().map(|n| n.id.as_str()).collect();
    let mut loop_ids = BTreeSet::new();
    for node in &workflow.nodes {
        if let Some(policy) = &node.loop_policy {
            if policy.max_iterations == 0
                || policy.max_iterations > MAX_ITERATIONS_LIMIT
                || policy.loop_id.trim().is_empty()
            {
                issues.push(
                    ValidationIssue::new(IssueCode::InvalidLoopPolicy)
                        .node(&node.id)
                        .with("max", MAX_ITERATIONS_LIMIT.to_string()),
                );
            }
            if !loop_ids.insert(policy.loop_id.as_str()) {
                issues.push(
                    ValidationIssue::new(IssueCode::DuplicateLoopId)
                        .node(&node.id)
                        .with("loopId", policy.loop_id.clone()),
                );
            }
        }
        match &node.kind {
            NodeKind::Agent(agent) => {
                if agent.agent_id.trim().is_empty() {
                    issues.push(ValidationIssue::new(IssueCode::MissingAgent).node(&node.id));
                } else if !agents.agent_exists(&agent.agent_id) {
                    issues.push(
                        ValidationIssue::new(IssueCode::UnknownAgent)
                            .node(&node.id)
                            .with("agentId", agent.agent_id.clone()),
                    );
                }
                if agent.retry_policy.max_retries > MAX_RETRIES_LIMIT {
                    issues.push(
                        ValidationIssue::new(IssueCode::TooManyRetries)
                            .node(&node.id)
                            .with("max", MAX_RETRIES_LIMIT.to_string()),
                    );
                }
                if let Some(target) = node.failure_route() {
                    if !ids.contains(target) {
                        issues.push(
                            ValidationIssue::new(IssueCode::FailureRouteUnknownNode)
                                .node(&node.id)
                                .with("target", target),
                        );
                    }
                }
            }
            NodeKind::Condition(condition) => {
                if let Err(error) = condition.condition.validate() {
                    issues.push(condition_issue(error).node(&node.id));
                }
            }
            NodeKind::End(_) => {}
        }
    }
}

fn condition_issue(error: ConditionError) -> ValidationIssue {
    ValidationIssue::new(IssueCode::InvalidCondition).with(
        "reason",
        match error {
            ConditionError::UnknownField => "unknown_field",
            ConditionError::MissingValue => "missing_value",
        },
    )
}

fn check_edges(workflow: &Workflow, agents: &dyn AgentCatalog, issues: &mut Vec<ValidationIssue>) {
    let ids: BTreeSet<&str> = workflow.nodes.iter().map(|n| n.id.as_str()).collect();
    for edge in &workflow.edges {
        for end in [&edge.source_node_id, &edge.target_node_id] {
            if !ids.contains(end.as_str()) {
                issues.push(
                    ValidationIssue::new(IssueCode::EdgeUnknownNode)
                        .edge(&edge.id)
                        .with("node", end.clone()),
                );
            }
        }
        if let Some(condition) = &edge.condition {
            if let Err(error) = condition.validate() {
                issues.push(condition_issue(error).edge(&edge.id));
            } else if let Some(issue) = undeclared_outcome(workflow, agents, edge, condition) {
                issues.push(issue);
            }
        }
        if let Some(source) = workflow.node(&edge.source_node_id) {
            if matches!(source.kind, NodeKind::End(_)) {
                issues.push(ValidationIssue::new(IssueCode::EndHasOutgoing).edge(&edge.id));
            }
        }
    }
}

/// An edge that compares `result.outcome` with a value its source agent never declares can never
/// fire (or always fires, for `not_equals`): the user hears of it before the run, not during it.
fn undeclared_outcome(
    workflow: &Workflow,
    agents: &dyn AgentCatalog,
    edge: &WorkflowEdge,
    condition: &Condition,
) -> Option<ValidationIssue> {
    if condition.field != "result.outcome" || condition.value.is_none() {
        return None;
    }
    let source = workflow.node(&edge.source_node_id)?;
    let agent_id = source.agent_id()?;
    if !agents.agent_exists(agent_id) {
        return None;
    }
    let value = condition.value.as_deref()?.trim();
    let declared = agents.declared_outcomes(agent_id);
    if declared.iter().any(|d| d.eq_ignore_ascii_case(value)) {
        return None;
    }
    Some(
        ValidationIssue::new(IssueCode::UndeclaredOutcome)
            .edge(&edge.id)
            .node(&source.id)
            .with("outcome", value)
            .with("agent", agents.agent_name(agent_id)),
    )
}

fn check_shape(workflow: &Workflow, issues: &mut Vec<ValidationIssue>) {
    let graph = Graph::new(workflow);
    let starts = graph.starts();
    if starts.is_empty() {
        issues.push(ValidationIssue::new(IssueCode::NoStartNode));
    }
    if !workflow
        .nodes
        .iter()
        .any(|n| matches!(n.kind, NodeKind::End(_)))
    {
        issues.push(ValidationIssue::new(IssueCode::NoEndNode));
    }
    let reachable = graph.reachable_from_starts();
    for node in &workflow.nodes {
        let connected =
            graph.in_links(&node.id).next().is_some() || graph.out_links(&node.id).next().is_some();
        if !connected && workflow.nodes.len() > 1 {
            issues.push(ValidationIssue::new(IssueCode::IsolatedNode).node(&node.id));
        } else if connected && !reachable.contains(node.id.as_str()) {
            issues.push(ValidationIssue::new(IssueCode::UnreachableNode).node(&node.id));
        }
        // Every step that is not an End must lead somewhere through an edge; otherwise the run
        // reaches it and has nowhere to go.
        let leads_on = workflow.edges.iter().any(|e| e.source_node_id == node.id);
        if !matches!(node.kind, NodeKind::End(_)) && connected && !leads_on {
            issues.push(ValidationIssue::new(IssueCode::DeadEnd).node(&node.id));
        }
    }
    // A loop is fine; a loop nothing bounds is not. Take out the nodes that carry a limit: any
    // cycle left has no limit on it.
    let bounded: BTreeSet<&str> = workflow
        .nodes
        .iter()
        .filter(|n| n.loop_policy.as_ref().is_some_and(|p| p.max_iterations > 0))
        .map(|n| n.id.as_str())
        .collect();
    let unbounded = graph.cycle_nodes_without(&bounded);
    if !unbounded.is_empty() {
        let mut issue =
            ValidationIssue::new(IssueCode::CycleWithoutLimit).with("nodes", unbounded.join(","));
        issue.node_id = unbounded.first().map(|id| (*id).to_owned());
        issues.push(issue);
    }
}
