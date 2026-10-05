//! Small text tools the engine's comparisons are made of. Lines are the unit: the prompt's
//! context is made of short, self-contained lines (bullets, labelled facts), and a line is either
//! said again or it is not.

use std::collections::HashSet;

/// A line worth comparing: it says something (enough words), is not a heading or a marker, and is
/// not inside a code fence.
pub const MIN_WORDS: usize = 4;

/// Lowercase words (letters and digits), as a set.
pub fn words(line: &str) -> HashSet<String> {
    line.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// The line with case, punctuation and spacing ignored: what "the same" means for the
/// normalized comparison.
pub fn normalized(line: &str) -> String {
    line.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether every word of `line` is in `other`. A line contained in another says nothing the other
/// does not (the other may say more).
pub fn contained_in(line: &HashSet<String>, other: &HashSet<String>) -> bool {
    !line.is_empty() && line.iter().all(|w| other.contains(w))
}

/// For each line of `text`: whether it is inside a code fence (the fence lines included). Code and
/// literal blocks are never compared or edited.
pub fn fenced_lines(text: &str) -> Vec<bool> {
    let mut inside = false;
    text.lines()
        .map(|line| {
            let fence = line.trim_start().starts_with("```");
            if fence {
                inside = !inside;
                true
            } else {
                inside
            }
        })
        .collect()
}

/// A bullet or a sub-bullet of a list.
pub fn is_bullet(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("- ") || t.starts_with("* ")
}

/// A line that only introduces a list (`Decisions:`).
pub fn is_list_heading(line: &str) -> bool {
    let t = line.trim_end();
    !t.is_empty() && t.ends_with(':') && !is_bullet(t)
}

/// The start of a text for the audit trail, on one line.
pub fn preview(text: &str) -> String {
    let one: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() <= 80 {
        one
    } else {
        let mut cut: String = one.chars().take(80).collect();
        cut.push('…');
        cut
    }
}

/// Collapses runs of blank lines to one and trims trailing spaces, outside code fences. No word
/// changes. Returns the text and whether anything changed.
pub fn compress_whitespace(text: &str) -> (String, bool) {
    let fenced = fenced_lines(text);
    let mut out: Vec<&str> = Vec::new();
    let mut blank_run = false;
    for (line, in_fence) in text.lines().zip(fenced) {
        if in_fence {
            out.push(line);
            blank_run = false;
            continue;
        }
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            if blank_run {
                continue;
            }
            blank_run = true;
        } else {
            blank_run = false;
        }
        out.push(trimmed);
    }
    let mut result = out.join("\n");
    if text.ends_with('\n') {
        result.push('\n');
    }
    let changed = result != text;
    (result, changed)
}
