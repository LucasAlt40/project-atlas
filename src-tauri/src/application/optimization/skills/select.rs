//! Which skills a task calls for. Deterministic and explainable: keywords, stack, personality,
//! file types, an explicit request and the skill's own priority. No model judges a skill; that can
//! come later if it ever proves better than this.

use std::collections::HashSet;

use super::model::ParsedSkill;
use crate::application::optimization::context::text::words;

/// Below this a skill is not worth its tokens: two words of its name, or one plus its stack.
pub const MIN_SCORE: i32 = 4;
/// Skills loaded for one execution, at most.
pub const MAX_ACTIVE: usize = 3;

pub struct SelectionInput<'a> {
    /// What the work is: the task, and for a step its label and instructions.
    pub task: &'a str,
    pub personality_id: &'a str,
    pub technologies: &'a [String],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub name: String,
    pub score: i32,
    /// The task asked for this skill by name.
    pub explicit: bool,
    /// Why, for the audit trail.
    pub reasons: Vec<String>,
}

const STOP_WORDS: [&str; 44] = [
    "the", "and", "for", "with", "that", "this", "from", "into", "when", "use", "using", "your",
    "you", "are", "any", "all", "not", "but", "can", "will", "should", "make", "task", "step",
    "work", "agent", "para", "com", "que", "uma", "uns", "dos", "das", "nos", "nas", "por", "como",
    "mais", "isso", "esta", "este", "usar", "quando", "fazer",
];

/// A word without its plural, so "invoice" finds "invoices". Nothing cleverer: a stemmer that
/// guesses wrong selects the wrong skill.
fn stem(word: &str) -> String {
    if let Some(root) = word.strip_suffix("ies") {
        if root.chars().count() >= 2 {
            return format!("{root}y");
        }
    }
    match word.strip_suffix('s') {
        Some(root) if word.chars().count() > 3 && !root.ends_with('s') => root.to_owned(),
        _ => word.to_owned(),
    }
}

pub(super) fn meaningful(text: &str) -> HashSet<String> {
    words(text)
        .into_iter()
        .filter(|w| w.chars().count() >= 3 && !STOP_WORDS.contains(&w.as_str()))
        .map(|w| stem(&w))
        .collect()
}

/// File extensions the task mentions (`src/app.tsx`, `main.rs`), lowercase, without the dot.
fn extensions_in(task: &str) -> HashSet<String> {
    task.split_whitespace()
        .filter_map(|token| {
            let token = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '.');
            let (_, ext) = token.rsplit_once('.')?;
            (!ext.is_empty() && ext.len() <= 5 && ext.chars().all(|c| c.is_ascii_alphanumeric()))
                .then(|| ext.to_lowercase())
        })
        .collect()
}

fn asked_for(task_lower: &str, name: &str) -> bool {
    let bounded = |pattern: &str| {
        task_lower.match_indices(pattern).any(|(at, _)| {
            let after = task_lower[at + pattern.len()..].chars().next();
            after.is_none_or(|c| !(c.is_alphanumeric() || c == '-'))
        })
    };
    bounded(&format!("/{name}"))
        || bounded(&format!("skill {name}"))
        || task_lower.contains(&format!("{name} skill"))
}

fn score(
    skill: &ParsedSkill,
    input: &SelectionInput<'_>,
    task_words: &HashSet<String>,
    task_lower: &str,
    extensions: &HashSet<String>,
) -> Candidate {
    let name = skill.id().to_owned();
    let mut reasons = Vec::new();
    if asked_for(task_lower, &name) {
        return Candidate {
            name,
            score: 1_000,
            explicit: true,
            reasons: vec!["asked for by name".to_owned()],
        };
    }
    let name_words = meaningful(&name.replace('-', " "));
    let description = meaningful(skill.description.as_deref().unwrap_or(""));
    let mut total = 0;
    let mut in_name: Vec<&String> = task_words.intersection(&name_words).collect();
    in_name.sort();
    if !in_name.is_empty() {
        total += 3 * i32::try_from(in_name.len()).unwrap_or(0);
        reasons.push(format!(
            "task words in the name: {}",
            in_name
                .iter()
                .map(|w| w.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let mut in_description: Vec<&String> = task_words
        .intersection(&description)
        .filter(|w| !name_words.contains(*w))
        .collect();
    in_description.sort();
    if !in_description.is_empty() {
        total += i32::try_from(in_description.len()).unwrap_or(0);
        reasons.push(format!(
            "task words in the description: {}",
            in_description
                .iter()
                .map(|w| w.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let stacks = skill.hint("atlas-stacks");
    let known: HashSet<String> = name_words
        .iter()
        .chain(description.iter())
        .cloned()
        .collect();
    for tech in input.technologies {
        let lower = tech.to_lowercase();
        let tech_words = meaningful(&lower);
        let mentioned =
            stacks.contains(&lower) || (!tech_words.is_empty() && tech_words.is_subset(&known));
        if mentioned {
            total += 2;
            reasons.push(format!("stack: {tech}"));
        }
    }
    if skill
        .hint("atlas-personalities")
        .iter()
        .any(|p| p == &input.personality_id.to_lowercase())
    {
        total += 3;
        reasons.push(format!("personality: {}", input.personality_id));
    }
    let wanted: HashSet<String> = skill
        .hint("atlas-files")
        .into_iter()
        .map(|f| {
            f.trim_start_matches("*.")
                .trim_start_matches('.')
                .to_owned()
        })
        .collect();
    if wanted.iter().any(|ext| extensions.contains(ext)) {
        total += 2;
        reasons.push("file type in the task".to_owned());
    }
    // Priority breaks ties and tips a near miss; it never selects a skill by itself.
    if total > 0 {
        total += skill.priority().clamp(-5, 5);
    }
    Candidate {
        name,
        score: total,
        explicit: false,
        reasons,
    }
}

/// The skills worth loading for this work, best first, at most [`MAX_ACTIVE`]. An explicit request
/// always counts; anything else needs [`MIN_SCORE`].
pub fn rank(skills: &[&ParsedSkill], input: &SelectionInput<'_>) -> Vec<Candidate> {
    let task_words = meaningful(input.task);
    let task_lower = input.task.to_lowercase();
    let extensions = extensions_in(input.task);
    let mut candidates: Vec<Candidate> = skills
        .iter()
        .map(|skill| score(skill, input, &task_words, &task_lower, &extensions))
        .filter(|c| c.explicit || c.score >= MIN_SCORE)
        .collect();
    candidates.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.name.cmp(&b.name)));
    candidates.truncate(MAX_ACTIVE);
    candidates
}
