//! What Atlas can observe about what it sends to a model and what that costs (Optimization
//! Layer, phase 0). Observability only: nothing here changes a prompt, a context selection, a
//! runtime or a tool. Numbers the core cannot measure are absent (`None`), never invented.
//!
//! Tokens the runtime reported stay in [`UsageMetrics`](super::usage::UsageMetrics); this module
//! only says how many tokens *Atlas expects* a prompt to be, and how sure that is.

use serde::{Deserialize, Serialize};

use super::usage::{UsageMetrics, UsageSource};

/// How a token figure was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenSource {
    /// Counted by the model's own tokenizer or reported by the runtime.
    Exact,
    /// Atlas's own approximation ([`estimate_tokens`]). Never to be shown as a real count.
    Estimated,
    /// Nothing is known.
    Unavailable,
}

/// How many bytes and characters a piece of text has. Characters are what the token estimate
/// is based on, so a text in Portuguese is not over-counted for its accents.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextSize {
    pub bytes: usize,
    pub chars: usize,
}

impl TextSize {
    pub fn of(text: &str) -> Self {
        Self {
            bytes: text.len(),
            chars: text.chars().count(),
        }
    }

    #[must_use]
    pub fn saturating_sub(self, other: Self) -> Self {
        Self {
            bytes: self.bytes.saturating_sub(other.bytes),
            chars: self.chars.saturating_sub(other.chars),
        }
    }

    pub fn estimated_tokens(self) -> u64 {
        estimate_tokens(self.chars)
    }
}

impl std::ops::Add for TextSize {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            bytes: self.bytes + other.bytes,
            chars: self.chars + other.chars,
        }
    }
}

/// Characters per token assumed by [`estimate_tokens`]. A rough average for English prose and
/// code; the real ratio depends on the model and the text, which is why the figure is always
/// reported as [`TokenSource::Estimated`].
pub const CHARS_PER_TOKEN: usize = 4;

/// The only token estimate Atlas makes: `ceil(chars / 4)`.
pub fn estimate_tokens(chars: usize) -> u64 {
    u64::try_from(chars.div_ceil(CHARS_PER_TOKEN)).unwrap_or(u64::MAX)
}

/// The parts a prompt is made of, in the order they appear.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionKind {
    /// The personality's own system instructions.
    Personality,
    AtlasRules,
    LiveNarration,
    PlanRule,
    /// The whole Harness context, used when no Task Context could be selected.
    Harness,
    /// The part of the Harness chosen for the task.
    TaskContext,
    /// The instructions of the skills selected for the task, and the notice that frames them.
    Skills,
    /// Project name, path and detected technologies.
    ProjectContext,
    /// The agent's standing instructions.
    AgentInstructions,
    /// A workflow step's `WORKFLOW CONTEXT` block (state, artifacts, decisions, reports).
    BriefWorkflowContext,
    /// The `WORKFLOW HANDOFF` block handed over by the previous step.
    BriefHandoff,
    /// What a step is told about how to answer: human input, result and interaction protocols.
    BriefProtocols,
    /// What is left of the instruction: the task itself (for a step, the node's instructions and
    /// the overall task).
    Task,
    /// Section titles and separators `Prompt::combined` adds around the parts.
    Framing,
}

/// One part of a prompt, measured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptSection {
    pub section: SectionKind,
    pub bytes: usize,
    pub estimated_tokens: u64,
    pub token_source: TokenSource,
}

impl PromptSection {
    pub fn estimated(section: SectionKind, size: TextSize) -> Self {
        Self {
            section,
            bytes: size.bytes,
            estimated_tokens: size.estimated_tokens(),
            token_source: TokenSource::Estimated,
        }
    }
}

/// The prompt as sent, split into its parts. `sections` add up to `total_bytes`: whatever the
/// parts do not explain is [`SectionKind::Framing`], so nothing hides in a gap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptBreakdown {
    pub total_bytes: usize,
    pub estimated_tokens: u64,
    pub token_source: TokenSource,
    pub sections: Vec<PromptSection>,
}

impl PromptBreakdown {
    /// Splits `whole` (the prompt exactly as sent) into `parts`; the rest of it is framing.
    /// Parts that are empty are not listed.
    pub fn new(whole: TextSize, parts: impl IntoIterator<Item = (SectionKind, TextSize)>) -> Self {
        let mut sections = Vec::new();
        let mut explained = TextSize::default();
        for (kind, size) in parts {
            if size.bytes == 0 {
                continue;
            }
            explained = explained + size;
            sections.push(PromptSection::estimated(kind, size));
        }
        let framing = whole.saturating_sub(explained);
        if framing.bytes > 0 {
            sections.push(PromptSection::estimated(SectionKind::Framing, framing));
        }
        Self {
            total_bytes: whole.bytes,
            estimated_tokens: whole.estimated_tokens(),
            token_source: TokenSource::Estimated,
            sections,
        }
    }

    #[cfg(test)]
    pub fn section(&self, kind: SectionKind) -> Option<&PromptSection> {
        self.sections.iter().find(|s| s.section == kind)
    }

    #[cfg(test)]
    pub fn section_bytes(&self, kind: SectionKind) -> usize {
        self.section(kind).map_or(0, |s| s.bytes)
    }
}

/// How a step's brief divides, measured where the brief is written (so no text is parsed back).
/// What it does not list is the task itself.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BriefLayout {
    pub workflow_context: TextSize,
    pub handoff: TextSize,
    pub protocols: TextSize,
}

/// The parts of a workflow step's brief as written, so what the Context Engine may rework can be
/// told from what it must leave alone. `layout()` is how big each is.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BriefParts {
    pub workflow_context: String,
    pub handoff: String,
    pub protocols: String,
}

impl BriefParts {
    pub fn layout(&self) -> BriefLayout {
        BriefLayout {
            workflow_context: TextSize::of(&self.workflow_context),
            handoff: TextSize::of(&self.handoff),
            protocols: TextSize::of(&self.protocols),
        }
    }
}

/// How the Harness part of the prompt was chosen, in numbers. `None`: the project has no Harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextMetrics {
    pub selected_items: u32,
    pub omitted_items: u32,
    pub selected_characters: u64,
    pub total_harness_characters: u64,
}

/// Where the time of an execution went, in milliseconds. Each figure is `None` when Atlas has no
/// honest way to observe it (a runtime that never reports the stage, no Harness to build…).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_field_names)]
pub struct LatencyMetrics {
    /// Reading the Harness and selecting the Task Context.
    pub context_build_ms: Option<f64>,
    /// Assembling the prompt text.
    pub prompt_build_ms: Option<f64>,
    /// From launching the runtime to its reporting it is waiting for the model.
    pub runtime_startup_ms: Option<f64>,
    /// From that point to the runtime's answer.
    pub runtime_execution_ms: Option<f64>,
    /// The whole call to the runtime.
    pub runtime_ms: Option<f64>,
    /// Everything `run_step` did, before and after the runtime included.
    pub total_ms: Option<f64>,
    /// What measuring itself cost, so the instrumentation can be held to account.
    pub instrumentation_ms: Option<f64>,
}

/// What the runtime showed of its tools. Atlas does not own the tool loop of a CLI runtime, so
/// this is only what the runtime chose to stream.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolMetrics {
    /// Tool calls the runtime reported. `None` when it reported none: Atlas cannot tell "no tool
    /// was used" from "this runtime does not stream tool events".
    pub calls: Option<u32>,
    /// Never observable today: the output of a tool goes from the CLI's tool to the model without
    /// passing through Atlas.
    pub total_output_bytes: Option<u64>,
    /// The tools the model could call in this run, as the runtime itself listed them when it
    /// started (names, MCP tools included). `None` when the runtime does not say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exposed: Option<Vec<String>>,
    /// The tools the model did call, from the runtime's tool events, most used first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub used: Vec<ToolUse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolUse {
    pub name: String,
    pub calls: u32,
}

/// What else the runtime loaded around the model on its own, as it reported at start: the user's
/// own MCP servers, skills and plugins that Atlas did not choose. Each costs context and widens
/// what an agent can reach.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeExtensions {
    pub mcp_servers: Vec<String>,
    pub skills: u32,
    pub slash_commands: u32,
    pub plugins: u32,
}

/// What a step was handed by the steps before it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HandoffMetrics {
    /// `None` outside a workflow step.
    pub bytes: Option<u64>,
}

/// What optimization did to the prompt. Phase 0 changes nothing, so the work counters are zero
/// (and true); caches do not exist yet, so their counters are unavailable.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizationCounters {
    pub cache_hits: Option<u32>,
    pub cache_misses: Option<u32>,
    pub deduplicated_items: u32,
    pub compressed_items: u32,
    /// Harness items left out of the Task Context by its own budget and relevance rules (not by
    /// the Optimization Layer). `None` without a Harness.
    pub dropped_items: Option<u32>,
}

/// Why the Context Engine changed (or declined to change) a piece of context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionKind {
    /// A line that is the same, character for character, as one already in the prompt.
    DuplicateExact,
    /// The same once case, punctuation and spacing are ignored.
    DuplicateNormalized,
    /// Every word of the line is already in one earlier line (which says at least as much).
    DuplicateOverlap,
    /// Whitespace only (blank runs, trailing spaces); no word changed.
    Compressed,
    /// A whole item left out to fit the budget; the prompt names it.
    Omitted,
}

/// One thing the Context Engine did, with where the kept copy is, so nothing is lost silently.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextDecision {
    pub kind: DecisionKind,
    /// Where the removed text was.
    pub source: SectionKind,
    /// Where the same information stays, for duplicates.
    pub kept_in: Option<SectionKind>,
    pub bytes_saved: usize,
    /// The start of what was removed, for the audit trail (never the whole of a large text).
    pub preview: String,
}

/// The prompt's size budget was not met by what may be shortened. Nothing was cut to force it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_field_names)]
pub struct BudgetOverrun {
    pub budget_tokens: u64,
    pub estimated_tokens: u64,
    /// What cannot be removed (required context) on its own, estimated.
    pub required_tokens: u64,
}

/// What the Context Engine did to one prompt. Sizes are estimates (`chars / 4`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextEngineMetrics {
    pub raw_bytes: usize,
    pub final_bytes: usize,
    pub raw_estimated_tokens: u64,
    pub final_estimated_tokens: u64,
    pub token_source: TokenSource,
    pub deduplicated_lines: u32,
    pub compressed_items: u32,
    pub omitted_items: u32,
    /// At most the first 40.
    pub decisions: Vec<ContextDecision>,
    pub over_budget: Option<BudgetOverrun>,
    /// Why the engine left the prompt as it was, when it did (a brief it could not take apart).
    pub skipped: Option<String>,
}

impl ContextEngineMetrics {
    pub fn saved_bytes(&self) -> usize {
        self.raw_bytes.saturating_sub(self.final_bytes)
    }
}

/// What the skills layer did for one execution. Token figures are estimates (`chars / 4`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillMetrics {
    /// Skill folders found (project and user).
    pub discovered: u32,
    /// Of those, the ones that can be used (valid, not shadowed).
    pub usable: u32,
    /// Quality issues found across all of them.
    pub issues: u32,
    /// What the whole catalogue's names and descriptions would cost if they were all sent
    /// (level 1). They are not: it is what discovery avoided.
    pub level1_tokens: u64,
    /// Skills worth loading for the task.
    pub candidates: u32,
    pub activated: Vec<String>,
    /// The instructions of the activated skills (level 2).
    pub level2_tokens: u64,
    /// Resources the activated skills have, and how many the task called for.
    pub resources_available: u32,
    pub resources_loaded: u32,
    pub level3_tokens: u64,
    pub cache_hits: u32,
    pub cache_misses: u32,
    pub select_ms: Option<f64>,
    pub token_source: TokenSource,
}

/// Everything observed about one execution. The tokens the runtime reported are not here: they
/// are `Execution::usage`, and [`PromptVersusRuntime`] puts the two side by side.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizationMetrics {
    pub prompt: PromptBreakdown,
    pub context: Option<ContextMetrics>,
    pub latency: LatencyMetrics,
    pub tools: ToolMetrics,
    pub handoff: HandoffMetrics,
    pub optimization: OptimizationCounters,
    /// What the Context Engine did, when it ran. `None` when it is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_engine: Option<ContextEngineMetrics>,
    /// What the skills layer did, when it ran. `None` when it is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<SkillMetrics>,
    /// What the runtime loaded on its own (MCP servers, skills, plugins), when it says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<RuntimeExtensions>,
}

/// What Atlas assembled against what the runtime says it received.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptVersusRuntime {
    pub atlas_estimated_prompt_tokens: u64,
    pub runtime_input_tokens: Option<u64>,
    pub runtime_output_tokens: Option<u64>,
    pub runtime_cached_input_tokens: Option<u64>,
    /// `Exact` only when the runtime reported the input tokens itself.
    pub runtime_token_source: TokenSource,
}

impl PromptVersusRuntime {
    pub fn of(metrics: &OptimizationMetrics, usage: Option<&UsageMetrics>) -> Self {
        let reported = usage.filter(|u| u.source == UsageSource::RuntimeReported);
        let input = reported.and_then(|u| u.input_tokens);
        Self {
            atlas_estimated_prompt_tokens: metrics.prompt.estimated_tokens,
            runtime_input_tokens: input,
            runtime_output_tokens: reported.and_then(|u| u.output_tokens),
            runtime_cached_input_tokens: reported.and_then(|u| u.cached_input_tokens),
            runtime_token_source: if input.is_some() {
                TokenSource::Exact
            } else {
                TokenSource::Unavailable
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_estimate_is_chars_over_four_rounded_up() {
        assert_eq!(estimate_tokens(0), 0);
        assert_eq!(estimate_tokens(1), 1);
        assert_eq!(estimate_tokens(4), 1);
        assert_eq!(estimate_tokens(5), 2);
        // Accents are characters, not their two bytes.
        let size = TextSize::of("ação");
        assert_eq!((size.bytes, size.chars), (6, 4));
        assert_eq!(size.estimated_tokens(), 1);
    }

    #[test]
    fn sections_always_add_up_to_the_whole_prompt() {
        let breakdown = PromptBreakdown::new(
            TextSize {
                bytes: 100,
                chars: 100,
            },
            [
                (
                    SectionKind::Personality,
                    TextSize {
                        bytes: 40,
                        chars: 40,
                    },
                ),
                (SectionKind::Harness, TextSize::default()),
                (
                    SectionKind::Task,
                    TextSize {
                        bytes: 50,
                        chars: 50,
                    },
                ),
            ],
        );

        assert_eq!(breakdown.total_bytes, 100);
        assert_eq!(
            breakdown.sections.iter().map(|s| s.bytes).sum::<usize>(),
            100
        );
        assert_eq!(breakdown.section_bytes(SectionKind::Framing), 10);
        // An empty part is not listed at all.
        assert!(breakdown.section(SectionKind::Harness).is_none());
        assert!(breakdown
            .sections
            .iter()
            .all(|s| s.token_source == TokenSource::Estimated));
    }

    #[test]
    fn what_was_saved_before_the_layer_existed_still_loads() {
        use crate::domain::execution::StoredExecution;

        // Usage without the cache figure.
        let usage: UsageMetrics = serde_json::from_str(
            r#"{"inputTokens":10,"outputTokens":2,"totalTokens":12,"cost":null,"currency":null,"source":"runtime_reported"}"#,
        )
        .unwrap();
        assert_eq!(usage.cached_input_tokens, None);

        // A stored execution without `optimization`.
        let stored: StoredExecution = serde_json::from_str(
            r#"{"id":"e1","workspaceId":"w","agentId":"a","status":"completed","task":"t",
                "startedAt":1,"completedAt":2,"runtimeId":"r","modelId":"m","failure":null,
                "metadata":{},"usage":null,"events":[]}"#,
        )
        .unwrap();
        assert!(stored.optimization.is_none());
        // And it is not written out when absent, so old readers see the old shape.
        assert!(!serde_json::to_string(&stored)
            .unwrap()
            .contains("optimization"));

        // Settings without the switch keep metrics on.
        let settings: crate::application::config::AppSettings =
            serde_json::from_str(r#"{"language":"pt-BR"}"#).unwrap();
        assert!(settings.optimization.metrics_enabled);
        let partial: crate::application::config::AppSettings =
            serde_json::from_str(r#"{"language":"pt-BR","optimization":{}}"#).unwrap();
        assert!(partial.optimization.metrics_enabled);
    }

    fn metrics_with(estimated: u64) -> OptimizationMetrics {
        OptimizationMetrics {
            prompt: PromptBreakdown {
                total_bytes: 0,
                estimated_tokens: estimated,
                token_source: TokenSource::Estimated,
                sections: vec![],
            },
            context: None,
            latency: LatencyMetrics::default(),
            tools: ToolMetrics::default(),
            handoff: HandoffMetrics::default(),
            optimization: OptimizationCounters::default(),
            context_engine: None,
            skills: None,
            extensions: None,
        }
    }

    #[test]
    fn runtime_tokens_are_exact_only_when_the_runtime_reported_them() {
        let metrics = metrics_with(900);
        let reported = UsageMetrics {
            input_tokens: Some(1_200),
            output_tokens: Some(80),
            total_tokens: Some(1_280),
            cached_input_tokens: Some(700),
            cost: None,
            currency: None,
            source: UsageSource::RuntimeReported,
        };

        let compared = PromptVersusRuntime::of(&metrics, Some(&reported));
        assert_eq!(compared.atlas_estimated_prompt_tokens, 900);
        assert_eq!(compared.runtime_input_tokens, Some(1_200));
        assert_eq!(compared.runtime_cached_input_tokens, Some(700));
        assert_eq!(compared.runtime_token_source, TokenSource::Exact);

        // Atlas's own sums are not what a runtime counted.
        let calculated = UsageMetrics {
            source: UsageSource::AtlasCalculated,
            ..reported
        };
        for usage in [None, Some(&calculated)] {
            let compared = PromptVersusRuntime::of(&metrics, usage);
            assert_eq!(compared.runtime_input_tokens, None);
            assert_eq!(compared.runtime_token_source, TokenSource::Unavailable);
        }
    }
}
