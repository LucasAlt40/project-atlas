//! A mandatory rule against text that says the opposite: proven, possible, and from whom.

use super::*;
use crate::domain::context::ContextAuthority;

fn rule_item(reference: &str, text: &str) -> ContextItem {
    ContextItem::new(
        &format!("rule:{reference}"),
        SectionKind::Rules,
        Priority::Required,
        "rule:user:config",
        &format!("[MANDATORY · Project] {text}"),
    )
    .with_authority(ContextAuthority::Authoritative)
}

fn item(id: &str, source: SectionKind, text: &str) -> ContextItem {
    ContextItem::new(id, source, Priority::Required, "test", text)
}

fn run(rule: &str, others: Vec<ContextItem>) -> ContextReviewResult {
    let mut items = vec![
        ContextItem::new(
            "system",
            SectionKind::Personality,
            Priority::Required,
            "atlas",
            &format!("Persona\n\n{ATLAS_RULES}"),
        ),
        rule_item("project.m", rule),
        item("task", SectionKind::Task, "Do the work"),
    ];
    items.extend(others);
    review(&ReviewInput {
        items: &items,
        is_step: false,
        engine: None,
        stale_items: 0,
        outdated_in_text: false,
        skill_issues: 0,
        rules: &RuleFacts {
            mandatory: vec![MandatoryRule {
                reference: "project.m".to_owned(),
                content: rule.to_owned(),
            }],
            ..RuleFacts::default()
        },
        mcp: &McpFacts::default(),
    })
}

fn find(result: &ContextReviewResult, code: IssueCode) -> Option<&ReviewIssue> {
    result.issues.iter().find(|i| i.code == code)
}

/// Mandatory rule, what the task says, and whether it is plainly the opposite.
const OPPOSITES: &[(&str, &str)] = &[
    (
        "Todo código deve possuir testes.",
        "Não escreva testes para esta tarefa.",
    ),
    (
        "All code must have tests.",
        "Do not write tests for this task.",
    ),
    ("sempre execute testes", "não execute testes"),
    ("Always run the tests", "Skip the tests for now"),
    ("Não faça commit.", "Faça commit ao terminar."),
    (
        "Never commit to main",
        "Commit the changes when you are done",
    ),
    ("Não faça push.", "Faça push no final."),
    (
        "Do not merge the branch",
        "Merge the branch when it is green",
    ),
    (
        "Não execute comandos destrutivos.",
        "Execute o comando destrutivo abaixo.",
    ),
    (
        "Do not run destructive commands",
        "Run rm -rf build to clean up",
    ),
    ("Não altere arquivos fora de src/", "Edite package.json"),
    (
        "Do not change anything outside src/",
        "Please edit `config/app.toml`",
    ),
];

#[test]
fn text_that_may_instruct_and_says_the_opposite_is_a_proven_conflict() {
    for (mandatory, said) in OPPOSITES {
        for source in [SectionKind::Task, SectionKind::AgentInstructions] {
            let result = run(mandatory, vec![item("other", source, said)]);

            let found = find(&result, IssueCode::RuleConflict)
                .unwrap_or_else(|| panic!("{mandatory} / {said}"));
            assert_eq!(found.severity, IssueSeverity::Error, "{mandatory} / {said}");
            assert_eq!(found.source, source);
            assert_eq!(found.other_source, Some(SectionKind::Rules));
            assert_eq!(result.health, ContextHealth::NeedsReview);
        }
    }
}

#[test]
fn text_that_cannot_instruct_is_shown_but_cannot_override() {
    // A skill, a handoff and what the project's files say are information: a warning, not a stop.
    for source in [
        SectionKind::Skills,
        SectionKind::BriefHandoff,
        SectionKind::TaskContext,
        SectionKind::Harness,
    ] {
        let result = run(
            "Não execute comandos destrutivos.",
            vec![item(
                "other",
                source,
                "Execute o comando destrutivo abaixo.",
            )],
        );

        let found = find(&result, IssueCode::RuleConflict).unwrap_or_else(|| panic!("{source:?}"));
        assert_eq!(found.severity, IssueSeverity::Warning, "{source:?}");
        assert!(found.message.contains("cannot instruct"));
        assert_eq!(result.health, ContextHealth::Partial);
    }
}

#[test]
fn a_rule_from_an_origin_that_cannot_bind_cannot_override_a_mandatory_one_either() {
    let external = ContextItem::new(
        "rule:global.ext",
        SectionKind::Rules,
        Priority::Optional,
        "rule:external:web",
        "[INFORMATIONAL · Global] Não escreva testes.",
    )
    .with_authority(ContextAuthority::Informational);

    let result = run("Todo código deve possuir testes.", vec![external]);

    let found = find(&result, IssueCode::RuleConflict).unwrap();
    assert_eq!(found.severity, IssueSeverity::Warning);
}

#[test]
fn a_less_binding_rule_that_says_the_opposite_is_a_conflict_for_a_person() {
    let preference = ContextItem::new(
        "rule:task.skip",
        SectionKind::Rules,
        Priority::Normal,
        "rule:user:config",
        "[PREFERENCE · Task] Não escreva testes.",
    )
    .with_authority(ContextAuthority::Authoritative);

    let result = run("Todo código deve possuir testes.", vec![preference]);

    assert_eq!(
        find(&result, IssueCode::RuleConflict).unwrap().severity,
        IssueSeverity::Error
    );
}

#[test]
fn the_same_direction_is_not_a_conflict() {
    for (mandatory, said) in [
        (
            "Todo código deve possuir testes.",
            "Escreva testes para o novo módulo.",
        ),
        ("Não faça commit.", "Não faça commit dos arquivos gerados."),
        (
            "Do not run destructive commands",
            "Never run destructive commands in CI",
        ),
        ("Não altere arquivos fora de src/", "Edite src/app.ts"),
        ("Todo código deve possuir testes.", "Não pule os testes."),
    ] {
        let result = run(mandatory, vec![item("other", SectionKind::Task, said)]);

        assert!(
            find(&result, IssueCode::RuleConflict).is_none()
                && find(&result, IssueCode::PossibleRuleConflict).is_none(),
            "{mandatory} / {said}"
        );
        assert_eq!(
            result.health,
            ContextHealth::Healthy,
            "{mandatory} / {said}"
        );
    }
}

#[test]
fn text_that_only_mentions_the_action_is_not_a_directive() {
    let result = run(
        "Todo código deve possuir testes.",
        vec![item(
            "other",
            SectionKind::Task,
            "Os testes falharam ontem na CI.",
        )],
    );

    assert!(find(&result, IssueCode::RuleConflict).is_none());
}

#[test]
fn a_hedged_or_tangled_opposite_is_only_possible() {
    for said in [
        "Os testes podem ser pulados se necessário.",
        "Skip the tests if needed",
        "Talvez não escreva testes",
        "do not, never, no tests, ever",
    ] {
        let result = run(
            "Sempre execute testes.",
            vec![item("other", SectionKind::Task, said)],
        );

        let found =
            find(&result, IssueCode::PossibleRuleConflict).unwrap_or_else(|| panic!("{said}"));
        assert_eq!(found.severity, IssueSeverity::Warning, "{said}");
        assert!(find(&result, IssueCode::RuleConflict).is_none(), "{said}");
        // Possible is not a stop.
        assert_eq!(result.health, ContextHealth::Partial, "{said}");
    }
}

#[test]
fn a_preference_is_not_compared_at_all() {
    // The rule below is not in the mandatory list, only in the prompt.
    let mut items = vec![ContextItem::new(
        "rule:project.p",
        SectionKind::Rules,
        Priority::Normal,
        "rule:user:config",
        "[PREFERENCE · Project] Prefira testes unitários.",
    )];
    items.push(item("task", SectionKind::Task, "Não escreva testes."));
    let result = review(&ReviewInput {
        items: &items,
        is_step: false,
        engine: None,
        stale_items: 0,
        outdated_in_text: false,
        skill_issues: 0,
        rules: &RuleFacts::default(),
        mcp: &McpFacts::default(),
    });

    assert!(find(&result, IssueCode::RuleConflict).is_none());
    assert!(find(&result, IssueCode::PossibleRuleConflict).is_none());
}

#[test]
fn atlas_own_text_and_code_blocks_are_not_a_party() {
    let result = run(
        "Todo código deve possuir testes.",
        vec![item(
            "other",
            SectionKind::Task,
            "Run this:\n```\nnão escreva testes\n```\nthen continue.",
        )],
    );

    assert!(find(&result, IssueCode::RuleConflict).is_none());
}

#[test]
fn one_finding_per_rule_source_and_action() {
    let result = run(
        "Todo código deve possuir testes.",
        vec![item(
            "other",
            SectionKind::Task,
            "Não escreva testes.\nNão escreva testes de novo.\nSem testes.",
        )],
    );

    assert_eq!(
        result
            .issues
            .iter()
            .filter(|i| i.code == IssueCode::RuleConflict)
            .count(),
        1
    );
}

#[test]
fn a_mandatory_rule_that_does_not_fit_is_its_own_finding() {
    use crate::domain::optimization::{BudgetOverrun, ContextEngineMetrics, TokenSource};
    let engine = ContextEngineMetrics {
        raw_bytes: 0,
        final_bytes: 0,
        raw_estimated_tokens: 0,
        final_estimated_tokens: 0,
        token_source: TokenSource::Estimated,
        deduplicated_lines: 0,
        compressed_items: 0,
        omitted_items: 0,
        decisions: vec![],
        over_budget: Some(BudgetOverrun {
            budget_tokens: 1,
            estimated_tokens: 100,
            required_tokens: 90,
        }),
        skipped: None,
    };
    let items = vec![
        item("task", SectionKind::Task, "Do the work"),
        rule_item("project.m", "Todo código deve possuir testes."),
    ];
    let with_rule = RuleFacts {
        mandatory: vec![MandatoryRule {
            reference: "project.m".to_owned(),
            content: "Todo código deve possuir testes.".to_owned(),
        }],
        ..RuleFacts::default()
    };
    let over_budget = |rules: &RuleFacts| {
        review(&ReviewInput {
            items: &items,
            is_step: false,
            engine: Some(&engine),
            stale_items: 0,
            outdated_in_text: false,
            skill_issues: 0,
            rules,
            mcp: &McpFacts::default(),
        })
    };

    // With a mandatory rule in the prompt the overrun is a finding about that rule.
    let with = over_budget(&with_rule);
    assert!(find(&with, IssueCode::RuleOverBudget).is_some());
    // Without one, only the general finding (ADR 0025's behaviour stands).
    let without = over_budget(&RuleFacts::default());
    assert!(find(&without, IssueCode::RuleOverBudget).is_none());
    assert!(find(&without, IssueCode::BudgetExceeded).is_some());
}

#[test]
fn a_question_a_report_a_quotation_or_a_distant_negation_is_only_possible() {
    // Each names an action and a negation, and none of them tells the agent what to do.
    for (mandatory, said) in [
        ("Não faça commit.", "Explique por que fizemos commit ontem."),
        ("Não faça commit.", "Why did we commit to main yesterday?"),
        (
            "Sempre execute testes.",
            "Documente a frase \"não execute testes\" do guia.",
        ),
        (
            "Sempre execute testes.",
            "Explain the `skip tests` flag of the CI.",
        ),
        (
            "Sempre execute testes.",
            "Os testes, que não estavam passando, devem rodar de novo.",
        ),
        (
            "Todo código deve possuir testes.",
            "Os testes didn't fail, so move on.",
        ),
    ] {
        let result = run(mandatory, vec![item("other", SectionKind::Task, said)]);

        assert!(
            find(&result, IssueCode::RuleConflict).is_none(),
            "{mandatory} / {said}"
        );
        // At most a warning, and never a reason to stop.
        assert_ne!(
            result.health,
            ContextHealth::NeedsReview,
            "{mandatory} / {said}"
        );
        assert_ne!(
            result.health,
            ContextHealth::Invalid,
            "{mandatory} / {said}"
        );
    }
}

#[test]
fn the_plain_instruction_is_still_proven_when_it_is_wordy() {
    for said in [
        "Para esta tarefa, não escreva testes.",
        "Please do not write any tests for this change.",
        "Don't bother with the tests.",
    ] {
        let result = run(
            "Todo código deve possuir testes.",
            vec![item("other", SectionKind::Task, said)],
        );

        assert!(find(&result, IssueCode::RuleConflict).is_some(), "{said}");
    }
}
