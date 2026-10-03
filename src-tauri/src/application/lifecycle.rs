use std::sync::Arc;

use super::agents::AgentService;
use super::chat::{AgentLock, ChatService};
use super::errors::{AppError, ErrorCode};
use super::settings::SettingsService;
use super::usage::UsageLedger;
use super::workspace::WorkspaceService;
use crate::domain::workspace::Workspace;

/// Use case: remove an agent or a workspace for good, with everything that hangs off it. Agents,
/// workspaces, conversations and usage are separate concerns, so this is the one place that knows
/// they must be cleaned up together.
pub struct AgentLifecycle {
    agents: Arc<AgentService>,
    chat: ChatService,
    workspaces: Arc<WorkspaceService>,
    ledger: Arc<UsageLedger>,
    settings: Arc<SettingsService>,
}

impl AgentLifecycle {
    pub fn new(
        agents: Arc<AgentService>,
        chat: ChatService,
        workspaces: Arc<WorkspaceService>,
        ledger: Arc<UsageLedger>,
        settings: Arc<SettingsService>,
    ) -> Self {
        Self {
            agents,
            chat,
            workspaces,
            ledger,
            settings,
        }
    }

    /// Deletes the agent, its place in every workspace and its conversations, and returns the
    /// updated workspaces. What the agent already consumed stays in the usage ledger (it was
    /// really spent). An agent that is working on a message anywhere cannot be deleted.
    ///
    /// # Errors
    ///
    /// Fails if the agent does not exist, is busy, or the change cannot be saved. In that case
    /// nothing is removed.
    pub fn delete_agent(&self, agent_id: &str) -> Result<Vec<Workspace>, AppError> {
        // Held until the end: no message can start while the agent is being removed.
        let _locks = self.lock_agent_in_every_workspace(agent_id)?;
        self.agents.delete(agent_id)?;
        self.workspaces.remove_agent_everywhere(agent_id)?;
        self.chat.discard_agent(agent_id);
        Ok(self.workspaces.list())
    }

    /// Deletes the workspace with its conversations and the usage recorded in it; agents are
    /// untouched. A workspace in which an agent is working cannot be deleted. Returns the
    /// remaining workspaces.
    ///
    /// # Errors
    ///
    /// Fails if the workspace does not exist, an agent is busy in it, or saving fails.
    pub fn delete_workspace(&self, workspace_id: &str) -> Result<Vec<Workspace>, AppError> {
        if self.workspaces.get(workspace_id).is_none() {
            return Err(AppError::new(ErrorCode::WorkspaceNotFound));
        }
        let mut locks = Vec::new();
        for agent in self.agents.list() {
            locks.push(
                self.chat
                    .lock_agent(workspace_id, &agent.id)
                    .map_err(|_| AppError::new(ErrorCode::WorkspaceBusy))?,
            );
        }
        self.workspaces.delete(workspace_id)?;
        self.settings.clear_selection_if(workspace_id)?;
        self.ledger.forget_workspace(workspace_id)?;
        self.chat.discard_workspace(workspace_id);
        Ok(self.workspaces.list())
    }

    fn lock_agent_in_every_workspace(&self, agent_id: &str) -> Result<Vec<AgentLock>, AppError> {
        self.workspaces
            .list()
            .iter()
            .map(|workspace| {
                self.chat
                    .lock_agent(&workspace.id, agent_id)
                    .map_err(|_| AppError::new(ErrorCode::AgentBusy))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::agents::CreateAgentRequest;
    use crate::application::chat::{ChatObserver, SendMessageRequest};
    use crate::application::config::memory::MemoryStore;
    use crate::application::config::ConfigRepository;
    use crate::application::executions::{ExecutionObserver, ExecutionService};
    use crate::application::personalities::PersonalityService;
    use crate::application::projects::fake::FakeInspector;
    use crate::application::runtimes::fake::FakeRuntime;
    use crate::application::runtimes::RuntimeRegistry;
    use crate::application::workspace::WorkspaceInput;
    use crate::domain::conversation::Message;
    use crate::domain::execution::ExecutionEvent;

    struct Silent;

    impl ExecutionObserver for Silent {
        fn on_event(&self, _: &ExecutionEvent) {}
    }

    impl ChatObserver for Silent {
        fn on_message(&self, _: &Message) {}
    }

    struct Fixture {
        lifecycle: AgentLifecycle,
        agents: Arc<AgentService>,
        chat: ChatService,
        workspaces: Arc<WorkspaceService>,
        settings: Arc<SettingsService>,
        ledger: Arc<UsageLedger>,
        ids: Vec<String>,
        w1: String,
        w2: String,
    }

    fn fixture() -> Fixture {
        let config = Arc::new(ConfigRepository::load(Box::<MemoryStore>::default()));
        let personalities = Arc::new(PersonalityService::new(config.clone()));
        let runtimes = Arc::new(RuntimeRegistry::new(vec![Arc::new(FakeRuntime::new(
            "rt",
            Ok("answer"),
        ))]));
        let agents = Arc::new(AgentService::new(
            config.clone(),
            personalities.clone(),
            runtimes.clone(),
        ));
        let ids: Vec<String> = (0..2)
            .map(|i| {
                agents
                    .create(CreateAgentRequest {
                        name: format!("Agent {i}"),
                        personality_id: "architect".to_owned(),
                        runtime_id: "rt".to_owned(),
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
            Arc::new(FakeInspector::with(&[("/a", &[]), ("/b", &[])])),
        ));
        let make = |name: &str, path: &str| {
            workspaces
                .create(&WorkspaceInput {
                    name: name.to_owned(),
                    project_path: path.to_owned(),
                    description: None,
                })
                .unwrap()
                .id
        };
        let (w1, w2) = (make("A", "/a"), make("B", "/b"));
        for id in &ids {
            workspaces.add_agent(&w1, id).unwrap();
        }
        workspaces.add_agent(&w2, &ids[0]).unwrap();
        let executions = Arc::new(ExecutionService::new(
            agents.clone(),
            personalities,
            runtimes,
            workspaces.clone(),
        ));
        let ledger = Arc::new(UsageLedger::new(config.clone()));
        let settings = Arc::new(SettingsService::new(config));
        let chat = ChatService::new(
            agents.clone(),
            executions,
            workspaces.clone(),
            ledger.clone(),
        );
        Fixture {
            lifecycle: AgentLifecycle::new(
                agents.clone(),
                chat.clone(),
                workspaces.clone(),
                ledger.clone(),
                settings.clone(),
            ),
            agents,
            chat,
            workspaces,
            settings,
            ledger,
            ids,
            w1,
            w2,
        }
    }

    fn talk(f: &Fixture, workspace: &str, agent: &str) {
        let (_, run) = f
            .chat
            .send(SendMessageRequest {
                workspace_id: workspace.to_owned(),
                agent_id: agent.to_owned(),
                content: "hello".to_owned(),
            })
            .unwrap();
        run.run(&Silent);
    }

    #[test]
    fn deleting_an_agent_removes_its_slots_and_conversations_everywhere_but_keeps_what_it_spent() {
        let f = fixture();
        talk(&f, &f.w1, &f.ids[0]);
        talk(&f, &f.w2, &f.ids[0]);
        talk(&f, &f.w1, &f.ids[1]);

        let workspaces = f.lifecycle.delete_agent(&f.ids[0]).unwrap();

        assert!(f.agents.find(&f.ids[0]).is_none());
        assert_eq!(workspaces, f.workspaces.list());
        let placed = |ws: &str| -> Vec<String> {
            f.workspaces
                .get(ws)
                .unwrap()
                .layout
                .agent_placements
                .into_iter()
                .map(|p| p.agent_id)
                .collect()
        };
        assert_eq!(placed(&f.w1), [f.ids[1].clone()]);
        assert_eq!(placed(&f.w2), Vec::<String>::new());
        assert_eq!(f.chat.messages(None, Some(&f.ids[0])), []);
        assert_eq!(f.chat.messages(None, Some(&f.ids[1])).len(), 2);
        assert_eq!(f.ledger.records().len(), 3, "usage already spent is kept");
    }

    #[test]
    fn a_busy_agent_cannot_be_deleted_and_nothing_is_removed() {
        let f = fixture();
        let (_, running) = f
            .chat
            .send(SendMessageRequest {
                workspace_id: f.w2.clone(),
                agent_id: f.ids[0].clone(),
                content: "x".to_owned(),
            })
            .unwrap();

        let error = f.lifecycle.delete_agent(&f.ids[0]).unwrap_err();

        assert!(error.is(ErrorCode::AgentBusy));
        assert!(f.agents.find(&f.ids[0]).is_some());
        assert_eq!(
            f.workspaces
                .get(&f.w1)
                .unwrap()
                .layout
                .agent_placements
                .len(),
            2
        );
        running.run(&Silent);
        f.lifecycle.delete_agent(&f.ids[0]).unwrap();
    }

    #[test]
    fn deleting_an_unknown_agent_changes_nothing() {
        let f = fixture();

        assert!(f
            .lifecycle
            .delete_agent("ghost")
            .unwrap_err()
            .is(ErrorCode::AgentNotFound));
        assert_eq!(
            f.workspaces
                .get(&f.w1)
                .unwrap()
                .layout
                .agent_placements
                .len(),
            2
        );
    }

    #[test]
    fn deleting_a_workspace_removes_its_conversations_and_usage_but_not_agents_or_other_workspaces()
    {
        let f = fixture();
        talk(&f, &f.w1, &f.ids[0]);
        talk(&f, &f.w2, &f.ids[0]);
        f.settings.select_workspace(Some(&f.w1)).unwrap();

        let remaining = f.lifecycle.delete_workspace(&f.w1).unwrap();

        let ids: Vec<&str> = remaining.iter().map(|w| w.id.as_str()).collect();
        assert_eq!(ids, [f.w2.as_str()]);
        assert_eq!(f.chat.messages(Some(&f.w1), None), []);
        assert_eq!(f.chat.messages(Some(&f.w2), None).len(), 2);
        let records = f.ledger.records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].workspace_id, f.w2);
        assert_eq!(f.agents.list().len(), 2);
        assert_eq!(f.settings.get().selected_workspace_id, None);
    }

    #[test]
    fn a_workspace_with_a_working_agent_cannot_be_deleted() {
        let f = fixture();
        let (_, running) = f
            .chat
            .send(SendMessageRequest {
                workspace_id: f.w1.clone(),
                agent_id: f.ids[1].clone(),
                content: "x".to_owned(),
            })
            .unwrap();

        assert!(f
            .lifecycle
            .delete_workspace(&f.w1)
            .unwrap_err()
            .is(ErrorCode::WorkspaceBusy));
        assert!(f.workspaces.get(&f.w1).is_some());
        assert!(f
            .lifecycle
            .delete_workspace("ghost")
            .unwrap_err()
            .is(ErrorCode::WorkspaceNotFound));
        running.run(&Silent);
        f.lifecycle.delete_workspace(&f.w1).unwrap();
    }
}
