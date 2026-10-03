use std::sync::Arc;

use super::agents::AgentService;
use super::chat::ChatService;
use super::errors::{AppError, ErrorCode};
use super::usage::{summarize_agent, summarize_workspace, UsageLedger};
use crate::domain::usage::{AgentUsageSummary, UsageWindows, WorkspaceUsageSummary};

/// Use case: answer "how much has this consumed?" from what Atlas observed. Combines the usage
/// ledger with the conversations (which executions belong to the current conversation).
pub struct UsageReporter {
    ledger: Arc<UsageLedger>,
    chat: ChatService,
    agents: Arc<AgentService>,
}

impl UsageReporter {
    pub fn new(ledger: Arc<UsageLedger>, chat: ChatService, agents: Arc<AgentService>) -> Self {
        Self {
            ledger,
            chat,
            agents,
        }
    }

    /// # Errors
    ///
    /// Fails if the agent does not exist.
    pub fn agent_summary(
        &self,
        workspace_id: &str,
        agent_id: &str,
        windows: UsageWindows,
    ) -> Result<AgentUsageSummary, AppError> {
        let agent = self
            .agents
            .find(agent_id)
            .ok_or_else(|| AppError::new(ErrorCode::AgentNotFound))?;
        Ok(summarize_agent(
            &self.ledger.records(),
            workspace_id,
            agent_id,
            &self.chat.conversation_executions(workspace_id, agent_id),
            windows,
            self.ledger.quota_of(&agent.runtime_id),
        ))
    }

    pub fn workspace_summary(
        &self,
        workspace_id: &str,
        windows: UsageWindows,
    ) -> WorkspaceUsageSummary {
        summarize_workspace(&self.ledger.records(), workspace_id, windows)
    }
}
