//! `ModelLimits` (what a runtime and model accept) and `ExecutionBudget` (what one execution may
//! use). Both are about one **runtime + model** pair, never the model alone: the same model can run
//! with different limits through two runtimes.
//!
//! Nothing here knows a model. When no layer states a limit, the limit is `Unknown`; it is never
//! filled with a plausible number. Atlas has no table of model limits, and no runtime reports one
//! today, so in practice the only known figure is what the user configured (see ADR 0026).

use serde::{Deserialize, Serialize};

use super::figure::{Figure, FigureSource, Precision};
use super::ContextWarning;

/// Defaults that belong to the budget. Every number the budget needs lives here.
pub mod defaults {
    /// How much of the Harness a prompt may carry, in characters (the Harness renders text, not
    /// tokens). This was the Harness's own `ContextBudget`; it is a section allocation of the
    /// execution, not a model limit.
    pub const HARNESS_CHARS: usize = 6_000;
}

/// Which side of the window a limit is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitDimension {
    /// What the model accepts as input.
    Input,
    /// What the model may write back.
    Output,
    /// The window shared by input and output, when the model states one.
    Total,
}

/// Where an override comes from. A later layer can only narrow what an earlier one set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverrideLayer {
    /// `optimization.context.maxTokens`, the user's application-wide setting.
    Settings,
    /// A workspace's own limit (the layer exists; nothing stores one yet).
    Workspace,
    /// An agent's or workflow's own limit (the layer exists; nothing stores one yet).
    Agent,
}

/// What a runtime states about a model's limits. No runtime does today.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportedLimits {
    pub input: Option<u64>,
    pub output: Option<u64>,
    pub total: Option<u64>,
    /// What the runtime claims about its own numbers.
    pub precision: Precision,
}

/// What a limit override asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimitOverride {
    pub layer: OverrideLayer,
    pub input: Option<u64>,
    pub output: Option<u64>,
    pub total: Option<u64>,
}

impl LimitOverride {
    pub fn input(layer: OverrideLayer, tokens: u64) -> Self {
        Self {
            layer,
            input: Some(tokens),
            output: None,
            total: None,
        }
    }
}

/// What happened to an override.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResolutionOutcome {
    /// Nothing was known; the override became the limit.
    Applied,
    /// A known limit was lowered.
    Narrowed,
    /// The override equals what was already there.
    Kept,
    /// The override asked for more than a limit already known; the known limit stays.
    #[serde(rename_all = "camelCase")]
    Clamped {
        limit: u64,
        limit_source: FigureSource,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitResolution {
    pub layer: OverrideLayer,
    pub dimension: LimitDimension,
    pub requested: u64,
    pub outcome: ResolutionOutcome,
}

/// The limits of one runtime + model pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelLimits {
    pub runtime_id: String,
    pub model_id: String,
    pub input: Figure,
    pub output: Figure,
    pub total: Figure,
}

impl ModelLimits {
    /// Nothing is known about this pair.
    pub fn unknown(runtime_id: &str, model_id: &str) -> Self {
        Self {
            runtime_id: runtime_id.to_owned(),
            model_id: model_id.to_owned(),
            input: Figure::unknown(),
            output: Figure::unknown(),
            total: Figure::unknown(),
        }
    }

    /// What the runtime itself stated.
    pub fn reported(runtime_id: &str, model_id: &str, stated: ReportedLimits) -> Self {
        let figure = |value: Option<u64>| {
            value.map_or_else(Figure::unknown, |v| Figure::reported(v, stated.precision))
        };
        Self {
            input: figure(stated.input),
            output: figure(stated.output),
            total: figure(stated.total),
            ..Self::unknown(runtime_id, model_id)
        }
    }

    /// Applies overrides in order. An override can only narrow: it never raises a limit that is
    /// already known, and a `Reported` or `Exact` limit is a ceiling nothing above it survives.
    /// Every decision is returned, so the effective numbers can be explained.
    #[must_use]
    pub fn resolve(mut self, overrides: &[LimitOverride]) -> (Self, Vec<LimitResolution>) {
        let mut log = Vec::new();
        for over in overrides {
            for (dimension, requested) in [
                (LimitDimension::Input, over.input),
                (LimitDimension::Output, over.output),
                (LimitDimension::Total, over.total),
            ] {
                let Some(requested) = requested else {
                    continue;
                };
                let current = match dimension {
                    LimitDimension::Input => &mut self.input,
                    LimitDimension::Output => &mut self.output,
                    LimitDimension::Total => &mut self.total,
                };
                let outcome = narrow(current, requested);
                log.push(LimitResolution {
                    layer: over.layer,
                    dimension,
                    requested,
                    outcome,
                });
            }
        }
        (self, log)
    }
}

fn narrow(current: &mut Figure, requested: u64) -> ResolutionOutcome {
    match current.value {
        None => {
            *current = Figure::configured(requested);
            ResolutionOutcome::Applied
        }
        Some(known) if requested < known => {
            *current = Figure::configured(requested);
            ResolutionOutcome::Narrowed
        }
        Some(known) if requested == known => ResolutionOutcome::Kept,
        Some(known) => ResolutionOutcome::Clamped {
            limit: known,
            limit_source: current.source,
        },
    }
}

/// How the prompt compares with the input limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputBudget {
    pub limit: Figure,
    pub used: Figure,
    pub remaining: Figure,
}

/// The budget of one execution: the limits of its runtime + model, what is reserved, and how much
/// of the input the prompt uses. Independent of any runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionBudget {
    pub runtime_id: String,
    pub model_id: String,
    /// The limits after every override.
    pub limits: ModelLimits,
    /// Tokens kept for the answer. Unknown until a policy sets it.
    pub output_reserve: Figure,
    /// Tokens kept as slack. Unknown until a policy sets it.
    pub safety_margin: Figure,
    /// What the MCP tools' definitions weigh in the window. Unknown: the runtimes' start-up reports
    /// list names, not descriptions or schemas, and the runtime counts them inside its own request.
    /// Atlas does not guess a figure for what it cannot see.
    #[serde(default = "Figure::unknown")]
    pub tool_definitions: Figure,
    pub input: InputBudget,
    /// How the limits were reached.
    pub resolution: Vec<LimitResolution>,
}

impl ExecutionBudget {
    /// The budget for `base` (what the runtime states, or nothing) once `overrides` are applied.
    /// The prompt is not measured yet (`used` is unknown).
    pub fn resolve(base: ModelLimits, overrides: &[LimitOverride]) -> Self {
        let (limits, resolution) = base.resolve(overrides);
        let mut budget = Self {
            runtime_id: limits.runtime_id.clone(),
            model_id: limits.model_id.clone(),
            input: InputBudget {
                limit: Figure::unknown(),
                used: Figure::unknown(),
                remaining: Figure::unknown(),
            },
            limits,
            output_reserve: Figure::unknown(),
            safety_margin: Figure::unknown(),
            tool_definitions: Figure::unknown(),
            resolution,
        };
        budget.input.limit = budget.derive_input_limit();
        budget
    }

    /// Sets what is kept for the answer and as slack, and derives the input limit again. No policy
    /// sets a reserve yet (both stay unknown), so only the tests call it.
    #[cfg_attr(not(test), allow(dead_code))]
    #[must_use]
    pub fn with_reserves(mut self, output_reserve: Figure, safety_margin: Figure) -> Self {
        self.output_reserve = output_reserve;
        self.safety_margin = safety_margin;
        self.input.limit = self.derive_input_limit();
        let used = self.input.used;
        self.measured(used)
    }

    /// The input limit: what the model states as input, or its window minus the reserves, and the
    /// smaller of the two when both are known.
    fn derive_input_limit(&self) -> Figure {
        let direct = self.limits.input;
        let reserve = if self.output_reserve.is_known() {
            self.output_reserve
        } else {
            self.limits.output
        };
        let from_total = match (self.limits.total.value, reserve.value) {
            (Some(total), Some(kept)) => {
                let margin = self.safety_margin.value.unwrap_or(0);
                let mut precision = self.limits.total.precision.weakest(reserve.precision);
                if self.safety_margin.is_known() {
                    precision = precision.weakest(self.safety_margin.precision);
                }
                Some(Figure {
                    value: Some(total.saturating_sub(kept).saturating_sub(margin)),
                    source: self.limits.total.source,
                    precision,
                    method: None,
                })
            }
            _ => None,
        };
        match (direct.value, from_total) {
            (Some(d), Some(t)) if t.value.is_some_and(|t| t < d) => t,
            (Some(_), _) => direct,
            (None, Some(t)) => t,
            (None, None) => Figure::unknown(),
        }
    }

    /// Records how much of the input the prompt uses (`used` is an estimate unless the caller
    /// has a count).
    #[must_use]
    pub fn measured(mut self, used: Figure) -> Self {
        self.input.used = used;
        self.input.remaining = match (self.input.limit.value, used.value) {
            (Some(limit), Some(used_tokens)) => Figure {
                value: Some(limit.saturating_sub(used_tokens)),
                source: self.input.limit.source,
                precision: self.input.limit.precision.weakest(used.precision),
                method: used.method,
            },
            _ => Figure::unknown(),
        };
        self
    }

    /// The tokens the prompt may weigh, for the Context Engine to fit against. `None`: no limit is
    /// known, so nothing is left out for size.
    pub fn available_for_context(&self) -> Option<u64> {
        self.input.limit.value
    }

    /// True when the model's own limit is known (stated by the runtime or exact), as opposed to a
    /// budget somebody configured.
    pub fn model_limit_known(&self) -> bool {
        self.limits.input.is_ceiling() || self.limits.total.is_ceiling()
    }

    pub fn over_budget(&self) -> bool {
        matches!(
            (self.input.limit.value, self.input.used.value),
            (Some(limit), Some(used)) if used > limit
        )
    }

    pub fn warnings(&self) -> Vec<ContextWarning> {
        let mut warnings = Vec::new();
        if !self.model_limit_known() {
            warnings.push(ContextWarning::ModelLimitUnknown);
        }
        if self.input.used.precision == Precision::Estimated {
            warnings.push(ContextWarning::TokensEstimated);
        }
        if self
            .resolution
            .iter()
            .any(|r| matches!(r.outcome, ResolutionOutcome::Clamped { .. }))
        {
            warnings.push(ContextWarning::LimitClamped);
        }
        if self.over_budget() {
            warnings.push(ContextWarning::OverBudget);
        }
        warnings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unknown() -> ModelLimits {
        ModelLimits::unknown("rt", "m")
    }

    fn reported(input: u64) -> ModelLimits {
        ModelLimits::reported(
            "rt",
            "m",
            ReportedLimits {
                input: Some(input),
                output: None,
                total: None,
                precision: Precision::Exact,
            },
        )
    }

    #[test]
    fn a_limit_nobody_stated_stays_unknown() {
        let budget = ExecutionBudget::resolve(unknown(), &[]);

        assert_eq!(budget.limits.input, Figure::unknown());
        assert_eq!(budget.available_for_context(), None);
        assert!(!budget.model_limit_known());
        assert!(budget
            .warnings()
            .contains(&ContextWarning::ModelLimitUnknown));
        assert_eq!(
            (budget.runtime_id.as_str(), budget.model_id.as_str()),
            ("rt", "m")
        );
    }

    #[test]
    fn a_reported_limit_keeps_its_source_and_claimed_precision() {
        let budget = ExecutionBudget::resolve(reported(16_000), &[]);

        let limit = budget.input.limit;
        assert_eq!(limit.value, Some(16_000));
        assert_eq!(limit.source, FigureSource::Reported);
        assert_eq!(limit.precision, Precision::Exact);
        assert!(budget.model_limit_known());

        // A runtime can report a limit that is itself an approximation.
        let rough = ExecutionBudget::resolve(
            ModelLimits::reported(
                "rt",
                "m",
                ReportedLimits {
                    input: Some(8_000),
                    output: None,
                    total: None,
                    precision: Precision::Estimated,
                },
            ),
            &[],
        );
        assert_eq!(rough.input.limit.source, FigureSource::Reported);
        assert_eq!(rough.input.limit.precision, Precision::Estimated);
    }

    #[test]
    fn a_configured_limit_over_nothing_known_is_the_limit_but_not_the_models() {
        let budget = ExecutionBudget::resolve(
            unknown(),
            &[LimitOverride::input(OverrideLayer::Settings, 4_000)],
        );

        assert_eq!(budget.available_for_context(), Some(4_000));
        assert_eq!(budget.input.limit.source, FigureSource::Configured);
        assert_eq!(budget.input.limit.precision, Precision::Unknown);
        // The user's budget does not make the model's limit known.
        assert!(!budget.model_limit_known());
        assert_eq!(budget.resolution[0].outcome, ResolutionOutcome::Applied);
    }

    #[test]
    fn a_configured_override_never_raises_a_reported_or_exact_ceiling() {
        let budget = ExecutionBudget::resolve(
            reported(16_000),
            &[LimitOverride::input(OverrideLayer::Settings, 100_000)],
        );

        assert_eq!(budget.available_for_context(), Some(16_000));
        assert_eq!(budget.input.limit.source, FigureSource::Reported);
        assert_eq!(
            budget.resolution[0].outcome,
            ResolutionOutcome::Clamped {
                limit: 16_000,
                limit_source: FigureSource::Reported
            }
        );
        assert!(budget.warnings().contains(&ContextWarning::LimitClamped));
    }

    #[test]
    fn an_exact_ceiling_holds_even_when_it_was_not_reported() {
        let mut base = unknown();
        base.input = Figure {
            value: Some(1_000),
            source: FigureSource::Configured,
            precision: Precision::Exact,
            method: None,
        };

        let budget = ExecutionBudget::resolve(
            base,
            &[LimitOverride::input(OverrideLayer::Workspace, 5_000)],
        );

        assert_eq!(budget.available_for_context(), Some(1_000));
    }

    #[test]
    fn an_override_can_narrow_and_later_layers_can_only_narrow_further() {
        let budget = ExecutionBudget::resolve(
            reported(16_000),
            &[
                LimitOverride::input(OverrideLayer::Settings, 8_000),
                // A looser workspace cannot undo the user's tighter setting.
                LimitOverride::input(OverrideLayer::Workspace, 12_000),
                LimitOverride::input(OverrideLayer::Agent, 6_000),
            ],
        );

        assert_eq!(budget.available_for_context(), Some(6_000));
        assert_eq!(budget.input.limit.source, FigureSource::Configured);
        let outcomes: Vec<_> = budget.resolution.iter().map(|r| r.outcome).collect();
        assert_eq!(outcomes[0], ResolutionOutcome::Narrowed);
        assert!(matches!(
            outcomes[1],
            ResolutionOutcome::Clamped { limit: 8_000, .. }
        ));
        assert_eq!(outcomes[2], ResolutionOutcome::Narrowed);
    }

    #[test]
    fn an_override_equal_to_the_known_limit_keeps_the_original_figure() {
        let budget = ExecutionBudget::resolve(
            reported(16_000),
            &[LimitOverride::input(OverrideLayer::Settings, 16_000)],
        );

        assert_eq!(budget.resolution[0].outcome, ResolutionOutcome::Kept);
        assert_eq!(budget.input.limit.source, FigureSource::Reported);
    }

    #[test]
    fn the_input_limit_can_come_from_the_window_minus_what_is_kept_for_the_answer() {
        let base = ModelLimits::reported(
            "rt",
            "m",
            ReportedLimits {
                input: None,
                output: Some(4_000),
                total: Some(20_000),
                precision: Precision::Exact,
            },
        );

        let budget = ExecutionBudget::resolve(base.clone(), &[]);
        // The model's own maximum output is the natural reserve when no policy sets one.
        assert_eq!(budget.available_for_context(), Some(16_000));
        assert_eq!(budget.input.limit.precision, Precision::Exact);

        let kept = ExecutionBudget::resolve(base, &[])
            .with_reserves(Figure::configured(1_000), Figure::configured(500));
        assert_eq!(kept.available_for_context(), Some(18_500));
        // A configured reserve is not a measurement: the derived figure says so.
        assert_eq!(kept.input.limit.precision, Precision::Unknown);
    }

    #[test]
    fn the_smaller_of_the_direct_and_derived_input_limits_wins() {
        let base = ModelLimits::reported(
            "rt",
            "m",
            ReportedLimits {
                input: Some(30_000),
                output: Some(4_000),
                total: Some(20_000),
                precision: Precision::Exact,
            },
        );

        assert_eq!(
            ExecutionBudget::resolve(base, &[]).available_for_context(),
            Some(16_000)
        );
    }

    #[test]
    fn used_tokens_are_an_estimate_and_the_remainder_inherits_that() {
        let budget = ExecutionBudget::resolve(reported(1_000), &[])
            .measured(Figure::estimated_from_chars(2_400));

        assert_eq!(budget.input.used.value, Some(600));
        assert_eq!(budget.input.used.precision, Precision::Estimated);
        assert_eq!(budget.input.remaining.value, Some(400));
        // Exact limit minus an estimate is an estimate.
        assert_eq!(budget.input.remaining.precision, Precision::Estimated);
        assert!(!budget.over_budget());
        assert!(budget.warnings().contains(&ContextWarning::TokensEstimated));

        let over = ExecutionBudget::resolve(reported(100), &[])
            .measured(Figure::estimated_from_chars(2_400));
        assert!(over.over_budget());
        assert_eq!(over.input.remaining.value, Some(0));
        assert!(over.warnings().contains(&ContextWarning::OverBudget));
    }

    #[test]
    fn with_no_limit_the_remainder_is_unknown_not_made_up() {
        let budget =
            ExecutionBudget::resolve(unknown(), &[]).measured(Figure::estimated_from_chars(400));

        assert_eq!(budget.input.remaining, Figure::unknown());
        assert!(!budget.over_budget());
    }
}
