//! Builders and a tiny synchronous driver for tests of the engine and the orchestrator.

use std::collections::BTreeMap;

use super::engine::{StepCompletion, WorkflowEngine};
use super::validation::AgentCatalog;
use crate::domain::workflow::{
    AgentNode, Condition, ConditionNode, EndNode, EndOutcome, Facts, FailurePolicy, LoopPolicy,
    NodeKind, RetryPolicy, Workflow, WorkflowEdge, WorkflowExecution, WorkflowExecutionStatus,
    WorkflowMode, WorkflowNode, WorkflowStatus,
};

pub struct Agents(pub Vec<&'static str>);

impl AgentCatalog for Agents {
    fn agent_exists(&self, id: &str) -> bool {
        self.0.contains(&id)
    }
}

pub fn agent(id: &str, agent_id: &str) -> WorkflowNode {
    WorkflowNode {
        id: id.to_owned(),
        kind: NodeKind::Agent(AgentNode {
            agent_id: agent_id.to_owned(),
            label: id.to_owned(),
            instructions: String::new(),
            retry_policy: RetryPolicy::default(),
            failure_policy: FailurePolicy::StopWorkflow,
            execution_policy: crate::domain::workflow::ExecutionPolicy::default(),
        }),
        priority: 0,
        loop_policy: None,
        position: None,
    }
}

pub fn end(id: &str, outcome: EndOutcome) -> WorkflowNode {
    WorkflowNode {
        id: id.to_owned(),
        kind: NodeKind::End(EndNode {
            label: id.to_owned(),
            outcome,
        }),
        priority: 0,
        loop_policy: None,
        position: None,
    }
}

pub fn condition_node(id: &str, condition: Condition) -> WorkflowNode {
    WorkflowNode {
        id: id.to_owned(),
        kind: NodeKind::Condition(ConditionNode {
            label: id.to_owned(),
            condition,
        }),
        priority: 0,
        loop_policy: None,
        position: None,
    }
}

pub fn with_loop(mut node: WorkflowNode, id: &str, max: u32) -> WorkflowNode {
    node.loop_policy = Some(LoopPolicy {
        loop_id: id.to_owned(),
        max_iterations: max,
    });
    node
}

pub fn with_retries(mut node: WorkflowNode, max: u32) -> WorkflowNode {
    if let NodeKind::Agent(agent) = &mut node.kind {
        agent.retry_policy = RetryPolicy { max_retries: max };
    }
    node
}

pub fn with_failure_route(mut node: WorkflowNode, target: &str) -> WorkflowNode {
    if let NodeKind::Agent(agent) = &mut node.kind {
        agent.failure_policy = FailurePolicy::RouteToNode {
            node_id: target.to_owned(),
        };
    }
    node
}

pub fn edge(from: &str, to: &str) -> WorkflowEdge {
    WorkflowEdge {
        id: format!("{from}->{to}"),
        source_node_id: from.to_owned(),
        target_node_id: to.to_owned(),
        condition: None,
        label: String::new(),
    }
}

/// An edge taken only when the source's `result.status` is `status`.
pub fn when(from: &str, to: &str, status: &str) -> WorkflowEdge {
    WorkflowEdge {
        condition: Some(Condition::equals("result.status", status)),
        ..edge(from, to)
    }
}

/// An edge taken only when the source's `result.outcome` is `outcome`.
pub fn on_outcome(from: &str, to: &str, outcome: &str) -> WorkflowEdge {
    WorkflowEdge {
        condition: Some(Condition::equals("result.outcome", outcome)),
        ..edge(from, to)
    }
}

pub fn workflow(nodes: Vec<WorkflowNode>, edges: Vec<WorkflowEdge>) -> Workflow {
    Workflow {
        id: "wf-1".to_owned(),
        workspace_id: "ws-1".to_owned(),
        name: "Test workflow".to_owned(),
        description: String::new(),
        mode: WorkflowMode::Custom,
        version: 1,
        status: WorkflowStatus::Ready,
        template_id: None,
        nodes,
        edges,
        viewport: None,
        created_at: 0,
        updated_at: 0,
    }
}

pub fn facts(status: &str) -> Facts {
    BTreeMap::from([("result.status".to_owned(), status.to_owned())])
}

pub fn done(status: &str) -> StepCompletion {
    StepCompletion::Completed {
        facts: facts(status),
        summary: None,
    }
}

/// A step that ran well and concluded `outcome` (its own verdict, not a status of the run).
pub fn concluded(outcome: &str) -> StepCompletion {
    StepCompletion::Completed {
        facts: BTreeMap::from([
            ("result.status".to_owned(), "success".to_owned()),
            ("result.outcome".to_owned(), outcome.to_owned()),
        ]),
        summary: Some(format!("concluded {outcome}")),
    }
}

pub fn failed(message: &str) -> StepCompletion {
    StepCompletion::Failed {
        message: message.to_owned(),
    }
}

/// Runs an execution to its end synchronously: starts every candidate, then completes them in
/// order with whatever the script answers for `(node id, how many times it has started)`.
pub struct Sim {
    pub exec: WorkflowExecution,
    pub workflow: Workflow,
    pub clock: u64,
    pub started: Vec<String>,
}

impl Sim {
    pub fn new(workflow: Workflow) -> Self {
        let exec =
            WorkflowExecution::new("wfx-1".to_owned(), workflow.clone(), "task".to_owned(), 1);
        Self {
            exec,
            workflow,
            clock: 1,
            started: Vec::new(),
        }
    }

    pub fn tick(&mut self) -> u64 {
        self.clock += 1;
        self.clock
    }

    /// Starts the run and returns the nodes the engine would start first.
    pub fn start(&mut self) {
        let at = self.tick();
        WorkflowEngine::new(&self.workflow.clone()).start(&mut self.exec, at);
    }

    pub fn run(&mut self, script: impl FnMut(&str, usize) -> StepCompletion) {
        self.start();
        self.pump(script);
    }

    /// Picks the failed run up again at its Recovery Point (on the definition the simulation
    /// holds now) and runs it on.
    pub fn resume(
        &mut self,
        script: impl FnMut(&str, usize) -> StepCompletion,
    ) -> Result<crate::domain::workflow::RecoveryPlan, crate::domain::workflow::RecoveryProblem>
    {
        let at = self.tick();
        self.exec.workflow = self.workflow.clone();
        let workflow = self.workflow.clone();
        let (plan, _) = WorkflowEngine::new(&workflow).resume_failed(&mut self.exec, at)?;
        self.pump(script);
        Ok(plan)
    }

    fn pump(&mut self, mut script: impl FnMut(&str, usize) -> StepCompletion) {
        let workflow = self.workflow.clone();
        let engine = WorkflowEngine::new(&workflow);
        for _ in 0..200 {
            if self.exec.status.is_final() {
                return;
            }
            let candidates = engine.dispatch_candidates(&self.exec);
            if candidates.is_empty() {
                return;
            }
            let mut begun = Vec::new();
            for candidate in candidates {
                let at = self.tick();
                let execution_id = format!("exec-{}", self.started.len() + 1);
                if engine
                    .begin_step(&mut self.exec, &candidate.node_id, &execution_id, at)
                    .started
                {
                    self.started.push(candidate.node_id.clone());
                    begun.push(candidate.node_id);
                }
            }
            for node_id in begun {
                let count = self.started.iter().filter(|n| **n == node_id).count();
                let outcome = script(&node_id, count);
                let at = self.tick();
                engine.complete_step(&mut self.exec, &node_id, outcome, at);
            }
        }
        panic!("the simulated run did not end");
    }

    pub fn status_of(&self, node: &str) -> crate::domain::workflow::NodeStatus {
        self.exec.nodes[node].status
    }

    pub fn ended(&self) -> WorkflowExecutionStatus {
        self.exec.status
    }
}
