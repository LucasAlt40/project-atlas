//! Task-aware context selection: given what the Harness holds and a task, choose the knowledge
//! that task needs, and be able to say why.
//!
//! ```text
//! task text -> TaskAnalyzer -> TaskSignals
//! Harness items (candidates) + TaskSignals -> score each (and keep the reasons)
//!   -> filter by relevance -> order by priority -> fit the budget -> Selection
//! ```
//!
//! Everything here is deterministic and pure: no model, no embeddings, no disk, no commands, no
//! network. It creates no knowledge. The only statement it can add is the honest one that a
//! technology the task names was not identified in what the Harness holds. Tests read this very
//! file to keep it that way.
//!
//! ## Ranking
//!
//! An item's score is the sum of the weights in [`Weights`]: exact tag matches, the area the
//! task points at (or that orients any task of its intent), the category, a path of its
//! evidence that shares a word with the task, a word of the task in its text, a bonus when the
//! user vouched for it and a small one when it is verified; a layer mismatch (an Angular note
//! for a database migration) subtracts. An item is relevant from [`Weights::THRESHOLD`] up.
//!
//! Constraints and decisions are never judged: they are always included, constraints that touch
//! security first. The user's word and verified facts then come before inference, and
//! "unknown" and "outdated" last.

use std::collections::BTreeSet;

use super::taxonomy::{
    is_layer, same_word, tag_areas, tag_categories, tags_of_technology, technologies_of_words,
    technology_words, words,
};
use crate::domain::harness::{FindingCategory, Origin, VerificationStatus};
use crate::domain::task_context::{
    AlwaysIncluded, Area, ContextBlock, ContextEntry, ContextItem, EntryOutcome, ItemKind,
    OmittedGroup, SelectionReason, TaskIntent, TaskSignals,
};

/// The ranking weights, in one place.
pub struct Weights;

impl Weights {
    /// An exact tag of the task on the item. At most [`Self::MAX_TAG_MATCHES`] count.
    pub const TAG_MATCH: i32 = 50;
    pub const MAX_TAG_MATCHES: usize = 2;
    /// The item's area is one the task's tags ask for, and it matches something specific.
    pub const FOCUS_AREA: i32 = 30;
    /// The item's area orients any task of this intent (architecture for something new).
    pub const BASELINE_AREA: i32 = 25;
    /// The stack tells which language and framework a task is in, whatever the task.
    pub const STACK_ORIENTATION: i32 = 22;
    /// The item's category is one the task's tags ask for.
    pub const CATEGORY_MATCH: i32 = 20;
    /// A path of the item's evidence shares a word with the task.
    pub const PATH_MATCH: i32 = 20;
    /// A word of the task appears in the item's text.
    pub const KEYWORD_MATCH: i32 = 25;
    /// The user confirmed, corrected or wrote it (counted once the item matches anything).
    pub const USER_KNOWLEDGE: i32 = 40;
    /// Read from a repository file and still current (counted once the item matches anything).
    pub const VERIFIED: i32 = 5;
    /// The item belongs to a layer (frontend, backend, database) the task is not in.
    pub const LAYER_MISMATCH: i32 = -40;
    /// Recorded for constraints and decisions; they are included whatever they score.
    pub const CONSTRAINT_OR_DECISION: i32 = 100;
    /// From here an item is relevant.
    pub const THRESHOLD: i32 = 20;
}

/// Characters kept free for the notes that say what was left out.
const NOTES_RESERVE: usize = 450;
/// What the heading of a group (`Frameworks: `) costs the first time one of its items is kept.
const GROUP_HEADER_ESTIMATE: usize = 24;

const STOPWORDS: &[&str] = &[
    // English
    "the",
    "and",
    "for",
    "with",
    "from",
    "that",
    "this",
    "into",
    "using",
    "use",
    "used",
    "existing",
    "new",
    "all",
    "any",
    "please",
    "should",
    "can",
    "our",
    "your",
    "when",
    "then",
    "not",
    "are",
    "was",
    "were",
    "its",
    "has",
    "have",
    "how",
    "what",
    "why",
    "make",
    "need",
    "needs",
    "also",
    "about",
    "like",
    "same",
    "pattern",
    "standard",
    "current",
    "some",
    // Portuguese
    "para",
    "por",
    "com",
    "uma",
    "uns",
    "umas",
    "dos",
    "das",
    "nos",
    "nas",
    "que",
    "ao",
    "aos",
    "padrao",
    "existente",
    "usando",
    "usar",
    "novo",
    "nova",
    "favor",
    "como",
    "quando",
    "isso",
    "esse",
    "essa",
    "este",
    "esta",
    "seu",
    "sua",
    "mais",
    "sem",
    "sobre",
    "ser",
    "tem",
    "preciso",
    "quero",
];

/// Words that say what the task does, in order of the intents they mean. They are not keywords.
const INTENT_VERBS: &[(TaskIntent, &[&str])] = &[
    (
        TaskIntent::Fix,
        &[
            "fix",
            "bug",
            "repair",
            "debug",
            "resolve",
            "corrigir",
            "conserta*",
            "arrum*",
            "correc*",
            "corrig*",
        ],
    ),
    (
        TaskIntent::Refactor,
        &[
            "refactor*",
            "refator*",
            "rename",
            "renomear",
            "cleanup",
            "simplif*",
        ],
    ),
    (
        TaskIntent::Add,
        &[
            "add",
            "create",
            "implement*",
            "introduce",
            "build",
            "adicionar",
            "adicion*",
            "criar",
            "cria",
            "implementar",
            "novo",
            "nova",
        ],
    ),
    (
        TaskIntent::Change,
        &[
            "change", "update", "modify", "edit", "adjust", "alter*", "mudar", "muda", "atualiz*",
            "ajust*", "trocar",
        ],
    ),
    (
        TaskIntent::Test,
        &["test", "tests", "teste", "testes", "testar", "cover"],
    ),
    (
        TaskIntent::Document,
        &["document*", "documenta*", "describe", "descrever"],
    ),
    (
        TaskIntent::Investigate,
        &[
            "explain",
            "investigate",
            "analyze",
            "analyse",
            "review",
            "why",
            "explicar",
            "analisar",
            "revisar",
            "audit",
            "understand",
            "entender",
        ],
    ),
];

fn matches_verb(word: &str, verb: &str) -> bool {
    match verb.strip_suffix('*') {
        Some(prefix) => word.starts_with(prefix),
        None => word == verb,
    }
}

pub struct TaskAnalyzer;

impl TaskAnalyzer {
    /// Reads a task. Works on any text and any language: words it does not know simply carry no
    /// tag, and an empty task has no signals.
    pub fn analyze(task: &str) -> TaskSignals {
        let all = words(task);
        let intent = all
            .iter()
            .find_map(|w| {
                INTENT_VERBS
                    .iter()
                    .find(|(_, verbs)| verbs.iter().any(|v| matches_verb(w, v)))
                    .map(|(intent, _)| *intent)
            })
            .unwrap_or(TaskIntent::Unknown);
        let is_verb = |w: &str| {
            INTENT_VERBS
                .iter()
                .any(|(_, verbs)| verbs.iter().any(|v| matches_verb(w, v)))
        };
        let mut keywords: Vec<String> = Vec::new();
        for w in &all {
            if w.len() >= 3
                && !w.chars().all(|c| c.is_ascii_digit())
                && !STOPWORDS.contains(&w.as_str())
                && !is_verb(w)
                && !keywords.contains(w)
            {
                keywords.push(w.clone());
            }
        }
        keywords.truncate(24);

        // Tags come from the whole text, including the verbs: "write tests" is about testing.
        let tags = super::taxonomy::tags_of_words(&all);
        let mut areas: BTreeSet<Area> = BTreeSet::new();
        for tag in &tags {
            areas.extend(tag_areas(tag).iter().copied());
        }
        let technologies = technologies_of_words(&all)
            .into_iter()
            .map(str::to_owned)
            .collect();
        TaskSignals {
            keywords,
            areas: areas.into_iter().collect(),
            baseline_areas: baseline_areas(intent).to_vec(),
            tags: tags.into_iter().map(str::to_owned).collect(),
            intent,
            technologies,
        }
    }
}

fn baseline_areas(intent: TaskIntent) -> &'static [Area] {
    match intent {
        TaskIntent::Add | TaskIntent::Refactor => {
            &[Area::Architecture, Area::Conventions, Area::Testing]
        }
        TaskIntent::Change => &[Area::Conventions, Area::Testing],
        TaskIntent::Fix | TaskIntent::Test => &[Area::Testing, Area::Conventions],
        TaskIntent::Document => &[Area::Architecture, Area::Business],
        TaskIntent::Investigate | TaskIntent::Unknown => &[Area::Architecture],
    }
}

/// What selecting did, before the text is composed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    /// Included entries first, in the order they are told; then the rest.
    pub entries: Vec<ContextEntry>,
    /// Relevant items the budget could not fit, by area.
    pub over_budget: Vec<OmittedGroup>,
    /// Items judged unrelated to the task, by area.
    pub unrelated: Vec<OmittedGroup>,
}

impl Selection {
    pub fn included(&self) -> impl Iterator<Item = &ContextItem> {
        self.entries
            .iter()
            .filter(|e| e.outcome == EntryOutcome::Included)
            .map(|e| &e.item)
    }
}

pub struct TaskContextService;

impl TaskContextService {
    /// Chooses among `candidates`. `room` is how many characters the chosen items may take;
    /// constraints and decisions are kept even if they exceed it.
    pub fn select(
        signals: &TaskSignals,
        mut candidates: Vec<ContextItem>,
        room: usize,
    ) -> Selection {
        candidates.extend(not_identified(signals, &candidates));
        let mut scored: Vec<(ContextItem, SelectionReason)> = candidates
            .into_iter()
            .map(|item| {
                let reason = explain(signals, &item);
                (item, reason)
            })
            .collect();
        // The order they are told in, and the order the budget is spent in.
        scored.sort_by(|(a, ra), (b, rb)| {
            rank(a)
                .cmp(&rank(b))
                .then(rb.score.cmp(&ra.score))
                .then(a.id.cmp(&b.id))
        });

        let room = room.saturating_sub(NOTES_RESERVE);
        let mut used = 0usize;
        let mut groups: BTreeSet<(u8, Option<FindingCategory>)> = BTreeSet::new();
        let mut included = Vec::new();
        let mut over_budget = Vec::new();
        let mut not_relevant = Vec::new();
        for (item, reason) in scored {
            let relevant = reason.always_included.is_some() || reason.score >= Weights::THRESHOLD;
            if !relevant {
                not_relevant.push(ContextEntry {
                    item,
                    reason,
                    outcome: EntryOutcome::NotRelevant,
                });
                continue;
            }
            let header = match item.group {
                Some(g) if groups.insert((block_key(item.block), Some(g))) => GROUP_HEADER_ESTIMATE,
                _ => 0,
            };
            let cost = item.content.chars().count() + 4 + header;
            if reason.always_included.is_some() || used + cost <= room {
                used += cost;
                included.push(ContextEntry {
                    item,
                    reason,
                    outcome: EntryOutcome::Included,
                });
            } else {
                over_budget.push(ContextEntry {
                    item,
                    reason,
                    outcome: EntryOutcome::OverBudget,
                });
            }
        }
        not_relevant.sort_by(|a, b| {
            b.reason
                .score
                .cmp(&a.reason.score)
                .then(a.item.id.cmp(&b.item.id))
        });
        let over = count_by_area(&over_budget);
        let unrelated = count_by_area(&not_relevant);
        let mut entries = included;
        entries.extend(over_budget);
        entries.extend(not_relevant);
        Selection {
            entries,
            over_budget: over,
            unrelated,
        }
    }
}

fn block_key(block: ContextBlock) -> u8 {
    match block {
        ContextBlock::Know => 0,
        ContextBlock::Infer => 1,
        ContextBlock::User => 2,
        ContextBlock::Unknown => 3,
        ContextBlock::Outdated => 4,
    }
}

fn count_by_area(entries: &[ContextEntry]) -> Vec<OmittedGroup> {
    let mut counts: std::collections::BTreeMap<Area, usize> = std::collections::BTreeMap::new();
    for e in entries {
        *counts.entry(e.item.area).or_default() += 1;
    }
    counts
        .into_iter()
        .map(|(area, count)| OmittedGroup { area, count })
        .collect()
}

/// The order knowledge is told in and kept in when the budget is short. Lower first:
/// security constraints, decisions, the user's other constraints, the user's other words,
/// verified facts, architecture, modules, conventions, testing, dependencies and the rest of
/// the inferences, what is not known, what may be outdated.
fn rank(item: &ContextItem) -> u8 {
    match item.kind {
        ItemKind::Constraint => {
            if item.tags.iter().any(|t| t == "security") {
                10
            } else {
                30
            }
        }
        ItemKind::Decision => 20,
        _ => match item.block {
            ContextBlock::User => 35,
            ContextBlock::Know => 40,
            ContextBlock::Infer => match item.area {
                Area::Architecture => 50,
                Area::Modules => 60,
                Area::Conventions => 70,
                Area::Testing => 80,
                Area::Dependencies => 90,
                _ => 100,
            },
            ContextBlock::Unknown => 110,
            ContextBlock::Outdated => 120,
        },
    }
}

/// Scores one item and records why.
pub fn explain(signals: &TaskSignals, item: &ContextItem) -> SelectionReason {
    let mut reason = SelectionReason::default();
    let mut score = 0;
    match item.kind {
        ItemKind::Constraint => {
            reason.always_included = Some(AlwaysIncluded::Constraint);
            score += Weights::CONSTRAINT_OR_DECISION;
        }
        ItemKind::Decision => {
            reason.always_included = Some(AlwaysIncluded::Decision);
            score += Weights::CONSTRAINT_OR_DECISION;
        }
        // What is said about the analysis itself, and a disagreement nobody settled, are never
        // judged unrelated: they say how far to trust the rest.
        ItemKind::Notice | ItemKind::Conflict => score += Weights::THRESHOLD,
        _ => {}
    }

    reason.matched_tags = item
        .tags
        .iter()
        .filter(|t| signals.tags.contains(t))
        .cloned()
        .collect();
    let mut signal = 0;
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    {
        signal +=
            reason.matched_tags.len().min(Weights::MAX_TAG_MATCHES) as i32 * Weights::TAG_MATCH;
    }

    if let Some(category) = item.category {
        if signals
            .tags
            .iter()
            .any(|t| tag_categories(t).contains(&category))
        {
            reason.matched_categories.push(category);
            signal += Weights::CATEGORY_MATCH;
        }
    }

    for e in &item.evidence {
        let path_words = words(&e.source);
        if signals
            .keywords
            .iter()
            .any(|k| path_words.iter().any(|p| same_word(k, p)))
        {
            reason.matched_paths.push(e.source.clone());
        }
    }
    if !reason.matched_paths.is_empty() {
        signal += Weights::PATH_MATCH;
    }

    let text_words = words(&format!("{} {}", item.label, item.content));
    reason.matched_keywords = signals
        .keywords
        .iter()
        .filter(|k| text_words.iter().any(|w| same_word(k, w)))
        .cloned()
        .collect();
    if !reason.matched_keywords.is_empty() {
        signal += Weights::KEYWORD_MATCH;
    }

    // The area orients; it does not choose. An area the task's tags point at counts only
    // for items that also match something specific (every module is "about authentication"
    // otherwise), while the areas that orient any task of this intent, and the stack, count
    // on their own.
    if signals.baseline_areas.contains(&item.area) {
        reason.matched_areas.push(item.area);
        signal += Weights::BASELINE_AREA;
    } else if signals.areas.contains(&item.area) && signal > 0 {
        reason.matched_areas.push(item.area);
        signal += Weights::FOCUS_AREA;
    } else if item.area == Area::Stack
        && (signals.tags.is_empty() || !item.tags.iter().any(|t| is_layer(t)))
    {
        // Languages and tooling orient any task; a framework of one layer (Angular) orients
        // only a task that says nothing about layers or is in that layer (its tag then matches).
        reason.matched_areas.push(item.area);
        signal += Weights::STACK_ORIENTATION;
    }

    if signal > 0 {
        if item.provenance.is_some_and(Origin::is_user) {
            signal += Weights::USER_KNOWLEDGE;
        }
        if item.verification == Some(VerificationStatus::Verified)
            && item.block != ContextBlock::Outdated
        {
            signal += Weights::VERIFIED;
        }
    }

    let item_layers: BTreeSet<&str> = item
        .tags
        .iter()
        .map(String::as_str)
        .filter(|t| is_layer(t))
        .collect();
    let task_layers: BTreeSet<&str> = signals
        .tags
        .iter()
        .map(String::as_str)
        .filter(|t| is_layer(t))
        .collect();
    if !item_layers.is_empty() && !task_layers.is_empty() && item_layers.is_disjoint(&task_layers) {
        signal += Weights::LAYER_MISMATCH;
        reason.penalties.push("layer_mismatch".to_owned());
    }

    reason.score = score + signal;
    reason
}

/// Technologies the task names that nothing in the Harness mentions. Said as what they are: not
/// identified in the analyzed evidence, which is not proof they are not used.
fn not_identified(signals: &TaskSignals, candidates: &[ContextItem]) -> Vec<ContextItem> {
    if signals.technologies.is_empty() {
        return Vec::new();
    }
    let known: Vec<String> = candidates
        .iter()
        .flat_map(|c| {
            let mut w = words(&c.label);
            w.extend(words(&c.content));
            w.extend(c.evidence.iter().flat_map(|e| words(&e.source)));
            w
        })
        .collect();
    signals
        .technologies
        .iter()
        .filter(|tech| {
            !technology_words(tech)
                .iter()
                .any(|name| known.iter().any(|w| w == name))
        })
        .map(|tech| ContextItem {
            id: format!("gap:tech:{}", tech.to_lowercase()),
            block: ContextBlock::Unknown,
            kind: ItemKind::NotFound,
            area: Area::Infrastructure,
            category: None,
            label: format!("{tech} not identified"),
            content: format!(
                "Not found: {tech} was not identified in the analyzed evidence. This does not \
                 prove that {tech} is not used."
            ),
            provenance: Some(Origin::Generated),
            verification: None,
            confidence: None,
            evidence: Vec::new(),
            source_finding_id: None,
            tags: tags_of_technology(tech),
            priority: 10,
            group: None,
        })
        .collect()
}

#[cfg(test)]
mod tests;
