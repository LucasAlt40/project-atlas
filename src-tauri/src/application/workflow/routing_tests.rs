//! Static checks of a workflow against what its agents declare, and the assisted repair of the
//! routes those checks find. Nothing here runs a step: the definitions are judged as written.

use std::collections::BTreeMap;

use super::repair::{apply, propose, RepairChoice, RepairError};
use super::test_support::*;
use super::validation::{validate, AgentCatalog, IssueCode, Severity, ValidationReport};
use crate::domain::workflow::{Condition, EndOutcome, Workflow};

/// Agents by id, each with the outcomes its contract declares, in the contract's order.
struct Contracts(BTreeMap<&'static str, Vec<&'static str>>);

impl AgentCatalog for Contracts {
    fn agent_exists(&self, id: &str) -> bool {
        self.0.contains_key(id)
    }
    fn declared_outcomes(&self, id: &str) -> Vec<String> {
        self.0
            .get(id)
            .map(|o| o.iter().map(|s| (*s).to_owned()).collect())
            .unwrap_or_default()
    }
}

fn team() -> Contracts {
    Contracts(BTreeMap::from([
        ("dev", vec![]),
        ("qa", vec!["approved", "changes_requested"]),
        ("checker", vec!["pass", "fail"]),
        ("fixer", vec![]),
    ]))
}

fn check(w: &Workflow) -> ValidationReport {
    validate(w, Some("ws-1"), &team())
}

fn codes(report: &ValidationReport) -> Vec<(IssueCode, Severity)> {
    report.issues.iter().map(|i| (i.code, i.severity)).collect()
}

fn qa_workflow(routes: Vec<crate::domain::workflow::WorkflowEdge>) -> Workflow {
    let mut edges = vec![edge("dev", "qa"), edge("fixer", "qa")];
    edges.extend(routes);
    workflow(
        vec![
            agent("dev", "dev"),
            with_loop(agent("qa", "qa"), "qa_fix", 3),
            agent("fixer", "fixer"),
            end("done", EndOutcome::Done),
        ],
        edges,
    )
}

#[test]
fn a_qa_routed_on_its_declared_outcomes_is_valid() {
    let w = qa_workflow(vec![
        on_outcome("qa", "done", "approved"),
        on_outcome("qa", "fixer", "changes_requested"),
    ]);
    assert!(check(&w).issues.is_empty(), "{:?}", check(&w).issues);
}

#[test]
fn status_routes_on_an_agent_with_a_contract_are_reported_with_what_it_declares() {
    // The workflow of the real bug: QA declares approved / changes_requested, the edges read the
    // technical status.
    let w = qa_workflow(vec![
        when("qa", "done", "pass"),
        when("qa", "fixer", "fail"),
    ]);
    let report = check(&w);
    assert!(!report.valid);
    let status: Vec<_> = report
        .issues
        .iter()
        .filter(|i| i.code == IssueCode::StatusRouteOnContract)
        .collect();
    assert_eq!(status.len(), 2);
    assert_eq!(status[0].edge_id.as_deref(), Some("qa->done"));
    assert_eq!(status[0].params["status"], "pass");
    assert_eq!(status[0].params["outcomes"], "approved, changes_requested");
    // And neither declared outcome has a route.
    let missing: Vec<_> = report
        .issues
        .iter()
        .filter(|i| i.code == IssueCode::OutcomeWithoutRoute)
        .map(|i| i.params["outcome"].as_str())
        .collect();
    assert_eq!(missing, ["approved", "changes_requested"]);
}

#[test]
fn a_status_route_that_the_declared_outcome_also_reports_is_not_an_incompatibility() {
    // `pass` / `fail` are mirrored in `result.status`, so the older edges keep working.
    let w = workflow(
        vec![
            agent("check", "checker"),
            agent("fixer", "fixer"),
            end("done", EndOutcome::Done),
        ],
        vec![
            when("check", "done", "pass"),
            when("check", "fixer", "fail"),
            edge("fixer", "check"),
        ],
    );
    assert!(!codes(&check(&w))
        .iter()
        .any(|(c, _)| *c == IssueCode::StatusRouteOnContract));
}

#[test]
fn a_missing_route_is_an_error_when_the_others_exist_and_a_warning_when_none_do() {
    let checker = |routes: Vec<_>| {
        let mut edges = vec![edge("dev", "check"), edge("fixer", "check")];
        edges.extend(routes);
        workflow(
            vec![
                agent("dev", "dev"),
                with_loop(agent("check", "checker"), "check_fix", 3),
                agent("fixer", "fixer"),
                end("done", EndOutcome::Done),
            ],
            edges,
        )
    };
    // pass -> End, fail -> Fixer: valid.
    let both = checker(vec![
        on_outcome("check", "done", "pass"),
        on_outcome("check", "fixer", "fail"),
    ]);
    assert!(check(&both).valid);
    // pass -> End only: `fail` has no route.
    let one = check(&checker(vec![on_outcome("check", "done", "pass")]));
    assert!(!one.valid);
    let issue = one
        .issues
        .iter()
        .find(|i| i.code == IssueCode::OutcomeWithoutRoute)
        .unwrap();
    assert_eq!(issue.params["outcome"], "fail");
    assert_eq!(issue.severity, Severity::Error);
    // No route at all: a warning about the outcomes, next to the dead end it already is.
    let none = check(&checker(vec![]));
    let warning = none
        .issues
        .iter()
        .find(|i| i.code == IssueCode::OutcomeWithoutRoute)
        .unwrap();
    assert_eq!(warning.severity, Severity::Warning);
    assert_eq!(warning.params["outcomes"], "pass, fail");
}

#[test]
fn an_unconditional_edge_takes_every_outcome() {
    let w = workflow(
        vec![agent("check", "checker"), end("done", EndOutcome::Done)],
        vec![edge("check", "done")],
    );
    assert!(check(&w).valid);
}

#[test]
fn a_route_into_a_part_of_the_graph_that_never_ends_is_reported() {
    // approved -> A -> B -> A ...: nothing leads to the End.
    let w = workflow(
        vec![
            agent("qa", "qa"),
            with_loop(agent("a", "dev"), "loop_a", 3),
            agent("b", "dev"),
            agent("fixer", "fixer"),
            end("done", EndOutcome::Done),
        ],
        vec![
            on_outcome("qa", "a", "approved"),
            on_outcome("qa", "fixer", "changes_requested"),
            edge("a", "b"),
            edge("b", "a"),
            edge("fixer", "done"),
        ],
    );
    let report = check(&w);
    assert!(!report.valid);
    let flagged: Vec<_> = report
        .issues
        .iter()
        .filter(|i| i.code == IssueCode::CannotReachEnd)
        .filter_map(|i| i.edge_id.as_deref())
        .collect();
    assert!(flagged.contains(&"qa->a"), "{flagged:?}");
    assert!(!flagged.contains(&"qa->fixer"));
}

// ---- assisted repair ------------------------------------------------------------------------

fn broken() -> Workflow {
    qa_workflow(vec![
        when("qa", "done", "pass"),
        when("qa", "fixer", "fail"),
    ])
}

fn confirm(pairs: &[(&str, &str)]) -> Vec<RepairChoice> {
    pairs
        .iter()
        .map(|(edge, outcome)| RepairChoice {
            edge_id: (*edge).to_owned(),
            outcome: (*outcome).to_owned(),
        })
        .collect()
}

#[test]
fn the_proposal_pairs_edges_with_outcomes_by_shape_and_changes_nothing() {
    let w = broken();
    let before = w.clone();
    let proposals = propose(&w, &team());
    assert_eq!(w, before);
    assert_eq!(proposals.len(), 1);
    let qa = &proposals[0];
    assert_eq!(qa.node_id, "qa");
    assert_eq!(qa.declared, ["approved", "changes_requested"]);
    let pairs: Vec<_> = qa
        .edges
        .iter()
        .map(|e| {
            (
                e.edge_id.as_str(),
                e.target_node_id.as_str(),
                e.suggested.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        pairs,
        [
            ("qa->done", "done", Some("approved")),
            ("qa->fixer", "fixer", Some("changes_requested")),
        ]
    );
    // A workflow that is fine has nothing to propose.
    assert_eq!(
        propose(
            &qa_workflow(vec![
                on_outcome("qa", "done", "approved"),
                on_outcome("qa", "fixer", "changes_requested")
            ]),
            &team()
        )
        .len(),
        0
    );
}

#[test]
fn only_what_the_user_confirms_is_applied_and_each_change_is_recorded() {
    let mut w = broken();
    let records = apply(
        &mut w,
        &team(),
        &confirm(&[("qa->done", "approved"), ("qa->fixer", "changes_requested")]),
        42,
    )
    .unwrap();

    let routed: Vec<_> = w
        .edges
        .iter()
        .filter(|e| e.source_node_id == "qa")
        .map(|e| e.condition.clone().unwrap())
        .collect();
    assert_eq!(
        routed,
        [
            Condition::equals("result.outcome", "approved"),
            Condition::equals("result.outcome", "changes_requested")
        ]
    );
    assert!(check(&w).valid, "{:?}", check(&w).issues);
    assert_eq!(records.len(), 2);
    assert_eq!(
        records[0].previous,
        Some(Condition::equals("result.status", "pass"))
    );
    assert_eq!(
        records[0].current,
        Condition::equals("result.outcome", "approved")
    );
    assert_eq!(
        (
            records[0].node_id.as_str(),
            records[0].target_node_id.as_str()
        ),
        ("qa", "done")
    );
    assert_eq!(records[0].at, 42);
    // Nothing left to propose: repairing again is not possible.
    assert_eq!(propose(&w, &team()).len(), 0);
    assert_eq!(
        apply(&mut w, &team(), &confirm(&[("qa->done", "approved")]), 43).unwrap_err(),
        RepairError::NotRepairable
    );
}

#[test]
fn the_user_may_map_differently_and_a_bad_choice_changes_nothing() {
    let mut w = broken();
    // Crossed on purpose: the user decides what the outcomes mean.
    apply(
        &mut w,
        &team(),
        &confirm(&[("qa->done", "changes_requested"), ("qa->fixer", "approved")]),
        1,
    )
    .unwrap();
    assert_eq!(
        w.edges
            .iter()
            .find(|e| e.id == "qa->done")
            .unwrap()
            .condition,
        Some(Condition::equals("result.outcome", "changes_requested"))
    );

    let mut w = broken();
    let before = w.clone();
    for bad in [
        confirm(&[("qa->done", "approved"), ("qa->fixer", "success")]),
        confirm(&[("qa->done", "approved"), ("dev->qa", "approved")]),
        confirm(&[("qa->done", "approved"), ("qa->done", "changes_requested")]),
    ] {
        assert!(apply(&mut w, &team(), &bad, 1).is_err());
        assert_eq!(w, before, "a refused repair leaves the workflow as it was");
    }
}

#[test]
fn a_pass_fail_agent_is_never_remapped_by_the_repair() {
    // These edges are fine as they are (`pass` / `fail` mirror in the status): not offered.
    let w = workflow(
        vec![
            agent("check", "checker"),
            agent("fixer", "fixer"),
            end("done", EndOutcome::Done),
        ],
        vec![
            when("check", "done", "pass"),
            when("check", "fixer", "fail"),
            edge("fixer", "check"),
        ],
    );
    assert_eq!(propose(&w, &team()).len(), 0);
}

// ---- templates ------------------------------------------------------------------------------

/// Whatever the judging agents declare among Atlas's own presets, every template must build a
/// workflow that validates: a default may never be born broken.
#[test]
fn every_template_validates_with_the_contract_of_the_agents_it_is_given() {
    use super::templates::{instantiate, list, AgentChoice};
    use crate::domain::workflow::WorkflowMode;
    const PASS_FAIL: [&str; 2] = ["pass", "fail"];
    const REVIEW: [&str; 2] = ["approved", "changes_requested"];
    for (validator, qa) in [
        (PASS_FAIL, PASS_FAIL),
        (REVIEW, REVIEW),
        // The real case: the two judges do not share a contract.
        (PASS_FAIL, REVIEW),
        (REVIEW, PASS_FAIL),
    ] {
        let by_id: BTreeMap<&'static str, Vec<&'static str>> = BTreeMap::from([
            ("architect", vec![]),
            ("developer", vec![]),
            ("architecture-validator", validator.to_vec()),
            ("qa", qa.to_vec()),
            ("bug-fixer", vec![]),
        ]);
        let choices: Vec<AgentChoice> = by_id
            .iter()
            .map(|(id, outcomes)| AgentChoice {
                id: (*id).to_owned(),
                personality_id: (*id).to_owned(),
                placed: true,
                outcomes: outcomes.iter().map(|o| (*o).to_owned()).collect(),
            })
            .collect();
        for template in list() {
            let built = instantiate(
                template.id,
                "ws-1",
                "wf-1",
                None,
                &choices,
                WorkflowMode::Automatic,
                1,
            )
            .unwrap();
            assert_eq!(built.missing_roles.len(), 0);
            let report = validate(&built.workflow, Some("ws-1"), &Contracts(by_id.clone()));
            assert!(
                report.valid && report.issues.is_empty(),
                "{} with {validator:?}/{qa:?}: {:?}",
                template.id,
                report.issues
            );
        }
    }
}

#[test]
fn routes_on_outcomes_the_agent_does_not_declare_can_be_repaired_too() {
    // A workflow built when QA declared pass / fail, now with a QA that declares a review.
    let mut w = qa_workflow(vec![
        on_outcome("qa", "done", "pass"),
        on_outcome("qa", "fixer", "fail"),
    ]);
    let report = check(&w);
    assert!(report.has(IssueCode::UndeclaredOutcome));
    let proposals = propose(&w, &team());
    let pairs: Vec<_> = proposals[0]
        .edges
        .iter()
        .map(|e| (e.edge_id.as_str(), e.suggested.as_deref()))
        .collect();
    assert_eq!(
        pairs,
        [
            ("qa->done", Some("approved")),
            ("qa->fixer", Some("changes_requested"))
        ]
    );
    apply(
        &mut w,
        &team(),
        &confirm(&[("qa->done", "approved"), ("qa->fixer", "changes_requested")]),
        1,
    )
    .unwrap();
    assert!(check(&w).valid, "{:?}", check(&w).issues);
}
