use std::sync::Arc;

use super::*;
use crate::application::harness::context::{
    HarnessBudget, HarnessContextBuilder, HarnessLoad, TaskContextLoad,
};
use crate::application::harness::fake::MemoryHarnessStore;
use crate::application::harness::generator::{user_files, ExistingUserFiles};
use crate::application::harness::manifest::render_knowledge;
use crate::domain::harness::{
    AnalysisInfo, Claim, Confidence, Conflict, Evidence, Finding, FindingCategory,
    HarnessKnowledge, Origin, UserKnowledge, Verification, VerificationStatus, KNOWLEDGE_VERSION,
};
use crate::domain::task_context::{ContextMode, TaskContext};

// ---- fixture: a small ERP with an API, an Angular app, a database, CI and tests ----

fn fact(category: FindingCategory, key: &str, label: &str, source: &str) -> Finding {
    Finding::new(
        category,
        key,
        "true",
        Confidence::High,
        Origin::Fact,
        source,
        "",
    )
    .with_label(label)
}

fn inferred(category: FindingCategory, key: &str, label: &str, sources: &[&str]) -> Finding {
    Finding::new(
        category,
        key,
        "true",
        Confidence::Medium,
        Origin::Inference,
        sources[0],
        "",
    )
    .with_label(label)
    .with_evidence(sources.iter().map(|s| Evidence::new(s, None)).collect())
}

fn findings() -> Vec<Finding> {
    use FindingCategory as C;
    vec![
        fact(C::Language, "typescript", "TypeScript", "package.json"),
        fact(C::Framework, "angular", "Angular 18", "package.json"),
        fact(C::Framework, "nestjs", "NestJS 10", "package.json"),
        fact(
            C::Database,
            "postgresql",
            "PostgreSQL",
            "docker-compose.yml",
        ),
        fact(C::Testing, "vitest", "Vitest", "package.json"),
        fact(
            C::Ci,
            "github_actions",
            "GitHub Actions",
            ".github/workflows/ci.yml",
        ),
        fact(C::Build, "build", "npm run build", "package.json"),
        inferred(
            C::Architecture,
            "clean",
            "Clean Architecture",
            &["src/domain", "src/application", "src/infrastructure"],
        ),
        inferred(
            C::Module,
            "auth",
            "Authentication module",
            &["src/auth/password-reset.ts"],
        ),
        inferred(
            C::Module,
            "users",
            "Users module",
            &["src/users/users.service.ts"],
        ),
        inferred(
            C::Module,
            "billing",
            "Billing module",
            &["src/billing/invoice.ts"],
        ),
        inferred(C::Module, "maps", "Maps module", &["src/maps/geocoder.ts"]),
        inferred(
            C::Module,
            "reports",
            "Reports module",
            &["src/reports/export.ts"],
        ),
        inferred(
            C::Convention,
            "rest",
            "REST controllers under src/api",
            &["src/api/orders.controller.ts"],
        ),
        inferred(
            C::Convention,
            "errors",
            "Errors go through one exception filter",
            &["src/common/errors.ts"],
        ),
        inferred(
            C::Convention,
            "standalone",
            "Angular standalone components",
            &["src/app/app.component.ts"],
        ),
    ]
}

struct World {
    store: Arc<MemoryHarnessStore>,
}

impl World {
    fn with(findings: Vec<Finding>, conflicts: Vec<Conflict>, user: &UserKnowledge) -> Self {
        let manifest = "version: 1\nproject: {id: p, name: Transport ERP, initializedAt: 1}\n\
            repository: {type: git, root: .}\nstack: {languages: [TypeScript]}\n\
            context: {generated: true}\nharness: {version: 1}\n";
        let knowledge = HarnessKnowledge {
            version: KNOWLEDGE_VERSION,
            analysis: AnalysisInfo::default(),
            findings,
            conflicts,
        };
        let store = MemoryHarnessStore::with(&[
            ("project.yaml", manifest),
            (
                "knowledge/findings.yaml",
                &render_knowledge(&knowledge).unwrap(),
            ),
        ]);
        let (files, _) = user_files(&ExistingUserFiles::default(), user);
        for file in files {
            store
                .files
                .lock()
                .unwrap()
                .insert(file.path.clone(), file.content.clone());
        }
        Self {
            store: Arc::new(store),
        }
    }

    fn builder(&self) -> HarnessContextBuilder {
        HarnessContextBuilder::new(self.store.clone())
    }

    fn context(&self, task: &str) -> TaskContext {
        match self.builder().build_for_task("/erp", task) {
            TaskContextLoad::Ready(context) => *context,
            other => panic!("expected a task context, got {other:?}"),
        }
    }
}

fn user() -> UserKnowledge {
    UserKnowledge {
        purpose: "ERP for transport companies".to_owned(),
        users: "Dispatchers and drivers".to_owned(),
        concepts: "Shipment, route".to_owned(),
        business_rules:
            "Password reset links expire after one hour.\nInvoices are immutable once issued."
                .to_owned(),
        constraints: "Never log passwords or tokens.\nDo not change the infrastructure layer."
            .to_owned(),
        decisions: "Authentication uses JWT.\nDo not merge the domain and application layers."
            .to_owned(),
    }
}

fn included(c: &TaskContext) -> Vec<&str> {
    c.entries
        .iter()
        .filter(|e| e.outcome == EntryOutcome::Included)
        .map(|e| e.item.label.as_str())
        .collect()
}

fn excluded(c: &TaskContext) -> Vec<&str> {
    c.entries
        .iter()
        .filter(|e| e.outcome != EntryOutcome::Included)
        .map(|e| e.item.label.as_str())
        .collect()
}

// ---- TaskAnalyzer ----

#[test]
fn the_analyzer_reads_keywords_tags_areas_and_intent_from_an_english_task() {
    let s = TaskAnalyzer::analyze("Add password recovery endpoint.");

    assert_eq!(s.intent, TaskIntent::Add);
    assert_eq!(s.keywords, ["password", "recovery", "endpoint"]);
    for tag in ["password", "authentication", "api", "backend"] {
        assert!(
            s.tags.iter().any(|t| t == tag),
            "missing {tag}: {:?}",
            s.tags
        );
    }
    assert!(s.areas.contains(&Area::Business));
    assert!(s.baseline_areas.contains(&Area::Architecture));
    assert!(s.baseline_areas.contains(&Area::Testing));
}

#[test]
fn the_analyzer_reads_portuguese_as_well() {
    let s = TaskAnalyzer::analyze(
        "Adicionar endpoint de recuperação de senha usando o padrão existente.",
    );

    assert_eq!(s.intent, TaskIntent::Add);
    assert!(s.keywords.contains(&"senha".to_owned()));
    assert!(s.keywords.contains(&"recuperacao".to_owned()));
    // Stop words and verbs are not keywords.
    assert!(!s.keywords.contains(&"de".to_owned()));
    assert!(!s.keywords.contains(&"adicionar".to_owned()));
    for tag in ["password", "authentication", "api"] {
        assert!(s.tags.iter().any(|t| t == tag), "missing {tag}");
    }
}

#[test]
fn the_analyzer_normalizes_case_accents_and_punctuation() {
    let a = TaskAnalyzer::analyze("CORRIGIR o Erro de Autenticação!!!");
    let b = TaskAnalyzer::analyze("corrigir o erro de autenticacao");

    assert_eq!(a, b);
    assert_eq!(a.intent, TaskIntent::Fix);
    assert!(a.tags.iter().any(|t| t == "authentication"));
    assert!(a.tags.iter().any(|t| t == "errors"));
}

#[test]
fn an_empty_or_meaningless_task_has_no_signals() {
    for task in ["", "   ", "!!! ???", "a to of"] {
        let s = TaskAnalyzer::analyze(task);
        assert!(s.is_empty(), "{task:?} gave {s:?}");
        assert_eq!(s.intent, TaskIntent::Unknown);
    }
}

#[test]
fn the_analyzer_names_the_technologies_a_task_mentions() {
    let s = TaskAnalyzer::analyze("Add Redis caching");

    assert_eq!(s.technologies, ["Redis"]);
    assert!(s.tags.iter().any(|t| t == "caching"));
}

#[test]
fn intents_follow_the_verb() {
    for (task, intent) in [
        ("Fix CI pipeline", TaskIntent::Fix),
        ("Change the login page layout", TaskIntent::Change),
        ("Refactor the users module", TaskIntent::Refactor),
        ("Write tests for billing", TaskIntent::Test),
        ("Explain how sessions work", TaskIntent::Investigate),
        ("Document the API", TaskIntent::Document),
    ] {
        assert_eq!(TaskAnalyzer::analyze(task).intent, intent, "{task}");
    }
}

// ---- ranking ----

fn item(area: Area, tags: &[&str]) -> ContextItem {
    ContextItem {
        id: "x".to_owned(),
        block: ContextBlock::Know,
        kind: ItemKind::Finding,
        area,
        category: None,
        label: "Something".to_owned(),
        content: "Something".to_owned(),
        provenance: Some(Origin::Fact),
        verification: Some(VerificationStatus::Unverified),
        confidence: Some(Confidence::High),
        evidence: vec![],
        source_finding_id: None,
        tags: tags.iter().map(|t| (*t).to_owned()).collect(),
        priority: 8,
        group: None,
    }
}

fn signals(task: &str) -> TaskSignals {
    TaskAnalyzer::analyze(task)
}

#[test]
fn an_exact_tag_scores_and_the_reason_says_which() {
    let s = signals("Add password recovery endpoint");
    let reason = explain(&s, &item(Area::Modules, &["authentication"]));

    assert!(reason.score >= Weights::TAG_MATCH);
    assert_eq!(reason.matched_tags, ["authentication"]);
}

#[test]
fn an_item_with_no_tags_in_common_and_no_area_orientation_scores_nothing() {
    let s = signals("Fix CI pipeline");
    let reason = explain(&s, &item(Area::Business, &["billing"]));

    assert_eq!(reason.score, 0);
    assert!(reason.matched_tags.is_empty() && reason.matched_areas.is_empty());
}

#[test]
fn the_area_a_task_points_at_counts_only_with_something_specific_and_the_orienting_areas_alone() {
    let s = signals("Add password recovery endpoint");
    // Alone, the area the tags point at is not enough: every module would be "authentication".
    assert_eq!(explain(&s, &item(Area::Business, &[])).score, 0);
    let both = explain(&s, &item(Area::Business, &["password"]));
    let baseline = explain(&s, &item(Area::Architecture, &[]));

    assert_eq!(both.score, Weights::TAG_MATCH + Weights::FOCUS_AREA);
    assert_eq!(both.matched_areas, [Area::Business]);
    assert_eq!(baseline.score, Weights::BASELINE_AREA);
}

#[test]
fn the_category_a_task_asks_for_scores() {
    let s = signals("Change database migration for users");
    let mut db = item(Area::Infrastructure, &[]);
    db.category = Some(FindingCategory::Database);

    let reason = explain(&s, &db);

    assert_eq!(reason.matched_categories, [FindingCategory::Database]);
    assert!(reason.score >= Weights::CATEGORY_MATCH);
}

#[test]
fn a_path_that_shares_a_word_with_the_task_scores() {
    let s = signals("Add password recovery endpoint");
    let mut module = item(Area::Modules, &[]);
    module.evidence = vec![Evidence::new("src/auth/password-reset.ts", None)];
    let mut other = item(Area::Modules, &[]);
    other.evidence = vec![Evidence::new("src/maps/geocoder.ts", None)];

    let reason = explain(&s, &module);

    assert_eq!(reason.matched_paths, ["src/auth/password-reset.ts"]);
    assert_eq!(explain(&s, &other).matched_paths, Vec::<String>::new());
}

#[test]
fn constraints_and_decisions_are_always_included_whatever_they_score() {
    let s = signals("Fix CI pipeline");
    let mut constraint = item(Area::Constraints, &[]);
    constraint.kind = ItemKind::Constraint;
    let mut decision = item(Area::Decisions, &[]);
    decision.kind = ItemKind::Decision;

    let c = explain(&s, &constraint);
    let d = explain(&s, &decision);

    assert_eq!(c.always_included, Some(AlwaysIncluded::Constraint));
    assert_eq!(d.always_included, Some(AlwaysIncluded::Decision));
    assert!(c.score >= Weights::CONSTRAINT_OR_DECISION);
}

#[test]
fn a_verified_fact_outscores_the_same_match_when_only_inferred() {
    let s = signals("Add password recovery endpoint");
    let mut verified = item(Area::Modules, &["authentication"]);
    verified.verification = Some(VerificationStatus::Verified);
    let mut guess = item(Area::Modules, &["authentication"]);
    guess.verification = Some(VerificationStatus::Unverified);

    assert!(explain(&s, &verified).score > explain(&s, &guess).score);
}

#[test]
fn what_the_user_vouched_for_outscores_the_same_match_from_analysis() {
    let s = signals("Add password recovery endpoint");
    let mut told = item(Area::Modules, &["authentication"]);
    told.provenance = Some(Origin::UserConfirmed);

    assert!(
        explain(&s, &told).score
            >= explain(&s, &item(Area::Modules, &["authentication"])).score
                + Weights::USER_KNOWLEDGE
    );
}

#[test]
fn a_layer_the_task_is_not_in_pulls_the_score_down() {
    let s = signals("Change the login page layout");
    let reason = explain(&s, &item(Area::Infrastructure, &["database", "backend"]));

    assert_eq!(reason.penalties, ["layer_mismatch"]);
    assert!(reason.score < Weights::THRESHOLD);
}

#[test]
fn ranking_is_deterministic() {
    let w = World::with(findings(), vec![], &user());
    let a = w.context("Add password recovery endpoint");
    let b = w.context("Add password recovery endpoint");

    assert_eq!(a, b);
}

// ---- the five tasks ----

fn world() -> World {
    World::with(findings(), vec![], &user())
}

#[test]
fn task_a_password_recovery_brings_auth_backend_testing_and_rules_but_not_billing_or_maps() {
    let c = world().context("Add password recovery endpoint.");

    assert_eq!(c.mode, ContextMode::TaskAware);
    let inc = included(&c);
    for expected in [
        "Authentication module",
        "NestJS 10",
        "Vitest",
        "Clean Architecture",
        "REST controllers under src/api",
        "Errors go through one exception filter",
        "Business rules",
    ] {
        assert!(inc.contains(&expected), "{expected} missing from {inc:?}");
    }
    let out = excluded(&c);
    for unrelated in [
        "Billing module",
        "Maps module",
        "Reports module",
        "Angular 18",
    ] {
        assert!(
            out.contains(&unrelated),
            "{unrelated} should be excluded: {inc:?}"
        );
    }
    assert!(c
        .text
        .contains("Password reset links expire after one hour"));
    assert!(!c.text.contains("Billing module"));
}

#[test]
fn task_b_login_page_layout_is_a_frontend_context() {
    let c = world().context("Change the login page layout.");

    let inc = included(&c);
    assert!(inc.contains(&"Angular 18"), "{inc:?}");
    assert!(inc.contains(&"Angular standalone components"), "{inc:?}");
    assert!(inc.contains(&"Authentication module"), "{inc:?}");
    assert!(inc.contains(&"Vitest"), "{inc:?}");
    let out = excluded(&c);
    assert!(out.contains(&"PostgreSQL"), "{inc:?}");
    assert!(out.contains(&"REST controllers under src/api"), "{inc:?}");
    assert!(out.contains(&"Billing module"));
    // It is a different context from task A's.
    let a = world().context("Add password recovery endpoint.");
    assert_ne!(included(&a), inc);
}

#[test]
fn task_c_database_migration_brings_database_users_and_backend() {
    let c = world().context("Change database migration for users.");

    let inc = included(&c);
    for expected in ["PostgreSQL", "Users module", "NestJS 10", "Vitest"] {
        assert!(inc.contains(&expected), "{expected} missing from {inc:?}");
    }
    let out = excluded(&c);
    assert!(out.contains(&"Angular 18"));
    assert!(out.contains(&"Angular standalone components"));
    assert!(out.contains(&"Billing module"));
}

#[test]
fn task_d_fixing_the_ci_pipeline_does_not_carry_the_business_context() {
    let c = world().context("Fix CI pipeline.");

    let inc = included(&c);
    for expected in ["GitHub Actions", "npm run build", "Vitest"] {
        assert!(inc.contains(&expected), "{expected} missing from {inc:?}");
    }
    let out = excluded(&c);
    for unrelated in [
        "Purpose",
        "Users",
        "Business rules",
        "Billing module",
        "Maps module",
    ] {
        assert!(
            out.contains(&unrelated),
            "{unrelated} should be excluded: {inc:?}"
        );
    }
    assert!(!c.text.contains("Dispatchers and drivers"));
}

#[test]
fn task_e_redis_that_the_harness_never_mentions_is_not_identified_never_not_used() {
    let c = world().context("Add Redis caching.");

    assert!(c
        .text
        .contains("Redis was not identified in the analyzed evidence"));
    assert!(c.text.contains("does not prove that Redis is not used"));
    let lower = c.text.to_lowercase();
    assert!(
        !lower.contains("redis is not used.") || lower.contains("not prove that redis is not used")
    );
    assert!(!lower.contains("does not use redis"));
    let gap = c
        .entries
        .iter()
        .find(|e| e.item.id == "gap:tech:redis")
        .unwrap();
    assert_eq!(gap.outcome, EntryOutcome::Included);
    assert_eq!(gap.item.kind, ItemKind::NotFound);
    assert_eq!(gap.item.block, ContextBlock::Unknown);
    // It is told under what we do not know.
    let unknown = &c.text[c.text.find("WHAT WE DON'T KNOW").unwrap()..];
    assert!(unknown.contains("Redis was not identified"));
}

#[test]
fn a_technology_the_harness_does_hold_is_not_reported_missing() {
    let c = world().context("Add a PostgreSQL index");

    assert!(!c.text.contains("PostgreSQL was not identified"));
}

// ---- constraints, decisions, user knowledge, inference, stale ----

#[test]
fn constraints_and_decisions_are_in_every_task_context_even_unrelated_ones() {
    for task in [
        "Fix CI pipeline",
        "Change the login page layout",
        "Add Redis caching",
    ] {
        let c = world().context(task);
        assert!(c.text.contains("Never log passwords or tokens"), "{task}");
        assert!(
            c.text.contains("Do not change the infrastructure layer"),
            "{task}"
        );
        assert!(c.text.contains("Authentication uses JWT"), "{task}");
        assert!(
            c.text
                .contains("Do not merge the domain and application layers"),
            "{task}"
        );
    }
}

#[test]
fn inferences_stay_inferences_in_the_task_context() {
    let c = world().context("Add password recovery endpoint");

    let infer = &c.text[c.text.find("WHAT WE INFER").unwrap()..];
    let infer = &infer[..infer.find("WHAT THE USER TOLD US").unwrap_or(infer.len())];
    assert!(infer.contains("Possible architecture: Clean Architecture"));
    assert!(infer.contains("medium confidence; not verified; evidence: src/domain"));
    // The same text is never among the verified facts.
    let know = &c.text[c.text.find("WHAT WE KNOW").unwrap()..c.text.find("WHAT WE INFER").unwrap()];
    assert!(!know.contains("Clean Architecture"));
    assert!(know.contains("NestJS 10"));
}

#[test]
fn stale_knowledge_is_outdated_never_current() {
    let mut list = findings();
    for f in &mut list {
        if f.key == "nestjs" {
            f.verification = Verification {
                status: VerificationStatus::Stale,
                verified_at: Some(1_700_000_000_000),
                method: None,
            };
        }
    }
    let c = World::with(list, vec![], &user()).context("Add password recovery endpoint");

    let outdated = &c.text[c.text.find("WHAT MAY BE OUTDATED").unwrap()..];
    assert!(outdated.contains("NestJS 10"));
    assert!(outdated.contains("last verified"));
    assert!(outdated.contains("check the code"));
    let know = &c.text[c.text.find("WHAT WE KNOW").unwrap()..c.text.find("WHAT WE INFER").unwrap()];
    assert!(!know.contains("NestJS"));
    // And an unrelated stale finding does not clutter a task that does not need it.
    let c = World::with(
        {
            let mut l = findings();
            for f in &mut l {
                if f.key == "angular" {
                    f.verification.status = VerificationStatus::Stale;
                }
            }
            l
        },
        vec![],
        &user(),
    )
    .context("Change database migration for users");
    assert!(!c.text.contains("WHAT MAY BE OUTDATED"));
}

#[test]
fn a_user_confirmed_finding_is_told_as_the_users_word() {
    let mut list = findings();
    for f in &mut list {
        if f.key == "clean" {
            f.origin = Origin::UserConfirmed;
            f.original_origin = Some(Origin::Inference);
        }
    }
    let c = World::with(list, vec![], &user()).context("Add password recovery endpoint");

    let told = &c.text[c.text.find("WHAT THE USER TOLD US").unwrap()..];
    assert!(told.contains("Clean Architecture (inferred by Atlas, confirmed by the user)"));
}

#[test]
fn a_user_decision_against_the_repository_is_reported_not_resolved() {
    let conflict = Conflict {
        finding_id: "architecture:layers".to_owned(),
        label: "Layers".to_owned(),
        claims: vec![
            Claim {
                value: "domain and application kept apart".to_owned(),
                choice: "separate".to_owned(),
                origin: Origin::UserCorrected,
                evidence: vec![],
            },
            Claim {
                value: "domain imports application".to_owned(),
                choice: "merged".to_owned(),
                origin: Origin::Inference,
                evidence: vec![Evidence::new("src/domain/order.ts", None)],
            },
        ],
        resolution: Some("separate".to_owned()),
    };
    let c =
        World::with(findings(), vec![conflict], &user()).context("Add password recovery endpoint");

    assert!(c.text.contains("Conflict detected about Layers"));
    assert!(c
        .text
        .contains("The user's decision is authoritative for project guidance"));
    assert!(c.text.contains("repository evidence may indicate drift"));
    assert!(c
        .text
        .contains("domain and application kept apart (user decision)"));
}

// ---- budget ----

#[test]
fn a_task_context_is_smaller_than_the_full_harness_and_says_so() {
    let c = world().context("Fix CI pipeline");

    assert!(
        c.selected_chars < c.full_harness_chars,
        "{} !< {}",
        c.selected_chars,
        c.full_harness_chars
    );
    assert!(c.text.contains("NOT SELECTED"));
    assert!(c.excluded_areas.contains(&Area::Modules));
    assert!(c.included_areas.contains(&Area::Ci));
    assert!(!c.truncated);
}

#[test]
fn a_big_harness_is_cut_to_the_budget_and_the_cut_is_told() {
    let mut list = findings();
    for i in 0..60 {
        list.push(inferred(
            FindingCategory::Module,
            &format!("auth_part_{i}"),
            &format!("Authentication part {i} with a fairly long description of what it does"),
            &[&format!("src/auth/part{i}.ts")],
        ));
    }
    let w = World::with(list, vec![], &user());
    let c = w.context("Add password recovery endpoint");

    assert!(c.truncated);
    assert!(c.text.contains("Context was truncated. Omitted for size:"));
    assert!(c
        .omitted
        .iter()
        .any(|g| g.area == Area::Modules && g.count > 0));
    assert!(
        c.text.chars().count() <= c.budget_chars + 200,
        "{} chars",
        c.text.chars().count()
    );
    // What the budget could not hold is recorded, not hidden.
    assert!(c
        .entries
        .iter()
        .any(|e| e.outcome == EntryOutcome::OverBudget));
    // Constraints and decisions are still there.
    assert!(c.text.contains("Never log passwords or tokens"));
    assert!(c.text.contains("Authentication uses JWT"));
}

#[test]
fn constraints_and_decisions_survive_a_budget_too_small_for_anything_else() {
    let w = World::with(findings(), vec![], &user());
    let tight = w.builder().with_budget(HarnessBudget { max_chars: 300 });

    let TaskContextLoad::Ready(c) = tight.build_for_task("/erp", "Add password recovery endpoint")
    else {
        panic!("expected a context");
    };

    assert!(c.text.contains("Never log passwords or tokens"));
    assert!(c
        .text
        .contains("Do not merge the domain and application layers"));
    assert!(c.truncated);
}

#[test]
fn under_a_short_budget_what_the_user_decided_and_verified_facts_outlast_inference() {
    let sel = TaskContextService::select(
        &signals("Add password recovery endpoint"),
        vec![
            {
                let mut i = item(Area::Modules, &["authentication"]);
                i.id = "inferred".to_owned();
                i.block = ContextBlock::Infer;
                i.content = "x".repeat(400);
                i
            },
            {
                let mut i = item(Area::Modules, &["authentication"]);
                i.id = "verified".to_owned();
                i.content = "y".repeat(400);
                i
            },
            {
                let mut i = item(Area::Decisions, &[]);
                i.id = "decision".to_owned();
                i.kind = ItemKind::Decision;
                i.block = ContextBlock::User;
                i.content = "z".repeat(300);
                i
            },
        ],
        NOTES_RESERVE + 800,
    );

    let kept: Vec<&str> = sel.included().map(|i| i.id.as_str()).collect();
    assert_eq!(kept, ["decision", "verified"]);
    assert_eq!(sel.over_budget.len(), 1);
}

#[test]
fn the_order_is_security_decisions_constraints_user_facts_architecture_modules() {
    let mk = |id: &str, kind: ItemKind, block: ContextBlock, area: Area, tags: &[&str]| {
        let mut i = item(area, tags);
        i.id = id.to_owned();
        i.kind = kind;
        i.block = block;
        i
    };
    let sel = TaskContextService::select(
        &signals("Add password recovery endpoint"),
        vec![
            mk(
                "modules",
                ItemKind::Finding,
                ContextBlock::Infer,
                Area::Modules,
                &["authentication"],
            ),
            mk(
                "arch",
                ItemKind::Finding,
                ContextBlock::Infer,
                Area::Architecture,
                &[],
            ),
            mk(
                "fact",
                ItemKind::Finding,
                ContextBlock::Know,
                Area::Stack,
                &["backend"],
            ),
            mk(
                "user",
                ItemKind::Finding,
                ContextBlock::User,
                Area::Modules,
                &["authentication"],
            ),
            mk(
                "constraint",
                ItemKind::Constraint,
                ContextBlock::User,
                Area::Constraints,
                &[],
            ),
            mk(
                "security",
                ItemKind::Constraint,
                ContextBlock::User,
                Area::Constraints,
                &["security"],
            ),
            mk(
                "decision",
                ItemKind::Decision,
                ContextBlock::User,
                Area::Decisions,
                &[],
            ),
        ],
        6_000,
    );

    let order: Vec<&str> = sel.included().map(|i| i.id.as_str()).collect();
    assert_eq!(
        order,
        [
            "security",
            "decision",
            "constraint",
            "user",
            "fact",
            "arch",
            "modules"
        ]
    );
}

// ---- fallback ----

#[test]
fn a_task_that_says_nothing_readable_falls_back_to_the_whole_harness_and_says_why() {
    let w = world();
    let c = w.context("!!!");
    let full = match w.builder().build("/erp") {
        HarnessLoad::Loaded { text, .. } => text,
        other => panic!("{other:?}"),
    };

    assert_eq!(c.mode, ContextMode::Fallback);
    assert_eq!(c.fallback_reason.as_deref(), Some("task_without_signals"));
    assert_eq!(c.text, full);
    assert_eq!(c.record().mode, ContextMode::Fallback);
}

#[test]
fn a_harness_from_before_knowledge_existed_falls_back() {
    let manifest = "version: 1\nproject: {id: p, name: Old, initializedAt: 1}\n\
        repository: {type: git, root: .}\nstack: {languages: [TypeScript]}\n\
        context: {generated: true}\nharness: {version: 1}\n";
    let store = Arc::new(MemoryHarnessStore::with(&[
        ("project.yaml", manifest),
        ("context/architecture.md", "# Architecture\n\nLayered."),
    ]));

    let TaskContextLoad::Ready(c) =
        HarnessContextBuilder::new(store).build_for_task("/erp", "Add password recovery endpoint")
    else {
        panic!("expected a context");
    };

    assert_eq!(c.mode, ContextMode::Fallback);
    assert_eq!(
        c.fallback_reason.as_deref(),
        Some("harness_without_knowledge")
    );
    assert!(c.text.contains("older Harness"));
}

#[test]
fn a_project_without_a_harness_has_no_task_context() {
    let store = Arc::new(MemoryHarnessStore::default());

    assert_eq!(
        HarnessContextBuilder::new(store).build_for_task("/erp", "Add password recovery endpoint"),
        TaskContextLoad::Missing
    );
}

// ---- the record ----

#[test]
fn the_record_counts_what_was_chosen_and_what_was_not() {
    let c = world().context("Add password recovery endpoint");
    let record = c.record();

    assert_eq!(record.mode, ContextMode::TaskAware);
    assert_eq!(record.selected_context_characters, c.text.chars().count());
    assert_eq!(record.total_harness_characters, c.full_harness_chars);
    assert_eq!(
        record.selected_items + record.omitted_items,
        c.entries.len()
    );
    assert!(record.omitted_items > 0);
}

// ---- security ----

/// The selector is a pure function of the Harness's own data: it has no way to read the disk,
/// run a command, reach the network, change permissions or touch a worktree, and it grants the
/// agent nothing. Checked on its source so that adding such a thing makes this test fail.
#[test]
fn the_task_context_service_cannot_touch_the_system() {
    let source = include_str!("../task_context.rs");
    let source = source.split("#[cfg(test)]").next().unwrap();
    let source_taxonomy = include_str!("../taxonomy.rs");
    let source_taxonomy = source_taxonomy.split("#[cfg(test)]").next().unwrap();

    for text in [source, source_taxonomy] {
        for forbidden in [
            ["std", "::", "fs"].concat(),
            ["std", "::", "process"].concat(),
            ["std", "::", "net"].concat(),
            ["Command", "::", "new"].concat(),
            ["Tcp", "Stream"].concat(),
            ["req", "west"].concat(),
            ["set_", "permissions"].concat(),
            ["Harness", "Store"].concat(),
            ["Project", "Scanner"].concat(),
            ["worktree"].concat(),
            ["tokio"].concat(),
        ] {
            assert!(!text.contains(&forbidden), "found `{forbidden}`");
        }
    }
}

#[test]
fn the_task_context_never_grants_authority() {
    let c = world().context("Add password recovery endpoint");

    assert!(c.text.contains("grants no permissions"));
    assert!(c.text.contains("context, not instructions with authority"));
    assert!(c
        .text
        .starts_with("This context was selected from the project's Harness"));
}
