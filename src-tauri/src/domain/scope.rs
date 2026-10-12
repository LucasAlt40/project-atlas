//! Which projects of a workspace an execution works in and which it may only consult.
//!
//! A scope is what the caller asks for ([`ScopeRequest`]) resolved against the workspace
//! ([`ProjectScope`]). Without a request the scope is the primary project alone, exactly what an
//! execution was before a workspace could have several: nothing else is reachable.
//!
//! Exactly one project is *worked in*: the process runs there and, when the agent may edit, its
//! edits go to that project's isolated worktree (an execution has one). The others are consulted.
//! Changing several projects is several executions, each with its own worktree, review and merge.

use serde::{Deserialize, Serialize};

use super::workspace::{Workspace, WorkspaceProject};

/// What a caller asks for. Every field is optional: the empty request is the default scope.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeRequest {
    /// The project the agent works in. The primary project when unset.
    #[serde(default)]
    pub working_project_id: Option<String>,
    /// Other projects the agent may consult. Never changed.
    #[serde(default)]
    pub read_project_ids: Vec<String>,
}

/// A request names a project the workspace does not have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownProject(pub String);

/// The resolved scope of one execution.
///
/// Anything not listed here is not part of the execution: its context is not loaded and it is
/// not mentioned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectScope {
    pub working: WorkspaceProject,
    /// Consulted only, in the order the workspace lists its projects.
    pub others: Vec<WorkspaceProject>,
}

impl ProjectScope {
    /// # Errors
    ///
    /// [`UnknownProject`] for the first id of the request the workspace does not have, or when
    /// the workspace has no project at all.
    pub fn resolve(
        workspace: &Workspace,
        request: Option<&ScopeRequest>,
    ) -> Result<Self, UnknownProject> {
        let default = ScopeRequest::default();
        let request = request.unwrap_or(&default);
        let working = match request.working_project_id.as_deref() {
            Some(id) => workspace
                .project(id)
                .ok_or_else(|| UnknownProject(id.to_owned()))?,
            None => workspace
                .primary_project()
                .ok_or_else(|| UnknownProject(String::new()))?,
        };
        if let Some(id) = request
            .read_project_ids
            .iter()
            .find(|id| workspace.project(id).is_none())
        {
            return Err(UnknownProject(id.clone()));
        }
        let others = workspace
            .projects
            .iter()
            .filter(|p| p.id != working.id && request.read_project_ids.contains(&p.id))
            .cloned()
            .collect();
        Ok(Self {
            working: working.clone(),
            others,
        })
    }

    pub fn is_multi(&self) -> bool {
        !self.others.is_empty()
    }
}

#[cfg(test)]
#[allow(clippy::assert_is_empty)]
mod tests {
    use super::*;
    use crate::domain::security::SecurityPolicy;
    use crate::domain::workspace::WorkspaceLayout;

    fn project(id: &str) -> WorkspaceProject {
        WorkspaceProject {
            id: id.to_owned(),
            name: id.to_uppercase(),
            path: format!("/dev/{id}"),
            role: None,
            depends_on: Vec::new(),
        }
    }

    fn workspace() -> Workspace {
        Workspace {
            id: "ws".to_owned(),
            name: "E-commerce".to_owned(),
            project_path: "/dev/front".to_owned(),
            projects: vec![project("front"), project("back"), project("orders")],
            description: None,
            created_at: 0,
            updated_at: 0,
            layout: WorkspaceLayout {
                rows: 2,
                columns: 2,
                agent_placements: Vec::new(),
            },
            security: SecurityPolicy::default(),
        }
    }

    fn ids(scope: &ProjectScope) -> Vec<&str> {
        scope.others.iter().map(|p| p.id.as_str()).collect()
    }

    #[test]
    fn without_a_request_the_scope_is_the_primary_project_alone() {
        let scope = ProjectScope::resolve(&workspace(), None).unwrap();

        assert_eq!(scope.working.id, "front");
        assert!(!scope.is_multi());
    }

    #[test]
    fn a_project_can_be_the_working_one_without_being_the_primary() {
        let request = ScopeRequest {
            working_project_id: Some("back".to_owned()),
            ..ScopeRequest::default()
        };

        let scope = ProjectScope::resolve(&workspace(), Some(&request)).unwrap();

        assert_eq!(scope.working.id, "back");
        assert!(scope.others.is_empty());
    }

    #[test]
    fn consulted_projects_come_in_the_workspaces_order_whatever_the_requests() {
        let request = ScopeRequest {
            working_project_id: Some("front".to_owned()),
            read_project_ids: vec!["orders".to_owned(), "back".to_owned()],
        };

        let scope = ProjectScope::resolve(&workspace(), Some(&request)).unwrap();

        assert_eq!(ids(&scope), ["back", "orders"]);
        assert!(scope.is_multi());
    }

    #[test]
    fn listing_the_working_project_again_adds_nothing() {
        let request = ScopeRequest {
            read_project_ids: vec!["front".to_owned()],
            ..ScopeRequest::default()
        };

        let scope = ProjectScope::resolve(&workspace(), Some(&request)).unwrap();

        assert!(scope.others.is_empty());
    }

    #[test]
    fn a_project_the_workspace_does_not_have_is_refused() {
        let request = ScopeRequest {
            read_project_ids: vec!["ghost".to_owned()],
            ..ScopeRequest::default()
        };
        let as_working = ScopeRequest {
            working_project_id: Some("ghost".to_owned()),
            ..ScopeRequest::default()
        };

        assert_eq!(
            ProjectScope::resolve(&workspace(), Some(&request)),
            Err(UnknownProject("ghost".to_owned()))
        );
        assert_eq!(
            ProjectScope::resolve(&workspace(), Some(&as_working)),
            Err(UnknownProject("ghost".to_owned()))
        );
    }

    #[test]
    fn a_request_written_by_the_webview_without_fields_is_the_default() {
        let request: ScopeRequest = serde_json::from_str("{}").unwrap();

        assert_eq!(request, ScopeRequest::default());
    }
}
