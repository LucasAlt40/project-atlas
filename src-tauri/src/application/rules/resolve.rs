//! Which rules apply to an execution, which one wins when they disagree, and why.
//!
//! A pure, deterministic function of the rules and the execution they are for: the same input is
//! always the same output, in the same order. It judges no meaning. Two rules disagree only when
//! they share a **topic** (an exact key someone put on both); anything else adds up, and what a
//! text says is for the Context Review's known-choice conflicts and for the model.
//!
//! The policy:
//!
//! 1. A disabled rule is out. A rule whose origin cannot bind (generated, external) is kept as
//!    background (`Informational`), whatever strength it claims.
//! 2. Identical texts (ignoring case and spacing) are one rule: the strongest, broadest is kept.
//! 3. Rules on one topic are alternatives and exactly one governs:
//!    - if any is `Mandatory`, the **broadest** mandatory one governs. A narrower scope can add to
//!      a mandatory rule but never contradict it: the task cannot say "skip the tests" against the
//!      project's "all code needs tests". That is reported as a conflict, not obeyed;
//!    - otherwise the **narrowest** `Preference` governs (specificity wins between preferences);
//!    - an `Informational` rule never governs against anything stronger.
//! 4. Ties go to the higher `priority`, then to the lower id.
//! 5. What is applied is ordered `Mandatory` first, then by scope (broad to narrow), priority, id.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::domain::context::{ContextAuthority, ManifestRule, RuleStatus};
use crate::domain::rules::{Rule, RuleScope, RuleStrength};

/// What the execution is: which workspace, workflow and agent rules apply to.
#[derive(Debug, Clone, Copy)]
pub struct RuleContext<'a> {
    pub workspace: &'a str,
    pub workflow: Option<&'a str>,
    pub agent: &'a str,
}

/// A rule that goes to the agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedRule {
    pub rule: Rule,
    /// What it counts as: its strength, unless its origin cannot bind.
    pub strength: RuleStrength,
    /// Its origin could not bind and the strength it claimed was lowered to background.
    pub downgraded: bool,
}

impl AppliedRule {
    /// A name unique across scopes, used as the context item's id.
    pub fn reference(&self) -> String {
        reference(&self.rule)
    }
}

pub fn reference(rule: &Rule) -> String {
    format!("{:?}.{}", rule.scope, rule.id).to_lowercase()
}

/// Why a rule is not applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Exclusion {
    Disabled,
    /// The same text is already applied under another rule.
    #[serde(rename_all = "camelCase")]
    Duplicate {
        of: String,
    },
    /// Another rule on the same topic governs.
    #[serde(rename_all = "camelCase")]
    Overridden {
        by: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcludedRule {
    pub rule: Rule,
    pub reason: Exclusion,
}

/// What kind of disagreement was settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictKind {
    /// A narrower rule contradicts a broader mandatory one. The mandatory rule governs.
    NarrowerAgainstMandatory,
    /// Two mandatory rules of the same scope disagree: one is kept by priority, and a person
    /// should look.
    MandatoryAgainstMandatory,
    /// A routine settling between preferences or against background: nothing to worry about.
    Superseded,
}

impl ConflictKind {
    /// Whether a person should know before the agent starts.
    pub const fn needs_a_person(self) -> bool {
        matches!(
            self,
            Self::NarrowerAgainstMandatory | Self::MandatoryAgainstMandatory
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleConflict {
    pub kind: ConflictKind,
    pub topic: String,
    pub winner: String,
    pub loser: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Resolution {
    pub applied: Vec<AppliedRule>,
    pub excluded: Vec<ExcludedRule>,
    pub conflicts: Vec<RuleConflict>,
}

impl Resolution {
    pub fn count(&self, strength: RuleStrength) -> usize {
        self.applied
            .iter()
            .filter(|a| a.strength == strength)
            .count()
    }

    /// Every rule that applied to the execution, and what became of it, for the Context Manifest.
    /// `omitted` are the references of applied rules the Context Engine left out for the budget.
    pub fn records(&self, omitted: &BTreeSet<String>) -> Vec<ManifestRule> {
        let in_conflict: BTreeSet<&str> = self
            .conflicts
            .iter()
            .flat_map(|c| [c.winner.as_str(), c.loser.as_str()])
            .collect();
        let record = |rule: &Rule,
                      strength: RuleStrength,
                      status: RuleStatus,
                      by: Option<String>,
                      downgraded: bool| ManifestRule {
            reference: reference(rule),
            title: rule.title.clone(),
            scope: rule.scope,
            strength,
            authority: ContextAuthority::of_rule(rule),
            origin: rule.provenance.origin,
            source: rule.provenance.source.clone(),
            status,
            by,
            in_conflict: in_conflict.contains(reference(rule).as_str()),
            downgraded,
        };
        let mut records: Vec<ManifestRule> = self
            .applied
            .iter()
            .map(|a| {
                let status = if omitted.contains(&a.reference()) {
                    RuleStatus::OmittedForBudget
                } else {
                    RuleStatus::Applied
                };
                record(&a.rule, a.strength, status, None, a.downgraded)
            })
            .collect();
        records.extend(self.excluded.iter().map(|e| {
            let (status, by) = match &e.reason {
                Exclusion::Disabled => (RuleStatus::Disabled, None),
                Exclusion::Duplicate { of } => (RuleStatus::Duplicate, Some(of.clone())),
                Exclusion::Overridden { by } => (RuleStatus::Overridden, Some(by.clone())),
            };
            record(&e.rule, e.rule.effective_strength(), status, by, false)
        }));
        records
    }

    /// Everything about the resolution an approval must not outlive, as one stable text: which
    /// rules apply, at what strength, from where, which were left out and why, and which
    /// conflicts were settled. (The rules' own text is covered by their context items.)
    pub fn canonical(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        for applied in &self.applied {
            lines.push(format!(
                "applied|{}|{:?}|{:?}|{}|{}",
                applied.reference(),
                applied.strength,
                applied.rule.provenance.origin,
                applied.rule.provenance.source,
                applied.downgraded
            ));
        }
        for excluded in &self.excluded {
            lines.push(format!(
                "excluded|{}|{:?}",
                reference(&excluded.rule),
                excluded.reason
            ));
        }
        for conflict in &self.conflicts {
            lines.push(format!(
                "conflict|{:?}|{}|{}|{}",
                conflict.kind, conflict.topic, conflict.winner, conflict.loser
            ));
        }
        lines.join("\n")
    }
}

fn applies(rule: &Rule, ctx: &RuleContext<'_>) -> bool {
    let owner = rule.owner.as_deref();
    match rule.scope {
        // These arrive with the execution they belong to.
        RuleScope::Global | RuleScope::Project | RuleScope::Task => true,
        RuleScope::Workspace => owner == Some(ctx.workspace),
        RuleScope::Workflow => ctx.workflow.is_some() && owner == ctx.workflow,
        RuleScope::Agent => owner == Some(ctx.agent),
    }
}

fn normalized(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// The order rules are applied in (and the order ties are broken in for duplicates).
fn applied_order(a: &AppliedRule, b: &AppliedRule) -> std::cmp::Ordering {
    a.strength
        .cmp(&b.strength)
        .then(a.rule.scope.cmp(&b.rule.scope))
        .then(b.rule.priority.cmp(&a.rule.priority))
        .then(a.rule.id.cmp(&b.rule.id))
        .then(a.rule.owner.cmp(&b.rule.owner))
}

/// The rule that governs a topic.
fn governing<'a>(group: &[&'a AppliedRule]) -> &'a AppliedRule {
    let tie = |a: &&AppliedRule, b: &&AppliedRule| {
        b.rule
            .priority
            .cmp(&a.rule.priority)
            .then(a.rule.id.cmp(&b.rule.id))
    };
    let of = |strength: RuleStrength| {
        group
            .iter()
            .copied()
            .filter(move |a| a.strength == strength)
    };
    if let Some(best) = of(RuleStrength::Mandatory)
        .min_by(|a, b| a.rule.scope.cmp(&b.rule.scope).then_with(|| tie(a, b)))
    {
        return best;
    }
    if let Some(best) = of(RuleStrength::Preference)
        .min_by(|a, b| b.rule.scope.cmp(&a.rule.scope).then_with(|| tie(a, b)))
    {
        return best;
    }
    group
        .iter()
        .copied()
        .min_by(|a, b| b.rule.scope.cmp(&a.rule.scope).then_with(|| tie(a, b)))
        .expect("a topic group is never empty")
}

pub fn resolve(rules: &[Rule], ctx: &RuleContext<'_>) -> Resolution {
    let mut resolution = Resolution::default();
    let mut candidates: Vec<AppliedRule> = Vec::new();
    for rule in rules.iter().filter(|r| applies(r, ctx)) {
        if rule.enabled {
            let strength = rule.effective_strength();
            candidates.push(AppliedRule {
                rule: rule.clone(),
                strength,
                downgraded: strength != rule.strength,
            });
        } else {
            resolution.excluded.push(ExcludedRule {
                rule: rule.clone(),
                reason: Exclusion::Disabled,
            });
        }
    }
    candidates.sort_by(applied_order);

    // Identical text is one rule: the first in the order above (strongest, broadest) stays.
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    let mut unique: Vec<AppliedRule> = Vec::new();
    for candidate in candidates {
        let key = normalized(&candidate.rule.content);
        if let Some(kept) = seen.get(&key) {
            resolution.excluded.push(ExcludedRule {
                rule: candidate.rule,
                reason: Exclusion::Duplicate { of: kept.clone() },
            });
        } else {
            seen.insert(key, candidate.reference());
            unique.push(candidate);
        }
    }

    // Rules on one topic are alternatives.
    let mut topics: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, candidate) in unique.iter().enumerate() {
        if let Some(topic) = candidate.rule.topic_key() {
            topics.entry(topic).or_default().push(index);
        }
    }
    let mut overridden: BTreeMap<usize, String> = BTreeMap::new();
    for (topic, members) in &topics {
        if members.len() < 2 {
            continue;
        }
        let group: Vec<&AppliedRule> = members.iter().map(|i| &unique[*i]).collect();
        let winner = governing(&group);
        for (index, loser) in members.iter().zip(&group) {
            if std::ptr::eq(*loser, winner) {
                continue;
            }
            let kind = match (winner.strength, loser.strength) {
                (RuleStrength::Mandatory, RuleStrength::Mandatory)
                    if loser.rule.scope == winner.rule.scope =>
                {
                    ConflictKind::MandatoryAgainstMandatory
                }
                (RuleStrength::Mandatory, RuleStrength::Mandatory | RuleStrength::Preference)
                    if loser.rule.scope > winner.rule.scope =>
                {
                    ConflictKind::NarrowerAgainstMandatory
                }
                _ => ConflictKind::Superseded,
            };
            resolution.conflicts.push(RuleConflict {
                kind,
                topic: topic.clone(),
                winner: winner.reference(),
                loser: loser.reference(),
            });
            overridden.insert(*index, winner.reference());
        }
    }
    for (index, candidate) in unique.into_iter().enumerate() {
        match overridden.remove(&index) {
            Some(by) => resolution.excluded.push(ExcludedRule {
                rule: candidate.rule,
                reason: Exclusion::Overridden { by },
            }),
            None => resolution.applied.push(candidate),
        }
    }
    resolution
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::rules::tests::rule;
    use crate::domain::rules::{RuleOrigin, RuleScope::*, RuleStrength::*};

    const CTX: RuleContext<'static> = RuleContext {
        workspace: "w1",
        workflow: Some("wf1"),
        agent: "a1",
    };

    fn owned(mut r: Rule, owner: &str) -> Rule {
        r.owner = Some(owner.to_owned());
        r
    }

    fn topic(mut r: Rule, topic: &str) -> Rule {
        r.topic = Some(topic.to_owned());
        r
    }

    fn ids(resolution: &Resolution) -> Vec<String> {
        resolution
            .applied
            .iter()
            .map(AppliedRule::reference)
            .collect()
    }

    #[test]
    fn only_the_rules_of_this_execution_apply() {
        let rules = vec![
            rule("g", Global, Preference, "global"),
            owned(rule("w-mine", Workspace, Preference, "mine"), "w1"),
            owned(
                rule("w-other", Workspace, Preference, "other workspace"),
                "w2",
            ),
            owned(rule("f-mine", Workflow, Preference, "flow"), "wf1"),
            owned(rule("f-other", Workflow, Preference, "other flow"), "wf2"),
            owned(rule("a-mine", Agent, Preference, "agent"), "a1"),
            owned(rule("a-other", Agent, Preference, "other agent"), "a2"),
            rule("t", Task, Preference, "task"),
        ];

        let resolution = resolve(&rules, &CTX);

        assert_eq!(
            ids(&resolution),
            [
                "global.g",
                "workspace.w-mine",
                "workflow.f-mine",
                "agent.a-mine",
                "task.t"
            ]
        );
        // What belongs elsewhere is not "excluded": it never applied.
        assert_eq!(resolution.excluded, []);
    }

    #[test]
    fn a_workflow_rule_does_not_apply_outside_a_workflow() {
        let rules = vec![owned(rule("f", Workflow, Preference, "flow"), "wf1")];
        let outside = RuleContext {
            workflow: None,
            ..CTX
        };

        assert_eq!(resolve(&rules, &outside).applied, []);
    }

    #[test]
    fn a_disabled_rule_is_listed_as_excluded_and_not_applied() {
        let mut off = rule("off", Global, Mandatory, "Never deploy on Friday");
        off.enabled = false;

        let resolution = resolve(&[off], &CTX);

        assert_eq!(resolution.applied, []);
        assert_eq!(resolution.excluded[0].reason, Exclusion::Disabled);
    }

    #[test]
    fn mandatory_comes_first_then_broad_to_narrow_then_priority_then_id() {
        let mut high = rule("b", Project, Preference, "high priority");
        high.priority = 5;
        let rules = vec![
            rule("z", Task, Preference, "task pref"),
            rule("a", Project, Preference, "project pref"),
            high,
            rule("m", Task, Mandatory, "task mandatory"),
            rule("i", Global, Informational, "background"),
            rule("g", Global, Mandatory, "global mandatory"),
        ];

        assert_eq!(
            ids(&resolve(&rules, &CTX)),
            [
                "global.g",
                "task.m",
                "project.b",
                "project.a",
                "task.z",
                "global.i"
            ]
        );
    }

    #[test]
    fn resolution_is_deterministic_whatever_the_input_order() {
        let rules = vec![
            topic(
                rule("a", Project, Mandatory, "Every change has tests"),
                "tests",
            ),
            topic(rule("b", Task, Preference, "Skip tests"), "tests"),
            rule("c", Global, Preference, "Be brief"),
            owned(rule("d", Agent, Informational, "Background"), "a1"),
            rule("e", Project, Preference, "be   BRIEF"),
        ];
        let forward = resolve(&rules, &CTX);

        let mut reversed = rules.clone();
        reversed.reverse();
        let backward = resolve(&reversed, &CTX);

        assert_eq!(forward, backward);
    }

    #[test]
    fn the_same_text_is_one_rule_and_the_strongest_broadest_is_kept() {
        let rules = vec![
            rule("task", Task, Preference, "Use  small commits"),
            rule("global", Global, Preference, "use small commits"),
        ];

        let resolution = resolve(&rules, &CTX);

        assert_eq!(ids(&resolution), ["global.global"]);
        assert_eq!(
            resolution.excluded[0].reason,
            Exclusion::Duplicate {
                of: "global.global".to_owned()
            }
        );
    }

    #[test]
    fn a_narrower_rule_cannot_relax_a_mandatory_one() {
        let rules = vec![
            topic(
                rule("tests", Project, Mandatory, "All code must have tests"),
                "testing",
            ),
            topic(
                rule("skip", Task, Preference, "Ignore the tests"),
                "testing",
            ),
        ];

        let resolution = resolve(&rules, &CTX);

        assert_eq!(ids(&resolution), ["project.tests"]);
        assert_eq!(
            resolution.excluded[0].reason,
            Exclusion::Overridden {
                by: "project.tests".to_owned()
            }
        );
        assert_eq!(
            resolution.conflicts[0].kind,
            ConflictKind::NarrowerAgainstMandatory
        );
        assert!(resolution.conflicts[0].kind.needs_a_person());
        assert_eq!(resolution.conflicts[0].loser, "task.skip");
    }

    #[test]
    fn even_a_narrower_mandatory_rule_cannot_relax_a_broader_mandatory_one() {
        let rules = vec![
            topic(
                rule("tests", Project, Mandatory, "All code must have tests"),
                "testing",
            ),
            topic(
                rule("skip", Task, Mandatory, "No tests for this fix"),
                "testing",
            ),
        ];

        let resolution = resolve(&rules, &CTX);

        assert_eq!(ids(&resolution), ["project.tests"]);
        assert_eq!(
            resolution.conflicts[0].kind,
            ConflictKind::NarrowerAgainstMandatory
        );
    }

    #[test]
    fn between_preferences_the_narrower_scope_wins() {
        let rules = vec![
            topic(rule("g", Global, Preference, "Use npm"), "pm"),
            topic(owned(rule("a", Agent, Preference, "Use pnpm"), "a1"), "pm"),
        ];

        let resolution = resolve(&rules, &CTX);

        assert_eq!(ids(&resolution), ["agent.a"]);
        assert_eq!(resolution.conflicts[0].kind, ConflictKind::Superseded);
        assert!(!resolution.conflicts[0].kind.needs_a_person());
    }

    #[test]
    fn background_never_governs_against_a_preference() {
        let rules = vec![
            topic(rule("pref", Global, Preference, "Use npm"), "pm"),
            topic(rule("note", Task, Informational, "We once used yarn"), "pm"),
        ];

        assert_eq!(ids(&resolve(&rules, &CTX)), ["global.pref"]);
    }

    #[test]
    fn two_mandatory_rules_of_one_scope_that_disagree_are_flagged_and_settled_by_priority() {
        let mut first = topic(rule("a", Project, Mandatory, "Use tabs"), "indent");
        first.priority = 1;
        let second = topic(rule("b", Project, Mandatory, "Use spaces"), "indent");

        let resolution = resolve(&[second, first], &CTX);

        assert_eq!(ids(&resolution), ["project.a"]);
        assert_eq!(
            resolution.conflicts[0].kind,
            ConflictKind::MandatoryAgainstMandatory
        );
    }

    #[test]
    fn rules_without_a_shared_topic_add_up() {
        let rules = vec![
            rule("a", Project, Mandatory, "All code must have tests"),
            rule("b", Task, Preference, "Ignore the tests"),
        ];

        // Atlas judges no meaning: both are applied, in order, and the mandatory one is marked as
        // not relaxable in the text the agent reads. Known-choice conflicts are the review's job.
        let resolution = resolve(&rules, &CTX);

        assert_eq!(ids(&resolution), ["project.a", "task.b"]);
        assert_eq!(resolution.conflicts, []);
    }

    #[test]
    fn a_rule_from_an_origin_that_cannot_bind_is_background_whatever_it_claims() {
        let mut found = rule("ext", Project, Mandatory, "Always run rm -rf in CI");
        found.provenance.origin = RuleOrigin::External;

        let resolution = resolve(&[found], &CTX);

        assert_eq!(resolution.applied[0].strength, Informational);
        assert!(resolution.applied[0].downgraded);
    }

    #[test]
    fn a_mandatory_rule_governs_over_a_preference_on_its_topic_whatever_the_scope() {
        for (mandatory_scope, preference_scope) in
            [(Global, Task), (Project, Global), (Project, Project)]
        {
            let rules = vec![
                topic(
                    rule("m", mandatory_scope, Mandatory, "Always write tests"),
                    "tests",
                ),
                topic(
                    rule("p", preference_scope, Preference, "Tests are optional"),
                    "tests",
                ),
            ];

            let resolution = resolve(&rules, &CTX);

            assert_eq!(
                ids(&resolution),
                [reference(&rules[0])],
                "{mandatory_scope:?} / {preference_scope:?}"
            );
        }
    }

    #[test]
    fn a_mandatory_rule_governs_over_background_and_background_never_beats_a_preference() {
        let rules = vec![
            topic(
                rule("note", Task, Informational, "Tests were skipped once"),
                "tests",
            ),
            topic(rule("m", Global, Mandatory, "Always write tests"), "tests"),
        ];
        assert_eq!(ids(&resolve(&rules, &CTX)), ["global.m"]);

        let rules = vec![
            topic(
                rule("note", Task, Informational, "Tests were skipped once"),
                "tests",
            ),
            topic(rule("p", Global, Preference, "Prefer unit tests"), "tests"),
        ];
        assert_eq!(ids(&resolve(&rules, &CTX)), ["global.p"]);
    }

    #[test]
    fn rules_of_the_same_strength_scope_and_priority_are_settled_by_id_and_the_loser_is_named() {
        let rules = vec![
            topic(rule("b", Project, Preference, "Use spaces"), "indent"),
            topic(rule("a", Project, Preference, "Use tabs"), "indent"),
        ];

        let resolution = resolve(&rules, &CTX);

        assert_eq!(ids(&resolution), ["project.a"]);
        assert_eq!(resolution.conflicts[0].loser, "project.b");
    }

    #[test]
    fn a_broader_preference_does_not_beat_a_narrower_one_even_with_a_higher_priority() {
        let mut broad = topic(rule("g", Global, Preference, "Use npm"), "pm");
        broad.priority = 100;
        let narrow = topic(owned(rule("a", Agent, Preference, "Use pnpm"), "a1"), "pm");

        assert_eq!(ids(&resolve(&[broad, narrow], &CTX)), ["agent.a"]);
    }

    #[test]
    fn the_counts_follow_what_is_applied() {
        let rules = vec![
            rule("a", Project, Mandatory, "one"),
            rule("b", Project, Preference, "two"),
            rule("c", Project, Preference, "three"),
        ];

        let resolution = resolve(&rules, &CTX);

        assert_eq!(resolution.count(Mandatory), 1);
        assert_eq!(resolution.count(Preference), 2);
        assert_eq!(resolution.count(Informational), 0);
    }
}
