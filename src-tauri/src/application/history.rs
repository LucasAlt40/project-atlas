use std::collections::HashSet;
use std::sync::Arc;

use super::config::ConfigRepository;
use super::errors::AppError;
use super::support::{new_id, now_ms};
use crate::domain::conversation::{Message, MessageRole};
use crate::domain::execution::{
    ExecutionEvent, ExecutionEventKind, ExecutionFailure, ExecutionStatus, FailureKind,
    StoredExecution,
};

/// Most messages kept per conversation (an agent in a workspace); the oldest go first.
const MAX_MESSAGES: usize = 200;
/// Most executions kept per conversation.
const MAX_EXECUTIONS: usize = 100;
/// Most timeline events kept per execution.
const MAX_EVENTS: usize = 300;

/// Stores nothing: the history of a chat that is not meant to survive (tests).
#[cfg(test)]
struct DiscardStore;

#[cfg(test)]
impl super::config::ConfigStore for DiscardStore {
    fn load(&self) -> Result<super::config::UserConfig, String> {
        Ok(super::config::UserConfig::default())
    }

    fn save(&self, _config: &super::config::UserConfig) -> Result<(), String> {
        Ok(())
    }
}

/// Use case: remember what was said and what was run, per agent and workspace, across
/// restarts. It lives in the user config (see ADR 0008): the volume is small and bounded, so no
/// database is needed. Raw terminal output is deliberately not part of it.
pub struct ConversationHistory {
    config: Arc<ConfigRepository>,
}

impl ConversationHistory {
    pub fn new(config: Arc<ConfigRepository>) -> Self {
        Self { config }
    }

    /// A history that forgets everything when dropped.
    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self::new(Arc::new(ConfigRepository::load(Box::new(DiscardStore))))
    }

    /// Messages, oldest first, optionally narrowed to a workspace and/or an agent.
    pub fn messages(&self, workspace_id: Option<&str>, agent_id: Option<&str>) -> Vec<Message> {
        self.config.read(|config| {
            config
                .messages
                .iter()
                .filter(|m| workspace_id.is_none_or(|id| m.workspace_id == id))
                .filter(|m| agent_id.is_none_or(|id| m.agent_id == id))
                .cloned()
                .collect()
        })
    }

    /// Ended executions, newest first, optionally narrowed to a workspace and/or an agent.
    pub fn executions(
        &self,
        workspace_id: Option<&str>,
        agent_id: Option<&str>,
    ) -> Vec<StoredExecution> {
        self.config.read(|config| {
            config
                .executions
                .iter()
                .rev()
                .filter(|e| workspace_id.is_none_or(|id| e.workspace_id == id))
                .filter(|e| agent_id.is_none_or(|id| e.agent_id == id))
                .cloned()
                .collect()
        })
    }

    /// Adds a message. Losing one to a full disk must not break the conversation on screen,
    /// so a failed save is reported and otherwise ignored.
    pub fn push_message(&self, message: Message) {
        let result = self.config.modify(|config| {
            let key = (message.workspace_id.clone(), message.agent_id.clone());
            config.messages.push(message);
            trim_oldest(&mut config.messages, MAX_MESSAGES, |m| {
                (m.workspace_id.clone(), m.agent_id.clone()) == key
            });
            Ok(())
        });
        report("message", result);
    }

    /// Adds an ended execution (bounded timeline, no streamed text).
    pub fn record_execution(&self, mut execution: StoredExecution) {
        execution
            .events
            .retain(|e| e.kind != ExecutionEventKind::OutputChunk);
        if execution.events.len() > MAX_EVENTS {
            let excess = execution.events.len() - MAX_EVENTS;
            execution.events.drain(..excess);
        }
        let result = self.config.modify(|config| {
            let key = (execution.workspace_id.clone(), execution.agent_id.clone());
            config.executions.push(execution);
            trim_oldest(&mut config.executions, MAX_EXECUTIONS, |e| {
                (e.workspace_id.clone(), e.agent_id.clone()) == key
            });
            Ok(())
        });
        report("execution", result);
    }

    /// Deletes every conversation and execution of the workspace.
    pub fn discard_workspace(&self, workspace_id: &str) {
        let result = self.config.modify(|config| {
            config.messages.retain(|m| m.workspace_id != workspace_id);
            config.executions.retain(|e| e.workspace_id != workspace_id);
            Ok(())
        });
        report("workspace history", result);
    }

    /// Deletes the agent's conversations and executions in every workspace.
    pub fn discard_agent(&self, agent_id: &str) {
        let result = self.config.modify(|config| {
            config.messages.retain(|m| m.agent_id != agent_id);
            config.executions.retain(|e| e.agent_id != agent_id);
            Ok(())
        });
        report("agent history", result);
    }

    /// The number after the highest `exec-N` ever stored, so ids of a new run never repeat
    /// those of an earlier run of the app.
    pub fn next_execution_number(&self) -> u64 {
        self.config.read(|config| {
            let ids = config
                .messages
                .iter()
                .filter_map(|m| m.execution_id.as_deref())
                .chain(config.executions.iter().map(|e| e.id.as_str()));
            ids.filter_map(|id| id.strip_prefix("exec-")?.parse::<u64>().ok())
                .max()
                .map_or(1, |highest| highest + 1)
        })
    }

    /// A message the user sent whose execution never ended means Atlas was closed while it ran:
    /// answer it with a failed execution, so the conversation is not left waiting for ever.
    /// Call once at startup, before anything runs.
    pub fn recover_interrupted(&self) {
        let answered: HashSet<String> = self.config.read(|config| {
            config
                .messages
                .iter()
                .filter(|m| m.role == MessageRole::Assistant)
                .filter_map(|m| m.execution_id.clone())
                .collect()
        });
        let orphans: Vec<Message> = self
            .messages(None, None)
            .into_iter()
            .filter(|m| m.role == MessageRole::User)
            .filter(|m| {
                m.execution_id
                    .as_ref()
                    .is_some_and(|id| !answered.contains(id))
            })
            .collect();
        let now = now_ms();
        for user in orphans {
            let Some(execution_id) = user.execution_id.clone() else {
                continue;
            };
            let text = "Atlas was closed while this execution was running.";
            self.push_message(Message {
                id: new_id("msg"),
                workspace_id: user.workspace_id.clone(),
                agent_id: user.agent_id.clone(),
                execution_id: Some(execution_id.clone()),
                role: MessageRole::Assistant,
                content: text.to_owned(),
                timestamp: now,
                failed: true,
                failure_kind: Some(FailureKind::AppClosed.as_str().to_owned()),
            });
            let stored = self
                .config
                .read(|config| config.executions.iter().any(|e| e.id == execution_id));
            if !stored {
                self.record_execution(StoredExecution {
                    id: execution_id.clone(),
                    workspace_id: user.workspace_id,
                    agent_id: user.agent_id,
                    status: ExecutionStatus::Failed,
                    task: user.content,
                    started_at: user.timestamp,
                    completed_at: Some(now),
                    runtime_id: String::new(),
                    model_id: String::new(),
                    failure: Some(ExecutionFailure {
                        kind: FailureKind::AppClosed,
                        message: text.to_owned(),
                        details: None,
                    }),
                    metadata: std::collections::BTreeMap::new(),
                    usage: None,
                    interaction: None,
                    context: None,
                    optimization: None,
                    workflow: None,
                    events: vec![ExecutionEvent {
                        execution_id,
                        workspace_id: String::new(),
                        task_id: String::new(),
                        agent_id: String::new(),
                        kind: ExecutionEventKind::Failed,
                        message: text.to_owned(),
                        timestamp: now,
                        metadata: [(
                            "failureKind".to_owned(),
                            FailureKind::AppClosed.as_str().to_owned(),
                        )]
                        .into(),
                    }],
                });
            }
        }
    }
}

/// Drops the oldest items that match `same_group` once the group exceeds `max`.
fn trim_oldest<T>(items: &mut Vec<T>, max: usize, same_group: impl Fn(&T) -> bool) {
    let count = items.iter().filter(|item| same_group(item)).count();
    let mut excess = count.saturating_sub(max);
    items.retain(|item| {
        if excess > 0 && same_group(item) {
            excess -= 1;
            false
        } else {
            true
        }
    });
}

fn report(what: &str, result: Result<(), AppError>) {
    if let Err(error) = result {
        eprintln!("could not save {what}: {error:?}");
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::application::config::memory::MemoryStore;

    fn message(workspace: &str, agent: &str, execution: &str, role: MessageRole) -> Message {
        Message {
            id: new_id("msg"),
            workspace_id: workspace.to_owned(),
            agent_id: agent.to_owned(),
            execution_id: Some(execution.to_owned()),
            role,
            content: "hi".to_owned(),
            timestamp: now_ms(),
            failed: false,
            failure_kind: None,
        }
    }

    fn execution(workspace: &str, agent: &str, id: &str) -> StoredExecution {
        StoredExecution {
            id: id.to_owned(),
            workspace_id: workspace.to_owned(),
            agent_id: agent.to_owned(),
            status: ExecutionStatus::Completed,
            task: "task".to_owned(),
            started_at: 1,
            completed_at: Some(2),
            runtime_id: "rt".to_owned(),
            model_id: "m".to_owned(),
            failure: None,
            metadata: std::collections::BTreeMap::new(),
            usage: None,
            interaction: None,
            context: None,
            optimization: None,
            workflow: None,
            events: Vec::new(),
        }
    }

    /// Two histories over one store: the second is the app after a restart.
    fn restart(store: &Arc<MemoryStore>) -> ConversationHistory {
        ConversationHistory::new(Arc::new(ConfigRepository::load(Box::new(store.clone()))))
    }

    #[test]
    fn conversations_and_executions_survive_a_restart() {
        let store = Arc::new(MemoryStore::default());
        let history = restart(&store);
        history.push_message(message("w1", "a1", "exec-1", MessageRole::User));
        history.push_message(message("w1", "a1", "exec-1", MessageRole::Assistant));
        history.record_execution(execution("w1", "a1", "exec-1"));

        let reopened = restart(&store);
        assert_eq!(reopened.messages(Some("w1"), Some("a1")).len(), 2);
        assert_eq!(reopened.executions(Some("w1"), Some("a1"))[0].id, "exec-1");
    }

    #[test]
    fn workspaces_and_agents_never_see_each_others_history() {
        let history = ConversationHistory::in_memory();
        history.push_message(message("w1", "a1", "exec-1", MessageRole::User));
        history.push_message(message("w2", "a1", "exec-2", MessageRole::User));
        history.push_message(message("w1", "a2", "exec-3", MessageRole::User));
        history.record_execution(execution("w1", "a1", "exec-1"));
        history.record_execution(execution("w2", "a1", "exec-2"));

        assert_eq!(history.messages(Some("w1"), Some("a1")).len(), 1);
        assert_eq!(history.messages(Some("w2"), None).len(), 1);
        assert_eq!(history.executions(Some("w1"), Some("a1")).len(), 1);
        assert_eq!(history.executions(Some("w2"), Some("a2")).len(), 0);

        history.discard_workspace("w1");
        assert_eq!(history.messages(None, None).len(), 1);
        assert_eq!(history.executions(None, None)[0].id, "exec-2");
    }

    #[test]
    fn a_conversation_is_bounded_without_touching_the_others() {
        let history = ConversationHistory::in_memory();
        history.push_message(message("w2", "a1", "exec-0", MessageRole::User));
        for n in 0..(MAX_MESSAGES + 5) {
            history.push_message(message("w1", "a1", &format!("exec-{n}"), MessageRole::User));
        }
        assert_eq!(history.messages(Some("w1"), Some("a1")).len(), MAX_MESSAGES);
        assert_eq!(history.messages(Some("w2"), Some("a1")).len(), 1);
    }

    #[test]
    fn executions_list_newest_first_and_keep_the_timeline_tail() {
        let history = ConversationHistory::in_memory();
        let mut long = execution("w1", "a1", "exec-1");
        long.events = (0..MAX_EVENTS + 20)
            .map(|n| ExecutionEvent {
                execution_id: "exec-1".to_owned(),
                workspace_id: "w1".to_owned(),
                task_id: "t".to_owned(),
                agent_id: "a1".to_owned(),
                kind: if n % 2 == 0 {
                    ExecutionEventKind::OutputChunk
                } else {
                    ExecutionEventKind::ToolStarted
                },
                message: n.to_string(),
                timestamp: n as u64,
                metadata: std::collections::BTreeMap::new(),
            })
            .collect();
        history.record_execution(long);
        history.record_execution(execution("w1", "a1", "exec-2"));

        let listed = history.executions(Some("w1"), None);
        assert_eq!(listed[0].id, "exec-2");
        // Streamed text is not kept, and what is kept is in order.
        assert!(listed[1]
            .events
            .iter()
            .all(|e| e.kind == ExecutionEventKind::ToolStarted));
        assert_eq!(listed[1].events.len(), MAX_EVENTS / 2 + 10);
    }

    #[test]
    fn ids_continue_after_the_highest_stored_one() {
        let store = Arc::new(MemoryStore::default());
        assert_eq!(restart(&store).next_execution_number(), 1);
        let history = restart(&store);
        history.push_message(message("w1", "a1", "exec-7", MessageRole::User));
        history.record_execution(execution("w1", "a1", "exec-12"));
        assert_eq!(restart(&store).next_execution_number(), 13);
    }

    #[test]
    fn a_run_cut_short_by_closing_atlas_is_answered_once_on_the_next_start() {
        let store = Arc::new(MemoryStore::default());
        let history = restart(&store);
        history.push_message(message("w1", "a1", "exec-1", MessageRole::User));
        history.push_message(message("w1", "a1", "exec-2", MessageRole::User));
        history.push_message(message("w1", "a1", "exec-2", MessageRole::Assistant));

        let reopened = restart(&store);
        reopened.recover_interrupted();
        reopened.recover_interrupted();

        let messages = reopened.messages(Some("w1"), Some("a1"));
        let answers: Vec<_> = messages
            .iter()
            .filter(|m| {
                m.role == MessageRole::Assistant && m.execution_id.as_deref() == Some("exec-1")
            })
            .collect();
        assert_eq!(answers.len(), 1);
        assert_eq!(answers[0].failure_kind.as_deref(), Some("app_closed"));
        let stored = reopened.executions(Some("w1"), Some("a1"));
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].status, ExecutionStatus::Failed);
    }
}
