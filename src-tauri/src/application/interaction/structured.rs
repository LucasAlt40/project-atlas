//! The `atlas-interaction` block: how an agent, on any runtime, says it cannot go on without a
//! person. Read as data and validated; only a closed set of fields is kept.
//!
//! The agent ends its answer with a fenced block tagged `atlas-interaction` holding JSON:
//! `{"type":"clarification","question":"Which endpoint?","options":["/api/users"]}`.
//!
//! If the answer also ends with an `atlas-result` block *after* it, the agent concluded and the
//! interaction block is stale: nothing waits.

use serde_json::Value;

use super::clip;
use crate::application::orchestration::result_parser::RESULT_FENCE;
use crate::domain::interaction::{
    decision_options, DetectionSource, InteractionDetection, InteractionKind, InteractionOption,
    MAX_CONTEXT, MAX_DOCUMENT, MAX_OPTION, MAX_OPTIONS, MAX_QUESTION,
};

pub const INTERACTION_FENCE: &str = "atlas-interaction";

pub enum Block {
    /// A valid request for a person.
    Interaction(InteractionDetection),
    /// A result block closes the answer: the agent finished.
    Concluded,
    /// No block either way.
    Absent,
}

pub fn detect(text: &str) -> Block {
    let interaction = last_block(text, INTERACTION_FENCE);
    let result = last_block(text, RESULT_FENCE);
    match (interaction, result) {
        (None, None) => Block::Absent,
        (None, Some(_)) => Block::Concluded,
        (Some((at, _)), Some((result_at, _))) if result_at > at => Block::Concluded,
        (Some((at, body)), _) => parse(body)
            .map(|mut found| {
                found.document = clip(&text[..at], MAX_DOCUMENT);
                found
            })
            .map_or(Block::Absent, Block::Interaction),
    }
}

/// Position and body of the last fenced block with this tag.
fn last_block<'a>(text: &'a str, tag: &str) -> Option<(usize, &'a str)> {
    let marker = format!("```{tag}");
    let at = text.rfind(&marker)?;
    let rest = &text[at + marker.len()..];
    let end = rest.find("```")?;
    Some((at, rest[..end].trim()))
}

fn parse(body: &str) -> Option<InteractionDetection> {
    let value: Value = serde_json::from_str(body).ok()?;
    let object = value.as_object()?;
    let question = clip(object.get("question")?.as_str()?, MAX_QUESTION);
    if question.is_empty() {
        return None;
    }
    let kind = match object.get("type").and_then(Value::as_str) {
        Some(said) => InteractionKind::parse(said)?,
        None => InteractionKind::Clarification,
    };
    let context = object
        .get("context")
        .and_then(Value::as_str)
        .map(|c| clip(c, MAX_CONTEXT))
        .unwrap_or_default();
    let offered = options(object.get("options"));
    // The buttons of a decision are Atlas's: an agent cannot invent what "approve" means.
    let options = match kind {
        InteractionKind::Clarification | InteractionKind::RuntimeConfirmation
            if !offered.is_empty() =>
        {
            offered
        }
        _ => decision_options(kind),
    };
    Some(InteractionDetection {
        detected: true,
        kind: Some(kind),
        confidence: 100,
        question,
        context,
        document: String::new(),
        options,
        source: DetectionSource::Structured,
    })
}

fn options(value: Option<&Value>) -> Vec<InteractionOption> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out: Vec<InteractionOption> = Vec::new();
    for item in items {
        let (id, label) = match item {
            Value::String(text) => (text.as_str(), text.as_str()),
            Value::Object(map) => {
                let label = map
                    .get("label")
                    .or_else(|| map.get("id"))
                    .and_then(Value::as_str);
                let id = map.get("id").and_then(Value::as_str).or(label);
                match (id, label) {
                    (Some(id), Some(label)) => (id, label),
                    _ => continue,
                }
            }
            _ => continue,
        };
        let (id, label) = (clip(id, MAX_OPTION), clip(label, MAX_OPTION));
        if id.is_empty() || out.iter().any(|o| o.id == id) {
            continue;
        }
        out.push(InteractionOption { id, label });
        if out.len() == MAX_OPTIONS {
            break;
        }
    }
    out
}
