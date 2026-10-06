use super::*;
use crate::domain::guardrail::{
    ContextReviewResult, IssueCode, IssueSeverity, ReviewIssue, SourceTrust,
};
use crate::domain::security::{DecisionSource, PermissionOutcome};

fn result(health: ContextHealth, issues: Vec<ReviewIssue>) -> ContextReviewResult {
    ContextReviewResult {
        health,
        issues,
        sources: Vec::new(),
        required_items: 3,
        high_items: 0,
        normal_items: 0,
        optional_items: 0,
        stale_items: 0,
    }
}

fn issue(code: IssueCode, severity: IssueSeverity) -> ReviewIssue {
    ReviewIssue {
        code,
        severity,
        source: SectionKind::TaskContext,
        other_source: Some(SectionKind::AgentInstructions),
        message: "two sources disagree on the database: PostgreSQL vs MongoDB".to_owned(),
        excerpt: String::new(),
    }
}

fn conflict() -> ContextReviewResult {
    result(
        ContextHealth::NeedsReview,
        vec![issue(
            IssueCode::ConflictingInstructions,
            IssueSeverity::Error,
        )],
    )
}

const ASKABLE: ReviewContext = ReviewContext {
    interactive: true,
    answered: None,
};

#[test]
fn a_context_that_is_fine_or_only_worth_knowing_about_is_allowed() {
    for health in [ContextHealth::Healthy, ContextHealth::Partial] {
        let decision = decide_context(&result(health, vec![]), ASKABLE);
        assert_eq!(decision.decision, GuardrailDecision::Allow);
        assert_eq!(decision.reason, None);
        assert!(!decision.requires_approval);
    }
}

#[test]
fn a_context_that_needs_review_asks_when_someone_can_answer() {
    let decision = decide_context(&conflict(), ASKABLE);

    assert_eq!(decision.decision, GuardrailDecision::Ask);
    assert!(decision.requires_approval);
    assert_eq!(decision.reason, Some(Reason::ContextNeedsReview));
    assert_eq!(decision.rule, "context.needs_review");
    assert_eq!(decision.matched_rules, ["conflicting_instructions"]);
    assert_eq!(decision.stage, GuardrailStage::BeforeAgent);
}

#[test]
fn only_an_answer_from_a_person_settles_an_ask() {
    let allowed = decide_context(
        &conflict(),
        ReviewContext {
            interactive: true,
            answered: Some(Answered::Allowed),
        },
    );
    assert_eq!(allowed.decision, GuardrailDecision::Allow);
    assert!(allowed.decided_by_user);
    assert_eq!(allowed.rule, "context.needs_review.approved");

    let declined = decide_context(
        &conflict(),
        ReviewContext {
            interactive: true,
            answered: Some(Answered::Declined),
        },
    );
    assert_eq!(declined.decision, GuardrailDecision::Deny);
    assert!(declined.decided_by_user);
    assert_eq!(declined.reason, Some(Reason::ApprovalRejected));
}

#[test]
fn with_nobody_to_ask_the_findings_are_reported_and_the_run_goes_on() {
    let decision = decide_context(
        &conflict(),
        ReviewContext {
            interactive: false,
            answered: None,
        },
    );

    assert_eq!(decision.decision, GuardrailDecision::Allow);
    assert_eq!(decision.rule, "context.needs_review.unattended");
    assert!(!decision.decided_by_user);
}

#[test]
fn an_invalid_context_is_never_sent_whatever_anyone_answers() {
    let invalid = result(
        ContextHealth::Invalid,
        vec![issue(IssueCode::MissingRequired, IssueSeverity::Blocking)],
    );

    for answered in [None, Some(Answered::Allowed), Some(Answered::Declined)] {
        for interactive in [true, false] {
            let decision = decide_context(
                &invalid,
                ReviewContext {
                    interactive,
                    answered,
                },
            );
            assert_eq!(
                decision.decision,
                GuardrailDecision::Deny,
                "{answered:?} {interactive}"
            );
            assert_eq!(decision.reason, Some(Reason::ContextInvalid));
            assert_eq!(decision.matched_rules, ["missing_required"]);
        }
    }
}

#[test]
fn edit_tools_are_granted_only_inside_an_isolated_worktree_and_say_why_not() {
    let granted = edit_files(EditGrant::Granted);
    assert_eq!(granted.decision, GuardrailDecision::Allow);
    assert_eq!(granted.rule, "write.worktree_scope");

    let loose = edit_files(EditGrant::NotIsolated);
    assert_eq!(loose.decision, GuardrailDecision::Deny);
    assert_eq!(loose.reason, Some(Reason::WriteNotIsolated));

    let incapable = edit_files(EditGrant::RuntimeCannotEdit);
    assert_eq!(incapable.decision, GuardrailDecision::Deny);
    assert_eq!(incapable.reason, Some(Reason::RuntimeCannotEdit));
}

#[test]
fn removing_a_secret_is_a_transformation_that_never_carries_the_content() {
    let evaluation = redaction(SectionKind::BriefHandoff, 2);

    assert_eq!(evaluation.decision, GuardrailDecision::Transform);
    assert_eq!(evaluation.reason, Some(Reason::SecretsRedacted));
    assert_eq!(
        evaluation.transformation.as_deref(),
        Some("2 line(s) redacted")
    );
    assert_eq!(evaluation.target, "briefhandoff");
}

#[test]
fn the_question_to_the_person_comes_from_the_findings_and_offers_atlas_s_own_buttons() {
    let detection = ask(&conflict(), "ev-1");
    assert_eq!(detection.evaluation.as_deref(), Some("ev-1"));

    assert_eq!(detection.kind, Some(InteractionKind::Permission));
    assert_eq!(detection.source, DetectionSource::Guardrail);
    assert_eq!(detection.confidence, 100);
    assert!(detection.question.contains("1 problem(s)"));
    assert!(detection.context.contains("conflicting_instructions"));
    assert!(detection.context.contains("PostgreSQL vs MongoDB"));
    assert!(detection
        .context
        .contains("taskcontext / agentinstructions"));
    let ids: Vec<&str> = detection.options.iter().map(|o| o.id.as_str()).collect();
    assert_eq!(ids, ["allow", "deny"]);
    assert!(detection.question.len() <= MAX_QUESTION && detection.context.len() <= MAX_CONTEXT);
}

#[test]
fn every_evaluation_becomes_an_entry_of_the_one_audit_trail() {
    let scope = crate::domain::guardrail::AuditScope {
        execution_id: "exec-1".to_owned(),
        workspace_id: "ws-1".to_owned(),
        task_id: "task-1".to_owned(),
        agent_id: "agent-1".to_owned(),
    };

    let denied = decide_context(
        &result(
            ContextHealth::Invalid,
            vec![issue(IssueCode::MissingRequired, IssueSeverity::Blocking)],
        ),
        ASKABLE,
    )
    .to_event(&scope, 42);
    assert_eq!(denied.decision, PermissionOutcome::Denied);
    assert_eq!(denied.source, DecisionSource::Policy);
    assert_eq!(denied.reason, Some(Reason::ContextInvalid));
    assert!(denied.notes.contains(&"rule:context.invalid".to_owned()));
    assert!(denied
        .notes
        .contains(&"matched:missing_required".to_owned()));
    assert_eq!(
        (denied.execution_id.as_str(), denied.timestamp),
        ("exec-1", 42)
    );

    let asked = decide_context(&conflict(), ASKABLE).to_event(&scope, 1);
    assert_eq!(asked.decision, PermissionOutcome::ApprovalRequested);

    let approved = decide_context(
        &conflict(),
        ReviewContext {
            interactive: true,
            answered: Some(Answered::Allowed),
        },
    )
    .to_event(&scope, 2);
    assert_eq!(approved.decision, PermissionOutcome::Approved);
    assert_eq!(approved.source, DecisionSource::User);

    let transformed = redaction(SectionKind::Skills, 1).to_event(&scope, 3);
    assert_eq!(transformed.decision, PermissionOutcome::Transformed);
    assert!(transformed
        .notes
        .contains(&"transformed:1 line(s) redacted".to_owned()));
    // What it concerned is a name for display; never the content.
    assert_eq!(transformed.target, "skills");
    let _ = SourceTrust::Atlas;
}

#[test]
fn a_permission_decision_maps_onto_the_same_four_answers() {
    use crate::domain::security::PermissionDecision;
    assert_eq!(
        GuardrailDecision::from(PermissionDecision::Allowed),
        GuardrailDecision::Allow
    );
    assert_eq!(
        GuardrailDecision::from(PermissionDecision::RequiresApproval),
        GuardrailDecision::Ask
    );
    assert_eq!(
        GuardrailDecision::from(PermissionDecision::Denied),
        GuardrailDecision::Deny
    );
}
