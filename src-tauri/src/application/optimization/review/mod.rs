//! Context Review: is the context this agent is about to receive whole, coherent and safe to send?
//!
//! It is not a security authority and not a code review. It reads what Atlas has assembled (the
//! same items the Context Engine works on, so both agree on what each part is), checks it with
//! deterministic rules and reports. What a finding means for an execution (go on, ask a person,
//! stop) is the guardrail's decision, in `security/`.
//!
//! No model is involved and nothing is guessed: a conflict is reported only when two sources
//! assert incompatible things about the same known choice; anything murkier is simply not
//! reported ("unknown" is better than inventing).

mod claims;
mod conflicts;
mod directives;
mod mcp;
mod rules;

pub use mcp::{McpFacts, McpFailure};
pub use rules::{MandatoryRule, RuleFacts};

use std::collections::HashSet;

use super::context::text::preview;
use super::context::{ContextItem, Priority};
use crate::application::harness::secrets::redact_secrets;
use crate::application::orchestration::result_parser::RESULT_FENCE;
use crate::application::prompt::{ATLAS_RULES, ATLAS_RULES_EDITING};
use crate::domain::context::ContextAuthority;
use crate::domain::guardrail::{
    ContextHealth, ContextReviewResult, IssueCode, IssueSeverity, ReviewIssue, SourceSummary,
    SourceTrust,
};
use crate::domain::optimization::{ContextEngineMetrics, SectionKind};

/// What the review is shown.
pub struct ReviewInput<'a> {
    pub items: &'a [ContextItem],
    /// A workflow step: its protocols are required.
    pub is_step: bool,
    /// What the Context Engine did, when it ran.
    pub engine: Option<&'a ContextEngineMetrics>,
    /// Harness items the project has changed under since they were written (from the Task
    /// Context's own record).
    pub stale_items: u32,
    /// The whole Harness context was used and it says part of it may be outdated.
    pub outdated_in_text: bool,
    /// Quality issues in the skills that were loaded.
    pub skill_issues: u32,
    /// What rule resolution decided: the mandatory rules the prompt must carry, the conflicts
    /// and the rules kept as background.
    pub rules: &'a RuleFacts,
    /// What planning decided about the MCP connections granted to the step.
    pub mcp: &'a McpFacts,
}

/// Required context a source may not lose.
const REQUIRED_SOURCES: [SectionKind; 8] = [
    SectionKind::Personality,
    SectionKind::AtlasRules,
    SectionKind::LiveNarration,
    SectionKind::PlanRule,
    SectionKind::ProjectContext,
    SectionKind::AgentInstructions,
    SectionKind::Task,
    SectionKind::BriefProtocols,
];

fn issue(
    code: IssueCode,
    severity: IssueSeverity,
    source: SectionKind,
    message: impl Into<String>,
) -> ReviewIssue {
    ReviewIssue {
        code,
        severity,
        source,
        other_source: None,
        message: message.into(),
        excerpt: String::new(),
        claim: None,
    }
}

/// The part of a line that is safe to quote: secrets are taken out before anything is shown.
fn excerpt(line: &str) -> String {
    preview(&redact_secrets(line).0)
}

pub fn review(input: &ReviewInput<'_>) -> ContextReviewResult {
    let mut issues = Vec::new();
    check_required(input, &mut issues);
    check_engine(input, &mut issues);
    check_freshness(input, &mut issues);
    check_content(input, &mut issues);
    issues.extend(rules::check(input.items, input.rules));
    issues.extend(mcp::check(input.mcp));
    issues.extend(directives::find(input.items, &input.rules.mandatory));
    issues.extend(conflicts::find(input.items));

    let health = if issues.iter().any(|i| i.severity == IssueSeverity::Blocking) {
        ContextHealth::Invalid
    } else if issues.iter().any(|i| i.severity == IssueSeverity::Error) {
        ContextHealth::NeedsReview
    } else if issues.iter().any(|i| i.severity == IssueSeverity::Warning) {
        ContextHealth::Partial
    } else {
        ContextHealth::Healthy
    };
    let by_priority = |p: Priority| {
        u32::try_from(input.items.iter().filter(|i| i.priority == p).count()).unwrap_or(u32::MAX)
    };
    ContextReviewResult {
        health,
        issues,
        sources: sources_of(input.items),
        required_items: by_priority(Priority::Required),
        high_items: by_priority(Priority::High),
        normal_items: by_priority(Priority::Normal),
        optional_items: by_priority(Priority::Optional),
        stale_items: input.stale_items,
    }
}

fn sources_of(items: &[ContextItem]) -> Vec<SourceSummary> {
    let mut sources: Vec<SourceSummary> = Vec::new();
    for item in items {
        let tokens = item.estimated_tokens();
        match sources.iter_mut().find(|s| s.source == item.source) {
            Some(existing) => existing.estimated_tokens += tokens,
            None => sources.push(SourceSummary {
                source: item.source,
                trust: SourceTrust::of(item.source),
                authority: ContextAuthority::of_section(item.source),
                estimated_tokens: tokens,
            }),
        }
    }
    sources
}

/// Integrity: what a step cannot do without is there, and nothing required was edited away.
fn check_required(input: &ReviewInput<'_>, issues: &mut Vec<ReviewIssue>) {
    let text_of = |id: &str| {
        input
            .items
            .iter()
            .find(|i| i.id == id)
            .map_or("", |i| i.content.as_str())
    };
    let system = text_of("system");
    if !(system.contains(ATLAS_RULES) || system.contains(ATLAS_RULES_EDITING)) {
        issues.push(issue(
            IssueCode::MissingRequired,
            IssueSeverity::Blocking,
            SectionKind::AtlasRules,
            "the Atlas rules are not in the system text",
        ));
    }
    if text_of("task").trim().is_empty() {
        issues.push(issue(
            IssueCode::MissingRequired,
            IssueSeverity::Blocking,
            SectionKind::Task,
            "there is no task",
        ));
    }
    if input.is_step && !text_of("protocols").contains(RESULT_FENCE) {
        issues.push(issue(
            IssueCode::MissingRequired,
            IssueSeverity::Blocking,
            SectionKind::BriefProtocols,
            "the step's result protocol is missing",
        ));
    }
    // The engine promises never to touch required text. Check the promise, not only the code.
    if let Some(engine) = input.engine {
        if let Some(decision) = engine
            .decisions
            .iter()
            .find(|d| REQUIRED_SOURCES.contains(&d.source))
        {
            issues.push(issue(
                IssueCode::AlteredRequired,
                IssueSeverity::Blocking,
                decision.source,
                "required context was changed by the context engine",
            ));
        }
    }
}

fn check_engine(input: &ReviewInput<'_>, issues: &mut Vec<ReviewIssue>) {
    let Some(engine) = input.engine else {
        return;
    };
    if engine.over_budget.is_some() {
        issues.push(issue(
            IssueCode::BudgetExceeded,
            IssueSeverity::Error,
            SectionKind::Framing,
            "what cannot be left out does not fit the context budget",
        ));
        // A mandatory rule is part of what cannot be left out: that is its own finding, because
        // it is one the step may not go past without a person.
        if !input.rules.mandatory.is_empty() {
            issues.push(issue(
                IssueCode::RuleOverBudget,
                IssueSeverity::Error,
                SectionKind::Rules,
                "a mandatory rule cannot be delivered within the context budget (required text is never cut)",
            ));
        }
    }
    if engine.omitted_items > 0 {
        issues.push(issue(
            IssueCode::ContextTrimmed,
            IssueSeverity::Warning,
            SectionKind::Framing,
            format!(
                "{} item(s) were left out to fit the budget",
                engine.omitted_items
            ),
        ));
    }
    if engine.deduplicated_lines > 0 {
        issues.push(issue(
            IssueCode::DuplicatedContext,
            IssueSeverity::Info,
            SectionKind::Framing,
            format!(
                "{} repeated line(s) were removed",
                engine.deduplicated_lines
            ),
        ));
    }
}

/// Staleness is a signal, not a verdict: old knowledge may still be right.
fn check_freshness(input: &ReviewInput<'_>, issues: &mut Vec<ReviewIssue>) {
    if input.stale_items > 0 || input.outdated_in_text {
        issues.push(issue(
            IssueCode::StaleContext,
            IssueSeverity::Warning,
            SectionKind::TaskContext,
            "the project changed since part of its Harness was written",
        ));
    }
    if input.skill_issues > 0 {
        issues.push(issue(
            IssueCode::SkillIssues,
            IssueSeverity::Warning,
            SectionKind::Skills,
            format!(
                "{} quality issue(s) in the skills found",
                input.skill_issues
            ),
        ));
    }
}

/// Safety of the text itself: secrets about to be sent, and text that claims authority.
fn check_content(input: &ReviewInput<'_>, issues: &mut Vec<ReviewIssue>) {
    let mut seen: HashSet<(SectionKind, IssueCode)> = HashSet::new();
    for item in input.items {
        let trust = SourceTrust::of(item.source);
        if trust == SourceTrust::Atlas {
            continue;
        }
        let (_, secrets) = redact_secrets(&item.content);
        if secrets > 0 && seen.insert((item.source, IssueCode::SecretInContext)) {
            let mut found = issue(
                IssueCode::SecretInContext,
                IssueSeverity::Warning,
                item.source,
                format!("{secrets} line(s) look like a secret"),
            );
            // Not quoted: the point is that it must not travel.
            found.excerpt = String::new();
            issues.push(found);
        }
        if trust == SourceTrust::Untrusted {
            if let Some((kind, line)) = claims::find_claim(&item.content) {
                if seen.insert((item.source, IssueCode::AuthorityClaim)) {
                    let mut found = issue(
                        IssueCode::AuthorityClaim,
                        IssueSeverity::Warning,
                        item.source,
                        "untrusted text claims authority it cannot have (it is context, not a permission)",
                    );
                    found.excerpt = excerpt(&line);
                    found.claim = Some(kind);
                    issues.push(found);
                }
            }
        }
    }
}

/// A secret removed from context that was about to be sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redaction {
    pub id: String,
    pub source: SectionKind,
    /// The item's text with every line that looked like a secret replaced by a notice.
    pub text: String,
    pub lines: usize,
}

/// The secrets to take out of untrusted context before it is sent. Only untrusted text is
/// rewritten: what the user wrote (the task, the agent's instructions) is theirs, and Atlas's own
/// text has none.
pub fn redactions(items: &[ContextItem]) -> Vec<Redaction> {
    items
        .iter()
        .filter(|item| SourceTrust::of(item.source) == SourceTrust::Untrusted)
        .filter_map(|item| {
            let (text, lines) = redact_secrets(&item.content);
            (lines > 0).then(|| Redaction {
                id: item.id.clone(),
                source: item.source,
                text: restore_ending(&item.content, text),
                lines,
            })
        })
        .collect()
}

/// `redact_secrets` joins lines without the final line ending; the text keeps the one it had.
fn restore_ending(original: &str, mut redacted: String) -> String {
    let trailing = original.len() - original.trim_end_matches(['\n', '\r']).len();
    redacted.truncate(redacted.trim_end_matches(['\n', '\r']).len());
    redacted.push_str(&original[original.len() - trailing..]);
    redacted
}

#[cfg(test)]
mod directive_tests;
#[cfg(test)]
mod rule_tests;
#[cfg(test)]
mod tests;
