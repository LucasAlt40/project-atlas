//! Rules enter the existing pipeline as context items, and the engine treats them by strength.

use super::engine::ContextEngine;
use super::{prompt_items, ContextItem, Priority};
use crate::application::prompt::Prompt;
use crate::application::rules::{resolve, RuleBlock, RuleContext};
use crate::domain::context::ContextAuthority;
use crate::domain::optimization::SectionKind;
use crate::domain::rules::tests::rule;
use crate::domain::rules::{RuleOrigin, RuleScope, RuleStrength};

fn blocks(rules: &[crate::domain::rules::Rule]) -> Vec<RuleBlock> {
    let ctx = RuleContext {
        workspace: "w",
        workflow: None,
        agent: "a",
    };
    resolve(rules, &ctx)
        .applied
        .iter()
        .map(RuleBlock::of)
        .collect()
}

fn prompt() -> Prompt {
    Prompt {
        system: "SYS".to_owned(),
        harness: None,
        task_aware: false,
        skills: None,
        rules: Some("rules text".to_owned()),
        context: "CTX".to_owned(),
        instruction: "Task:\nDo it".to_owned(),
    }
}

fn items_for(rules: &[crate::domain::rules::Rule]) -> Vec<ContextItem> {
    prompt_items(&prompt(), "", "Do it", None, &[], &blocks(rules)).items
}

fn filler(seed: &str) -> String {
    (0..80)
        .map(|i| format!("{seed}{i}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn each_rule_is_a_context_item_with_the_priority_its_strength_deserves() {
    let items = items_for(&[
        rule("m", RuleScope::Global, RuleStrength::Mandatory, "must"),
        rule("p", RuleScope::Global, RuleStrength::Preference, "should"),
        rule(
            "i",
            RuleScope::Global,
            RuleStrength::Informational,
            "background",
        ),
    ]);

    let priority = |id: &str| items.iter().find(|i| i.id == id).unwrap().priority;
    assert_eq!(priority("rules_notice"), Priority::Required);
    assert_eq!(priority("rule:global.m"), Priority::Required);
    assert_eq!(priority("rule:global.p"), Priority::Normal);
    assert_eq!(priority("rule:global.i"), Priority::Optional);
    assert_eq!(
        items
            .iter()
            .filter(|i| i.source == SectionKind::Rules)
            .count(),
        4
    );
}

#[test]
fn an_item_carries_where_the_rule_came_from_and_what_it_may_do() {
    let mut external = rule(
        "e",
        RuleScope::Project,
        RuleStrength::Mandatory,
        "from outside",
    );
    external.provenance.origin = RuleOrigin::External;
    external.provenance.source = "https://example.test/rules".to_owned();
    let items = items_for(&[
        rule("m", RuleScope::Global, RuleStrength::Mandatory, "must"),
        external,
    ]);

    let find = |id: &str| items.iter().find(|i| i.id == id).unwrap();
    assert_eq!(
        find("rule:global.m").authority,
        ContextAuthority::Authoritative
    );
    assert_eq!(find("rule:global.m").provenance, "rule:user:config");
    // Text from outside is background, whatever strength it claims.
    assert_eq!(
        find("rule:project.e").authority,
        ContextAuthority::Informational
    );
    assert_eq!(find("rule:project.e").priority, Priority::Optional);
    assert!(find("rule:project.e").provenance.contains("external"));
    assert_eq!(
        find("rules_notice").authority,
        ContextAuthority::Authoritative
    );
}

#[test]
fn without_rules_there_are_no_rule_items() {
    assert!(items_for(&[])
        .iter()
        .all(|i| i.source != SectionKind::Rules));
}

#[test]
fn a_budget_leaves_out_background_then_preferences_and_never_a_mandatory_rule() {
    let (m, p, i) = (filler("m"), filler("p"), filler("i"));
    let items = items_for(&[
        rule("m", RuleScope::Global, RuleStrength::Mandatory, &m),
        rule("p", RuleScope::Global, RuleStrength::Preference, &p),
        rule("i", RuleScope::Global, RuleStrength::Informational, &i),
    ]);

    // Room for the required part and one more rule's worth: the background goes first.
    let required: u64 = items
        .iter()
        .filter(|item| item.priority == Priority::Required)
        .map(ContextItem::estimated_tokens)
        .sum();
    let one_more = items
        .iter()
        .find(|i| i.id == "rule:global.p")
        .unwrap()
        .estimated_tokens();
    let plan = ContextEngine::plan(items.clone(), Some(required + one_more + 40));
    let text = |id: &str| {
        plan.items
            .iter()
            .find(|i| i.id == id)
            .unwrap()
            .content
            .clone()
    };
    assert!(text("rule:global.i").starts_with("[Left out"));
    assert!(text("rule:global.p").contains(&p));
    assert!(text("rule:global.m").contains(&m));

    // Room for nothing: both optional ones go, the mandatory rule stays whole, and the engine
    // says the required part does not fit instead of cutting it.
    let tight = ContextEngine::plan(items, Some(1));
    let text = |id: &str| {
        tight
            .items
            .iter()
            .find(|i| i.id == id)
            .unwrap()
            .content
            .clone()
    };
    assert!(text("rule:global.p").starts_with("[Left out"));
    assert!(text("rule:global.i").starts_with("[Left out"));
    assert!(text("rule:global.m").contains(&m));
    assert!(tight.over_budget.is_some());
}

#[test]
fn a_mandatory_rule_is_not_edited_even_when_it_repeats_what_the_prompt_says() {
    let repeated = "Never write outside your working directory under any condition";
    let items = items_for(&[rule(
        "m",
        RuleScope::Global,
        RuleStrength::Mandatory,
        repeated,
    )]);

    let plan = ContextEngine::plan(items, None);

    let kept = plan.items.iter().find(|i| i.id == "rule:global.m").unwrap();
    assert!(kept.content.contains(repeated));
}
