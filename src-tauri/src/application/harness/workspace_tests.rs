#![allow(clippy::assert_is_empty)]
//! Several projects in one workspace: each keeps its own Harness, and the workspace has a
//! context of its own that relates them.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use super::context::HarnessContextBuilder;
use super::fake::{FakeScanner, MemoryHarnessStore};
use super::scope_context::{self, Access, FragmentOutcome, WorkspaceText};
use super::snapshot::ScanSnapshot;
use super::tests::angular_project;
use super::workspace_context::{
    told, ContextFile, MemoryContextStore, ProjectHarnessState, ProjectInitProgress,
    ProjectInitStatus, WorkspaceContextStore,
};
use super::{HarnessService, HarnessStore, ProjectScanner};
use crate::application::agents::AgentService;
use crate::application::config::memory::MemoryStore;
use crate::application::config::ConfigRepository;
use crate::application::errors::{AppError, ErrorCode};
use crate::application::personalities::PersonalityService;
use crate::application::projects::fake::FakeInspector;
use crate::application::runtimes::RuntimeRegistry;
use crate::application::workspace::{ProjectInput, WorkspaceInput, WorkspaceService};
use crate::domain::harness::{HarnessFile, HarnessStatus, InitMode, InitializeInput, WriteReport};
use crate::domain::scope::{ProjectScope, ScopeRequest};
use crate::domain::workspace::ProjectRole;

/// One `.atlas/` per project folder.
#[derive(Default)]
pub(crate) struct PerProjectStore(Mutex<HashMap<String, Arc<MemoryHarnessStore>>>);

impl PerProjectStore {
    pub(crate) fn of(&self, path: &str) -> Arc<MemoryHarnessStore> {
        self.0
            .lock()
            .unwrap()
            .entry(path.to_owned())
            .or_insert_with(|| Arc::new(MemoryHarnessStore::default()))
            .clone()
    }
}

impl HarnessStore for PerProjectStore {
    fn read_manifest(&self, path: &str) -> Result<Option<String>, AppError> {
        self.of(path).read_manifest(path)
    }
    fn has_atlas_dir(&self, path: &str) -> bool {
        self.of(path).has_atlas_dir(path)
    }
    fn read_context(&self, path: &str, name: &str) -> Option<String> {
        self.of(path).read_context(path, name)
    }
    fn read_file(&self, path: &str, file: &str) -> Result<Option<String>, AppError> {
        self.of(path).read_file(path, file)
    }
    fn write(&self, path: &str, files: &[HarnessFile]) -> Result<WriteReport, AppError> {
        self.of(path).write(path, files)
    }
    fn ensure_gitignored(
        &self,
        path: &str,
    ) -> Result<crate::domain::harness::GitIgnoreStatus, AppError> {
        self.of(path).ensure_gitignored(path)
    }
}

/// One scan per project folder; a folder with no entry is not there, one marked broken fails.
#[derive(Default)]
pub(crate) struct PerProjectScanner {
    pub(crate) snapshots: Mutex<HashMap<String, Arc<FakeScanner>>>,
    pub(crate) broken: Mutex<BTreeSet<String>>,
    scans: Mutex<BTreeMap<String, usize>>,
}

impl PerProjectScanner {
    pub(crate) fn set(&self, path: &str, snapshot: ScanSnapshot) {
        self.snapshots
            .lock()
            .unwrap()
            .insert(path.to_owned(), Arc::new(FakeScanner::new(snapshot)));
    }
    fn scans_of(&self, path: &str) -> usize {
        self.scans.lock().unwrap().get(path).copied().unwrap_or(0)
    }
    fn inner(&self, path: &str) -> Option<Arc<FakeScanner>> {
        self.snapshots.lock().unwrap().get(path).cloned()
    }
}

impl ProjectScanner for PerProjectScanner {
    fn scan(&self, path: &str) -> Result<ScanSnapshot, AppError> {
        *self
            .scans
            .lock()
            .unwrap()
            .entry(path.to_owned())
            .or_default() += 1;
        if self.broken.lock().unwrap().contains(path) {
            return Err(AppError::new(ErrorCode::ProjectAnalysisFailed));
        }
        self.inner(path)
            .ok_or_else(|| AppError::new(ErrorCode::ProjectNotFound))?
            .scan(path)
    }
    fn folder_exists(&self, path: &str) -> bool {
        self.inner(path).is_some()
    }
    fn exists(&self, path: &str, relative: &str) -> bool {
        self.inner(path).is_some_and(|s| s.exists(path, relative))
    }
    fn read_files(&self, path: &str, paths: &[String], max: usize) -> Vec<(String, String)> {
        self.inner(path)
            .map(|s| s.read_files(path, paths, max))
            .unwrap_or_default()
    }
}

pub(crate) fn backend_project(name: &str) -> ScanSnapshot {
    let mut s = ScanSnapshot {
        root_name: name.to_owned(),
        ..ScanSnapshot::default()
    };
    s.dir(".git");
    s.git_head = Some("ref: refs/heads/main".to_owned());
    s.file(
        "pom.xml",
        Some("<project><parent><artifactId>spring-boot-starter-parent</artifactId></parent></project>"),
    );
    s
}

pub(crate) struct Shop {
    pub(crate) service: HarnessService,
    pub(crate) workspaces: Arc<WorkspaceService>,
    pub(crate) agents: Arc<AgentService>,
    pub(crate) personalities: Arc<PersonalityService>,
    pub(crate) harnesses: Arc<PerProjectStore>,
    pub(crate) scanner: Arc<PerProjectScanner>,
    pub(crate) context: Arc<MemoryContextStore>,
    pub(crate) workspace_id: String,
    /// The project ids in the order they were added: front, back, orders.
    pub(crate) ids: Vec<String>,
}

pub(crate) const FRONT: &str = "/shop/front";
pub(crate) const BACK: &str = "/shop/back";
pub(crate) const ORDERS: &str = "/shop/orders";

fn shop() -> Shop {
    shop_with(Arc::new(RuntimeRegistry::new(vec![])))
}

/// A workspace `Shop` of three projects (front, API, Orders), each with its own folder, and the
/// services over them. Agents can only use the runtimes of `runtimes`.
pub(crate) fn shop_with(runtimes: Arc<RuntimeRegistry>) -> Shop {
    let config = Arc::new(ConfigRepository::load(Box::<MemoryStore>::default()));
    let personalities = Arc::new(PersonalityService::new(config.clone()));
    let agents = Arc::new(AgentService::new(
        config.clone(),
        personalities.clone(),
        runtimes,
    ));
    let workspaces = Arc::new(WorkspaceService::new(
        config,
        agents.clone(),
        Arc::new(FakeInspector::with(&[
            (FRONT, &["Angular"]),
            (BACK, &["Maven"]),
            (ORDERS, &["Maven"]),
        ])),
    ));
    let workspace = workspaces
        .create(&WorkspaceInput {
            name: "Shop".to_owned(),
            project_path: FRONT.to_owned(),
            description: Some("The online store".to_owned()),
        })
        .unwrap();
    let add = |path: &str, name: &str, role| {
        workspaces
            .add_project(
                &workspace.id,
                &ProjectInput {
                    path: path.to_owned(),
                    name: Some(name.to_owned()),
                    role: Some(role),
                    depends_on: Vec::new(),
                },
            )
            .unwrap()
    };
    add(BACK, "API", ProjectRole::Backend);
    let full = add(ORDERS, "Orders", ProjectRole::Service);
    let harnesses = Arc::new(PerProjectStore::default());
    let scanner = Arc::new(PerProjectScanner::default());
    scanner.set(FRONT, angular_project());
    scanner.set(BACK, backend_project("shop-api"));
    scanner.set(ORDERS, backend_project("orders-service"));
    let context = Arc::new(MemoryContextStore::default());
    let service = HarnessService::new(workspaces.clone(), scanner.clone(), harnesses.clone())
        .with_workspace_store(context.clone());
    Shop {
        service,
        workspaces,
        agents,
        personalities,
        harnesses,
        scanner,
        context,
        workspace_id: workspace.id,
        ids: full.projects.iter().map(|p| p.id.clone()).collect(),
    }
}

#[allow(clippy::needless_pass_by_value)]
fn quiet(_: super::workspace_context::ProjectInitUpdate) {}

fn run(shop: &Shop, only: Option<&[String]>) -> super::workspace_context::WorkspaceInitReport {
    shop.service
        .initialize_workspace(&shop.workspace_id, only, true, &quiet)
        .unwrap()
}

#[test]
fn every_project_gets_its_own_harness_and_the_workspace_gets_a_context() {
    let shop = shop();

    let report = run(&shop, None);

    assert!(report.complete);
    let statuses: Vec<_> = report.projects.iter().map(|p| p.status).collect();
    assert_eq!(statuses, [ProjectInitStatus::Initialized; 3]);
    // Each folder holds a manifest of its own project and nothing of another.
    let named = |path: &str| shop.harnesses.of(path).get("project.yaml").unwrap();
    assert!(named(FRONT).contains("transport-erp"));
    assert!(named(BACK).contains("shop-api") && !named(BACK).contains("transport-erp"));
    assert!(named(ORDERS).contains("orders-service") && !named(ORDERS).contains("shop-api"));
    // The workspace context lists all three, with roles and folders, outside every project.
    let text = shop
        .context
        .read(&shop.workspace_id, ContextFile::Generated)
        .unwrap();
    for expected in [
        "# Workspace: Shop",
        "### API — backend",
        "### Orders — service",
        FRONT,
        BACK,
        ORDERS,
    ] {
        assert!(text.contains(expected), "{expected}\n{text}");
    }
    assert!(report.context.is_some_and(|c| c.exists && !c.outdated));
    for path in [FRONT, BACK, ORDERS] {
        assert!(shop.harnesses.of(path).get("context.md").is_none());
    }
}

#[test]
fn one_project_failing_does_not_stop_the_others_and_the_run_is_not_called_complete() {
    let shop = shop();
    shop.scanner.broken.lock().unwrap().insert(BACK.to_owned());

    let report = run(&shop, None);

    let statuses: Vec<_> = report.projects.iter().map(|p| p.status).collect();
    assert_eq!(
        statuses,
        [
            ProjectInitStatus::Initialized,
            ProjectInitStatus::Failed,
            ProjectInitStatus::Initialized
        ]
    );
    assert!(!report.complete);
    assert_eq!(
        report.projects[1].error.as_ref().map(|e| e.code),
        Some(ErrorCode::ProjectAnalysisFailed)
    );
    // Nothing was written for the one that failed; the others have their Harness.
    assert!(shop.harnesses.of(BACK).get("project.yaml").is_none());
    assert!(shop.harnesses.of(FRONT).get("project.yaml").is_some());
    // The workspace context still exists and says that project has none.
    let text = shop
        .context
        .read(&shop.workspace_id, ContextFile::Generated)
        .unwrap();
    assert!(text.contains("### API — backend") && text.contains("not initialized"));
}

#[test]
fn running_again_after_a_failure_only_does_the_missing_work() {
    let shop = shop();
    shop.scanner.broken.lock().unwrap().insert(BACK.to_owned());
    run(&shop, None);
    shop.scanner.broken.lock().unwrap().clear();
    let manifest_before = shop.harnesses.of(FRONT).get("project.yaml");
    let knowledge_before = shop.harnesses.of(FRONT).get("knowledge/findings.yaml");

    let report = run(&shop, None);

    let statuses: Vec<_> = report.projects.iter().map(|p| p.status).collect();
    assert_eq!(
        statuses,
        [
            ProjectInitStatus::AlreadyInitialized,
            ProjectInitStatus::Initialized,
            ProjectInitStatus::AlreadyInitialized
        ]
    );
    assert!(report.complete);
    // The projects that were fine were not analysed again: their Harness was not written (nor
    // their `.gitignore` asked about) a second time, and is exactly what it was.
    for path in [FRONT, ORDERS] {
        assert_eq!(
            *shop.harnesses.of(path).ignored.lock().unwrap(),
            1,
            "{path}"
        );
    }
    assert_eq!(
        shop.harnesses.of(FRONT).get("project.yaml"),
        manifest_before
    );
    assert_eq!(
        shop.harnesses.of(FRONT).get("knowledge/findings.yaml"),
        knowledge_before
    );
}

#[test]
fn a_project_added_later_is_initialized_alone_and_the_context_catches_up() {
    let shop = shop();
    run(&shop, Some(&shop.ids[..2]));
    assert!(shop.harnesses.of(ORDERS).get("project.yaml").is_none());
    let front_manifest = shop.harnesses.of(FRONT).get("project.yaml");

    let report = run(&shop, Some(&shop.ids[2..]));

    assert_eq!(report.projects.len(), 1);
    assert_eq!(report.projects[0].status, ProjectInitStatus::Initialized);
    assert_eq!(shop.harnesses.of(FRONT).get("project.yaml"), front_manifest);
    assert_eq!(*shop.harnesses.of(FRONT).ignored.lock().unwrap(), 1);
    assert!(shop.harnesses.of(ORDERS).get("project.yaml").is_some());
    let text = shop
        .context
        .read(&shop.workspace_id, ContextFile::Generated)
        .unwrap();
    assert!(text.contains("### Orders — service"));
}

#[test]
fn progress_is_reported_for_each_project() {
    let shop = shop();
    let seen = Mutex::new(Vec::new());

    shop.service
        .initialize_workspace(&shop.workspace_id, None, true, &|update| {
            seen.lock().unwrap().push((update.project_id, update.state));
        })
        .unwrap();

    let seen = seen.into_inner().unwrap();
    for id in &shop.ids {
        let states: Vec<_> = seen
            .iter()
            .filter(|(p, _)| p == id)
            .map(|(_, s)| *s)
            .collect();
        assert_eq!(
            states,
            [
                ProjectInitProgress::Running,
                ProjectInitProgress::Initialized
            ]
        );
    }
}

#[test]
fn a_project_or_workspace_that_does_not_exist_is_refused_before_anything_runs() {
    let shop = shop();

    let stranger = shop
        .service
        .initialize_workspace(
            &shop.workspace_id,
            Some(&["ghost".to_owned()]),
            true,
            &quiet,
        )
        .unwrap_err();
    let nowhere = shop
        .service
        .initialize_workspace("nope", None, true, &quiet)
        .unwrap_err();

    assert!(stranger.is(ErrorCode::WorkspaceProjectNotFound));
    assert!(nowhere.is(ErrorCode::WorkspaceNotFound));
    assert_eq!(shop.scanner.scans_of(FRONT), 0);
}

#[test]
fn a_project_is_analysed_and_read_on_its_own() {
    let shop = shop();

    let analysis = shop
        .service
        .analyze_in(&shop.workspace_id, Some(&shop.ids[1]), None)
        .unwrap();
    let outcome = shop
        .service
        .initialize_in(
            &shop.workspace_id,
            Some(&shop.ids[1]),
            InitializeInput {
                mode: InitMode::Create,
                ..InitializeInput::default()
            },
        )
        .unwrap();

    assert_eq!(analysis.project_name, "shop-api");
    assert_eq!(analysis.path, BACK);
    assert_eq!(outcome.summary.status, HarnessStatus::Initialized);
    // Asked without a project, it is the primary one, still not initialized.
    assert_eq!(
        shop.service.get(&shop.workspace_id).unwrap().status,
        HarnessStatus::NotInitialized
    );
    assert_eq!(
        shop.service
            .get_in(&shop.workspace_id, Some(&shop.ids[1]))
            .unwrap()
            .status,
        HarnessStatus::Initialized
    );
    assert!(shop
        .service
        .get_in(&shop.workspace_id, Some("ghost"))
        .unwrap_err()
        .is(ErrorCode::WorkspaceProjectNotFound));
}

#[test]
fn two_projects_reviewed_at_once_do_not_swap_their_analyses() {
    let shop = shop();
    shop.service
        .analyze_in(&shop.workspace_id, Some(&shop.ids[0]), None)
        .unwrap();
    shop.service
        .analyze_in(&shop.workspace_id, Some(&shop.ids[1]), None)
        .unwrap();

    for id in [&shop.ids[1], &shop.ids[0]] {
        shop.service
            .initialize_in(
                &shop.workspace_id,
                Some(id),
                InitializeInput {
                    mode: InitMode::Create,
                    ..InitializeInput::default()
                },
            )
            .unwrap();
    }

    assert!(shop
        .harnesses
        .of(FRONT)
        .get("project.yaml")
        .unwrap()
        .contains("transport-erp"));
    assert!(shop
        .harnesses
        .of(BACK)
        .get("project.yaml")
        .unwrap()
        .contains("shop-api"));
}

#[test]
fn the_overview_says_where_each_project_stands() {
    let shop = shop();
    run(&shop, Some(&shop.ids[..1]));
    // The third project's folder is gone.
    shop.scanner.snapshots.lock().unwrap().remove(ORDERS);

    let overview = shop.service.workspace_overview(&shop.workspace_id).unwrap();

    let states: Vec<_> = overview.projects.iter().map(|p| p.state).collect();
    assert_eq!(
        states,
        [
            ProjectHarnessState::Ready,
            ProjectHarnessState::NotInitialized,
            ProjectHarnessState::FolderMissing
        ]
    );
    assert_eq!(overview.projects[0].project_id, shop.ids[0]);
    assert!(!overview.projects[0].summary.stack.is_empty());
}

#[test]
fn the_context_is_outdated_when_a_project_changes_and_current_after_rebuilding() {
    let shop = shop();
    run(&shop, None);
    assert!(
        !shop
            .service
            .workspace_overview(&shop.workspace_id)
            .unwrap()
            .context
            .outdated
    );

    shop.workspaces
        .update_project(
            &shop.workspace_id,
            &shop.ids[1],
            &crate::application::workspace::ProjectPatch {
                name: "Billing API".to_owned(),
                path: BACK.to_owned(),
                role: Some(ProjectRole::Backend),
                depends_on: vec![shop.ids[2].clone()],
            },
        )
        .unwrap();
    assert!(
        shop.service
            .workspace_overview(&shop.workspace_id)
            .unwrap()
            .context
            .outdated
    );

    let rebuilt = shop
        .service
        .build_workspace_context(&shop.workspace_id)
        .unwrap();

    assert!(!rebuilt.outdated);
    let text = shop
        .context
        .read(&shop.workspace_id, ContextFile::Generated)
        .unwrap();
    assert!(text.contains("### Billing API — backend"));
    assert!(text.contains("Billing API depends on Orders"));
}

#[test]
fn the_users_notes_are_never_touched_by_regenerating_and_reach_the_prompt_text() {
    let shop = shop();
    shop.service
        .save_workspace_notes(&shop.workspace_id, "  Orders owns the price rules.  ")
        .unwrap();

    run(&shop, None);
    shop.service
        .build_workspace_context(&shop.workspace_id)
        .unwrap();

    assert_eq!(
        shop.service.workspace_notes(&shop.workspace_id),
        "Orders owns the price rules."
    );
    let text = told(
        shop.context
            .read(&shop.workspace_id, ContextFile::Generated)
            .as_deref(),
        shop.context
            .read(&shop.workspace_id, ContextFile::Notes)
            .as_deref(),
    )
    .unwrap();
    assert!(text.contains("### API — backend"));
    assert!(text.ends_with("Notes from the user:\nOrders owns the price rules."));
    // The marker line is Atlas's bookkeeping, not something an agent is told.
    assert!(!text.contains("atlas:workspace-context"));
}

#[test]
fn a_workspace_with_one_project_has_no_workspace_context() {
    let shop = shop();
    for id in &shop.ids[1..] {
        shop.workspaces
            .remove_project(&shop.workspace_id, id)
            .unwrap();
    }

    let info = shop
        .service
        .build_workspace_context(&shop.workspace_id)
        .unwrap();

    assert!(!info.exists);
    assert_eq!(
        shop.context
            .read(&shop.workspace_id, ContextFile::Generated),
        None
    );
}

#[test]
fn deleting_the_workspace_forgets_its_context_and_leaves_the_projects_alone() {
    let shop = shop();
    run(&shop, None);

    shop.service.forget_workspace(&shop.workspace_id);

    assert_eq!(
        shop.context
            .read(&shop.workspace_id, ContextFile::Generated),
        None
    );
    assert!(shop.harnesses.of(FRONT).get("project.yaml").is_some());
}

// ---- what an execution is told ----

fn builder(shop: &Shop) -> HarnessContextBuilder {
    HarnessContextBuilder::new(shop.harnesses.clone())
}

fn scope_of(shop: &Shop, working: usize, read: &[usize]) -> ProjectScope {
    let workspace = shop.workspaces.get(&shop.workspace_id).unwrap();
    ProjectScope::resolve(
        &workspace,
        Some(&ScopeRequest {
            working_project_id: Some(shop.ids[working].clone()),
            read_project_ids: read.iter().map(|i| shop.ids[*i].clone()).collect(),
        }),
    )
    .unwrap()
}

#[test]
fn each_projects_harness_is_told_under_its_own_label_with_the_workspace_context_first() {
    let shop = shop();
    run(&shop, None);
    let scope = scope_of(&shop, 0, &[1, 2]);
    let generated = shop
        .context
        .read(&shop.workspace_id, ContextFile::Generated);
    let workspace_text = told(generated.as_deref(), None).unwrap();

    let built = scope_context::build(
        &builder(&shop),
        &scope,
        Some(WorkspaceText {
            name: "Shop",
            text: &workspace_text,
        }),
        "change the checkout form",
        6_000,
    );

    let text = built.text.unwrap();
    let at = |needle: &str| {
        text.find(needle)
            .unwrap_or_else(|| panic!("{needle}\n{text}"))
    };
    // The workspace first, then the working project, then the others in the workspace's order.
    assert!(at("WORKSPACE CONTEXT — Shop") < at("PROJECT — front (working project)"));
    assert!(at("PROJECT — front") < at("PROJECT — API (backend, read-only)"));
    assert!(at("PROJECT — API") < at("PROJECT — Orders (service, read-only)"));
    // Every fragment says which folder its paths are relative to.
    assert!(text.contains(&format!("at {BACK}")) && text.contains("relative to that folder"));
    let accesses: Vec<_> = built.fragments.iter().map(|f| f.access).collect();
    assert_eq!(
        accesses,
        [Access::Working, Access::ReadOnly, Access::ReadOnly]
    );
    assert!(built
        .fragments
        .iter()
        .all(|f| matches!(f.outcome, FragmentOutcome::Included { .. })));
    // The whole thing stays within the one allocation the Harness section has.
    assert!(
        text.chars().count() <= 6_000 + 600,
        "{}",
        text.chars().count()
    );
}

#[test]
fn a_project_with_no_harness_is_said_to_have_none_and_the_rest_still_arrive() {
    let shop = shop();
    run(&shop, Some(&shop.ids[..1]));
    let scope = scope_of(&shop, 0, &[1]);

    let built = scope_context::build(&builder(&shop), &scope, None, "checkout form", 6_000);

    let text = built.text.unwrap();
    assert!(text.contains("PROJECT — API"));
    assert!(text.contains("has no Harness yet"));
    assert_eq!(built.fragments[1].outcome, FragmentOutcome::Missing);
    assert!(matches!(
        built.fragments[0].outcome,
        FragmentOutcome::Included { .. }
    ));
}

#[test]
fn nothing_to_tell_is_no_text() {
    let shop = shop();
    let scope = scope_of(&shop, 0, &[]);

    let built = scope_context::build(&builder(&shop), &scope, None, "anything", 6_000);

    assert_eq!(built.text, None);
    assert!(built.record.is_none());
}

#[test]
fn what_one_project_knows_does_not_leak_into_another_projects_fragment() {
    let shop = shop();
    run(&shop, None);
    let only_front = scope_of(&shop, 0, &[]);

    let built = scope_context::build(&builder(&shop), &only_front, None, "checkout form", 6_000);

    let text = built.text.unwrap();
    assert!(!text.contains("shop-api") && !text.contains("orders-service"));
    assert!(!text.contains("PROJECT — API"));
}

#[test]
fn the_sizes_of_the_other_projects_are_added_to_the_working_projects_record() {
    let shop = shop();
    run(&shop, None);
    let alone = scope_context::build(
        &builder(&shop),
        &scope_of(&shop, 0, &[]),
        None,
        "checkout form",
        6_000,
    );
    let together = scope_context::build(
        &builder(&shop),
        &scope_of(&shop, 0, &[1, 2]),
        None,
        "checkout form",
        6_000,
    );

    let (alone, together) = (alone.record.unwrap(), together.record.unwrap());

    assert!(together.total_harness_characters > alone.total_harness_characters);
    assert!(together.selected_items >= alone.selected_items);
}
