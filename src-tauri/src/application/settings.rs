use std::sync::Arc;

use super::config::{AppSettings, ConfigRepository};
use super::errors::{AppError, ErrorCode};

/// Languages the UI is translated into.
pub const SUPPORTED_LANGUAGES: [&str; 2] = ["pt-BR", "en-US"];

/// Use case: settings of the whole application (not of any workspace): the UI language and the
/// workspace that was open last.
pub struct SettingsService {
    config: Arc<ConfigRepository>,
}

impl SettingsService {
    pub fn new(config: Arc<ConfigRepository>) -> Self {
        Self { config }
    }

    pub fn get(&self) -> AppSettings {
        self.config.snapshot().settings
    }

    /// # Errors
    ///
    /// Fails if the language is not supported or saving fails.
    pub fn set_language(&self, language: &str) -> Result<AppSettings, AppError> {
        if !SUPPORTED_LANGUAGES.contains(&language) {
            return Err(AppError::new(ErrorCode::LanguageUnsupported).with("language", language));
        }
        self.config.modify(|config| {
            language.clone_into(&mut config.settings.language);
            Ok(config.settings.clone())
        })
    }

    /// The most tokens the runs of one specification may use together (`None`: no limit).
    ///
    /// # Errors
    ///
    /// Fails if saving fails.
    pub fn set_specs_max_tokens(&self, max: Option<u64>) -> Result<AppSettings, AppError> {
        self.config.modify(|config| {
            config.settings.optimization.specs_max_tokens = max.filter(|m| *m > 0);
            Ok(config.settings.clone())
        })
    }

    /// Which scopes of the Memory are recovered and proposed to, and how much a prompt may spend
    /// on them. All off is the default: it adds text to prompts.
    ///
    /// # Errors
    ///
    /// Fails if saving fails.
    pub fn set_memory_settings(
        &self,
        memory: crate::domain::memory::MemorySettings,
    ) -> Result<AppSettings, AppError> {
        self.config.modify(|config| {
            config.settings.optimization.memory = memory;
            Ok(config.settings.clone())
        })
    }

    /// The memory server on this machine that Atlas may read notes from (off by default).
    ///
    /// # Errors
    ///
    /// Fails if the server address is not a loopback one, or if saving fails.
    pub fn set_external_memory(
        &self,
        external: crate::domain::memory::ExternalMemorySettings,
    ) -> Result<AppSettings, AppError> {
        if external.enabled && external.loopback_authority().is_none() {
            return Err(AppError::new(ErrorCode::ExternalAddressInvalid));
        }
        let external = crate::domain::memory::ExternalMemorySettings {
            max_hits: external.max_hits.clamp(1, 10),
            max_tokens: external.max_tokens.clamp(50, 2000),
            ..external
        };
        self.config.modify(|config| {
            config.settings.external_memory = external;
            Ok(config.settings.clone())
        })
    }

    /// Whether, and how strictly, the runtime process of an execution is confined by the OS.
    ///
    /// # Errors
    ///
    /// Fails if saving fails.
    pub fn set_sandbox_settings(
        &self,
        sandbox: crate::domain::security::SandboxSettings,
    ) -> Result<AppSettings, AppError> {
        self.config.modify(|config| {
            config.settings.sandbox = sandbox;
            Ok(config.settings.clone())
        })
    }

    /// What Atlas shares on the person's Discord profile (off by default).
    ///
    /// # Errors
    ///
    /// Fails if saving fails.
    pub fn set_presence_settings(
        &self,
        presence: crate::domain::presence::PresenceSettings,
    ) -> Result<AppSettings, AppError> {
        self.config.modify(|config| {
            config.settings.presence = presence;
            Ok(config.settings.clone())
        })
    }

    /// Remembers which workspace is open (`None`: none).
    ///
    /// # Errors
    ///
    /// Fails if the workspace does not exist or saving fails.
    pub fn select_workspace(&self, workspace_id: Option<&str>) -> Result<AppSettings, AppError> {
        self.config.modify(|config| {
            if let Some(id) = workspace_id {
                if !config.workspaces.iter().any(|w| w.id == id) {
                    return Err(AppError::new(ErrorCode::WorkspaceNotFound));
                }
            }
            config.settings.selected_workspace_id = workspace_id.map(str::to_owned);
            Ok(config.settings.clone())
        })
    }

    /// Forgets the selection if it points at `workspace_id` (which is being deleted).
    ///
    /// # Errors
    ///
    /// Fails if saving fails.
    pub fn clear_selection_if(&self, workspace_id: &str) -> Result<(), AppError> {
        self.config.modify(|config| {
            if config.settings.selected_workspace_id.as_deref() == Some(workspace_id) {
                config.settings.selected_workspace_id = None;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::application::config::memory::MemoryStore;
    use crate::domain::workspace::{Workspace, WorkspaceLayout};

    fn service_with_workspace() -> (SettingsService, Arc<MemoryStore>) {
        let store = Arc::new(MemoryStore::default());
        let config = Arc::new(ConfigRepository::load(Box::new(store.clone())));
        config
            .modify(|c| {
                c.workspaces.push(Workspace {
                    id: "w1".to_owned(),
                    name: "W".to_owned(),
                    project_path: "/p".to_owned(),
                    projects: Vec::new(),
                    description: None,
                    created_at: 1,
                    updated_at: 1,
                    layout: WorkspaceLayout {
                        rows: 2,
                        columns: 2,
                        agent_placements: vec![],
                    },
                    security: crate::domain::security::SecurityPolicy::default(),
                });
                Ok(())
            })
            .unwrap();
        (SettingsService::new(config), store)
    }

    #[test]
    fn defaults_to_portuguese_with_no_workspace_selected() {
        let (service, _) = service_with_workspace();

        assert_eq!(
            service.get(),
            AppSettings {
                language: "pt-BR".to_owned(),
                selected_workspace_id: None,
                optimization: crate::application::config::OptimizationSettings::default(),
                sandbox: crate::domain::security::SandboxSettings::default(),
                presence: crate::domain::presence::PresenceSettings::default(),
                external_memory: crate::domain::memory::ExternalMemorySettings::default(),
            }
        );
    }

    #[test]
    fn the_token_limit_of_a_spec_persists_and_zero_means_none() {
        let (service, store) = service_with_workspace();
        assert_eq!(service.get().optimization.specs_max_tokens, None);

        service.set_specs_max_tokens(Some(250_000)).unwrap();
        let restarted = SettingsService::new(Arc::new(ConfigRepository::load(Box::new(store))));
        assert_eq!(restarted.get().optimization.specs_max_tokens, Some(250_000));

        restarted.set_specs_max_tokens(Some(0)).unwrap();
        assert_eq!(restarted.get().optimization.specs_max_tokens, None);
    }

    #[test]
    fn language_and_selected_workspace_persist() {
        let (service, store) = service_with_workspace();

        service.set_language("en-US").unwrap();
        service.select_workspace(Some("w1")).unwrap();

        let restarted = SettingsService::new(Arc::new(ConfigRepository::load(Box::new(store))));
        assert_eq!(restarted.get().language, "en-US");
        assert_eq!(restarted.get().selected_workspace_id.as_deref(), Some("w1"));
    }

    #[test]
    fn rejects_unsupported_languages_and_unknown_workspaces() {
        let (service, _) = service_with_workspace();

        assert!(service
            .set_language("fr-FR")
            .unwrap_err()
            .is(ErrorCode::LanguageUnsupported));
        assert!(service
            .select_workspace(Some("ghost"))
            .unwrap_err()
            .is(ErrorCode::WorkspaceNotFound));
        assert_eq!(service.get().language, "pt-BR");
    }

    #[test]
    fn clears_the_selection_only_for_the_deleted_workspace() {
        let (service, _) = service_with_workspace();
        service.select_workspace(Some("w1")).unwrap();

        service.clear_selection_if("other").unwrap();
        assert_eq!(service.get().selected_workspace_id.as_deref(), Some("w1"));
        service.clear_selection_if("w1").unwrap();
        assert_eq!(service.get().selected_workspace_id, None);
    }
}
