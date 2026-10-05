use std::collections::HashSet;

use super::item::{ContextItem, Priority};
use super::text::{
    compress_whitespace, contained_in, fenced_lines, is_bullet, is_list_heading, normalized,
    preview, words, MIN_WORDS,
};
use crate::domain::optimization::{
    BudgetOverrun, ContextDecision, DecisionKind, SectionKind, TextSize,
};

/// How much a prompt may weigh, in estimated tokens. `max_tokens: None` is no budget: nothing is
/// ever left out for size.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContextBudget {
    pub max_tokens: Option<u64>,
    pub reserved_for_output: u64,
    pub reserved_for_tools: u64,
    pub reserved_for_reasoning: u64,
}

impl ContextBudget {
    /// What is left for the context itself once the reservations are made.
    fn available(&self) -> Option<u64> {
        self.max_tokens.map(|max| {
            max.saturating_sub(self.reserved_for_output)
                .saturating_sub(self.reserved_for_tools)
                .saturating_sub(self.reserved_for_reasoning)
        })
    }
}

/// What the engine decided: the items as they should be sent, and why each change was made.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextPlan {
    pub items: Vec<ContextItem>,
    pub decisions: Vec<ContextDecision>,
    pub deduplicated_lines: u32,
    pub compressed_items: u32,
    pub omitted_items: u32,
    pub over_budget: Option<BudgetOverrun>,
}

impl ContextPlan {
    pub fn changed(&self) -> bool {
        !self.decisions.is_empty()
    }
}

/// A line already in the prompt, kept so a later line can be compared against it.
struct Seen {
    exact: String,
    normalized: String,
    words: HashSet<String>,
    source: SectionKind,
}

pub struct ContextEngine;

impl ContextEngine {
    /// Plans `items` (in prompt order) under `budget`: duplicates out, whitespace tidied, then,
    /// only if a budget is still not met, whole items below `High` left out. `Required` items are
    /// compared against and never changed.
    pub fn plan(items: Vec<ContextItem>, budget: &ContextBudget) -> ContextPlan {
        let mut plan = ContextPlan {
            items,
            decisions: Vec::new(),
            deduplicated_lines: 0,
            compressed_items: 0,
            omitted_items: 0,
            over_budget: None,
        };
        Self::deduplicate(&mut plan);
        Self::compress(&mut plan);
        Self::fit(&mut plan, budget);
        plan
    }

    fn deduplicate(plan: &mut ContextPlan) {
        let mut seen: Vec<Seen> = Vec::new();
        for index in 0..plan.items.len() {
            let editable = plan.items[index].editable();
            let source = plan.items[index].source;
            let text = plan.items[index].content.clone();
            let lines: Vec<&str> = text.lines().collect();
            let fenced = fenced_lines(&text);
            let mut dropped = vec![false; lines.len()];
            for (i, line) in lines.iter().enumerate() {
                if fenced[i] || is_bullet_free_marker(line) {
                    continue;
                }
                let line_words = words(line);
                if line_words.len() < MIN_WORDS || (is_list_heading(line) && !is_bullet(line)) {
                    continue;
                }
                let exact = line.trim().to_owned();
                let norm = normalized(line);
                let found = seen.iter().find_map(|s| {
                    let kind = if s.exact == exact {
                        DecisionKind::DuplicateExact
                    } else if s.normalized == norm {
                        DecisionKind::DuplicateNormalized
                    } else if contained_in(&line_words, &s.words) {
                        DecisionKind::DuplicateOverlap
                    } else {
                        return None;
                    };
                    Some((kind, s.source))
                });
                match found {
                    Some((kind, kept_in)) if editable => {
                        dropped[i] = true;
                        plan.deduplicated_lines += 1;
                        plan.decisions.push(ContextDecision {
                            kind,
                            source,
                            kept_in: Some(kept_in),
                            bytes_saved: line.len() + 1,
                            preview: preview(line),
                        });
                    }
                    // A required line stays; it is also not registered twice.
                    Some(_) => {}
                    None => seen.push(Seen {
                        exact,
                        normalized: norm,
                        words: line_words,
                        source,
                    }),
                }
            }
            if dropped.iter().any(|d| *d) {
                Self::drop_empty_headings(&lines, &fenced, &mut dropped, source, plan);
                let mut kept: Vec<&str> = lines
                    .iter()
                    .zip(&dropped)
                    .filter(|(_, d)| !**d)
                    .map(|(l, _)| *l)
                    .collect();
                if kept.is_empty() {
                    kept.push("");
                }
                let mut rebuilt = kept.join("\n");
                if text.ends_with('\n') {
                    rebuilt.push('\n');
                }
                let item = &mut plan.items[index];
                item.content = rebuilt;
                item.size = TextSize::of(&item.content);
            }
        }
    }

    /// A heading that introduced a list whose every bullet went has nothing left to introduce.
    fn drop_empty_headings(
        lines: &[&str],
        fenced: &[bool],
        dropped: &mut [bool],
        source: SectionKind,
        plan: &mut ContextPlan,
    ) {
        for i in 0..lines.len() {
            if dropped[i] || fenced[i] || !is_list_heading(lines[i]) {
                continue;
            }
            let bullets: Vec<usize> = (i + 1..lines.len())
                .take_while(|j| is_bullet(lines[*j]) || lines[*j].trim().is_empty())
                .filter(|j| is_bullet(lines[*j]))
                .collect();
            if !bullets.is_empty() && bullets.iter().all(|j| dropped[*j]) {
                dropped[i] = true;
                plan.decisions.push(ContextDecision {
                    kind: DecisionKind::Compressed,
                    source,
                    kept_in: None,
                    bytes_saved: lines[i].len() + 1,
                    preview: preview(lines[i]),
                });
            }
        }
    }

    fn compress(plan: &mut ContextPlan) {
        for item in plan.items.iter_mut().filter(|i| i.editable()) {
            let (text, changed) = compress_whitespace(&item.content);
            if changed {
                let saved = item.content.len().saturating_sub(text.len());
                plan.compressed_items += 1;
                plan.decisions.push(ContextDecision {
                    kind: DecisionKind::Compressed,
                    source: item.source,
                    kept_in: None,
                    bytes_saved: saved,
                    preview: preview(&item.content),
                });
                item.content = text;
                item.size = TextSize::of(&item.content);
            }
        }
    }

    /// Leaves out whole items below `High` until the budget is met, stale and larger ones first,
    /// each replaced by a line saying so. What cannot be left out is reported, not cut.
    fn fit(plan: &mut ContextPlan, budget: &ContextBudget) {
        let Some(available) = budget.available() else {
            return;
        };
        let total = |plan: &ContextPlan| -> u64 {
            plan.items.iter().map(ContextItem::estimated_tokens).sum()
        };
        while total(plan) > available {
            let candidate = plan
                .items
                .iter()
                .enumerate()
                .filter(|(_, item)| item.priority < Priority::High && !is_notice(item))
                .min_by_key(|(_, item)| {
                    (
                        item.priority,
                        !item.stale,
                        std::cmp::Reverse(item.size.bytes),
                    )
                })
                .map(|(i, _)| i);
            let Some(index) = candidate else {
                break;
            };
            let item = &mut plan.items[index];
            let notice = format!(
                "[Left out to fit the context budget: {} ({} estimated tokens)]",
                item.id,
                item.estimated_tokens()
            );
            plan.decisions.push(ContextDecision {
                kind: DecisionKind::Omitted,
                source: item.source,
                kept_in: None,
                bytes_saved: item.content.len().saturating_sub(notice.len()),
                preview: preview(&item.content),
            });
            item.content = notice;
            item.size = TextSize::of(&item.content);
            plan.omitted_items += 1;
        }
        let estimated = total(plan);
        if estimated > available {
            plan.over_budget = Some(BudgetOverrun {
                budget_tokens: available,
                estimated_tokens: estimated,
                required_tokens: plan
                    .items
                    .iter()
                    .filter(|i| i.priority == Priority::Required)
                    .map(ContextItem::estimated_tokens)
                    .sum(),
            });
        }
    }
}

fn is_notice(item: &ContextItem) -> bool {
    item.content
        .starts_with("[Left out to fit the context budget:")
}

/// Markers that structure the prompt itself (the handoff's start and end lines) are not content.
fn is_bullet_free_marker(line: &str) -> bool {
    let t = line.trim();
    t.starts_with("## ") || t.starts_with("END ")
}

#[cfg(test)]
mod tests;
