use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use super::errors::AppError;
use crate::domain::agent::Agent;
use crate::domain::conversation::Message;
use crate::domain::execution::StoredExecution;
use crate::domain::personality::PersonalityProfile;
use crate::domain::usage::{QuotaInfo, UsageRecord};
use crate::domain::workflow::{Workflow, WorkflowExecution};
use crate::domain::workspace::Workspace;
use crate::domain::worktree::ExecutionWorktree;

pub const DEFAULT_LANGUAGE: &str = "pt-BR";

/// Settings that belong to the whole application, not to any workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub language: String,
    /// The workspace that was open last, so it opens again next time.
    #[serde(default)]
    pub selected_workspace_id: Option<String>,
    /// Switches of the Optimization Layer.
    #[serde(default)]
    pub optimization: OptimizationSettings,
}

/// Which parts of the Optimization Layer are on. Phase 0 has only the metrics, and they only
/// measure: on or off, the prompt, the runtime and the permissions are the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)] // independent switches, one per part of the layer
pub struct OptimizationSettings {
    /// `optimization.metrics.enabled`: record what each prompt is made of and how long each stage
    /// took. On by default: a baseline is only worth having if it is always there.
    #[serde(default = "default_metrics_enabled")]
    pub metrics_enabled: bool,
    /// `optimization.context.enabled`: the Context Engine removes what a prompt says twice (and
    /// keeps a budget, if one is set). Off by default: unlike the metrics it changes the prompt,
    /// so it is switched on on purpose and compared with the benchmark first.
    #[serde(default)]
    pub context_enabled: bool,
    /// `optimization.context.maxTokens`: an estimated-token budget for the prompt. Only items that
    /// are not required, relevant or the user's own can be left out for it; if that is not
    /// enough, the overrun is reported and nothing is cut. `None`: no budget.
    #[serde(default)]
    pub context_max_tokens: Option<u64>,
    /// `optimization.skills.enabled`: Atlas looks for skills (`<project>/.atlas/skills`, the
    /// user's `skills` folder) and sends the ones a task calls for. Off by default: it adds text
    /// to prompts.
    #[serde(default)]
    pub skills_enabled: bool,
    /// `optimization.guardrails.enabled`: before an agent starts, Atlas reviews the context it is
    /// about to receive (and asks, or stops, when it is not fit) and takes secrets out of context
    /// that came from files and other agents. On by default: it is a safety check, and it changes
    /// a prompt only to remove something that looked like a secret.
    #[serde(default = "default_guardrails_enabled")]
    pub guardrails_enabled: bool,
}

fn default_guardrails_enabled() -> bool {
    true
}

fn default_metrics_enabled() -> bool {
    true
}

impl Default for OptimizationSettings {
    fn default() -> Self {
        Self {
            metrics_enabled: true,
            context_enabled: false,
            context_max_tokens: None,
            skills_enabled: false,
            guardrails_enabled: true,
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            language: DEFAULT_LANGUAGE.to_owned(),
            selected_workspace_id: None,
            optimization: OptimizationSettings::default(),
        }
    }
}

/// Everything the user creates and Atlas must remember. Built-in personalities ship with the
/// app and are not stored, except for the user's changes to them. Secrets never go here.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserConfig {
    #[serde(default)]
    pub personalities: Vec<PersonalityProfile>,
    #[serde(default)]
    pub agents: Vec<Agent>,
    /// The user's edited versions of built-in personalities (same ids as the built-ins).
    #[serde(default)]
    pub builtin_overrides: Vec<PersonalityProfile>,
    /// Ids of built-in personalities the user removed.
    #[serde(default)]
    pub hidden_builtins: Vec<String>,
    #[serde(default)]
    pub workspaces: Vec<Workspace>,
    #[serde(default)]
    pub settings: AppSettings,
    /// What Atlas observed each execution consume, newest last (old entries are pruned).
    #[serde(default)]
    pub usage: Vec<UsageRecord>,
    /// The last quota each runtime reported, by runtime id.
    #[serde(default)]
    pub quotas: BTreeMap<String, QuotaInfo>,
    /// Every agent's conversations, oldest first (bounded per conversation; see `history`).
    #[serde(default)]
    pub messages: Vec<Message>,
    /// The executions that ended, oldest first. Raw terminal output is never stored.
    #[serde(default)]
    pub executions: Vec<StoredExecution>,
    /// The Git worktree of every execution that ran isolated, oldest first. Written when the
    /// worktree is created (not when the execution ends) so a crash cannot lose track of it.
    #[serde(default)]
    pub worktrees: Vec<ExecutionWorktree>,
    /// Workflow definitions. Kept apart from the runs of them: a definition is edited, a run is
    /// history.
    #[serde(default)]
    pub workflows: Vec<Workflow>,
    /// Runs of workflows, oldest first, each with the snapshot of the definition it started from.
    #[serde(default)]
    pub workflow_executions: Vec<WorkflowExecution>,
}

/// Port: durable storage for [`UserConfig`]. Implemented in `infrastructure/`.
pub trait ConfigStore: Send + Sync {
    /// # Errors
    ///
    /// Fails with a readable message if the stored data cannot be read.
    fn load(&self) -> Result<UserConfig, String>;

    /// # Errors
    ///
    /// Fails with a readable message if the data cannot be written.
    fn save(&self, config: &UserConfig) -> Result<(), String>;
}

/// In-memory copy of the user config that writes through to the store on every change.
pub struct ConfigRepository {
    store: Box<dyn ConfigStore>,
    config: Mutex<UserConfig>,
}

/// Agents saved before permission profiles could be chosen have none, which reads as the most
/// restrictive profile. They get the developer profile once, written down, so every agent can
/// edit and the user can change each one. The workspace's policy still bounds it. Returns whether
/// anything changed.
fn grant_developer_to_unset_agents(config: &mut UserConfig) -> bool {
    let mut changed = false;
    for agent in config
        .agents
        .iter_mut()
        .filter(|agent| agent.permission_profile_id.is_none())
    {
        agent.permission_profile_id = Some("developer".to_owned());
        changed = true;
    }
    changed
}

/// Workflows saved before routing read the agent's declared outcome tested `result.status`, which
/// only says whether the step ran (`success`/`failed`), so an edge on `pass` or `fail` could never
/// fire and the run ended with no route. Where the source agent declares that outcome, the edge
/// now tests `result.outcome`, and the workflow becomes a new version, so a run that failed on
/// the old routes can go on under the corrected ones. Returns whether anything changed.
fn route_edges_on_declared_outcomes(config: &mut UserConfig) -> bool {
    let mut changed = false;
    for workflow in &mut config.workflows {
        let mut edited = false;
        let sources: Vec<(String, Option<String>)> = workflow
            .nodes
            .iter()
            .map(|n| (n.id.clone(), n.agent_id().map(str::to_owned)))
            .collect();
        for edge in &mut workflow.edges {
            let Some(condition) = edge.condition.as_mut() else {
                continue;
            };
            if condition.field != "result.status" {
                continue;
            }
            let Some(value) = condition.value.as_deref() else {
                continue;
            };
            let declared = sources
                .iter()
                .find(|(id, _)| *id == edge.source_node_id)
                .and_then(|(_, agent_id)| agent_id.as_deref())
                .and_then(|agent_id| config.agents.iter().find(|a| a.id == agent_id))
                .is_some_and(|agent| {
                    agent
                        .result_contract
                        .outcomes
                        .iter()
                        .any(|o| o.id.eq_ignore_ascii_case(value))
                });
            if declared {
                "result.outcome".clone_into(&mut condition.field);
                edited = true;
            }
        }
        if edited {
            workflow.version += 1;
            changed = true;
        }
    }
    changed
}

impl ConfigRepository {
    pub fn load(store: Box<dyn ConfigStore>) -> Self {
        let mut config = store.load().unwrap_or_else(|error| {
            eprintln!("could not load user config, starting empty: {error}");
            UserConfig::default()
        });
        let outcomes_routed = route_edges_on_declared_outcomes(&mut config);
        if grant_developer_to_unset_agents(&mut config) || outcomes_routed {
            if let Err(error) = store.save(&config) {
                eprintln!("could not save the migrated agent permissions: {error}");
            }
        }
        Self {
            store,
            config: Mutex::new(config),
        }
    }

    pub fn agents(&self) -> Vec<Agent> {
        self.lock().agents.clone()
    }

    /// Reads the stored config without copying it.
    pub fn read<T>(&self, read: impl FnOnce(&UserConfig) -> T) -> T {
        read(&self.lock())
    }

    /// A copy of everything stored.
    pub fn snapshot(&self) -> UserConfig {
        self.lock().clone()
    }

    /// # Errors
    ///
    /// Fails (leaving the config unchanged) if saving fails.
    pub fn add_personality(&self, personality: PersonalityProfile) -> Result<(), AppError> {
        self.modify(|config| {
            config.personalities.push(personality);
            Ok(())
        })
    }

    /// # Errors
    ///
    /// Fails (leaving the config unchanged) if saving fails.
    pub fn add_agent(&self, agent: Agent) -> Result<(), AppError> {
        self.modify(|config| {
            config.agents.push(agent);
            Ok(())
        })
    }

    /// Applies `change` to a copy and saves it; the stored config is replaced only if both the
    /// change and the save succeed, so a failure leaves everything as it was.
    ///
    /// # Errors
    ///
    /// Fails with whatever `change` returns, or if saving fails.
    pub fn modify<T>(
        &self,
        change: impl FnOnce(&mut UserConfig) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let mut guard = self.lock();
        let mut next = guard.clone();
        let value = change(&mut next)?;
        self.store.save(&next).map_err(AppError::storage)?;
        *guard = next;
        Ok(value)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, UserConfig> {
        self.config.lock().expect("user config lock poisoned")
    }
}

#[cfg(test)]
pub mod memory {
    use std::sync::Mutex;

    use std::sync::Arc;

    use super::{ConfigStore, UserConfig};

    /// Lets two repositories share one store, to simulate restarting the app on the same data.
    impl ConfigStore for Arc<MemoryStore> {
        fn load(&self) -> Result<UserConfig, String> {
            self.as_ref().load()
        }

        fn save(&self, config: &UserConfig) -> Result<(), String> {
            self.as_ref().save(config)
        }
    }

    /// Store for tests; `fail_saves` simulates a full disk.
    #[derive(Default)]
    pub struct MemoryStore {
        pub saved: Mutex<Option<UserConfig>>,
        pub fail_saves: bool,
    }

    impl ConfigStore for MemoryStore {
        fn load(&self) -> Result<UserConfig, String> {
            Ok(self.saved.lock().unwrap().clone().unwrap_or_default())
        }

        fn save(&self, config: &UserConfig) -> Result<(), String> {
            if self.fail_saves {
                return Err("disk full".to_owned());
            }
            *self.saved.lock().unwrap() = Some(config.clone());
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::memory::MemoryStore;
    use super::*;

    fn agent() -> Agent {
        Agent {
            id: "a".to_owned(),
            name: "A".to_owned(),
            personality_id: "p".to_owned(),
            runtime_id: "x".to_owned(),
            model_id: "m".to_owned(),
            instructions: String::new(),
            permission_profile_id: None,
            worktree_isolation: true,
            result_contract: crate::domain::result_contract::ResultContract::default(),
            created_at: 1,
        }
    }

    #[test]
    fn agents_saved_without_a_profile_get_the_developer_profile_once_and_keep_a_chosen_one() {
        let store = MemoryStore::default();
        let mut stored = UserConfig::default();
        let mut chosen = agent();
        chosen.id = "b".to_owned();
        chosen.permission_profile_id = Some("read_only".to_owned());
        stored.agents = vec![agent(), chosen];
        store.save(&stored).unwrap();

        let repo = ConfigRepository::load(Box::new(store));

        let profiles: Vec<_> = repo
            .agents()
            .iter()
            .map(|a| a.permission_profile_id.clone())
            .collect();
        assert_eq!(
            profiles,
            [Some("developer".to_owned()), Some("read_only".to_owned())]
        );
    }

    #[test]
    fn status_edges_on_a_declared_outcome_are_routed_on_the_outcome() {
        use crate::application::workflow::test_support::{agent as node, when, workflow};
        use crate::domain::result_contract::Outcome;
        let mut validator = agent();
        validator.result_contract.outcomes = ["pass", "fail"]
            .map(|id| Outcome {
                id: id.to_owned(),
                label: id.to_owned(),
                description: String::new(),
            })
            .to_vec();
        let mut config = UserConfig {
            agents: vec![validator],
            workflows: vec![workflow(
                vec![node("v", "a"), node("next", "a")],
                vec![when("v", "next", "fail"), when("v", "next", "success")],
            )],
            ..UserConfig::default()
        };

        assert!(route_edges_on_declared_outcomes(&mut config));

        let fields: Vec<_> = config.workflows[0]
            .edges
            .iter()
            .map(|e| e.condition.as_ref().unwrap().field.as_str())
            .collect();
        assert_eq!(fields, ["result.outcome", "result.status"]);
        assert_eq!(config.workflows[0].version, 2);
        // Run again, nothing is left to migrate.
        assert!(!route_edges_on_declared_outcomes(&mut config));
        assert_eq!(config.workflows[0].version, 2);
    }

    #[test]
    fn a_status_edge_is_never_taken_for_an_outcome_the_agent_did_not_declare() {
        use crate::application::workflow::test_support::{agent as node, when, workflow};
        use crate::domain::result_contract::{ContractKind, ResultContract};
        // QA declares approved / changes_requested: `pass` and `fail` are not among them, and
        // Atlas does not assume that they mean the same thing.
        let mut qa = agent();
        qa.result_contract = ResultContract::preset(ContractKind::Review);
        let mut config = UserConfig {
            agents: vec![qa],
            workflows: vec![workflow(
                vec![node("qa", "a"), node("done", "a"), node("fix", "a")],
                vec![when("qa", "done", "pass"), when("qa", "fix", "fail")],
            )],
            ..UserConfig::default()
        };
        let before = config.workflows.clone();

        // Opening the app, validating and saving again never touch it, however often.
        for _ in 0..3 {
            assert!(!route_edges_on_declared_outcomes(&mut config));
            assert_eq!(config.workflows, before);
        }
    }

    #[test]
    fn writes_through_and_keeps_state_when_saving_fails() {
        let repo = ConfigRepository::load(Box::<MemoryStore>::default());
        repo.add_agent(agent()).unwrap();
        assert_eq!(repo.agents().len(), 1);

        let failing = ConfigRepository::load(Box::new(MemoryStore {
            fail_saves: true,
            ..MemoryStore::default()
        }));
        assert_eq!(
            failing.add_agent(agent()).unwrap_err(),
            AppError::storage("disk full")
        );
        assert_eq!(failing.agents(), []);
    }
}
