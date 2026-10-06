//! The guardrail stage of an execution: after the prompt is built and before the runtime starts.
//!
//! It reviews the context the agent is about to receive (`optimization::review`), takes secrets
//! out of context that came from files and other agents, asks the security layer's rules what to
//! do (`security::guardrails`) and records every decision on the execution's one audit trail.
//! Nothing here reads text for instructions: the review looks at the context as data, and the
//! only approval that exists is a person's answer to a `PendingInteraction`.

use std::collections::BTreeMap;

use super::{EditAccess, Emitter, Execution, ExecutionService};
use crate::application::optimization::context::{prompt_items, rebuild_task};
use crate::application::optimization::review::{self, ReviewInput};
use crate::application::optimization::skills::{SkillBlock, SkillPlan};
use crate::application::prompt::Prompt;
use crate::application::security::guardrails::{self, EditGrant, ReviewContext};
use crate::application::support::now_ms;
use crate::domain::execution::{ExecutionEventKind, ExecutionFailure, FailureKind};
use crate::domain::guardrail::{
    AuditScope, ContextHealth, ContextReviewMetrics, ContextReviewResult, GuardrailDecision,
    GuardrailEvaluation, GuardrailMetrics, ReviewAnswer,
};
use crate::domain::interaction::InteractionDetection;
use crate::domain::optimization::{BriefParts, ContextEngineMetrics, SkillMetrics};
use crate::domain::security::Reason;
use crate::domain::task_context::ContextRecord;

/// What the prompt is made of, as the inputs the builder takes. The engine and the guardrails
/// rework these (never a built prompt), and the builder builds again from them.
#[derive(Clone)]
pub(super) struct PromptInputs {
    pub harness: Option<String>,
    pub skills: Vec<SkillBlock>,
    pub description: String,
    pub parts: Option<BriefParts>,
}

impl PromptInputs {
    pub fn skills_text(&self) -> Option<String> {
        if self.skills.is_empty() {
            return None;
        }
        let blocks: Vec<&str> = self.skills.iter().map(|b| b.text.as_str()).collect();
        Some(SkillPlan::render(SkillPlan::notice(), &blocks))
    }
}

/// What to do with the execution.
pub(super) enum Gate {
    Proceed,
    /// Pause it and ask a person (through the workflow's pending interaction).
    Ask(InteractionDetection),
    /// Do not start it.
    Deny(ExecutionFailure),
}

pub(super) struct GuardStage {
    pub review: ContextReviewResult,
    pub metrics: GuardrailMetrics,
    /// The inputs with secrets taken out, when there were any.
    pub redacted: Option<PromptInputs>,
    pub gate: Gate,
}

pub(super) struct GuardContext<'a> {
    pub prompt: &'a Prompt,
    pub agent_instructions: &'a str,
    pub inputs: &'a PromptInputs,
    pub engine: Option<&'a ContextEngineMetrics>,
    pub skills: Option<&'a SkillMetrics>,
    pub record: Option<&'a ContextRecord>,
    /// A workflow step: it can wait for an answer, and its protocols are required.
    pub step: bool,
    /// What a person answered to an earlier question, with the evaluation it was about. It
    /// counts only if that is still this evaluation.
    pub answered: Option<ReviewAnswer>,
    pub access: EditAccess,
    /// What else the decision depends on, so an approval cannot outlive any of it.
    pub binding: Binding,
}

/// Everything besides the context that an approval is bound to.
pub(super) struct Binding {
    pub agent_id: String,
    pub runtime_id: String,
    pub model_id: String,
    /// The runtime's capabilities as launched, rendered.
    pub capabilities: String,
    /// The agent's resolved policies (not the files they came from), rendered.
    pub policy: String,
    /// Where the agent works: the worktree it is given.
    pub worktree: String,
}

impl ExecutionService {
    /// Whether the guardrail stage is on (`optimization.guardrails.enabled`).
    pub(super) fn guardrails_on(&self) -> bool {
        self.optimization
            .as_ref()
            .is_some_and(|flags| flags.guardrails_enabled())
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn guard_before_agent(
        &self,
        emitter: &Emitter<'_>,
        execution: &Execution,
        ctx: &GuardContext<'_>,
    ) -> GuardStage {
        let items = prompt_items(
            ctx.prompt,
            ctx.agent_instructions,
            &ctx.inputs.description,
            ctx.inputs.parts.as_ref(),
            &ctx.inputs.skills,
        );
        let harness_text = ctx.inputs.harness.as_deref().unwrap_or("");
        let review = review::review(&ReviewInput {
            items: &items.items,
            is_step: ctx.step,
            engine: ctx.engine,
            stale_items: ctx
                .record
                .map_or(0, |r| u32::try_from(r.stale_items).unwrap_or(u32::MAX)),
            outdated_in_text: harness_text.contains("WHAT MAY BE OUTDATED"),
            skill_issues: ctx.skills.map_or(0, |s| s.issues),
        });

        let mut evaluations: Vec<GuardrailEvaluation> = Vec::new();
        // Secrets that would travel from files or other agents are taken out first.
        let redactions = review::redactions(&items.items);
        let redacted = (!redactions.is_empty()).then(|| {
            let mut inputs = ctx.inputs.clone();
            for found in &redactions {
                evaluations.push(guardrails::redaction(found.source, found.lines));
                match found.id.as_str() {
                    "task_context" | "harness" => inputs.harness = Some(found.text.clone()),
                    "workflow_context" => {
                        if let Some(parts) = inputs.parts.as_mut() {
                            parts.workflow_context.clone_from(&found.text);
                        }
                    }
                    "handoff" => {
                        if let Some(parts) = inputs.parts.as_mut() {
                            parts.handoff.clone_from(&found.text);
                        }
                    }
                    id => {
                        if let Some(block) = id
                            .strip_prefix("skill:")
                            .and_then(|name| inputs.skills.iter_mut().find(|b| b.name == name))
                        {
                            block.text.clone_from(&found.text);
                        }
                    }
                }
            }
            if let (Some(prefix), Some(parts)) = (&items.prefix, &inputs.parts) {
                inputs.description = rebuild_task(
                    prefix,
                    &parts.workflow_context,
                    &parts.handoff,
                    &parts.protocols,
                );
            }
            inputs
        });

        let evaluation = guardrails::evaluation_fingerprint(&guardrails::Evaluated {
            items: &items.items,
            review: &review,
            interactive: ctx.step,
            access: &format!("{:?}", ctx.access),
            agent_id: &ctx.binding.agent_id,
            runtime_id: &ctx.binding.runtime_id,
            model_id: &ctx.binding.model_id,
            capabilities: &ctx.binding.capabilities,
            policy: &ctx.binding.policy,
            worktree: &ctx.binding.worktree,
        });
        // An answer is for one evaluation: if anything it looked at has changed, it is void and
        // the question is asked again.
        let stale_approval = ctx
            .answered
            .as_ref()
            .is_some_and(|a| a.evaluation != evaluation);
        let answered = ctx
            .answered
            .as_ref()
            .filter(|a| a.evaluation == evaluation)
            .map(|a| a.answered);
        let decision = guardrails::decide_context(
            &review,
            ReviewContext {
                interactive: ctx.step,
                answered,
            },
        );
        evaluations.push(decision.clone());
        match ctx.access {
            EditAccess::Allowed => evaluations.push(guardrails::edit_files(EditGrant::Granted)),
            EditAccess::NotIsolated => {
                evaluations.push(guardrails::edit_files(EditGrant::NotIsolated));
            }
            EditAccess::RuntimeCannotEdit => {
                evaluations.push(guardrails::edit_files(EditGrant::RuntimeCannotEdit));
            }
            // The policy does not allow writing: the normal state of a read-only agent.
            EditAccess::PolicyDenied => {}
        }

        let gate = match decision.decision {
            GuardrailDecision::Ask => Gate::Ask(guardrails::ask(&review, &evaluation)),
            GuardrailDecision::Deny => Gate::Deny(denial(&decision, &review)),
            _ => Gate::Proceed,
        };
        let metrics = GuardrailMetrics::of(&evaluations, matches!(gate, Gate::Deny(_)));
        self.record_guardrails(
            emitter,
            execution,
            &review,
            &evaluations,
            &metrics,
            &decision,
            stale_approval,
        );
        GuardStage {
            review,
            metrics,
            redacted,
            gate,
        }
    }

    /// One entry on the audit trail per evaluation, and the few events worth showing.
    #[allow(clippy::too_many_arguments)]
    fn record_guardrails(
        &self,
        emitter: &Emitter<'_>,
        execution: &Execution,
        review: &ContextReviewResult,
        evaluations: &[GuardrailEvaluation],
        metrics: &GuardrailMetrics,
        context_decision: &GuardrailEvaluation,
        stale_approval: bool,
    ) {
        let scope = AuditScope {
            execution_id: execution.id.clone(),
            workspace_id: emitter.workspace_id.clone(),
            task_id: emitter.task_id.clone(),
            agent_id: emitter.agent_id.clone(),
        };
        for evaluation in evaluations {
            self.audit.record(evaluation.to_event(&scope, now_ms()));
        }
        let review_metrics = ContextReviewMetrics::from(review);
        emitter.announce(
            ExecutionEventKind::OptimizationContextReviewed,
            "Context reviewed".to_owned(),
            BTreeMap::from([
                (
                    "health".to_owned(),
                    format!("{:?}", review_metrics.health).to_lowercase(),
                ),
                ("warnings".to_owned(), review_metrics.warnings.to_string()),
                ("errors".to_owned(), review_metrics.errors.to_string()),
                (
                    "blockingIssues".to_owned(),
                    review_metrics.blocking_issues.to_string(),
                ),
                (
                    "staleItems".to_owned(),
                    review_metrics.stale_items.to_string(),
                ),
            ]),
        );
        let facts = |e: &GuardrailEvaluation| {
            BTreeMap::from([
                ("rule".to_owned(), e.rule.clone()),
                (
                    "reason".to_owned(),
                    e.reason.map_or_else(String::new, |r| r.as_str().to_owned()),
                ),
                ("matched".to_owned(), e.matched_rules.join(",")),
                ("action".to_owned(), e.action.clone()),
            ])
        };
        match context_decision.decision {
            GuardrailDecision::Ask => {
                let mut asked = facts(context_decision);
                if stale_approval {
                    // The earlier answer was for a different evaluation: it does not carry over.
                    asked.insert("staleApproval".to_owned(), "true".to_owned());
                }
                emitter.announce(
                    ExecutionEventKind::OptimizationGuardrailAsked,
                    if stale_approval {
                        "Context changed since it was approved: a person must decide again"
                    } else {
                        "Context needs a person's decision"
                    }
                    .to_owned(),
                    asked,
                );
            }
            GuardrailDecision::Deny if review.health == ContextHealth::Invalid => emitter.announce(
                ExecutionEventKind::OptimizationContextReviewBlocked,
                "Context is not fit to send".to_owned(),
                facts(context_decision),
            ),
            _ => {}
        }
        for denied in evaluations
            .iter()
            .filter(|e| e.decision == GuardrailDecision::Deny)
        {
            emitter.announce(
                ExecutionEventKind::OptimizationGuardrailDenied,
                "Guardrail denied".to_owned(),
                facts(denied),
            );
        }
        emitter.announce(
            ExecutionEventKind::OptimizationGuardrailEvaluated,
            "Guardrails evaluated".to_owned(),
            BTreeMap::from([
                ("evaluations".to_owned(), metrics.evaluations.to_string()),
                ("allowed".to_owned(), metrics.allowed.to_string()),
                ("asked".to_owned(), metrics.asked.to_string()),
                ("denied".to_owned(), metrics.denied.to_string()),
                ("transformed".to_owned(), metrics.transformed.to_string()),
                ("blocked".to_owned(), metrics.blocked.to_string()),
            ]),
        );
    }
}

/// The failure an execution gets when it is not started.
fn denial(decision: &GuardrailEvaluation, review: &ContextReviewResult) -> ExecutionFailure {
    let (message, reason) = match decision.reason {
        Some(Reason::ApprovalRejected) => (
            "The person declined to run this step with the context Atlas flagged.".to_owned(),
            Reason::ApprovalRejected,
        ),
        _ => (
            "Atlas did not start the agent: its context is not fit to send.".to_owned(),
            Reason::ContextInvalid,
        ),
    };
    let blocking: Vec<String> = review
        .issues
        .iter()
        .filter(|i| i.severity >= crate::domain::guardrail::IssueSeverity::Error)
        .map(|i| format!("{}: {}", i.code.as_str(), i.message))
        .collect();
    ExecutionFailure {
        kind: FailureKind::PermissionDenied,
        message,
        details: Some(format!(
            "rule {}; reason {}; {}",
            decision.rule,
            reason.as_str(),
            blocking.join("; ")
        )),
    }
}
