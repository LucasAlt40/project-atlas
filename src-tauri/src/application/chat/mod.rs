use std::collections::HashSet;
use std::fmt;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use super::agents::AgentService;
use super::errors::{AppError, ErrorCode};
use super::executions::{ExecutionObserver, ExecutionService, RunAgentRequest};
use super::support::{new_id, now_ms};
use super::usage::UsageLedger;
use super::workspace::WorkspaceService;
use crate::domain::conversation::{Message, MessageRole};
use crate::domain::execution::{
    ExecutionEvent, ExecutionEventKind, ExecutionRecord, ExecutionStatus, FailureKind,
};
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
    messages: Mutex<Vec<Message>>,
    busy: Mutex<HashSet<(String, String)>>,
}

/// Use case: chat with an agent in a workspace. Sits above [`ExecutionService`]: a user message
/// starts an execution, and its outcome becomes the assistant message. A conversation belongs to
/// one agent in one workspace, so the same agent has separate conversations in different
/// workspaces. Conversations are in memory.
#[derive(Clone)]
pub struct ChatService {
    inner: Arc<Inner>,
}

impl ChatService {
    pub fn new(
        agents: Arc<AgentService>,
        executions: Arc<ExecutionService>,
        workspaces: Arc<WorkspaceService>,
        ledger: Arc<UsageLedger>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                agents,
                executions,
                workspaces,
                ledger,
                messages: Mutex::new(Vec::new()),
                busy: Mutex::new(HashSet::new()),
            }),
        }
    }

    /// Messages, oldest first, optionally narrowed to a workspace and/or an agent.
    pub fn messages(&self, workspace_id: Option<&str>, agent_id: Option<&str>) -> Vec<Message> {
        self.inner
            .messages
            .lock()
            .expect("messages lock poisoned")
            .iter()
            .filter(|m| workspace_id.is_none_or(|id| m.workspace_id == id))
            .filter(|m| agent_id.is_none_or(|id| m.agent_id == id))
            .cloned()
            .collect()
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

    /// Deletes every conversation of the workspace.
    pub fn discard_workspace(&self, workspace_id: &str) {
        self.inner
            .messages
            .lock()
            .expect("messages lock poisoned")
            .retain(|m| m.workspace_id != workspace_id);
    }

    /// Deletes the agent's conversations in every workspace.
    pub fn discard_agent(&self, agent_id: &str) {
        self.inner
            .messages
            .lock()
            .expect("messages lock poisoned")
            .retain(|m| m.agent_id != agent_id);
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
                execution_id,
                _busy: busy,
            },
        ))
    }
}

/// Holds an agent still in a workspace while something about it is being removed; see
/// [`ChatService::lock_agent`]. Released when dropped.
pub struct AgentLock {
    _busy: BusyGuard,
}

impl Inner {
    fn push(&self, message: Message) {
        self.messages
            .lock()
            .expect("messages lock poisoned")
            .push(message);
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
    content: String,
    execution_id: String,
    _busy: BusyGuard,
}

impl PendingRun {
    /// Runs the execution through [`ExecutionService`] (blocking), records what it consumed,
    /// then records and announces the assistant message. Progress events go to `observer` as they
    /// happen.
    pub fn run(self, observer: &dyn ChatObserver) {
        let result = self.inner.executions.run_with_id(
            self.execution_id.clone(),
            RunAgentRequest {
                task_id: new_id("task"),
                workspace_id: self.workspace_id.clone(),
                agent_id: self.agent_id.clone(),
                description: self.content.clone(),
            },
            observer,
        );

        let (content, failure_kind) = match &result {
            Ok(record) => {
                self.record_usage(record);
                match record.execution.status {
                    ExecutionStatus::Completed => {
                        (record.execution.result.clone().unwrap_or_default(), None)
                    }
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
            succeeded: execution.status == ExecutionStatus::Completed,
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
        workspaces: Arc<WorkspaceService>,
        agent_ids: Vec<String>,
        workspace_id: String,
    }

    /// One agent per fake runtime, all in one workspace on `/atlas`.
    fn fixture(runtimes: Vec<FakeRuntime>) -> Fixture {
        let config = Arc::new(ConfigRepository::load(Box::<MemoryStore>::default()));
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
        ));
        let ledger = Arc::new(UsageLedger::new(config));
        Fixture {
            chat: ChatService::new(agents, executions, workspaces.clone(), ledger.clone()),
            ledger,
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
        use crate::application::runtimes::inspect;
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

        let runner: Arc<dyn ProcessRunner> = Arc::new(SystemProcessRunner::new());
        let registry = Arc::new(RuntimeRegistry::with_default_runtimes(&runner));
        let opencode_model = inspect(registry.find("opencode").unwrap().as_ref()).available_models
            [0]
        .id
        .clone();
        let build = |registry: &Arc<RuntimeRegistry>| {
            let config = Arc::new(ConfigRepository::load(Box::new(JsonConfigStore::new(
                dir.join("config.json"),
            ))));
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
            ));
            let ledger = Arc::new(UsageLedger::new(config));
            let chat = ChatService::new(
                agents.clone(),
                executions,
                workspaces.clone(),
                ledger.clone(),
            );
            (agents, workspaces, ledger, chat)
        };
        let (agents, workspaces, ledger, chat) = build(&registry);
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
        let (agents2, workspaces2, ledger2, chat2) = build(&registry);
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
