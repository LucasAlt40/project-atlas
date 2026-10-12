//! Agents: a personality bound to a runtime, a model and standing instructions.
//!
//! What an agent needs from the installation it lands on is its runtime and model, and those are
//! not the exporter's to promise. The file keeps what the agent used; the preview asks this
//! installation's own runtimes whether they can run it, and offers what they do have. Nothing is
//! substituted without the person choosing it.

use serde::Serialize;

use super::super::apply::{reject, Applier, ModelOutcome, Written};
use super::super::export::Exporter;
use super::super::format::{FormatError, Item, Kind};
use super::super::inspect::{Excerpt, Fact, Inspection, Inspector};
use super::super::spec::{reason_of, AgentSpec, ContractSpec, ModelRef, RuntimeRef};
use super::super::{decode, Category, Dependency, Described, Existing, Issue, Scope};
use super::{existing_of, invalid};
use crate::application::agents::{check_fields, check_model_id, check_name};
use crate::application::config::UserConfig;
use crate::application::errors::{AppError, ErrorCode};
use crate::application::support::new_id;
use crate::domain::agent::Agent;
use crate::domain::runtime::{Availability, ModelDiscovery, RuntimeStatus};
use crate::domain::security::PermissionProfile;

pub struct AgentKind;

/// Whether the runtime and model an agent was exported with can be used here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelStatus {
    /// The runtime is ready and lists the model.
    Available,
    /// The runtime is ready but cannot list its models, so the model cannot be confirmed (the
    /// same position as typing its id into the agent form).
    Unverified,
    /// The runtime is ready and does not list the model.
    ModelMissing,
    /// The runtime is here but cannot be used now (not installed, not signed in, failing).
    RuntimeNotReady,
    /// The runtime is here but cannot run tasks: an agent cannot be made with it.
    RuntimeUnsupported,
    /// This Atlas has no such runtime.
    RuntimeMissing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OriginalModel {
    pub runtime_id: String,
    pub runtime_name: Option<String>,
    pub provider: Option<String>,
    pub model_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelOption {
    pub id: String,
    pub name: String,
}

/// A runtime that can run the agent here, with the models it offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeChoice {
    pub runtime_id: String,
    pub runtime_name: String,
    pub models: Vec<ModelOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelAssessment {
    pub original: OriginalModel,
    pub status: ModelStatus,
    /// The agent can be imported with its original runtime and model (it may then need
    /// configuring before it runs). Not when this Atlas cannot make an agent with that runtime.
    pub keep_allowed: bool,
    /// Where the agent could run instead, the original's runtime first.
    pub alternatives: Vec<RuntimeChoice>,
}

const MAX_MODELS_PER_RUNTIME: usize = 300;

/// Judges the agent's runtime and model against what the runtimes of this installation report.
pub fn assess(statuses: &[RuntimeStatus], runtime: &RuntimeRef, model_id: &str) -> ModelAssessment {
    let found = statuses.iter().find(|s| s.runtime.id == runtime.id);
    let status = match found {
        None => ModelStatus::RuntimeMissing,
        Some(s) if !s.runtime.capabilities.non_interactive_execution => {
            ModelStatus::RuntimeUnsupported
        }
        Some(s) if s.availability != Availability::Ready => ModelStatus::RuntimeNotReady,
        Some(s) if s.model_discovery != ModelDiscovery::Discovered => ModelStatus::Unverified,
        Some(s) if s.available_models.iter().any(|m| m.id == model_id) => ModelStatus::Available,
        Some(_) => ModelStatus::ModelMissing,
    };
    let mut alternatives: Vec<RuntimeChoice> = statuses
        .iter()
        .filter(|s| {
            s.runtime.capabilities.non_interactive_execution
                && s.availability == Availability::Ready
                && s.model_discovery == ModelDiscovery::Discovered
                && !s.available_models.is_empty()
        })
        .map(|s| RuntimeChoice {
            runtime_id: s.runtime.id.clone(),
            runtime_name: s.runtime.name.clone(),
            models: s
                .available_models
                .iter()
                .take(MAX_MODELS_PER_RUNTIME)
                .map(|m| ModelOption {
                    id: m.id.clone(),
                    name: m.name.clone(),
                })
                .collect(),
        })
        .collect();
    alternatives.sort_by_key(|choice| choice.runtime_id != runtime.id);
    if status == ModelStatus::Available {
        alternatives.retain(|choice| choice.runtime_id == runtime.id);
    }
    ModelAssessment {
        original: OriginalModel {
            runtime_id: runtime.id.clone(),
            runtime_name: runtime
                .name
                .clone()
                .or_else(|| found.map(|s| s.runtime.name.clone())),
            provider: runtime
                .provider
                .clone()
                .or_else(|| found.map(|s| s.runtime.provider.name.clone())),
            model_id: model_id.to_owned(),
        },
        status,
        keep_allowed: !matches!(
            status,
            ModelStatus::RuntimeUnsupported | ModelStatus::RuntimeMissing
        ),
        alternatives,
    }
}

fn profile_of(spec: &AgentSpec) -> Result<PermissionProfile, AppError> {
    spec.permission_profile
        .as_deref()
        .map_or(Ok(PermissionProfile::ReadOnly), |id| {
            PermissionProfile::parse(id)
                .ok_or_else(|| AppError::new(ErrorCode::PermissionProfileInvalid))
        })
}

impl Category for AgentKind {
    fn check(&self, item: &Item, at: &str) -> Result<Vec<Dependency>, FormatError> {
        let spec: AgentSpec = decode(item, at)?;
        Ok(vec![Dependency {
            key: spec.personality,
            kind: Kind::Personality,
        }])
    }

    fn list(&self, scope: &Scope<'_>) -> Vec<Existing> {
        existing_of(&scope.config.agents, |a| &a.id, |a| &a.name)
    }

    fn describe(&self, config: &UserConfig, id: &str) -> Option<Described> {
        config
            .agents
            .iter()
            .find(|a| a.id == id)
            .map(|a| Described {
                name: a.name.clone(),
                description: String::new(),
                builtin_id: None,
            })
    }

    fn export(&self, cx: &mut Exporter<'_>, id: &str) -> Result<Item, AppError> {
        let agent = cx
            .config
            .agents
            .iter()
            .find(|a| a.id == id)
            .ok_or_else(|| AppError::new(ErrorCode::AgentNotFound))?;
        let personality = cx
            .reference(Kind::Personality, &agent.personality_id)?
            .ok_or_else(|| AppError::new(ErrorCode::PersonalityNotFound))?;
        let info = cx.runtimes.find(&agent.runtime_id).map(|r| r.info());
        let spec = AgentSpec {
            personality,
            runtime: RuntimeRef {
                id: agent.runtime_id.clone(),
                name: info.as_ref().map(|i| i.name.clone()),
                provider: info.as_ref().map(|i| i.provider.name.clone()),
            },
            model: ModelRef {
                id: agent.model_id.clone(),
            },
            instructions: agent.instructions.clone(),
            permission_profile: agent.permission_profile_id.clone(),
            worktree_isolation: agent.worktree_isolation,
            result_contract: ContractSpec::from(&agent.result_contract),
        };
        Ok(Item {
            key: String::new(),
            kind: Kind::Agent,
            name: agent.name.clone(),
            description: String::new(),
            spec: serde_json::to_value(spec).unwrap_or_default(),
        })
    }

    fn inspect(&self, cx: &Inspector<'_>, item: &Item) -> Inspection {
        let Ok(spec) = decode::<AgentSpec>(item, "") else {
            return Inspection::default();
        };
        let mut inspection = Inspection {
            text: (!spec.instructions.trim().is_empty()).then(|| Excerpt::of(&spec.instructions)),
            ..Inspection::default()
        };
        let model = assess(cx.runtime_statuses(), &spec.runtime, &spec.model.id);
        let runtime_name = model
            .original
            .runtime_name
            .clone()
            .unwrap_or_else(|| spec.runtime.id.clone());
        inspection.facts = vec![
            Fact::new("runtime", runtime_name),
            Fact::new("model", spec.model.id.clone()),
            Fact::new(
                "permissions",
                spec.permission_profile
                    .clone()
                    .unwrap_or_else(|| "read_only".to_owned()),
            ),
            Fact::new(
                "isolation",
                if spec.worktree_isolation { "on" } else { "off" },
            ),
            Fact::new(
                "contract",
                spec.result_contract
                    .outcomes
                    .iter()
                    .map(|o| o.id.as_str())
                    .collect::<Vec<_>>()
                    .join(" / "),
            ),
        ];
        let profile = profile_of(&spec);
        if let Err(error) = &profile {
            inspection
                .issues
                .push(Issue::error("invalid_spec").with("reason", reason_of(error)));
        }
        if let Err(error) = spec.result_contract.to_domain() {
            inspection
                .issues
                .push(Issue::error("invalid_spec").with("reason", reason_of(&error)));
        }
        if let Err(error) = check_name(&item.name).and(check_model_id(&spec.model.id)) {
            inspection
                .issues
                .push(Issue::error("invalid_spec").with("reason", reason_of(&error)));
        }
        if profile == Ok(PermissionProfile::Developer) {
            inspection.issues.push(Issue::warning("grants_editing"));
            let cannot_edit = cx
                .runtime_statuses()
                .iter()
                .find(|s| s.runtime.id == spec.runtime.id)
                .is_some_and(|s| !s.runtime.capabilities.file_edit);
            if cannot_edit {
                inspection
                    .issues
                    .push(Issue::warning("runtime_cannot_edit"));
            }
        }
        let personality = cx.local_for(&spec.personality);
        // What the agent may do here is the person's own decision (an import never widens it),
        // so it is not part of being the same agent.
        inspection.identical = personality.is_some()
            && self
                .existing(&cx.scope(), item)
                .and_then(|e| cx.config.agents.iter().find(|a| a.id == e.id))
                .is_some_and(|a| {
                    Some(&a.personality_id) == personality.as_ref()
                        && a.runtime_id == spec.runtime.id
                        && a.model_id == spec.model.id
                        && a.instructions == spec.instructions.trim()
                        && a.worktree_isolation == spec.worktree_isolation
                        && spec.result_contract.to_domain().as_ref() == Ok(&a.result_contract)
                });
        inspection.model = Some(model);
        inspection
    }

    fn write(
        &self,
        cx: &mut Applier<'_>,
        config: &mut UserConfig,
        item: &Item,
        replacing: Option<&Existing>,
    ) -> Result<Written, AppError> {
        let spec: AgentSpec = decode(item, "").map_err(|_| reject("invalid", &item.name))?;
        let personality_id = cx.local(&spec.personality, item)?;
        let (runtime_id, model_id) = cx.request.models.get(&item.key).map_or_else(
            || (spec.runtime.id.clone(), spec.model.id.clone()),
            |choice| (choice.runtime_id.clone(), choice.model_id.clone()),
        );
        let (_, model_id) = check_fields(cx.runtimes, &item.name, &runtime_id, &model_id)
            .map_err(|error| invalid(item, &error))?;
        let contract = spec
            .result_contract
            .to_domain()
            .map_err(|error| invalid(item, &error))?;
        let requested = profile_of(&spec).map_err(|error| invalid(item, &error))?;
        let granted = if cx.request.keep_permission_profiles {
            requested
        } else {
            PermissionProfile::ReadOnly
        };
        let mut issues = Vec::new();
        if granted != requested {
            issues.push(
                Issue::warning("permissions_lowered")
                    .with("requested", requested.id())
                    .with("granted", granted.id()),
            );
        }
        let taken: Vec<String> = config
            .agents
            .iter()
            .filter(|a| replacing.is_none_or(|r| r.id != a.id))
            .map(|a| a.name.to_lowercase())
            .collect();
        let (name, _) = Applier::free_name(
            |candidate| taken.contains(&candidate.trim().to_lowercase()),
            item.name.trim(),
        );
        let id = if let Some(existing) = replacing {
            let agent = config
                .agents
                .iter_mut()
                .find(|a| a.id == existing.id)
                .ok_or_else(|| AppError::new(ErrorCode::AgentNotFound))?;
            agent.name.clone_from(&name);
            agent.personality_id = personality_id;
            agent.runtime_id.clone_from(&runtime_id);
            agent.model_id.clone_from(&model_id);
            spec.instructions.trim().clone_into(&mut agent.instructions);
            agent.permission_profile_id = Some(granted.id().to_owned());
            agent.worktree_isolation = spec.worktree_isolation;
            agent.result_contract = contract;
            agent.id.clone()
        } else {
            let id = new_id("agent");
            config.agents.push(Agent {
                id: id.clone(),
                name: name.clone(),
                personality_id,
                runtime_id: runtime_id.clone(),
                model_id: model_id.clone(),
                instructions: spec.instructions.trim().to_owned(),
                permission_profile_id: Some(granted.id().to_owned()),
                worktree_isolation: spec.worktree_isolation,
                result_contract: contract,
                created_at: cx.now,
            });
            id
        };
        Ok(Written {
            id,
            name,
            issues,
            model: Some(ModelOutcome {
                original_runtime: spec.runtime.id,
                original_model: spec.model.id,
                runtime: runtime_id,
                model: model_id,
            }),
        })
    }
}
