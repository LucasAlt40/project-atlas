use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::Deserialize;

use super::agents::AgentService;
use super::errors::{AppError, ErrorCode};
use super::personalities::PersonalityService;
use super::prompt::PromptBuilder;
use super::runtimes::{RuntimeEvent, RuntimeRegistry, RuntimeRequest};
use super::support::now_ms;
use super::workspace::WorkspaceService;
use crate::domain::execution::{Execution, ExecutionEvent, ExecutionEventKind, ExecutionRecord};
use crate::domain::task::{Task, TaskStatus};

/// Port: receives progress while an execution runs. The Tauri adapter in `commands/`
/// forwards these to the webview; tests collect them.
pub trait ExecutionObserver: Send + Sync {
    fn on_event(&self, event: &ExecutionEvent);
}

/// What to run: which agent, in which workspace (whose project folder is the working
/// directory), and the task. `task_id` is chosen by the caller.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunAgentRequest {
    pub task_id: String,
    pub workspace_id: String,
    pub agent_id: String,
    pub description: String,
}

/// The request was not acceptable, so no execution was created. Runtime problems are not
/// errors here: they produce a *failed* execution that is recorded like any other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    EmptyTask,
    UnknownAgent(String),
    UnknownPersonality(String),
    UnknownRuntime(String),
    UnknownWorkspace(String),
}

impl From<&ExecutionError> for AppError {
    fn from(error: &ExecutionError) -> Self {
        let code = match error {
            ExecutionError::EmptyTask => ErrorCode::MessageEmpty,
            ExecutionError::UnknownAgent(_) => ErrorCode::AgentNotFound,
            ExecutionError::UnknownPersonality(_) => ErrorCode::PersonalityNotFound,
            ExecutionError::UnknownRuntime(_) => ErrorCode::RuntimeRequired,
            ExecutionError::UnknownWorkspace(_) => ErrorCode::WorkspaceNotFound,
        };
        AppError::new(code).with_detail(error.to_string())
    }
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTask => write!(f, "The task description is empty"),
            Self::UnknownAgent(id) => write!(f, "Unknown agent: {id}"),
            Self::UnknownPersonality(id) => {
                write!(f, "The agent's personality no longer exists: {id}")
            }
            Self::UnknownRuntime(id) => write!(f, "The agent's runtime is unknown: {id}"),
            Self::UnknownWorkspace(id) => write!(f, "Unknown workspace: {id}"),
        }
    }
}

impl std::error::Error for ExecutionError {}

/// Use case: run a task with an agent through its runtime. Knows nothing about any specific
/// runtime.
pub struct ExecutionService {
    agents: Arc<AgentService>,
    personalities: Arc<PersonalityService>,
    runtimes: Arc<RuntimeRegistry>,
    workspaces: Arc<WorkspaceService>,
    next_id: AtomicU64,
}

impl ExecutionService {
    pub fn new(
        agents: Arc<AgentService>,
        personalities: Arc<PersonalityService>,
        runtimes: Arc<RuntimeRegistry>,
        workspaces: Arc<WorkspaceService>,
    ) -> Self {
        Self {
            agents,
            personalities,
            runtimes,
            workspaces,
            next_id: AtomicU64::new(1),
        }
    }

    /// An id for an execution that will be started with [`Self::run_with_id`], for callers
    /// that must tell the user which execution they started before it finishes.
    pub fn next_execution_id(&self) -> String {
        format!("exec-{}", self.next_id.fetch_add(1, Ordering::Relaxed))
    }

    /// Builds the prompt, calls the agent's runtime and returns the outcome. Blocks until the
    /// runtime answers, so call it from a blocking context.
    ///
    /// # Errors
    ///
    /// Fails before any execution is created if the task is empty or the agent, its
    /// personality or runtime, or the workspace is unknown.
    pub fn run_with_id(
        &self,
        id: String,
        request: RunAgentRequest,
        observer: &dyn ExecutionObserver,
    ) -> Result<ExecutionRecord, ExecutionError> {
        let description = request.description.trim();
        if description.is_empty() {
            return Err(ExecutionError::EmptyTask);
        }
        let agent = self
            .agents
            .find(&request.agent_id)
            .ok_or_else(|| ExecutionError::UnknownAgent(request.agent_id.clone()))?;
        let personality = self
            .personalities
            .find(&agent.personality_id)
            .ok_or_else(|| ExecutionError::UnknownPersonality(agent.personality_id.clone()))?;
        let runtime = self
            .runtimes
            .find(&agent.runtime_id)
            .ok_or_else(|| ExecutionError::UnknownRuntime(agent.runtime_id.clone()))?;
        let project = self
            .workspaces
            .project_context(&request.workspace_id)
            .map_err(|_| ExecutionError::UnknownWorkspace(request.workspace_id.clone()))?;

        let mut task = Task {
            id: request.task_id,
            description: description.to_owned(),
            agent_id: agent.id.clone(),
            status: TaskStatus::Pending,
        };
        let prompt = PromptBuilder::build(&personality, &project, &agent, &task);
        let runtime_name = runtime.info().name;
        let mut execution = Execution::start(
            id,
            request.workspace_id.clone(),
            task.id.clone(),
            agent.runtime_id.clone(),
            agent.model_id.clone(),
            prompt.combined(),
            now_ms(),
        );

        task.status = TaskStatus::Running;
        let emitter = Emitter {
            observer,
            execution_id: execution.id.clone(),
            workspace_id: request.workspace_id,
            task_id: task.id.clone(),
            agent_id: agent.id.clone(),
            logs: RefCell::new(vec![
                "Task created".to_owned(),
                format!(
                    "Prompt built from the {} personality and project context",
                    personality.name
                ),
            ]),
        };
        emitter.emit(ExecutionEventKind::Started, "Execution started".to_owned());

        let runtime_request = RuntimeRequest {
            model_id: agent.model_id.clone(),
            prompt,
            working_dir: project.path.into(),
        };
        let outcome = runtime.execute(&runtime_request, &|stage| {
            emitter.runtime_event(stage, &runtime_name, &agent.model_id);
        });

        match outcome {
            Ok(output) => {
                emitter.emit_with(
                    ExecutionEventKind::Completed,
                    "Execution completed".to_owned(),
                    output.metadata.clone(),
                );
                execution.complete(
                    output.text,
                    output.metadata,
                    output.usage,
                    output.quota,
                    now_ms(),
                );
                task.status = TaskStatus::Completed;
            }
            Err(error) => {
                let failure = error.into_failure();
                emitter.emit_with(
                    ExecutionEventKind::Failed,
                    failure.message.clone(),
                    [("failureKind".to_owned(), failure.kind.as_str().to_owned())].into(),
                );
                execution.fail(failure, now_ms());
                task.status = TaskStatus::Failed;
            }
        }
        execution.logs = emitter.logs.into_inner();

        Ok(ExecutionRecord { task, execution })
    }
}

/// Records each step in the execution's log and announces it to the observer, stamped with
/// the ids a listener needs to tell executions apart.
struct Emitter<'a> {
    observer: &'a dyn ExecutionObserver,
    execution_id: String,
    workspace_id: String,
    task_id: String,
    agent_id: String,
    logs: RefCell<Vec<String>>,
}

impl Emitter<'_> {
    /// Announces what a runtime reported. Answer text is streamed to the listener only: it is
    /// not logged, because the complete answer is recorded as the result. The names the UI needs
    /// to word a step in the user's language travel in `metadata`.
    fn runtime_event(&self, event: RuntimeEvent, runtime_name: &str, model_id: &str) {
        let named = |key: &str, value: &str| -> BTreeMap<String, String> {
            [(key.to_owned(), value.to_owned())].into()
        };
        match event {
            RuntimeEvent::Starting => self.emit_with(
                ExecutionEventKind::StartingRuntime,
                format!("Starting {runtime_name}"),
                named("runtime", runtime_name),
            ),
            RuntimeEvent::Sending => self.emit(
                ExecutionEventKind::SendingPrompt,
                "Sending prompt".to_owned(),
            ),
            RuntimeEvent::Waiting => self.emit_with(
                ExecutionEventKind::WaitingForModel,
                format!("Waiting for {model_id}"),
                named("model", model_id),
            ),
            RuntimeEvent::Output(text) => {
                self.announce(ExecutionEventKind::OutputChunk, text, BTreeMap::new());
            }
            RuntimeEvent::ToolStarted(tool) => self.emit_with(
                ExecutionEventKind::ToolStarted,
                format!("Using {tool}"),
                named("tool", &tool),
            ),
            RuntimeEvent::ToolCompleted(tool) => self.emit_with(
                ExecutionEventKind::ToolCompleted,
                format!("Finished {tool}"),
                named("tool", &tool),
            ),
        }
    }

    fn emit(&self, kind: ExecutionEventKind, message: String) {
        self.emit_with(kind, message, BTreeMap::new());
    }

    fn emit_with(
        &self,
        kind: ExecutionEventKind,
        message: String,
        metadata: BTreeMap<String, String>,
    ) {
        self.logs.borrow_mut().push(message.clone());
        self.announce(kind, message, metadata);
    }

    fn announce(
        &self,
        kind: ExecutionEventKind,
        message: String,
        metadata: BTreeMap<String, String>,
    ) {
        self.observer.on_event(&ExecutionEvent {
            execution_id: self.execution_id.clone(),
            workspace_id: self.workspace_id.clone(),
            task_id: self.task_id.clone(),
            agent_id: self.agent_id.clone(),
            kind,
            message,
            timestamp: now_ms(),
            metadata,
        });
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::application::agents::CreateAgentRequest;
    use crate::application::config::memory::MemoryStore;
    use crate::application::config::ConfigRepository;
    use crate::application::projects::fake::FakeInspector;
    use crate::application::runtimes::fake::FakeRuntime;
    use crate::application::runtimes::RuntimeError;
    use crate::application::workspace::WorkspaceInput;
    use crate::domain::execution::{ExecutionStatus, FailureKind};

    #[derive(Default)]
    struct Collector(Mutex<Vec<ExecutionEvent>>);

    impl ExecutionObserver for Collector {
        fn on_event(&self, event: &ExecutionEvent) {
            self.0.lock().unwrap().push(event.clone());
        }
    }

    struct Fixture {
        service: ExecutionService,
        agents: Arc<AgentService>,
        runtimes: Vec<Arc<FakeRuntime>>,
        workspace_id: String,
    }

    /// An execution service over several fake runtimes `(id, scripted answer)`.
    fn fixture(scripted: Vec<(&str, Result<&str, RuntimeError>)>) -> Fixture {
        let config = Arc::new(ConfigRepository::load(Box::<MemoryStore>::default()));
        let personalities = Arc::new(PersonalityService::new(config.clone()));
        let runtimes: Vec<_> = scripted
            .into_iter()
            .map(|(id, answer)| Arc::new(FakeRuntime::new(id, answer)))
            .collect();
        let registry = Arc::new(RuntimeRegistry::new(
            runtimes.iter().map(|r| r.clone() as Arc<_>).collect(),
        ));
        let agents = Arc::new(AgentService::new(
            config.clone(),
            personalities.clone(),
            registry.clone(),
        ));
        let workspaces = Arc::new(WorkspaceService::new(
            config,
            agents.clone(),
            Arc::new(FakeInspector::with(&[("/atlas", &["Rust"])])),
        ));
        let workspace_id = workspaces
            .create(&WorkspaceInput {
                name: "Project Atlas".to_owned(),
                project_path: "/atlas".to_owned(),
                description: None,
            })
            .unwrap()
            .id;
        Fixture {
            service: ExecutionService::new(agents.clone(), personalities, registry, workspaces),
            agents,
            runtimes,
            workspace_id,
        }
    }

    fn create_agent(f: &Fixture, runtime_id: &str) -> String {
        f.agents
            .create(CreateAgentRequest {
                name: format!("{runtime_id} agent"),
                personality_id: "architect".to_owned(),
                runtime_id: runtime_id.to_owned(),
                model_id: "m1".to_owned(),
                instructions: "Be brief.".to_owned(),
            })
            .unwrap()
            .id
    }

    fn request(f: &Fixture, agent_id: &str, description: &str) -> RunAgentRequest {
        RunAgentRequest {
            task_id: "task-1".to_owned(),
            workspace_id: f.workspace_id.clone(),
            agent_id: agent_id.to_owned(),
            description: description.to_owned(),
        }
    }

    fn run(
        f: &Fixture,
        agent_id: &str,
        description: &str,
        events: &Collector,
    ) -> Result<ExecutionRecord, ExecutionError> {
        f.service.run_with_id(
            f.service.next_execution_id(),
            request(f, agent_id, description),
            events,
        )
    }

    #[test]
    fn sends_personality_workspace_project_and_instruction_to_the_selected_model() {
        let f = fixture(vec![("rt-a", Ok("Three improvements."))]);
        let agent_id = create_agent(&f, "rt-a");
        let events = Collector::default();

        let record = run(&f, &agent_id, "  Find improvements ", &events).unwrap();

        let sent = f.runtimes[0].requests.lock().unwrap()[0].clone();
        assert_eq!(sent.model_id, "m1");
        assert_eq!(sent.working_dir, std::path::PathBuf::from("/atlas"));
        let prompt = sent.prompt.combined();
        assert!(prompt.contains("experienced software architect"));
        assert!(prompt.contains("Project: atlas"));
        assert!(prompt.contains("- Rust"));
        assert!(prompt.contains("Be brief."));
        assert!(prompt.contains("Find improvements"));
        assert_eq!(record.execution.prompt, prompt);
        assert_eq!(record.execution.workspace_id, f.workspace_id);
        assert_eq!(record.task.status, TaskStatus::Completed);
        assert_eq!(record.execution.status, ExecutionStatus::Completed);
        assert_eq!(
            record.execution.result.as_deref(),
            Some("Three improvements.")
        );
        assert_eq!(
            (
                record.execution.runtime_id.as_str(),
                record.execution.model_id.as_str()
            ),
            ("rt-a", "m1")
        );
        assert_eq!(record.execution.metadata["durationMs"], "5");
        let kinds: Vec<_> = events.0.lock().unwrap().iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            [
                ExecutionEventKind::Started,
                ExecutionEventKind::StartingRuntime,
                ExecutionEventKind::SendingPrompt,
                ExecutionEventKind::WaitingForModel,
                ExecutionEventKind::Completed,
            ]
        );
    }

    #[test]
    fn events_carry_the_workspace_and_the_names_the_ui_needs_to_word_them() {
        let f = fixture(vec![("rt-a", Ok("x"))]);
        let agent_id = create_agent(&f, "rt-a");
        let events = Collector::default();

        run(&f, &agent_id, "go", &events).unwrap();

        let events = events.0.lock().unwrap();
        assert!(events
            .iter()
            .all(|e| e.workspace_id == f.workspace_id && e.agent_id == agent_id));
        let starting = events
            .iter()
            .find(|e| e.kind == ExecutionEventKind::StartingRuntime)
            .unwrap();
        assert_eq!(starting.metadata["runtime"], "Fake rt-a");
        let waiting = events
            .iter()
            .find(|e| e.kind == ExecutionEventKind::WaitingForModel)
            .unwrap();
        assert_eq!(waiting.metadata["model"], "m1");
    }

    #[test]
    fn runs_agents_of_different_runtimes_through_the_same_path() {
        let f = fixture(vec![("rt-a", Ok("from A")), ("rt-b", Ok("from B"))]);
        let (a, b) = (create_agent(&f, "rt-a"), create_agent(&f, "rt-b"));
        let events = Collector::default();

        let from_a = run(&f, &a, "x", &events).unwrap();
        let from_b = run(&f, &b, "x", &events).unwrap();

        assert_eq!(from_a.execution.result.as_deref(), Some("from A"));
        assert_eq!(from_b.execution.result.as_deref(), Some("from B"));
        let prompts: Vec<_> = f
            .runtimes
            .iter()
            .map(|r| r.requests.lock().unwrap()[0].prompt.combined())
            .collect();
        assert_eq!(prompts[0], prompts[1]);
    }

    #[test]
    fn records_runtime_failures_as_failed_executions() {
        let f = fixture(vec![("rt-a", Err(RuntimeError::AuthenticationRequired))]);
        let agent_id = create_agent(&f, "rt-a");
        let events = Collector::default();

        let record = run(&f, &agent_id, "x", &events).unwrap();

        assert_eq!(record.task.status, TaskStatus::Failed);
        assert_eq!(record.execution.status, ExecutionStatus::Failed);
        let failure = record.execution.failure.unwrap();
        assert_eq!(failure.kind, FailureKind::AuthenticationRequired);
        assert_eq!(
            events.0.lock().unwrap().last().map(|e| e.kind),
            Some(ExecutionEventKind::Failed)
        );
        assert_eq!(record.execution.usage, None);
    }

    #[test]
    fn rejects_invalid_requests_without_creating_an_execution() {
        let f = fixture(vec![("rt-a", Ok(""))]);
        let agent_id = create_agent(&f, "rt-a");
        let events = Collector::default();

        assert_eq!(
            run(&f, &agent_id, "  ", &events),
            Err(ExecutionError::EmptyTask)
        );
        assert_eq!(
            run(&f, "ghost", "x", &events),
            Err(ExecutionError::UnknownAgent("ghost".to_owned()))
        );
        let mut bad_workspace = request(&f, &agent_id, "x");
        bad_workspace.workspace_id = "nope".to_owned();
        assert_eq!(
            f.service
                .run_with_id("e".to_owned(), bad_workspace, &events),
            Err(ExecutionError::UnknownWorkspace("nope".to_owned()))
        );
        assert_eq!(events.0.lock().unwrap().len(), 0);
    }
}
