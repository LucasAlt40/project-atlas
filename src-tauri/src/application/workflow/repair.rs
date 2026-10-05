//! Assisted route repair. Validation can tell that an edge of an agent with a result contract
//! waits for a result the agent cannot give: `result.status`, which never says what the agent
//! declared, or an outcome it does not declare. It cannot tell which
//! declared outcome the edge was meant for: `pass` may have meant a technical check and
//! `approved` a review, and that is the user's to say. So this module only *proposes* a mapping
//! (from the order of the edges and of the declared outcomes), and applies what the user
//! confirms; nothing is rewritten on its own.

use serde::Serialize;

use super::validation::{cannot_match_contract, routes_outcome, AgentCatalog};
use crate::domain::workflow::{Condition, RouteRepair, Workflow};

/// One edge to repair, with the outcome Atlas would pair it with, if any.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeProposal {
    pub edge_id: String,
    pub target_node_id: String,
    pub current: Condition,
    pub suggested: Option<String>,
}

/// The edges of one step that cannot match what its agent declares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairProposal {
    pub node_id: String,
    pub agent_id: String,
    pub agent: String,
    pub declared: Vec<String>,
    pub edges: Vec<EdgeProposal>,
}

/// What the user confirmed for one edge.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairChoice {
    pub edge_id: String,
    pub outcome: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairError {
    /// The edge is gone, or is not one that needs repair any more.
    NotRepairable,
    /// The outcome is not one the step's agent declares.
    UndeclaredOutcome,
}

/// The repairs worth offering. The suggestion pairs the incompatible edges, in the order they
/// were drawn, with the declared outcomes nothing routes yet, in the order of the contract. It
/// is a starting point built from the shape of the workflow, not a claim about meaning.
pub fn propose(workflow: &Workflow, agents: &dyn AgentCatalog) -> Vec<RepairProposal> {
    let mut proposals = Vec::new();
    for node in &workflow.nodes {
        let Some(agent_id) = node.agent_id() else {
            continue;
        };
        if !agents.agent_exists(agent_id) {
            continue;
        }
        let declared = agents.declared_outcomes(agent_id);
        let edges: Vec<_> = workflow
            .edges
            .iter()
            .filter(|e| e.source_node_id == node.id)
            .collect();
        let broken: Vec<_> = edges
            .iter()
            .filter(|e| cannot_match_contract(workflow, agents, e))
            .collect();
        if broken.is_empty() {
            continue;
        }
        let mut unrouted = declared
            .iter()
            .filter(|o| !edges.iter().any(|e| routes_outcome(e, o)))
            .cloned();
        proposals.push(RepairProposal {
            node_id: node.id.clone(),
            agent_id: agent_id.to_owned(),
            agent: agents.agent_name(agent_id),
            declared: declared.clone(),
            edges: broken
                .iter()
                .filter_map(|e| {
                    Some(EdgeProposal {
                        edge_id: e.id.clone(),
                        target_node_id: e.target_node_id.clone(),
                        current: e.condition.clone()?,
                        suggested: unrouted.next(),
                    })
                })
                .collect(),
        });
    }
    proposals
}

/// Rewrites the confirmed edges to test `result.outcome`. Every choice is checked against the
/// workflow as it is now, so a stale confirmation changes nothing. Returns the records of what
/// changed (the caller stamps the version).
///
/// # Errors
///
/// Fails, changing nothing, if any choice is not repairable or names an undeclared outcome.
pub fn apply(
    workflow: &mut Workflow,
    agents: &dyn AgentCatalog,
    choices: &[RepairChoice],
    at: u64,
) -> Result<Vec<RouteRepair>, RepairError> {
    let mut next = workflow.clone();
    let mut records = Vec::new();
    for (index, choice) in choices.iter().enumerate() {
        if choices[..index].iter().any(|c| c.edge_id == choice.edge_id) {
            return Err(RepairError::NotRepairable);
        }
        let edge = next
            .edges
            .iter()
            .find(|e| e.id == choice.edge_id)
            .ok_or(RepairError::NotRepairable)?
            .clone();
        if !cannot_match_contract(workflow, agents, &edge) {
            return Err(RepairError::NotRepairable);
        }
        let source = next
            .node(&edge.source_node_id)
            .ok_or(RepairError::NotRepairable)?;
        let agent_id = source
            .agent_id()
            .ok_or(RepairError::NotRepairable)?
            .to_owned();
        let outcome = agents
            .declared_outcomes(&agent_id)
            .into_iter()
            .find(|o| o.eq_ignore_ascii_case(choice.outcome.trim()))
            .ok_or(RepairError::UndeclaredOutcome)?;
        let current = Condition::equals("result.outcome", &outcome);
        let slot = next
            .edges
            .iter_mut()
            .find(|e| e.id == edge.id)
            .ok_or(RepairError::NotRepairable)?;
        let old_value = edge.condition.as_ref().and_then(|c| c.value.clone());
        if slot.label.is_empty() || Some(&slot.label) == old_value.as_ref() {
            slot.label.clone_from(&outcome);
        }
        slot.condition = Some(current.clone());
        records.push(RouteRepair {
            edge_id: edge.id.clone(),
            node_id: edge.source_node_id.clone(),
            agent_id,
            target_node_id: edge.target_node_id.clone(),
            previous: edge.condition,
            current,
            version: 0,
            at,
        });
    }
    *workflow = next;
    Ok(records)
}
