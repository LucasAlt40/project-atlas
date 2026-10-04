use super::engine::{StepCompletion, WorkflowEngine};
use super::graph::Graph;
use super::test_support::*;
use super::validation::{validate, IssueCode};
use crate::domain::workflow::{
    AttemptStatus, Condition, EndOutcome, FailureCode, NodeStatus, WorkflowEventKind,
    WorkflowExecution, WorkflowExecutionStatus,
};

const AGENTS: [&str; 6] = ["a1", "a2", "a3", "a4", "a5", "a6"];

fn report(w: &crate::domain::workflow::Workflow) -> super::validation::ValidationReport {
    validate(w, Some("ws-1"), &Agents(AGENTS.to_vec()))
}

fn linear() -> crate::domain::workflow::Workflow {
    workflow(
        vec![
            agent("architect", "a1"),
            agent("backend", "a2"),
            end("done", EndOutcome::Done),
        ],
        vec![edge("architect", "backend"), edge("backend", "done")],
    )
}

/// Architect -> Developer -> QA; QA pass -> end, QA fail -> Bug Fixer -> QA (QA bounded).
fn qa_loop(max: u32) -> crate::domain::workflow::Workflow {
    workflow(
        vec![
            agent("architect", "a1"),
            agent("dev", "a2"),
            with_loop(agent("qa", "a3"), "qa_bug_fix", max),
            agent("fixer", "a4"),
            end("done", EndOutcome::Done),
        ],
        vec![
            edge("architect", "dev"),
            edge("dev", "qa"),
            when("qa", "done", "pass"),
            when("qa", "fixer", "fail"),
            edge("fixer", "qa"),
        ],
    )
}

// ---- graph validation -----------------------------------------------------------------------

#[test]
fn a_valid_graph_has_no_issues() {
    assert!(report(&linear()).valid);
    assert!(report(&qa_loop(3)).valid);
}

#[test]
fn an_empty_workflow_is_invalid() {
    assert!(report(&workflow(vec![], vec![])).has(IssueCode::EmptyWorkflow));
}

#[test]
fn duplicate_ids_are_reported() {
    let mut w = linear();
    w.nodes.push(agent("backend", "a2"));
    w.edges.push(edge("architect", "backend"));
    let r = report(&w);
    assert!(r.has(IssueCode::DuplicateNodeId));
    assert!(r.has(IssueCode::DuplicateEdgeId));
}

#[test]
fn an_edge_to_a_missing_node_is_reported() {
    let mut w = linear();
    w.edges.push(edge("backend", "ghost"));
    let r = report(&w);
    assert!(r.has(IssueCode::EdgeUnknownNode));
    assert_eq!(
        r.issues
            .iter()
            .find(|i| i.code == IssueCode::EdgeUnknownNode)
            .unwrap()
            .edge_id
            .as_deref(),
        Some("backend->ghost")
    );
}

#[test]
fn a_workflow_needs_a_start_and_an_end() {
    let no_end = workflow(
        vec![agent("a", "a1"), agent("b", "a2")],
        vec![edge("a", "b")],
    );
    assert!(report(&no_end).has(IssueCode::NoEndNode));

    let no_start = workflow(
        vec![
            with_loop(agent("a", "a1"), "l", 2),
            with_loop(agent("b", "a2"), "m", 2),
            end("e", EndOutcome::Done),
        ],
        vec![edge("a", "b"), edge("b", "a"), edge("b", "e")],
    );
    let r = report(&no_start);
    assert!(r.has(IssueCode::NoStartNode));
}

#[test]
fn a_disconnected_node_is_reported() {
    let mut w = linear();
    w.nodes.push(agent("lonely", "a3"));
    assert!(report(&w).has(IssueCode::IsolatedNode));
}

#[test]
fn an_agent_step_that_leads_nowhere_is_a_dead_end() {
    let w = workflow(
        vec![
            agent("a", "a1"),
            agent("b", "a2"),
            end("e", EndOutcome::Done),
        ],
        vec![edge("a", "b"), edge("a", "e")],
    );
    let r = report(&w);
    assert!(r.has(IssueCode::DeadEnd));
}

#[test]
fn an_end_node_cannot_lead_anywhere() {
    let mut w = linear();
    w.nodes.push(agent("extra", "a3"));
    w.edges.push(edge("done", "extra"));
    assert!(report(&w).has(IssueCode::EndHasOutgoing));
}

#[test]
fn conditions_are_validated() {
    let mut w = qa_loop(3);
    w.edges[2].condition = Some(Condition::equals("shell.rm", "x"));
    w.edges[3].condition = Some(Condition::equals("result.status", " "));
    let r = report(&w);
    assert_eq!(
        r.issues
            .iter()
            .filter(|i| i.code == IssueCode::InvalidCondition)
            .count(),
        2
    );
    assert!(Condition::equals("result.status", "fail")
        .validate()
        .is_ok());
}

#[test]
fn agents_must_exist() {
    let mut w = linear();
    if let crate::domain::workflow::NodeKind::Agent(a) = &mut w.nodes[0].kind {
        a.agent_id = "missing".to_owned();
    }
    if let crate::domain::workflow::NodeKind::Agent(a) = &mut w.nodes[1].kind {
        a.agent_id = String::new();
    }
    let r = report(&w);
    assert!(r.has(IssueCode::UnknownAgent));
    assert!(r.has(IssueCode::MissingAgent));
}

#[test]
fn the_workflow_must_belong_to_the_workspace() {
    let w = linear();
    let r = validate(&w, Some("other"), &Agents(AGENTS.to_vec()));
    assert!(r.has(IssueCode::WorkspaceMismatch));
}

// ---- cycles ---------------------------------------------------------------------------------

#[test]
fn an_unbounded_cycle_is_invalid() {
    let w = workflow(
        vec![
            agent("start", "a1"),
            agent("a", "a2"),
            agent("b", "a3"),
            end("e", EndOutcome::Done),
        ],
        vec![
            edge("start", "a"),
            edge("a", "b"),
            edge("b", "a"),
            when("b", "e", "pass"),
        ],
    );
    let r = report(&w);
    assert!(r.has(IssueCode::CycleWithoutLimit));
}

#[test]
fn a_bounded_loop_is_valid() {
    assert!(!report(&qa_loop(3)).has(IssueCode::CycleWithoutLimit));
}

#[test]
fn a_loop_limit_must_be_a_positive_number() {
    assert!(report(&qa_loop(0)).has(IssueCode::InvalidLoopPolicy));
    assert!(report(&qa_loop(0)).has(IssueCode::CycleWithoutLimit));
}

#[test]
fn every_cycle_needs_a_limit_not_only_one_of_them() {
    // Two cycles through the fixer: only QA is bounded, validator <-> fixer is not.
    let w = workflow(
        vec![
            agent("dev", "a1"),
            agent("validator", "a2"),
            with_loop(agent("qa", "a3"), "qa", 3),
            agent("fixer", "a4"),
            end("done", EndOutcome::Done),
        ],
        vec![
            edge("dev", "validator"),
            when("validator", "qa", "pass"),
            when("validator", "fixer", "fail"),
            when("qa", "done", "pass"),
            when("qa", "fixer", "fail"),
            edge("fixer", "validator"),
        ],
    );
    assert!(report(&w).has(IssueCode::CycleWithoutLimit));
}

#[test]
fn a_node_that_leads_to_itself_is_a_cycle_that_needs_a_limit() {
    let looping = |max: Option<u32>| {
        let node = agent("a", "a1");
        workflow(
            vec![
                max.map_or(node.clone(), |m| with_loop(node, "self", m)),
                end("e", EndOutcome::Done),
            ],
            vec![when("a", "a", "fail"), when("a", "e", "pass")],
        )
    };
    // No start: the only step has an incoming edge; add a real start to isolate the cycle check.
    let mut unbounded = looping(None);
    unbounded.nodes.insert(0, agent("s", "a2"));
    unbounded.edges.push(edge("s", "a"));
    let mut bounded = looping(Some(2));
    bounded.nodes.insert(0, agent("s", "a2"));
    bounded.edges.push(edge("s", "a"));
    assert!(report(&unbounded).has(IssueCode::CycleWithoutLimit));
    assert!(!report(&bounded).has(IssueCode::CycleWithoutLimit));
}

#[test]
fn a_failure_route_counts_as_a_link_for_cycles() {
    let mut w = workflow(
        vec![
            agent("s", "a1"),
            agent("a", "a2"),
            agent("b", "a3"),
            end("e", EndOutcome::Done),
        ],
        vec![edge("s", "a"), edge("a", "b"), edge("b", "e")],
    );
    w.nodes[2] = with_failure_route(w.nodes[2].clone(), "a");
    assert!(report(&w).has(IssueCode::CycleWithoutLimit));
}

#[test]
fn cycle_detection_finds_each_component() {
    let w = qa_loop(3);
    let g = Graph::new(&w);
    assert_eq!(g.cycle_of("qa").len(), 2);
    assert_eq!(g.cycle_of("dev").len(), 0);
    assert!(!g.independent("architect", "architect"));
}

// ---- scheduler ------------------------------------------------------------------------------

fn engine_run(w: &crate::domain::workflow::Workflow) -> (WorkflowExecution, u64) {
    let mut exec = WorkflowExecution::new("x".to_owned(), w.clone(), "t".to_owned(), 1);
    WorkflowEngine::new(w).start(&mut exec, 2);
    (exec, 2)
}

#[test]
fn initial_nodes_become_ready_and_the_rest_wait() {
    let w = linear();
    let (exec, _) = engine_run(&w);
    assert_eq!(exec.nodes["architect"].status, NodeStatus::Ready);
    assert_eq!(exec.nodes["backend"].status, NodeStatus::Pending);
    assert_eq!(exec.nodes["done"].status, NodeStatus::Pending);
    assert!(exec
        .events
        .iter()
        .any(|e| e.kind == WorkflowEventKind::Started));
}

#[test]
fn a_completed_dependency_unlocks_the_next_node() {
    let w = linear();
    let (mut exec, _) = engine_run(&w);
    let engine = WorkflowEngine::new(&w);
    assert!(engine.begin_step(&mut exec, "architect", "e1", 3).started);
    assert_eq!(exec.nodes["backend"].status, NodeStatus::Pending);
    engine.complete_step(&mut exec, "architect", done("success"), 4);
    assert_eq!(exec.nodes["backend"].status, NodeStatus::Ready);
}

fn diamond() -> crate::domain::workflow::Workflow {
    workflow(
        vec![
            agent("architect", "a1"),
            agent("backend", "a2"),
            agent("frontend", "a3"),
            agent("qa", "a4"),
            end("done", EndOutcome::Done),
        ],
        vec![
            edge("architect", "backend"),
            edge("architect", "frontend"),
            edge("backend", "qa"),
            edge("frontend", "qa"),
            edge("qa", "done"),
        ],
    )
}

#[test]
fn a_join_waits_for_every_branch() {
    let w = diamond();
    let (mut exec, _) = engine_run(&w);
    let engine = WorkflowEngine::new(&w);
    engine.begin_step(&mut exec, "architect", "e1", 3);
    engine.complete_step(&mut exec, "architect", done("success"), 4);
    // Both branches are ready together and can run at the same time.
    let ids: Vec<_> = engine
        .dispatch_candidates(&exec)
        .into_iter()
        .map(|c| c.node_id)
        .collect();
    assert_eq!(ids, ["backend", "frontend"]);
    engine.begin_step(&mut exec, "backend", "e2", 5);
    engine.begin_step(&mut exec, "frontend", "e3", 5);
    engine.complete_step(&mut exec, "backend", done("success"), 6);
    assert_eq!(exec.nodes["qa"].status, NodeStatus::Pending);
    engine.complete_step(&mut exec, "frontend", done("success"), 7);
    assert_eq!(exec.nodes["qa"].status, NodeStatus::Ready);
}

#[test]
fn ready_nodes_are_ordered_by_priority_then_creation_then_id() {
    let mut w = workflow(
        vec![
            agent("s", "a1"),
            agent("zeta", "a2"),
            agent("alpha", "a3"),
            agent("urgent", "a4"),
            end("e", EndOutcome::Done),
        ],
        vec![
            edge("s", "zeta"),
            edge("s", "alpha"),
            edge("s", "urgent"),
            edge("zeta", "e"),
            edge("alpha", "e"),
            edge("urgent", "e"),
        ],
    );
    w.nodes[3].priority = 5;
    let (mut exec, _) = engine_run(&w);
    let engine = WorkflowEngine::new(&w);
    engine.begin_step(&mut exec, "s", "e1", 3);
    engine.complete_step(&mut exec, "s", done("success"), 4);
    let ids: Vec<_> = engine
        .dispatch_candidates(&exec)
        .into_iter()
        .map(|c| c.node_id)
        .collect();
    assert_eq!(ids, ["urgent", "zeta", "alpha"]);
}

#[test]
fn the_scheduler_is_deterministic() {
    let run = || {
        let mut sim = Sim::new(diamond());
        sim.run(|_, _| done("success"));
        (
            sim.started.clone(),
            sim.exec
                .events
                .iter()
                .map(|e| (e.kind, e.node_id.clone()))
                .collect::<Vec<_>>(),
        )
    };
    assert_eq!(run(), run());
}

#[test]
fn parallelism_is_capped() {
    let mut nodes = vec![agent("s", "a1")];
    let mut edges = Vec::new();
    for i in 0..6 {
        let id = format!("p{i}");
        nodes.push(agent(&id, &format!("agent-{i}")));
        edges.push(edge("s", &id));
        edges.push(edge(&id, "e"));
    }
    nodes.push(end("e", EndOutcome::Done));
    let w = workflow(nodes, edges);
    let (mut exec, _) = engine_run(&w);
    let engine = WorkflowEngine::new(&w);
    engine.begin_step(&mut exec, "s", "e0", 3);
    engine.complete_step(&mut exec, "s", done("success"), 4);
    assert_eq!(engine.dispatch_candidates(&exec).len(), 4);
    exec.max_parallel_steps = 2;
    assert_eq!(engine.dispatch_candidates(&exec).len(), 2);
    engine.begin_step(&mut exec, "p0", "e1", 5);
    assert_eq!(engine.dispatch_candidates(&exec).len(), 1);
}

#[test]
fn one_agent_never_runs_two_steps_at_once() {
    let w = workflow(
        vec![
            agent("s", "a1"),
            agent("backend", "shared"),
            agent("frontend", "shared"),
            agent("other", "a2"),
            end("e", EndOutcome::Done),
        ],
        vec![
            edge("s", "backend"),
            edge("s", "frontend"),
            edge("s", "other"),
            edge("backend", "e"),
            edge("frontend", "e"),
            edge("other", "e"),
        ],
    );
    let (mut exec, _) = engine_run(&w);
    let engine = WorkflowEngine::new(&w);
    engine.begin_step(&mut exec, "s", "e0", 3);
    engine.complete_step(&mut exec, "s", done("success"), 4);
    let ids: Vec<_> = engine
        .dispatch_candidates(&exec)
        .into_iter()
        .map(|c| c.node_id)
        .collect();
    // The second step of the shared agent waits; a different agent runs alongside.
    assert_eq!(ids, ["backend", "other"]);
    engine.begin_step(&mut exec, "backend", "e1", 5);
    let ids: Vec<_> = engine
        .dispatch_candidates(&exec)
        .into_iter()
        .map(|c| c.node_id)
        .collect();
    assert_eq!(ids, ["other"]);
    engine.complete_step(&mut exec, "backend", done("success"), 6);
    let ids: Vec<_> = engine
        .dispatch_candidates(&exec)
        .into_iter()
        .map(|c| c.node_id)
        .collect();
    assert_eq!(ids, ["frontend", "other"]);
}

// ---- running whole workflows ------------------------------------------------------------------

#[test]
fn a_linear_workflow_runs_to_completion() {
    let mut sim = Sim::new(linear());
    sim.run(|_, _| done("success"));
    assert_eq!(sim.ended(), WorkflowExecutionStatus::Completed);
    assert_eq!(sim.started, ["architect", "backend"]);
    assert_eq!(sim.status_of("done"), NodeStatus::Completed);
    assert_eq!(
        sim.exec.state.completed_nodes,
        ["architect", "backend", "done"]
    );
    assert!(sim.exec.completed_at.is_some());
}

#[test]
fn qa_failing_once_routes_through_the_bug_fixer_and_back() {
    let mut sim = Sim::new(qa_loop(3));
    sim.run(|node, count| match (node, count) {
        ("qa", 1) => done("fail"),
        ("qa", _) => done("pass"),
        _ => done("success"),
    });
    assert_eq!(sim.ended(), WorkflowExecutionStatus::Completed);
    assert_eq!(sim.started, ["architect", "dev", "qa", "fixer", "qa"]);
    assert_eq!(sim.exec.nodes["qa"].iterations, 2);
    assert_eq!(sim.exec.state.iteration_count["qa_bug_fix"], 2);
}

#[test]
fn a_loop_that_reaches_its_limit_fails_the_workflow() {
    let mut sim = Sim::new(qa_loop(2));
    sim.run(|node, _| {
        if node == "qa" {
            done("fail")
        } else {
            done("success")
        }
    });
    assert_eq!(sim.ended(), WorkflowExecutionStatus::Failed);
    let failure = sim.exec.failure.clone().unwrap();
    assert_eq!(failure.code, FailureCode::MaxIterationsReached);
    assert_eq!(failure.node_id.as_deref(), Some("qa"));
    // qa ran twice, the fixer once between them, and a third qa never started.
    assert_eq!(
        sim.started,
        ["architect", "dev", "qa", "fixer", "qa", "fixer"]
    );
    assert_eq!(sim.status_of("qa"), NodeStatus::Blocked);
    assert_eq!(sim.status_of("done"), NodeStatus::Cancelled);
}

#[test]
fn a_loop_runs_exactly_its_limit_when_it_keeps_failing_then_passes_on_the_last() {
    let mut sim = Sim::new(qa_loop(3));
    sim.run(|node, count| match (node, count) {
        ("qa", 3) => done("pass"),
        ("qa", _) => done("fail"),
        _ => done("success"),
    });
    assert_eq!(sim.ended(), WorkflowExecutionStatus::Completed);
    assert_eq!(sim.exec.nodes["qa"].iterations, 3);
}

#[test]
fn a_result_that_matches_no_edge_fails_the_workflow_honestly() {
    let mut sim = Sim::new(qa_loop(3));
    sim.run(|node, _| {
        if node == "qa" {
            done("unknown")
        } else {
            done("success")
        }
    });
    assert_eq!(sim.ended(), WorkflowExecutionStatus::Failed);
    assert_eq!(
        sim.exec.failure.as_ref().unwrap().code,
        FailureCode::NoRouteMatched
    );
    assert_eq!(
        sim.exec.failure.as_ref().unwrap().node_id.as_deref(),
        Some("qa")
    );
    assert_ne!(sim.status_of("done"), NodeStatus::Completed);
}

#[test]
fn two_distinct_loops_can_run_in_one_workflow() {
    let w = workflow(
        vec![
            agent("architect", "a1"),
            agent("dev", "a2"),
            with_loop(agent("validator", "a3"), "architecture_fix", 3),
            agent("fix-arch", "a4"),
            with_loop(agent("qa", "a5"), "qa_bug_fix", 3),
            agent("fix-bug", "a6"),
            end("done", EndOutcome::Done),
        ],
        vec![
            edge("architect", "dev"),
            edge("dev", "validator"),
            when("validator", "qa", "pass"),
            when("validator", "fix-arch", "fail"),
            edge("fix-arch", "validator"),
            when("qa", "done", "pass"),
            when("qa", "fix-bug", "fail"),
            edge("fix-bug", "qa"),
        ],
    );
    assert!(report(&w).valid, "{:?}", report(&w).issues);
    let mut sim = Sim::new(w);
    sim.run(|node, count| match (node, count) {
        ("validator" | "qa", 1) => done("fail"),
        ("validator" | "qa", _) => done("pass"),
        _ => done("success"),
    });
    assert_eq!(sim.ended(), WorkflowExecutionStatus::Completed);
    assert_eq!(
        sim.started,
        [
            "architect",
            "dev",
            "validator",
            "fix-arch",
            "validator",
            "qa",
            "fix-bug",
            "qa"
        ]
    );
    // The path not taken is skipped, not blocked.
    assert_eq!(sim.exec.state.iteration_count["architecture_fix"], 2);
}

#[test]
fn a_condition_node_routes_on_the_previous_result() {
    let w = workflow(
        vec![
            agent("qa", "a1"),
            condition_node("check", Condition::equals("result.status", "pass")),
            agent("ship", "a2"),
            agent("fix", "a3"),
            end("done", EndOutcome::Done),
            end("stop", EndOutcome::Failed),
        ],
        vec![
            edge("qa", "check"),
            WorkflowEdgeExt::on("check", "ship", "result.matched", "true"),
            WorkflowEdgeExt::on("check", "fix", "result.matched", "false"),
            edge("ship", "done"),
            edge("fix", "stop"),
        ],
    );
    assert!(report(&w).valid, "{:?}", report(&w).issues);
    let mut passing = Sim::new(w.clone());
    passing.run(|node, _| {
        if node == "qa" {
            done("pass")
        } else {
            done("success")
        }
    });
    assert_eq!(passing.started, ["qa", "ship"]);
    assert_eq!(passing.ended(), WorkflowExecutionStatus::Completed);
    // The route not taken is skipped.
    assert_eq!(passing.status_of("fix"), NodeStatus::Skipped);
    assert_eq!(passing.status_of("stop"), NodeStatus::Skipped);

    let mut failing = Sim::new(w);
    failing.run(|node, _| {
        if node == "qa" {
            done("fail")
        } else {
            done("success")
        }
    });
    assert_eq!(failing.started, ["qa", "fix"]);
    // Reached an End that means failure.
    assert_eq!(failing.ended(), WorkflowExecutionStatus::Failed);
    assert_eq!(
        failing.exec.failure.as_ref().unwrap().code,
        FailureCode::EndedInFailure
    );
}

struct WorkflowEdgeExt;

impl WorkflowEdgeExt {
    fn on(from: &str, to: &str, field: &str, value: &str) -> crate::domain::workflow::WorkflowEdge {
        crate::domain::workflow::WorkflowEdge {
            condition: Some(Condition::equals(field, value)),
            ..edge(from, to)
        }
    }
}

#[test]
fn condition_operators() {
    use crate::domain::workflow::ConditionOperator::{Equals, Exists, NotEquals, NotExists};
    let facts = facts("Failed");
    let c = |operator, value: Option<&str>, field: &str| Condition {
        field: field.to_owned(),
        operator,
        value: value.map(str::to_owned),
    };
    assert!(c(Equals, Some("failed"), "result.status").evaluate(&facts));
    assert!(!c(NotEquals, Some("failed"), "result.status").evaluate(&facts));
    assert!(c(NotEquals, Some("pass"), "result.status").evaluate(&facts));
    assert!(c(Exists, None, "result.status").evaluate(&facts));
    assert!(c(NotExists, None, "result.summary").evaluate(&facts));
    assert!(!c(Equals, Some("x"), "result.summary").evaluate(&facts));
    assert!(c(NotEquals, Some("x"), "result.summary").evaluate(&facts));
}

// ---- failure ----------------------------------------------------------------------------------

#[test]
fn a_failed_step_blocks_what_depends_on_it_and_fails_the_workflow() {
    let mut sim = Sim::new(linear());
    sim.run(|node, _| {
        if node == "architect" {
            failed("boom")
        } else {
            done("success")
        }
    });
    assert_eq!(sim.ended(), WorkflowExecutionStatus::Failed);
    assert_eq!(sim.status_of("architect"), NodeStatus::Failed);
    assert_eq!(sim.status_of("backend"), NodeStatus::Blocked);
    assert_eq!(sim.status_of("done"), NodeStatus::Blocked);
    let failure = sim.exec.failure.clone().unwrap();
    assert_eq!(
        (failure.code, failure.node_id.as_deref()),
        (FailureCode::NodeFailed, Some("architect"))
    );
    assert_eq!(sim.exec.state.failed_nodes, ["architect"]);
}

#[test]
fn a_failure_route_keeps_the_workflow_alive() {
    let w = workflow(
        vec![
            with_failure_route(agent("dev", "a1"), "fixer"),
            agent("fixer", "a2"),
            end("done", EndOutcome::Done),
        ],
        vec![edge("dev", "done"), edge("fixer", "done")],
    );
    let mut sim = Sim::new(w);
    sim.run(|node, _| {
        if node == "dev" {
            failed("compile error")
        } else {
            done("success")
        }
    });
    assert_eq!(sim.started, ["dev", "fixer"]);
    assert_eq!(sim.ended(), WorkflowExecutionStatus::Completed);
    assert_eq!(sim.status_of("dev"), NodeStatus::Failed);
}

#[test]
fn a_step_is_retried_with_its_own_execution_id_each_time() {
    let w = workflow(
        vec![
            with_retries(agent("dev", "a1"), 2),
            end("done", EndOutcome::Done),
        ],
        vec![edge("dev", "done")],
    );
    let mut sim = Sim::new(w);
    sim.run(|_, count| {
        if count < 3 {
            failed("flaky")
        } else {
            done("success")
        }
    });
    assert_eq!(sim.ended(), WorkflowExecutionStatus::Completed);
    let attempts = &sim.exec.nodes["dev"].attempts;
    assert_eq!(
        attempts.iter().map(|a| a.attempt).collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(
        attempts.iter().map(|a| a.status).collect::<Vec<_>>(),
        [
            AttemptStatus::Failed,
            AttemptStatus::Failed,
            AttemptStatus::Completed
        ]
    );
    let ids: std::collections::BTreeSet<_> =
        attempts.iter().map(|a| a.execution_id.clone()).collect();
    assert_eq!(ids.len(), 3);
    // Retrying is not a loop iteration.
    assert_eq!(sim.exec.nodes["dev"].iterations, 1);
    assert_eq!(
        sim.exec
            .events
            .iter()
            .filter(|e| e.kind == WorkflowEventKind::NodeRetrying)
            .count(),
        2
    );
}

#[test]
fn retries_are_limited() {
    let w = workflow(
        vec![
            with_retries(agent("dev", "a1"), 1),
            end("done", EndOutcome::Done),
        ],
        vec![edge("dev", "done")],
    );
    let mut sim = Sim::new(w);
    sim.run(|_, _| failed("always"));
    assert_eq!(sim.started, ["dev", "dev"]);
    assert_eq!(sim.ended(), WorkflowExecutionStatus::Failed);
    assert_eq!(sim.exec.nodes["dev"].attempts.len(), 2);
}

#[test]
fn a_failure_in_one_branch_lets_the_other_finish_before_the_run_fails() {
    let mut sim = Sim::new(diamond());
    sim.run(|node, _| {
        if node == "backend" {
            failed("x")
        } else {
            done("success")
        }
    });
    assert_eq!(sim.ended(), WorkflowExecutionStatus::Failed);
    assert_eq!(sim.status_of("frontend"), NodeStatus::Completed);
    assert_eq!(sim.status_of("qa"), NodeStatus::Blocked);
}

#[test]
fn a_run_with_no_end_reached_has_no_path_to_completion() {
    let w = workflow(
        vec![
            agent("a", "a1"),
            agent("b", "a2"),
            end("e", EndOutcome::Done),
        ],
        vec![edge("a", "b"), when("b", "e", "pass")],
    );
    let mut sim = Sim::new(w);
    sim.run(|_, _| done("fail"));
    assert_eq!(sim.ended(), WorkflowExecutionStatus::Failed);
    assert_eq!(
        sim.exec.failure.as_ref().unwrap().code,
        FailureCode::NoRouteMatched
    );
}

// ---- pause, cancel, approval, interruption -------------------------------------------------------

#[test]
fn pausing_stops_new_steps_but_not_the_one_running() {
    let w = linear();
    let (mut exec, _) = engine_run(&w);
    let engine = WorkflowEngine::new(&w);
    engine.begin_step(&mut exec, "architect", "e1", 3);
    engine.pause(&mut exec, 4);
    assert_eq!(exec.status, WorkflowExecutionStatus::Paused);
    assert_eq!(exec.nodes["architect"].status, NodeStatus::Running);
    // The running step finishes and its result is kept; nothing new is offered.
    engine.complete_step(&mut exec, "architect", done("success"), 5);
    assert_eq!(exec.nodes["architect"].status, NodeStatus::Completed);
    assert_eq!(exec.nodes["backend"].status, NodeStatus::Ready);
    assert_eq!(engine.dispatch_candidates(&exec).len(), 0);
    assert_eq!(exec.status, WorkflowExecutionStatus::Paused);
    engine.resume(&mut exec, 6);
    assert_eq!(exec.status, WorkflowExecutionStatus::Running);
    assert_eq!(engine.dispatch_candidates(&exec).len(), 1);
}

#[test]
fn cancelling_cancels_pending_nodes_waits_for_running_ones_and_keeps_completed() {
    let w = diamond();
    let (mut exec, _) = engine_run(&w);
    let engine = WorkflowEngine::new(&w);
    engine.begin_step(&mut exec, "architect", "e1", 3);
    engine.complete_step(&mut exec, "architect", done("success"), 4);
    engine.begin_step(&mut exec, "backend", "e2", 5);
    engine.request_cancel(&mut exec, 6);
    // Pending and ready nodes are cancelled at once; the run waits for the running step.
    assert_eq!(exec.nodes["architect"].status, NodeStatus::Completed);
    assert_eq!(exec.nodes["frontend"].status, NodeStatus::Cancelled);
    assert_eq!(exec.nodes["qa"].status, NodeStatus::Cancelled);
    assert_eq!(exec.nodes["backend"].status, NodeStatus::Running);
    assert_eq!(exec.status, WorkflowExecutionStatus::Running);
    assert_eq!(engine.dispatch_candidates(&exec).len(), 0);
    engine.complete_step(&mut exec, "backend", StepCompletion::Cancelled, 7);
    assert_eq!(exec.nodes["backend"].status, NodeStatus::Cancelled);
    assert_eq!(exec.status, WorkflowExecutionStatus::Cancelled);
    assert_eq!(
        exec.nodes["backend"].attempts[0].status,
        AttemptStatus::Cancelled
    );
    assert!(exec
        .events
        .iter()
        .any(|e| e.kind == WorkflowEventKind::Cancelled));
}

#[test]
fn a_step_that_finishes_during_a_cancel_keeps_its_result_but_routes_nowhere() {
    let w = linear();
    let (mut exec, _) = engine_run(&w);
    let engine = WorkflowEngine::new(&w);
    engine.begin_step(&mut exec, "architect", "e1", 3);
    engine.request_cancel(&mut exec, 4);
    engine.complete_step(&mut exec, "architect", done("success"), 5);
    assert_eq!(exec.nodes["architect"].status, NodeStatus::Completed);
    assert_eq!(exec.nodes["backend"].status, NodeStatus::Cancelled);
    assert_eq!(exec.status, WorkflowExecutionStatus::Cancelled);
}

#[test]
fn cancelling_with_nothing_running_ends_at_once() {
    let w = linear();
    let (mut exec, _) = engine_run(&w);
    WorkflowEngine::new(&w).request_cancel(&mut exec, 3);
    assert_eq!(exec.status, WorkflowExecutionStatus::Cancelled);
    assert!(exec
        .nodes
        .values()
        .all(|s| s.status == NodeStatus::Cancelled));
}

#[test]
fn waiting_for_approval_is_a_state_of_a_running_step_and_dependents_stay_pending() {
    let w = linear();
    let (mut exec, _) = engine_run(&w);
    let engine = WorkflowEngine::new(&w);
    engine.begin_step(&mut exec, "architect", "e1", 3);
    let events = engine.set_waiting_approval(&mut exec, "architect", true, 4);
    assert_eq!(exec.nodes["architect"].status, NodeStatus::WaitingApproval);
    assert_eq!(events[0].kind, WorkflowEventKind::NodeWaitingApproval);
    assert_eq!(exec.nodes["backend"].status, NodeStatus::Pending);
    // Still in flight: it holds its slot and its agent.
    assert_eq!(exec.in_flight(), 1);
    // Not a state a pending node can be put in.
    assert_eq!(
        engine
            .set_waiting_approval(&mut exec, "backend", true, 5)
            .len(),
        0
    );
    engine.set_waiting_approval(&mut exec, "architect", false, 6);
    assert_eq!(exec.nodes["architect"].status, NodeStatus::Running);
    engine.complete_step(&mut exec, "architect", done("success"), 7);
    assert_eq!(exec.nodes["backend"].status, NodeStatus::Ready);
}

#[test]
fn a_denied_step_that_fails_blocks_its_dependents() {
    let w = linear();
    let (mut exec, _) = engine_run(&w);
    let engine = WorkflowEngine::new(&w);
    engine.begin_step(&mut exec, "architect", "e1", 3);
    engine.set_waiting_approval(&mut exec, "architect", true, 4);
    engine.complete_step(&mut exec, "architect", failed("permission denied"), 5);
    assert_eq!(exec.nodes["architect"].status, NodeStatus::Failed);
    assert_eq!(exec.nodes["backend"].status, NodeStatus::Blocked);
    assert_eq!(exec.status, WorkflowExecutionStatus::Failed);
}

#[test]
fn an_interrupted_run_is_never_marked_as_finished_and_can_be_resumed() {
    let w = linear();
    let (mut exec, _) = engine_run(&w);
    let engine = WorkflowEngine::new(&w);
    engine.begin_step(&mut exec, "architect", "e1", 3);
    engine.interrupt(&mut exec, 4);
    assert_eq!(exec.status, WorkflowExecutionStatus::Interrupted);
    assert_eq!(
        exec.nodes["architect"].attempts[0].status,
        AttemptStatus::Interrupted
    );
    assert!(exec.completed_at.is_none());
    assert_eq!(engine.dispatch_candidates(&exec).len(), 0);

    engine.resume_interrupted(&mut exec, 5);
    assert_eq!(exec.status, WorkflowExecutionStatus::Running);
    assert_eq!(exec.nodes["architect"].status, NodeStatus::Ready);
    assert!(engine.begin_step(&mut exec, "architect", "e2", 6).started);
    // The interrupted attempt stays on record; the rerun is another attempt, same iteration.
    assert_eq!(exec.nodes["architect"].attempts.len(), 2);
    assert_eq!(exec.nodes["architect"].iterations, 1);
}

#[test]
fn an_interrupted_run_can_be_cancelled() {
    let w = linear();
    let (mut exec, _) = engine_run(&w);
    let engine = WorkflowEngine::new(&w);
    engine.begin_step(&mut exec, "architect", "e1", 3);
    engine.interrupt(&mut exec, 4);
    engine.request_cancel(&mut exec, 5);
    assert_eq!(exec.status, WorkflowExecutionStatus::Cancelled);
    assert_eq!(exec.nodes["architect"].status, NodeStatus::Cancelled);
}

#[test]
fn the_snapshot_keeps_the_run_on_the_version_it_started_from() {
    let mut definition = linear();
    let exec = WorkflowExecution::new("x".to_owned(), definition.clone(), "t".to_owned(), 1);
    definition.version = 2;
    definition.nodes.pop();
    assert_eq!(exec.workflow_version, 1);
    assert_eq!(exec.workflow.nodes.len(), 3);
}

#[test]
fn shared_state_serialises_deterministically() {
    let mut sim = Sim::new(diamond());
    sim.run(|_, _| done("success"));
    let a = serde_json::to_string(&sim.exec.state).unwrap();
    let mut other = Sim::new(diamond());
    other.run(|_, _| done("success"));
    assert_eq!(a, serde_json::to_string(&other.exec.state).unwrap());
    assert_eq!(sim.exec.state.completed_agents.len(), 4);
    assert!(sim.exec.state.active_agents.is_empty());
}

// ---- recovery -------------------------------------------------------------------------------

/// Architect -> Developer -> Validator; the validator's `pass` goes to the end.
fn unrouted_fail() -> crate::domain::workflow::Workflow {
    workflow(
        vec![
            agent("architect", "a1"),
            agent("dev", "a2"),
            with_loop(agent("validator", "a3"), "fix", 3),
            end("done", EndOutcome::Done),
        ],
        vec![
            edge("architect", "dev"),
            edge("dev", "validator"),
            on_outcome("validator", "done", "pass"),
        ],
    )
}

/// The same, with the missing route and the Bug Fixer that loops back.
fn routed_fail() -> crate::domain::workflow::Workflow {
    let mut w = unrouted_fail();
    w.nodes.push(agent("fixer", "a4"));
    w.edges.push(on_outcome("validator", "fixer", "fail"));
    w.edges.push(edge("fixer", "validator"));
    w
}

fn count(sim: &Sim, node: &str) -> usize {
    sim.started.iter().filter(|n| *n == node).count()
}

#[test]
fn a_fail_outcome_with_no_route_fails_the_run_and_names_the_step() {
    let mut sim = Sim::new(unrouted_fail());
    sim.run(|_, _| concluded("fail"));
    assert_eq!(sim.ended(), WorkflowExecutionStatus::Failed);
    let failure = sim.exec.failure.as_ref().unwrap();
    assert_eq!(failure.code, FailureCode::NoRouteMatched);
    assert_eq!(failure.node_id.as_deref(), Some("validator"));
    assert_eq!(failure.detail.as_deref(), Some("fail"));
    // The step itself ran well: the verdict is not a technical failure.
    assert_eq!(sim.status_of("validator"), NodeStatus::Completed);
}

#[test]
fn resuming_after_a_route_is_added_goes_on_without_running_finished_steps_again() {
    let mut sim = Sim::new(unrouted_fail());
    sim.run(|_, _| concluded("fail"));
    sim.workflow = routed_fail();
    sim.exec.nodes.insert(
        "fixer".to_owned(),
        crate::domain::workflow::NodeState::pending(),
    );

    let plan = sim
        .resume(|node, n| match (node, n) {
            ("validator", _) => concluded("pass"),
            _ => concluded("fixed"),
        })
        .expect("a route exists now");

    assert_eq!(plan.kind, crate::domain::workflow::RecoveryKind::Resume);
    assert_eq!(plan.restart_node_ids, ["fixer"]);
    assert_eq!(plan.reused_node_ids, ["architect", "dev", "validator"]);
    assert_eq!(sim.ended(), WorkflowExecutionStatus::Completed);
    assert_eq!(count(&sim, "architect"), 1);
    assert_eq!(count(&sim, "dev"), 1);
    assert_eq!(count(&sim, "fixer"), 1);
    assert_eq!(count(&sim, "validator"), 2);
    assert_eq!(sim.exec.recoveries.len(), 1);
    assert!(sim.exec.failure.is_none());
}

#[test]
fn a_resume_with_still_no_route_says_so_and_leaves_the_run_as_it_was() {
    let mut sim = Sim::new(unrouted_fail());
    sim.run(|_, _| concluded("fail"));
    let before = sim.exec.clone();
    let problem = sim.resume(|_, _| concluded("pass")).unwrap_err();
    assert_eq!(problem, crate::domain::workflow::RecoveryProblem::NoRoute);
    assert_eq!(sim.exec, before);
}

#[test]
fn a_technical_failure_is_retried_on_the_same_step_and_nothing_else_runs_again() {
    let mut sim = Sim::new(unrouted_fail());
    sim.run(|node, _| match node {
        "dev" => failed("the runtime crashed"),
        _ => concluded("pass"),
    });
    assert_eq!(
        sim.exec.failure.as_ref().unwrap().code,
        FailureCode::NodeFailed
    );

    let plan = sim.resume(|_, _| concluded("pass")).expect("a retry");

    assert_eq!(plan.kind, crate::domain::workflow::RecoveryKind::Retry);
    assert_eq!(plan.restart_node_ids, ["dev"]);
    assert_eq!(sim.ended(), WorkflowExecutionStatus::Completed);
    assert_eq!(count(&sim, "architect"), 1);
    assert_eq!(count(&sim, "dev"), 2);
}

#[test]
fn a_loop_that_reached_its_limit_resumes_only_when_the_limit_was_raised() {
    let mut sim = Sim::new(routed_fail());
    sim.run(|node, _| match node {
        "validator" => concluded("fail"),
        _ => concluded("fixed"),
    });
    assert_eq!(
        sim.exec.failure.as_ref().unwrap().code,
        FailureCode::MaxIterationsReached
    );
    assert_eq!(count(&sim, "validator"), 3);
    assert_eq!(
        sim.resume(|_, _| concluded("pass")).unwrap_err(),
        crate::domain::workflow::RecoveryProblem::LoopLimit
    );

    for node in &mut sim.workflow.nodes {
        if let Some(policy) = node.loop_policy.as_mut() {
            policy.max_iterations = 5;
        }
    }
    sim.resume(|node, _| match node {
        "validator" => concluded("pass"),
        _ => concluded("fixed"),
    })
    .expect("room for another pass");
    assert_eq!(sim.ended(), WorkflowExecutionStatus::Completed);
    assert_eq!(count(&sim, "architect"), 1);
}

#[test]
fn a_run_that_reached_a_failing_end_cannot_be_resumed() {
    let mut w = unrouted_fail();
    w.nodes.push(end("broken", EndOutcome::Failed));
    w.edges.push(on_outcome("validator", "broken", "fail"));
    let mut sim = Sim::new(w);
    sim.run(|_, _| concluded("fail"));
    assert_eq!(sim.ended(), WorkflowExecutionStatus::Failed);
    assert_eq!(
        sim.resume(|_, _| concluded("pass")).unwrap_err(),
        crate::domain::workflow::RecoveryProblem::NotRecoverable
    );
}

// ---- routing on what the agent concluded ---------------------------------------------------------

fn qa_by_outcome() -> crate::domain::workflow::Workflow {
    workflow(
        vec![
            agent("qa", "a1"),
            agent("fixer", "a2"),
            end("done", EndOutcome::Done),
        ],
        vec![
            on_outcome("qa", "done", "approved"),
            on_outcome("qa", "fixer", "changes_requested"),
            edge("fixer", "done"),
        ],
    )
}

#[test]
fn an_outcome_goes_along_the_route_that_names_it_whatever_the_execution_status_says() {
    for (concluded_as, fixer_runs) in [("approved", 0), ("changes_requested", 1)] {
        let mut sim = Sim::new(qa_by_outcome());
        sim.run(|_, _| concluded(concluded_as));
        // Both steps ran well (`success`): what decided the road was the verdict.
        assert_eq!(sim.exec.nodes["qa"].facts["result.status"], "success");
        assert_eq!(count(&sim, "fixer"), fixer_runs, "{concluded_as}");
        assert_eq!(sim.ended(), WorkflowExecutionStatus::Completed);
    }
}

#[test]
fn an_outcome_the_workflow_does_not_route_fails_the_run_instead_of_completing_it() {
    let mut sim = Sim::new(qa_by_outcome());
    sim.run(|_, _| concluded("fail"));
    assert_eq!(sim.ended(), WorkflowExecutionStatus::Failed);
    assert_eq!(
        sim.exec.failure.as_ref().unwrap().code,
        FailureCode::NoRouteMatched
    );
    // `status` is not the verdict: an edge on it never stands in for one.
    let mut w = qa_by_outcome();
    w.edges.push(when("qa", "done", "fail"));
    let mut sim = Sim::new(w);
    sim.run(|_, _| concluded("fail"));
    assert_eq!(sim.ended(), WorkflowExecutionStatus::Failed);
}
