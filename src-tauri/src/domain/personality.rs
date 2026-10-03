use serde::{Deserialize, Serialize};

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
}
