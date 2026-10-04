use std::sync::Arc;

use serde::Deserialize;

use super::agents::AgentService;
use super::config::ConfigRepository;
use super::errors::{AppError, ErrorCode};
use super::projects::{project_context, ProjectInspector};
use super::support::{new_id, now_ms};
use crate::domain::project::ProjectContext;
use crate::domain::security::SecurityPolicy;
use crate::domain::workspace::{AgentPlacement, Workspace, WorkspaceLayout};

const MAX_NAME_LEN: usize = 80;
/// V0.5 shows four agents at once, so the default layout is 2 x 2. The size is data on the
/// workspace: later versions let the user choose it.
const DEFAULT_ROWS: u32 = 2;
const DEFAULT_COLUMNS: u32 = 2;

/// What the user provides to create or change a workspace.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInput {
    pub name: String,
    pub project_path: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// Use case: project workspaces and where agents sit in them. Persisted in the user config.
/// A placement is only a position; adding or removing one never creates or deletes the agent.
pub struct WorkspaceService {
    config: Arc<ConfigRepository>,
    agents: Arc<AgentService>,
    inspector: Arc<dyn ProjectInspector>,
}

impl WorkspaceService {
    pub fn new(
        config: Arc<ConfigRepository>,
        agents: Arc<AgentService>,
        inspector: Arc<dyn ProjectInspector>,
    ) -> Self {
        Self {
            config,
            agents,
            inspector,
        }
    }

    /// Workspaces in creation order.
    pub fn list(&self) -> Vec<Workspace> {
        self.config.snapshot().workspaces
    }

    pub fn get(&self, id: &str) -> Option<Workspace> {
        self.list().into_iter().find(|w| w.id == id)
    }

    /// What agents are told about the workspace's project: its name, path and the technologies
    /// recognised in its folder right now.
    ///
    /// # Errors
    ///
    /// Fails if the workspace does not exist.
    pub fn project_context(&self, id: &str) -> Result<ProjectContext, AppError> {
        let workspace = self.get(id).ok_or_else(not_found)?;
        Ok(project_context(
            self.inspector.as_ref(),
            &workspace.project_path,
        ))
    }

    /// # Errors
    ///
    /// Fails if the name is empty or too long, the project folder is missing or not a folder,
    /// or saving fails.
    pub fn create(&self, input: &WorkspaceInput) -> Result<Workspace, AppError> {
        let fields = self.validate(input)?;
        let now = now_ms();
        let workspace = Workspace {
            id: new_id("ws"),
            name: fields.name,
            project_path: fields.path,
            description: fields.description,
            created_at: now,
            updated_at: now,
            layout: WorkspaceLayout {
                rows: DEFAULT_ROWS,
                columns: DEFAULT_COLUMNS,
                agent_placements: Vec::new(),
            },
            security: SecurityPolicy::default(),
        };
        self.config.modify(|config| {
            config.workspaces.push(workspace.clone());
            Ok(())
        })?;
        Ok(workspace)
    }

    /// Renames the workspace and/or points it at another folder. Its layout is kept.
    ///
    /// # Errors
    ///
    /// Fails if the workspace does not exist, the new values are invalid, or saving fails.
    pub fn update(&self, id: &str, input: &WorkspaceInput) -> Result<Workspace, AppError> {
        let fields = self.validate(input)?;
        self.config.modify(|config| {
            let workspace = config
                .workspaces
                .iter_mut()
                .find(|w| w.id == id)
                .ok_or_else(not_found)?;
            workspace.name = fields.name;
            workspace.project_path = fields.path;
            workspace.description = fields.description;
            workspace.updated_at = now_ms();
            Ok(workspace.clone())
        })
    }

    /// Removes the workspace itself. Its conversations and usage are cleaned up by
    /// `AgentLifecycle`; agents are not touched.
    ///
    /// # Errors
    ///
    /// Fails if the workspace does not exist or saving fails.
    pub fn delete(&self, id: &str) -> Result<(), AppError> {
        self.config.modify(|config| {
            let before = config.workspaces.len();
            config.workspaces.retain(|w| w.id != id);
            if config.workspaces.len() == before {
                return Err(not_found());
            }
            Ok(())
        })
    }

    /// Places the agent in the first free position.
    ///
    /// # Errors
    ///
    /// Fails if the workspace or agent is unknown, the agent is already placed, the grid is
    /// full, or saving fails.
    pub fn add_agent(&self, workspace_id: &str, agent_id: &str) -> Result<Workspace, AppError> {
        if self.agents.find(agent_id).is_none() {
            return Err(AppError::new(ErrorCode::AgentNotFound));
        }
        self.config.modify(|config| {
            let workspace = config
                .workspaces
                .iter_mut()
                .find(|w| w.id == workspace_id)
                .ok_or_else(not_found)?;
            if workspace.layout.is_placed(agent_id) {
                return Err(AppError::new(ErrorCode::AgentAlreadyPlaced));
            }
            let position = workspace
                .layout
                .first_free_position()
                .ok_or_else(|| AppError::new(ErrorCode::WorkspaceFull))?;
            workspace.layout.agent_placements.push(AgentPlacement {
                agent_id: agent_id.to_owned(),
                position,
            });
            workspace.updated_at = now_ms();
            Ok(workspace.clone())
        })
    }

    /// Frees the agent's position. The agent itself is untouched; removing an agent that is
    /// not placed does nothing.
    ///
    /// # Errors
    ///
    /// Fails if the workspace is unknown or saving fails.
    pub fn remove_agent(&self, workspace_id: &str, agent_id: &str) -> Result<Workspace, AppError> {
        self.config.modify(|config| {
            let workspace = config
                .workspaces
                .iter_mut()
                .find(|w| w.id == workspace_id)
                .ok_or_else(not_found)?;
            workspace
                .layout
                .agent_placements
                .retain(|p| p.agent_id != agent_id);
            Ok(workspace.clone())
        })
    }

    /// Takes the agent out of every workspace.
    ///
    /// # Errors
    ///
    /// Fails if saving fails.
    pub fn remove_agent_everywhere(&self, agent_id: &str) -> Result<(), AppError> {
        self.config.modify(|config| {
            for workspace in &mut config.workspaces {
                workspace
                    .layout
                    .agent_placements
                    .retain(|p| p.agent_id != agent_id);
            }
            Ok(())
        })
    }

    fn validate(&self, input: &WorkspaceInput) -> Result<Fields, AppError> {
        let name = input.name.trim();
        if name.is_empty() {
            return Err(AppError::new(ErrorCode::NameRequired));
        }
        if name.chars().count() > MAX_NAME_LEN {
            return Err(AppError::new(ErrorCode::NameTooLong).with("max", MAX_NAME_LEN.to_string()));
        }
        let path = input.project_path.trim();
        if path.is_empty() {
            return Err(AppError::new(ErrorCode::ProjectFolderRequired));
        }
        if !self.inspector.is_directory(path) {
            return Err(AppError::new(ErrorCode::ProjectFolderNotFound).with("path", path));
        }
        Ok(Fields {
            name: name.to_owned(),
            path: path.to_owned(),
            description: input
                .description
                .as_deref()
                .map(str::trim)
                .filter(|d| !d.is_empty())
                .map(str::to_owned),
        })
    }
}

struct Fields {
    name: String,
    path: String,
    description: Option<String>,
}

fn not_found() -> AppError {
    AppError::new(ErrorCode::WorkspaceNotFound)
}

#[cfg(test)]
#[allow(clippy::assert_is_empty, clippy::cloned_ref_to_slice_refs)]
mod tests {
    use super::*;
    use crate::application::agents::CreateAgentRequest;
    use crate::application::config::memory::MemoryStore;
    use crate::application::personalities::PersonalityService;
    use crate::application::projects::fake::FakeInspector;
    use crate::application::runtimes::fake::FakeRuntime;
    use crate::application::runtimes::RuntimeRegistry;
    use crate::domain::workspace::GridPosition;

    struct Fixture {
        service: WorkspaceService,
        agents: Arc<AgentService>,
        store: Arc<MemoryStore>,
    }

    fn fixture() -> Fixture {
        let store = Arc::new(MemoryStore::default());
        build(&store)
    }

    /// A service over the given store, so a second one can "restart" on the same data.
    fn build(store: &Arc<MemoryStore>) -> Fixture {
        let config = Arc::new(ConfigRepository::load(Box::new(store.clone())));
        let personalities = Arc::new(PersonalityService::new(config.clone()));
        let runtimes = Arc::new(RuntimeRegistry::new(vec![Arc::new(FakeRuntime::new(
            "fake",
            Ok("x"),
        ))]));
        let agents = Arc::new(AgentService::new(config.clone(), personalities, runtimes));
        let inspector = Arc::new(FakeInspector::with(&[
            ("/dev/erp", &["Angular", ".NET"]),
            ("/dev/atlas", &["Rust"]),
        ]));
        Fixture {
            service: WorkspaceService::new(config, agents.clone(), inspector),
            agents,
            store: store.clone(),
        }
    }

    fn input(name: &str, path: &str) -> WorkspaceInput {
        WorkspaceInput {
            name: name.to_owned(),
            project_path: path.to_owned(),
            description: Some("  ERP system ".to_owned()),
        }
    }

    fn agent(f: &Fixture, n: usize) -> String {
        f.agents
            .create(CreateAgentRequest {
                permission_profile_id: None,
                name: format!("Agent {n}"),
                personality_id: "architect".to_owned(),
                runtime_id: "fake".to_owned(),
                model_id: "m1".to_owned(),
                instructions: String::new(),
                worktree_isolation: Some(false),
                result_contract: None,
            })
            .unwrap()
            .id
    }

    fn pos(row: u32, column: u32) -> GridPosition {
        GridPosition { row, column }
    }

    #[test]
    fn creates_a_workspace_that_stores_its_project_folder() {
        let f = fixture();

        let created = f
            .service
            .create(&input("  Acme ERP ", " /dev/erp "))
            .unwrap();

        assert_eq!(created.name, "Acme ERP");
        assert_eq!(created.project_path, "/dev/erp");
        assert_eq!(created.description.as_deref(), Some("ERP system"));
        assert_eq!((created.layout.rows, created.layout.columns), (2, 2));
        assert!(created.layout.agent_placements.is_empty());
        assert_eq!(f.service.list(), [created.clone()]);
        assert_eq!(f.service.get(&created.id), Some(created));
    }

    #[test]
    fn rejects_a_missing_name_and_an_invalid_project_folder() {
        let f = fixture();

        assert!(f
            .service
            .create(&input(" ", "/dev/erp"))
            .unwrap_err()
            .is(ErrorCode::NameRequired));
        assert!(f
            .service
            .create(&input("x", " "))
            .unwrap_err()
            .is(ErrorCode::ProjectFolderRequired));
        let missing = f.service.create(&input("x", "/nowhere")).unwrap_err();
        assert!(missing.is(ErrorCode::ProjectFolderNotFound));
        assert_eq!(missing.params["path"], "/nowhere");
        assert_eq!(f.service.list(), []);
    }

    #[test]
    fn renames_and_repoints_a_workspace_keeping_its_layout() {
        let f = fixture();
        let created = f.service.create(&input("Old", "/dev/erp")).unwrap();
        let agent = agent(&f, 1);
        f.service.add_agent(&created.id, &agent).unwrap();

        let updated = f
            .service
            .update(&created.id, &input("New", "/dev/atlas"))
            .unwrap();

        assert_eq!(
            (updated.name.as_str(), updated.project_path.as_str()),
            ("New", "/dev/atlas")
        );
        assert_eq!(updated.layout, f.service.get(&created.id).unwrap().layout);
        assert_eq!(updated.layout.agent_placements.len(), 1);
        assert_eq!(updated.created_at, created.created_at);
        assert!(f
            .service
            .update("ghost", &input("x", "/dev/erp"))
            .unwrap_err()
            .is(ErrorCode::WorkspaceNotFound));
        assert!(f
            .service
            .update(&created.id, &input("x", "/nowhere"))
            .unwrap_err()
            .is(ErrorCode::ProjectFolderNotFound));
        assert_eq!(f.service.get(&created.id).unwrap().name, "New");
    }

    #[test]
    fn deletes_a_workspace_but_not_its_agents() {
        let f = fixture();
        let created = f.service.create(&input("W", "/dev/erp")).unwrap();
        let agent = agent(&f, 1);
        f.service.add_agent(&created.id, &agent).unwrap();

        f.service.delete(&created.id).unwrap();

        assert_eq!(f.service.list(), []);
        assert!(f.agents.find(&agent).is_some());
        assert!(f
            .service
            .delete(&created.id)
            .unwrap_err()
            .is(ErrorCode::WorkspaceNotFound));
    }

    #[test]
    fn workspaces_survive_a_restart() {
        let f = fixture();
        let created = f.service.create(&input("Atlas", "/dev/atlas")).unwrap();
        let agent = agent(&f, 1);
        f.service.add_agent(&created.id, &agent).unwrap();
        let before = f.service.get(&created.id).unwrap();

        // A new service on the same stored data, as after restarting the app.
        let restarted = build(&f.store);

        assert_eq!(restarted.service.list(), [before]);
        assert!(restarted.agents.find(&agent).is_some());
    }

    #[test]
    fn exposes_the_project_context_of_the_workspace_folder() {
        let f = fixture();
        let created = f.service.create(&input("ERP", "/dev/erp")).unwrap();

        let context = f.service.project_context(&created.id).unwrap();

        assert_eq!(
            (context.name.as_str(), context.path.as_str()),
            ("erp", "/dev/erp")
        );
        assert_eq!(context.technologies, ["Angular", ".NET"]);
        assert!(f
            .service
            .project_context("ghost")
            .unwrap_err()
            .is(ErrorCode::WorkspaceNotFound));
    }

    #[test]
    fn places_agents_in_the_first_free_position_and_frees_it_on_removal() {
        let f = fixture();
        let ws = f.service.create(&input("W", "/dev/erp")).unwrap().id;
        let ids: Vec<_> = (0..3).map(|n| agent(&f, n)).collect();

        f.service.add_agent(&ws, &ids[0]).unwrap();
        f.service.add_agent(&ws, &ids[1]).unwrap();
        let positions: Vec<_> = f
            .service
            .get(&ws)
            .unwrap()
            .layout
            .agent_placements
            .iter()
            .map(|p| p.position)
            .collect();
        assert_eq!(positions, [pos(0, 0), pos(0, 1)]);

        f.service.remove_agent(&ws, &ids[0]).unwrap();
        let workspace = f.service.add_agent(&ws, &ids[2]).unwrap();

        let reused = workspace
            .layout
            .agent_placements
            .iter()
            .find(|p| p.agent_id == ids[2])
            .unwrap();
        assert_eq!(reused.position, pos(0, 0));
    }

    #[test]
    fn placement_is_separate_from_the_agent_and_per_workspace() {
        let f = fixture();
        let (a, b) = (
            f.service.create(&input("A", "/dev/erp")).unwrap().id,
            f.service.create(&input("B", "/dev/atlas")).unwrap().id,
        );
        let shared = agent(&f, 1);
        let before = f.agents.find(&shared).unwrap();

        f.service.add_agent(&a, &shared).unwrap();
        f.service.add_agent(&b, &shared).unwrap();
        f.service.remove_agent(&a, &shared).unwrap();

        // The same agent configuration is reused; only placements changed.
        assert_eq!(f.agents.find(&shared), Some(before));
        assert!(f
            .service
            .get(&a)
            .unwrap()
            .layout
            .agent_placements
            .is_empty());
        assert_eq!(f.service.get(&b).unwrap().layout.agent_placements.len(), 1);
        f.service.remove_agent_everywhere(&shared).unwrap();
        assert!(f
            .service
            .get(&b)
            .unwrap()
            .layout
            .agent_placements
            .is_empty());
        assert!(f.agents.find(&shared).is_some());
    }

    #[test]
    fn rejects_unknown_duplicate_and_overflowing_placements() {
        let f = fixture();
        let ws = f.service.create(&input("W", "/dev/erp")).unwrap().id;
        let ids: Vec<_> = (0..5).map(|n| agent(&f, n)).collect();

        assert!(f
            .service
            .add_agent("other", &ids[0])
            .unwrap_err()
            .is(ErrorCode::WorkspaceNotFound));
        assert!(f
            .service
            .add_agent(&ws, "ghost")
            .unwrap_err()
            .is(ErrorCode::AgentNotFound));
        f.service.add_agent(&ws, &ids[0]).unwrap();
        assert!(f
            .service
            .add_agent(&ws, &ids[0])
            .unwrap_err()
            .is(ErrorCode::AgentAlreadyPlaced));
        // The default 2 x 2 grid has four cells; the fifth agent does not fit.
        for id in &ids[1..4] {
            f.service.add_agent(&ws, id).unwrap();
        }
        assert!(f
            .service
            .add_agent(&ws, &ids[4])
            .unwrap_err()
            .is(ErrorCode::WorkspaceFull));
    }
}
