use serde::{Deserialize, Serialize};

use super::authority::ContextAuthority;
use super::figure::Figure;
use super::surface::RuntimeSurface;
use super::ContextWarning;
use crate::domain::mcp::McpRecord;
use crate::domain::optimization::PromptSection;
use crate::domain::rules::{RuleOrigin, RuleScope, RuleStrength};
use crate::domain::runtime::SystemPromptChannel;

/// The identifiers the architecture already has. The Manifest adds none of its own: a workflow
/// step and attempt are found through the execution (`Execution::id`, `task_id`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestIds {
    pub execution: String,
    pub workspace: String,
    pub task: String,
    pub agent: String,
    pub runtime: String,
    pub model: String,
}

/// What was handed (or was about to be handed) to the runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryRecord {
    /// False when the guardrails stopped or paused the step: the payload was prepared and hashed
    /// but never handed to a process.
    pub delivered: bool,
    /// `sha256:<hex>` of the exact payload the runtime adapter delivers, taken from the same
    /// `delivery()` function that delivers it.
    pub prompt_hash: String,
    pub bytes: usize,
    pub chars: usize,
    pub estimated_tokens: Figure,
    /// How Atlas's system instructions travelled: on the runtime's own channel, or (every runtime
    /// today) inside the prompt body. `prompt_hash` covers both parts when they are separate.
    #[serde(default)]
    pub system_channel: SystemPromptChannel,
    /// Bytes sent on the system channel (0 when there is none).
    #[serde(default)]
    pub system_bytes: usize,
}

/// What became of a rule that applied to the execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleStatus {
    /// In the prompt.
    Applied,
    /// Applied, but the Context Engine left it out for the budget (never a mandatory rule).
    OmittedForBudget,
    Disabled,
    /// The same text is applied under another rule.
    Duplicate,
    /// Another rule on the same topic governs.
    Overridden,
}

/// One rule as the manifest records it: enough to explain why it is (or is not) in the prompt,
/// without repeating its text (the prompt is the record of that, and the hash vouches for it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestRule {
    pub reference: String,
    pub title: String,
    pub scope: RuleScope,
    /// What it counted as (its claimed strength, unless its origin cannot bind).
    pub strength: RuleStrength,
    pub authority: ContextAuthority,
    pub origin: RuleOrigin,
    pub source: String,
    pub status: RuleStatus,
    /// The rule that governs it (`Overridden`) or that it repeats (`Duplicate`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
    /// Involved in a conflict that settled against it, or that settled in its favour.
    pub in_conflict: bool,
    /// Its claimed strength was lowered because its origin cannot bind.
    pub downgraded: bool,
}

/// What Atlas actually prepared and delivered to the runtime for one execution, as opposed to the
/// [`ContextPlan`](super::ContextPlan) it meant to. The budget it was prepared under is
/// `OptimizationMetrics::budget`; it is not copied here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextManifest {
    pub execution_id: String,
    pub workspace_id: String,
    pub task_id: String,
    pub agent_id: String,
    pub runtime_id: String,
    pub model_id: String,
    /// Milliseconds since the Unix epoch.
    pub created_at: u64,
    /// The plan this was made from (`ContextPlan::fingerprint`).
    pub plan_fingerprint: String,
    /// The prompt differs from the plan: something changed it afterwards (a secret was taken out).
    pub diverged_from_plan: bool,
    /// The sections of the prompt as delivered.
    pub sections: Vec<PromptSection>,
    pub delivery: DeliveryRecord,
    /// The rules that applied to this execution and what became of each.
    #[serde(default)]
    pub rules: Vec<ManifestRule>,
    /// The MCP connections of the workspace as this step saw them: authorized, exposed, reported,
    /// used. Empty when the workspace has none.
    #[serde(default)]
    pub mcp: McpRecord,
    /// What else shapes the execution. `Declared` entries are filled before the run; `Reported`
    /// ones once the runtime has said what it loaded.
    pub surface: RuntimeSurface,
    pub warnings: Vec<ContextWarning>,
}

impl ContextManifest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ids: ManifestIds,
        created_at: u64,
        plan_fingerprint: String,
        diverged_from_plan: bool,
        sections: Vec<PromptSection>,
        delivery: DeliveryRecord,
        rules: Vec<ManifestRule>,
        mcp: McpRecord,
        surface: RuntimeSurface,
    ) -> Self {
        let mut manifest = Self {
            execution_id: ids.execution,
            workspace_id: ids.workspace,
            task_id: ids.task,
            agent_id: ids.agent,
            runtime_id: ids.runtime,
            model_id: ids.model,
            created_at,
            plan_fingerprint,
            diverged_from_plan,
            sections,
            delivery,
            rules,
            mcp,
            surface,
            warnings: Vec::new(),
        };
        manifest.refresh_warnings(&[]);
        manifest
    }

    /// Recomputes the manifest's own warnings (and keeps `extra`, the budget's). Called again
    /// when the surface learns what the runtime reported.
    pub fn refresh_warnings(&mut self, extra: &[ContextWarning]) {
        let mut warnings: Vec<ContextWarning> = extra.to_vec();
        if self.delivery.estimated_tokens.precision != super::figure::Precision::Exact
            && !warnings.contains(&ContextWarning::TokensEstimated)
        {
            warnings.push(ContextWarning::TokensEstimated);
        }
        if !self.delivery.delivered {
            warnings.push(ContextWarning::NotDelivered);
        }
        if self.diverged_from_plan {
            warnings.push(ContextWarning::DivergedFromPlan);
        }
        if self.surface.partly_unobserved() {
            warnings.push(ContextWarning::SurfacePartlyUnobserved);
        }
        self.warnings = warnings;
    }
}
