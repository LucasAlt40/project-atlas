use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use super::agents::AgentService;
use super::errors::{AppError, ErrorCode};
use super::executions::{
    ExecutionError, ExecutionObserver, ExecutionService, RunAgentRequest, StepOptions,
};
use super::history::ConversationHistory;
use super::support::{new_id, now_ms};
use super::usage::UsageLedger;
use super::workspace::WorkspaceService;
use crate::domain::conversation::{Message, MessageRole};
use crate::domain::execution::{
    ExecutionEvent, ExecutionEventKind, ExecutionFailure, ExecutionRecord, ExecutionStatus,
    FailureKind, StoredExecution, WorkflowLink,
};
use crate::domain::interaction::InteractionDetection;
use crate::domain::usage::UsageRecord;

/// Port: everything the UI wants to hear about while a message is being answered. The Tauri
/// adapter in `commands/` turns these into webview events; tests collect them.
pub trait ChatObserver: ExecutionObserver {
    fn on_message(&self, message: &Message);
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageRequest {
    pub workspace_id: String,
    pub agent_id: String,
    pub content: String,
}

/// A step of a workflow run to be done by an agent. The workflow engine decides *that* it runs;
/// everything else about the run is the same as for a message.
#[derive(Debug, Clone)]
pub struct WorkflowStepRequest {
    pub workspace_id: String,
    pub agent_id: String,
    /// What the conversation shows as the request.
    pub display_task: String,
    /// The instruction sent to the agent.
    pub instruction: String,
    /// What the Task Context is selected for.
    pub context_query: String,
    pub link: WorkflowLink,
    /// The primary worktree of the workflow run, when it has one.
    pub shared_worktree: Option<String>,
}

/// How a run ended, as much as a caller driving it needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSummary {
    pub status: ExecutionStatus,
    /// The agent's answer, when it completed.
    pub result: Option<String>,
    /// Why it failed, when it did.
    pub failure: Option<String>,
    /// What it asked, when it ended waiting for a person.
    pub interaction: Option<InteractionDetection>,
}

/// What the caller gets back immediately, before the agent has answered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SentMessage {
    pub user_message: Message,
    /// The execution this message started; its events and its assistant message carry it.
    pub execution_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatError {
    EmptyMessage,
    UnknownAgent(String),
    UnknownWorkspace(String),
    /// Each agent runs one execution at a time in a workspace (different agents, and the same
    /// agent in different workspaces, run independently).
    AgentBusy(String),
}

impl From<&ChatError> for AppError {
    fn from(error: &ChatError) -> Self {
        let code = match error {
            ChatError::EmptyMessage => ErrorCode::MessageEmpty,
            ChatError::UnknownAgent(_) => ErrorCode::AgentNotFound,
            ChatError::UnknownWorkspace(_) => ErrorCode::WorkspaceNotFound,
            ChatError::AgentBusy(_) => ErrorCode::AgentBusy,
        };
        AppError::new(code)
    }
}

impl fmt::Display for ChatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyMessage => write!(f, "The message is empty"),
            Self::UnknownAgent(id) => write!(f, "Unknown agent: {id}"),
            Self::UnknownWorkspace(id) => write!(f, "Unknown workspace: {id}"),
            Self::AgentBusy(_) => write!(f, "This agent is still working on your last message"),
        }
    }
}

impl std::error::Error for ChatError {}

struct Inner {
    agents: Arc<AgentService>,
    executions: Arc<ExecutionService>,
    workspaces: Arc<WorkspaceService>,
    ledger: Arc<UsageLedger>,
    history: Arc<ConversationHistory>,
    busy: Mutex<HashSet<(String, String)>>,
}

/// Use case: chat with an agent in a workspace. Sits above [`ExecutionService`]: a user message
/// starts an execution, and its outcome becomes the assistant message. A conversation belongs to
/// one agent in one workspace, so the same agent has separate conversations in different
/// workspaces. Conversations and ended executions are kept by [`ConversationHistory`].
#[derive(Clone)]
pub struct ChatService {
    inner: Arc<Inner>,
}

impl ChatService {
    /// A chat whose history is forgotten when the app ends.
    #[cfg(test)]
    pub fn new(
        agents: Arc<AgentService>,
        executions: Arc<ExecutionService>,
        workspaces: Arc<WorkspaceService>,
        ledger: Arc<UsageLedger>,
    ) -> Self {
        Self::with_history(
            agents,
            executions,
            workspaces,
            ledger,
            Arc::new(ConversationHistory::in_memory()),
        )
    }

    pub fn with_history(
        agents: Arc<AgentService>,
        executions: Arc<ExecutionService>,
        workspaces: Arc<WorkspaceService>,
        ledger: Arc<UsageLedger>,
        history: Arc<ConversationHistory>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                agents,
                executions,
                workspaces,
                ledger,
                history,
                busy: Mutex::new(HashSet::new()),
            }),
        }
    }

    /// Messages, oldest first, optionally narrowed to a workspace and/or an agent.
    pub fn messages(&self, workspace_id: Option<&str>, agent_id: Option<&str>) -> Vec<Message> {
        self.inner.history.messages(workspace_id, agent_id)
    }

    /// Ended executions, newest first, optionally narrowed to a workspace and/or an agent.
    pub fn executions(
        &self,
        workspace_id: Option<&str>,
        agent_id: Option<&str>,
    ) -> Vec<StoredExecution> {
        self.inner.history.executions(workspace_id, agent_id)
    }

    /// The executions that have messages in this agent's conversation in the workspace.
    pub fn conversation_executions(&self, workspace_id: &str, agent_id: &str) -> HashSet<String> {
        self.messages(Some(workspace_id), Some(agent_id))
            .into_iter()
            .filter_map(|m| m.execution_id)
            .collect()
    }

    /// Marks the agent busy in the workspace so nothing new can start for it.
    ///
    /// # Errors
    ///
    /// Fails if the agent is already working on a message there.
    pub fn lock_agent(&self, workspace_id: &str, agent_id: &str) -> Result<AgentLock, ChatError> {
        Ok(AgentLock {
            _busy: BusyGuard::acquire(&self.inner, workspace_id, agent_id)?,
        })
    }

    /// Deletes every conversation and execution of the workspace.
    pub fn discard_workspace(&self, workspace_id: &str) {
        self.inner.history.discard_workspace(workspace_id);
    }

    /// Deletes the agent's conversations and executions in every workspace.
    pub fn discard_agent(&self, agent_id: &str) {
        self.inner.history.discard_agent(agent_id);
    }

    /// Records the user's message and reserves the execution it starts. The agent has not
    /// started working yet: run the returned [`PendingRun`] (on a blocking thread) to do it.
    ///
    /// # Errors
    ///
    /// Fails if the message is empty, the agent or workspace is unknown, or the agent is
    /// already busy in that workspace.
    pub fn send(
        &self,
        request: SendMessageRequest,
    ) -> Result<(SentMessage, PendingRun), ChatError> {
        let content = request.content.trim();
        if content.is_empty() {
            return Err(ChatError::EmptyMessage);
        }
        if self.inner.agents.find(&request.agent_id).is_none() {
            return Err(ChatError::UnknownAgent(request.agent_id));
        }
        if self.inner.workspaces.get(&request.workspace_id).is_none() {
            return Err(ChatError::UnknownWorkspace(request.workspace_id));
        }
        let busy = BusyGuard::acquire(&self.inner, &request.workspace_id, &request.agent_id)?;

        let execution_id = self.inner.executions.next_execution_id();
        let user_message = Message {
            id: new_id("msg"),
            workspace_id: request.workspace_id.clone(),
            agent_id: request.agent_id.clone(),
            execution_id: Some(execution_id.clone()),
            role: MessageRole::User,
            content: content.to_owned(),
            timestamp: now_ms(),
            failed: false,
            failure_kind: None,
        };
        self.inner.push(user_message.clone());

        Ok((
            SentMessage {
                user_message,
                execution_id: execution_id.clone(),
            },
            PendingRun {
                inner: self.inner.clone(),
                workspace_id: request.workspace_id,
                agent_id: request.agent_id,
                content: content.to_owned(),
                instruction: content.to_owned(),
                context_query: None,
                shared_worktree: None,
                workflow: None,
                announce: None,
                execution_id,
                _busy: busy,
            },
        ))
    }

    /// Records the request of a workflow step in the agent's conversation and reserves its
    /// execution, exactly as [`Self::send`] does for a message, so the step is an execution like
    /// any other: it shows in history, in the Execution Inspector and in usage.
    ///
    /// # Errors
    ///
    /// Fails if the agent or workspace is unknown, or the agent is busy in that workspace (the
    /// caller waits and tries again: an agent never runs two executions at once).
    pub fn send_workflow_step(
        &self,
        step: WorkflowStepRequest,
    ) -> Result<(SentMessage, PendingRun), ChatError> {
        let (sent, mut pending) = self.send(SendMessageRequest {
            workspace_id: step.workspace_id,
            agent_id: step.agent_id,
            content: step.display_task,
        })?;
        pending.instruction = step.instruction;
        pending.context_query = Some(step.context_query);
        pending.workflow = Some(step.link);
        pending.shared_worktree = step.shared_worktree;
        pending.announce = Some(sent.user_message.clone());
        Ok((sent, pending))
    }
}

/// Holds an agent still in a workspace while something about it is being removed; see
/// [`ChatService::lock_agent`]. Released when dropped.
pub struct AgentLock {
    _busy: BusyGuard,
}

impl Inner {
    fn push(&self, message: Message) {
        self.history.push_message(message);
    }
}

/// Marks an agent busy in a workspace until dropped, even if its run panics.
struct BusyGuard {
    inner: Arc<Inner>,
    key: (String, String),
}

impl BusyGuard {
    fn acquire(inner: &Arc<Inner>, workspace_id: &str, agent_id: &str) -> Result<Self, ChatError> {
        let key = (workspace_id.to_owned(), agent_id.to_owned());
        let mut busy = inner.busy.lock().expect("busy lock poisoned");
        if !busy.insert(key.clone()) {
            return Err(ChatError::AgentBusy(agent_id.to_owned()));
        }
        Ok(Self {
            inner: inner.clone(),
            key,
        })
    }
}

impl Drop for BusyGuard {
    fn drop(&mut self) {
        if let Ok(mut busy) = self.inner.busy.lock() {
            busy.remove(&self.key);
        }
    }
}

/// A message whose answer has not been produced yet.
pub struct PendingRun {
    inner: Arc<Inner>,
    workspace_id: String,
    agent_id: String,
    /// What the conversation shows as the request.
    content: String,
    /// What the agent is sent (the message itself, unless this is a workflow step).
    instruction: String,
    /// What the Task Context is chosen for, when it is not the instruction.
    context_query: Option<String>,
    shared_worktree: Option<String>,
    workflow: Option<WorkflowLink>,
    /// A message nobody was told about yet (a workflow step's request).
    announce: Option<Message>,
    execution_id: String,
    _busy: BusyGuard,
}

impl PendingRun {
    /// Runs the execution through [`ExecutionService`] (blocking), records what it consumed,
    /// then records and announces the assistant message. Progress events go to `observer` as they
    /// happen.
    pub fn run(self, observer: &dyn ChatObserver) -> RunSummary {
        if let Some(message) = &self.announce {
            observer.on_message(message);
        }
        let timeline = Timeline::new(observer);
        let request = RunAgentRequest {
            task_id: new_id("task"),
            workspace_id: self.workspace_id.clone(),
            agent_id: self.agent_id.clone(),
            description: self.instruction.clone(),
        };
        let options = StepOptions {
            context_query: self.context_query.as_deref(),
            shared_worktree: self.shared_worktree.as_deref(),
            // A workflow step that asks a person something waits for the answer; in a
            // conversation the person just replies.
            detect_interaction: self.workflow.is_some(),
        };
        let result = if options.context_query.is_some() || options.detect_interaction {
            self.inner
                .executions
                .run_step(self.execution_id.clone(), request, options, &timeline)
        } else {
            self.inner
                .executions
                .run_with_id(self.execution_id.clone(), request, &timeline)
        };
        let summary = Self::summarize(&result);

        let (content, failure_kind) = match &result {
            Ok(record) => {
                self.record_usage(record);
                match record.execution.status {
                    // The question the person is asked is the agent's last message.
                    ExecutionStatus::Completed | ExecutionStatus::WaitingForInput => {
                        (record.execution.result.clone().unwrap_or_default(), None)
                    }
                    // Stopped by the user: not a failure, but worded like one's absence.
                    ExecutionStatus::Cancelled => (
                        "Execution cancelled.".to_owned(),
                        Some(FailureKind::Cancelled),
                    ),
                    ExecutionStatus::Failed | ExecutionStatus::Running => {
                        let failure = record.execution.failure.as_ref();
                        (
                            failure_text(failure.map_or("", |f| f.message.as_str())),
                            Some(failure.map_or(FailureKind::ExecutionFailed, |f| f.kind)),
                        )
                    }
                }
            }
            Err(error) => {
                // No execution exists, so nothing else announced the failure.
                observer.on_event(&ExecutionEvent {
                    execution_id: self.execution_id.clone(),
                    workspace_id: self.workspace_id.clone(),
                    task_id: String::new(),
                    agent_id: self.agent_id.clone(),
                    kind: ExecutionEventKind::Failed,
                    message: error.to_string(),
                    timestamp: now_ms(),
                    metadata: [(
                        "failureKind".to_owned(),
                        FailureKind::InvalidRequest.as_str().to_owned(),
                    )]
                    .into(),
                });
                (
                    failure_text(&error.to_string()),
                    Some(FailureKind::InvalidRequest),
                )
            }
        };

        self.inner
            .history
            .record_execution(self.stored(&result, timeline.into_events()));

        let assistant = Message {
            id: new_id("msg"),
            workspace_id: self.workspace_id.clone(),
            agent_id: self.agent_id.clone(),
            execution_id: Some(self.execution_id.clone()),
            role: MessageRole::Assistant,
            content,
            timestamp: now_ms(),
            failed: failure_kind.is_some(),
            failure_kind: failure_kind.map(|k| k.as_str().to_owned()),
        };
        self.inner.push(assistant.clone());
        observer.on_message(&assistant);
        summary
    }

    fn summarize(result: &Result<ExecutionRecord, ExecutionError>) -> RunSummary {
        match result {
            Ok(record) => RunSummary {
                status: record.execution.status,
                result: record.execution.result.clone(),
                failure: record.execution.failure.as_ref().map(|f| f.message.clone()),
                interaction: record.execution.interaction.clone(),
            },
            Err(error) => RunSummary {
                status: ExecutionStatus::Failed,
                result: None,
                failure: Some(error.to_string()),
                interaction: None,
            },
        }
    }

    /// What is kept of the execution once it has ended.
    fn stored(
        &self,
        result: &Result<ExecutionRecord, ExecutionError>,
        events: Vec<ExecutionEvent>,
    ) -> StoredExecution {
        match result {
            Ok(record) => {
                let execution = &record.execution;
                StoredExecution {
                    id: execution.id.clone(),
                    workspace_id: self.workspace_id.clone(),
                    agent_id: self.agent_id.clone(),
                    status: execution.status,
                    task: self.content.clone(),
                    started_at: execution.started_at,
                    completed_at: execution.completed_at,
                    runtime_id: execution.runtime_id.clone(),
                    model_id: execution.model_id.clone(),
                    failure: execution.failure.clone(),
                    metadata: execution.metadata.clone(),
                    usage: execution.usage.clone(),
                    interaction: execution.interaction.clone(),
                    context: execution.context.clone(),
                    workflow: self.workflow.clone(),
                    events,
                }
            }
            Err(error) => {
                let now = now_ms();
                StoredExecution {
                    id: self.execution_id.clone(),
                    workspace_id: self.workspace_id.clone(),
                    agent_id: self.agent_id.clone(),
                    status: ExecutionStatus::Failed,
                    task: self.content.clone(),
                    started_at: now,
                    completed_at: Some(now),
                    runtime_id: String::new(),
                    model_id: String::new(),
                    failure: Some(ExecutionFailure {
                        kind: FailureKind::InvalidRequest,
                        message: error.to_string(),
                        details: None,
                    }),
                    metadata: BTreeMap::new(),
                    usage: None,
                    interaction: None,
                    context: None,
                    workflow: self.workflow.clone(),
                    events,
                }
            }
        }
    }

    /// Remembers what the execution consumed, exactly as the runtime reported it.
    fn record_usage(&self, record: &ExecutionRecord) {
        let execution = &record.execution;
        let usage = UsageRecord {
            execution_id: execution.id.clone(),
            workspace_id: self.workspace_id.clone(),
            agent_id: self.agent_id.clone(),
            runtime_id: execution.runtime_id.clone(),
            model_id: execution.model_id.clone(),
            started_at: execution.started_at,
            completed_at: execution.completed_at.unwrap_or(execution.started_at),
            // Waiting for a person is not a failure: the runtime did what it was asked.
            succeeded: matches!(
                execution.status,
                ExecutionStatus::Completed | ExecutionStatus::WaitingForInput
            ),
            metrics: execution.usage.clone(),
        };
        let quota = execution
            .quota
            .clone()
            .map(|quota| (execution.runtime_id.clone(), quota));
        if let Err(error) = self.inner.ledger.record(usage, quota) {
            // Losing a usage entry must not fail the conversation.
            eprintln!("could not record usage: {error}");
        }
    }
}

/// Passes progress on to the real observer while keeping the timeline, so it can be stored
/// when the execution ends. Streamed answer text is not kept: it becomes the assistant message.
struct Timeline<'a> {
    next: &'a dyn ChatObserver,
    events: Mutex<Vec<ExecutionEvent>>,
}

impl<'a> Timeline<'a> {
    fn new(next: &'a dyn ChatObserver) -> Self {
        Self {
            next,
            events: Mutex::new(Vec::new()),
        }
    }

    fn into_events(self) -> Vec<ExecutionEvent> {
        self.events.into_inner().unwrap_or_default()
    }
}

impl ExecutionObserver for Timeline<'_> {
    fn on_event(&self, event: &ExecutionEvent) {
        if event.kind != ExecutionEventKind::OutputChunk {
            if let Ok(mut events) = self.events.lock() {
                events.push(event.clone());
            }
        }
        self.next.on_event(event);
    }
}

/// The assistant's reply when it could not answer: short, never a raw dump. The UI words the
/// failure in the user's language from `failure_kind`; this text is only a fallback.
fn failure_text(reason: &str) -> String {
    if reason.is_empty() {
        "Failed to execute request.".to_owned()
    } else {
        format!("Failed to execute request. {reason}")
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Barrier;

    use super::*;
    use crate::application::agents::CreateAgentRequest;
    use crate::application::config::memory::MemoryStore;
    use crate::application::config::ConfigRepository;
    use crate::application::personalities::PersonalityService;
    use crate::application::projects::fake::FakeInspector;
    use crate::application::runtimes::fake::FakeRuntime;
    use crate::application::runtimes::{RuntimeError, RuntimeRegistry};
    use crate::application::workspace::WorkspaceInput;
    use crate::domain::usage::{UsageMetrics, UsageSource};

    #[derive(Default)]
    struct Collector {
        events: Mutex<Vec<ExecutionEvent>>,
        messages: Mutex<Vec<Message>>,
    }

    impl ExecutionObserver for Collector {
        fn on_event(&self, event: &ExecutionEvent) {
            self.events.lock().unwrap().push(event.clone());
        }
    }

    impl ChatObserver for Collector {
        fn on_message(&self, message: &Message) {
            self.messages.lock().unwrap().push(message.clone());
        }
    }

    struct Fixture {
        chat: ChatService,
        ledger: Arc<UsageLedger>,
        store: Arc<MemoryStore>,
        workspaces: Arc<WorkspaceService>,
        agent_ids: Vec<String>,
        workspace_id: String,
    }

    /// One agent per fake runtime, all in one workspace on `/atlas`.
    fn fixture(runtimes: Vec<FakeRuntime>) -> Fixture {
        let store = Arc::new(MemoryStore::default());
        let config = Arc::new(ConfigRepository::load(Box::new(store.clone())));
        let personalities = Arc::new(PersonalityService::new(config.clone()));
        let ids: Vec<String> = runtimes.iter().map(|r| r.id.clone()).collect();
        let registry = Arc::new(RuntimeRegistry::new(
            runtimes
                .into_iter()
                .map(|r| Arc::new(r) as Arc<_>)
                .collect(),
        ));
        let agents = Arc::new(AgentService::new(
            config.clone(),
            personalities.clone(),
            registry.clone(),
        ));
        let agent_ids = ids
            .iter()
            .map(|runtime_id| {
                agents
                    .create(CreateAgentRequest {
                        name: format!("{runtime_id} agent"),
                        personality_id: "architect".to_owned(),
                        runtime_id: runtime_id.clone(),
                        model_id: "m1".to_owned(),
                        instructions: String::new(),
                        worktree_isolation: Some(false),
                        result_contract: None,
                    })
                    .unwrap()
                    .id
            })
            .collect();
        let workspaces = Arc::new(WorkspaceService::new(
            config.clone(),
            agents.clone(),
            Arc::new(FakeInspector::with(&[("/atlas", &[]), ("/other", &[])])),
        ));
        let workspace_id = workspaces
            .create(&WorkspaceInput {
                name: "Atlas".to_owned(),
                project_path: "/atlas".to_owned(),
                description: None,
            })
            .unwrap()
            .id;
        let executions = Arc::new(ExecutionService::new(
            agents.clone(),
            personalities,
            registry,
            workspaces.clone(),
            Arc::new(crate::application::security::AuditLog::default()),
        ));
        let ledger = Arc::new(UsageLedger::new(config.clone()));
        Fixture {
            chat: ChatService::with_history(
                agents,
                executions,
                workspaces.clone(),
                ledger.clone(),
                Arc::new(ConversationHistory::new(config.clone())),
            ),
            ledger,
            store,
            workspaces,
            agent_ids,
            workspace_id,
        }
    }

    fn request(f: &Fixture, agent_id: &str, content: &str) -> SendMessageRequest {
        SendMessageRequest {
            workspace_id: f.workspace_id.clone(),
            agent_id: agent_id.to_owned(),
            content: content.to_owned(),
        }
    }

    fn reported(total: u64, cost: f64) -> UsageMetrics {
        UsageMetrics {
            input_tokens: Some(total - 1),
            output_tokens: Some(1),
            total_tokens: Some(total),
            cost: Some(cost),
            currency: Some("USD".to_owned()),
            source: UsageSource::RuntimeReported,
        }
    }

    #[test]
    fn the_conversation_and_the_ended_execution_are_still_there_after_a_restart() {
        let f = fixture(vec![FakeRuntime::new("rt", Ok("The answer."))]);
        let agent = &f.agent_ids[0];
        let (sent, pending) = f.chat.send(request(&f, agent, "Analyze auth")).unwrap();
        pending.run(&Collector::default());

        let restarted =
            ConversationHistory::new(Arc::new(ConfigRepository::load(Box::new(f.store.clone()))));
        let messages = restarted.messages(Some(&f.workspace_id), Some(agent));
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].content, "The answer.");
        let ended = restarted.executions(Some(&f.workspace_id), Some(agent));
        assert_eq!(ended.len(), 1);
        assert_eq!(ended[0].id, sent.execution_id);
        assert_eq!(ended[0].task, "Analyze auth");
        assert_eq!(ended[0].status, ExecutionStatus::Completed);
        assert_eq!(ended[0].runtime_id, "rt");
        let kinds: Vec<_> = ended[0].events.iter().map(|e| e.kind).collect();
        assert_eq!(kinds.first(), Some(&ExecutionEventKind::Started));
        assert_eq!(kinds.last(), Some(&ExecutionEventKind::Completed));
        assert_eq!(restarted.next_execution_number(), 2);
    }

    #[test]
    fn a_failed_run_is_kept_as_failed_with_its_reason() {
        let f = fixture(vec![FakeRuntime::new("rt", Err(RuntimeError::Timeout))]);
        let agent = &f.agent_ids[0];
        let (_, pending) = f.chat.send(request(&f, agent, "Do it")).unwrap();
        pending.run(&Collector::default());

        let ended = f.chat.executions(Some(&f.workspace_id), Some(agent));
        assert_eq!(ended[0].status, ExecutionStatus::Failed);
        assert_eq!(
            ended[0].failure.as_ref().unwrap().kind,
            FailureKind::Timeout
        );
    }

    #[test]
    fn a_user_message_starts_an_execution_whose_answer_is_the_assistant_message() {
        let f = fixture(vec![FakeRuntime::new("rt", Ok("The answer."))]);
        let agent = &f.agent_ids[0];
        let observer = Collector::default();

        let (sent, pending) = f.chat.send(request(&f, agent, "  Analyze auth  ")).unwrap();

        // The user message exists before the agent has done anything.
        assert_eq!(sent.user_message.role, MessageRole::User);
        assert_eq!(sent.user_message.content, "Analyze auth");
        assert_eq!(sent.user_message.workspace_id, f.workspace_id);
        assert_eq!(
            f.chat.messages(None, Some(agent)),
            std::slice::from_ref(&sent.user_message)
        );
        assert_eq!(observer.events.lock().unwrap().len(), 0);

        pending.run(&observer);

        let messages = f.chat.messages(Some(&f.workspace_id), Some(agent));
        assert_eq!(messages.len(), 2);
        let assistant = &messages[1];
        assert_eq!(assistant.role, MessageRole::Assistant);
        assert_eq!(assistant.content, "The answer.");
        assert!(!assistant.failed);
        assert_eq!(assistant.execution_id.as_ref(), Some(&sent.execution_id));
        assert_eq!(
            *observer.messages.lock().unwrap(),
            std::slice::from_ref(assistant)
        );
        let events = observer.events.lock().unwrap();
        assert!(events
            .iter()
            .all(|e| e.execution_id == sent.execution_id && &e.agent_id == agent));
        assert!(events
            .iter()
            .all(|e| e.workspace_id == f.workspace_id && e.timestamp > 0));
        let kinds: Vec<_> = events.iter().map(|e| e.kind).collect();
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
    fn a_failed_execution_gives_a_concise_failed_message_with_its_failure_kind() {
        let f = fixture(vec![FakeRuntime::new(
            "rt",
            Err(RuntimeError::ExecutionFailed("raw stderr dump".to_owned())),
        )]);
        let observer = Collector::default();

        let (sent, pending) = f.chat.send(request(&f, &f.agent_ids[0], "x")).unwrap();
        pending.run(&observer);

        let assistant = f.chat.messages(None, Some(&f.agent_ids[0])).pop().unwrap();
        assert!(assistant.failed);
        assert_eq!(assistant.failure_kind.as_deref(), Some("execution_failed"));
        assert!(!assistant.content.contains("raw stderr"));
        assert_eq!(assistant.execution_id, Some(sent.execution_id));
        let events = observer.events.lock().unwrap();
        assert_eq!(
            events.last().unwrap().metadata["failureKind"],
            "execution_failed"
        );
    }

    #[test]
    fn a_request_the_execution_layer_rejects_still_ends_with_a_failed_event_and_message() {
        let f = fixture(vec![FakeRuntime::new("rt", Ok("x"))]);
        let observer = Collector::default();
        // The workspace disappears between sending and running.
        let (sent, pending) = f.chat.send(request(&f, &f.agent_ids[0], "x")).unwrap();
        f.workspaces.delete(&f.workspace_id).unwrap();

        pending.run(&observer);

        let events = observer.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, ExecutionEventKind::Failed);
        assert_eq!(events[0].execution_id, sent.execution_id);
        let assistant = observer.messages.lock().unwrap()[0].clone();
        assert!(assistant.failed);
        assert_eq!(assistant.failure_kind.as_deref(), Some("invalid_request"));
    }

    #[test]
    fn rejects_empty_unknown_and_busy_requests() {
        let f = fixture(vec![FakeRuntime::new("rt", Ok("x"))]);
        let agent = &f.agent_ids[0];

        assert_eq!(
            f.chat.send(request(&f, agent, "  ")).err(),
            Some(ChatError::EmptyMessage)
        );
        assert_eq!(
            f.chat.send(request(&f, "ghost", "x")).err(),
            Some(ChatError::UnknownAgent("ghost".to_owned()))
        );
        let mut elsewhere = request(&f, agent, "x");
        elsewhere.workspace_id = "nope".to_owned();
        assert_eq!(
            f.chat.send(elsewhere).err(),
            Some(ChatError::UnknownWorkspace("nope".to_owned()))
        );
        let (_sent, pending) = f.chat.send(request(&f, agent, "first")).unwrap();
        assert_eq!(
            f.chat.send(request(&f, agent, "second")).err(),
            Some(ChatError::AgentBusy(agent.clone()))
        );

        // Finishing the run frees the agent again.
        pending.run(&Collector::default());
        assert!(f.chat.send(request(&f, agent, "third")).is_ok());
        assert_eq!(f.chat.messages(None, Some(agent)).len(), 3);
    }

    #[test]
    fn two_agents_run_concurrently_and_stay_independent() {
        let barrier = Arc::new(Barrier::new(2));
        let f = fixture(vec![
            FakeRuntime::new("rt-a", Ok("from A")).with_barrier(barrier.clone()),
            FakeRuntime::new("rt-b", Ok("from B")).with_barrier(barrier),
        ]);
        let (a, b) = (f.agent_ids[0].clone(), f.agent_ids[1].clone());
        let observer = Collector::default();

        let (sent_a, run_a) = f.chat.send(request(&f, &a, "question A")).unwrap();
        let (sent_b, run_b) = f.chat.send(request(&f, &b, "question B")).unwrap();
        assert_ne!(sent_a.execution_id, sent_b.execution_id);
        std::thread::scope(|scope| {
            scope.spawn(|| run_a.run(&observer));
            scope.spawn(|| run_b.run(&observer));
        });

        let events = observer.events.lock().unwrap();
        for (agent, sent) in [(&a, &sent_a), (&b, &sent_b)] {
            let mine: Vec<_> = events.iter().filter(|e| &e.agent_id == agent).collect();
            assert_eq!(mine.len(), 5);
            assert!(mine.iter().all(|e| e.execution_id == sent.execution_id));
        }
        let texts = |agent: &String| -> Vec<String> {
            f.chat
                .messages(None, Some(agent))
                .into_iter()
                .map(|m| m.content)
                .collect()
        };
        assert_eq!(texts(&a), ["question A", "from A"]);
        assert_eq!(texts(&b), ["question B", "from B"]);
    }

    #[test]
    fn the_same_agent_has_separate_conversations_and_can_run_in_two_workspaces_at_once() {
        let f = fixture(vec![FakeRuntime::new("rt", Ok("answer"))]);
        let agent = &f.agent_ids[0];
        let second = f
            .workspaces
            .create(&WorkspaceInput {
                name: "Other".to_owned(),
                project_path: "/other".to_owned(),
                description: None,
            })
            .unwrap()
            .id;
        let (_, run_one) = f.chat.send(request(&f, agent, "in the first")).unwrap();
        // Busy in the first workspace does not block the second.
        let (_, run_two) = f
            .chat
            .send(SendMessageRequest {
                workspace_id: second.clone(),
                agent_id: agent.clone(),
                content: "in the second".to_owned(),
            })
            .unwrap();
        let observer = Collector::default();
        run_one.run(&observer);
        run_two.run(&observer);

        let in_first: Vec<_> = f
            .chat
            .messages(Some(&f.workspace_id), Some(agent))
            .into_iter()
            .map(|m| m.content)
            .collect();
        let in_second: Vec<_> = f
            .chat
            .messages(Some(&second), Some(agent))
            .into_iter()
            .map(|m| m.content)
            .collect();
        assert_eq!(in_first, ["in the first", "answer"]);
        assert_eq!(in_second, ["in the second", "answer"]);
        let events = observer.events.lock().unwrap();
        assert!(events.iter().any(|e| e.workspace_id == second));
        f.chat.discard_workspace(&second);
        assert_eq!(f.chat.messages(Some(&second), None), []);
        assert_eq!(f.chat.messages(Some(&f.workspace_id), None).len(), 2);
    }

    #[test]
    fn what_the_runtime_reports_is_recorded_and_nothing_is_invented_when_it_reports_nothing() {
        let f = fixture(vec![
            FakeRuntime::new("with", Ok("a")).with_usage(reported(100, 0.25)),
            FakeRuntime::new("without", Ok("b")),
        ]);
        let observer = Collector::default();

        for agent in &f.agent_ids {
            let (_, run) = f.chat.send(request(&f, agent, "go")).unwrap();
            run.run(&observer);
        }

        let records = f.ledger.records();
        assert_eq!(records.len(), 2);
        let with = records
            .iter()
            .find(|r| r.agent_id == f.agent_ids[0])
            .unwrap();
        assert_eq!(with.workspace_id, f.workspace_id);
        assert_eq!(with.metrics, Some(reported(100, 0.25)));
        let without = records
            .iter()
            .find(|r| r.agent_id == f.agent_ids[1])
            .unwrap();
        assert_eq!(without.metrics, None);
        assert!(without.succeeded);
    }

    #[test]
    fn conversation_executions_and_agent_discarding() {
        let f = fixture(vec![FakeRuntime::new("rt", Ok("a"))]);
        let agent = &f.agent_ids[0];
        let (sent, run) = f.chat.send(request(&f, agent, "go")).unwrap();
        run.run(&Collector::default());

        assert_eq!(
            f.chat.conversation_executions(&f.workspace_id, agent),
            [sent.execution_id].into()
        );
        f.chat.discard_agent(agent);
        assert_eq!(f.chat.messages(None, None), []);
    }

    #[test]
    fn live_output_reaches_the_listener_tagged_with_agent_and_execution_but_is_not_logged() {
        let f =
            fixture(vec![FakeRuntime::new("rt", Ok("Final answer."))
                .with_chunks(&["Reading ", "the files."])]);
        let agent = &f.agent_ids[0];
        let observer = Collector::default();

        let (sent, pending) = f.chat.send(request(&f, agent, "go")).unwrap();
        pending.run(&observer);

        let events = observer.events.lock().unwrap();
        let chunks: Vec<_> = events
            .iter()
            .filter(|e| e.kind == ExecutionEventKind::OutputChunk)
            .collect();
        assert_eq!(
            chunks
                .iter()
                .map(|e| e.message.as_str())
                .collect::<Vec<_>>(),
            ["Reading ", "the files."]
        );
        assert!(chunks
            .iter()
            .all(|e| &e.agent_id == agent && e.execution_id == sent.execution_id));
        let kinds: Vec<_> = events.iter().map(|e| e.kind).collect();
        assert_eq!(kinds[4], ExecutionEventKind::OutputChunk);
        assert_eq!(kinds.last(), Some(&ExecutionEventKind::Completed));
        assert_eq!(
            observer.messages.lock().unwrap()[0].content,
            "Final answer."
        );
    }

    /// Both real runtimes through a persisted workspace on a real folder, with the usage each one
    /// really reports. Run with `cargo test real_workspace -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs OpenCode and the Claude CLI installed; makes real model calls"]
    #[allow(clippy::too_many_lines)]
    fn real_workspace_usage_from_both_runtimes() {
        use crate::application::process::ProcessRunner;
        use crate::application::runtimes::{inspect, RUNTIME_PROGRAMS};
        use crate::application::security::{
            ApprovalBroker, AuditLog, GuardedProcessRunner, NoSandbox, SecurityService,
        };
        use crate::application::usage_reports::UsageReporter;
        use crate::domain::usage::UsageWindows;
        use crate::infrastructure::{FsProjectInspector, JsonConfigStore, SystemProcessRunner};

        let dir = std::env::temp_dir().join(format!("atlas-real-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let project = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .display()
            .to_string();

        let load = || {
            Arc::new(ConfigRepository::load(Box::new(JsonConfigStore::new(
                dir.join("config.json"),
            ))))
        };
        // Through the real guard, like the app: it reads the same config the services write.
        let first_config = load();
        let runner: Arc<dyn ProcessRunner> = Arc::new(GuardedProcessRunner::new(
            Arc::new(SystemProcessRunner::new()),
            Arc::new(SecurityService::new(first_config.clone())),
            Arc::new(ApprovalBroker::new()),
            Arc::new(AuditLog::default()),
            Arc::new(NoSandbox),
            RUNTIME_PROGRAMS.map(str::to_owned).to_vec(),
        ));
        let registry = Arc::new(RuntimeRegistry::with_default_runtimes(&runner));
        let opencode_model = inspect(registry.find("opencode").unwrap().as_ref()).available_models
            [0]
        .id
        .clone();
        let build = |config: Arc<ConfigRepository>, registry: &Arc<RuntimeRegistry>| {
            let personalities = Arc::new(PersonalityService::new(config.clone()));
            let agents = Arc::new(AgentService::new(
                config.clone(),
                personalities.clone(),
                registry.clone(),
            ));
            let workspaces = Arc::new(WorkspaceService::new(
                config.clone(),
                agents.clone(),
                Arc::new(FsProjectInspector),
            ));
            let executions = Arc::new(ExecutionService::new(
                agents.clone(),
                personalities,
                registry.clone(),
                workspaces.clone(),
                Arc::new(crate::application::security::AuditLog::default()),
            ));
            let ledger = Arc::new(UsageLedger::new(config.clone()));
            let chat = ChatService::new(
                agents.clone(),
                executions,
                workspaces.clone(),
                ledger.clone(),
            );
            (agents, workspaces, ledger, chat)
        };
        let (agents, workspaces, ledger, chat) = build(first_config, &registry);
        let workspace = workspaces
            .create(&WorkspaceInput {
                name: "Atlas".to_owned(),
                project_path: project.clone(),
                description: None,
            })
            .unwrap();
        println!(
            "project context: {:?}",
            workspaces.project_context(&workspace.id).unwrap()
        );
        let create = |name: &str, runtime: &str, model: &str| {
            agents
                .create(CreateAgentRequest {
                    name: name.to_owned(),
                    personality_id: "architect".to_owned(),
                    runtime_id: runtime.to_owned(),
                    model_id: model.to_owned(),
                    instructions: String::new(),
                    worktree_isolation: Some(false),
                    result_contract: None,
                })
                .unwrap()
                .id
        };
        let open_code_agent = create("OpenCode agent", "opencode", &opencode_model);
        let claude_agent = create("Claude agent", "claude", "sonnet");
        workspaces
            .add_agent(&workspace.id, &open_code_agent)
            .unwrap();
        workspaces.add_agent(&workspace.id, &claude_agent).unwrap();

        let observer = Collector::default();
        for agent in [&open_code_agent, &claude_agent] {
            let (_, run) = chat
                .send(SendMessageRequest {
                    workspace_id: workspace.id.clone(),
                    agent_id: agent.clone(),
                    content: "Reply with exactly the word: pong".to_owned(),
                })
                .unwrap();
            run.run(&observer);
        }

        let windows = UsageWindows {
            today_start: 0,
            week_start: 0,
            month_start: 0,
        };
        let reporter = UsageReporter::new(ledger.clone(), chat.clone(), agents.clone());
        for (name, id) in [("OpenCode", &open_code_agent), ("Claude", &claude_agent)] {
            let summary = reporter.agent_summary(&workspace.id, id, windows).unwrap();
            println!(
                "--- {name}\nlatest: {:#?}\nconversation: {:?}\nquota: {:?}",
                summary.latest_execution, summary.conversation, summary.quota
            );
            let latest = summary.latest_execution.unwrap();
            assert!(
                latest.succeeded,
                "{name} failed: {:?}",
                chat.messages(None, Some(id))
            );
            assert!(latest.metrics.is_some(), "{name} reported no usage at all");
            assert_eq!(summary.conversation.runs, 1);
        }
        println!(
            "workspace: {:?}",
            reporter.workspace_summary(&workspace.id, windows)
        );

        // A restart: a new service on the same config.json still has the workspace, the agents
        // and the recorded usage (conversations are in memory and start empty).
        let (agents2, workspaces2, ledger2, chat2) = build(load(), &registry);
        assert_eq!(workspaces2.list().len(), 1);
        assert_eq!(
            workspaces2
                .get(&workspace.id)
                .unwrap()
                .layout
                .agent_placements
                .len(),
            2
        );
        assert_eq!(agents2.list().len(), 2);
        assert_eq!(ledger2.records().len(), 2);
        assert_eq!(chat2.messages(None, None), []);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
