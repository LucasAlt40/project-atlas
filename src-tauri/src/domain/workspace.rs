use serde::{Deserialize, Serialize};

use super::security::SecurityPolicy;

/// A cell of the workspace grid. Independent of the agent so layouts can later be saved,
/// swapped or resized without touching agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridPosition {
    pub row: u32,
    pub column: u32,
}

/// Where an agent sits in a workspace. The agent itself does not know.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentPlacement {
    pub agent_id: String,
    pub position: GridPosition,
}

/// Which agents are shown and where. The grid size is data, not a rule of the domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceLayout {
    pub rows: u32,
    pub columns: u32,
    pub agent_placements: Vec<AgentPlacement>,
}

impl WorkspaceLayout {
    /// The first free cell, scanning row by row.
    pub fn first_free_position(&self) -> Option<GridPosition> {
        (0..self.rows)
            .flat_map(|row| (0..self.columns).map(move |column| GridPosition { row, column }))
            .find(|position| {
                !self
                    .agent_placements
                    .iter()
                    .any(|placement| placement.position == *position)
            })
    }

    pub fn is_placed(&self, agent_id: &str) -> bool {
        self.agent_placements
            .iter()
            .any(|placement| placement.agent_id == agent_id)
    }
}

/// A project environment where agents work: a project folder, the agents placed in it and
/// their layout. Agents, personalities and runtimes are global; only their placement lives here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub name: String,
    /// The project's folder. It is the context boundary; nothing is read from it automatically.
    pub project_path: String,
    pub description: Option<String>,
    /// Milliseconds since the Unix epoch.
    pub created_at: u64,
    pub updated_at: u64,
    pub layout: WorkspaceLayout,
    /// What agents may do here. Stored per workspace; profiles and runtimes only narrow it.
    /// Files written before V0.6 have none and get the default.
    #[serde(default)]
    pub security: SecurityPolicy,
}
