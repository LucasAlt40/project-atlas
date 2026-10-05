use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::memory::MemorySkillStore;
use super::*;
use crate::application::optimization::skills::validate::{IssueCode, Severity};

const PROJECT: &str = "/work/project";

fn project_dir() -> PathBuf {
    Path::new(PROJECT).join(".atlas").join("skills")
}

fn user_dir() -> PathBuf {
    PathBuf::from("/home/me/skills")
}

fn service(store: &Arc<MemorySkillStore>) -> SkillService {
    SkillService::new(store.clone(), Some(user_dir()))
}

fn skill_md(name: &str, description: &str, body: &str) -> String {
    format!("---\nname: {name}\ndescription: {description}\n---\n{body}")
}

const BILLING: &str = "Use when changing invoices, taxes or billing totals in the finance module";
const FRONTEND: &str = "Use when building Angular screens and PrimeNG components for the web app";

fn input<'a>(task: &'a str, technologies: &'a [String]) -> SelectionInput<'a> {
    SelectionInput {
        task,
        personality_id: "developer",
        technologies,
    }
}

fn store_with_two() -> Arc<MemorySkillStore> {
    let store = Arc::new(MemorySkillStore::default());
    store.put(
        &project_dir(),
        "billing-rules",
        &skill_md(
            "billing-rules",
            BILLING,
            "Round every invoice total to two decimals.\nApply the tax table of the invoice country.\n",
        ),
    );
    store.put(
        &project_dir(),
        "angular-screens",
        &skill_md(
            "angular-screens",
            FRONTEND,
            "Build screens with standalone Angular components.\nUse PrimeNG tables for lists.\n",
        ),
    );
    store
}

#[test]
fn discovery_reads_only_names_descriptions_and_metadata_and_keeps_them() {
    let store = store_with_two();
    let skills = service(&store);

    let first = skills.discover(PROJECT);
    assert_eq!(first.entries.len(), 2);
    assert_eq!((first.cache_hits, first.cache_misses), (0, 2));
    assert!(first.entries.iter().all(SkillEntry::usable));
    assert_eq!(store.reads(), 2);

    // Nothing changed: nothing is read again.
    let second = skills.discover(PROJECT);
    assert_eq!((second.cache_hits, second.cache_misses), (2, 0));
    assert_eq!(store.reads(), 2);
}

#[test]
fn a_changed_skill_is_read_again_and_the_others_are_not() {
    let store = store_with_two();
    let skills = service(&store);
    skills.discover(PROJECT);

    store.put(
        &project_dir(),
        "billing-rules",
        &skill_md(
            "billing-rules",
            BILLING,
            "A new instruction about rounding totals.\n",
        ),
    );
    let again = skills.discover(PROJECT);

    assert_eq!((again.cache_hits, again.cache_misses), (1, 1));
    assert_eq!(store.reads(), 3);
    let billing = again
        .entries
        .iter()
        .find(|e| e.dir == "billing-rules")
        .unwrap();
    assert!(billing
        .skill
        .as_ref()
        .unwrap()
        .body
        .contains("new instruction"));
}

#[test]
fn only_the_skills_the_task_calls_for_are_loaded_and_the_rest_cost_nothing() {
    let store = store_with_two();
    let skills = service(&store);

    let plan = skills.prepare(
        PROJECT,
        &input("Fix the invoice total rounding in the billing module", &[]),
    );

    assert_eq!(plan.metrics.activated, ["billing-rules"]);
    let text = plan.text().unwrap();
    assert!(text.contains("### Skill: billing-rules (project)"));
    assert!(text.contains("Round every invoice total"));
    // The other skill is not in the prompt at all, and neither is a list of what was not loaded.
    assert!(!text.contains("angular-screens"));
    assert!(!text.contains("PrimeNG"));
    assert!(text.starts_with("Skills Atlas selected for this task."));
    // Discovery saw both and avoided sending their metadata.
    assert_eq!(plan.metrics.discovered, 2);
    assert_eq!(plan.metrics.usable, 2);
    assert!(plan.metrics.level1_tokens > 0);
    assert!(plan.metrics.level2_tokens > 0);
    assert_eq!(plan.metrics.token_source, TokenSource::Estimated);
}

#[test]
fn a_task_no_skill_fits_loads_nothing_and_the_prompt_has_no_skills_section() {
    let store = store_with_two();
    let skills = service(&store);

    let plan = skills.prepare(PROJECT, &input("Rename the variable in the parser", &[]));

    assert_eq!(plan.blocks.len(), 0);
    assert_eq!(plan.text(), None);
    assert_eq!(plan.metrics.level2_tokens, 0);
}

#[test]
fn naming_a_skill_loads_it_even_when_the_words_do_not_match() {
    let store = store_with_two();
    let skills = service(&store);

    let plan = skills.prepare(
        PROJECT,
        &input("Please use the /angular-screens skill here", &[]),
    );

    assert_eq!(plan.metrics.activated, ["angular-screens"]);
    assert!(plan.blocks[0].explicit);
    assert_eq!(plan.candidates[0].reasons, ["asked for by name"]);
}

#[test]
fn stack_and_personality_hints_tip_a_skill_over_the_threshold() {
    let store = Arc::new(MemorySkillStore::default());
    store.put(
        &project_dir(),
        "release-checks",
        "---\nname: release-checks\ndescription: Use when preparing a release of the product for customers\nmetadata:\n  atlas-personalities: qa, developer\n  atlas-stacks: rust\n  atlas-files: '*.toml'\n---\nRun the release checklist.\n",
    );
    let skills = service(&store);
    let rust = ["Rust".to_owned()];

    // One word of the name ("release") alone is 3 points: below the bar.
    let weak = skills.prepare(
        PROJECT,
        &SelectionInput {
            task: "Prepare the release",
            personality_id: "architect",
            technologies: &[],
        },
    );
    assert_eq!(weak.blocks.len(), 0);
    // With the stack, the personality and a file type the task mentions, it clears it.
    let strong = skills.prepare(
        PROJECT,
        &input("Prepare the release and bump Cargo.toml", &rust),
    );
    assert_eq!(strong.metrics.activated, ["release-checks"]);
    let reasons = strong.candidates[0].reasons.join(" | ");
    assert!(reasons.contains("stack: Rust"), "{reasons}");
    assert!(reasons.contains("personality: developer"), "{reasons}");
    assert!(reasons.contains("file type"), "{reasons}");
}

#[test]
fn at_most_three_skills_load_and_the_best_scoring_come_first() {
    let store = Arc::new(MemorySkillStore::default());
    for name in ["invoice-one", "invoice-two", "invoice-three"] {
        store.put(
            &project_dir(),
            name,
            &skill_md(
                name,
                "Use when changing invoice totals in the finance module",
                "Do the thing for invoices.\n",
            ),
        );
    }
    store.put(
        &project_dir(),
        "invoice-four",
        "---\nname: invoice-four\ndescription: Use when changing invoice totals in the finance module\nmetadata:\n  atlas-priority: 5\n---\nDo the thing for invoices.\n",
    );
    let skills = service(&store);

    let plan = skills.prepare(PROJECT, &input("Change the invoice totals", &[]));

    assert_eq!(plan.blocks.len(), 3);
    // The priority breaks the tie: the fourth skill is the first.
    assert_eq!(plan.blocks[0].name, "invoice-four");
    assert!(plan.candidates[0].score > plan.candidates[2].score);
}

#[test]
fn a_reference_comes_in_only_when_the_task_points_at_it() {
    let store = store_with_two();
    store.put_resource(
        &project_dir(),
        "billing-rules",
        "references/tax-table.md",
        "Brazil: 17 percent.\nGermany: 19 percent.\n",
    );
    store.put_resource(
        &project_dir(),
        "billing-rules",
        "references/history.md",
        "Long history nobody needs.\n",
    );
    store.put_resource(
        &project_dir(),
        "billing-rules",
        "scripts/recalculate.sh",
        "echo recalculating",
    );
    let skills = service(&store);

    let plain = skills.prepare(
        PROJECT,
        &input("Fix the invoice total rounding in billing", &[]),
    );
    assert!(!plain.text().unwrap().contains("Brazil"));
    assert_eq!(plain.metrics.resources_available, 3);
    assert_eq!(plain.metrics.resources_loaded, 0);

    let pointed = skills.prepare(
        PROJECT,
        &input("Fix the invoice total in billing using the tax table", &[]),
    );
    let text = pointed.text().unwrap();
    assert!(text.contains("#### Reference: references/tax-table.md"));
    assert!(text.contains("Brazil: 17 percent."));
    // The one nobody pointed at and the script are not sent.
    assert!(!text.contains("Long history"));
    assert!(!text.contains("recalculating"));
    assert_eq!(pointed.metrics.resources_loaded, 1);
    assert!(pointed.metrics.level3_tokens > 0);
}

#[test]
fn a_reference_the_skill_links_comes_in_when_the_task_uses_the_links_words() {
    let store = Arc::new(MemorySkillStore::default());
    store.put(
        &project_dir(),
        "billing-rules",
        &skill_md(
            "billing-rules",
            BILLING,
            "Round totals. For country rates see [the rates guide](references/g1.md).\n",
        ),
    );
    store.put_resource(
        &project_dir(),
        "billing-rules",
        "references/g1.md",
        "Rates by country.\n",
    );
    let skills = service(&store);

    let plan = skills.prepare(
        PROJECT,
        &input("Fix the invoice totals in billing using the rates", &[]),
    );

    assert_eq!(plan.metrics.resources_loaded, 1);
    assert!(plan.text().unwrap().contains("Rates by country."));
}

#[test]
fn a_reference_too_big_for_the_prompt_is_left_out() {
    let store = store_with_two();
    store.put_resource(
        &project_dir(),
        "billing-rules",
        "references/tax-table.md",
        &"tax ".repeat(20_000),
    );
    let skills = service(&store);

    let plan = skills.prepare(
        PROJECT,
        &input("Fix the invoice total in billing using the tax table", &[]),
    );

    assert_eq!(plan.metrics.resources_loaded, 0);
    assert_eq!(plan.metrics.level3_tokens, 0);
}

#[test]
fn a_broken_skill_is_listed_with_why_and_never_selected() {
    let store = store_with_two();
    store.put(
        &project_dir(),
        "no-frontmatter",
        "just text, no frontmatter",
    );
    store.put(
        &project_dir(),
        "no-description",
        "---\nname: no-description\n---\nInstructions about invoices and billing totals.\n",
    );
    store.put(
        &project_dir(),
        "Bad_Name",
        &skill_md(
            "Bad_Name",
            BILLING,
            "Invoice billing totals instructions.\n",
        ),
    );
    let skills = service(&store);

    let discovery = skills.discover(PROJECT);
    let unusable: Vec<_> = discovery
        .entries
        .iter()
        .filter(|e| !e.usable())
        .map(|e| e.dir.as_str())
        .collect();
    assert_eq!(unusable.len(), 3, "{unusable:?}");
    let code_of = |dir: &str| {
        discovery
            .entries
            .iter()
            .find(|e| e.dir == dir)
            .unwrap()
            .issues
            .iter()
            .map(|i| i.code)
            .collect::<Vec<_>>()
    };
    assert!(code_of("no-frontmatter").contains(&IssueCode::Malformed));
    assert!(code_of("no-description").contains(&IssueCode::MissingDescription));
    assert!(code_of("Bad_Name").contains(&IssueCode::InvalidName));

    let plan = skills.prepare(PROJECT, &input("Change invoice billing totals", &[]));
    assert_eq!(plan.metrics.activated, ["billing-rules"]);
    assert_eq!(plan.metrics.discovered, 5);
    assert_eq!(plan.metrics.usable, 2);
    assert!(plan.metrics.issues >= 3);
}

#[test]
fn a_project_skill_shadows_a_user_skill_of_the_same_name() {
    let store = Arc::new(MemorySkillStore::default());
    store.put(
        &project_dir(),
        "billing-rules",
        &skill_md(
            "billing-rules",
            BILLING,
            "The project's way to round invoice totals.\n",
        ),
    );
    store.put(
        &user_dir(),
        "billing-rules",
        &skill_md(
            "billing-rules",
            BILLING,
            "My own way to round invoice totals.\n",
        ),
    );
    let skills = service(&store);

    let plan = skills.prepare(PROJECT, &input("Round the invoice totals in billing", &[]));

    let text = plan.text().unwrap();
    assert!(text.contains("The project's way"));
    assert!(!text.contains("My own way"));
    let discovery = skills.discover(PROJECT);
    let shadowed = discovery
        .entries
        .iter()
        .find(|e| e.scope == SkillScope::User)
        .unwrap();
    assert!(shadowed
        .issues
        .iter()
        .any(|i| i.code == IssueCode::Shadowed && i.severity == Severity::Error));
}

#[test]
fn a_skill_that_repeats_another_is_flagged() {
    let store = Arc::new(MemorySkillStore::default());
    let shared = "Always run the formatter before committing any change.\nKeep functions small and focused on one thing.\nWrite a test for every bug you fix today.\n";
    store.put(
        &project_dir(),
        "style-a",
        &skill_md(
            "style-a",
            "Use when writing code in the repository for style review",
            shared,
        ),
    );
    store.put(
        &project_dir(),
        "style-b",
        &skill_md(
            "style-b",
            "Use when reviewing code in the repository for style issues",
            shared,
        ),
    );
    let skills = service(&store);

    let discovery = skills.discover(PROJECT);

    let b = discovery
        .entries
        .iter()
        .find(|e| e.dir == "style-b")
        .unwrap();
    assert!(b
        .issues
        .iter()
        .any(|i| i.code == IssueCode::DuplicatedInstructions));
    let a = discovery
        .entries
        .iter()
        .find(|e| e.dir == "style-a")
        .unwrap();
    assert!(!a.issues.iter().any(|i| i.message.contains("repeats three")));
}

#[test]
fn no_skills_folder_means_an_empty_catalogue_not_an_error() {
    let store = Arc::new(MemorySkillStore::default());
    let skills = SkillService::new(store, None);

    let plan = skills.prepare(PROJECT, &input("Anything at all about invoices", &[]));

    assert_eq!(plan.blocks.len(), 0);
    assert_eq!(plan.metrics.discovered, 0);
}

#[test]
fn progressive_disclosure_sends_a_fraction_of_a_catalogue_loaded_whole() {
    let store = Arc::new(MemorySkillStore::default());
    let topics = [
        "billing invoices",
        "angular screens",
        "database migrations",
        "release notes",
        "security reviews",
        "accessibility audits",
        "api versioning",
        "error handling",
        "logging conventions",
        "performance tuning",
    ];
    let mut everything = 0;
    for topic in topics {
        let name = topic.replace(' ', "-");
        let body = (0..40)
            .map(|i| format!("Rule {i} for {topic}: keep the {topic} work small and explicit."))
            .collect::<Vec<_>>()
            .join("\n");
        everything += crate::domain::optimization::TextSize::of(&body).estimated_tokens();
        store.put(
            &project_dir(),
            &name,
            &skill_md(
                &name,
                &format!("Use when the task is about {topic} in the product code base"),
                &body,
            ),
        );
    }
    let skills = service(&store);

    let plan = skills.prepare(PROJECT, &input("Update the billing invoices module", &[]));

    assert_eq!(plan.metrics.activated, ["billing-invoices"]);
    let sent = plan.metrics.level2_tokens + plan.metrics.level3_tokens;
    // One skill of ten: about a tenth of the instructions, and none of the catalogue metadata.
    assert!(sent * 8 < everything, "{sent} of {everything}");
    println!(
        "SKILLS: catalogue loaded whole {} tokens (+{} of metadata), progressive {} tokens",
        everything, plan.metrics.level1_tokens, sent
    );
}
