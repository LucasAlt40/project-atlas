//! Starting a workflow from an approved spec, and following it afterwards (ADR 0035). The
//! workflow is the one the person chose; what this adds is what the run is given of the spec (its
//! revisions, a bounded digest, its task list) and the link back, so the spec knows what was built
//! from it and the run knows what it was told.

use std::sync::Arc;

use super::progress::{of_link, ImplementationProgress, TaskState};
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
    /// The run works in the scope's project (the spec's own when none is asked for) and consults
    /// the workspace's others. With `next_project_ids` the spec is implemented in each of them
    /// too, one run after the other (ADR 0043), every run given the same spec.
    ///
    /// # Errors
    ///
    /// When the workflow or spec is unknown, the spec is not ready or not this workspace's, the
    /// scope names a project the workspace does not have, or the workflow cannot start.
    pub fn start(
        &self,
        spec_id: &str,
        workflow_id: &str,
        task: &str,
        auto_approve_plans: bool,
        scope: Option<ScopeRequest>,
        next_project_ids: &[String],
    ) -> Result<WorkflowExecution, AppError> {
        let workflow = self
            .workflows
            .get(workflow_id)
            .ok_or_else(|| AppError::new(ErrorCode::WorkflowNotFound))?;
        let context = self
            .specs
            .implementation_context(spec_id, &workflow.workspace_id)?;
        let working = scope
            .as_ref()
            .and_then(|s| s.working_project_id.clone())
            .unwrap_or_else(|| context.project_id.clone());
        // Whatever it works in, the run may read every other project of the workspace: the spec
        // was written knowing all of them.
        let mut read = scope.map(|s| s.read_project_ids).unwrap_or_default();
        for id in context
            .sibling_project_ids
            .iter()
            .chain(std::iter::once(&context.project_id))
        {
            if *id != working && !read.contains(id) {
                read.push(id.clone());
            }
        }
        let scope = Some(ScopeRequest {
            working_project_id: Some(working),
            read_project_ids: read,
        });
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
        let execution = self.workflows.start_chain(
            workflow_id,
            &task,
            auto_approve_plans,
            scope,
            Some(link),
            next_project_ids,
        )?;
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
            log::warn!(
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
        let mut out = Vec::new();
        for link in spec.implementations.iter().rev() {
            let execution = self.workflows.execution(&link.workflow_execution_id);
            let chain = execution
                .as_ref()
                .and_then(|e| e.chain.as_ref())
                .map(|c| c.id.clone());
            match chain {
                // A spec implemented in several projects: one run each, all linked through the
                // first. The later ones are shown before it, newest first like the rest.
                Some(chain_id) => {
                    let runs: Vec<ImplementationProgress> = self
                        .workflows
                        .chain_runs(&chain_id)
                        .iter()
                        .rev()
                        .map(|run| {
                            let run_link = ImplementationLink {
                                workflow_execution_id: run.id.clone(),
                                started_at: run.started_at,
                                ..link.clone()
                            };
                            of_link(&spec, &run_link, Some(run))
                        })
                        .collect();
                    out.extend(merged_over_chain(runs));
                }
                None => out.push(of_link(&spec, link, execution.as_ref())),
            }
        }
        Ok(out)
    }
}

/// Each run of a chain does the tasks of its own project, so a task is as far as the furthest any
/// run of the chain got with it: every run of the chain shows the spec's progress over all of them.
fn merged_over_chain(mut runs: Vec<ImplementationProgress>) -> Vec<ImplementationProgress> {
    fn rank(state: TaskState) -> u8 {
        match state {
            TaskState::Pending => 0,
            TaskState::Reported => 1,
            TaskState::Confirmed => 2,
        }
    }
    let mut best: std::collections::BTreeMap<String, TaskState> = std::collections::BTreeMap::new();
    for run in &runs {
        for task in &run.tasks {
            let entry = best.entry(task.id.clone()).or_insert(TaskState::Pending);
            if rank(task.state) > rank(*entry) {
                *entry = task.state;
            }
        }
    }
    for run in &mut runs {
        for task in &mut run.tasks {
            if let Some(state) = best.get(&task.id) {
                task.state = *state;
            }
        }
        let count = |f: &dyn Fn(TaskState) -> bool| {
            u32::try_from(run.tasks.iter().filter(|t| f(t.state)).count()).unwrap_or(u32::MAX)
        };
        run.reported = count(&|s| s != TaskState::Pending);
        run.confirmed = count(&|s| s == TaskState::Confirmed);
    }
    runs
}
