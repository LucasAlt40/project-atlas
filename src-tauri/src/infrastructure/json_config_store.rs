use std::fs;
use std::path::PathBuf;

use crate::application::config::{ConfigStore, UserConfig};

/// Stores the user config as one pretty-printed JSON file.
pub struct JsonConfigStore {
    path: PathBuf,
}

impl JsonConfigStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl ConfigStore for JsonConfigStore {
    fn load(&self) -> Result<UserConfig, String> {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(UserConfig::default());
            }
            Err(error) => return Err(format!("{}: {error}", self.path.display())),
        };
        serde_json::from_str(&text).map_err(|error| {
            // Keep the unreadable file instead of overwriting it with the next save.
            let backup = self.path.with_extension("json.corrupt");
            let _ = fs::rename(&self.path, &backup);
            format!(
                "{} is not valid ({error}); moved to {}",
                self.path.display(),
                backup.display()
            )
        })
    }

    fn save(&self, config: &UserConfig) -> Result<(), String> {
        let text = serde_json::to_string_pretty(config).map_err(|error| error.to_string())?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        // Write then rename so a crash never leaves a half-written config.
        let temp = self.path.with_extension("json.tmp");
        fs::write(&temp, text).map_err(|error| error.to_string())?;
        fs::rename(&temp, &self.path).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::config::AppSettings;
    use crate::domain::agent::Agent;
    use crate::domain::personality::{PersonalityProfile, PersonalitySource};
    use crate::domain::workspace::{Workspace, WorkspaceLayout};

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("atlas-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join("config.json")
    }

    #[test]
    fn missing_file_loads_as_empty_and_saved_data_round_trips() {
        let path = temp_path("roundtrip");
        let store = JsonConfigStore::new(path.clone());
        assert_eq!(store.load().unwrap(), UserConfig::default());

        let config = UserConfig {
            personalities: vec![],
            agents: vec![Agent {
                id: "a".to_owned(),
                name: "A".to_owned(),
                personality_id: "architect".to_owned(),
                runtime_id: "opencode".to_owned(),
                model_id: "m".to_owned(),
                instructions: "i".to_owned(),
                created_at: 5,
            }],
            builtin_overrides: vec![PersonalityProfile {
                id: "architect".to_owned(),
                name: "My Architect".to_owned(),
                description: String::new(),
                system_instructions: "x".to_owned(),
                behavior: vec![],
                tags: vec![],
                source: PersonalitySource::Builtin,
            }],
            hidden_builtins: vec!["qa".to_owned()],
            workspaces: vec![Workspace {
                id: "ws-1".to_owned(),
                name: "Atlas".to_owned(),
                project_path: "/p".to_owned(),
                description: Some("d".to_owned()),
                created_at: 1,
                updated_at: 2,
                layout: WorkspaceLayout {
                    rows: 2,
                    columns: 2,
                    agent_placements: vec![],
                },
            }],
            settings: AppSettings {
                language: "en-US".to_owned(),
                selected_workspace_id: Some("ws-1".to_owned()),
            },
            ..UserConfig::default()
        };
        store.save(&config).unwrap();

        assert_eq!(JsonConfigStore::new(path).load().unwrap(), config);
    }

    #[test]
    fn corrupt_files_are_set_aside_not_overwritten() {
        let path = temp_path("corrupt");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{ nope").unwrap();
        let store = JsonConfigStore::new(path.clone());

        assert!(store.load().is_err());

        assert!(!path.exists());
        assert!(path.with_extension("json.corrupt").exists());
        assert_eq!(store.load().unwrap(), UserConfig::default());
    }
}
