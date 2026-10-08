use crate::application::harness::fingerprint::digest_text;
use crate::domain::context::ContextAuthority;
use crate::domain::optimization::{SectionKind, TextSize};

/// How much a piece of context matters to the step, from the order the Optimization Layer
/// follows: the request and the rules first, optional history last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[allow(dead_code)] // `Optional` and `Normal` are for what later phases add (history, hints).
pub enum Priority {
    /// May be omitted for a budget, first.
    Optional,
    Normal,
    /// Relevant context (Harness, handoff): may be shortened of duplicates, not omitted.
    High,
    /// Never edited, dropped or truncated.
    Required,
}

/// One piece of context a prompt would carry, with what the engine needs to judge it.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextItem {
    pub id: String,
    /// Where it sits in the prompt (and what the audit trail calls it).
    pub source: SectionKind,
    pub content: String,
    pub priority: Priority,
    /// How relevant it is to the task, when something knows (the Task Context's own selection
    /// reasons are not carried per item yet).
    pub relevance: Option<f32>,
    /// Said to be outdated by whatever produced it. Left out before fresh items of the same
    /// priority when a budget forces a choice.
    pub stale: bool,
    /// Where it came from (`harness`, `workflow.handoff`, `agent`…).
    pub provenance: String,
    /// What the text may do (`ContextAuthority`): by default what its section allows; a rule's
    /// depends on its strength and origin.
    pub authority: ContextAuthority,
    /// Stable digest of the text, to tell when it changed or repeats.
    pub fingerprint: String,
    pub size: TextSize,
}

impl ContextItem {
    pub fn new(
        id: &str,
        source: SectionKind,
        priority: Priority,
        provenance: &str,
        content: &str,
    ) -> Self {
        Self {
            id: id.to_owned(),
            source,
            content: content.to_owned(),
            priority,
            relevance: None,
            stale: false,
            provenance: provenance.to_owned(),
            authority: ContextAuthority::of_section(source),
            fingerprint: digest_text(content),
            size: TextSize::of(content),
        }
    }

    #[must_use]
    pub fn with_authority(mut self, authority: ContextAuthority) -> Self {
        self.authority = authority;
        self
    }

    #[cfg(test)]
    #[must_use]
    pub fn stale(mut self) -> Self {
        self.stale = true;
        self
    }

    /// The engine may rework this item.
    pub fn editable(&self) -> bool {
        self.priority != Priority::Required
    }

    pub fn estimated_tokens(&self) -> u64 {
        self.size.estimated_tokens()
    }
}
