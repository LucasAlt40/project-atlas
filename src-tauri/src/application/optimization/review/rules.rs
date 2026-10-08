//! What the Context Review checks about rules: that what is mandatory is really in the prompt, that
//! rules do not contradict each other, that a rule did not arrive claiming a strength its origin
//! cannot have, and that none tries to give itself powers.
//!
//! The review reads what Atlas resolved (`application::rules`) and what the prompt carries; it
//! resolves nothing and decides nothing. A finding is a signal: `security/guardrails` decides what
//! it means for the execution.

use super::claims::find_claim;
use super::{excerpt, issue};
use crate::application::optimization::context::ContextItem;
use crate::application::rules::{ConflictKind, RuleConflict};
use crate::domain::guardrail::{IssueCode, IssueSeverity, ReviewIssue};
use crate::domain::optimization::SectionKind;

/// A mandatory rule the prompt must carry, as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MandatoryRule {
    /// `AppliedRule::reference`.
    pub reference: String,
    pub content: String,
}

/// What resolution decided, as far as the review needs it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuleFacts {
    pub mandatory: Vec<MandatoryRule>,
    pub conflicts: Vec<RuleConflict>,
    /// Rules whose claimed strength was lowered because their origin cannot bind.
    pub downgraded: Vec<String>,
}

pub fn check(items: &[ContextItem], facts: &RuleFacts) -> Vec<ReviewIssue> {
    let mut issues = Vec::new();
    // Integrity: nothing mandatory is missing or edited.
    for rule in &facts.mandatory {
        let id = format!("rule:{}", rule.reference);
        match items.iter().find(|item| item.id == id) {
            None => issues.push(issue(
                IssueCode::MissingRequired,
                IssueSeverity::Blocking,
                SectionKind::Rules,
                format!("the mandatory rule {} is not in the prompt", rule.reference),
            )),
            Some(item) if !item.content.contains(rule.content.as_str()) => {
                issues.push(issue(
                    IssueCode::AlteredRequired,
                    IssueSeverity::Blocking,
                    SectionKind::Rules,
                    format!(
                        "the mandatory rule {} was changed on its way to the prompt",
                        rule.reference
                    ),
                ));
            }
            Some(_) => {}
        }
    }
    // Consistency: a narrower rule against a mandatory one, or two mandatory rules disagreeing.
    for conflict in facts.conflicts.iter().filter(|c| c.kind.needs_a_person()) {
        let what = match conflict.kind {
            ConflictKind::NarrowerAgainstMandatory => {
                "a narrower rule contradicts a mandatory one, which governs"
            }
            _ => "two mandatory rules disagree, one was kept by priority",
        };
        issues.push(issue(
            IssueCode::RuleConflict,
            IssueSeverity::Error,
            SectionKind::Rules,
            format!(
                "{what} (topic {}: {} over {})",
                conflict.topic, conflict.winner, conflict.loser
            ),
        ));
    }
    // Provenance: text nobody in charge wrote is background, whatever it says it is.
    if !facts.downgraded.is_empty() {
        issues.push(issue(
            IssueCode::UnknownProvenance,
            IssueSeverity::Warning,
            SectionKind::Rules,
            format!(
                "{} rule(s) came from a source that cannot bind and were kept as background: {}",
                facts.downgraded.len(),
                facts.downgraded.join(", ")
            ),
        ));
    }
    // Injection: a rule is where authority is expected, so one that tries to claim powers the
    // hierarchy does not give (override Atlas, grant itself permission, switch the guardrails
    // off, say a person approved) is a contradiction in terms. Not rewritten, not hidden: a
    // person is asked.
    for item in items
        .iter()
        .filter(|i| i.source == SectionKind::Rules && i.id != "rules_notice")
    {
        if let Some((kind, line)) = find_claim(&item.content) {
            let mut found = issue(
                IssueCode::AuthorityClaim,
                IssueSeverity::Error,
                SectionKind::Rules,
                format!(
                    "the rule {} claims an authority a rule cannot have (it is guidance, not a permission)",
                    item.id.trim_start_matches("rule:")
                ),
            );
            found.excerpt = excerpt(&line);
            found.claim = Some(kind);
            issues.push(found);
        }
    }
    issues
}
