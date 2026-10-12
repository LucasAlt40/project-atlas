//! One module per kind of configuration (see `Category`), and what they share.

pub mod agent;
pub mod personality;
pub mod template;
pub mod workflow;

use std::collections::BTreeMap;

use super::apply::reject;
use super::format::Item;
use super::inspect::{Excerpt, Inspector, StepPreview};
use super::spec::{reason_of, EdgeSpec, NodeSpec, StepSpec};
use super::Existing;
use crate::application::config::UserConfig;
use crate::application::errors::AppError;
use crate::application::workflow::validation::AgentCatalog;
use crate::domain::workflow::{WorkflowEdge, WorkflowNode};

/// A domain rule an item breaks, as the reason the import is refused for it.
pub(super) fn invalid(item: &Item, error: &AppError) -> AppError {
    reject(reason_of(error), &item.name)
}

/// What the steps of a graph say, for the person to read before agreeing to them.
pub(super) fn step_previews(nodes: &[NodeSpec]) -> Vec<StepPreview> {
    nodes
        .iter()
        .map(|node| match &node.step {
            StepSpec::Agent(step) => StepPreview {
                label: step.label.clone(),
                kind: "agent",
                agent: step.agent.clone(),
                instructions: (!step.instructions.trim().is_empty())
                    .then(|| Excerpt::of(&step.instructions)),
                read_only: step.read_only,
            },
            StepSpec::Condition(step) => StepPreview {
                label: step.label.clone(),
                kind: "condition",
                agent: None,
                instructions: None,
                read_only: false,
            },
            StepSpec::End(step) => StepPreview {
                label: step.label.clone(),
                kind: "end",
                agent: None,
                instructions: None,
                read_only: false,
            },
        })
        .collect()
}

/// The graph as it would be were the file imported as it stands: agents the file reuses by
/// their local ids, the others by a placeholder (`~ref`) that only the preview ever sees.
/// Also says whether every agent is one that exists already.
pub(super) fn candidate_graph(
    cx: &Inspector<'_>,
    nodes: &[NodeSpec],
    edges: &[EdgeSpec],
) -> (Vec<WorkflowNode>, Vec<WorkflowEdge>, bool) {
    let mut all_known = true;
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    for node in nodes {
        if let StepSpec::Agent(step) = &node.step {
            if let Some(key) = &step.agent {
                let id = cx.local_for(key).unwrap_or_else(|| {
                    all_known = false;
                    format!("~{key}")
                });
                seen.insert(key.clone(), id);
            }
        }
    }
    let (nodes, edges) = super::spec::graph_to_domain(nodes, edges, &|key| {
        seen.get(key).cloned().unwrap_or_default()
    });
    (nodes, edges, all_known)
}

/// Whether two graphs are the same process: steps, routes and policies, wherever the steps sit on
/// the canvas.
pub(super) fn same_graph(
    a: (&[WorkflowNode], &[WorkflowEdge]),
    b: (&[WorkflowNode], &[WorkflowEdge]),
) -> bool {
    let strip = |nodes: &[WorkflowNode]| {
        let mut nodes = nodes.to_vec();
        for node in &mut nodes {
            node.position = None;
        }
        nodes
    };
    strip(a.0) == strip(b.0) && a.1 == b.1
}

/// The agents (and the outcomes each declares) as the workflow validator asks about them: the
/// installation's own, plus the ones a preview is about to add, under their placeholder ids.
pub(crate) struct PreviewCatalog<'a> {
    pub config: &'a UserConfig,
    pub pending: BTreeMap<String, Vec<String>>,
}

impl AgentCatalog for PreviewCatalog<'_> {
    fn agent_exists(&self, id: &str) -> bool {
        self.pending.contains_key(id) || self.config.agents.iter().any(|a| a.id == id)
    }

    fn declared_outcomes(&self, id: &str) -> Vec<String> {
        if let Some(outcomes) = self.pending.get(id) {
            return outcomes.clone();
        }
        self.config
            .agents
            .iter()
            .find(|a| a.id == id)
            .map(|a| {
                a.result_contract
                    .outcomes
                    .iter()
                    .map(|o| o.id.clone())
                    .collect()
            })
            .unwrap_or_default()
    }
}

pub(super) fn existing_of<T>(
    items: &[T],
    id: impl Fn(&T) -> &str,
    name: impl Fn(&T) -> &str,
) -> Vec<Existing> {
    items
        .iter()
        .map(|item| Existing {
            id: id(item).to_owned(),
            name: name(item).to_owned(),
        })
        .collect()
}
