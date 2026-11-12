use serde::{Deserialize, Serialize};

use super::result_contract::ResultContract;

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
    /// Which permission profile bounds the agent (`read_only`, `developer`). Unset or
    /// unknown means the most restrictive one. A profile is a bundle of permissions kept in
    /// the core; the agent does not carry a policy of its own.
    #[serde(default)]
    pub permission_profile_id: Option<String>,
    /// Whether each execution works in its own Git worktree instead of the project's checkout.
    /// On unless the user turns it off; agents saved before this setting existed get it on.
    #[serde(default = "default_worktree_isolation")]
    pub worktree_isolation: bool,
    /// What the agent promises to say at the end of a step, for workflows to route on. Agents
    /// saved before contracts existed (and agents that declare none) are `general`: no outcome
    /// is required of them. The personality may suggest a contract; the agent's own is what counts.
    #[serde(default)]
    pub result_contract: ResultContract,
    /// The runtime and model a configuration file asked for, when the agent was imported with
    /// others (ADR 0032). Kept so the original survives the choice of an alternative; cleared
    /// when the person changes the agent's runtime or model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported_model: Option<ImportedModel>,
    /// Milliseconds since the Unix epoch.
    pub created_at: u64,
}

/// What an imported agent's file asked to run on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedModel {
    pub runtime_id: String,
    pub model_id: String,
}

fn default_worktree_isolation() -> bool {
    true
}
