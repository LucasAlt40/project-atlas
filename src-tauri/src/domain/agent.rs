use serde::{Deserialize, Serialize};

/// A concrete agent: a personality bound to a provider, a model and standing instructions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub personality_id: String,
    /// Which runtime executes the agent. Files written before runtimes existed call this
    /// `providerId`.
    #[serde(alias = "providerId")]
    pub runtime_id: String,
    pub model_id: String,
    pub instructions: String,
    /// Milliseconds since the Unix epoch.
    pub created_at: u64,
}
