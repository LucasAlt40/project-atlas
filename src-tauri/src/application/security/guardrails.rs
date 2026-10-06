//! Guardrail decisions: the security authority's answer, in one shape.
//!
//! This is not a second policy engine. The permission guard (`guard`, `evaluator`) still decides
//! what may start; the policy (`SecurityPolicy`, `restrict`) still decides what is granted; the
//! audit trail (`AuditLog`, `PermissionEvent`) still records it; an `ASK` is still a
//! `PendingInteraction`. What lives here is the part those did not cover: the decisions about
//! what Atlas hands an agent (its context) and what it grants it (edit tools) before it starts,
//! stated as `ALLOW`, `ASK`, `DENY` or `TRANSFORM`, with a rule, a reason and an audit entry.
//!
//! Everything is a deterministic function of its inputs. Nothing here reads text for
//! instructions, and nothing a model, a skill, a handoff or a file says can be an input: the
//! only way an `ASK` becomes an approval is [`Answered`], which comes from an answered
//! `PendingInteraction` in the workflow run.

use std::fmt::Write as _;

use crate::application::harness::fingerprint::digest_text;
use crate::application::optimization::context::ContextItem;
use crate::domain::guardrail::{
    Answered, ContextHealth, ContextReviewResult, GuardrailDecision, GuardrailEvaluation,
    GuardrailStage, IssueSeverity, ReviewIssue,
};
use crate::domain::interaction::{
    decision_options, DetectionSource, InteractionDetection, InteractionKind, MAX_CONTEXT,
    MAX_QUESTION,
};
use crate::domain::optimization::SectionKind;
use crate::domain::security::Reason;

/// What the guardrail needs to know about the execution that is about to start.
#[derive(Debug, Clone, Copy)]
pub struct ReviewContext {
    /// There is a way to ask a person: a workflow step can wait for an answer; a message typed
    /// in a conversation cannot.
    pub interactive: bool,
    /// What a person answered to an earlier `ASK` for this step, if anything.
    pub answered: Option<Answered>,
}

fn codes(issues: &[ReviewIssue], at_least: IssueSeverity) -> Vec<String> {
    let mut codes: Vec<String> = issues
        .iter()
        .filter(|i| i.severity >= at_least)
        .map(|i| i.code.as_str().to_owned())
        .collect();
    codes.dedup();
    codes
}

/// May an agent be started with this context?
///
/// | Health        | Answer                                                                |
/// | ------------- | --------------------------------------------------------------------- |
/// | `Healthy`     | `ALLOW`                                                               |
/// | `Partial`     | `ALLOW` (warnings are reported, not obstacles)                        |
/// | `NeedsReview` | `ASK` when someone can answer; their answer decides; else `ALLOW`     |
/// | `Invalid`     | `DENY`, whatever anyone says: it must not be sent                     |
pub fn decide_context(review: &ContextReviewResult, ctx: ReviewContext) -> GuardrailEvaluation {
    let evaluation = |decision, rule: &str| {
        GuardrailEvaluation::new(
            GuardrailStage::BeforeAgent,
            "launch_context",
            decision,
            rule,
            "context",
        )
    };
    match review.health {
        ContextHealth::Healthy => evaluation(GuardrailDecision::Allow, "context.healthy"),
        ContextHealth::Partial => evaluation(GuardrailDecision::Allow, "context.partial")
            .matching(codes(&review.issues, IssueSeverity::Warning)),
        ContextHealth::Invalid => evaluation(GuardrailDecision::Deny, "context.invalid")
            .because(Reason::ContextInvalid)
            .matching(codes(&review.issues, IssueSeverity::Blocking)),
        ContextHealth::NeedsReview => {
            let matched = codes(&review.issues, IssueSeverity::Error);
            match ctx.answered {
                Some(Answered::Allowed) => {
                    let mut approved =
                        evaluation(GuardrailDecision::Allow, "context.needs_review.approved")
                            .because(Reason::ContextNeedsReview)
                            .matching(matched);
                    approved.decided_by_user = true;
                    approved
                }
                Some(Answered::Declined) => {
                    let mut declined =
                        evaluation(GuardrailDecision::Deny, "context.needs_review.declined")
                            .because(Reason::ApprovalRejected)
                            .matching(matched);
                    declined.decided_by_user = true;
                    declined
                }
                None if ctx.interactive => {
                    evaluation(GuardrailDecision::Ask, "context.needs_review")
                        .because(Reason::ContextNeedsReview)
                        .matching(matched)
                }
                None => evaluation(GuardrailDecision::Allow, "context.needs_review.unattended")
                    .because(Reason::ContextNeedsReview)
                    .matching(matched),
            }
        }
    }
}

/// Secrets taken out of context before it is sent: the one transformation Atlas makes, because
/// the rule is exact (a line that looks like a secret is replaced by a notice) and safe.
pub fn redaction(source: SectionKind, lines: usize) -> GuardrailEvaluation {
    let mut evaluation = GuardrailEvaluation::new(
        GuardrailStage::BeforeAgent,
        "launch_context",
        GuardrailDecision::Transform,
        "secrets.redact",
        &format!("{source:?}").to_lowercase(),
    )
    .because(Reason::SecretsRedacted);
    evaluation.transformation = Some(format!("{lines} line(s) redacted"));
    evaluation
}

/// Why an execution that should be able to edit files cannot, or that it can.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditGrant {
    Granted,
    NotIsolated,
    RuntimeCannotEdit,
}

/// Edit tools are granted only when the agent's policy allows writing, the agent works in an
/// isolated worktree and its runtime can be launched with such tools. The three checks already
/// exist where they are made; this puts their outcome on the audit trail. (A policy that does not
/// allow writing is the normal state of a read-only agent, and nobody asked: it is not recorded.)
pub fn edit_files(grant: EditGrant) -> GuardrailEvaluation {
    let evaluation = |decision, rule: &str| {
        GuardrailEvaluation::new(
            GuardrailStage::BeforeAgent,
            "edit_files",
            decision,
            rule,
            "edit tools",
        )
    };
    match grant {
        EditGrant::Granted => evaluation(GuardrailDecision::Allow, "write.worktree_scope"),
        EditGrant::NotIsolated => evaluation(GuardrailDecision::Deny, "write.not_isolated")
            .because(Reason::WriteNotIsolated),
        EditGrant::RuntimeCannotEdit => {
            evaluation(GuardrailDecision::Deny, "write.runtime_capability")
                .because(Reason::RuntimeCannotEdit)
        }
    }
}

/// What a person is asked when the context needs a look. The kind and the buttons are Atlas's
/// (`Permission`: allow or deny); the text is built from the review's findings, never from the
/// context itself, so nothing in a skill or a handoff can word the question.
pub fn ask(review: &ContextReviewResult, evaluation: &str) -> InteractionDetection {
    let problems: Vec<&ReviewIssue> = review
        .issues
        .iter()
        .filter(|i| i.severity == IssueSeverity::Error)
        .collect();
    let mut question = format!(
        "Atlas checked the context for this step and found {} problem(s) a person should look at \
         before the agent starts. Allow it to run anyway?",
        problems.len()
    );
    question.truncate(floor_boundary(&question, MAX_QUESTION));
    let mut context = String::new();
    for issue in &problems {
        let others = issue
            .other_source
            .map_or_else(String::new, |o| format!(" / {o:?}").to_lowercase());
        let source = format!("{:?}", issue.source).to_lowercase();
        let _ = writeln!(
            context,
            "- {} ({source}{others}): {}",
            issue.code.as_str(),
            issue.message
        );
    }
    context.truncate(floor_boundary(&context, MAX_CONTEXT));
    InteractionDetection {
        detected: true,
        kind: Some(InteractionKind::Permission),
        confidence: 100,
        question,
        context: context.trim_end().to_owned(),
        document: String::new(),
        options: decision_options(InteractionKind::Permission),
        source: DetectionSource::Guardrail,
        evaluation: Some(evaluation.to_owned()),
    }
}

/// Everything one context evaluation looked at.
pub struct Evaluated<'a> {
    pub items: &'a [ContextItem],
    pub review: &'a ContextReviewResult,
    pub interactive: bool,
    pub access: &'a str,
    pub agent_id: &'a str,
    pub runtime_id: &'a str,
    pub model_id: &'a str,
    pub capabilities: &'a str,
    pub policy: &'a str,
    pub worktree: &'a str,
}

/// The identity of an evaluation: a digest of the context (each item's source and content), the
/// review's findings, the agent, runtime, model and capabilities, the resolved policy, the
/// worktree and what edit access was granted. An approval carries it; change any of those and
/// the digest changes, so the approval is for something that no longer exists. (It is a
/// fingerprint of data Atlas itself assembled, not a signature: it protects against stale
/// approvals, not against someone who can already write Atlas's run files.)
pub fn evaluation_fingerprint(evaluated: &Evaluated<'_>) -> String {
    let mut canonical = String::from("atlas.guardrail.evaluation.v1\n");
    let mut line = |key: &str, value: &str| {
        let _ = writeln!(canonical, "{key}={value}");
    };
    line("stage", "before_agent");
    line("interactive", &evaluated.interactive.to_string());
    line("agent", evaluated.agent_id);
    line("runtime", evaluated.runtime_id);
    line("model", evaluated.model_id);
    line("capabilities", evaluated.capabilities);
    line("policy", evaluated.policy);
    line("worktree", evaluated.worktree);
    line("access", evaluated.access);
    line("health", &format!("{:?}", evaluated.review.health));
    for item in evaluated.items {
        line(
            "item",
            &format!("{}|{:?}|{}", item.id, item.source, item.fingerprint),
        );
    }
    for issue in &evaluated.review.issues {
        line(
            "issue",
            &format!(
                "{}|{:?}|{:?}|{:?}",
                issue.code.as_str(),
                issue.severity,
                issue.source,
                issue.other_source
            ),
        );
    }
    digest_text(&canonical)
}

fn floor_boundary(text: &str, max: usize) -> usize {
    let mut end = text.len().min(max);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    end
}

#[cfg(test)]
mod tests;
