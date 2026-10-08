use super::*;
use crate::application::optimization::context::item::{ContextItem, Priority};
use crate::domain::context::{ExecutionBudget, ModelLimits, Precision, ReportedLimits};
use crate::domain::optimization::{DecisionKind, SectionKind};

fn item(id: &str, source: SectionKind, priority: Priority, content: &str) -> ContextItem {
    ContextItem::new(id, source, priority, "test", content)
}

fn required(id: &str, source: SectionKind, content: &str) -> ContextItem {
    item(id, source, Priority::Required, content)
}

fn high(id: &str, source: SectionKind, content: &str) -> ContextItem {
    item(id, source, Priority::High, content)
}

fn no_budget() -> Option<u64> {
    None
}

/// `n` words no other filler shares, so fillers never repeat one another.
fn filler(seed: &str, n: usize) -> String {
    (0..n)
        .map(|i| format!("{seed}{i}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn content<'a>(plan: &'a EnginePlan, id: &str) -> &'a str {
    &plan.items.iter().find(|i| i.id == id).unwrap().content
}

#[test]
fn an_exact_repeat_is_removed_from_the_later_item_and_the_earlier_one_keeps_it() {
    let plan = ContextEngine::plan(
        vec![
            required(
                "rules",
                SectionKind::AtlasRules,
                "The project uses Angular 22 with PrimeNG\nBe brief",
            ),
            high(
                "harness",
                SectionKind::Harness,
                "The project uses Angular 22 with PrimeNG\nBackend is Rust and Tauri",
            ),
        ],
        no_budget(),
    );

    assert_eq!(
        content(&plan, "harness"),
        "Backend is Rust and Tauri",
        "{plan:?}"
    );
    // The required item is untouched.
    assert!(content(&plan, "rules").contains("Angular 22"));
    assert_eq!(plan.deduplicated_lines, 1);
    let decision = &plan.decisions[0];
    assert_eq!(decision.kind, DecisionKind::DuplicateExact);
    assert_eq!(decision.source, SectionKind::Harness);
    assert_eq!(decision.kept_in, Some(SectionKind::AtlasRules));
}

#[test]
fn case_punctuation_and_spacing_do_not_make_a_line_different() {
    let plan = ContextEngine::plan(
        vec![
            required(
                "a",
                SectionKind::AtlasRules,
                "- Stack: Angular 22 + PrimeNG.",
            ),
            high(
                "b",
                SectionKind::Harness,
                "  stack - angular 22   primeng  ",
            ),
        ],
        no_budget(),
    );

    assert_eq!(plan.decisions[0].kind, DecisionKind::DuplicateNormalized);
    assert_eq!(plan.deduplicated_lines, 1);
}

#[test]
fn a_line_whose_every_word_is_in_one_earlier_line_adds_nothing_and_goes() {
    let plan = ContextEngine::plan(
        vec![
            required(
                "context",
                SectionKind::BriefWorkflowContext,
                "- architecture.md (docs/architecture.md) from Backend: POST /password-reset, 15m token",
            ),
            high(
                "handoff",
                SectionKind::BriefHandoff,
                "- architecture.md (docs/architecture.md): POST /password-reset, 15m token",
            ),
        ],
        no_budget(),
    );

    assert_eq!(plan.decisions[0].kind, DecisionKind::DuplicateOverlap);
    assert_eq!(plan.deduplicated_lines, 1);
}

#[test]
fn a_line_that_says_something_new_is_kept_even_if_it_shares_most_words() {
    let plan = ContextEngine::plan(
        vec![
            required(
                "a",
                SectionKind::AtlasRules,
                "Use JWT for the reset token and expire it in fifteen minutes",
            ),
            high(
                "b",
                SectionKind::Harness,
                "Use JWT for the reset token and never expire it",
            ),
        ],
        no_budget(),
    );

    // "never" is new: the second line is not contained in the first.
    assert_eq!(plan.deduplicated_lines, 0);
    assert!(!plan.changed());
}

#[test]
fn short_lines_headings_and_code_are_never_compared() {
    let plan = ContextEngine::plan(
        vec![
            required(
                "a",
                SectionKind::AtlasRules,
                "Decisions:\n- JWT\n```\nlet total = price + tax;\n```",
            ),
            high(
                "b",
                SectionKind::Harness,
                "Decisions:\n- JWT\n```\nlet total = price + tax;\n```",
            ),
        ],
        no_budget(),
    );

    // Two words are not a claim; a heading is structure; code is literal.
    assert!(!plan.changed(), "{plan:?}");
}

#[test]
fn a_list_whose_every_bullet_was_a_repeat_loses_its_heading_too() {
    let plan = ContextEngine::plan(
        vec![
            required(
                "context",
                SectionKind::BriefWorkflowContext,
                "Decisions to respect:\n- JWT: use JWT for the reset token — the project already uses JWT\n- Mail: send the reset link by email only\n",
            ),
            high(
                "handoff",
                SectionKind::BriefHandoff,
                "Summary of the work\nDecisions:\n- JWT: use JWT for the reset token\n- Mail: send the reset link by email only\nArtifacts:\n- notes.md: a new note about the flow\n",
            ),
        ],
        no_budget(),
    );

    let handoff = content(&plan, "handoff");
    assert!(!handoff.contains("Decisions:"), "{handoff}");
    assert!(!handoff.contains("JWT"), "{handoff}");
    // What was not said before stays, with its own heading.
    assert!(handoff.contains("Artifacts:\n- notes.md"), "{handoff}");
    assert!(handoff.contains("Summary of the work"));
}

#[test]
fn required_context_is_never_edited_even_when_it_repeats_itself() {
    let repeated = "Never write outside your working directory under any condition";
    let plan = ContextEngine::plan(
        vec![
            required("rules", SectionKind::AtlasRules, repeated),
            required("protocols", SectionKind::BriefProtocols, repeated),
        ],
        Some(1),
    );

    assert!(
        !plan.changed()
            || plan
                .decisions
                .iter()
                .all(|d| d.kind != DecisionKind::DuplicateExact)
    );
    assert_eq!(content(&plan, "rules"), repeated);
    assert_eq!(content(&plan, "protocols"), repeated);
    // Too big for the budget, and nothing was cut to hide it.
    let over = plan.over_budget.expect("reported, not cut");
    assert_eq!(over.budget_tokens, 1);
    assert!(over.required_tokens >= over.budget_tokens);
}

#[test]
fn a_budget_leaves_out_the_least_important_whole_items_first_and_says_so() {
    let (a, b, c) = (filler("a", 200), filler("b", 200), filler("c", 200));
    let plan = ContextEngine::plan(
        vec![
            required("task", SectionKind::Task, "Fix the invoice total"),
            high("harness", SectionKind::Harness, &a),
            item("history", SectionKind::Task, Priority::Optional, &b),
            item("hints", SectionKind::Task, Priority::Normal, &c),
        ],
        Some(400),
    );

    // Optional goes before Normal; High and Required stay.
    assert!(content(&plan, "history").starts_with("[Left out to fit the context budget: history"));
    assert!(content(&plan, "hints").starts_with("[Left out"));
    assert_eq!(content(&plan, "harness"), a.as_str());
    assert_eq!(content(&plan, "task"), "Fix the invoice total");
    assert_eq!(plan.omitted_items, 2);
    assert!(plan
        .decisions
        .iter()
        .any(|d| d.kind == DecisionKind::Omitted && d.preview.starts_with('b')));
}

#[test]
fn only_as_much_is_left_out_as_the_budget_needs_and_stale_goes_first() {
    let (fresh, old) = (filler("f", 100), filler("o", 100));
    let plan = ContextEngine::plan(
        vec![
            required("task", SectionKind::Task, "Fix the invoice total"),
            item("fresh", SectionKind::Task, Priority::Normal, &fresh),
            item("old", SectionKind::Task, Priority::Normal, &old).stale(),
        ],
        // One filler fits, two do not.
        Some(150),
    );

    assert!(content(&plan, "old").starts_with("[Left out"));
    assert_eq!(content(&plan, "fresh"), fresh.as_str());
    assert_eq!(plan.omitted_items, 1);
    assert!(plan.over_budget.is_none());
}

#[test]
fn what_is_kept_for_the_answer_comes_out_of_the_budget() {
    let extra = filler("e", 100);
    let items = || {
        vec![
            required("task", SectionKind::Task, "Fix the invoice total"),
            item("extra", SectionKind::Task, Priority::Optional, &extra),
        ]
    };
    // The same window, with more or less of it kept for the answer.
    let budget = |kept_for_output: u64| {
        ExecutionBudget::resolve(
            ModelLimits::reported(
                "rt",
                "m",
                ReportedLimits {
                    input: None,
                    output: Some(kept_for_output),
                    total: Some(250),
                    precision: Precision::Exact,
                },
            ),
            &[],
        )
    };

    assert_eq!(
        ContextEngine::plan(items(), budget(50).available_for_context()).omitted_items,
        0
    );
    assert_eq!(
        ContextEngine::plan(items(), budget(200).available_for_context()).omitted_items,
        1
    );
}

#[test]
fn without_a_budget_nothing_is_ever_left_out() {
    let filler = "word ".repeat(5_000);
    let plan = ContextEngine::plan(
        vec![item(
            "history",
            SectionKind::Task,
            Priority::Optional,
            &filler,
        )],
        no_budget(),
    );

    assert_eq!(plan.omitted_items, 0);
    assert!(plan.over_budget.is_none());
}

#[test]
fn whitespace_is_tidied_in_editable_items_only_and_no_word_changes() {
    let rules = "Rule one stays here\n\n\n\nRule two stays too   \n";
    let harness = "First fact stays here\n\n\n\nSecond fact stays too   \n";
    let plan = ContextEngine::plan(
        vec![
            required("rules", SectionKind::AtlasRules, rules),
            high("harness", SectionKind::Harness, harness),
        ],
        no_budget(),
    );

    assert_eq!(content(&plan, "rules"), rules);
    assert_eq!(
        content(&plan, "harness"),
        "First fact stays here\n\nSecond fact stays too\n"
    );
    assert_eq!(plan.compressed_items, 1);
}

#[test]
fn fingerprints_follow_the_text() {
    let a = ContextItem::new("a", SectionKind::Task, Priority::High, "t", "same text");
    let b = ContextItem::new("b", SectionKind::Harness, Priority::High, "t", "same text");
    let c = ContextItem::new("c", SectionKind::Task, Priority::High, "t", "other text");

    assert_eq!(a.fingerprint, b.fingerprint);
    assert_ne!(a.fingerprint, c.fingerprint);
}
