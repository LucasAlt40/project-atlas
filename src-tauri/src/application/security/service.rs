//! Policy lookup: which rules apply to this agent, in this workspace, running through this
//! runtime? Always derived from what Atlas stores, never from what a caller says.
//!
//! Precedence, widest to narrowest; each layer can only restrict the one before it
//! ([`SecurityPolicy::restrict`] takes the lower of every setting):
//!
//! 1. Atlas's global maximum ([`SecurityPolicy::global_maximum`])
//! 2. the workspace's policy
//! 3. the agent's permission profile
//! 4. the runtime's own limits ([`ToolAccess::ceiling`])

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;

use crate::application::config::ConfigRepository;
use crate::application::errors::{AppError, ErrorCode};
use crate::application::process::ExecutionScope;
use crate::application::runtimes::RuntimeRegistry;
use crate::domain::security::{Permission, PermissionProfile, Reason, SecurityPolicy, ToolAccess};
use crate::domain::worktree::WorktreeStatus;

/// Everything the guard needs to judge a process of one execution.
#[derive(Debug, Clone)]
pub struct ResolvedScope {
    pub project_root: PathBuf,
    /// Global maximum, workspace and profile: what the *agent* may be granted.
    pub agent_policy: SecurityPolicy,
    /// `agent_policy` narrowed by the runtime's own limits: what applies to commands requested
    /// on the agent's behalf.
    pub policy: SecurityPolicy,
    pub agent_name: String,
}

/// Port: finds the policy for an execution's workspace and agent.
pub trait PolicyResolver: Send + Sync {
    /// # Errors
    ///
    /// Fails (and so denies) if the workspace or agent in `scope` does not exist.
    fn resolve(&self, scope: &ExecutionScope) -> Result<ResolvedScope, Reason>;
}

pub struct SecurityService {
    config: Arc<ConfigRepository>,
}

impl SecurityService {
    pub fn new(config: Arc<ConfigRepository>) -> Self {
        Self { config }
    }

    /// The most a workspace can grant any agent: global maximum narrowed by the workspace's
    /// own policy. A hand-edited config cannot exceed the global maximum.
    fn workspace_ceiling(workspace_policy: &SecurityPolicy) -> SecurityPolicy {
        SecurityPolicy::global_maximum().restrict(workspace_policy)
    }

    /// The workspace's effective policy.
    ///
    /// # Errors
    ///
    /// Fails if the workspace does not exist.
    pub fn workspace_policy(
        &self,
        workspace_id: &str,
    ) -> Result<(String, SecurityPolicy), AppError> {
        let workspace = self
            .config
            .snapshot()
            .workspaces
            .into_iter()
            .find(|w| w.id == workspace_id)
            .ok_or_else(|| AppError::new(ErrorCode::WorkspaceNotFound))?;
        Ok((
            workspace.project_path,
            Self::workspace_ceiling(&workspace.security),
        ))
    }
}

impl PolicyResolver for SecurityService {
    fn resolve(&self, scope: &ExecutionScope) -> Result<ResolvedScope, Reason> {
        let config = self.config.snapshot();
        let workspace = config
            .workspaces
            .iter()
            .find(|w| w.id == scope.workspace_id)
            .ok_or(Reason::UnknownScope)?;
        let agent = config
            .agents
            .iter()
            .find(|a| a.id == scope.agent_id)
            .ok_or(Reason::UnknownScope)?;
        let profile = PermissionProfile::from_id_or_default(agent.permission_profile_id.as_deref());
        let agent_policy = Self::workspace_ceiling(&workspace.security).restrict(&profile.policy());
        let policy = agent_policy.restrict(&scope.runtime_access.ceiling());
        // An isolated execution is held to its worktree, not to the project's checkout: from
        // here on the worktree *is* the project. Where it is comes from what Atlas stored
        // for this execution, and only while it is running; anything else denies.
        let project_root = if scope.isolated {
            let worktree = config
                .worktrees
                .iter()
                .find(|w| {
                    w.execution_id == scope.execution_id
                        && w.workspace_id == scope.workspace_id
                        && w.agent_id == scope.agent_id
                        && w.status == WorktreeStatus::Active
                })
                .ok_or(Reason::UnknownScope)?;
            PathBuf::from(&worktree.working_dir)
        } else {
            PathBuf::from(&workspace.project_path)
        };
        Ok(ResolvedScope {
            project_root,
            agent_policy,
            policy,
            agent_name: agent.name.clone(),
        })
    }
}

/// How restrictive a workspace's policy is, as one word for the compact indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityLabel {
    /// Agents cannot change files or run commands.
    Secure,
    /// Agents may be granted development permissions.
    Developer,
}

impl SecurityLabel {
    fn of(policy: &SecurityPolicy) -> Self {
        if policy.filesystem.write == Permission::Denied
            && policy.processes.mode == Permission::Denied
        {
            Self::Secure
        } else {
            Self::Developer
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSecurity {
    pub workspace_id: String,
    pub project_path: String,
    pub policy: SecurityPolicy,
    pub label: SecurityLabel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentPermissions {
    pub workspace_id: String,
    pub agent_id: String,
    /// The agent's permission profile id.
    pub profile: String,
    pub available_profiles: Vec<String>,
    /// What the profile grants inside this workspace (global maximum, workspace and profile).
    pub policy: SecurityPolicy,
    /// What applies once the runtime's own limits are taken into account. Never wider than
    /// `policy`.
    pub effective: SecurityPolicy,
    pub runtime_access: ToolAccess,
    /// Capabilities of the runtime's tools that exceed `policy`: Atlas cannot restrict them
    /// (it can only report them). Empty when the runtime stays within the policy.
    pub unenforced: Vec<String>,
}

/// What the UI reads to show security. Read-only: the single way to change anything is
/// [`crate::application::agents::AgentService::set_permission_profile`].
pub struct SecurityOverview {
    security: Arc<SecurityService>,
    config: Arc<ConfigRepository>,
    runtimes: Arc<RuntimeRegistry>,
}

impl SecurityOverview {
    pub fn new(
        security: Arc<SecurityService>,
        config: Arc<ConfigRepository>,
        runtimes: Arc<RuntimeRegistry>,
    ) -> Self {
        Self {
            security,
            config,
            runtimes,
        }
    }

    /// # Errors
    ///
    /// Fails if the workspace does not exist.
    pub fn workspace(&self, workspace_id: &str) -> Result<WorkspaceSecurity, AppError> {
        let (project_path, policy) = self.security.workspace_policy(workspace_id)?;
        Ok(WorkspaceSecurity {
            workspace_id: workspace_id.to_owned(),
            project_path,
            label: SecurityLabel::of(&policy),
            policy,
        })
    }

    /// # Errors
    ///
    /// Fails if the workspace or the agent does not exist.
    pub fn agent(&self, workspace_id: &str, agent_id: &str) -> Result<AgentPermissions, AppError> {
        let agent = self
            .config
            .agents()
            .into_iter()
            .find(|a| a.id == agent_id)
            .ok_or_else(|| AppError::new(ErrorCode::AgentNotFound))?;
        let runtime_access = self
            .runtimes
            .find(&agent.runtime_id)
            .map_or(ToolAccess::NONE, |r| r.info().capabilities.tool_access);
        let scope = ExecutionScope {
            workspace_id: workspace_id.to_owned(),
            agent_id: agent_id.to_owned(),
            execution_id: String::new(),
            task_id: String::new(),
            runtime_access,
            isolated: false,
        };
        let resolved = self
            .security
            .resolve(&scope)
            .map_err(|_| AppError::new(ErrorCode::WorkspaceNotFound))?;
        Ok(AgentPermissions {
            workspace_id: workspace_id.to_owned(),
            agent_id: agent_id.to_owned(),
            profile: PermissionProfile::from_id_or_default(agent.permission_profile_id.as_deref())
                .id()
                .to_owned(),
            available_profiles: PermissionProfile::ALL
                .iter()
                .map(|p| p.id().to_owned())
                .collect(),
            unenforced: runtime_access
                .exceeding(&resolved.agent_policy)
                .into_iter()
                .map(str::to_owned)
                .collect(),
            policy: resolved.agent_policy,
            effective: resolved.policy,
            runtime_access,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::config::memory::MemoryStore;
    use crate::domain::agent::Agent;
    use crate::domain::workspace::{Workspace, WorkspaceLayout};

    fn repo(workspace_policy: SecurityPolicy, profile: Option<&str>) -> Arc<ConfigRepository> {
        let config = Arc::new(ConfigRepository::load(Box::<MemoryStore>::default()));
        config
            .modify(|c| {
                c.workspaces.push(Workspace {
                    id: "ws".to_owned(),
                    name: "W".to_owned(),
                    project_path: "/p".to_owned(),
                    description: None,
                    created_at: 1,
                    updated_at: 1,
                    layout: WorkspaceLayout {
                        rows: 2,
                        columns: 2,
                        agent_placements: vec![],
                    },
                    security: workspace_policy,
                });
                c.agents.push(Agent {
                    id: "a".to_owned(),
                    name: "A".to_owned(),
                    personality_id: "p".to_owned(),
                    runtime_id: "x".to_owned(),
                    model_id: "m".to_owned(),
                    instructions: String::new(),
                    permission_profile_id: profile.map(str::to_owned),
                    worktree_isolation: false,
                    result_contract: crate::domain::result_contract::ResultContract::default(),
                    created_at: 1,
                });
                Ok(())
            })
            .unwrap();
        config
    }

    fn scope(access: ToolAccess) -> ExecutionScope {
        ExecutionScope {
            workspace_id: "ws".to_owned(),
            agent_id: "a".to_owned(),
            execution_id: "e".to_owned(),
            task_id: "t".to_owned(),
            runtime_access: access,
            isolated: false,
        }
    }

    #[test]
    fn a_stored_policy_beyond_the_global_maximum_is_clamped() {
        let mut everything = SecurityPolicy::developer();
        everything.network.mode = Permission::Allowed;
        everything.git.destructive = Permission::Allowed;
        let service = SecurityService::new(repo(everything, Some("developer")));

        let (_, policy) = service.workspace_policy("ws").unwrap();

        assert_eq!(policy.network.mode, Permission::ApprovalRequired);
        assert_eq!(policy.git.destructive, Permission::ApprovalRequired);
    }

    #[test]
    fn the_layers_only_narrow() {
        let open = ToolAccess {
            filesystem_write: true,
            process_execution: true,
            network: true,
        };
        let developer = SecurityService::new(repo(SecurityPolicy::developer(), Some("developer")));
        let read_only = SecurityService::new(repo(SecurityPolicy::developer(), Some("read_only")));
        let locked = SecurityService::new(repo(SecurityPolicy::read_only(), Some("developer")));

        let dev = developer.resolve(&scope(open)).unwrap();
        assert_eq!(dev.policy, SecurityPolicy::developer());
        // A runtime that cannot write narrows a developer agent...
        let limited = developer.resolve(&scope(ToolAccess::NONE)).unwrap();
        assert_eq!(limited.agent_policy, SecurityPolicy::developer());
        assert_eq!(limited.policy.filesystem.write, Permission::Denied);
        // ...a read-only profile narrows a developer workspace...
        assert_eq!(
            read_only.resolve(&scope(open)).unwrap().policy,
            SecurityPolicy::read_only()
        );
        // ...and a developer profile cannot widen a read-only workspace.
        assert_eq!(
            locked.resolve(&scope(open)).unwrap().policy,
            SecurityPolicy::read_only()
        );
    }

    #[test]
    fn unknown_workspaces_and_agents_do_not_resolve() {
        let service = SecurityService::new(repo(SecurityPolicy::developer(), None));
        let mut s = scope(ToolAccess::NONE);
        s.workspace_id = "other".to_owned();
        assert_eq!(service.resolve(&s).unwrap_err(), Reason::UnknownScope);
        let mut s = scope(ToolAccess::NONE);
        s.agent_id = "ghost".to_owned();
        assert_eq!(service.resolve(&s).unwrap_err(), Reason::UnknownScope);
    }

    #[test]
    fn the_indicator_says_secure_only_when_nothing_can_change_or_run() {
        assert_eq!(
            SecurityLabel::of(&SecurityPolicy::read_only()),
            SecurityLabel::Secure
        );
        assert_eq!(
            SecurityLabel::of(&SecurityPolicy::developer()),
            SecurityLabel::Developer
        );
    }
}
