//! Starting a workflow from an approved spec, and following it afterwards (ADR 0035). The
//! workflow is the one the person chose; what this adds is what the run is given of the spec (its
//! revisions, a bounded digest, its task list) and the link back, so the spec knows what was built
//! from it and the run knows what it was told.

use std::sync::Arc;

use super::progress::{of_link, ImplementationProgress};
use super::service::SpecService;
use crate::application::errors::{AppError, ErrorCode};
use crate::application::workflow::service::WorkflowService;
use crate::domain::scope::ScopeRequest;
use crate::domain::spec::ImplementationLink;
use crate::domain::workflow::{SpecIntegrationRef, SpecLink, SpecTaskRef, WorkflowExecution};

pub struct SpecImplementation {
    specs: Arc<SpecService>,
    workflows: Arc<WorkflowService>,
}

impl SpecImplementation {
    pub fn new(specs: Arc<SpecService>, workflows: Arc<WorkflowService>) -> Self {
        Self { specs, workflows }
    }

    /// Starts `workflow_id` on the spec. An empty `task` is the spec's own ("implement … following
    /// its task list"). Only a ready spec of the workflow's workspace can be started from.
    ///
    /// # Errors
    ///
    /// When the workflow or spec is unknown, the spec is not ready or not this workspace's, the
    /// scope names another project than the spec's, or the workflow cannot start.
    pub fn start(
        &self,
        spec_id: &str,
        workflow_id: &str,
        task: &str,
        auto_approve_plans: bool,
        scope: Option<ScopeRequest>,
    ) -> Result<WorkflowExecution, AppError> {
        let workflow = self
            .workflows
            .get(workflow_id)
            .ok_or_else(|| AppError::new(ErrorCode::WorkflowNotFound))?;
        let context = self
            .specs
            .implementation_context(spec_id, &workflow.workspace_id)?;
        // The run works in the project the spec is for, and in no other.
        let scope = match scope {
            Some(requested) => {
                let working = requested.working_project_id.as_deref();
                if working.is_some_and(|id| id != context.project_id) {
                    return Err(
                        AppError::new(ErrorCode::SpecInvalid).with("reason", "project_mismatch")
                    );
                }
                Some(ScopeRequest {
                    working_project_id: Some(context.project_id.clone()),
                    ..requested
                })
            }
            None => Some(ScopeRequest {
                working_project_id: Some(context.project_id.clone()),
                read_project_ids: Vec::new(),
            }),
        };
        let task = if task.trim().is_empty() {
            context.task.clone()
        } else {
            task.trim().to_owned()
        };
        let link = SpecLink {
            spec_id: context.spec_id.clone(),
            title: context.title.clone(),
            change_id: context.change_id.clone(),
            revisions: context.revisions.clone(),
            digest: context.digest.clone(),
            tasks: context
                .tasks
                .iter()
                .map(|t| SpecTaskRef {
                    id: t.id.clone(),
                    text: t.text.clone(),
                })
                .collect(),
            published_root: context.published_root.clone(),
            integrations: context
                .integrations
                .iter()
                .map(|i| SpecIntegrationRef {
                    name: i.name.clone(),
                    tools: i.tools.clone(),
                })
                .collect(),
        };
        let execution =
            self.workflows
                .start_for_spec(workflow_id, &task, auto_approve_plans, scope, link)?;
        let linked = self.specs.link_implementation(
            spec_id,
            ImplementationLink {
                workflow_execution_id: execution.id.clone(),
                workflow_id: workflow.id.clone(),
                workflow_name: workflow.name.clone(),
                revisions: context.revisions,
                started_at: execution.started_at,
            },
        );
        if let Err(error) = linked {
            // The run is already going and has what it was given; the spec's record is behind.
            eprintln!(
                "the run {} could not be linked to spec {spec_id}: {error}",
                execution.id
            );
        }
        Ok(execution)
    }

    /// What each run started from the spec has got through.
    ///
    /// # Errors
    ///
    /// When there is no such spec.
    pub fn progress(&self, spec_id: &str) -> Result<Vec<ImplementationProgress>, AppError> {
        let spec = self
            .specs
            .get(spec_id)
            .ok_or_else(|| AppError::new(ErrorCode::SpecNotFound))?;
        Ok(spec
            .implementations
            .iter()
            .rev()
            .map(|link| {
                let execution = self.workflows.execution(&link.workflow_execution_id);
                of_link(&spec, link, execution.as_ref())
            })
            .collect())
    }
}
