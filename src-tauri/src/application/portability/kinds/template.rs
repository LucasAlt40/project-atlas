//! Saved workflow templates: the steps and routes of a workflow kept to start others from, and
//! the tools it suggests for them. A template only ever *suggests* tools: a suggestion becomes a
//! grant when a person starts a workflow with that option on, never because a file said so.

use std::collections::BTreeMap;

use super::super::apply::{reject, Applier, Written};
use super::super::export::{ExportWarning, Exporter};
use super::super::format::{FormatError, Item, Kind};
use super::super::inspect::{Fact, Inspection, Inspector};
use super::super::spec::{
    check_graph, check_tools, graph_to_domain, graph_to_spec, StepSpec, TemplateSpec, ToolSpec,
};
use super::super::{decode, Category, Dependency, Described, Existing, Issue, Scope};
use super::{candidate_graph, existing_of, invalid, same_graph, step_previews};
use crate::application::config::UserConfig;
use crate::application::errors::{AppError, ErrorCode};
use crate::application::support::new_id;
use crate::application::workflow::service::clean_name;
use crate::application::workflow::templates;
use crate::domain::workflow::{CustomTemplate, ToolSuggestion};

pub struct TemplateKind;

/// A built-in template id is one that ships with Atlas; anything else is not a template to
/// say a saved one started from.
fn known_base(spec: &TemplateSpec) -> Option<String> {
    spec.based_on
        .clone()
        .filter(|id| templates::find(id).is_some())
}

impl Category for TemplateKind {
    fn check(&self, item: &Item, at: &str) -> Result<Vec<Dependency>, FormatError> {
        let spec: TemplateSpec = decode(item, at)?;
        check_tools(&spec.tools, &spec.nodes, at)?;
        Ok(check_graph(&spec.nodes, &spec.edges, at)?
            .into_iter()
            .map(|key| Dependency {
                key,
                kind: Kind::Agent,
            })
            .collect())
    }

    fn list(&self, scope: &Scope<'_>) -> Vec<Existing> {
        existing_of(&scope.config.workflow_templates, |t| &t.id, |t| &t.name)
    }

    fn describe(&self, config: &UserConfig, id: &str) -> Option<Described> {
        config
            .workflow_templates
            .iter()
            .find(|t| t.id == id)
            .map(|t| Described {
                name: t.name.clone(),
                description: t.description.clone(),
                builtin_id: None,
            })
    }

    fn export(&self, cx: &mut Exporter<'_>, id: &str) -> Result<Item, AppError> {
        let config = cx.config;
        let template = config
            .workflow_templates
            .iter()
            .find(|t| t.id == id)
            .ok_or_else(|| AppError::new(ErrorCode::WorkflowTemplateNotFound))?;
        let mut refs: BTreeMap<String, Option<String>> = BTreeMap::new();
        for node in &template.nodes {
            if let Some(agent_id) = node.agent_id() {
                if !refs.contains_key(agent_id) {
                    refs.insert(agent_id.to_owned(), cx.reference(Kind::Agent, agent_id)?);
                }
            }
        }
        if refs.values().any(Option::is_none) {
            cx.warnings.push(ExportWarning {
                code: "agent_missing",
                name: template.name.clone(),
            });
        }
        let (nodes, edges) = graph_to_spec(&template.nodes, &template.edges, &mut |agent_id| {
            refs.get(agent_id).cloned().flatten()
        });
        let spec = TemplateSpec {
            based_on: template.based_on.clone(),
            nodes,
            edges,
            tools: template.tools.iter().map(ToolSpec::from).collect(),
        };
        Ok(Item {
            key: String::new(),
            kind: Kind::WorkflowTemplate,
            name: template.name.clone(),
            description: template.description.clone(),
            spec: serde_json::to_value(spec).unwrap_or_default(),
        })
    }

    fn inspect(&self, cx: &Inspector<'_>, item: &Item) -> Inspection {
        let Ok(spec) = decode::<TemplateSpec>(item, "") else {
            return Inspection::default();
        };
        let mut inspection = Inspection {
            steps: step_previews(&spec.nodes),
            facts: vec![
                Fact::new("steps", spec.nodes.len().to_string()),
                Fact::new("tools", spec.tools.len().to_string()),
            ],
            ..Inspection::default()
        };
        if spec
            .nodes
            .iter()
            .any(|n| matches!(&n.step, StepSpec::Agent(s) if s.agent.is_none()))
        {
            inspection.issues.push(Issue::warning("step_without_agent"));
        }
        if !spec.tools.is_empty() {
            inspection
                .issues
                .push(Issue::new("tools_suggested", super::super::Severity::Info));
        }
        let (nodes, edges, all_known) = candidate_graph(cx, &spec.nodes, &spec.edges);
        let tools: Vec<ToolSuggestion> = spec.tools.iter().map(ToolSuggestion::from).collect();
        inspection.identical = all_known
            && self
                .existing(&cx.scope(), item)
                .and_then(|e| cx.config.workflow_templates.iter().find(|t| t.id == e.id))
                .is_some_and(|t| {
                    t.description == item.description.trim()
                        && t.tools == tools
                        && same_graph((&t.nodes, &t.edges), (&nodes, &edges))
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
        let spec: TemplateSpec = decode(item, "").map_err(|_| reject("invalid", &item.name))?;
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
            .workflow_templates
            .iter()
            .filter(|t| replacing.is_none_or(|r| r.id != t.id))
            .map(|t| t.name.to_lowercase())
            .collect();
        let (name, _) = Applier::free_name(
            |candidate| taken.contains(&candidate.trim().to_lowercase()),
            &wanted,
        );
        let tools: Vec<ToolSuggestion> = spec.tools.iter().map(ToolSuggestion::from).collect();
        let description = item.description.trim().to_owned();
        let based_on = known_base(&spec);
        let id = if let Some(existing) = replacing {
            let template = config
                .workflow_templates
                .iter_mut()
                .find(|t| t.id == existing.id)
                .ok_or_else(|| AppError::new(ErrorCode::WorkflowTemplateNotFound))?;
            template.name.clone_from(&name);
            template.description = description;
            template.based_on = based_on;
            template.nodes = nodes;
            template.edges = edges;
            template.tools = tools;
            template.updated_at = cx.now;
            template.id.clone()
        } else {
            let id = new_id("template");
            config.workflow_templates.push(CustomTemplate {
                id: id.clone(),
                name: name.clone(),
                description,
                based_on,
                nodes,
                edges,
                tools,
                created_at: cx.now,
                updated_at: cx.now,
            });
            id
        };
        Ok(Written {
            id,
            name,
            issues: Vec::new(),
            model: None,
        })
    }
}
