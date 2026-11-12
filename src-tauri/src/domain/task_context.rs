//! Task-aware context (V0.8): which part of the Project Harness an agent is told for one task.
//!
//! These are plain data types. The Harness stays the only source of knowledge: a
//! [`ContextItem`] is a projection of something the Harness already holds (a finding, the user's
//! constraints, a gap), never new knowledge. Selecting, ranking and explaining live in
//! `application/harness/task_context.rs`.

use serde::{Deserialize, Serialize};

use super::harness::{Confidence, Evidence, FindingCategory, Origin, VerificationStatus};

/// What a piece of knowledge is about. A small, closed set: every finding and every item of the
/// user's own files belongs to at most one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Area {
    Architecture,
    Stack,
    Modules,
    Conventions,
    Business,
    Testing,
    Dependencies,
    EntryPoints,
    Ci,
    Constraints,
    Decisions,
    Infrastructure,
    /// Statements about the Harness itself (a partial analysis) or that fit no other area.
    General,
}

impl Area {
    /// The name used in notes such as "Omitted: 3 testing findings".
    pub fn label(self) -> &'static str {
        match self {
            Self::Architecture => "architecture",
            Self::Stack => "stack",
            Self::Modules => "modules",
            Self::Conventions => "conventions",
            Self::Business => "business",
            Self::Testing => "testing",
            Self::Dependencies => "dependencies",
            Self::EntryPoints => "entry points",
            Self::Ci => "ci",
            Self::Constraints => "constraints",
            Self::Decisions => "decisions",
            Self::Infrastructure => "infrastructure",
            Self::General => "general",
        }
    }

    /// The area a finding of this category belongs to.
    pub fn of(category: FindingCategory) -> Self {
        match category {
            FindingCategory::Architecture => Self::Architecture,
            FindingCategory::Language
            | FindingCategory::Framework
            | FindingCategory::Runtime
            | FindingCategory::PackageManager
            | FindingCategory::Repository => Self::Stack,
            FindingCategory::Module => Self::Modules,
            FindingCategory::Convention | FindingCategory::Tooling => Self::Conventions,
            FindingCategory::Testing => Self::Testing,
            FindingCategory::Dependency => Self::Dependencies,
            FindingCategory::EntryPoint => Self::EntryPoints,
            FindingCategory::Ci | FindingCategory::Build => Self::Ci,
            FindingCategory::Database
            | FindingCategory::Data
            | FindingCategory::Integration
            | FindingCategory::Infrastructure
            | FindingCategory::Environment => Self::Infrastructure,
        }
    }
}

/// How a rendered item is told: which block of the context it belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextBlock {
    /// Verified facts.
    Know,
    /// Candidates, not verified.
    Infer,
    /// The user's word.
    User,
    /// What Atlas does not know (not found, unknown, weak signals, conflicts).
    Unknown,
    /// Knowledge whose evidence lives where the project changed.
    Outdated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Finding,
    Constraint,
    Decision,
    Business,
    NotFound,
    Unknown,
    Conflict,
    /// A statement about the analysis itself.
    Notice,
}

/// One candidate for the context: a projection of a finding, of the user's files or of a gap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextItem {
    /// The finding's id, or a stable name for the rest (`constraints`, `gap:database`).
    pub id: String,
    pub block: ContextBlock,
    pub kind: ItemKind,
    pub area: Area,
    pub category: Option<FindingCategory>,
    /// A name for people (`Angular 18`, `Constraints`).
    pub label: String,
    /// What the agent is told.
    pub content: String,
    pub provenance: Option<Origin>,
    pub verification: Option<VerificationStatus>,
    pub confidence: Option<Confidence>,
    pub evidence: Vec<Evidence>,
    pub source_finding_id: Option<String>,
    /// Deterministic tags derived from the finding's category, text and evidence paths.
    pub tags: Vec<String>,
    /// Lower is kept first by the full-Harness budget: constraints and decisions 1 (never
    /// dropped), the user's other words and stack facts 2, outdated and conflicts 3,
    /// architecture 4, modules 5, conventions 6, testing 7, inferences 8, business 9,
    /// unknowns 10.
    #[serde(skip)]
    pub priority: u8,
    /// Items of the same group are told together on one line (`Frameworks: Angular; Vite`).
    #[serde(skip)]
    pub group: Option<FindingCategory>,
}

/// What the user asked for, as far as Atlas needs to know to choose context. The task is only
/// its text: this is not a task manager.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskContextRequest {
    pub task: String,
    pub workspace_id: String,
    pub agent_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskIntent {
    Add,
    Change,
    Fix,
    Refactor,
    Test,
    Document,
    Investigate,
    Unknown,
}

/// What the deterministic analysis read from the task text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSignals {
    pub keywords: Vec<String>,
    /// Areas the task's tags point at.
    pub areas: Vec<Area>,
    /// Areas that orient almost any task of this intent (architecture for something new).
    pub baseline_areas: Vec<Area>,
    pub tags: Vec<String>,
    pub intent: TaskIntent,
    /// Technologies the task names (`Redis`), whether or not the Harness knows them.
    pub technologies: Vec<String>,
}

impl TaskSignals {
    pub fn is_empty(&self) -> bool {
        self.keywords.is_empty() && self.tags.is_empty() && self.technologies.is_empty()
    }
}

/// Why an item scored what it did. Kept for every candidate, for debugging and for the UI.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionReason {
    pub score: i32,
    pub matched_areas: Vec<Area>,
    pub matched_tags: Vec<String>,
    pub matched_categories: Vec<FindingCategory>,
    /// Evidence paths that share a word with the task.
    pub matched_paths: Vec<String>,
    pub matched_keywords: Vec<String>,
    /// Set when the item is included regardless of its score, and why.
    pub always_included: Option<AlwaysIncluded>,
    /// Layer mismatch and the like: what pulled the score down.
    pub penalties: Vec<String>,
}

/// Knowledge that is never dropped for looking unrelated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlwaysIncluded {
    Constraint,
    Decision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryOutcome {
    Included,
    /// Scored below the relevance threshold.
    NotRelevant,
    /// Relevant, but the budget did not fit it.
    OverBudget,
}

/// One candidate, what became of it and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextEntry {
    pub item: ContextItem,
    pub reason: SelectionReason,
    pub outcome: EntryOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextMode {
    TaskAware,
    /// The task context could not be built; the whole Harness context was used.
    Fallback,
    /// The project has no usable Harness.
    None,
}

/// How many items of one area were left out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OmittedGroup {
    pub area: Area,
    pub count: usize,
}

/// What a task is told about its project, and how it was chosen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskContext {
    pub mode: ContextMode,
    /// Why the full Harness context was used instead (`snake_case`).
    pub fallback_reason: Option<String>,
    pub signals: Option<TaskSignals>,
    /// The text the agent receives.
    pub text: String,
    pub entries: Vec<ContextEntry>,
    pub included_areas: Vec<Area>,
    /// Areas none of whose items were selected.
    pub excluded_areas: Vec<Area>,
    pub selected_chars: usize,
    /// The size of the whole Harness context, for comparison.
    pub full_harness_chars: usize,
    pub budget_chars: usize,
    /// Left out because of the budget (not for being unrelated).
    pub truncated: bool,
    pub omitted: Vec<OmittedGroup>,
}

/// What an execution keeps of how its context was chosen. Counts only: the text is the prompt
/// the execution already stores.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextRecord {
    pub mode: ContextMode,
    pub total_harness_characters: usize,
    pub selected_context_characters: usize,
    pub selected_items: usize,
    pub omitted_items: usize,
    /// Items included that the project has changed under since they were written (the Outdated
    /// block). Zero for executions recorded before this was counted.
    #[serde(default)]
    pub stale_items: usize,
    /// Included because they matched the task (a score above zero), not because they are always
    /// included (constraints, decisions, the user's own word). Zero for older executions.
    #[serde(default)]
    pub matched_items: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
}

impl ContextRecord {
    /// How much of what was included was chosen for this task, as a share (0 to 1): the only
    /// relevance Atlas measures of a Task Context. `None` when nothing was included or the
    /// context was not chosen for the task (fallback, none).
    pub fn relevance(&self) -> Option<f32> {
        if self.mode != ContextMode::TaskAware || self.selected_items == 0 {
            return None;
        }
        #[allow(clippy::cast_precision_loss)] // counts of a few dozen items
        Some(self.matched_items.min(self.selected_items) as f32 / self.selected_items as f32)
    }
}

impl TaskContext {
    pub fn record(&self) -> ContextRecord {
        ContextRecord {
            mode: self.mode,
            total_harness_characters: self.full_harness_chars,
            selected_context_characters: self.selected_chars,
            selected_items: self
                .entries
                .iter()
                .filter(|e| e.outcome == EntryOutcome::Included)
                .count(),
            omitted_items: self
                .entries
                .iter()
                .filter(|e| e.outcome != EntryOutcome::Included)
                .count(),
            stale_items: self
                .entries
                .iter()
                .filter(|e| {
                    e.outcome == EntryOutcome::Included && e.item.block == ContextBlock::Outdated
                })
                .count(),
            matched_items: self
                .entries
                .iter()
                .filter(|e| {
                    e.outcome == EntryOutcome::Included
                        && e.reason.score > 0
                        && e.reason.always_included.is_none()
                })
                .count(),
            fallback_reason: self.fallback_reason.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewStatus {
    Ready,
    /// The project has no Harness: the agent runs without project context.
    Missing,
    /// `.atlas/` is there but unusable: it is ignored.
    Invalid,
}

/// What an agent would be told for a task, shown before it runs. Inspection only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskContextPreview {
    pub status: PreviewStatus,
    pub context: Option<TaskContext>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(mode: ContextMode, selected: usize, matched: usize) -> ContextRecord {
        ContextRecord {
            mode,
            total_harness_characters: 100,
            selected_context_characters: 50,
            selected_items: selected,
            omitted_items: 0,
            stale_items: 0,
            matched_items: matched,
            fallback_reason: None,
        }
    }

    /// ADR 0021, C4: the only relevance Atlas measures of a Task Context is the share of what it
    /// included because it matched the task. Nothing else is scored.
    #[test]
    fn a_task_contexts_relevance_is_the_share_it_chose_for_the_task() {
        assert_eq!(record(ContextMode::TaskAware, 4, 3).relevance(), Some(0.75));
        assert_eq!(record(ContextMode::TaskAware, 4, 0).relevance(), Some(0.0));
        // Nothing included, or not chosen for the task: unknown, not zero.
        assert_eq!(record(ContextMode::TaskAware, 0, 0).relevance(), None);
        assert_eq!(record(ContextMode::Fallback, 4, 4).relevance(), None);
        // An older record, without the count, reads as nothing matched.
        let old: ContextRecord = serde_json::from_str(
            r#"{"mode":"task_aware","totalHarnessCharacters":1,"selectedContextCharacters":1,"selectedItems":2,"omittedItems":0}"#,
        )
        .unwrap();
        assert_eq!(old.matched_items, 0);
    }
}
