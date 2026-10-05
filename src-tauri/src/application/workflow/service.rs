//! Use case: workflow definitions and the record of their runs. Definitions and runs live in the
//! same config store as everything else (atomic writes, no new database) but in separate lists,
//! and a run holds a snapshot of the definition it started from.

use std::sync::Arc;

use serde::Deserialize;

use super::engine::WorkflowEngine;
use super::repair::{self, RepairChoice, RepairProposal};
use super::templates::{self, AgentChoice, Role, TemplateInfo};
use super::validation::{validate, AgentCatalog, ValidationReport};
use crate::application::agents::AgentService;
use crate::application::config::ConfigRepository;
use crate::application::errors::{AppError, ErrorCode};
use crate::application::support::{new_id, now_ms};
use crate::application::workspace::WorkspaceService;
use crate::domain::result_contract::ResultContract;
use crate::domain::workflow::{
    NodeState, RouteRepair, Viewport, Workflow, WorkflowEdge, WorkflowExecution,
    WorkflowExecutionStatus, WorkflowMode, WorkflowNode, WorkflowStatus,
};

/// Finished runs kept per workspace; the oldest go first.
const KEPT_FINISHED_RUNS: usize = 40;
const MAX_NAME_LEN: usize = 80;

/// A workflow as the user composes it, before Atlas gives it an id.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewWorkflow {
    pub workspace_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub mode: WorkflowMode,
    #[serde(default)]
    pub nodes: Vec<WorkflowNode>,
    #[serde(default)]
    pub edges: Vec<WorkflowEdge>,
    #[serde(default)]
    pub viewport: Option<Viewport>,
}

pub struct FromTemplate {
    pub workflow: Workflow,
    pub missing_roles: Vec<Role>,
}

pub struct WorkflowService {
    config: Arc<ConfigRepository>,
    agents: Arc<AgentService>,
    workspaces: Arc<WorkspaceService>,
}

struct Catalog<'a>(&'a AgentService);

impl AgentCatalog for Catalog<'_> {
    fn agent_exists(&self, id: &str) -> bool {
        self.0.find(id).is_some()
    }

    fn declared_outcomes(&self, id: &str) -> Vec<String> {
        self.0.find(id).map_or_else(Vec::new, |agent| {
            agent
                .result_contract
                .outcomes
                .into_iter()
                .map(|o| o.id)
                .collect()
        })
    }

    fn agent_name(&self, id: &str) -> String {
        self.0.find(id).map_or_else(|| id.to_owned(), |a| a.name)
    }
}

fn clean_name(name: &str) -> Result<String, AppError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::new(ErrorCode::NameRequired));
    }
    if name.chars().count() > MAX_NAME_LEN {
        return Err(AppError::new(ErrorCode::NameTooLong).with("max", MAX_NAME_LEN.to_string()));
    }
    Ok(name.to_owned())
}

impl WorkflowService {
    pub fn new(
        config: Arc<ConfigRepository>,
        agents: Arc<AgentService>,
        workspaces: Arc<WorkspaceService>,
    ) -> Self {
        Self {
            config,
            agents,
            workspaces,
        }
    }

    // ---- definitions -------------------------------------------------------------------------

    pub fn list(&self, workspace_id: &str) -> Vec<Workflow> {
        self.config.read(|c| {
            c.workflows
                .iter()
                .filter(|w| w.workspace_id == workspace_id)
                .cloned()
                .collect()
        })
    }

    pub fn get(&self, id: &str) -> Option<Workflow> {
        self.config
            .read(|c| c.workflows.iter().find(|w| w.id == id).cloned())
    }

    #[allow(clippy::unused_self)] // the service is how the UI reaches the registry
    pub fn templates(&self) -> Vec<TemplateInfo> {
        templates::list()
    }

    /// The template Automatic mode proposes for a task.
    #[allow(clippy::unused_self)]
    pub fn select_template(&self, task: &str) -> &'static str {
        templates::select_for_task(task)
    }

    pub fn validate(&self, workflow: &Workflow) -> ValidationReport {
        validate(
            workflow,
            Some(&workflow.workspace_id),
            &Catalog(&self.agents),
        )
    }

    /// The agents of the workflow that work in an isolated worktree (the agent's own setting,
    /// which a node never overrides), without repeats.
    pub fn isolated_agents(&self, workflow: &Workflow) -> Vec<String> {
        workflow
            .agent_ids()
            .into_iter()
            .filter(|id| self.agents.find(id).is_some_and(|a| a.worktree_isolation))
            .map(str::to_owned)
            .collect()
    }

    /// The personality of an agent, to tell what kind of step it does.
    pub fn personality_of(&self, agent_id: &str) -> Option<String> {
        self.agents.find(agent_id).map(|a| a.personality_id)
    }

    /// What the agent promises to say at the end of a step. A general contract when the agent
    /// is gone: no outcome is then required, and the run says so itself.
    pub fn contract_of(&self, agent_id: &str) -> ResultContract {
        self.agents
            .find(agent_id)
            .map(|a| a.result_contract)
            .unwrap_or_default()
    }

    fn require_workspace(&self, workspace_id: &str) -> Result<(), AppError> {
        self.workspaces
            .get(workspace_id)
            .map(|_| ())
            .ok_or_else(|| AppError::new(ErrorCode::WorkspaceNotFound))
    }

    /// # Errors
    ///
    /// Fails if the workspace is unknown, the name is invalid or saving fails.
    pub fn create(&self, new: NewWorkflow) -> Result<Workflow, AppError> {
        self.require_workspace(&new.workspace_id)?;
        let now = now_ms();
        let mut workflow = Workflow {
            id: new_id("workflow"),
            workspace_id: new.workspace_id,
            name: clean_name(&new.name)?,
            description: new.description.trim().to_owned(),
            mode: new.mode,
            version: 1,
            status: WorkflowStatus::Draft,
            template_id: None,
            nodes: new.nodes,
            edges: new.edges,
            viewport: new.viewport,
            route_repairs: Vec::new(),
            created_at: now,
            updated_at: now,
        };
        workflow.status = self.status_of(&workflow);
        self.store(workflow.clone())?;
        Ok(workflow)
    }

    /// Builds a workflow from a template, using the workspace's agents by personality.
    ///
    /// # Errors
    ///
    /// Fails if the workspace or template is unknown, or saving fails.
    pub fn create_from_template(
        &self,
        workspace_id: &str,
        template_id: &str,
        name: Option<&str>,
        mode: WorkflowMode,
    ) -> Result<FromTemplate, AppError> {
        let workspace = self
            .workspaces
            .get(workspace_id)
            .ok_or_else(|| AppError::new(ErrorCode::WorkspaceNotFound))?;
        let choices: Vec<AgentChoice> = self
            .agents
            .list()
            .into_iter()
            .map(|agent| AgentChoice {
                placed: workspace.layout.is_placed(&agent.id),
                id: agent.id,
                personality_id: agent.personality_id,
                outcomes: agent
                    .result_contract
                    .outcomes
                    .iter()
                    .map(|o| o.id.clone())
                    .collect(),
            })
            .collect();
        let name = name.map(clean_name).transpose()?;
        let built = templates::instantiate(
            template_id,
            workspace_id,
            &new_id("workflow"),
            name.as_deref(),
            &choices,
            mode,
            now_ms(),
        )
        .ok_or_else(|| AppError::new(ErrorCode::WorkflowTemplateNotFound))?;
        let mut workflow = built.workflow;
        workflow.status = self.status_of(&workflow);
        self.store(workflow.clone())?;
        Ok(FromTemplate {
            workflow,
            missing_roles: built.missing_roles,
        })
    }

    fn status_of(&self, workflow: &Workflow) -> WorkflowStatus {
        if self.validate(workflow).valid {
            WorkflowStatus::Ready
        } else {
            WorkflowStatus::Draft
        }
    }

    fn store(&self, workflow: Workflow) -> Result<(), AppError> {
        self.config.modify(|config| {
            config.workflows.push(workflow);
            Ok(())
        })
    }

    /// Replaces a definition. A structural change (nodes, edges, policies) makes a new version;
    /// moving nodes around, the viewport and the name do not. While the workflow has a run in
    /// progress it cannot be edited: the run keeps its own snapshot, and the user waits or saves
    /// a copy.
    ///
    /// # Errors
    ///
    /// Fails if the workflow is unknown, has a run in progress, or saving fails.
    pub fn update(&self, workflow: Workflow) -> Result<Workflow, AppError> {
        self.save_definition(workflow, Vec::new())
    }

    fn save_definition(
        &self,
        mut workflow: Workflow,
        repairs: Vec<RouteRepair>,
    ) -> Result<Workflow, AppError> {
        let current = self
            .get(&workflow.id)
            .ok_or_else(|| AppError::new(ErrorCode::WorkflowNotFound))?;
        if self.active_execution(&current.id).is_some() {
            return Err(AppError::new(ErrorCode::WorkflowRunning));
        }
        workflow.name = clean_name(&workflow.name)?;
        // What the caller may not change.
        workflow.workspace_id.clone_from(&current.workspace_id);
        workflow.created_at = current.created_at;
        workflow.template_id.clone_from(&current.template_id);
        workflow.version = if workflow.same_structure(&current) {
            current.version
        } else {
            current.version + 1
        };
        // The history of repairs is only ever added to, by a confirmed repair.
        workflow.route_repairs.clone_from(&current.route_repairs);
        workflow
            .route_repairs
            .extend(repairs.into_iter().map(|mut r| {
                r.version = workflow.version;
                r
            }));
        workflow.updated_at = now_ms();
        workflow.status = self.status_of(&workflow);
        let saved = workflow.clone();
        self.config.modify(|config| {
            let slot = config
                .workflows
                .iter_mut()
                .find(|w| w.id == saved.id)
                .ok_or_else(|| AppError::new(ErrorCode::WorkflowNotFound))?;
            *slot = saved.clone();
            Ok(())
        })?;
        Ok(workflow)
    }

    /// The repairs Atlas can propose for the routes that cannot match their agent's contract.
    /// Nothing is changed.
    pub fn route_repairs(&self, workflow: &Workflow) -> Vec<RepairProposal> {
        repair::propose(workflow, &Catalog(&self.agents))
    }

    /// Applies the repairs the user confirmed, as a new version of the workflow with a record of
    /// each: what the edge tested before, what it tests now.
    ///
    /// # Errors
    ///
    /// Fails if the workflow is unknown or running, a choice is stale or names an outcome the
    /// agent does not declare (nothing is then changed), or saving fails.
    pub fn repair_routes(
        &self,
        workflow_id: &str,
        choices: &[RepairChoice],
    ) -> Result<Workflow, AppError> {
        let mut workflow = self
            .get(workflow_id)
            .ok_or_else(|| AppError::new(ErrorCode::WorkflowNotFound))?;
        let records = repair::apply(&mut workflow, &Catalog(&self.agents), choices, now_ms())
            .map_err(|_| AppError::new(ErrorCode::WorkflowInvalid))?;
        self.save_definition(workflow, records)
    }

    /// # Errors
    ///
    /// Fails if the workflow is unknown or has a run in progress.
    pub fn delete(&self, id: &str) -> Result<(), AppError> {
        if self.get(id).is_none() {
            return Err(AppError::new(ErrorCode::WorkflowNotFound));
        }
        if self.active_execution(id).is_some() {
            return Err(AppError::new(ErrorCode::WorkflowRunning));
        }
        self.config.modify(|config| {
            config.workflows.retain(|w| w.id != id);
            Ok(())
        })
    }

    /// Deletes every workflow and run of the workspace.
    ///
    /// # Errors
    ///
    /// Fails if saving fails.
    pub fn discard_workspace(&self, workspace_id: &str) -> Result<(), AppError> {
        self.config.modify(|config| {
            config.workflows.retain(|w| w.workspace_id != workspace_id);
            config
                .workflow_executions
                .retain(|e| e.workspace_id != workspace_id);
            Ok(())
        })
    }

    // ---- runs --------------------------------------------------------------------------------

    /// Creates a run of the workflow from a snapshot of its current version. The run is saved
    /// but not started: the orchestrator drives it.
    ///
    /// # Errors
    ///
    /// Fails if the workflow is unknown, invalid, already running, or the task is empty.
    pub fn start(&self, workflow_id: &str, task: &str) -> Result<WorkflowExecution, AppError> {
        self.start_checked(workflow_id, task, true)
    }

    /// Starts without validating: only tests use it, to watch the engine on a definition
    /// validation would have refused (a missing route must still end the run deterministically).
    #[cfg(test)]
    pub fn start_unvalidated(
        &self,
        workflow_id: &str,
        task: &str,
    ) -> Result<WorkflowExecution, AppError> {
        self.start_checked(workflow_id, task, false)
    }

    fn start_checked(
        &self,
        workflow_id: &str,
        task: &str,
        validated: bool,
    ) -> Result<WorkflowExecution, AppError> {
        let workflow = self
            .get(workflow_id)
            .ok_or_else(|| AppError::new(ErrorCode::WorkflowNotFound))?;
        let task = task.trim();
        if task.is_empty() {
            return Err(AppError::new(ErrorCode::MessageEmpty));
        }
        if validated && !self.validate(&workflow).valid {
            return Err(AppError::new(ErrorCode::WorkflowInvalid));
        }
        let now = now_ms();
        let execution = WorkflowExecution::new(new_id("wfx"), workflow, task.to_owned(), now);
        let stored = execution.clone();
        self.config.modify(|config| {
            // One run at a time per workflow: a second would fight the first for its agents.
            if config
                .workflow_executions
                .iter()
                .any(|e| e.workflow_id == stored.workflow_id && e.status.is_active())
            {
                return Err(AppError::new(ErrorCode::WorkflowRunning));
            }
            config.workflow_executions.push(stored.clone());
            prune(&mut config.workflow_executions, &stored.workspace_id);
            Ok(())
        })?;
        Ok(execution)
    }

    /// Lets a failed run go on under the workflow as it stands now, when the user has edited it
    /// since the run began (a route added, a loop limit raised…). Only when nothing the run has
    /// done is taken away: every step it knows is still there, of the same kind, and the new
    /// definition is valid. Otherwise the run keeps the snapshot it started from.
    pub fn adopt_current_definition(&self, exec: &mut WorkflowExecution) -> bool {
        let Some(current) = self.get(&exec.workflow_id) else {
            return false;
        };
        if current.version == exec.workflow_version || !self.validate(&current).valid {
            return false;
        }
        let compatible =
            exec.nodes
                .keys()
                .all(|id| match (exec.workflow.node(id), current.node(id)) {
                    (Some(old), Some(new)) => {
                        std::mem::discriminant(&old.kind) == std::mem::discriminant(&new.kind)
                    }
                    _ => false,
                });
        if !compatible {
            return false;
        }
        for node in &current.nodes {
            exec.nodes
                .entry(node.id.clone())
                .or_insert_with(NodeState::pending);
        }
        exec.workflow_version = current.version;
        exec.workflow = current;
        true
    }

    pub fn execution(&self, id: &str) -> Option<WorkflowExecution> {
        self.config
            .read(|c| c.workflow_executions.iter().find(|e| e.id == id).cloned())
    }

    /// Runs, newest first, of a workspace and optionally of one workflow.
    pub fn executions(
        &self,
        workspace_id: &str,
        workflow_id: Option<&str>,
    ) -> Vec<WorkflowExecution> {
        self.config.read(|c| {
            c.workflow_executions
                .iter()
                .rev()
                .filter(|e| e.workspace_id == workspace_id)
                .filter(|e| workflow_id.is_none_or(|id| e.workflow_id == id))
                .cloned()
                .collect()
        })
    }

    /// Whether any run in the workspace has not ended (running, paused or interrupted).
    pub fn has_active_execution(&self, workspace_id: &str) -> bool {
        self.config.read(|c| {
            c.workflow_executions.iter().any(|e| {
                e.workspace_id == workspace_id
                    && (e.status.is_active() || e.status == WorkflowExecutionStatus::Interrupted)
            })
        })
    }

    /// The run of the workflow that has not ended (running, paused or interrupted).
    pub fn active_execution(&self, workflow_id: &str) -> Option<WorkflowExecution> {
        self.config.read(|c| {
            c.workflow_executions
                .iter()
                .find(|e| {
                    e.workflow_id == workflow_id
                        && (e.status.is_active()
                            || e.status == WorkflowExecutionStatus::Interrupted)
                })
                .cloned()
        })
    }

    /// Saves the state of a run. Called by the orchestrator at every transition.
    ///
    /// # Errors
    ///
    /// Fails if the run is unknown or saving fails.
    pub fn save_execution(&self, execution: &WorkflowExecution) -> Result<(), AppError> {
        self.config.modify(|config| {
            let slot = config
                .workflow_executions
                .iter_mut()
                .find(|e| e.id == execution.id)
                .ok_or_else(|| AppError::new(ErrorCode::WorkflowExecutionNotFound))?;
            *slot = execution.clone();
            Ok(())
        })
    }

    /// Marks every run the previous session never saw end as interrupted. Nothing is resumed
    /// automatically and nothing is called finished: the user decides.
    pub fn recover_interrupted(&self) {
        let result = self.config.modify(|config| {
            let at = now_ms();
            for execution in &mut config.workflow_executions {
                if execution.status.is_active() {
                    let workflow = execution.workflow.clone();
                    WorkflowEngine::new(&workflow).interrupt(execution, at);
                }
            }
            Ok(())
        });
        if let Err(error) = result {
            eprintln!("could not mark interrupted workflow runs: {error}");
        }
    }
}

fn prune(executions: &mut Vec<WorkflowExecution>, workspace_id: &str) {
    let finished: Vec<usize> = executions
        .iter()
        .enumerate()
        .filter(|(_, e)| e.workspace_id == workspace_id && e.status.is_final())
        .map(|(i, _)| i)
        .collect();
    if finished.len() > KEPT_FINISHED_RUNS {
        let drop: std::collections::BTreeSet<usize> = finished
            .into_iter()
            .take(finished_len_excess(executions, workspace_id))
            .collect();
        let mut index = 0;
        executions.retain(|_| {
            let keep = !drop.contains(&index);
            index += 1;
            keep
        });
    }
}

fn finished_len_excess(executions: &[WorkflowExecution], workspace_id: &str) -> usize {
    executions
        .iter()
        .filter(|e| e.workspace_id == workspace_id && e.status.is_final())
        .count()
        .saturating_sub(KEPT_FINISHED_RUNS)
}
