//! The spec use cases working together: the service, the runner, the real chat/execution layer
//! with a scripted runtime (no model is ever called), an in-memory store and project folder.

#![allow(clippy::assert_is_empty)]

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};

use super::cli::OpenSpecCli;
use super::cli_service::SpecCliService;
use super::implementation::SpecImplementation;
use super::openspec::validate::tests_support::{DESIGN, PROPOSAL, SPEC, TASKS};
use super::openspec::OpenSpecAdapter;
use super::ports::{MemoryProjectFiles, MemorySpecStore};
use super::runner::SpecRunner;
use super::service::{NewSpecRequest, NoEvents, SpecEvents, SpecService};
use crate::application::agents::{AgentService, CreateAgentRequest};
use crate::application::chat::{ChatObserver, ChatService};
use crate::application::config::memory::MemoryStore;
use crate::application::config::ConfigRepository;
use crate::application::errors::ErrorCode;
use crate::application::executions::{ExecutionObserver, ExecutionService};
use crate::application::history::ConversationHistory;
use crate::application::personalities::PersonalityService;
use crate::application::projects::fake::FakeInspector;
use crate::application::runtimes::fake::FakeRuntime;
use crate::application::runtimes::{
    Detection, ModelRuntime, RuntimeError, RuntimeEvent, RuntimeOutput, RuntimeRegistry,
    RuntimeRequest,
};
use crate::application::sessions::SessionRegistry;
use crate::application::support::{now_ms, LockExt};
use crate::application::usage::UsageLedger;
use crate::application::workflow::service::{NewWorkflow, WorkflowService};
use crate::application::workflow::test_support::{agent as agent_node, edge, end};
use crate::application::workspace::{WorkspaceInput, WorkspaceService};
use crate::domain::execution::ExecutionEvent;
use crate::domain::runtime::{ModelInfo, RuntimeInfo};
use crate::domain::spec::{
    DecisionKind, FindingStatus, Generation, Publication, QuestionStatus, Review, ReviewRun,
    SpecStatus,
};
use crate::domain::workflow::{EndOutcome, WorkflowMode};

/// A runtime that answers from a script, in order, and keeps the prompts it was sent.
struct Scripted {
    inner: FakeRuntime,
    script: Mutex<VecDeque<Result<String, RuntimeError>>>,
    prompts: Mutex<Vec<String>>,
    edits_allowed: Mutex<Vec<bool>>,
}

impl Scripted {
    fn say(&self, answer: &str) {
        self.script
            .lock_or_recover()
            .push_back(Ok(answer.to_owned()));
    }

    fn fail(&self, error: RuntimeError) {
        self.script.lock_or_recover().push_back(Err(error));
    }

    fn last_prompt(&self) -> String {
        self.prompts
            .lock_or_recover()
            .last()
            .cloned()
            .unwrap_or_default()
    }

    fn runs(&self) -> usize {
        self.prompts.lock_or_recover().len()
    }
}

impl ModelRuntime for Scripted {
    fn info(&self) -> RuntimeInfo {
        self.inner.info()
    }

    fn detect(&self) -> Detection {
        self.inner.detect()
    }

    fn list_models(&self) -> Result<Vec<ModelInfo>, RuntimeError> {
        self.inner.list_models()
    }

    fn execute(
        &self,
        request: &RuntimeRequest,
        progress: &dyn Fn(RuntimeEvent),
    ) -> Result<RuntimeOutput, RuntimeError> {
        progress(RuntimeEvent::Starting);
        self.prompts
            .lock_or_recover()
            .push(request.prompt.combined());
        self.edits_allowed
            .lock_or_recover()
            .push(request.allow_edits);
        let next = self.script.lock_or_recover().pop_front();
        match next {
            Some(Ok(text)) => Ok(RuntimeOutput {
                text,
                metadata: [("durationMs".to_owned(), "5".to_owned())].into(),
                usage: None,
                quota: None,
            }),
            Some(Err(error)) => Err(error),
            None => Err(RuntimeError::Unavailable("nothing scripted".to_owned())),
        }
    }
}

struct Quiet;

impl ExecutionObserver for Quiet {
    fn on_event(&self, _event: &ExecutionEvent) {}
}

impl ChatObserver for Quiet {
    fn on_message(&self, _message: &crate::domain::conversation::Message) {}
}

#[derive(Default)]
struct RecordedEvents(Mutex<Vec<(String, u64)>>);

impl SpecEvents for RecordedEvents {
    fn changed(&self, spec_id: &str, version: u64) {
        self.0.lock_or_recover().push((spec_id.to_owned(), version));
    }
}

struct World {
    specs: Arc<SpecService>,
    runner: SpecRunner,
    store: Arc<MemorySpecStore>,
    files: Arc<MemoryProjectFiles>,
    runtime: Arc<Scripted>,
    events: Arc<RecordedEvents>,
    workspaces: Arc<WorkspaceService>,
    agents: Arc<AgentService>,
    workflows: Arc<WorkflowService>,
    work: SpecImplementation,
    ledger: Arc<UsageLedger>,
    budget: Arc<Mutex<Option<u64>>>,
    workspace_id: String,
    project_id: String,
    agent_id: String,
    other_agent_id: String,
}

#[allow(clippy::too_many_lines)] // the whole wiring, once
fn world() -> World {
    let config = Arc::new(ConfigRepository::load(Box::<MemoryStore>::default()));
    let personalities = Arc::new(PersonalityService::new(config.clone()));
    let runtime = Arc::new(Scripted {
        inner: FakeRuntime::new("scripted", Ok("unused")),
        script: Mutex::new(VecDeque::new()),
        prompts: Mutex::new(Vec::new()),
        edits_allowed: Mutex::new(Vec::new()),
    });
    let registry = Arc::new(RuntimeRegistry::new(vec![runtime.clone()]));
    let agents = Arc::new(AgentService::new(
        config.clone(),
        personalities.clone(),
        registry.clone(),
    ));
    let create_agent = |name: &str| {
        agents
            .create(CreateAgentRequest {
                permission_profile_id: Some("developer".to_owned()),
                name: name.to_owned(),
                personality_id: "architect".to_owned(),
                runtime_id: "scripted".to_owned(),
                model_id: "m1".to_owned(),
                instructions: String::new(),
                worktree_isolation: Some(false),
                result_contract: None,
            })
            .unwrap()
            .id
    };
    let agent_id = create_agent("Writer");
    let other_agent_id = create_agent("Reviewer");
    let workspaces = Arc::new(WorkspaceService::new(
        config.clone(),
        agents.clone(),
        Arc::new(FakeInspector::with(&[("/atlas", &[]), ("/other", &[])])),
    ));
    let workspace = workspaces
        .create(&WorkspaceInput {
            name: "Atlas".to_owned(),
            project_path: "/atlas".to_owned(),
            description: None,
        })
        .unwrap();
    workspaces.add_agent(&workspace.id, &agent_id).unwrap();
    workspaces
        .add_agent(&workspace.id, &other_agent_id)
        .unwrap();
    let project_id = workspaces.get(&workspace.id).unwrap().projects[0]
        .id
        .clone();
    let workflows = Arc::new(WorkflowService::new(
        config.clone(),
        agents.clone(),
        workspaces.clone(),
    ));
    let executions = Arc::new(
        ExecutionService::new(
            agents.clone(),
            personalities,
            registry,
            workspaces.clone(),
            Arc::new(crate::application::security::AuditLog::default()),
        )
        .with_policies(Arc::new(
            crate::application::security::SecurityService::new(config.clone()),
        )),
    );
    let ledger = Arc::new(UsageLedger::new(config.clone()));
    let chat = ChatService::with_history(
        agents.clone(),
        executions,
        workspaces.clone(),
        ledger.clone(),
        Arc::new(ConversationHistory::new(config)),
    );
    let store = Arc::new(MemorySpecStore::default());
    let files = Arc::new(MemoryProjectFiles::default());
    let events = Arc::new(RecordedEvents::default());
    let budget = Arc::new(Mutex::new(None));
    let specs = Arc::new(
        SpecService::new(
            store.clone(),
            files.clone(),
            Arc::new(OpenSpecAdapter),
            workspaces.clone(),
            agents.clone(),
            events.clone(),
        )
        .with_usage(ledger.clone())
        .with_budget({
            let budget = budget.clone();
            Arc::new(move || *budget.lock_or_recover())
        }),
    );
    let runner = SpecRunner::new(specs.clone(), chat, Arc::new(SessionRegistry::default()));
    let work = SpecImplementation::new(specs.clone(), workflows.clone());
    World {
        workflows,
        work,
        ledger,
        budget,
        specs,
        runner,
        store,
        files,
        runtime,
        events,
        workspaces,
        agents,
        workspace_id: workspace.id,
        project_id,
        agent_id,
        other_agent_id,
    }
}

fn doc(path: &str, content: &str) -> String {
    format!("<<<ATLAS-DOCUMENT path=\"{path}\">>>\n{content}\n<<<END-ATLAS-DOCUMENT>>>\n")
}

fn new_request(w: &World) -> NewSpecRequest {
    NewSpecRequest {
        workspace_id: w.workspace_id.clone(),
        project_id: None,
        title: "Dark mode".to_owned(),
        request: "Add a dark mode to the settings screen".to_owned(),
        context: "Web app only".to_owned(),
        author_agent_id: w.agent_id.clone(),
        reviewer_agent_id: Some(w.other_agent_id.clone()),
    }
}

fn answer_for(artifact: &str) -> String {
    match artifact {
        "proposal" => doc("proposal.md", PROPOSAL),
        "specs" => doc("specs/dark-mode/spec.md", SPEC),
        "design" => doc("design.md", DESIGN),
        _ => doc("tasks.md", TASKS),
    }
}

/// Starts the artifact's turn and waits for it: what a command does on a blocking thread.
fn generate(w: &World, id: &str, artifact: &str) {
    w.runner
        .prepare_generation(id, artifact)
        .unwrap()
        .run(&Quiet);
}

fn approve(w: &World, id: &str, artifact: &str) {
    let revision = w
        .specs
        .get(id)
        .unwrap()
        .artifact(artifact)
        .unwrap()
        .latest_number()
        .unwrap();
    w.specs
        .decide(id, artifact, revision, DecisionKind::Approve, None)
        .unwrap();
}

/// Writes and approves the four documents.
fn all_documents(w: &World, id: &str) {
    for artifact in ["proposal", "specs", "design", "tasks"] {
        w.runtime.say(&answer_for(artifact));
        generate(w, id, artifact);
        approve(w, id, artifact);
    }
}

fn started(w: &World) -> String {
    let view = w.specs.create(new_request(w)).unwrap();
    w.specs.start(&view.spec.id).unwrap();
    view.spec.id
}

fn review_answer(findings: &str) -> String {
    format!("Reviewed.\n```atlas-review\n{{\"summary\":\"ok\",\"findings\":{findings}}}\n```\n")
}

fn ready(w: &World) -> String {
    let id = started(w);
    all_documents(w, &id);
    w.runtime.say(&review_answer("[]"));
    w.runner.prepare_review(&id).unwrap().run(&Quiet);
    w.specs.approve_final(&id).unwrap();
    id
}

// ---- creating -----------------------------------------------------------------------------------

#[test]
fn creating_a_spec_records_the_request_and_runs_nothing() {
    let w = world();

    let view = w.specs.create(new_request(&w)).unwrap();

    assert_eq!(view.status, SpecStatus::Draft);
    assert_eq!(view.spec.change_id, "dark-mode");
    assert_eq!(view.spec.request, "Add a dark mode to the settings screen");
    assert!(!view.spec.started);
    assert!(
        view.ready_to_generate.is_empty(),
        "nothing can run until it is started"
    );
    assert_eq!(w.runtime.runs(), 0);
    assert!(w.store.stored(&view.spec.id).is_some());
}

#[test]
fn a_request_with_nothing_in_it_or_an_agent_from_elsewhere_is_refused() {
    let w = world();
    let refused = |request: NewSpecRequest| w.specs.create(request).unwrap_err();

    let mut no_title = new_request(&w);
    no_title.title = "  ".to_owned();
    assert_eq!(refused(no_title).params["reason"], "title_required");

    let mut no_request = new_request(&w);
    no_request.request = String::new();
    assert_eq!(refused(no_request).params["reason"], "request_required");

    let mut long = new_request(&w);
    long.request = "x".repeat(8_001);
    assert_eq!(refused(long).params["reason"], "request_too_long");

    let mut stranger = new_request(&w);
    stranger.author_agent_id = "ghost".to_owned();
    assert_eq!(refused(stranger).params["reason"], "agent_invalid");

    let mut project = new_request(&w);
    project.project_id = Some("nope".to_owned());
    assert_eq!(refused(project).params["reason"], "project_invalid");

    let mut workspace = new_request(&w);
    workspace.workspace_id = "ghost".to_owned();
    assert!(refused(workspace).is(ErrorCode::WorkspaceNotFound));
}

#[test]
fn two_specs_with_the_same_title_get_different_change_ids() {
    let w = world();

    let first = w.specs.create(new_request(&w)).unwrap();
    let second = w.specs.create(new_request(&w)).unwrap();

    assert_eq!(first.spec.change_id, "dark-mode");
    assert_eq!(second.spec.change_id, "dark-mode-2");
}

#[test]
fn a_change_folder_already_in_the_project_is_not_reused_as_an_id() {
    let w = world();
    w.files.put("openspec/changes/dark-mode/proposal.md", "# P");

    let view = w.specs.create(new_request(&w)).unwrap();

    assert_eq!(view.spec.change_id, "dark-mode-2");
}

// ---- writing, reviewing, approving ---------------------------------------------------------------

#[test]
fn the_first_document_is_written_read_only_validated_and_waits_for_the_person() {
    let w = world();
    let id = started(&w);
    w.runtime
        .say(&format!("A note for you.\n{}", answer_for("proposal")));

    generate(&w, &id, "proposal");

    let view = w.specs.view(&id).unwrap();
    let proposal = view.spec.artifact("proposal").unwrap();
    assert_eq!(proposal.revisions.len(), 1);
    assert_eq!(proposal.revisions[0].files[0].path, "proposal.md");
    assert_eq!(proposal.review, Review::AwaitingReview);
    assert_eq!(proposal.generation, Generation::Idle);
    assert!(
        proposal
            .validation
            .as_ref()
            .is_some_and(|v| v.revision == 1 && v.errors() == 0),
        "the structure is checked as soon as the draft exists"
    );
    assert_eq!(view.status, SpecStatus::AwaitingReview);
    assert!(!proposal.is_approved(), "finishing a run approves nothing");
    // The agent could not have edited anything, whatever its profile says.
    assert_eq!(*w.runtime.edits_allowed.lock_or_recover(), vec![false]);
    let prompt = w.runtime.last_prompt();
    assert!(prompt.contains("Add a dark mode to the settings screen"));
    assert!(prompt.contains("Web app only"));
    assert!(prompt.contains("<<<ATLAS-DOCUMENT path=\"proposal.md\">>>"));
}

#[test]
fn the_turn_is_an_execution_like_any_other_and_shows_in_the_agents_history() {
    let w = world();
    let id = started(&w);
    w.runtime.say(&answer_for("proposal"));

    generate(&w, &id, "proposal");

    let spec = w.specs.get(&id).unwrap();
    assert_eq!(spec.runs.len(), 1);
    assert!(spec.runs[0].id.starts_with("exec-"), "{}", spec.runs[0].id);
    assert_eq!(
        spec.runs[0].outcome,
        Some(crate::domain::spec::RunOutcome::Completed)
    );
}

#[test]
fn approving_lets_the_next_document_be_written_from_the_approved_one() {
    let w = world();
    let id = started(&w);
    w.runtime.say(&answer_for("proposal"));
    generate(&w, &id, "proposal");
    // The specs cannot be written before the proposal is approved.
    let early = w.runner.prepare_generation(&id, "specs");
    assert!(early.is_err());

    approve(&w, &id, "proposal");
    w.runtime.say(&answer_for("specs"));
    generate(&w, &id, "specs");

    let prompt = w.runtime.last_prompt();
    assert!(prompt.contains("APPROVED DOCUMENTS"));
    assert!(
        prompt.contains("Users working at night"),
        "the approved proposal is in the prompt"
    );
    assert_eq!(
        w.specs.view(&id).unwrap().status,
        SpecStatus::AwaitingReview
    );
}

#[test]
fn asking_for_changes_sends_the_current_document_and_the_feedback_and_keeps_the_history() {
    let w = world();
    let id = started(&w);
    w.runtime.say(&answer_for("proposal"));
    generate(&w, &id, "proposal");
    let blank = w
        .specs
        .decide(&id, "proposal", 1, DecisionKind::Revise, Some(" "));
    assert!(blank.unwrap_err().is(ErrorCode::SpecNoteRequired));
    w.specs
        .decide(
            &id,
            "proposal",
            1,
            DecisionKind::Revise,
            Some("Mention night shift workers"),
        )
        .unwrap();
    assert_eq!(
        w.specs.view(&id).unwrap().status,
        SpecStatus::ChangesRequested
    );

    w.runtime.say(&doc(
        "proposal.md",
        &format!("{PROPOSAL}\nNight shift workers too.\n"),
    ));
    generate(&w, &id, "proposal");

    let prompt = w.runtime.last_prompt();
    assert!(prompt.contains("REVISE the existing proposal"));
    assert!(prompt.contains("Mention night shift workers"));
    assert!(
        prompt.contains("Users working at night"),
        "the current document, not a blank page"
    );
    let spec = w.specs.get(&id).unwrap();
    let proposal = spec.artifact("proposal").unwrap();
    assert_eq!(proposal.revisions.len(), 2);
    assert!(proposal.revisions[1].files[0]
        .content
        .contains("Night shift"));
    assert!(proposal.revisions[0].files[0]
        .content
        .contains("Users working at night"));
}

#[test]
fn an_approval_for_a_revision_the_person_is_no_longer_looking_at_is_refused() {
    let w = world();
    let id = started(&w);
    w.runtime.say(&answer_for("proposal"));
    generate(&w, &id, "proposal");
    w.specs
        .decide(&id, "proposal", 1, DecisionKind::Redo, None)
        .unwrap();
    w.runtime.say(&answer_for("proposal"));
    generate(&w, &id, "proposal");

    let stale = w
        .specs
        .decide(&id, "proposal", 1, DecisionKind::Approve, None);

    let error = stale.unwrap_err();
    assert!(error.is(ErrorCode::SpecRevisionOutdated));
    assert_eq!(error.params["current"], "2");
    assert!(!w
        .specs
        .get(&id)
        .unwrap()
        .artifact("proposal")
        .unwrap()
        .is_approved());
}

#[test]
fn a_document_with_errors_cannot_be_approved_until_it_is_fixed() {
    let w = world();
    let id = started(&w);
    w.runtime
        .say(&doc("proposal.md", "# Proposal\n\n## Why\n\nShort.\n"));
    generate(&w, &id, "proposal");

    let refused = w
        .specs
        .decide(&id, "proposal", 1, DecisionKind::Approve, None);

    assert!(refused.unwrap_err().is(ErrorCode::SpecValidationBlocking));
    let spec = w.specs.get(&id).unwrap();
    let codes: Vec<&str> = spec
        .artifact("proposal")
        .unwrap()
        .validation
        .as_ref()
        .unwrap()
        .issues
        .iter()
        .map(|i| i.code.as_str())
        .collect();
    assert!(codes.contains(&"why_too_short"));
    assert!(codes.contains(&"what_changes_missing"));
}

// ---- when the agent does not deliver ----------------------------------------------------------------

#[test]
fn an_answer_that_is_not_a_document_fails_the_run_and_keeps_the_earlier_revision() {
    let w = world();
    let id = started(&w);
    w.runtime.say(&answer_for("proposal"));
    generate(&w, &id, "proposal");
    w.specs
        .decide(&id, "proposal", 1, DecisionKind::Redo, None)
        .unwrap();
    w.runtime
        .say("Sure! Here is a great proposal for you. Let me know if you want more.");

    generate(&w, &id, "proposal");

    let view = w.specs.view(&id).unwrap();
    let proposal = view.spec.artifact("proposal").unwrap();
    assert!(
        matches!(&proposal.generation, Generation::Failed { message, .. } if message == "output_no_document")
    );
    assert_eq!(
        proposal.revisions.len(),
        1,
        "what was written before is untouched"
    );
    assert_eq!(view.status, SpecStatus::Failed);
    // It can be tried again.
    w.runtime.say(&answer_for("proposal"));
    generate(&w, &id, "proposal");
    assert_eq!(
        w.specs
            .get(&id)
            .unwrap()
            .artifact("proposal")
            .unwrap()
            .revisions
            .len(),
        2
    );
}

#[test]
fn a_runtime_failure_is_recorded_as_the_runs_failure() {
    let w = world();
    let id = started(&w);
    w.runtime
        .fail(RuntimeError::Unavailable("not signed in".to_owned()));

    generate(&w, &id, "proposal");

    let spec = w.specs.get(&id).unwrap();
    let artifact = spec.artifact("proposal").unwrap();
    match &artifact.generation {
        Generation::Failed {
            message,
            interrupted,
            ..
        } => {
            assert!(message.starts_with("run_failed|"), "{message}");
            assert!(!interrupted);
        }
        other => panic!("{other:?}"),
    }
    assert!(artifact.revisions.is_empty());
}

#[test]
fn a_document_cut_off_in_the_middle_is_not_used() {
    let w = world();
    let id = started(&w);
    w.runtime
        .say("<<<ATLAS-DOCUMENT path=\"proposal.md\">>>\n# Proposal\n\n## Why\nhalf a sent");

    generate(&w, &id, "proposal");

    let spec = w.specs.get(&id).unwrap();
    assert!(matches!(
        &spec.artifact("proposal").unwrap().generation,
        Generation::Failed { message, .. } if message == "output_cut_off|proposal.md"
    ));
    assert!(spec.artifact("proposal").unwrap().revisions.is_empty());
}

// ---- asking the person -------------------------------------------------------------------------------

const QUESTION: &str = "I cannot tell who the dark mode is for.\n\n```atlas-interaction\n{\"type\":\"clarification\",\"question\":\"Which users get it?\",\"context\":\"The request does not say\",\"options\":[\"Everyone\",\"Admins only\"]}\n```\n";

#[test]
fn an_agent_that_needs_something_asks_and_the_question_survives_a_restart() {
    let w = world();
    let id = started(&w);
    w.runtime.say(QUESTION);

    generate(&w, &id, "proposal");

    let view = w.specs.view(&id).unwrap();
    assert_eq!(view.status, SpecStatus::WaitingForInput);
    assert_eq!(view.pending_questions, 1);
    let question = &view.spec.questions[0];
    assert_eq!(question.question, "Which users get it?");
    assert_eq!(question.options.len(), 2);
    assert_eq!(question.status, QuestionStatus::Pending);
    assert!(
        view.spec.artifact("proposal").unwrap().revisions.is_empty(),
        "no document was invented"
    );

    // The app is closed and opened again.
    let reopened = SpecService::new(
        w.store.clone(),
        w.files.clone(),
        Arc::new(OpenSpecAdapter),
        w.workspaces.clone(),
        w.agents.clone(),
        Arc::new(NoEvents),
    );
    let after = reopened.view(&id).unwrap();
    assert_eq!(after.pending_questions, 1);
    assert_eq!(after.status, SpecStatus::WaitingForInput);
}

#[test]
fn the_answer_goes_back_to_the_agent_and_the_run_goes_on_from_it() {
    let w = world();
    let id = started(&w);
    w.runtime.say(QUESTION);
    generate(&w, &id, "proposal");
    let question_id = w.specs.get(&id).unwrap().questions[0].id.clone();
    // It cannot be written again while the question is open.
    assert!(w.runner.prepare_generation(&id, "proposal").is_err());

    let (source, artifact) = w
        .specs
        .answer_question(
            &id,
            &question_id,
            Some("Admins only"),
            Some("Only admins, for now"),
        )
        .unwrap();
    assert_eq!(artifact.as_deref(), Some("proposal"));
    assert_eq!(source, crate::domain::spec::QuestionSource::Artifact);
    w.runtime.say(&answer_for("proposal"));
    generate(&w, &id, "proposal");

    let prompt = w.runtime.last_prompt();
    assert!(prompt.contains("HUMAN INPUT"));
    assert!(prompt.contains("Which users get it?"));
    assert!(prompt.contains("Only admins, for now"));
    assert_eq!(
        w.specs.view(&id).unwrap().status,
        SpecStatus::AwaitingReview
    );
}

#[test]
fn an_answer_that_is_empty_or_not_an_option_is_refused_and_a_question_is_answered_once() {
    let w = world();
    let id = started(&w);
    w.runtime.say(QUESTION);
    generate(&w, &id, "proposal");
    let question_id = w.specs.get(&id).unwrap().questions[0].id.clone();

    let empty = w.specs.answer_question(&id, &question_id, None, Some("  "));
    assert_eq!(empty.unwrap_err().params["reason"], "answer_required");
    let outside = w
        .specs
        .answer_question(&id, &question_id, Some("Martians"), None);
    assert_eq!(outside.unwrap_err().params["reason"], "answer_invalid");
    let long = w
        .specs
        .answer_question(&id, &question_id, None, Some(&"x".repeat(2_001)));
    assert_eq!(long.unwrap_err().params["reason"], "answer_too_long");

    w.specs
        .answer_question(&id, &question_id, Some("Everyone"), None)
        .unwrap();
    let twice = w
        .specs
        .answer_question(&id, &question_id, Some("Everyone"), None);
    assert!(twice.unwrap_err().is(ErrorCode::SpecStateInvalid));
}

// ---- runs that arrive late or twice ---------------------------------------------------------------------

#[test]
fn the_result_of_a_run_that_is_no_longer_the_current_one_changes_nothing() {
    let w = world();
    let id = started(&w);
    w.runtime.say(&answer_for("proposal"));
    generate(&w, &id, "proposal");
    let before = w.specs.get(&id).unwrap();

    let late = w.specs.finish_generation(
        &id,
        "proposal",
        "exec-old",
        super::service::GenerationEnd::Document(vec![crate::domain::spec::SpecFile {
            path: "proposal.md".to_owned(),
            content: "# stray".to_owned(),
        }]),
    );

    assert!(late.is_err());
    let after = w.specs.get(&id).unwrap();
    assert_eq!(
        after.artifact("proposal").unwrap().revisions,
        before.artifact("proposal").unwrap().revisions
    );
}

#[test]
fn only_one_run_is_started_for_a_spec_at_a_time() {
    let w = world();
    let id = started(&w);
    let guard = w.specs.launch_guard(&id).unwrap();

    let second = w.runner.prepare_generation(&id, "proposal");

    assert!(second.err().unwrap().is(ErrorCode::SpecRunInProgress));
    drop(guard);
    assert!(w.runner.prepare_generation(&id, "proposal").is_ok());
}

#[test]
fn an_agent_that_is_busy_does_not_start_a_second_run_and_leaves_the_spec_alone() {
    let w = world();
    let id = started(&w);
    let first = w.runner.prepare_generation(&id, "proposal").unwrap();

    // The same agent in the same workspace is busy until the first run is done.
    let second = w.runner.prepare_generation(&id, "proposal");

    assert!(second.is_err());
    drop(first);
}

#[test]
fn after_a_restart_a_run_nobody_saw_end_is_reported_interrupted_and_its_answer_is_dropped() {
    let w = world();
    let id = started(&w);
    let prepared = w.runner.prepare_generation(&id, "proposal").unwrap();
    let run_id = prepared.execution_id().to_owned();
    assert_eq!(w.specs.view(&id).unwrap().status, SpecStatus::Generating);

    // The app ends here; the next session starts from the store.
    let next = Arc::new(SpecService::new(
        w.store.clone(),
        w.files.clone(),
        Arc::new(OpenSpecAdapter),
        w.workspaces.clone(),
        w.agents.clone(),
        Arc::new(NoEvents),
    ));
    next.recover();

    let view = next.view(&id).unwrap();
    assert!(matches!(
        &view.spec.artifact("proposal").unwrap().generation,
        Generation::Failed {
            interrupted: true,
            ..
        }
    ));
    assert_eq!(view.status, SpecStatus::Failed);
    // The old run's answer, if it ever came, would be refused.
    let late = next.finish_generation(
        &id,
        "proposal",
        &run_id,
        super::service::GenerationEnd::Document(vec![crate::domain::spec::SpecFile {
            path: "proposal.md".to_owned(),
            content: PROPOSAL.to_owned(),
        }]),
    );
    assert!(late.is_err());
    assert!(next
        .view(&id)
        .unwrap()
        .spec
        .artifact("proposal")
        .unwrap()
        .revisions
        .is_empty());
}

#[test]
fn when_the_disk_fails_the_spec_is_as_it_was() {
    let w = world();
    let id = started(&w);
    let before = w.specs.get(&id).unwrap();
    w.store.fail_saves(true);

    let result = w.specs.decide(&id, "proposal", 1, DecisionKind::Redo, None);
    let create = w.specs.create(new_request(&w));

    assert!(result.is_err());
    assert!(create.unwrap_err().is(ErrorCode::StorageFailed));
    assert_eq!(w.specs.get(&id).unwrap(), before);
    w.store.fail_saves(false);
    assert!(w.specs.get(&id).is_some());
}

#[test]
fn every_change_tells_the_screens() {
    let w = world();
    let view = w.specs.create(new_request(&w)).unwrap();
    let before = w.events.0.lock_or_recover().len();

    w.specs.start(&view.spec.id).unwrap();

    let events = w.events.0.lock_or_recover();
    assert!(events.len() > before);
    assert_eq!(events.last().unwrap().0, view.spec.id);
    assert!(events.last().unwrap().1 > view.spec.version);
}

// ---- cancelling ------------------------------------------------------------------------------------------

#[test]
fn a_cancelled_run_leaves_the_document_to_be_asked_for_again() {
    let w = world();
    let id = started(&w);
    let run = w.runner.prepare_generation(&id, "proposal").unwrap();
    let execution_id = run.execution_id().to_owned();

    w.specs
        .finish_generation(
            &id,
            "proposal",
            &execution_id,
            super::service::GenerationEnd::Failed {
                message: "cancelled".to_owned(),
                outcome: crate::domain::spec::RunOutcome::Cancelled,
            },
        )
        .unwrap();

    assert_eq!(
        w.specs.view(&id).unwrap().status,
        SpecStatus::AwaitingGeneration
    );
    drop(run);
}

#[test]
fn cancelling_when_nothing_runs_is_said() {
    let w = world();
    let id = started(&w);

    let error = w.runner.cancel(&id).unwrap_err();

    assert!(error.is(ErrorCode::SpecStateInvalid));
}

// ---- the final review ----------------------------------------------------------------------------------------

#[test]
fn the_review_needs_every_document_approved_first() {
    let w = world();
    let id = started(&w);
    w.runtime.say(&answer_for("proposal"));
    generate(&w, &id, "proposal");
    approve(&w, &id, "proposal");

    let early = w.runner.prepare_review(&id);

    assert!(early.is_err());
}

#[test]
fn a_spec_with_everything_approved_is_still_not_ready_until_it_is_reviewed_and_approved() {
    let w = world();
    let id = started(&w);

    all_documents(&w, &id);

    let view = w.specs.view(&id).unwrap();
    assert_eq!(view.status, SpecStatus::AwaitingValidation);
    assert!(!view.readiness.ready);
    let codes: Vec<&str> = view
        .readiness
        .blockers
        .iter()
        .map(|b| b.code.as_str())
        .collect();
    assert!(codes.contains(&"review_not_run"));
    assert!(codes.contains(&"final_approval_missing"));
    assert!(
        view.consistency_current,
        "the checks across the documents ran by themselves"
    );
    assert!(w
        .specs
        .approve_final(&id)
        .unwrap_err()
        .is(ErrorCode::SpecNotReady));
}

#[test]
fn a_review_that_finds_something_serious_blocks_until_the_person_decides() {
    let w = world();
    let id = started(&w);
    all_documents(&w, &id);
    w.runtime.say(&review_answer(
        r#"[{"category":"missing_scenario","severity":"high","artifact":"specs","reference":"Requirement: User can switch the theme","title":"No scenario for a failed save","description":"The choice is never saved.","evidence":"The system SHALL let the user pick light or dark.","question":"Should the choice persist?"},
            {"category":"risk","severity":"low","artifact":"design","title":"Contrast","description":"Low contrast."}]"#,
    ));

    w.runner.prepare_review(&id).unwrap().run(&Quiet);

    let view = w.specs.view(&id).unwrap();
    assert_eq!(view.spec.findings.len(), 2);
    assert_eq!(view.open_findings, 2);
    assert_eq!(view.status, SpecStatus::Blocked);
    assert!(view.review_current);
    assert!(w.specs.approve_final(&id).is_err());
    let finding = view
        .spec
        .findings
        .iter()
        .find(|f| f.severity == crate::domain::spec::FindingSeverity::High)
        .unwrap()
        .id
        .clone();

    // Accepting a risk needs words.
    let silent = w
        .specs
        .resolve_finding(&id, &finding, FindingStatus::AcceptedRisk, None);
    assert!(silent.unwrap_err().is(ErrorCode::SpecNoteRequired));
    w.specs
        .resolve_finding(
            &id,
            &finding,
            FindingStatus::AcceptedRisk,
            Some("Ops handles it"),
        )
        .unwrap();

    let view = w.specs.approve_final(&id).unwrap();
    assert_eq!(view.status, SpecStatus::ReadyForImplementation);
    assert!(view.readiness.ready);
}

// ---- what a spec costs ----------------------------------------------------------------------------

fn record_usage(w: &World, execution_id: &str, input: u64, output: u64) {
    use crate::domain::usage::{UsageMetrics, UsageRecord, UsageSource};
    w.ledger
        .record(
            UsageRecord {
                execution_id: execution_id.to_owned(),
                workspace_id: w.workspace_id.clone(),
                agent_id: w.agent_id.clone(),
                runtime_id: "scripted".to_owned(),
                model_id: "m1".to_owned(),
                started_at: now_ms(),
                completed_at: now_ms(),
                succeeded: true,
                metrics: Some(UsageMetrics {
                    input_tokens: Some(input),
                    output_tokens: Some(output),
                    total_tokens: Some(input + output),
                    cached_input_tokens: Some(input / 2),
                    cost: Some(0.5),
                    currency: Some("USD".to_owned()),
                    source: UsageSource::RuntimeReported,
                }),
            },
            None,
        )
        .unwrap();
}

#[test]
fn the_view_says_what_the_runs_of_the_spec_used_and_how_many_of_them_were_measured() {
    let w = world();
    let id = started(&w);
    w.runtime.say(&answer_for("proposal"));
    generate(&w, &id, "proposal");
    w.runtime.say(&answer_for("specs"));
    // The first turn reported its use; the second has nothing in the ledger.
    let spec = w.specs.get(&id).unwrap();
    record_usage(&w, &spec.runs[0].id, 10_000, 2_000);
    approve(&w, &id, "proposal");
    generate(&w, &id, "specs");

    let usage = w.specs.view(&id).unwrap().usage;

    assert_eq!(usage.runs_total, 2);
    assert!(usage.runs_measured <= 1);
    assert_eq!(usage.input_tokens, Some(10_000));
    assert_eq!(usage.output_tokens, Some(2_000));
    assert_eq!(usage.cached_input_tokens, Some(5_000));
    assert_eq!(usage.cost, Some(0.5));
    assert_eq!(usage.currency.as_deref(), Some("USD"));
}

#[test]
fn a_spec_that_has_used_its_budget_starts_no_new_run_until_the_person_says_to_go_on() {
    let w = world();
    let id = started(&w);
    w.runtime.say(&answer_for("proposal"));
    generate(&w, &id, "proposal");
    let first = w.specs.get(&id).unwrap().runs[0].id.clone();
    record_usage(&w, &first, 9_000, 1_000);
    *w.budget.lock_or_recover() = Some(10_000);

    let view = w.specs.view(&id).unwrap();
    assert!(view.budget.reached);
    assert_eq!(view.budget.spent, Some(10_000));
    let refused = w.runner.prepare_generation(&id, "proposal").err().unwrap();
    assert!(refused.is(ErrorCode::SpecBudgetReached));
    assert!(w.runner.prepare_review(&id).err().is_some());
    // The document and what the person decided are as they were.
    assert_eq!(
        w.specs
            .get(&id)
            .unwrap()
            .artifact("proposal")
            .unwrap()
            .revisions
            .len(),
        1
    );

    let raised = w.specs.raise_budget(&id).unwrap();
    assert!(!raised.budget.reached);
    assert_eq!(raised.budget.limit, Some(20_000));
    w.specs
        .decide(&id, "proposal", 1, DecisionKind::Redo, None)
        .unwrap();
    w.runtime.say(&answer_for("proposal"));
    assert!(w.runner.prepare_generation(&id, "proposal").is_ok());
}

#[test]
fn without_a_budget_or_without_reported_usage_nothing_is_ever_refused() {
    let w = world();
    let id = started(&w);
    *w.budget.lock_or_recover() = Some(1);
    w.runtime.say(&answer_for("proposal"));

    // Nothing was reported, so nothing is known to be spent: the limit is not guessed at.
    assert!(w.runner.prepare_generation(&id, "proposal").is_ok());
    assert!(!w.specs.view(&id).unwrap().budget.reached);
}

// ---- changing a document by sections ------------------------------------------------------------

/// A design long enough that a request for changes is answered by sections.
fn long_design() -> String {
    let mut text = String::from(DESIGN.trim_end());
    for n in 1..=30 {
        let _ = write!(
            text,
            "\n\n### Extra decision {n}\nWe keep this part of the approach as it is, for reasons that are written down here at some length."
        );
    }
    text
}

fn with_long_design(w: &World) -> String {
    let id = started(w);
    for artifact in ["proposal", "specs"] {
        w.runtime.say(&answer_for(artifact));
        generate(w, &id, artifact);
        approve(w, &id, artifact);
    }
    w.runtime.say(&doc("design.md", &long_design()));
    generate(w, &id, "design");
    id
}

fn ask_for_changes(w: &World, id: &str, note: &str) {
    let revision = w
        .specs
        .get(id)
        .unwrap()
        .artifact("design")
        .unwrap()
        .latest_number()
        .unwrap();
    w.specs
        .decide(id, "design", revision, DecisionKind::Revise, Some(note))
        .unwrap();
}

fn section_edit(heading: &str, body: &str) -> String {
    format!(
        "<<<ATLAS-EDIT path=\"design.md\" op=\"replace\" heading=\"{heading}\">>>\n{body}\n<<<END-ATLAS-EDIT>>>\n"
    )
}

#[test]
fn a_request_for_changes_to_a_long_document_asks_for_sections_and_splices_them_in() {
    let w = world();
    let id = with_long_design(&w);
    ask_for_changes(&w, &id, "Decision 5 should be about the webhook");
    let before = w.specs.get(&id).unwrap();
    let old = before.artifact("design").unwrap().latest().unwrap().files[0]
        .content
        .clone();
    w.runtime.say(&section_edit(
        "### Extra decision 7",
        "### Extra decision 7\nChanged on request.",
    ));

    generate(&w, &id, "design");

    let prompt = w.runtime.last_prompt();
    assert!(prompt.contains("Answer with only the sections that change"));
    assert!(prompt.contains("<<<ATLAS-EDIT path=\"design.md\" op=\"replace\""));
    let spec = w.specs.get(&id).unwrap();
    let design = spec.artifact("design").unwrap();
    assert_eq!(design.revisions.len(), 2);
    let new = &design.latest().unwrap().files[0].content;
    assert!(new.contains("### Extra decision 7\nChanged on request."));
    // Everything outside the section is what it was.
    let expected = old.replace(
        "### Extra decision 7\nWe keep this part of the approach as it is, for reasons that are written down here at some length.",
        "### Extra decision 7\nChanged on request.",
    );
    assert_eq!(new.trim(), expected.trim());
}

#[test]
fn an_edit_that_cannot_be_applied_fails_the_run_and_the_next_try_asks_for_the_whole_document() {
    let w = world();
    let id = with_long_design(&w);
    ask_for_changes(&w, &id, "Rewrite decision 3");
    w.runtime.say(&section_edit(
        "### Not a heading of this document",
        "### Not a heading of this document\nx",
    ));

    generate(&w, &id, "design");

    let spec = w.specs.get(&id).unwrap();
    let design = spec.artifact("design").unwrap();
    assert_eq!(design.revisions.len(), 1, "the document is untouched");
    assert!(matches!(
        &design.generation,
        Generation::Failed { message, .. } if message.starts_with("output_edit_heading_missing")
    ));

    w.runtime.say(&doc("design.md", &long_design()));
    generate(&w, &id, "design");

    let prompt = w.runtime.last_prompt();
    assert!(prompt.contains("Return the COMPLETE document again"));
    assert!(!prompt.contains("Answer with only the sections that change"));
}

#[test]
fn a_short_document_is_revised_whole() {
    let w = world();
    let id = started(&w);
    w.runtime.say(&answer_for("proposal"));
    generate(&w, &id, "proposal");
    let revision = w
        .specs
        .get(&id)
        .unwrap()
        .artifact("proposal")
        .unwrap()
        .latest_number()
        .unwrap();
    w.specs
        .decide(
            &id,
            "proposal",
            revision,
            DecisionKind::Revise,
            Some("Say more about night owls"),
        )
        .unwrap();
    w.runtime.say(&answer_for("proposal"));

    generate(&w, &id, "proposal");

    let prompt = w.runtime.last_prompt();
    assert!(prompt.contains("Return the COMPLETE document again"));
    assert!(!prompt.contains("ATLAS-EDIT"));
}

#[test]
fn a_second_review_is_told_what_changed_since_the_first_and_still_gets_every_document() {
    let w = world();
    let id = started(&w);
    all_documents(&w, &id);
    w.runtime.say(&review_answer("[]"));
    w.runner.prepare_review(&id).unwrap().run(&Quiet);
    // The design is written again with something added, and approved; the tasks follow it.
    let revision = w
        .specs
        .get(&id)
        .unwrap()
        .artifact("design")
        .unwrap()
        .latest_number()
        .unwrap();
    w.specs
        .decide(
            &id,
            "design",
            revision,
            DecisionKind::Revise,
            Some("Add a note"),
        )
        .unwrap();
    w.runtime
        .say(&doc("design.md", &format!("{DESIGN}\n\n## Notes\nAdded.")));
    generate(&w, &id, "design");
    approve(&w, &id, "design");
    w.specs.keep_outdated(&id, "tasks", 1).unwrap();
    approve(&w, &id, "tasks");
    w.runtime.say(&review_answer("[]"));

    w.runner.prepare_review(&id).unwrap().run(&Quiet);

    let prompt = w.runtime.last_prompt();
    assert!(prompt.contains("SINCE YOUR LAST REVIEW"));
    assert!(prompt.contains("- design: revision 1 → 2"));
    assert!(
        !prompt.contains("- proposal:"),
        "unchanged documents are not listed as changed"
    );
    assert!(
        prompt.contains("===== proposal.md"),
        "but they are all still there"
    );
}

#[test]
fn a_revision_that_says_the_same_leaves_what_is_written_from_it_as_it_was() {
    let w = world();
    let id = started(&w);
    all_documents(&w, &id);
    let revision = w
        .specs
        .get(&id)
        .unwrap()
        .artifact("specs")
        .unwrap()
        .latest_number()
        .unwrap();
    w.specs
        .decide(
            &id,
            "specs",
            revision,
            DecisionKind::Revise,
            Some("Look again"),
        )
        .unwrap();
    // Only blanks at the ends of lines differ from what was approved.
    w.runtime
        .say(&doc("specs/dark-mode/spec.md", &format!("{SPEC}  \n")));

    generate(&w, &id, "specs");

    let spec = w.specs.get(&id).unwrap();
    assert_eq!(spec.artifact("specs").unwrap().revisions.len(), 2);
    assert!(spec.artifact("design").unwrap().outdated.is_none());
    assert!(spec.artifact("tasks").unwrap().outdated.is_none());
}

#[test]
fn answering_several_findings_at_once_asks_each_document_for_changes_with_all_its_answers() {
    let w = world();
    let id = started(&w);
    all_documents(&w, &id);
    w.runtime.say(&review_answer(
        r#"[{"category":"business_rule","severity":"medium","artifact":"specs","title":"First","description":"d","question":"Q1?"},
            {"category":"risk","severity":"high","artifact":"design","title":"Second","description":"d","question":"Q2?"},
            {"category":"risk","severity":"low","artifact":"design","title":"Third","description":"d","question":"Q3?"}]"#,
    ));
    w.runner.prepare_review(&id).unwrap().run(&Quiet);
    let ids: Vec<String> = w
        .specs
        .get(&id)
        .unwrap()
        .findings
        .iter()
        .map(|f| f.id.clone())
        .collect();

    // An empty answer anywhere changes nothing.
    assert!(w
        .specs
        .answer_findings(
            &id,
            &[
                (ids[0].clone(), "A1".to_owned()),
                (ids[1].clone(), "  ".to_owned())
            ]
        )
        .is_err());
    assert!(w
        .specs
        .get(&id)
        .unwrap()
        .findings
        .iter()
        .all(|f| f.status == FindingStatus::Open));

    let rewrite = w
        .specs
        .answer_findings(
            &id,
            &ids.iter()
                .enumerate()
                .map(|(i, f)| (f.clone(), format!("A{}", i + 1)))
                .collect::<Vec<_>>(),
        )
        .unwrap();

    // The first document in order is the one to write now; the other waits, asked for changes.
    assert_eq!(rewrite.as_deref(), Some("specs"));
    let spec = w.specs.get(&id).unwrap();
    assert!(spec
        .findings
        .iter()
        .all(|f| f.status == FindingStatus::Resolved));
    let asked = |artifact: &str| {
        let last = spec
            .artifact(artifact)
            .unwrap()
            .feedback
            .last()
            .cloned()
            .unwrap();
        assert_eq!(last.kind, DecisionKind::Revise);
        last.note.unwrap()
    };
    assert!(asked("specs").contains("A1"));
    let note = asked("design");
    assert!(note.contains("A2") && note.contains("A3") && !note.contains("A1"));
}

#[test]
fn answering_a_finding_sends_the_document_back_to_its_agent_and_closes_the_finding() {
    let w = world();
    let id = started(&w);
    all_documents(&w, &id);
    w.runtime.say(&review_answer(
        r#"[{"category":"business_rule","severity":"medium","artifact":"specs","title":"Persistence unclear","description":"d","question":"Should the choice persist?"}]"#,
    ));
    w.runner.prepare_review(&id).unwrap().run(&Quiet);
    let finding = w.specs.get(&id).unwrap().findings[0].id.clone();

    let rewrite = w
        .specs
        .answer_findings(&id, &[(finding, "Yes, across restarts".to_owned())])
        .unwrap();

    assert_eq!(rewrite.as_deref(), Some("specs"));
    let view = w.specs.view(&id).unwrap();
    assert_eq!(view.spec.findings[0].status, FindingStatus::Resolved);
    assert_eq!(
        view.spec.findings[0].resolution_note.as_deref(),
        Some("Yes, across restarts")
    );
    assert_eq!(view.status, SpecStatus::ChangesRequested);
    // The agent is told what the person answered, and its document now says something more.
    w.runtime.say(&doc(
        "specs/dark-mode/spec.md",
        &format!("{SPEC}\n\nThe choice persists across restarts."),
    ));
    generate(&w, &id, "specs");
    let prompt = w.runtime.last_prompt();
    assert!(prompt.contains("Yes, across restarts"));
    assert!(prompt.contains("Persistence unclear"));
    // Everything after the changed document has to be looked at again.
    assert!(view.spec.artifact("design").unwrap().outdated.is_none());
    let after = w.specs.view(&id).unwrap();
    assert!(after.spec.artifact("design").unwrap().outdated.is_some());
    assert!(after.spec.artifact("tasks").unwrap().outdated.is_some());
    assert!(!after.review_current, "the review read older text");
}

#[test]
fn a_review_that_is_not_in_the_agreed_shape_fails_and_says_so() {
    let w = world();
    let id = started(&w);
    all_documents(&w, &id);
    w.runtime.say("Looks fine to me!");

    w.runner.prepare_review(&id).unwrap().run(&Quiet);

    let view = w.specs.view(&id).unwrap();
    assert!(matches!(
        &view.spec.ai_review.as_ref().unwrap().run,
        ReviewRun::Failed { message, .. } if message == "review_no_report"
    ));
    assert_eq!(view.status, SpecStatus::Failed);
    assert!(view.spec.findings.is_empty());

    w.runtime.say("```atlas-review\n{ broken\n```");
    w.runner.prepare_review(&id).unwrap().run(&Quiet);
    assert!(matches!(
        &w.specs.get(&id).unwrap().ai_review.as_ref().unwrap().run,
        ReviewRun::Failed { message, .. } if message == "review_malformed"
    ));
}

#[test]
fn the_reviewer_can_ask_the_person_and_the_review_goes_on_after_the_answer() {
    let w = world();
    let id = started(&w);
    all_documents(&w, &id);
    w.runtime.say(QUESTION);

    w.runner.prepare_review(&id).unwrap().run(&Quiet);

    let view = w.specs.view(&id).unwrap();
    assert_eq!(view.status, SpecStatus::WaitingForInput);
    let question_id = view.spec.questions[0].id.clone();
    let (source, artifact) = w
        .specs
        .answer_question(&id, &question_id, Some("Everyone"), None)
        .unwrap();
    assert_eq!(source, crate::domain::spec::QuestionSource::Review);
    assert_eq!(artifact, None);
    w.runtime.say(&review_answer("[]"));
    w.runner.prepare_review(&id).unwrap().run(&Quiet);
    assert!(w.runtime.last_prompt().contains("Everyone"));
    assert!(w.specs.view(&id).unwrap().review_current);
}

#[test]
fn the_review_can_be_waived_only_in_writing() {
    let w = world();
    let id = started(&w);
    all_documents(&w, &id);

    assert!(w
        .specs
        .waive_review(&id, "  ")
        .unwrap_err()
        .is(ErrorCode::SpecNoteRequired));
    w.specs.waive_review(&id, "Small change, no time").unwrap();

    let view = w.specs.approve_final(&id).unwrap();
    assert!(matches!(
        &view.spec.ai_review.as_ref().unwrap().run,
        ReviewRun::Waived { note, .. } if note == "Small change, no time"
    ));
    assert_eq!(view.status, SpecStatus::ReadyForImplementation);
}

#[test]
fn the_final_approval_records_the_warnings_that_were_showing() {
    let w = world();
    let id = ready(&w);

    let spec = w.specs.get(&id).unwrap();

    let approval = spec.final_approval.as_ref().unwrap();
    assert_eq!(approval.revisions, spec.current_revisions());
}

// ---- publishing --------------------------------------------------------------------------------------------------

#[test]
fn a_ready_spec_is_published_to_the_project_and_confirmed_by_reading_it_back() {
    let w = world();
    let id = ready(&w);
    assert!(
        w.files.paths().is_empty(),
        "nothing reached the project before this"
    );

    let view = w.specs.publish(&id, None).unwrap();

    assert_eq!(view.status, SpecStatus::Published);
    let paths = w.files.paths();
    assert_eq!(
        paths,
        vec![
            "openspec/changes/dark-mode/.openspec.yaml",
            "openspec/changes/dark-mode/design.md",
            "openspec/changes/dark-mode/proposal.md",
            "openspec/changes/dark-mode/specs/dark-mode/spec.md",
            "openspec/changes/dark-mode/tasks.md",
        ]
    );
    assert!(w
        .files
        .get("openspec/changes/dark-mode/.openspec.yaml")
        .unwrap()
        .starts_with("schema: spec-driven\ncreated: "));
    match &view.spec.publication {
        Publication::Published { root, files, .. } => {
            assert_eq!(root, "openspec/changes/dark-mode");
            assert_eq!(files.len(), 5);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_unfinished_spec_is_not_published() {
    let w = world();
    let id = started(&w);
    all_documents(&w, &id);

    let refused = w.specs.publish(&id, None);

    assert!(refused.unwrap_err().is(ErrorCode::SpecNotReady));
    assert!(w.files.paths().is_empty());
    assert_eq!(
        w.specs.get(&id).unwrap().publication,
        Publication::NotPublished
    );
}

#[test]
fn a_change_that_is_already_in_the_project_is_never_overwritten_and_another_id_is_offered() {
    let w = world();
    let id = ready(&w);
    w.files
        .put("openspec/changes/dark-mode/proposal.md", "someone else's");

    let refused = w.specs.publish(&id, None).unwrap_err();

    assert!(refused.is(ErrorCode::SpecChangeExists));
    assert_eq!(refused.params["change"], "dark-mode");
    assert_eq!(
        w.files
            .get("openspec/changes/dark-mode/proposal.md")
            .as_deref(),
        Some("someone else's")
    );
    assert_eq!(w.files.paths().len(), 1, "not a single file was written");
    assert_eq!(
        w.specs.get(&id).unwrap().publication,
        Publication::NotPublished
    );

    let view = w.specs.publish(&id, Some("dark-mode-v2")).unwrap();

    assert_eq!(view.spec.change_id, "dark-mode-v2");
    assert!(w
        .files
        .get("openspec/changes/dark-mode-v2/proposal.md")
        .is_some());
    assert_eq!(
        w.files
            .get("openspec/changes/dark-mode/proposal.md")
            .as_deref(),
        Some("someone else's")
    );
}

#[test]
fn an_id_that_is_not_kebab_case_is_refused_before_anything_is_written() {
    let w = world();
    let id = ready(&w);

    let refused = w.specs.publish(&id, Some("../../etc")).unwrap_err();

    assert!(refused.is(ErrorCode::SpecChangeIdInvalid));
    assert!(w.files.paths().is_empty());
}

#[test]
fn a_publication_that_stopped_half_way_is_picked_up_without_duplicating_anything() {
    let w = world();
    let id = ready(&w);
    w.files
        .fail_on
        .lock_or_recover()
        .push("openspec/changes/dark-mode/specs/dark-mode/spec.md".to_owned());

    let failed = w.specs.publish(&id, None).unwrap_err();

    assert!(failed.is(ErrorCode::SpecPublishFailed));
    let mid = w.specs.view(&id).unwrap();
    assert!(
        matches!(&mid.spec.publication, Publication::Failed { written, .. } if !written.is_empty())
    );
    assert!(
        !mid.readiness.ready,
        "a publication left half way is not a coherent state"
    );
    assert!(mid
        .readiness
        .blockers
        .iter()
        .any(|b| b.code == "publication_failed"));
    let partial = w.files.paths().len();
    assert!(partial > 0 && partial < 5);

    w.files.fail_on.lock_or_recover().clear();
    let view = w.specs.publish(&id, None).unwrap();

    assert_eq!(view.status, SpecStatus::Published);
    assert_eq!(w.files.paths().len(), 5);
}

#[test]
fn publishing_again_is_harmless() {
    let w = world();
    let id = ready(&w);
    w.specs.publish(&id, None).unwrap();
    let before = w.files.paths();

    let view = w.specs.publish(&id, None).unwrap();

    assert_eq!(view.status, SpecStatus::Published);
    assert_eq!(w.files.paths(), before);
}

#[test]
fn a_spec_changed_after_publishing_has_to_be_published_again_and_may_replace_its_own_files() {
    let w = world();
    let id = ready(&w);
    w.specs.publish(&id, None).unwrap();
    w.specs
        .decide(&id, "tasks", 1, DecisionKind::Redo, None)
        .unwrap();
    w.runtime.say(&doc(
        "tasks.md",
        &format!("{TASKS}- [ ] 1.3 Extra step and verify it\n"),
    ));
    generate(&w, &id, "tasks");
    approve(&w, &id, "tasks");
    w.runtime.say(&review_answer("[]"));
    w.runner.prepare_review(&id).unwrap().run(&Quiet);
    let stale = w.specs.view(&id).unwrap();
    assert!(stale
        .readiness
        .blockers
        .iter()
        .any(|b| b.code == "publication_outdated"));
    w.specs.approve_final(&id).unwrap();

    let view = w.specs.publish(&id, None).unwrap();

    assert_eq!(view.status, SpecStatus::Published);
    assert!(w
        .files
        .get("openspec/changes/dark-mode/tasks.md")
        .unwrap()
        .contains("1.3 Extra step"));
    assert_eq!(
        w.files.paths().len(),
        5,
        "no file was duplicated or left behind"
    );
}

#[test]
fn archiving_closes_the_record_of_a_published_spec_and_nothing_else() {
    let w = world();
    let id = ready(&w);

    assert!(w.specs.archive_record(&id, None, "atlas").is_err());
    w.specs.publish(&id, None).unwrap();
    let view = w.specs.archive_record(&id, None, "atlas").unwrap();

    assert_eq!(view.status, SpecStatus::Archived);
    assert_eq!(
        w.files.paths().len(),
        5,
        "the project's files are not touched"
    );
    assert!(w
        .specs
        .decide(&id, "proposal", 1, DecisionKind::Redo, None)
        .is_err());
}

// ---- importing --------------------------------------------------------------------------------------------------------

fn put_change(w: &World, id: &str, with_tasks: bool) {
    let base = format!("openspec/changes/{id}");
    w.files.put(
        &format!("{base}/.openspec.yaml"),
        "schema: spec-driven\ncreated: 2026-01-01\n",
    );
    w.files.put(&format!("{base}/proposal.md"), PROPOSAL);
    w.files
        .put(&format!("{base}/specs/dark-mode/spec.md"), SPEC);
    w.files.put(&format!("{base}/design.md"), DESIGN);
    if with_tasks {
        w.files.put(&format!("{base}/tasks.md"), TASKS);
    }
}

#[test]
fn the_changes_in_the_project_are_listed_with_what_they_lack() {
    let w = world();
    put_change(&w, "add-dark-mode", false);
    w.files.put(
        "openspec/changes/archive/2026-01-01-old/proposal.md",
        "# old",
    );

    let found = w.specs.importable(&w.workspace_id, &w.project_id).unwrap();

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].change_id, "add-dark-mode");
    assert!(found[0].problems.contains(&"missing:tasks".to_owned()));
    assert_eq!(found[0].imported_as, None);
}

#[test]
fn an_imported_change_is_read_as_it_is_and_nothing_is_approved_for_it() {
    let w = world();
    put_change(&w, "add-dark-mode", true);
    let files_before = w.files.paths();

    let view = w
        .specs
        .import(
            &w.workspace_id,
            &w.project_id,
            "add-dark-mode",
            &w.agent_id,
            None,
        )
        .unwrap();

    assert!(matches!(
        view.spec.origin,
        crate::domain::spec::SpecOrigin::Imported { .. }
    ));
    assert_eq!(view.spec.change_id, "add-dark-mode");
    assert_eq!(
        view.spec
            .artifacts
            .iter()
            .filter(|a| a.latest().is_some())
            .count(),
        4
    );
    assert!(
        view.spec.artifacts.iter().all(|a| !a.is_approved()),
        "the person decides"
    );
    assert_eq!(view.status, SpecStatus::AwaitingReview);
    assert!(
        view.spec.artifact("proposal").unwrap().validation.is_some(),
        "it is checked on the way in"
    );
    assert!(matches!(
        view.spec.publication,
        Publication::Published { .. }
    ));
    assert_eq!(w.files.paths(), files_before, "the project was only read");
    assert_eq!(
        view.spec.artifact("proposal").unwrap().revisions[0].files[0].content,
        PROPOSAL,
        "the content is kept as it was"
    );
}

#[test]
fn a_change_is_not_imported_twice() {
    let w = world();
    put_change(&w, "add-dark-mode", true);
    w.specs
        .import(
            &w.workspace_id,
            &w.project_id,
            "add-dark-mode",
            &w.agent_id,
            None,
        )
        .unwrap();

    let again = w.specs.import(
        &w.workspace_id,
        &w.project_id,
        "add-dark-mode",
        &w.agent_id,
        None,
    );

    assert!(again.unwrap_err().is(ErrorCode::SpecAlreadyImported));
    let listed = w.specs.importable(&w.workspace_id, &w.project_id).unwrap();
    assert!(listed[0].imported_as.is_some());
}

#[test]
fn an_incomplete_change_is_imported_and_the_missing_document_can_be_written_from_the_rest() {
    let w = world();
    put_change(&w, "add-dark-mode", false);
    let id = w
        .specs
        .import(
            &w.workspace_id,
            &w.project_id,
            "add-dark-mode",
            &w.agent_id,
            None,
        )
        .unwrap()
        .spec
        .id;
    for artifact in ["proposal", "specs", "design"] {
        approve(&w, &id, artifact);
    }

    let view = w.specs.view(&id).unwrap();
    assert_eq!(view.ready_to_generate, vec!["tasks".to_owned()]);
    w.runtime.say(&answer_for("tasks"));
    generate(&w, &id, "tasks");

    assert!(w.runtime.last_prompt().contains("write consistently"));
    assert_eq!(
        w.specs
            .get(&id)
            .unwrap()
            .artifact("tasks")
            .unwrap()
            .revisions
            .len(),
        1
    );
}

#[test]
fn a_change_that_is_not_there_or_has_a_bad_name_is_not_imported() {
    let w = world();

    let missing = w
        .specs
        .import(&w.workspace_id, &w.project_id, "ghost", &w.agent_id, None);
    let bad = w.specs.import(
        &w.workspace_id,
        &w.project_id,
        "../ghost",
        &w.agent_id,
        None,
    );

    assert!(missing.unwrap_err().is(ErrorCode::SpecImportFailed));
    assert!(bad.unwrap_err().is(ErrorCode::SpecChangeIdInvalid));
}

// ---- starting an implementation -----------------------------------------------------------------------------------------

#[test]
fn only_a_ready_spec_of_the_workspace_can_be_implemented() {
    let w = world();
    let id = started(&w);
    all_documents(&w, &id);

    assert!(w.specs.ready_for_workspace(&w.workspace_id).is_empty());
    assert!(w
        .specs
        .implementation_context(&id, &w.workspace_id)
        .unwrap_err()
        .is(ErrorCode::SpecNotReady));

    w.specs.waive_review(&id, "ok").unwrap();
    w.specs.approve_final(&id).unwrap();

    let ready = w.specs.ready_for_workspace(&w.workspace_id);
    assert_eq!(ready.len(), 1);
    assert!(w.specs.ready_for_workspace("other-workspace").is_empty());
    assert!(w
        .specs
        .implementation_context(&id, "other-workspace")
        .unwrap_err()
        .is(ErrorCode::SpecNotFound));
    let context = w
        .specs
        .implementation_context(&id, &w.workspace_id)
        .unwrap();
    assert!(
        context.digest.contains("1.1"),
        "the task list is in the digest"
    );
    assert!(context.task.contains("Dark mode"));
    assert_eq!(context.tasks.len(), 2);
}

#[test]
fn a_spec_that_an_implementation_started_from_cannot_be_deleted() {
    let w = world();
    let id = ready(&w);
    let link = crate::domain::spec::ImplementationLink {
        workflow_execution_id: "wfx-1".to_owned(),
        workflow_id: "wf-1".to_owned(),
        workflow_name: "Feature".to_owned(),
        revisions: w.specs.get(&id).unwrap().current_revisions(),
        started_at: 1,
    };
    w.specs.link_implementation(&id, link).unwrap();

    assert!(w.specs.delete(&id).unwrap_err().is(ErrorCode::SpecInUse));
    let other = w.specs.create(new_request(&w)).unwrap().spec.id;
    w.specs.delete(&other).unwrap();
    assert!(w.specs.get(&other).is_none());
    assert!(w.store.stored(&other).is_none());
}

#[test]
fn the_specs_of_a_workspace_that_goes_away_go_with_it() {
    let w = world();
    let id = started(&w);

    w.specs.discard_workspace(&w.workspace_id);

    assert!(w.specs.get(&id).is_none());
    assert!(w.store.stored(&id).is_none());
}

#[test]
fn the_list_shows_where_each_spec_is() {
    let w = world();
    let id = started(&w);
    w.runtime.say(&answer_for("proposal"));
    generate(&w, &id, "proposal");

    let list = w.specs.list(&w.workspace_id);

    assert_eq!(list.len(), 1);
    assert_eq!(list[0].status, SpecStatus::AwaitingReview);
    assert_eq!(list[0].approved, 0);
    assert_eq!(list[0].stage_total, 4);
    assert_eq!(list[0].current_artifact.as_deref(), Some("proposal"));
    assert!(w.specs.list("elsewhere").is_empty());
}

#[test]
fn agents_that_are_gone_are_said_so_and_can_be_replaced() {
    let w = world();
    let id = started(&w);
    w.workspaces
        .remove_agent(&w.workspace_id, &w.agent_id)
        .unwrap();

    let view = w.specs.view(&id).unwrap();
    assert!(!view.author_available);
    assert!(w.runner.prepare_generation(&id, "proposal").is_err());

    w.specs
        .assign_agents(&id, &w.other_agent_id, &w.other_agent_id)
        .unwrap();
    w.runtime.say(&answer_for("proposal"));
    generate(&w, &id, "proposal");
    assert_eq!(
        w.specs
            .get(&id)
            .unwrap()
            .artifact("proposal")
            .unwrap()
            .revisions
            .len(),
        1
    );
}

// ---- the workflow ----------------------------------------------------------------------------------------------

fn workflow_in(w: &World, workspace_id: &str) -> String {
    w.workflows
        .create(NewWorkflow {
            workspace_id: workspace_id.to_owned(),
            name: "Feature".to_owned(),
            description: String::new(),
            mode: WorkflowMode::Custom,
            nodes: vec![
                agent_node("dev", &w.agent_id),
                end("done", EndOutcome::Done),
            ],
            edges: vec![edge("dev", "done")],
            viewport: None,
        })
        .unwrap()
        .id
}

#[test]
fn a_ready_spec_starts_a_workflow_that_carries_what_it_was_given_of_it() {
    let w = world();
    let id = ready(&w);
    let workflow = workflow_in(&w, &w.workspace_id);

    let run = w.work.start(&id, &workflow, "", false, None).unwrap();

    let link = run.spec.as_ref().expect("the run keeps its spec");
    assert_eq!(link.spec_id, id);
    assert_eq!(link.change_id, "dark-mode");
    assert_eq!(
        link.revisions,
        w.specs.get(&id).unwrap().current_revisions()
    );
    assert!(link.digest.contains("1.1"));
    assert_eq!(link.tasks.len(), 2);
    assert!(run.task.contains("Dark mode"), "{}", run.task);
    assert_eq!(
        run.scope
            .as_ref()
            .and_then(|s| s.working_project_id.clone()),
        Some(w.project_id.clone()),
        "the run works in the spec's project"
    );
    let spec = w.specs.get(&id).unwrap();
    assert_eq!(spec.implementations.len(), 1);
    assert_eq!(spec.implementations[0].workflow_execution_id, run.id);
    assert_eq!(spec.implementations[0].revisions, link.revisions);
}

#[test]
fn a_spec_that_is_not_ready_cannot_start_a_workflow_and_nothing_is_started() {
    let w = world();
    let id = started(&w);
    all_documents(&w, &id);
    let workflow = workflow_in(&w, &w.workspace_id);

    let refused = w.work.start(&id, &workflow, "", false, None).unwrap_err();

    assert!(refused.is(ErrorCode::SpecNotReady));
    assert!(w.workflows.executions(&w.workspace_id, None).is_empty());
    assert!(w.specs.get(&id).unwrap().implementations.is_empty());
}

#[test]
fn a_workflow_of_another_workspace_is_not_started_from_this_spec() {
    let w = world();
    let id = ready(&w);
    let other = w
        .workspaces
        .create(&WorkspaceInput {
            name: "Other".to_owned(),
            project_path: "/other".to_owned(),
            description: None,
        })
        .unwrap();
    w.workspaces.add_agent(&other.id, &w.agent_id).unwrap();
    let workflow = workflow_in(&w, &other.id);

    let refused = w.work.start(&id, &workflow, "", false, None).unwrap_err();

    assert!(refused.is(ErrorCode::SpecNotFound));
}

#[test]
fn the_run_works_in_the_specs_project_and_in_no_other() {
    let w = world();
    let id = ready(&w);
    let workflow = workflow_in(&w, &w.workspace_id);

    let elsewhere = crate::domain::scope::ScopeRequest {
        working_project_id: Some("somewhere-else".to_owned()),
        read_project_ids: Vec::new(),
    };
    let refused = w
        .work
        .start(&id, &workflow, "", false, Some(elsewhere))
        .unwrap_err();

    assert_eq!(refused.params["reason"], "project_mismatch");
}

#[test]
fn a_run_not_started_from_a_spec_is_exactly_what_it_always_was() {
    let w = world();
    let workflow = workflow_in(&w, &w.workspace_id);

    let run = w
        .workflows
        .start_scoped(&workflow, "Just do it", false, None)
        .unwrap();

    assert!(run.spec.is_none());
    let json = serde_json::to_value(&run).unwrap();
    assert!(json.get("spec").is_none(), "nothing new in what is stored");
}

#[test]
fn the_progress_of_an_implementation_follows_the_run_and_the_spec() {
    let w = world();
    let id = ready(&w);
    let workflow = workflow_in(&w, &w.workspace_id);
    let run = w.work.start(&id, &workflow, "", false, None).unwrap();

    let before = w.work.progress(&id).unwrap();

    assert_eq!(before.len(), 1);
    assert_eq!(before[0].tasks.len(), 2);
    assert_eq!((before[0].reported, before[0].confirmed), (0, 0));
    assert!(!before[0].spec_changed);

    // The spec is changed after the run began.
    w.specs
        .decide(&id, "tasks", 1, DecisionKind::Redo, None)
        .unwrap();
    w.runtime.say(&answer_for("tasks"));
    generate(&w, &id, "tasks");
    let after = w.work.progress(&id).unwrap();
    assert!(after[0].spec_changed);
    assert_eq!(after[0].workflow_execution_id, run.id);
}

// ---- the optional OpenSpec command ------------------------------------------------------------------------------

use crate::application::process::fake::{ok, FakeProcessRunner};
use crate::application::process::ProcessSpec;
use crate::application::process::{ProcessError, ProcessOutput};

const CLI_VALID: &str =
    r#"{"items":[{"id":"dark-mode","type":"change","valid":true,"issues":[]}],"version":"1.0"}"#;

fn with_cli(
    w: &World,
    installed: bool,
    handler: impl Fn(&ProcessSpec) -> Result<ProcessOutput, ProcessError> + Send + Sync + 'static,
) -> (SpecCliService, Arc<FakeProcessRunner>) {
    let runner = Arc::new(FakeProcessRunner::new(
        if installed { &["openspec"] } else { &[] },
        handler,
    ));
    (
        SpecCliService::new(
            w.specs.clone(),
            w.workflows.clone(),
            OpenSpecCli::new(runner.clone()),
            w.files.clone(),
        ),
        runner,
    )
}

fn ok_cli() -> impl Fn(&ProcessSpec) -> Result<ProcessOutput, ProcessError> + Send + Sync {
    |spec| match spec.args[0].as_str() {
        "--version" => ok("1.3.1"),
        "validate" => ok(CLI_VALID),
        _ => ok("Change 'dark-mode' archived as '2026-10-13-dark-mode'.\n"),
    }
}

#[test]
fn the_whole_module_works_with_the_command_missing_and_says_so_when_asked_about_it() {
    let w = world();
    let id = ready(&w);
    w.specs.publish(&id, None).unwrap();
    let (cli, runner) = with_cli(&w, false, |_| ok(""));

    // Writing, checking, publishing, importing and the final approval needed no command at all.
    assert_eq!(w.specs.view(&id).unwrap().status, SpecStatus::Published);
    let status = cli.status(&id).unwrap();
    assert!(!status.available);
    assert_eq!(status.reason.as_deref(), Some("not_installed"));
    assert!(cli
        .check(&id)
        .unwrap_err()
        .is(ErrorCode::SpecCliUnavailable));
    assert!(cli
        .archive(&id)
        .unwrap_err()
        .is(ErrorCode::SpecCliUnavailable));
    assert!(runner.calls.lock().unwrap().is_empty());
    assert_eq!(
        w.specs.view(&id).unwrap().status,
        SpecStatus::Published,
        "nothing changed"
    );
}

#[test]
fn the_command_looks_at_the_published_change_and_what_it_says_is_kept_as_a_second_opinion() {
    let w = world();
    let id = ready(&w);
    let (cli, runner) = with_cli(&w, true, ok_cli());

    // Nothing to look at before it is published.
    assert!(cli.check(&id).unwrap_err().is(ErrorCode::SpecStateInvalid));
    w.specs.publish(&id, None).unwrap();
    let view = cli.check(&id).unwrap();

    let check = view.spec.cli_check.as_ref().unwrap();
    assert!(check.valid);
    assert_eq!(check.cli_version, "1.3.1");
    assert_eq!(check.revisions, view.spec.current_revisions());
    let validate = runner
        .calls
        .lock()
        .unwrap()
        .iter()
        .find(|c| c.args[0] == "validate")
        .cloned()
        .unwrap();
    assert_eq!(validate.args[1], "dark-mode");
    assert_eq!(
        validate.cwd.as_deref().and_then(|p| p.to_str()),
        Some("/atlas")
    );
    assert!(view.readiness.ready, "it never decides readiness");
}

#[test]
fn a_problem_the_command_finds_is_shown_and_does_not_block_the_spec() {
    let w = world();
    let id = ready(&w);
    w.specs.publish(&id, None).unwrap();
    let (cli, _) = with_cli(&w, true, |spec| {
        match spec.args[0].as_str() {
        "--version" => ok("1.3.1"),
        _ => Ok(ProcessOutput {
            exit_code: Some(1),
            stdout: r#"{"items":[{"id":"dark-mode","valid":false,"issues":[{"level":"ERROR","path":"x/spec.md","message":"nope"}]}]}"#.to_owned(),
            stderr: String::new(),
        }),
    }
    });

    let view = cli.check(&id).unwrap();

    let check = view.spec.cli_check.as_ref().unwrap();
    assert!(!check.valid);
    assert_eq!(check.issues[0].message, "nope");
    assert!(view.readiness.ready);
}

#[test]
fn the_archive_moves_the_change_with_the_command_and_keeps_the_spec_and_its_history() {
    let w = world();
    let id = ready(&w);
    let run = {
        let workflow = workflow_in(&w, &w.workspace_id);
        w.work.start(&id, &workflow, "", false, None).unwrap()
    };
    w.specs.publish(&id, None).unwrap();
    w.files.put(
        "openspec/changes/archive/2026-10-13-dark-mode/proposal.md",
        "# archived",
    );
    let (cli, runner) = with_cli(&w, true, ok_cli());

    // A run started from the spec that is still going keeps it from being archived.
    let blocked = cli.archive(&id).unwrap_err();
    assert_eq!(blocked.params["reason"], "implementation_running");
    assert!(runner
        .calls
        .lock()
        .unwrap()
        .iter()
        .all(|c| c.args[0] != "archive"));

    // Once it ended well, the person can have it archived.
    let mut ended = w.workflows.execution(&run.id).unwrap();
    ended.status = crate::domain::workflow::WorkflowExecutionStatus::Completed;
    w.workflows.save_execution(&ended).unwrap();
    let view = cli.archive(&id).unwrap();

    assert_eq!(view.status, SpecStatus::Archived);
    let archived = view.spec.archived.as_ref().unwrap();
    assert_eq!(archived.method, "cli");
    assert_eq!(
        archived.path.as_deref(),
        Some("openspec/changes/archive/2026-10-13-dark-mode")
    );
    assert_eq!(view.spec.implementations.len(), 1, "the history stays");
    assert!(view.spec.artifacts.iter().all(|a| !a.revisions.is_empty()));
    let call = runner
        .calls
        .lock()
        .unwrap()
        .iter()
        .find(|c| c.args[0] == "archive")
        .cloned()
        .unwrap();
    assert_eq!(call.args, ["archive", "dark-mode", "--yes"]);
}

#[test]
fn an_archive_the_command_refuses_leaves_the_spec_as_it_was() {
    let w = world();
    let id = ready(&w);
    w.specs.publish(&id, None).unwrap();
    let before = w.specs.get(&id).unwrap();
    let (cli, _) = with_cli(&w, true, |spec| match spec.args[0].as_str() {
        "--version" => ok("1.3.1"),
        _ => Ok(ProcessOutput {
            exit_code: Some(1),
            stdout: String::new(),
            stderr: "Validation failed".to_owned(),
        }),
    });

    let error = cli.archive(&id).unwrap_err();

    assert!(error.is(ErrorCode::SpecCliFailed));
    assert!(error
        .detail
        .as_deref()
        .unwrap()
        .contains("Validation failed"));
    assert_eq!(w.specs.get(&id).unwrap(), before);
}

#[test]
fn a_spec_that_is_not_ready_or_not_published_as_it_stands_is_not_archived() {
    let w = world();
    let id = started(&w);
    all_documents(&w, &id);
    let (cli, runner) = with_cli(&w, true, ok_cli());

    assert!(cli.archive(&id).unwrap_err().is(ErrorCode::SpecNotReady));

    w.specs.waive_review(&id, "ok").unwrap();
    w.specs.approve_final(&id).unwrap();
    assert_eq!(
        cli.archive(&id).unwrap_err().params["reason"],
        "not_published"
    );
    assert!(runner
        .calls
        .lock()
        .unwrap()
        .iter()
        .all(|c| c.args[0] != "archive"));
}
