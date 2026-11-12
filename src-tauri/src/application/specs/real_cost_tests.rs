//! C1 of the ADR checklist (ADR 0036): what a spec costs, with a real model, and what
//! optimizations 3 (a change request answered with section edits) and 5 (a second review that
//! knows what changed) save. Both are always on, so "before" is measured inside the same spec:
//!
//! - optimization 3: the same kind of change to `design.md`, once as a section edit (`Revise`) and
//!   once as a whole rewrite (`Redo`), on the same document;
//! - optimization 5: the first review, then a change, then the second review.
//!
//! The whole of Atlas runs (spec service, runner, chat, executions, guard, the real CLI); the
//! numbers are the ones the runtime reports, read from the usage ledger, never estimated.
//!
//! It costs tokens (about ten turns) and runs only by exact name:
//! `cargo test --lib application::specs::real_cost_tests::sdd_real_cost_before_and_after -- --ignored --exact --nocapture`.
//! `ATLAS_C1_RUNTIME` (default `claude`) and `ATLAS_C1_MODEL` (default `sonnet`) pick the agent.

use std::sync::Arc;

use super::openspec::OpenSpecAdapter;
use super::ports::{MemoryProjectFiles, MemorySpecStore};
use super::runner::SpecRunner;
use super::service::{NewSpecRequest, NoEvents, SpecService};
use crate::application::agents::{AgentService, CreateAgentRequest};
use crate::application::chat::{ChatObserver, ChatService};
use crate::application::config::memory::MemoryStore;
use crate::application::config::ConfigRepository;
use crate::application::executions::{ExecutionObserver, ExecutionService};
use crate::application::history::ConversationHistory;
use crate::application::personalities::PersonalityService;
use crate::application::process::ProcessRunner;
use crate::application::projects::fake::FakeInspector;
use crate::application::runtimes::RuntimeRegistry;
use crate::application::security::sandbox::NoSandbox;
use crate::application::security::testutil::TempDir;
use crate::application::security::{ApprovalBroker, AuditLog, GuardedProcessRunner};
use crate::application::sessions::SessionRegistry;
use crate::application::usage::UsageLedger;
use crate::application::workspace::{WorkspaceInput, WorkspaceService};
use crate::domain::execution::ExecutionEvent;
use crate::domain::spec::{DecisionKind, QuestionStatus};

struct Quiet;

impl ExecutionObserver for Quiet {
    fn on_event(&self, _event: &ExecutionEvent) {}
}

impl ChatObserver for Quiet {
    fn on_message(&self, _message: &crate::domain::conversation::Message) {}
}

/// A small project for the agents to read: a settings screen of a web app.
const PROJECT: &[(&str, &str)] = &[
    (
        "README.md",
        "# Notes app\n\nA small web app to write notes. Front end in `web/`, API in `api/`.\n",
    ),
    (
        "web/settings.js",
        "export function renderSettings(user) {\n  return `<h1>Settings</h1><p>${user.name}</p>`;\n}\n",
    ),
    (
        "web/theme.css",
        "body { background: #fff; color: #222; }\n.button { background: #0a66c2; color: #fff; }\n",
    ),
    (
        "api/users.js",
        "const users = new Map();\nexport function getUser(id) { return users.get(id); }\nexport function saveUser(user) { users.set(user.id, user); }\n",
    ),
];

struct World {
    specs: Arc<SpecService>,
    runner: SpecRunner,
    ledger: Arc<UsageLedger>,
    workspace_id: String,
    author: String,
    reviewer: String,
    _dir: TempDir,
}

fn world(runtime_id: &str, model: &str) -> World {
    let dir = TempDir::new("real-sdd");
    let project = dir.path().join("project");
    for (name, content) in PROJECT {
        let file = project.join(name);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, content).unwrap();
    }
    let path = project.to_string_lossy().into_owned();
    let config = Arc::new(ConfigRepository::load(Box::<MemoryStore>::default()));
    let security = Arc::new(crate::application::security::SecurityService::new(
        config.clone(),
    ));
    let runner: Arc<dyn ProcessRunner> = Arc::new(GuardedProcessRunner::new(
        Arc::new(crate::infrastructure::SystemProcessRunner::new()),
        security.clone(),
        Arc::new(ApprovalBroker::new()),
        Arc::new(AuditLog::default()),
        Arc::new(NoSandbox),
        crate::application::runtimes::RUNTIME_PROGRAMS
            .map(str::to_owned)
            .to_vec(),
    ));
    let registry = Arc::new(RuntimeRegistry::with_default_runtimes(&runner));
    let personalities = Arc::new(PersonalityService::new(config.clone()));
    let agents = Arc::new(AgentService::new(
        config.clone(),
        personalities.clone(),
        registry.clone(),
    ));
    let create = |name: &str| {
        agents
            .create(CreateAgentRequest {
                permission_profile_id: Some("developer".to_owned()),
                name: name.to_owned(),
                personality_id: "architect".to_owned(),
                runtime_id: runtime_id.to_owned(),
                model_id: model.to_owned(),
                instructions: String::new(),
                worktree_isolation: Some(false),
                result_contract: None,
            })
            .unwrap()
            .id
    };
    let author = create("Writer");
    let reviewer = create("Reviewer");
    let workspaces = Arc::new(WorkspaceService::new(
        config.clone(),
        agents.clone(),
        Arc::new(FakeInspector::with(&[(path.as_str(), &["JavaScript"])])),
    ));
    let workspace = workspaces
        .create(&WorkspaceInput {
            name: "Notes".to_owned(),
            project_path: path,
            description: None,
        })
        .unwrap();
    workspaces.add_agent(&workspace.id, &author).unwrap();
    workspaces.add_agent(&workspace.id, &reviewer).unwrap();
    let executions = Arc::new(
        ExecutionService::new(
            agents.clone(),
            personalities,
            registry,
            workspaces.clone(),
            Arc::new(AuditLog::default()),
        )
        .with_policies(security),
    );
    let ledger = Arc::new(UsageLedger::new(config.clone()));
    let chat = ChatService::with_history(
        agents.clone(),
        executions,
        workspaces.clone(),
        ledger.clone(),
        Arc::new(ConversationHistory::new(config)),
    );
    let specs = Arc::new(
        SpecService::new(
            Arc::new(MemorySpecStore::default()),
            Arc::new(MemoryProjectFiles::default()),
            Arc::new(OpenSpecAdapter),
            workspaces,
            agents,
            Arc::new(NoEvents),
        )
        .with_usage(ledger.clone()),
    );
    let runner = SpecRunner::new(specs.clone(), chat, Arc::new(SessionRegistry::default()));
    World {
        specs,
        runner,
        ledger,
        workspace_id: workspace.id,
        author,
        reviewer,
        _dir: dir,
    }
}

/// One turn, labelled for the table: the run's id, so its usage can be found.
struct Turn {
    label: String,
    run_id: String,
}

fn header() {
    eprintln!(
        "\n| turn | outcome | input | of it cached | output | cost (USD) |\n| --- | --- | --- | --- | --- | --- |"
    );
}

impl World {
    fn latest_run(&self, id: &str) -> String {
        self.specs
            .get(id)
            .unwrap()
            .runs
            .last()
            .map(|r| r.id.clone())
            .unwrap()
    }

    /// Writes `artifact`, answering once if the agent stops to ask (the answer tells it to choose
    /// the simplest option): every turn is kept for the table.
    fn generate(&self, id: &str, artifact: &str, label: &str, turns: &mut Vec<Turn>) {
        for attempt in 0..2 {
            self.runner
                .prepare_generation(id, artifact)
                .unwrap_or_else(|e| panic!("{label}: cannot start: {e:?}"))
                .run(&Quiet);
            let turn = Turn {
                label: if attempt == 0 {
                    label.to_owned()
                } else {
                    format!("{label} (after the answer)")
                },
                run_id: self.latest_run(id),
            };
            eprintln!("{}", row(self, id, &turn));
            turns.push(turn);
            let spec = self.specs.get(id).unwrap();
            let open = spec
                .questions
                .iter()
                .find(|q| q.status == QuestionStatus::Pending)
                .map(|q| q.id.clone());
            match open {
                Some(question) => {
                    self.specs
                        .answer_question(
                            id,
                            &question,
                            None,
                            Some("Choose the simplest option and go on; do not ask again."),
                        )
                        .unwrap();
                }
                None => return,
            }
        }
    }

    fn approve(&self, id: &str, artifact: &str) {
        let revision = self.revision(id, artifact);
        self.specs
            .decide(id, artifact, revision, DecisionKind::Approve, None)
            .unwrap_or_else(|e| panic!("approve {artifact}: {e:?}"));
    }

    fn revision(&self, id: &str, artifact: &str) -> u32 {
        self.specs
            .get(id)
            .unwrap()
            .artifact(artifact)
            .unwrap()
            .latest_number()
            .unwrap_or_else(|| panic!("{artifact} has no revision: {:?}", self.state(id, artifact)))
    }

    fn state(&self, id: &str, artifact: &str) -> String {
        format!(
            "{:?}",
            self.specs
                .get(id)
                .unwrap()
                .artifact(artifact)
                .unwrap()
                .generation
        )
    }

    fn change(
        &self,
        id: &str,
        artifact: &str,
        kind: DecisionKind,
        feedback: &str,
        label: &str,
        turns: &mut Vec<Turn>,
    ) {
        let revision = self.revision(id, artifact);
        self.specs
            .decide(id, artifact, revision, kind, Some(feedback))
            .unwrap_or_else(|e| panic!("{label}: {e:?}"));
        self.generate(id, artifact, label, turns);
    }

    /// Keeps every outdated document as it is, so the review can run.
    fn keep_outdated(&self, id: &str) {
        let spec = self.specs.get(id).unwrap();
        for artifact in &spec.artifacts {
            if artifact.outdated.is_some() {
                let revision = artifact.latest_number().unwrap();
                self.specs
                    .keep_outdated(id, &artifact.id, revision)
                    .unwrap_or_else(|e| panic!("keep {}: {e:?}", artifact.id));
                // Keeping it puts it back in front of the person: approved again, as they would.
                self.approve(id, &artifact.id);
            }
        }
    }

    fn review(&self, id: &str, label: &str, turns: &mut Vec<Turn>) {
        self.runner
            .prepare_review(id)
            .unwrap_or_else(|e| panic!("{label}: cannot start: {e:?}"))
            .run(&Quiet);
        let turn = Turn {
            label: label.to_owned(),
            run_id: self.latest_run(id),
        };
        eprintln!("{}", row(self, id, &turn));
        turns.push(turn);
    }
}

fn row(w: &World, id: &str, turn: &Turn) -> String {
    let spec = w.specs.get(id).unwrap();
    let outcome = spec
        .runs
        .iter()
        .find(|r| r.id == turn.run_id)
        .and_then(|r| r.outcome);
    let record = w.ledger.records_of(&[turn.run_id.as_str()]).pop();
    let metrics = record.and_then(|r| r.metrics);
    let show = |v: Option<u64>| v.map_or_else(|| "n/d".to_owned(), |n| n.to_string());
    match metrics {
        Some(m) => format!(
            "| {} | {:?} | {} | {} | {} | {} |",
            turn.label,
            outcome,
            show(m.input_tokens),
            show(m.cached_input_tokens),
            show(m.output_tokens),
            m.cost
                .map_or_else(|| "n/d".to_owned(), |c| format!("{c:.4}"))
        ),
        None => format!("| {} | {outcome:?} | n/d | n/d | n/d | n/d |", turn.label),
    }
}

#[test]
#[ignore = "calls a real model (tokens): about ten spec turns; run by exact name"]
fn sdd_real_cost_before_and_after() {
    let runtime = std::env::var("ATLAS_C1_RUNTIME").unwrap_or_else(|_| "claude".to_owned());
    let model = std::env::var("ATLAS_C1_MODEL").unwrap_or_else(|_| "sonnet".to_owned());
    let w = world(&runtime, &model);
    let view = w
        .specs
        .create(NewSpecRequest {
            workspace_id: w.workspace_id.clone(),
            project_id: None,
            title: "Dark mode".to_owned(),
            request: "Add a dark mode to the settings screen: a switch that changes the colours \
of the whole app and is remembered for the user."
                .to_owned(),
            context: "Web app only. Keep it small: no new dependencies. Make reasonable \
assumptions instead of asking."
                .to_owned(),
            author_agent_id: w.author.clone(),
            reviewer_agent_id: Some(w.reviewer.clone()),
            integrations: Vec::new(),
            skip_specs: false,
        })
        .unwrap();
    let id = view.spec.id;
    w.specs.start(&id).unwrap();
    let mut turns = Vec::new();
    eprintln!("[C1] runtime={runtime} model={model}");
    header();

    for artifact in ["proposal", "specs", "design", "tasks"] {
        w.generate(&id, artifact, &format!("write {artifact}"), &mut turns);
        w.approve(&id, artifact);
    }
    // Optimization 3: one change to a single section, answered with an edit, then the same kind
    // of change asked as a whole rewrite of the same document.
    w.change(
        &id,
        "design",
        DecisionKind::Revise,
        "Only in the section about decisions: add one decision saying the chosen theme is stored \
with the user's other settings in the API. Change nothing else.",
        "design: change one section (edit)",
        &mut turns,
    );
    let edited_design = w.state(&id, "design");
    w.approve(&id, "design");
    w.change(
        &id,
        "design",
        DecisionKind::Redo,
        "Write it again with the same content, adding one decision saying the theme follows the \
operating system until the user chooses one.",
        "design: write again (whole)",
        &mut turns,
    );
    w.approve(&id, "design");
    // The documents written from the design are now outdated. They are kept as they are (no
    // tokens): what is measured here is the change itself, not writing the others again.
    w.keep_outdated(&id);
    // Optimization 5: the first review, a change, and the second review that knows what changed.
    w.review(&id, "review 1", &mut turns);
    w.change(
        &id,
        "tasks",
        DecisionKind::Revise,
        "Add one task to test that the choice is remembered after a reload. Change nothing else.",
        "tasks: change one section (edit)",
        &mut turns,
    );
    w.approve(&id, "tasks");
    w.keep_outdated(&id);
    w.review(&id, "review 2 (knows what changed)", &mut turns);

    eprintln!("\n[C1] design after the edit: {edited_design}");
    let usage = w.specs.view(&id).unwrap().usage;
    eprintln!("[C1] spec total: {usage:?}");
    assert!(turns.len() >= 9, "the script did not get through");
}
