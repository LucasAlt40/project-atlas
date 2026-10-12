//! Workflows: a graph of steps, each run by an agent. Imported into one workspace, as a new
//! definition (or over one the person names), with every step pointing at an agent that exists
//! here. Importing it authorizes nothing to run: it is a definition, like any other, started
//! by a person.

use std::collections::BTreeMap;

use super::super::apply::{reject, Applier, Written};
use super::super::export::{ExportWarning, Exporter};
use super::super::format::{FormatError, Item, Kind};
use super::super::inspect::{Fact, Inspection, Inspector};
use super::super::spec::{
    check_graph, graph_to_domain, graph_to_spec, AgentSpec, ModeSpec, StepSpec, WorkflowSpec,
};
use super::super::{decode, Category, Dependency, Described, Existing, Issue, Scope};
use super::{candidate_graph, invalid, same_graph, step_previews, PreviewCatalog};
use crate::application::config::UserConfig;
use crate::application::errors::{AppError, ErrorCode};
use crate::application::support::new_id;
use crate::application::workflow::service::clean_name;
use crate::application::workflow::validation::{validate, AgentCatalog};
use crate::domain::workflow::{Workflow, WorkflowExecutionStatus, WorkflowStatus};

pub struct WorkflowKind;

/// The agents of an installation, for the validator.
struct Agents<'a>(&'a UserConfig);

impl AgentCatalog for Agents<'_> {
    fn agent_exists(&self, id: &str) -> bool {
        self.0.agents.iter().any(|a| a.id == id)
    }

    fn declared_outcomes(&self, id: &str) -> Vec<String> {
        PreviewCatalog {
            config: self.0,
            pending: BTreeMap::new(),
        }
        .declared_outcomes(id)
    }
}

fn status_of(workflow: &Workflow, config: &UserConfig) -> WorkflowStatus {
    if validate(workflow, Some(&workflow.workspace_id), &Agents(config)).valid {
        WorkflowStatus::Ready
    } else {
        WorkflowStatus::Draft
    }
}

impl Category for WorkflowKind {
    fn check(&self, item: &Item, at: &str) -> Result<Vec<Dependency>, FormatError> {
        let spec: WorkflowSpec = decode(item, at)?;
        Ok(check_graph(&spec.nodes, &spec.edges, at)?
            .into_iter()
            .map(|key| Dependency {
                key,
                kind: Kind::Agent,
            })
            .collect())
    }

    fn list(&self, scope: &Scope<'_>) -> Vec<Existing> {
        let Some(workspace) = scope.workspace_id else {
            return Vec::new();
        };
        let here: Vec<&Workflow> = scope
            .config
            .workflows
            .iter()
            .filter(|w| w.workspace_id == workspace)
            .collect();
        here.iter()
            .map(|w| Existing {
                id: w.id.clone(),
                name: w.name.clone(),
            })
            .collect()
    }

    fn describe(&self, config: &UserConfig, id: &str) -> Option<Described> {
        config
            .workflows
            .iter()
            .find(|w| w.id == id)
            .map(|w| Described {
                name: w.name.clone(),
                description: w.description.clone(),
                builtin_id: None,
            })
    }

    fn export(&self, cx: &mut Exporter<'_>, id: &str) -> Result<Item, AppError> {
        let config = cx.config;
        let workflow = config
            .workflows
            .iter()
            .find(|w| w.id == id)
            .ok_or_else(|| AppError::new(ErrorCode::WorkflowNotFound))?;
        let mut refs: BTreeMap<String, Option<String>> = BTreeMap::new();
        for agent_id in workflow.agent_ids() {
            refs.insert(agent_id.to_owned(), cx.reference(Kind::Agent, agent_id)?);
        }
        if refs.values().any(Option::is_none) {
            cx.warnings.push(ExportWarning {
                code: "agent_missing",
                name: workflow.name.clone(),
            });
        }
        let (nodes, edges) = graph_to_spec(&workflow.nodes, &workflow.edges, &mut |agent_id| {
            refs.get(agent_id).cloned().flatten()
        });
        let spec = WorkflowSpec {
            mode: ModeSpec::from(workflow.mode),
            nodes,
            edges,
        };
        Ok(Item {
            key: String::new(),
            kind: Kind::Workflow,
            name: workflow.name.clone(),
            description: workflow.description.clone(),
            spec: serde_json::to_value(spec).unwrap_or_default(),
        })
    }

    fn inspect(&self, cx: &Inspector<'_>, item: &Item) -> Inspection {
        let Ok(spec) = decode::<WorkflowSpec>(item, "") else {
            return Inspection::default();
        };
        let mut inspection = Inspection {
            steps: step_previews(&spec.nodes),
            facts: vec![
                Fact::new("steps", spec.nodes.len().to_string()),
                Fact::new("routes", spec.edges.len().to_string()),
                Fact::new(
                    "mode",
                    match spec.mode {
                        ModeSpec::Automatic => "automatic",
                        ModeSpec::Custom => "custom",
                    },
                ),
            ],
            ..Inspection::default()
        };
        let (nodes, edges, all_known) = candidate_graph(cx, &spec.nodes, &spec.edges);
        // The agents the file brings, under the placeholder ids the candidate uses.
        let pending = cx
            .package
            .items
            .iter()
            .filter(|i| i.kind == Kind::Agent)
            .filter_map(|i| {
                let agent: AgentSpec = decode(i, "").ok()?;
                Some((
                    format!("~{}", i.key),
                    agent
                        .result_contract
                        .outcomes
                        .into_iter()
                        .map(|o| o.id)
                        .collect(),
                ))
            })
            .collect();
        let candidate = Workflow {
            id: String::new(),
            workspace_id: cx.workspace_id.unwrap_or_default().to_owned(),
            name: item.name.clone(),
            description: item.description.clone(),
            mode: spec.mode.into(),
            version: 1,
            status: WorkflowStatus::Draft,
            template_id: None,
            nodes,
            edges,
            viewport: None,
            route_repairs: Vec::new(),
            created_at: 0,
            updated_at: 0,
        };
        let report = validate(
            &candidate,
            None,
            &PreviewCatalog {
                config: cx.config,
                pending,
            },
        );
        if !report.valid {
            let errors = report
                .issues
                .iter()
                .filter(|i| i.severity == crate::application::workflow::validation::Severity::Error)
                .count();
            inspection
                .issues
                .push(Issue::warning("workflow_draft").with("errors", errors.to_string()));
        }
        if spec
            .nodes
            .iter()
            .any(|n| matches!(&n.step, StepSpec::Agent(s) if s.agent.is_none()))
        {
            inspection.issues.push(Issue::warning("step_without_agent"));
        }
        inspection.identical = all_known
            && self
                .existing(&cx.scope(), item)
                .and_then(|e| cx.config.workflows.iter().find(|w| w.id == e.id))
                .is_some_and(|w| {
                    w.description == item.description.trim()
                        && same_graph((&w.nodes, &w.edges), (&candidate.nodes, &candidate.edges))
                });
        inspection
    }

    fn write(
        &self,
        cx: &mut Applier<'_>,
        config: &mut UserConfig,
        item: &Item,
        replacing: Option<&Existing>,
    ) -> Result<Written, AppError> {
        let spec: WorkflowSpec = decode(item, "").map_err(|_| reject("invalid", &item.name))?;
        let workspace_id = cx
            .workspace_id
            .ok_or_else(|| reject("workspace_required", &item.name))?
            .to_owned();
        let mut agent_ids: BTreeMap<String, String> = BTreeMap::new();
        for node in &spec.nodes {
            if let StepSpec::Agent(step) = &node.step {
                if let Some(key) = &step.agent {
                    agent_ids.insert(key.clone(), cx.local(key, item)?);
                }
            }
        }
        let (nodes, edges) = graph_to_domain(&spec.nodes, &spec.edges, &|key| {
            agent_ids.get(key).cloned().unwrap_or_default()
        });
        let wanted = clean_name(&item.name).map_err(|error| invalid(item, &error))?;
        let taken: Vec<String> = config
            .workflows
            .iter()
            .filter(|w| w.workspace_id == workspace_id)
            .filter(|w| replacing.is_none_or(|r| r.id != w.id))
            .map(|w| w.name.to_lowercase())
            .collect();
        let (name, _) = Applier::free_name(
            |candidate| taken.contains(&candidate.trim().to_lowercase()),
            &wanted,
        );
        let mode = spec.mode.into();
        let description = item.description.trim().to_owned();
        let workflow = if let Some(existing) = replacing {
            let current = config
                .workflows
                .iter()
                .find(|w| w.id == existing.id)
                .ok_or_else(|| AppError::new(ErrorCode::WorkflowNotFound))?;
            let running = config.workflow_executions.iter().any(|e| {
                e.workflow_id == current.id
                    && (e.status.is_active() || e.status == WorkflowExecutionStatus::Interrupted)
            });
            if running {
                return Err(reject("workflow_running", &item.name));
            }
            let mut next = current.clone();
            next.name.clone_from(&name);
            next.description = description;
            next.mode = mode;
            next.nodes = nodes;
            next.edges = edges;
            if !next.same_structure(current) {
                next.version = current.version + 1;
            }
            next.updated_at = cx.now;
            next
        } else {
            Workflow {
                id: new_id("workflow"),
                workspace_id,
                name: name.clone(),
                description,
                mode,
                version: 1,
                status: WorkflowStatus::Draft,
                template_id: None,
                nodes,
                edges,
                viewport: None,
                route_repairs: Vec::new(),
                created_at: cx.now,
                updated_at: cx.now,
            }
        };
        let mut workflow = workflow;
        workflow.status = status_of(&workflow, config);
        let mut issues = Vec::new();
        if workflow.status == WorkflowStatus::Draft {
            issues.push(Issue::warning("workflow_draft"));
        }
        let id = workflow.id.clone();
        match config.workflows.iter_mut().find(|w| w.id == id) {
            Some(slot) => *slot = workflow,
            None => config.workflows.push(workflow),
        }
        Ok(Written {
            id,
            name,
            issues,
            model: None,
        })
    }
}
