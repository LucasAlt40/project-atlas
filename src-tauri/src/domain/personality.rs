use serde::{Deserialize, Serialize};

use super::result_contract::ResultContract;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonalitySource {
    Builtin,
    Custom,
}

/// How an agent behaves. A reusable preset: many agents can share one personality.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalityProfile {
    pub id: String,
    pub name: String,
    pub description: String,
    pub system_instructions: String,
    /// Short human-readable summary of the behaviour, shown before the profile is used.
    #[serde(default)]
    pub behavior: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub source: PersonalitySource,
    /// The contract new agents of this personality start from. A suggestion for the editor:
    /// the agent can change it, and it never applies to an agent by itself.
    #[serde(default)]
    pub suggested_contract: ResultContract,
    /// The permission profile new agents of this personality start from (`developer` or
    /// `read_only`). A suggestion for the editor, like the contract: the agent can change it, and
    /// it never applies to an agent by itself.
    #[serde(default = "default_suggested_profile")]
    pub suggested_permission_profile: String,
}

fn default_suggested_profile() -> String {
    "developer".to_owned()
}
