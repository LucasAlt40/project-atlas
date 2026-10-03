use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use super::errors::AppError;
use crate::domain::agent::Agent;
use crate::domain::personality::PersonalityProfile;
use crate::domain::usage::{QuotaInfo, UsageRecord};
use crate::domain::workspace::Workspace;

pub const DEFAULT_LANGUAGE: &str = "pt-BR";

/// Settings that belong to the whole application, not to any workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub language: String,
    /// The workspace that was open last, so it opens again next time.
    #[serde(default)]
    pub selected_workspace_id: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            language: DEFAULT_LANGUAGE.to_owned(),
            selected_workspace_id: None,
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

impl ConfigRepository {
    pub fn load(store: Box<dyn ConfigStore>) -> Self {
        let config = store.load().unwrap_or_else(|error| {
            eprintln!("could not load user config, starting empty: {error}");
            UserConfig::default()
        });
        Self {
            store,
            config: Mutex::new(config),
        }
    }

    pub fn agents(&self) -> Vec<Agent> {
        self.lock().agents.clone()
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
            created_at: 1,
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
