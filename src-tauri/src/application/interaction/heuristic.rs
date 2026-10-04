//! Text analysis, the last resort. It looks only at how the answer *ends*: an agent that is
//! waiting for a person stops on its question, while a question in the middle of a report
//! ("I checked whether the endpoint exists.") or after the work is described is not a block.
//!
//! A hit needs all of: the final sentence is a question, it is addressed to the person (an
//! offer to do something, a choice, a request to confirm), and the answer carries no result.
//! Offers to do *more* ("Would you like me to also…") are not blocks and score below the bar.

use super::{clip, MIN_CONFIDENCE};
use crate::domain::interaction::{
    decision_options, DetectionSource, InteractionDetection, InteractionKind, MAX_CONTEXT,
    MAX_DOCUMENT, MAX_QUESTION,
};

/// Phrases of someone asking the person to decide or to give information.
const ADDRESSED: &[&str] = &[
    "should i",
    "shall i",
    "may i",
    "can i ",
    "could i ",
    "do you want",
    "do you allow",
    "allow me",
    "permitir",
    "would you like",
    "do you prefer",
    "would you prefer",
    "want me to",
    "which ",
    "what should",
    "what do you",
    "what is the ",
    "what's the ",
    "how should",
    "how do you want",
    "can you ",
    "could you ",
    "please confirm",
    "is it ok",
    "is that ok",
    "is this ok",
    "are you ok",
    "do you have",
    "did you mean",
    "posso",
    "devo ",
    "devemos",
    "podemos",
    "pode ",
    "aprova",
    "approve",
    "approval",
    "go ahead",
    "proceed",
    "confirma",
    "autoriza",
    "seguir",
    "prosseguir",
    "continuar",
    "deseja",
    "quer que",
    "qual ",
    "quais ",
    "como devo",
    "gostaria",
    "prefere",
    "pode confirmar",
    "pode me dizer",
    "voce quer",
    "você quer",
    "você prefere",
];

/// Offers of extra work: the agent is done and asks if more is wanted.
const OFFERS: &[&str] = &[
    "also ",
    "additionally",
    "in addition",
    "anything else",
    "any other",
    "further ",
    "too?",
    "também",
    "tambem",
    "além",
    "mais alguma",
];

/// Signs the runtime itself asks for a keystroke.
const RUNTIME_PROMPTS: &[&str] = &[
    "[y/n]",
    "(y/n)",
    "[y/n]?",
    "(yes/no)",
    "[yes/no]",
    "allow this tool",
    "allow this operation",
    "allow this command",
    "allow this action",
    "apply these changes",
    "apply this change",
];

const INTERROGATIVES: &[&str] = &[
    "which ", "what ", "how ", "where ", "who ", "qual ", "quais ", "como ", "onde ", "quem ",
];

const PERMISSION_WORDS: &[&str] = &[
    "allow",
    "permit",
    "permission",
    "permissão",
    "permissao",
    "permitir",
    "autoriz",
    "authoriz",
];

const DECISION_WORDS: &[&str] = &[
    "may i",
    "shall i",
    "can i ",
    "could i ",
    "should i",
    "do you want",
    "want me to",
    "would you like",
    "posso",
    "devo ",
    "devemos",
    "podemos",
    "aprova",
    "approve",
    "seguir",
    "prosseguir",
    "deseja",
    "quer que",
    "proceed",
    "go ahead",
    "continue",
    "confirm",
    "ready to",
    "pronto para",
    "implement",
    "implementar",
    "is it ok",
    "is that ok",
    "is this ok",
];

pub fn detect(text: &str) -> InteractionDetection {
    detect_with(text, true)
}

/// For a step that ended with no valid result: any question that ends the message is taken as
/// a request for the person, at a confidence below the bar a normal answer must clear. The
/// caller decides what to do with it (an unfinished step is better asked about than failed).
pub fn detect_unfinished(text: &str) -> InteractionDetection {
    let at_the_end = detect_with(text, false);
    if at_the_end.detected {
        return at_the_end;
    }
    question_in_the_tail(text)
}

/// How far back from the end of an unfinished message a question still counts.
const TAIL_CHARS: usize = 700;

/// A question anywhere in the last stretch of a message that ended without a result: agents
/// often add a summary after "Ready to implement?". Only for steps with no valid outcome, which
/// would otherwise just fail.
fn question_in_the_tail(text: &str) -> InteractionDetection {
    let none = InteractionDetection::none(DetectionSource::Heuristic);
    let stripped = strip_code(text);
    let body = stripped.trim();
    let start = body
        .char_indices()
        .rev()
        .nth(TAIL_CHARS)
        .map_or(0, |(i, _)| i);
    let tail = &body[start..];
    let Some(mark) = tail.rfind('?') else {
        return none;
    };
    let before = &tail[..mark];
    let from = before
        .char_indices()
        .rev()
        .find(|(_, c)| matches!(c, '.' | '!' | '?' | '\n'))
        .map_or(0, |(i, c)| i + c.len_utf8());
    let sentence = tail[from..=mark].trim();
    if sentence.split_whitespace().count() < 3 {
        return none;
    }
    let lower = sentence.to_lowercase();
    let paragraph_lower = tail.to_lowercase();
    let kind = classify(&lower, &paragraph_lower, false);
    InteractionDetection {
        detected: true,
        kind: Some(kind),
        confidence: 60,
        question: clip(sentence, MAX_QUESTION),
        context: clip(&tail[..from], MAX_CONTEXT),
        document: clip(text, MAX_DOCUMENT),
        options: decision_options(kind),
        source: DetectionSource::Heuristic,
    }
}

fn detect_with(text: &str, strict: bool) -> InteractionDetection {
    let none = || InteractionDetection::none(DetectionSource::Heuristic);
    let stripped = strip_code(text);
    let body = trim_closers(stripped.trim());
    if body.is_empty() {
        return none();
    }
    let (before, last_paragraph) = split_last_paragraph(body);
    let Some((lead, question)) = final_sentence(last_paragraph) else {
        return none();
    };
    if !question.trim_end().ends_with('?') {
        return none();
    }
    let lower = question.to_lowercase();
    let paragraph_lower = last_paragraph.to_lowercase();

    let runtime = RUNTIME_PROMPTS.iter().any(|p| paragraph_lower.contains(p));
    let addressed = ADDRESSED.iter().any(|p| lower.contains(p));
    if strict && !runtime && !addressed {
        return none();
    }
    let offer = OFFERS.iter().any(|p| lower.contains(p));

    let mut confidence: u8 = if runtime {
        90
    } else if addressed {
        80
    } else {
        60
    };
    if offer {
        confidence -= 35;
    }
    // A long report that happens to end on a question is more likely a courtesy than a block.
    if body.chars().count() > 3000 {
        confidence -= 10;
    }
    if strict && confidence < MIN_CONFIDENCE {
        return none();
    }

    let kind = classify(&lower, &paragraph_lower, runtime);
    let question_text = clip(&format!("{lead}{question}"), MAX_QUESTION);
    let context = clip(before, MAX_CONTEXT);
    InteractionDetection {
        detected: true,
        kind: Some(kind),
        confidence,
        question: question_text,
        context,
        document: clip(text, MAX_DOCUMENT),
        options: decision_options(kind),
        source: DetectionSource::Heuristic,
    }
}

fn classify(question: &str, paragraph: &str, runtime: bool) -> InteractionKind {
    if runtime {
        return InteractionKind::RuntimeConfirmation;
    }
    if PERMISSION_WORDS.iter().any(|w| question.contains(w))
        || (PERMISSION_WORDS.iter().any(|w| paragraph.contains(w))
            && DECISION_WORDS.iter().any(|w| question.contains(w)))
    {
        return InteractionKind::Permission;
    }
    // "Which API should I use?" asks for information even though it says "should I".
    let asks_for_information = INTERROGATIVES.iter().any(|w| question.contains(w));
    if !asks_for_information && DECISION_WORDS.iter().any(|w| question.contains(w)) {
        return InteractionKind::Approval;
    }
    InteractionKind::Clarification
}

/// Phrases of a sign-off that follows a question without changing it ("I am ready to code.").
const CLOSERS: &[&str] = &[
    "ready to",
    "awaiting",
    "waiting for",
    "let me know",
    "clear plan",
    "pronto para",
    "aguardo",
    "estou à disposição",
];

/// Drops the sign-offs an agent puts after its question (a name, "I have a plan and am ready to
/// code."), so the question is again the end of what it said. Only short or recognisable
/// sentences go; anything that says something else stays and the question is no longer the end.
fn trim_closers(text: &str) -> &str {
    let mut body = text.trim_end();
    loop {
        let Some(last) = body.chars().last() else {
            return body;
        };
        if last == '?' {
            return body;
        }
        let head = &body[..body.len() - last.len_utf8()];
        let start = head
            .char_indices()
            .rev()
            .find(|(_, c)| matches!(c, '.' | '!' | '?' | '\n'))
            .map_or(0, |(i, c)| i + c.len_utf8());
        let sentence = body[start..].trim();
        let lower = sentence.to_lowercase();
        let short = sentence.split_whitespace().count() <= 3;
        if start == 0 || !(short || CLOSERS.iter().any(|c| lower.contains(c))) {
            return body;
        }
        body = body[..start].trim_end();
    }
}

/// The text without fenced code, where a `?` means nothing.
fn strip_code(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_code = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if !in_code {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// (everything before, the last paragraph).
fn split_last_paragraph(body: &str) -> (&str, &str) {
    match body.rfind("\n\n") {
        Some(at) => (body[..at].trim(), body[at..].trim()),
        None => ("", body),
    }
}

/// The last sentence of a paragraph, and what precedes it in the paragraph. A sentence ends at
/// `.`, `!`, `?` or a line break; the final `?` stays with it.
fn final_sentence(paragraph: &str) -> Option<(&str, &str)> {
    let paragraph = strip_choice_hint(paragraph.trim_end());
    let trimmed = paragraph.trim_end_matches(['*', '_', '"', '\'', ')', '`', ' ']);
    if !trimmed.ends_with('?') {
        return None;
    }
    // Walk back from just before the final `?` to the previous sentence boundary.
    let head = &trimmed[..trimmed.len() - 1];
    let start = head
        .char_indices()
        .rev()
        .find(|(_, c)| matches!(c, '.' | '!' | '?' | '\n'))
        .map_or(0, |(i, c)| i + c.len_utf8());
    let (lead, question) = paragraph.split_at(start);
    Some((lead.trim_start_matches('\n'), question.trim_start()))
}

/// Removes a trailing `[y/N]`-style hint, so the question before it is the end of the text.
fn strip_choice_hint(text: &str) -> &str {
    let trimmed = text.trim_end();
    for (open, close) in [('[', ']'), ('(', ')')] {
        if trimmed.ends_with(close) {
            if let Some(at) = trimmed.rfind(open) {
                let hint = trimmed[at + 1..trimmed.len() - 1].to_lowercase();
                if matches!(
                    hint.as_str(),
                    "y/n" | "yes/no" | "y/n/a" | "yes/no/always" | "s/n" | "sim/não"
                ) {
                    return trimmed[..at].trim_end();
                }
            }
        }
    }
    trimmed
}
