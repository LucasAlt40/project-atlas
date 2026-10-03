use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
}

/// One entry in an agent's conversation. A user message starts an execution; the
/// assistant message answering it carries that execution's id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub workspace_id: String,
    pub agent_id: String,
    pub execution_id: Option<String>,
    pub role: MessageRole,
    pub content: String,
    /// Milliseconds since the Unix epoch.
    pub timestamp: u64,
    /// The assistant could not answer: `content` is a short user-facing explanation and the
    /// technical details live on the execution.
    pub failed: bool,
    /// Why the assistant could not answer (`failureKind` codes), when it failed.
    pub failure_kind: Option<String>,
}
