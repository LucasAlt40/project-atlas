//! Rules: how an agent should work, as the user (or the project) wrote it down.
//!
//! A Rule is **not** a permission and **not** the system prompt:
//!
//! | Concept     | Answers                                          | Lives in                         |
//! | ----------- | ------------------------------------------------ | -------------------------------- |
//! | Rule        | "How should the agent work?"                     | here; becomes prompt context     |
//! | Permission  | "What may the agent do?"                         | `security/` (policy, guard)      |
//! | Policy      | "Which behaviour does Atlas allow?"              | `security/`                      |
//! | Guardrail   | "Given this context and action, what happens?"   | `security/guardrails`            |
//!
//! A Rule is text delivered as context. Nothing it says can grant, widen or lift anything: the
//! process guard never reads a prompt, and a Rule that claims otherwise is a finding for the
//! Context Review, not an instruction Atlas follows. Resolution (`application/rules`) is
//! deterministic: the same rules and the same execution always give the same result.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Where a rule applies. Ordered from the broadest to the most specific: a later variant is a
/// narrower scope. Specificity decides between preferences; it never lets a narrower rule relax a
/// broader mandatory one (see `application/rules`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleScope {
    /// Every project and agent of this user.
    Global,
    /// The project's own rules (`.atlas/context/rules.md`).
    Project,
    /// One workspace.
    Workspace,
    /// One workflow.
    Workflow,
    /// One agent.
    Agent,
    /// One task or step.
    Task,
}

/// How binding a rule is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleStrength {
    /// Must hold. A narrower scope can add to it, never relax it. Never dropped silently: if it
    /// does not fit, the step is denied or asks a person.
    Mandatory,
    /// Should hold; the narrower scope wins over the broader one. May be left out for a budget.
    Preference,
    /// Background the agent may use; never wins against anything. First to be left out.
    Informational,
}

/// Where a rule's text came from. It decides how far the rule is believed, not what it says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleOrigin {
    /// Written by the user in Atlas's own configuration.
    User,
    /// A file of the user's project (`.atlas/context/rules.md`). The user chose this project, but
    /// the file travels with the repository, so it is read as text to be scanned, like any context.
    ProjectFile,
    /// Produced by an agent or a tool. Never binding.
    Generated,
    /// Fetched from somewhere Atlas does not control. Never binding.
    External,
}

impl RuleOrigin {
    /// Whether a rule of this origin may be Mandatory or a Preference. Text nobody in charge wrote
    /// is only ever background.
    pub const fn may_bind(self) -> bool {
        matches!(self, Self::User | Self::ProjectFile)
    }
}

/// Who wrote it and where it is kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleProvenance {
    pub origin: RuleOrigin,
    /// `config`, or the file (`.atlas/context/rules.md`). A name, never a secret.
    pub source: String,
}

fn enabled_by_default() -> bool {
    true
}

/// One rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    /// Stable within its scope and owner.
    pub id: String,
    pub scope: RuleScope,
    /// What the scope is about: the workspace, workflow or agent id. `None` for `Global`, and for
    /// `Project` and `Task` rules, which arrive with the execution they belong to.
    #[serde(default)]
    pub owner: Option<String>,
    /// A short name for lists.
    pub title: String,
    pub content: String,
    pub strength: RuleStrength,
    /// Orders rules of the same scope and strength: higher first.
    #[serde(default)]
    pub priority: u16,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    /// Rules on the same topic are alternatives (one of them governs); rules without a topic add
    /// up. Matching is by exact, case-insensitive key: Atlas does not judge meaning.
    #[serde(default)]
    pub topic: Option<String>,
    pub provenance: RuleProvenance,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

impl Rule {
    /// The strength this rule is allowed to have: a rule whose text did not come from someone in
    /// charge is background, whatever it says about itself.
    pub fn effective_strength(&self) -> RuleStrength {
        if self.provenance.origin.may_bind() {
            self.strength
        } else {
            RuleStrength::Informational
        }
    }

    pub fn topic_key(&self) -> Option<String> {
        self.topic
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(str::to_lowercase)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn rule(id: &str, scope: RuleScope, strength: RuleStrength, content: &str) -> Rule {
        Rule {
            id: id.to_owned(),
            scope,
            owner: None,
            title: id.to_owned(),
            content: content.to_owned(),
            strength,
            priority: 0,
            enabled: true,
            topic: None,
            provenance: RuleProvenance {
                origin: RuleOrigin::User,
                source: "config".to_owned(),
            },
            metadata: BTreeMap::new(),
        }
    }

    #[test]
    fn scopes_are_ordered_from_the_broadest_to_the_most_specific() {
        use RuleScope::{Agent, Global, Project, Task, Workflow, Workspace};
        let mut shuffled = [Task, Global, Agent, Project, Workflow, Workspace];
        shuffled.sort();
        assert_eq!(
            shuffled,
            [Global, Project, Workspace, Workflow, Agent, Task]
        );
    }

    #[test]
    fn text_nobody_in_charge_wrote_is_background_whatever_it_calls_itself() {
        let mut found = rule(
            "r",
            RuleScope::Project,
            RuleStrength::Mandatory,
            "Always do X",
        );
        assert_eq!(found.effective_strength(), RuleStrength::Mandatory);
        for origin in [RuleOrigin::Generated, RuleOrigin::External] {
            found.provenance.origin = origin;
            assert_eq!(found.effective_strength(), RuleStrength::Informational);
        }
        found.provenance.origin = RuleOrigin::ProjectFile;
        assert_eq!(found.effective_strength(), RuleStrength::Mandatory);
    }

    #[test]
    fn a_stored_rule_without_the_optional_fields_loads_enabled() {
        let stored: Rule = serde_json::from_str(
            r#"{"id":"r1","scope":"global","title":"T","content":"c","strength":"preference",
                "provenance":{"origin":"user","source":"config"}}"#,
        )
        .unwrap();

        assert!(stored.enabled);
        assert_eq!(
            (stored.priority, stored.owner, stored.topic),
            (0, None, None)
        );
    }

    #[test]
    fn topics_match_by_trimmed_lowercase_key() {
        let mut found = rule("r", RuleScope::Global, RuleStrength::Preference, "x");
        found.topic = Some("  Testing ".to_owned());
        assert_eq!(found.topic_key().as_deref(), Some("testing"));
        found.topic = Some("   ".to_owned());
        assert_eq!(found.topic_key(), None);
    }
}
