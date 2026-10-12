use serde::{Deserialize, Serialize};

use super::project::folder_name;
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

/// What a project does in the system. Only a label for people and agents: nothing branches on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectRole {
    Frontend,
    Backend,
    Bff,
    Service,
    Library,
    Infrastructure,
    Other,
}

/// A folder registered in a workspace. Its identity is the id, stable for as long as it stays in
/// the workspace; the path may change (the folder moved) without it becoming another project.
/// The Harness of the project lives in the folder itself (`.atlas/`), not here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceProject {
    pub id: String,
    /// Friendly name; the folder's name until the user chooses another.
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub role: Option<ProjectRole>,
    /// Ids of projects of the same workspace this one calls or consumes: declared by the user,
    /// never guessed. They appear in the workspace context.
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// A project environment where agents work: the project folders registered in it, the agents
/// placed in it and their layout. Agents, personalities and runtimes are global; only their
/// placement lives here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub name: String,
    /// The folder of the primary project (`projects[0]`), kept so that what was written before a
    /// workspace could have several projects, and everything that only needs one folder, keeps
    /// working. Always equal to the first project's path (see [`Workspace::normalize`]).
    pub project_path: String,
    /// The registered projects, primary first. Files written before multi-project workspaces
    /// have none: [`Workspace::normalize`] derives the one from `project_path`.
    #[serde(default)]
    pub projects: Vec<WorkspaceProject>,
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

impl Workspace {
    /// Brings a workspace to the current shape. A workspace saved with only `project_path` gets
    /// one project made from it (a stable id derived from the workspace's, so repeating this
    /// changes nothing); otherwise `project_path` is made to follow the primary project, and a
    /// dependency on a project that is gone is dropped. Returns whether anything changed.
    pub fn normalize(&mut self) -> bool {
        let mut changed = false;
        if self.projects.is_empty() && !self.project_path.is_empty() {
            self.projects.push(WorkspaceProject {
                id: format!("{}-main", self.id),
                name: folder_name(&self.project_path),
                path: self.project_path.clone(),
                role: None,
                depends_on: Vec::new(),
            });
            changed = true;
        }
        if let Some(primary) = self.projects.first() {
            if self.project_path != primary.path {
                self.project_path.clone_from(&primary.path);
                changed = true;
            }
        }
        let ids: Vec<String> = self.projects.iter().map(|p| p.id.clone()).collect();
        for project in &mut self.projects {
            let before = project.depends_on.len();
            project
                .depends_on
                .retain(|id| *id != project.id && ids.contains(id));
            changed |= project.depends_on.len() != before;
        }
        changed
    }

    pub fn project(&self, id: &str) -> Option<&WorkspaceProject> {
        self.projects.iter().find(|p| p.id == id)
    }

    /// The project agents work in unless told otherwise.
    pub fn primary_project(&self) -> Option<&WorkspaceProject> {
        self.projects.first()
    }
}

#[cfg(test)]
#[allow(clippy::assert_is_empty)]
mod tests {
    use super::*;

    fn legacy(path: &str) -> Workspace {
        Workspace {
            id: "ws-1".to_owned(),
            name: "Atlas".to_owned(),
            project_path: path.to_owned(),
            projects: Vec::new(),
            description: None,
            created_at: 1,
            updated_at: 1,
            layout: WorkspaceLayout {
                rows: 2,
                columns: 2,
                agent_placements: Vec::new(),
            },
            security: SecurityPolicy::default(),
        }
    }

    #[test]
    fn a_workspace_with_only_a_path_becomes_one_project() {
        let mut workspace = legacy("/dev/erp");

        assert!(workspace.normalize());

        assert_eq!(workspace.projects.len(), 1);
        let project = &workspace.projects[0];
        assert_eq!(project.id, "ws-1-main");
        assert_eq!(project.name, "erp");
        assert_eq!(project.path, "/dev/erp");
        assert_eq!(workspace.project_path, "/dev/erp");
    }

    #[test]
    fn normalizing_twice_changes_nothing_the_second_time() {
        let mut workspace = legacy("/dev/erp");
        workspace.normalize();
        let once = workspace.clone();

        assert!(!workspace.normalize());

        assert_eq!(workspace, once);
    }

    #[test]
    fn the_legacy_path_follows_the_primary_project() {
        let mut workspace = legacy("/old");
        workspace.normalize();
        workspace.projects[0].path = "/new".to_owned();

        assert!(workspace.normalize());

        assert_eq!(workspace.project_path, "/new");
    }

    #[test]
    fn a_dependency_on_a_project_that_is_gone_is_dropped() {
        let mut workspace = legacy("/a");
        workspace.normalize();
        workspace.projects[0].depends_on = vec!["gone".to_owned(), "ws-1-main".to_owned()];

        assert!(workspace.normalize());

        assert!(workspace.projects[0].depends_on.is_empty());
    }

    #[test]
    fn a_file_written_before_projects_existed_still_reads() {
        let json = r#"{"id":"w","name":"n","projectPath":"/p","description":null,
            "createdAt":1,"updatedAt":1,
            "layout":{"rows":2,"columns":2,"agentPlacements":[]}}"#;

        let mut workspace: Workspace = serde_json::from_str(json).unwrap();
        assert!(workspace.projects.is_empty());
        workspace.normalize();

        assert_eq!(workspace.projects[0].path, "/p");
    }
}
