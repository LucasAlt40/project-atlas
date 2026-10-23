//! Use case: workflow definitions and the record of their runs. Definitions and runs live in the
//! same config store as everything else (atomic writes, no new database) but in separate lists,
//! and a run holds a snapshot of the definition it started from.

use std::sync::Arc;

use serde::Deserialize;

use super::engine::WorkflowEngine;
use super::repair::{self, RepairChoice, RepairProposal};
use super::template_tools::{self, ToolPlanItem};
use super::templates::{self, AgentChoice, Role, TemplateInfo};
use super::validation::{validate, AgentCatalog, ValidationReport};
use crate::application::agents::AgentService;
use crate::application::config::ConfigRepository;
use crate::application::errors::{AppError, ErrorCode};
use crate::application::support::{new_id, now_ms};
use crate::application::workspace::WorkspaceService;
use crate::domain::result_contract::ResultContract;
use crate::domain::scope::ScopeRequest;
use crate::domain::workflow::{
    ChainProject, CustomTemplate, ProjectChain, SpecLink, ToolSuggestion,
};
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
    /// What the template suggests for the workspace's connections, resolved: nothing granted yet.
    pub tools: Vec<ToolPlanItem>,
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

pub(crate) fn clean_name(name: &str) -> Result<String, AppError> {
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

    /// The built-in templates, then the ones the person saved.
    pub fn templates(&self) -> Vec<TemplateInfo> {
        let mut all = templates::list();
        all.extend(
            self.config
                .read(|c| c.workflow_templates.clone())
                .iter()
                .map(templates::info_of_custom),
        );
        all
    }

    fn custom_template(&self, id: &str) -> Option<CustomTemplate> {
        self.config
            .read(|c| c.workflow_templates.iter().find(|t| t.id == id).cloned())
    }

    /// What a template's suggestions come to in a workspace: which connection would serve each,
    /// with which tools, or why none can. Reads; changes nothing.
    ///
    /// # Errors
    ///
    /// Fails if the template is unknown.
    pub fn tool_plan(
        &self,
        workspace_id: &str,
        template_id: &str,
    ) -> Result<Vec<ToolPlanItem>, AppError> {
        let suggestions = self.suggestions_of(template_id)?;
        Ok(self.config.read(|c| {
            let connections: Vec<_> = c
                .mcp_connections
                .iter()
                .filter(|m| m.workspace_id == workspace_id)
                .cloned()
                .collect();
            template_tools::resolve(&suggestions, &connections)
        }))
    }

    fn suggestions_of(&self, template_id: &str) -> Result<Vec<ToolSuggestion>, AppError> {
        if let Some(info) = templates::find(template_id) {
            return Ok(info.tools);
        }
        self.custom_template(template_id)
            .map(|t| t.tools)
            .ok_or_else(|| AppError::new(ErrorCode::WorkflowTemplateNotFound))
    }

    /// Saves a copy of a workflow's steps, routes and the tools granted to its steps as a
    /// template of the person's own. The workflow, and the template it came from, are untouched.
    ///
    /// # Errors
    ///
    /// Fails if the workflow is unknown, the name is invalid or saving fails.
    pub fn save_as_template(
        &self,
        workflow_id: &str,
        name: &str,
        description: &str,
    ) -> Result<CustomTemplate, AppError> {
        let workflow = self
            .get(workflow_id)
            .ok_or_else(|| AppError::new(ErrorCode::WorkflowNotFound))?;
        let name = clean_name(name)?;
        let tools = self.config.read(|c| {
            c.mcp_grants
                .iter()
                .filter(|g| {
                    g.workflow_id.as_deref() == Some(workflow.id.as_str()) && g.node_id.is_some()
                })
                .filter_map(|g| {
                    let connection = c.mcp_connections.iter().find(|m| m.id == g.connection_id)?;
                    Some(templates::suggestion_of_grant(
                        g.node_id.as_deref()?,
                        &connection.name,
                        &g.tools,
                    ))
                })
                .collect::<Vec<_>>()
        });
        // What it started from, so the list can say so: a built-in template, or the saved one a
        // copy of a copy came from.
        let based_on = workflow.template_id.as_ref().and_then(|id| {
            if templates::find(id).is_some() {
                Some(id.clone())
            } else {
                self.custom_template(id).and_then(|t| t.based_on)
            }
        });
        let now = now_ms();
        let template = CustomTemplate {
            id: new_id("template"),
            name,
            description: description.trim().to_owned(),
            based_on,
            nodes: workflow.nodes,
            edges: workflow.edges,
            tools,
            created_at: now,
            updated_at: now,
        };
        let saved = template.clone();
        self.config.modify(|config| {
            config.workflow_templates.push(saved);
            Ok(())
        })?;
        Ok(template)
    }

    /// Deletes a template the person saved. The built-in ones cannot be deleted. Workflows made
    /// from it keep working: they are copies.
    ///
    /// # Errors
    ///
    /// Fails if the template is a built-in one or unknown, or saving fails.
    pub fn delete_template(&self, id: &str) -> Result<(), AppError> {
        if templates::find(id).is_some() {
            return Err(AppError::new(ErrorCode::WorkflowTemplateBuiltin));
        }
        if self.custom_template(id).is_none() {
            return Err(AppError::new(ErrorCode::WorkflowTemplateNotFound));
        }
        self.config.modify(|config| {
            config.workflow_templates.retain(|t| t.id != id);
            Ok(())
        })
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
        if let Some(custom) = self.custom_template(template_id) {
            let name = name.map(clean_name).transpose()?;
            let mut workflow = templates::instantiate_custom(
                &custom,
                workspace_id,
                &new_id("workflow"),
                name.as_deref(),
                mode,
                now_ms(),
            );
            workflow.status = self.status_of(&workflow);
            self.store(workflow.clone())?;
            return Ok(FromTemplate {
                workflow,
                missing_roles: Vec::new(),
                tools: self.tool_plan(workspace_id, template_id)?,
            });
        }
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
            tools: template_tools::resolve(
                &built.tools,
                &self.config.read(|c| {
                    c.mcp_connections
                        .iter()
                        .filter(|m| m.workspace_id == workspace_id)
                        .cloned()
                        .collect::<Vec<_>>()
                }),
            ),
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
            // What was granted to its steps goes with it.
            config
                .mcp_grants
                .retain(|g| g.workflow_id.as_deref() != Some(id));
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
            let gone: Vec<String> = config
                .workflows
                .iter()
                .filter(|w| w.workspace_id == workspace_id)
                .map(|w| w.id.clone())
                .collect();
            config
                .mcp_grants
                .retain(|g| !g.workflow_id.as_ref().is_some_and(|id| gone.contains(id)));
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
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn start(&self, workflow_id: &str, task: &str) -> Result<WorkflowExecution, AppError> {
        self.start_checked(workflow_id, task, true, false, None, None, &[])
    }

    /// As [`Self::start`], with the person's standing instruction about the plans the steps draw
    /// up: approve them without asking.
    ///
    /// # Errors
    ///
    /// As [`Self::start`].
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn start_with_plans(
        &self,
        workflow_id: &str,
        task: &str,
        auto_approve_plans: bool,
    ) -> Result<WorkflowExecution, AppError> {
        self.start_checked(workflow_id, task, true, auto_approve_plans, None, None, &[])
    }

    /// As [`Self::start_with_plans`], for a run that works in one project of the workspace and
    /// may consult others. Every step of the run gets this scope.
    ///
    /// # Errors
    ///
    /// As [`Self::start`], and if the scope names a project the workspace does not have.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn start_scoped(
        &self,
        workflow_id: &str,
        task: &str,
        auto_approve_plans: bool,
        scope: Option<ScopeRequest>,
    ) -> Result<WorkflowExecution, AppError> {
        self.start_checked(
            workflow_id,
            task,
            true,
            auto_approve_plans,
            scope,
            None,
            &[],
        )
    }

    /// As [`Self::start_scoped`] (with `spec`: for a run that implements an approved spec, ADR
    /// 0035, which keeps what it was given of it), for a task that changes
    /// several projects (ADR 0043): this run works in the scope's project, and when it completes
    /// the orchestrator starts one run for each of `next_project_ids`, in order. Without any next
    /// project it is exactly the run [`Self::start_scoped`] starts.
    ///
    /// # Errors
    ///
    /// As [`Self::start_scoped`], and if a next project is not one of the workspace's.
    pub fn start_chain(
        &self,
        workflow_id: &str,
        task: &str,
        auto_approve_plans: bool,
        scope: Option<ScopeRequest>,
        spec: Option<SpecLink>,
        next_project_ids: &[String],
    ) -> Result<WorkflowExecution, AppError> {
        self.start_checked(
            workflow_id,
            task,
            true,
            auto_approve_plans,
            scope,
            spec,
            next_project_ids,
        )
    }

    /// Starts the run of `chain`'s current project, after `previous` (the run before it in the
    /// chain) completed. The run is saved but not driven; consulted projects removed from the
    /// workspace since are left out.
    ///
    /// # Errors
    ///
    /// When the workflow is gone or invalid now, the project was removed from the workspace, or
    /// another run of the workflow is going.
    pub fn start_next_in_chain(
        &self,
        previous: &WorkflowExecution,
        chain: ProjectChain,
    ) -> Result<WorkflowExecution, AppError> {
        let project = chain
            .current()
            .cloned()
            .ok_or_else(|| AppError::new(ErrorCode::ScopeInvalid))?;
        let workflow = self
            .get(&previous.workflow_id)
            .ok_or_else(|| AppError::new(ErrorCode::WorkflowNotFound))?;
        if !self.validate(&workflow).valid {
            return Err(AppError::new(ErrorCode::WorkflowInvalid));
        }
        let workspace = self
            .workspaces
            .get(&workflow.workspace_id)
            .ok_or_else(|| AppError::new(ErrorCode::WorkspaceNotFound))?;
        if workspace.project(&project.id).is_none() {
            return Err(AppError::new(ErrorCode::ScopeInvalid).with("project", project.id));
        }
        let scope = ScopeRequest {
            working_project_id: Some(project.id),
            read_project_ids: chain
                .consulted_ids()
                .into_iter()
                .filter(|id| workspace.project(id).is_some())
                .collect(),
        };
        let mut execution =
            WorkflowExecution::new(new_id("wfx"), workflow, previous.task.clone(), now_ms());
        execution.auto_approve_plans = previous.auto_approve_plans;
        execution.scope = Some(scope);
        execution.spec.clone_from(&previous.spec);
        execution.chain = Some(chain);
        self.insert_run(&execution)?;
        Ok(execution)
    }

    /// The runs of a chain that are still in the records, in the chain's order.
    pub fn chain_runs(&self, chain_id: &str) -> Vec<WorkflowExecution> {
        let mut runs: Vec<WorkflowExecution> = self.config.read(|c| {
            c.workflow_executions
                .iter()
                .filter(|e| e.chain.as_ref().is_some_and(|chain| chain.id == chain_id))
                .cloned()
                .collect()
        });
        runs.sort_by_key(|e| (e.chain.as_ref().map_or(0, |c| c.position), e.started_at));
        runs
    }

    /// The chain a first run begins, and the scope that run works with: `None` when the task
    /// changes one project.
    fn chain_of(
        &self,
        workspace_id: &str,
        first_run_id: &str,
        scope: Option<&ScopeRequest>,
        next_project_ids: &[String],
    ) -> Result<Option<(ScopeRequest, ProjectChain)>, AppError> {
        if next_project_ids.is_empty() {
            return Ok(None);
        }
        // Every id is checked against the workspace: a chain never names a project it lacks.
        self.workspaces.resolve_scope(
            workspace_id,
            Some(&ScopeRequest {
                working_project_id: scope.and_then(|s| s.working_project_id.clone()),
                read_project_ids: next_project_ids.to_vec(),
            }),
        )?;
        let working = self.workspaces.resolve_scope(workspace_id, scope)?.working;
        let workspace = self
            .workspaces
            .get(workspace_id)
            .ok_or_else(|| AppError::new(ErrorCode::WorkspaceNotFound))?;
        let mut projects = vec![ChainProject {
            id: working.id.clone(),
            name: working.name.clone(),
        }];
        for id in next_project_ids {
            if projects.iter().any(|p| p.id == *id) {
                continue;
            }
            if let Some(project) = workspace.project(id) {
                projects.push(ChainProject {
                    id: project.id.clone(),
                    name: project.name.clone(),
                });
            }
        }
        if projects.len() < 2 {
            return Ok(None);
        }
        let read_project_ids = scope
            .map(|s| s.read_project_ids.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|id| !projects.iter().any(|p| p.id == *id))
            .collect();
        let chain = ProjectChain {
            id: first_run_id.to_owned(),
            projects,
            position: 0,
            read_project_ids,
            earlier: Vec::new(),
            skipped: Vec::new(),
        };
        let scope = ScopeRequest {
            working_project_id: Some(working.id),
            read_project_ids: chain.consulted_ids(),
        };
        Ok(Some((scope, chain)))
    }

    /// Saves a new run, unless another run of its workflow is going.
    fn insert_run(&self, execution: &WorkflowExecution) -> Result<(), AppError> {
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
        })
    }

    /// Starts without validating: only tests use it, to watch the engine on a definition
    /// validation would have refused (a missing route must still end the run deterministically).
    #[cfg(test)]
    pub fn start_unvalidated(
        &self,
        workflow_id: &str,
        task: &str,
    ) -> Result<WorkflowExecution, AppError> {
        self.start_checked(workflow_id, task, false, false, None, None, &[])
    }

    #[allow(clippy::too_many_arguments)]
    fn start_checked(
        &self,
        workflow_id: &str,
        task: &str,
        validated: bool,
        auto_approve_plans: bool,
        scope: Option<ScopeRequest>,
        spec: Option<SpecLink>,
        next_project_ids: &[String],
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
        // A scope is checked against the workspace now, so a run never starts with one it cannot
        // be given. The default scope is not stored: the run is what it always was.
        if let Some(requested) = &scope {
            self.workspaces
                .resolve_scope(&workflow.workspace_id, Some(requested))?;
        }
        let now = now_ms();
        let id = new_id("wfx");
        let chain = self.chain_of(
            &workflow.workspace_id,
            &id,
            scope.as_ref(),
            next_project_ids,
        )?;
        let mut execution = WorkflowExecution::new(id, workflow, task.to_owned(), now);
        execution.auto_approve_plans = auto_approve_plans;
        match chain {
            Some((first, chain)) => {
                execution.scope = Some(first);
                execution.chain = Some(chain);
            }
            None => execution.scope = scope.filter(|s| *s != ScopeRequest::default()),
        }
        execution.spec = spec;
        self.insert_run(&execution)?;
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

    /// Runs, newest first, that have not ended or ended after `since` (milliseconds since the Unix
    /// epoch): what a listing needs, without copying every run ever made.
    pub fn executions_since(
        &self,
        workspace_id: &str,
        since: u64,
        limit: usize,
    ) -> Vec<WorkflowExecution> {
        self.config.read(|c| {
            c.workflow_executions
                .iter()
                .rev()
                .filter(|e| e.workspace_id == workspace_id)
                .filter(|e| !e.status.is_final() || e.updated_at >= since)
                .take(limit)
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
