//! Builds the pieces of the Context & Tooling Platform (ADR 0026) from a running execution: the
//! budget, the plan and the manifest. It composes nothing and decides nothing: `PromptBuilder`
//! stays the only assembly, the guardrails the only gate, and the runtime adapter the only owner
//! of the payload (`ModelRuntime::delivery`). This only reads what they produced.

use crate::application::runtimes::{sha256_hex, Delivery, ModelRuntime};
use crate::domain::context::{
    ContextManifest, ContextPlan, ExecutionBudget, Figure, LimitOverride, ManifestIds,
    ManifestRule, ModelLimits, OverrideLayer, RuntimeSurface,
};
use crate::domain::mcp::McpRecord;
use crate::domain::optimization::PromptBreakdown;

/// The budget of one execution of `model_id` through `runtime`: what the runtime states about the
/// model (nothing, today), narrowed by the user's configured limit when there is one in force.
pub fn execution_budget(
    runtime: &dyn ModelRuntime,
    model_id: &str,
    configured_input_tokens: Option<u64>,
) -> ExecutionBudget {
    let runtime_id = runtime.info().id;
    let base = runtime.reported_limits(model_id).map_or_else(
        || ModelLimits::unknown(&runtime_id, model_id),
        |stated| ModelLimits::reported(&runtime_id, model_id, stated),
    );
    let overrides: Vec<LimitOverride> = configured_input_tokens
        .map(|tokens| LimitOverride::input(OverrideLayer::Settings, tokens))
        .into_iter()
        .collect();
    ExecutionBudget::resolve(base, &overrides)
}

/// A digest of a prompt's sections (kind and size), to tell a plan from a prompt that changed.
fn sections_fingerprint(breakdown: &PromptBreakdown) -> String {
    let text = breakdown
        .sections
        .iter()
        .map(|s| format!("{:?}:{}", s.section, s.bytes))
        .collect::<Vec<_>>()
        .join(";");
    sha256_hex(text.as_bytes())
}

/// The plan: the prompt as built (and reworked by the Context Engine), before the guardrails.
pub fn plan_of(breakdown: &PromptBreakdown, engine_omitted_items: u32) -> ContextPlan {
    ContextPlan::from_breakdown(
        breakdown,
        sections_fingerprint(breakdown),
        engine_omitted_items,
    )
}

/// The budget with the prompt's size on it, as an estimate.
pub fn measured(budget: ExecutionBudget, prompt: &PromptBreakdown) -> ExecutionBudget {
    budget.measured(Figure::estimated(prompt.estimated_tokens))
}

pub struct ManifestSource<'a> {
    pub ids: ManifestIds,
    pub created_at: u64,
    pub plan: &'a ContextPlan,
    /// The prompt as it is about to be delivered (after the guardrails).
    pub delivered: &'a PromptBreakdown,
    pub delivery: &'a Delivery,
    /// False when the guardrails stopped or paused the step.
    pub will_deliver: bool,
    pub rules: Vec<ManifestRule>,
    pub mcp: McpRecord,
    pub surface: RuntimeSurface,
    pub budget: &'a ExecutionBudget,
}

pub fn manifest_of(source: &ManifestSource<'_>) -> ContextManifest {
    let mut manifest = ContextManifest::new(
        source.ids.clone(),
        source.created_at,
        source.plan.fingerprint.clone(),
        sections_fingerprint(source.delivered) != source.plan.fingerprint,
        source.delivered.sections.clone(),
        source.delivery.record(source.will_deliver),
        source.rules.clone(),
        source.mcp.clone(),
        source.surface.clone(),
    );
    manifest.refresh_warnings(&source.budget.warnings());
    manifest
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::prompt::Prompt;
    use crate::domain::context::{ContextWarning, Precision};
    use crate::domain::optimization::{SectionKind, TextSize};
    use crate::domain::runtime::SystemPromptChannel;

    fn breakdown(task_bytes: usize) -> PromptBreakdown {
        let size = |n| TextSize { bytes: n, chars: n };
        PromptBreakdown::new(
            size(100 + task_bytes),
            [
                (SectionKind::AtlasRules, size(60)),
                (SectionKind::Task, size(task_bytes)),
            ],
        )
    }

    fn prompt() -> Prompt {
        Prompt {
            system: "SYS".to_owned(),
            harness: None,
            task_aware: false,
            skills: None,
            rules: None,
            context: "CTX".to_owned(),
            instruction: "INS".to_owned(),
        }
    }

    fn ids() -> ManifestIds {
        ManifestIds {
            execution: "exec-1".to_owned(),
            workspace: "w".to_owned(),
            task: "t".to_owned(),
            agent: "a".to_owned(),
            runtime: "rt".to_owned(),
            model: "m".to_owned(),
        }
    }

    fn manifest(plan_task: usize, delivered_task: usize, will_deliver: bool) -> ContextManifest {
        let plan = plan_of(&breakdown(plan_task), 0);
        let delivered = breakdown(delivered_task);
        let budget = ExecutionBudget::resolve(ModelLimits::unknown("rt", "m"), &[]);
        let delivery = Delivery::of_prompt(&prompt());
        manifest_of(&ManifestSource {
            ids: ids(),
            created_at: 7,
            plan: &plan,
            delivered: &delivered,
            delivery: &delivery,
            will_deliver,
            rules: Vec::new(),
            mcp: McpRecord::default(),
            surface: RuntimeSurface::baseline("rt", SystemPromptChannel::Unsupported),
            budget: &budget,
        })
    }

    #[test]
    fn the_plan_is_the_prompt_sections_estimated_not_counted() {
        let plan = plan_of(&breakdown(40), 2);

        assert_eq!(plan.total_bytes, 140);
        assert_eq!(plan.total_tokens.value, Some(35));
        assert_eq!(plan.total_tokens.precision, Precision::Estimated);
        assert_eq!(plan.engine_omitted_items, 2);
        let kinds: Vec<_> = plan.sections.iter().map(|s| s.section).collect();
        assert_eq!(
            kinds,
            [
                SectionKind::AtlasRules,
                SectionKind::Task,
                SectionKind::Framing
            ]
        );
        // The same sections plan to the same fingerprint; another size does not.
        assert_eq!(plan.fingerprint, plan_of(&breakdown(40), 0).fingerprint);
        assert_ne!(plan.fingerprint, plan_of(&breakdown(41), 0).fingerprint);
    }

    #[test]
    fn the_manifest_carries_the_ids_the_plan_and_the_hash_of_the_payload() {
        let manifest = manifest(40, 40, true);

        assert_eq!(manifest.execution_id, "exec-1");
        assert_eq!(
            (manifest.runtime_id.as_str(), manifest.model_id.as_str()),
            ("rt", "m")
        );
        assert_eq!(manifest.created_at, 7);
        assert_eq!(
            manifest.plan_fingerprint,
            plan_of(&breakdown(40), 0).fingerprint
        );
        assert!(!manifest.diverged_from_plan);
        assert_eq!(manifest.sections.len(), 3);
        assert!(manifest.delivery.delivered);
        assert_eq!(
            manifest.delivery.prompt_hash,
            Delivery::of_prompt(&prompt()).prompt_hash()
        );
        assert_eq!(
            manifest.delivery.bytes,
            Delivery::of_prompt(&prompt()).body().len()
        );
    }

    #[test]
    fn a_prompt_changed_after_the_plan_is_said_to_differ() {
        let manifest = manifest(40, 30, true);

        assert!(manifest.diverged_from_plan);
        assert!(manifest
            .warnings
            .contains(&ContextWarning::DivergedFromPlan));
    }

    #[test]
    fn a_step_the_guardrails_held_back_is_not_called_delivered() {
        let manifest = manifest(40, 40, false);

        assert!(!manifest.delivery.delivered);
        assert!(manifest.warnings.contains(&ContextWarning::NotDelivered));
    }

    #[test]
    fn the_warnings_say_what_is_estimated_unknown_and_unobserved() {
        let manifest = manifest(40, 40, true);

        for expected in [
            ContextWarning::TokensEstimated,
            ContextWarning::ModelLimitUnknown,
            ContextWarning::SurfacePartlyUnobserved,
        ] {
            assert!(manifest.warnings.contains(&expected), "{expected:?}");
        }
    }
}
