//! The project's own rules: `.atlas/context/rules.md`, a file the user writes (Atlas never writes
//! it), next to the Harness's `business.md`, `constraints.md` and `decisions.md`. It is the one
//! place project rules live; the Harness's `constraints.md` stays knowledge ("what we know about
//! the project's limits") and is not copied here.
//!
//! One rule per bullet, optionally tagged:
//!
//! ```text
//! - [mandatory] All code must have unit tests
//! - [preference, topic=package-manager, priority=2] Use pnpm
//! - [informational] The billing module is legacy
//! - A bullet with no tag is a preference
//! ```
//!
//! The file travels with the repository, so its rules have the origin `ProjectFile`: believed as
//! rules, but scanned like any context. Lines that look like secrets are redacted before parsing.

use std::collections::BTreeMap;

use crate::application::harness::fingerprint::digest_text;
use crate::application::harness::secrets::redact_secrets;
use crate::domain::rules::{Rule, RuleOrigin, RuleProvenance, RuleScope, RuleStrength};

pub const PROJECT_RULES_FILE: &str = ".atlas/context/rules.md";
const TITLE_CHARS: usize = 60;

pub fn parse_project_rules(text: &str) -> Vec<Rule> {
    let (clean, _) = redact_secrets(text);
    clean.lines().filter_map(parse_line).collect()
}

fn parse_line(line: &str) -> Option<Rule> {
    let body = line
        .trim()
        .strip_prefix("- ")
        .or_else(|| line.trim().strip_prefix("* "))?
        .trim();
    let (tags, content) = match body.strip_prefix('[').and_then(|rest| rest.split_once(']')) {
        Some((tags, rest)) => (tags, rest.trim()),
        None => ("", body),
    };
    if content.is_empty() {
        return None;
    }
    let mut strength = RuleStrength::Preference;
    let mut topic = None;
    let mut priority = 0;
    for tag in tags.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        match tag.split_once('=') {
            Some(("topic", value)) => topic = Some(value.trim().to_owned()),
            Some(("priority", value)) => priority = value.trim().parse().unwrap_or(0),
            _ => match tag.to_lowercase().as_str() {
                "mandatory" => strength = RuleStrength::Mandatory,
                "preference" => strength = RuleStrength::Preference,
                "informational" => strength = RuleStrength::Informational,
                // An unknown tag is not a reason to change what the rule is.
                _ => {}
            },
        }
    }
    Some(Rule {
        // Content-derived: the same rule is the same rule on every read.
        id: format!("project-{}", &digest_text(content)[..12]),
        scope: RuleScope::Project,
        owner: None,
        title: content.chars().take(TITLE_CHARS).collect(),
        content: content.to_owned(),
        strength,
        priority,
        enabled: true,
        topic,
        provenance: RuleProvenance {
            origin: RuleOrigin::ProjectFile,
            source: PROJECT_RULES_FILE.to_owned(),
        },
        metadata: BTreeMap::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bullets_become_project_rules_with_their_tags() {
        let rules = parse_project_rules(
            "# Rules\n\nSome prose that is not a rule.\n\
             - [mandatory] All code must have unit tests\n\
             - [preference, topic=package-manager, priority=2] Use pnpm\n\
             * [informational] The billing module is legacy\n\
             - Prefer small functions\n",
        );

        let summary: Vec<_> = rules
            .iter()
            .map(|r| {
                (
                    r.strength,
                    r.topic.as_deref(),
                    r.priority,
                    r.content.as_str(),
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                (
                    RuleStrength::Mandatory,
                    None,
                    0,
                    "All code must have unit tests"
                ),
                (
                    RuleStrength::Preference,
                    Some("package-manager"),
                    2,
                    "Use pnpm"
                ),
                (
                    RuleStrength::Informational,
                    None,
                    0,
                    "The billing module is legacy"
                ),
                (RuleStrength::Preference, None, 0, "Prefer small functions"),
            ]
        );
        assert!(rules.iter().all(|r| r.scope == RuleScope::Project
            && r.provenance.origin == RuleOrigin::ProjectFile
            && r.provenance.source == PROJECT_RULES_FILE
            && r.enabled));
    }

    #[test]
    fn an_untagged_bullet_is_a_preference_never_a_mandate() {
        let rules = parse_project_rules("- Always do the thing\n- [whatever] Another\n");

        assert!(rules.iter().all(|r| r.strength == RuleStrength::Preference));
    }

    #[test]
    fn ids_follow_the_text_so_the_same_rule_is_the_same_rule() {
        let first = parse_project_rules("- [mandatory] Write tests\n");
        let again = parse_project_rules("- [mandatory] Write tests\n");
        let other = parse_project_rules("- [mandatory] Write more tests\n");

        assert_eq!(first[0].id, again[0].id);
        assert_ne!(first[0].id, other[0].id);
    }

    #[test]
    fn a_line_that_looks_like_a_secret_does_not_become_a_rule_with_the_secret_in_it() {
        let rules = parse_project_rules("- [mandatory] Deploy with password=hunter2hunter2\n");

        assert!(rules.iter().all(|r| !r.content.contains("hunter2hunter2")));
    }

    #[test]
    fn an_empty_bullet_is_not_a_rule() {
        assert_eq!(parse_project_rules("- \n- [mandatory]\n"), []);
    }
}
