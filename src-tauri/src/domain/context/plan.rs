use serde::{Deserialize, Serialize};

use super::figure::Figure;
use crate::domain::optimization::{PromptBreakdown, SectionKind};

/// One part of the planned prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedSection {
    pub section: SectionKind,
    pub bytes: usize,
    pub tokens: Figure,
}

/// What Atlas intended to assemble: the sections the `PromptBuilder` produced (after the Context
/// Engine, before the guardrails). It does not compose anything itself; the builder stays the only
/// assembly. A section that does not exist yet (rules, memory, tool definitions) is a new
/// [`SectionKind`], and nothing here changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextPlan {
    pub sections: Vec<PlannedSection>,
    pub total_bytes: usize,
    pub total_tokens: Figure,
    /// A digest of the sections' kinds and sizes, so the Manifest can say whether it is this plan.
    pub fingerprint: String,
    /// Items the Context Engine left out to fit the budget (0 with the engine off).
    pub engine_omitted_items: u32,
}

impl ContextPlan {
    pub fn from_breakdown(
        breakdown: &PromptBreakdown,
        fingerprint: String,
        engine_omitted_items: u32,
    ) -> Self {
        Self {
            sections: breakdown
                .sections
                .iter()
                .map(|s| PlannedSection {
                    section: s.section,
                    bytes: s.bytes,
                    // Characters, not bytes: the breakdown already counted them.
                    tokens: Figure::estimated(s.estimated_tokens),
                })
                .collect(),
            total_bytes: breakdown.total_bytes,
            total_tokens: Figure::estimated(breakdown.estimated_tokens),
            fingerprint,
            engine_omitted_items,
        }
    }
}
