//! The `atlas-memory` block: how an agent, on any runtime, proposes notes for the next run. Read
//! as data and validated; only a closed set of fields is kept, and what is kept is a *proposal*:
//! nothing here decides that a note enters (see `MemoryService::submit`).
//!
//! The agent ends its answer with a fenced block tagged `atlas-memory` holding a JSON array:
//! `[{"kind":"pitfall","title":"…","content":"…","topic":"…","paths":["…"],"scripts":["check"]}]`.

use serde_json::Value;

use crate::domain::memory::{MemoryKind, MemoryScope, MAX_CONTENT_CHARS, MAX_TITLE_CHARS};

pub const MEMORY_FENCE: &str = "atlas-memory";

/// The most notes one step may propose.
pub const MAX_PER_STEP: usize = 3;

/// What an agent is told when proposals are on. Short on purpose: it is paid for in every prompt.
pub const RULE: &str = "MEMORY\n\
If this task taught you something that would save the next run time or a failed attempt (a \
command that works, a trap you hit, a choice and its reason), you may end your answer with a \
fenced block tagged `atlas-memory` holding a JSON array of at most 3 notes: \
[{\"kind\":\"how_to|pitfall|decision\",\"title\":\"short\",\"content\":\"one or two sentences\",\
\"topic\":\"optional\",\"scope\":\"workspace|agent\",\"paths\":[\"project paths it depends on\"],\
\"scripts\":[\"npm scripts it depends on\"]}]. A note is a proposal a person may review: it grants \
nothing and never holds a secret. Skip the block when you learned nothing new.";

/// One proposed note, as read from the block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    pub scope: MemoryScope,
    pub kind: MemoryKind,
    pub title: String,
    pub content: String,
    pub topic: Option<String>,
    pub paths: Vec<String>,
    pub scripts: Vec<String>,
}

/// The proposals of an answer: the last `atlas-memory` block, validated. An unreadable block, or
/// no block, is none; an invalid note is left out and the others kept.
pub fn parse(text: &str) -> Vec<Proposal> {
    let Some(body) = last_block(text) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return Vec::new();
    };
    let items: Vec<&Value> = match &value {
        Value::Array(items) => items.iter().collect(),
        Value::Object(_) => vec![&value],
        _ => return Vec::new(),
    };
    items
        .into_iter()
        .filter_map(note)
        .take(MAX_PER_STEP)
        .collect()
}

/// The answer without its `atlas-memory` block: the notes are stored, and a person reading the
/// answer (or the next agent, in a handoff) has no use for their JSON.
pub fn strip(text: &str) -> String {
    let marker = format!("```{MEMORY_FENCE}");
    let Some(at) = text.rfind(&marker) else {
        return text.to_owned();
    };
    let after = &text[at + marker.len()..];
    let Some(end) = after.find("```") else {
        return text.to_owned();
    };
    let tail = after[end + 3..].trim_start_matches(['\r', '\n']);
    format!(
        "{}{}",
        text[..at].trim_end_matches(['\r', '\n', ' ']),
        if tail.is_empty() {
            String::new()
        } else {
            format!("\n\n{tail}")
        }
    )
}

fn last_block(text: &str) -> Option<&str> {
    let marker = format!("```{MEMORY_FENCE}");
    let at = text.rfind(&marker)?;
    let rest = &text[at + marker.len()..];
    let end = rest.find("```")?;
    Some(rest[..end].trim())
}

fn note(value: &Value) -> Option<Proposal> {
    let object = value.as_object()?;
    let text = |key: &str, max: usize| {
        object
            .get(key)
            .and_then(Value::as_str)
            .map(|t| t.trim().chars().take(max + 1).collect::<String>())
    };
    let kind = match object.get("kind").and_then(Value::as_str)? {
        "how_to" => MemoryKind::HowTo,
        "pitfall" => MemoryKind::Pitfall,
        "preference" => MemoryKind::Preference,
        "decision" => MemoryKind::Decision,
        _ => return None,
    };
    // An agent never writes to the user's own scope: that speaks for the person.
    let scope = match object.get("scope").and_then(Value::as_str) {
        Some("agent") => MemoryScope::Agent,
        _ => MemoryScope::Workspace,
    };
    let title = text("title", MAX_TITLE_CHARS)?;
    let content = text("content", MAX_CONTENT_CHARS)?;
    let topic = text("topic", MAX_TITLE_CHARS).filter(|t| !t.is_empty());
    Some(Proposal {
        scope,
        kind,
        title,
        content,
        topic,
        paths: strings(object.get("paths"), is_plain_path),
        scripts: strings(object.get("scripts"), is_script_name),
    })
}

fn strings(value: Option<&Value>, accept: fn(&str) -> bool) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|s| accept(s))
                .take(5)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// A project-relative path made of plain names: never `..`, absolute, or with a drive.
fn is_plain_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 200
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains(':')
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != ".." && part != ".")
}

fn is_script_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 60
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '.'))
}

/// Whether the answer's own result lets a note in without a person: no `atlas-result` block, or
/// one that is not a failure. A step that failed, or said it could not do the work, has nothing
/// to teach that Atlas should believe by itself.
pub fn answer_allows_auto(text: &str) -> bool {
    let marker = format!(
        "```{}",
        crate::application::orchestration::result_parser::RESULT_FENCE
    );
    let Some(at) = text.rfind(&marker) else {
        return true;
    };
    let rest = &text[at + marker.len()..];
    let Some(end) = rest.find("```") else {
        return false;
    };
    let Ok(Value::Object(result)) = serde_json::from_str::<Value>(rest[..end].trim()) else {
        return false;
    };
    let said = |key: &str| {
        result
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_lowercase)
    };
    let bad = ["fail", "failed", "blocked", "error"];
    ["status", "outcome"]
        .iter()
        .filter_map(|key| said(key))
        .all(|v| !bad.contains(&v.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(json: &str) -> String {
        format!("Done.\n\n```{MEMORY_FENCE}\n{json}\n```\n")
    }

    #[test]
    fn reads_the_notes_of_the_last_block_with_their_dependencies() {
        let text = block(
            r#"[{"kind":"pitfall","title":"Rounding","content":"Never use floats for totals",
                "topic":"Money","paths":["src/money.rs","../secret"],"scripts":["check","bad name"]}]"#,
        );

        let found = parse(&text);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, MemoryKind::Pitfall);
        assert_eq!(found[0].scope, MemoryScope::Workspace);
        assert_eq!(found[0].topic.as_deref(), Some("Money"));
        assert_eq!(found[0].paths, ["src/money.rs"]);
        assert_eq!(found[0].scripts, ["check"]);
    }

    #[test]
    fn an_agent_can_never_write_to_the_users_own_scope() {
        let text = block(
            r#"[{"kind":"how_to","title":"T","content":"c","scope":"user"},
                {"kind":"how_to","title":"T2","content":"c","scope":"agent"}]"#,
        );

        let scopes: Vec<_> = parse(&text).iter().map(|p| p.scope).collect();

        assert_eq!(scopes, [MemoryScope::Workspace, MemoryScope::Agent]);
    }

    #[test]
    fn no_block_a_broken_block_or_a_wrong_shape_is_no_proposal() {
        assert_eq!(parse("just an answer"), []);
        assert_eq!(parse(&block("{not json")), []);
        assert_eq!(parse(&block("\"text\"")), []);
        assert_eq!(
            parse(&block(r#"[{"kind":"poem","title":"T","content":"c"}]"#)),
            []
        );
        assert_eq!(parse(&block(r#"[{"kind":"how_to","content":"c"}]"#)), []);
    }

    #[test]
    fn at_most_three_notes_a_step_and_a_bad_one_does_not_stop_the_rest() {
        let note = |n: u8| format!(r#"{{"kind":"how_to","title":"T{n}","content":"c{n}"}}"#);
        let json = format!(
            "[{{\"kind\":\"bad\"}},{},{},{},{}]",
            note(1),
            note(2),
            note(3),
            note(4)
        );

        let found = parse(&block(&json));

        assert_eq!(
            found.iter().map(|p| p.title.as_str()).collect::<Vec<_>>(),
            ["T1", "T2", "T3"]
        );
    }

    #[test]
    fn a_single_object_is_read_as_one_note_and_oversized_text_is_kept_oversized_to_be_refused() {
        let long = "x".repeat(MAX_CONTENT_CHARS + 50);
        let found = parse(&block(&format!(
            r#"{{"kind":"how_to","title":"T","content":"{long}"}}"#
        )));

        assert_eq!(found.len(), 1);
        // One character past the limit survives, so validation refuses it instead of the parser
        // quietly cutting a lesson in half.
        assert_eq!(found[0].content.chars().count(), MAX_CONTENT_CHARS + 1);
    }

    #[test]
    fn a_failed_or_blocked_result_never_lets_a_note_in_by_itself() {
        let with = |result: &str| format!("x\n```atlas-result\n{result}\n```\n");

        assert!(answer_allows_auto("no result block at all"));
        assert!(answer_allows_auto(&with(r#"{"status":"pass"}"#)));
        assert!(answer_allows_auto(&with(
            r#"{"status":"success","outcome":"implemented"}"#
        )));
        assert!(!answer_allows_auto(&with(r#"{"status":"fail"}"#)));
        assert!(!answer_allows_auto(&with(
            r#"{"status":"pass","outcome":"blocked"}"#
        )));
        assert!(!answer_allows_auto(&with("{broken")));
    }

    #[test]
    fn stripping_takes_the_block_out_and_keeps_the_result_block_after_it() {
        let text = format!(
            "Done.\n\n```{MEMORY_FENCE}\n[{{\"kind\":\"how_to\",\"title\":\"T\",\"content\":\"c\"}}]\n```\n\n```atlas-result\n{{\"status\":\"pass\"}}\n```\n"
        );

        let stripped = strip(&text);

        assert_eq!(
            stripped,
            "Done.\n\n```atlas-result\n{\"status\":\"pass\"}\n```\n"
        );
        assert_eq!(strip("no block"), "no block");
        assert_eq!(
            strip("open ```atlas-memory never closed"),
            "open ```atlas-memory never closed"
        );
    }

    #[test]
    fn the_rule_is_short_and_says_a_note_grants_nothing() {
        assert!(RULE.contains("grants\nnothing") || RULE.contains("grants nothing"));
        assert!(RULE.chars().count() < 800);
    }
}
