//! The graph a [`Workflow`] describes, read the way the engine needs it: links in and out of
//! each node, which nodes sit in a cycle, which links close one.
//!
//! A *link* is an edge, or the failure route a node names. Both move the run from one node to
//! another, so validation, scheduling and cycle checks treat them alike. This module tolerates
//! a malformed workflow (dangling edges, duplicate ids): it simply ignores what it cannot place,
//! because the validator needs the graph to explain what is wrong.

use std::collections::{BTreeMap, BTreeSet};

use crate::domain::workflow::{Condition, NodeKind, Workflow};

/// The prefix of the id of a failure route: `failure:<node id>`.
pub const FAILURE_LINK_PREFIX: &str = "failure:";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub id: String,
    pub source: String,
    pub target: String,
    pub condition: Option<Condition>,
    pub failure_route: bool,
}

impl Link {
    /// Alternatives rather than requirements: a conditional link is one of several ways in.
    pub fn is_alternative(&self) -> bool {
        self.failure_route || self.condition.is_some()
    }
}

pub struct Graph<'a> {
    pub workflow: &'a Workflow,
    order: BTreeMap<&'a str, usize>,
    links: Vec<Link>,
    out: BTreeMap<&'a str, Vec<usize>>,
    inn: BTreeMap<&'a str, Vec<usize>>,
    /// Component id of each node that is in a cycle (including a node that loops on itself).
    cyclic: BTreeMap<&'a str, usize>,
}

impl<'a> Graph<'a> {
    pub fn new(workflow: &'a Workflow) -> Self {
        let mut order = BTreeMap::new();
        for (index, node) in workflow.nodes.iter().enumerate() {
            order.entry(node.id.as_str()).or_insert(index);
        }
        let mut links = Vec::new();
        let mut seen_edges = BTreeSet::new();
        for edge in &workflow.edges {
            if order.contains_key(edge.source_node_id.as_str())
                && order.contains_key(edge.target_node_id.as_str())
                && seen_edges.insert(edge.id.as_str())
            {
                links.push(Link {
                    id: edge.id.clone(),
                    source: edge.source_node_id.clone(),
                    target: edge.target_node_id.clone(),
                    condition: edge.condition.clone(),
                    failure_route: false,
                });
            }
        }
        for node in &workflow.nodes {
            if let Some(target) = node.failure_route() {
                if order.contains_key(target) {
                    links.push(Link {
                        id: format!("{FAILURE_LINK_PREFIX}{}", node.id),
                        source: node.id.clone(),
                        target: target.to_owned(),
                        condition: None,
                        failure_route: true,
                    });
                }
            }
        }
        let mut out: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        let mut inn: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        for (index, link) in links.iter().enumerate() {
            if let Some((&source, _)) = order.get_key_value(link.source.as_str()) {
                out.entry(source).or_default().push(index);
            }
            if let Some((&target, _)) = order.get_key_value(link.target.as_str()) {
                inn.entry(target).or_default().push(index);
            }
        }
        let mut graph = Self {
            workflow,
            order,
            links,
            out,
            inn,
            cyclic: BTreeMap::new(),
        };
        graph.cyclic = graph.cycles(&BTreeSet::new());
        graph
    }

    pub fn out_links(&self, node: &str) -> impl Iterator<Item = &Link> {
        self.out
            .get(node)
            .into_iter()
            .flatten()
            .map(|&i| &self.links[i])
    }

    pub fn in_links(&self, node: &str) -> impl Iterator<Item = &Link> {
        self.inn
            .get(node)
            .into_iter()
            .flatten()
            .map(|&i| &self.links[i])
    }

    /// The position of the node in the workflow: creation order, used to break ties.
    pub fn index_of(&self, node: &str) -> usize {
        self.order.get(node).copied().unwrap_or(usize::MAX)
    }

    /// Nodes nothing leads to: where a run begins. End nodes are not starts.
    pub fn starts(&self) -> Vec<&'a str> {
        self.workflow
            .nodes
            .iter()
            .filter(|n| !matches!(n.kind, NodeKind::End(_)))
            .filter(|n| self.in_links(&n.id).next().is_none())
            .map(|n| n.id.as_str())
            .collect()
    }

    /// Whether the link goes between two nodes of the same cycle, so that following it sends the
    /// run back through nodes it may already have been through.
    pub fn is_internal(&self, link: &Link) -> bool {
        match (
            self.cyclic.get(link.source.as_str()),
            self.cyclic.get(link.target.as_str()),
        ) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }

    /// The nodes of the cycle `node` belongs to (just `node` when it is in none).
    pub fn cycle_of(&self, node: &str) -> Vec<&str> {
        let Some(component) = self.cyclic.get(node) else {
            return vec![];
        };
        self.cyclic
            .iter()
            .filter(|(_, c)| *c == component)
            .map(|(n, _)| *n)
            .collect()
    }

    /// Every node reachable from `from` by following links (not including `from` itself unless
    /// it is on a cycle).
    pub fn descendants(&self, from: &str) -> BTreeSet<&str> {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut stack = vec![from];
        while let Some(node) = stack.pop() {
            for link in self.out_links(node) {
                if let Some((&target, _)) = self.order.get_key_value(link.target.as_str()) {
                    if seen.insert(target) {
                        stack.push(target);
                    }
                }
            }
        }
        seen
    }

    pub fn reachable_from_starts(&self) -> BTreeSet<&str> {
        let mut seen: BTreeSet<&str> = self.starts().into_iter().collect();
        for start in self.starts() {
            seen.extend(self.descendants(start));
        }
        seen
    }

    /// Whether the two nodes are independent: neither leads to the other, so they could run side
    /// by side.
    pub fn independent(&self, a: &str, b: &str) -> bool {
        a != b && !self.descendants(a).contains(b) && !self.descendants(b).contains(a)
    }

    /// The nodes that sit on a cycle once every node in `removed` is taken out of the graph.
    /// With no node removed these are all the nodes in a cycle; removing the nodes that carry a
    /// loop limit leaves the cycles that nothing bounds.
    pub fn cycle_nodes_without(&self, removed: &BTreeSet<&str>) -> Vec<&'a str> {
        self.cycles(removed).keys().copied().collect()
    }

    /// Tarjan's strongest-connected-components over the links between nodes not in `removed`.
    /// Returns, for each node in a cycle, the id of its component.
    fn cycles(&self, removed: &BTreeSet<&str>) -> BTreeMap<&'a str, usize> {
        struct State<'g, 'a> {
            graph: &'g Graph<'a>,
            removed: &'g BTreeSet<&'g str>,
            index: BTreeMap<&'a str, usize>,
            low: BTreeMap<&'a str, usize>,
            on_stack: BTreeSet<&'a str>,
            stack: Vec<&'a str>,
            next: usize,
            components: Vec<Vec<&'a str>>,
        }
        fn visit<'a>(state: &mut State<'_, 'a>, node: &'a str) {
            state.index.insert(node, state.next);
            state.low.insert(node, state.next);
            state.next += 1;
            state.stack.push(node);
            state.on_stack.insert(node);
            let targets: Vec<&'a str> = state
                .graph
                .out_links(node)
                .filter_map(|l| state.graph.order.get_key_value(l.target.as_str()))
                .map(|(&t, _)| t)
                .filter(|t| !state.removed.contains(t))
                .collect();
            for target in targets {
                if !state.index.contains_key(target) {
                    visit(state, target);
                    let low = state.low[node].min(state.low[target]);
                    state.low.insert(node, low);
                } else if state.on_stack.contains(target) {
                    let low = state.low[node].min(state.index[target]);
                    state.low.insert(node, low);
                }
            }
            if state.low[node] == state.index[node] {
                let mut component = Vec::new();
                while let Some(top) = state.stack.pop() {
                    state.on_stack.remove(top);
                    component.push(top);
                    if top == node {
                        break;
                    }
                }
                state.components.push(component);
            }
        }

        let mut state = State {
            graph: self,
            removed,
            index: BTreeMap::new(),
            low: BTreeMap::new(),
            on_stack: BTreeSet::new(),
            stack: Vec::new(),
            next: 0,
            components: Vec::new(),
        };
        for node in &self.workflow.nodes {
            let id = node.id.as_str();
            if !removed.contains(id) && !state.index.contains_key(id) {
                visit(&mut state, id);
            }
        }
        let mut cyclic = BTreeMap::new();
        for (component_id, component) in state.components.iter().enumerate() {
            let loops_on_itself = component.len() == 1
                && self
                    .out_links(component[0])
                    .any(|l| l.target == component[0]);
            if component.len() > 1 || loops_on_itself {
                for &node in component {
                    cyclic.insert(node, component_id);
                }
            }
        }
        cyclic
    }
}
