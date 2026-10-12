use std::sync::Arc;

use serde::Deserialize;

use super::agents::AgentService;
use super::config::ConfigRepository;
use super::errors::{AppError, ErrorCode};
use super::projects::{folder_name, project_context, ProjectInspector};
use super::support::{new_id, now_ms};
use crate::domain::project::ProjectContext;
use crate::domain::scope::{ProjectScope, ScopeRequest};
use crate::domain::security::SecurityPolicy;
use crate::domain::workspace::{
    AgentPlacement, ProjectRole, Workspace, WorkspaceLayout, WorkspaceProject,
};

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

/// A project to register in a workspace. The name is the folder's own unless one is given.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInput {
    pub path: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub role: Option<ProjectRole>,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// A change to a registered project. The id never changes; a path change keeps the project's
/// identity, its role and its Harness-independent settings.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPatch {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub role: Option<ProjectRole>,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// What the workspace screen shows of a registered project: the registration and what is true of
/// the folder right now.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    #[serde(flatten)]
    pub project: WorkspaceProject,
    /// The folder is there and is a folder. A project whose folder is gone stays registered so
    /// its path can be fixed.
    pub exists: bool,
    pub primary: bool,
    pub technologies: Vec<String>,
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

    /// Workspaces in creation order, each in the current shape (see [`Workspace::normalize`]).
    pub fn list(&self) -> Vec<Workspace> {
        let mut workspaces = self.config.snapshot().workspaces;
        for workspace in &mut workspaces {
            workspace.normalize();
        }
        workspaces
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
            project_path: fields.path.clone(),
            projects: vec![WorkspaceProject {
                id: new_id("prj"),
                name: folder_name(&fields.path),
                path: fields.path,
                role: None,
                depends_on: Vec::new(),
            }],
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

    /// Renames the workspace and/or points its primary project at another folder. Its layout and
    /// its other projects are kept.
    ///
    /// # Errors
    ///
    /// Fails if the workspace does not exist, the new values are invalid, the folder is already
    /// another project of the workspace, or saving fails.
    pub fn update(&self, id: &str, input: &WorkspaceInput) -> Result<Workspace, AppError> {
        let fields = self.validate(input)?;
        self.config.modify(|config| {
            let workspace = config
                .workspaces
                .iter_mut()
                .find(|w| w.id == id)
                .ok_or_else(not_found)?;
            workspace.normalize();
            let primary_id = workspace.projects.first().map(|p| p.id.clone());
            if let Some(primary_id) = &primary_id {
                self.ensure_free(workspace, &fields.path, Some(primary_id))?;
            }
            workspace.name = fields.name;
            if let Some(primary) = workspace.projects.first_mut() {
                // A project keeps the name the user chose; only a name that was the folder's own
                // follows the folder.
                if primary.name == folder_name(&primary.path) {
                    primary.name = folder_name(&fields.path);
                }
                primary.path.clone_from(&fields.path);
            }
            workspace.project_path = fields.path;
            workspace.description = fields.description;
            workspace.updated_at = now_ms();
            Ok(workspace.clone())
        })
    }

    /// Registers another project folder in the workspace. Nothing in the folder is read or
    /// changed.
    ///
    /// # Errors
    ///
    /// Fails if the workspace is unknown, the folder is missing or not a folder, it is (or
    /// resolves to) a project the workspace already has, the name is too long, or saving fails.
    pub fn add_project(
        &self,
        workspace_id: &str,
        input: &ProjectInput,
    ) -> Result<Workspace, AppError> {
        let path = self.validate_folder(&input.path)?;
        let name = match input.name.as_deref().map(str::trim) {
            Some(name) if !name.is_empty() => check_name(name)?,
            _ => folder_name(&path),
        };
        self.config.modify(|config| {
            let workspace = config
                .workspaces
                .iter_mut()
                .find(|w| w.id == workspace_id)
                .ok_or_else(not_found)?;
            workspace.normalize();
            self.ensure_free(workspace, &path, None)?;
            let depends_on = known_dependencies(workspace, &input.depends_on, None);
            workspace.projects.push(WorkspaceProject {
                id: new_id("prj"),
                name,
                path,
                role: input.role,
                depends_on,
            });
            workspace.updated_at = now_ms();
            Ok(workspace.clone())
        })
    }

    /// Changes a registered project's name, folder, role or declared dependencies. Its id stays,
    /// so what refers to it (dependencies, a Harness status, a scope) keeps pointing at it.
    ///
    /// # Errors
    ///
    /// As [`Self::add_project`], and if the project is not in the workspace.
    pub fn update_project(
        &self,
        workspace_id: &str,
        project_id: &str,
        patch: &ProjectPatch,
    ) -> Result<Workspace, AppError> {
        let folder = self.validate_folder(&patch.path)?;
        let name = check_name(patch.name.trim())?;
        self.config.modify(|config| {
            let workspace = config
                .workspaces
                .iter_mut()
                .find(|w| w.id == workspace_id)
                .ok_or_else(not_found)?;
            workspace.normalize();
            if workspace.project(project_id).is_none() {
                return Err(AppError::new(ErrorCode::WorkspaceProjectNotFound));
            }
            self.ensure_free(workspace, &folder, Some(project_id))?;
            let depends_on = known_dependencies(workspace, &patch.depends_on, Some(project_id));
            if let Some(project) = workspace.projects.iter_mut().find(|p| p.id == project_id) {
                project.name = name;
                project.path = folder;
                project.role = patch.role;
                project.depends_on = depends_on;
            }
            workspace.normalize();
            workspace.updated_at = now_ms();
            Ok(workspace.clone())
        })
    }

    /// Takes a project out of the workspace. Its folder, its `.atlas/` Harness and its Git
    /// repository are not touched. Removing the primary project makes the next one primary.
    ///
    /// # Errors
    ///
    /// Fails if the workspace or project is unknown, it is the workspace's only project, or
    /// saving fails.
    pub fn remove_project(
        &self,
        workspace_id: &str,
        project_id: &str,
    ) -> Result<Workspace, AppError> {
        self.config.modify(|config| {
            let workspace = config
                .workspaces
                .iter_mut()
                .find(|w| w.id == workspace_id)
                .ok_or_else(not_found)?;
            workspace.normalize();
            if workspace.project(project_id).is_none() {
                return Err(AppError::new(ErrorCode::WorkspaceProjectNotFound));
            }
            if workspace.projects.len() == 1 {
                return Err(AppError::new(ErrorCode::WorkspaceNeedsAProject));
            }
            workspace.projects.retain(|p| p.id != project_id);
            workspace.normalize();
            workspace.updated_at = now_ms();
            Ok(workspace.clone())
        })
    }

    /// The workspace's projects with the state of each folder right now.
    ///
    /// # Errors
    ///
    /// Fails if the workspace does not exist.
    pub fn projects(&self, workspace_id: &str) -> Result<Vec<ProjectInfo>, AppError> {
        let workspace = self.get(workspace_id).ok_or_else(not_found)?;
        Ok(workspace
            .projects
            .iter()
            .enumerate()
            .map(|(index, project)| {
                let exists = self.inspector.is_directory(&project.path);
                ProjectInfo {
                    project: project.clone(),
                    exists,
                    primary: index == 0,
                    technologies: if exists {
                        self.inspector.technologies(&project.path)
                    } else {
                        Vec::new()
                    },
                }
            })
            .collect())
    }

    /// The projects an execution may look at and change. Without a request: the primary project
    /// alone.
    ///
    /// # Errors
    ///
    /// Fails if the workspace is unknown or the request names a project it does not have.
    pub fn resolve_scope(
        &self,
        workspace_id: &str,
        request: Option<&ScopeRequest>,
    ) -> Result<ProjectScope, AppError> {
        let workspace = self.get(workspace_id).ok_or_else(not_found)?;
        ProjectScope::resolve(&workspace, request)
            .map_err(|unknown| AppError::new(ErrorCode::ScopeInvalid).with("project", unknown.0))
    }

    /// What agents are told about one project of a scope: name, path and technologies.
    pub fn context_of(&self, project: &WorkspaceProject) -> ProjectContext {
        ProjectContext {
            name: project.name.clone(),
            path: project.path.clone(),
            technologies: self.inspector.technologies(&project.path),
        }
    }

    /// Fails when `path` is, or resolves to, the folder of a project other than `except`.
    fn ensure_free(
        &self,
        workspace: &Workspace,
        path: &str,
        except: Option<&str>,
    ) -> Result<(), AppError> {
        let wanted = self.inspector.canonical(path);
        let taken = workspace
            .projects
            .iter()
            .filter(|p| Some(p.id.as_str()) != except)
            .any(|p| same_folder(&self.inspector.canonical(&p.path), &wanted));
        if taken {
            return Err(AppError::new(ErrorCode::ProjectAlreadyInWorkspace).with("path", path));
        }
        Ok(())
    }

    fn validate_folder(&self, path: &str) -> Result<String, AppError> {
        let path = path.trim();
        if path.is_empty() {
            return Err(AppError::new(ErrorCode::ProjectFolderRequired));
        }
        if !self.inspector.is_directory(path) {
            return Err(AppError::new(ErrorCode::ProjectFolderNotFound).with("path", path));
        }
        Ok(path.to_owned())
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
        let name = check_name(input.name.trim())?;
        let path = self.validate_folder(&input.project_path)?;
        Ok(Fields {
            name,
            path,
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

fn check_name(name: &str) -> Result<String, AppError> {
    if name.is_empty() {
        return Err(AppError::new(ErrorCode::NameRequired));
    }
    if name.chars().count() > MAX_NAME_LEN {
        return Err(AppError::new(ErrorCode::NameTooLong).with("max", MAX_NAME_LEN.to_string()));
    }
    Ok(name.to_owned())
}

/// Two spellings of one folder. Case is ignored on Windows and macOS, whose file systems are
/// usually case-insensitive, so `C:\Dev\App` and `c:\dev\app` are one project there.
fn same_folder(a: &str, b: &str) -> bool {
    if cfg!(any(windows, target_os = "macos")) {
        a.to_lowercase() == b.to_lowercase()
    } else {
        a == b
    }
}

/// The declared dependencies that are projects of the workspace (and not the project itself),
/// each once.
fn known_dependencies(
    workspace: &Workspace,
    wanted: &[String],
    owner: Option<&str>,
) -> Vec<String> {
    let mut known: Vec<String> = Vec::new();
    for id in wanted {
        if workspace.project(id).is_some() && Some(id.as_str()) != owner && !known.contains(id) {
            known.push(id.clone());
        }
    }
    known
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
        let inspector = Arc::new(
            FakeInspector::with(&[
                ("/dev/erp", &["Angular", ".NET"]),
                ("/dev/atlas", &["Rust"]),
                ("/dev/shop/front", &["Angular"]),
                ("/dev/shop/back", &["Maven"]),
                ("/dev/shop/orders", &["Maven"]),
            ])
            .alias("/links/front", "/dev/shop/front"),
        );
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

    fn shop(f: &Fixture) -> Workspace {
        let created = f.service.create(&input("Shop", "/dev/shop/front")).unwrap();
        f.service
            .add_project(
                &created.id,
                &ProjectInput {
                    path: "/dev/shop/back".to_owned(),
                    name: Some("API".to_owned()),
                    role: Some(ProjectRole::Backend),
                    depends_on: Vec::new(),
                },
            )
            .unwrap();
        f.service
            .add_project(
                &created.id,
                &ProjectInput {
                    path: "/dev/shop/orders".to_owned(),
                    name: None,
                    role: Some(ProjectRole::Service),
                    depends_on: Vec::new(),
                },
            )
            .unwrap()
    }

    fn only_path(path: &str) -> ProjectInput {
        ProjectInput {
            path: path.to_owned(),
            name: None,
            role: None,
            depends_on: Vec::new(),
        }
    }

    #[test]
    fn a_new_workspace_has_its_folder_as_the_primary_project() {
        let f = fixture();

        let created = f.service.create(&input("ERP", "/dev/erp")).unwrap();

        assert_eq!(created.projects.len(), 1);
        assert_eq!(created.projects[0].path, "/dev/erp");
        assert_eq!(created.projects[0].name, "erp");
        assert_eq!(created.project_path, "/dev/erp");
    }

    #[test]
    fn several_projects_live_in_one_workspace_in_the_order_they_were_added() {
        let f = fixture();

        let workspace = shop(&f);

        let seen: Vec<_> = workspace
            .projects
            .iter()
            .map(|p| (p.name.as_str(), p.role))
            .collect();
        assert_eq!(
            seen,
            [
                ("front", None),
                ("API", Some(ProjectRole::Backend)),
                ("orders", Some(ProjectRole::Service)),
            ]
        );
        // The ids are distinct and the primary folder is still the workspace's own.
        let ids: std::collections::BTreeSet<_> = workspace.projects.iter().map(|p| &p.id).collect();
        assert_eq!(ids.len(), 3);
        assert_eq!(workspace.project_path, "/dev/shop/front");
    }

    #[test]
    fn the_same_folder_cannot_be_added_twice_however_it_is_spelled() {
        let f = fixture();
        let workspace = shop(&f);

        for spelling in ["/dev/shop/back", "/dev/shop/back/", "/links/front"] {
            let error = f
                .service
                .add_project(&workspace.id, &only_path(spelling))
                .unwrap_err();
            assert!(error.is(ErrorCode::ProjectAlreadyInWorkspace), "{spelling}");
        }
        assert_eq!(f.service.get(&workspace.id).unwrap().projects.len(), 3);
    }

    #[test]
    fn a_missing_or_empty_folder_is_not_registered() {
        let f = fixture();
        let workspace = shop(&f);

        let missing = f
            .service
            .add_project(&workspace.id, &only_path("/dev/gone"))
            .unwrap_err();
        let empty = f
            .service
            .add_project(&workspace.id, &only_path("  "))
            .unwrap_err();

        assert!(missing.is(ErrorCode::ProjectFolderNotFound));
        assert!(empty.is(ErrorCode::ProjectFolderRequired));
        assert_eq!(f.service.get(&workspace.id).unwrap().projects.len(), 3);
    }

    #[test]
    fn removing_a_project_keeps_the_others_and_promotes_the_next_primary() {
        let f = fixture();
        let workspace = shop(&f);
        let primary = workspace.projects[0].id.clone();

        let after = f.service.remove_project(&workspace.id, &primary).unwrap();

        assert_eq!(after.projects.len(), 2);
        assert_eq!(after.projects[0].name, "API");
        assert_eq!(after.project_path, "/dev/shop/back");
    }

    #[test]
    fn the_last_project_cannot_be_removed_and_an_unknown_one_is_reported() {
        let f = fixture();
        let created = f.service.create(&input("ERP", "/dev/erp")).unwrap();

        let last = f
            .service
            .remove_project(&created.id, &created.projects[0].id)
            .unwrap_err();
        let unknown = f.service.remove_project(&created.id, "nope").unwrap_err();

        assert!(last.is(ErrorCode::WorkspaceNeedsAProject));
        assert!(unknown.is(ErrorCode::WorkspaceProjectNotFound));
    }

    #[test]
    fn changing_a_projects_folder_keeps_its_identity_and_refuses_another_projects_folder() {
        let f = fixture();
        let workspace = shop(&f);
        let api = workspace.projects[1].clone();
        let patch = |path: &str| ProjectPatch {
            name: "API".to_owned(),
            path: path.to_owned(),
            role: Some(ProjectRole::Backend),
            depends_on: vec![
                workspace.projects[2].id.clone(),
                api.id.clone(),
                "ghost".to_owned(),
            ],
        };

        let moved = f
            .service
            .update_project(&workspace.id, &api.id, &patch("/dev/erp"))
            .unwrap();
        let clash = f
            .service
            .update_project(&workspace.id, &api.id, &patch("/dev/shop/orders"))
            .unwrap_err();

        assert_eq!(moved.projects[1].id, api.id);
        assert_eq!(moved.projects[1].path, "/dev/erp");
        // Only a dependency on another project of the workspace is kept.
        assert_eq!(
            moved.projects[1].depends_on,
            [workspace.projects[2].id.clone()]
        );
        assert!(clash.is(ErrorCode::ProjectAlreadyInWorkspace));
    }

    #[test]
    fn the_projects_are_listed_with_the_state_of_their_folders() {
        let f = fixture();
        let workspace = shop(&f);
        // The folder of one project disappears after it was registered.
        let mut broken = f.store.saved.lock().unwrap().clone().unwrap();
        broken.workspaces[0].projects[2].path = "/dev/vanished".to_owned();
        *f.store.saved.lock().unwrap() = Some(broken);
        let restarted = build(&f.store);

        let info = restarted.service.projects(&workspace.id).unwrap();

        let seen: Vec<_> = info.iter().map(|p| (p.primary, p.exists)).collect();
        assert_eq!(seen, [(true, true), (false, true), (false, false)]);
        assert_eq!(info[0].technologies, ["Angular"]);
        assert!(info[2].technologies.is_empty());
    }

    #[test]
    fn projects_and_their_roles_survive_a_restart() {
        let f = fixture();
        let workspace = shop(&f);

        let restarted = build(&f.store);

        assert_eq!(
            restarted.service.get(&workspace.id).unwrap(),
            f.service.get(&workspace.id).unwrap()
        );
        assert_eq!(
            restarted.service.get(&workspace.id).unwrap().projects.len(),
            3
        );
    }

    #[test]
    fn updating_the_workspace_moves_only_the_primary_project() {
        let f = fixture();
        let workspace = shop(&f);

        let updated = f
            .service
            .update(&workspace.id, &input("Shop", "/dev/erp"))
            .unwrap();
        let clash = f
            .service
            .update(&workspace.id, &input("Shop", "/dev/shop/back"))
            .unwrap_err();

        assert_eq!(updated.projects[0].path, "/dev/erp");
        assert_eq!(updated.projects[0].id, workspace.projects[0].id);
        assert_eq!(updated.projects[1].path, "/dev/shop/back");
        assert!(clash.is(ErrorCode::ProjectAlreadyInWorkspace));
    }

    #[test]
    fn a_scope_is_resolved_against_the_workspace_and_a_stranger_is_refused() {
        let f = fixture();
        let workspace = shop(&f);
        let request = ScopeRequest {
            working_project_id: Some(workspace.projects[0].id.clone()),
            read_project_ids: vec![workspace.projects[1].id.clone()],
        };

        let scope = f
            .service
            .resolve_scope(&workspace.id, Some(&request))
            .unwrap();
        let default = f.service.resolve_scope(&workspace.id, None).unwrap();
        let stranger = f
            .service
            .resolve_scope(
                &workspace.id,
                Some(&ScopeRequest {
                    read_project_ids: vec!["ghost".to_owned()],
                    ..ScopeRequest::default()
                }),
            )
            .unwrap_err();

        assert_eq!(scope.others.len(), 1);
        assert!(!default.is_multi());
        assert!(stranger.is(ErrorCode::ScopeInvalid));
    }
}
