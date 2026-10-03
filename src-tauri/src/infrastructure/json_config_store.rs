use std::fs;
use std::io::Write;
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
            // Keep the unreadable file instead of overwriting it with the next save, and keep an
            // earlier one too: a second corruption must not destroy the first backup.
            let mut backup = self.path.with_extension("json.corrupt");
            let mut attempt = 1;
            while backup.exists() {
                backup = self.path.with_extension(format!("json.corrupt.{attempt}"));
                attempt += 1;
            }
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
        // Write, flush to disk, then rename: a crash never leaves a half-written config, and
        // never a renamed file whose contents had not reached the disk yet.
        let temp = self.path.with_extension("json.tmp");
        let mut file = fs::File::create(&temp).map_err(|error| error.to_string())?;
        file.write_all(text.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|error| error.to_string())?;
        drop(file);
        fs::rename(&temp, &self.path).map_err(|error| error.to_string())?;
        // Best effort: make the rename itself durable (a folder cannot be opened this way on
        // every platform).
        #[cfg(unix)]
        if let Some(parent) = self.path.parent() {
            if let Ok(dir) = fs::File::open(parent) {
                let _ = dir.sync_all();
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::config::AppSettings;
    use crate::domain::agent::Agent;
    use crate::domain::personality::{PersonalityProfile, PersonalitySource};
    use crate::domain::workspace::{Workspace, WorkspaceLayout};
    use crate::domain::worktree::{
        BlockReason, ExecutionWorktree, MergeStatus, Recommendation, Validation, WorktreeStatus,
    };

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
                permission_profile_id: None,
                worktree_isolation: false,
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
                security: crate::domain::security::SecurityPolicy::developer(),
            }],
            settings: AppSettings {
                language: "en-US".to_owned(),
                selected_workspace_id: Some("ws-1".to_owned()),
            },
            worktrees: vec![ExecutionWorktree {
                execution_id: "exec-42".to_owned(),
                workspace_id: "ws-1".to_owned(),
                agent_id: "a".to_owned(),
                base_branch: "main".to_owned(),
                base_commit: "a".repeat(40),
                branch_name: "atlas/exec-000042".to_owned(),
                worktree_path: "/data/worktrees/ws-1/exec-000042".to_owned(),
                working_dir: "/data/worktrees/ws-1/exec-000042".to_owned(),
                repository_path: "/p".to_owned(),
                status: WorktreeStatus::Completed,
                merge_status: MergeStatus::Pending,
                block_reason: Some(BlockReason::BaseDirty),
                base_dirty_at_start: true,
                created_at: 7,
                changes: None,
                validation: Validation::NotRun,
                recommendation: Some(Recommendation::Review),
            }],
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

    #[test]
    fn a_second_corruption_does_not_destroy_the_first_backup() {
        let path = temp_path("corrupt-twice");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let store = JsonConfigStore::new(path.clone());

        fs::write(&path, "first { bad").unwrap();
        assert!(store.load().is_err());
        fs::write(&path, "second { bad").unwrap();
        assert!(store.load().is_err());

        assert_eq!(
            fs::read_to_string(path.with_extension("json.corrupt")).unwrap(),
            "first { bad"
        );
        assert_eq!(
            fs::read_to_string(path.with_extension("json.corrupt.1")).unwrap(),
            "second { bad"
        );
    }

    #[test]
    fn saving_never_leaves_a_temporary_file_and_replaces_the_whole_config() {
        let path = temp_path("replace");
        let store = JsonConfigStore::new(path.clone());
        let mut config = UserConfig::default();
        config.settings.language = "en-US".to_owned();
        store.save(&config).unwrap();
        config.settings.language = "pt-BR".to_owned();

        store.save(&config).unwrap();

        assert!(!path.with_extension("json.tmp").exists());
        assert_eq!(store.load().unwrap().settings.language, "pt-BR");
    }
}
