use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde_json::{Map, Value};

use super::atomic_file;
use crate::application::config::{ConfigStore, UserConfig};
use crate::application::support::LockExt;
use crate::domain::app_info::{ConfigNotice, DroppedRecords};

/// Stores the user config as one pretty-printed JSON file.
pub struct JsonConfigStore {
    path: PathBuf,
    /// Set when the file exists but could not be read or set aside: saving would replace the
    /// person's real config with an empty one, so nothing is saved until the next start.
    frozen: AtomicBool,
    notice: Mutex<Option<ConfigNotice>>,
}

impl JsonConfigStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            frozen: AtomicBool::new(false),
            notice: Mutex::new(None),
        }
    }

    fn note(&self, notice: ConfigNotice) {
        *self.notice.lock_or_recover() = Some(notice);
    }

    /// Freezes saving and says why: the file is left exactly as it is.
    fn freeze(&self, detail: String) -> String {
        self.frozen.store(true, Ordering::SeqCst);
        self.note(ConfigNotice::Unreadable {
            path: self.path.display().to_string(),
        });
        detail
    }

    /// Moves the file Atlas could not read to a backup next to it, never over an earlier backup.
    fn set_aside(&self) -> std::io::Result<PathBuf> {
        let mut backup = self.path.with_extension("json.corrupt");
        let mut attempt = 1;
        while backup.exists() {
            backup = self.path.with_extension(format!("json.corrupt.{attempt}"));
            attempt += 1;
        }
        fs::rename(&self.path, &backup)?;
        Ok(backup)
    }
}

impl ConfigStore for JsonConfigStore {
    fn load(&self) -> Result<UserConfig, String> {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(UserConfig::default());
            }
            Err(error) => return Err(self.freeze(format!("{}: {error}", self.path.display()))),
        };
        let error = match serde_json::from_str(&text) {
            Ok(config) => return Ok(config),
            Err(error) => error,
        };
        // Keep the unreadable file instead of overwriting it with the next save, and keep an
        // earlier one too: a second corruption must not destroy the first backup.
        let backup = match self.set_aside() {
            Ok(backup) => backup,
            Err(rename) => {
                return Err(self.freeze(format!(
                    "{} is not valid ({error}) and could not be set aside ({rename})",
                    self.path.display()
                )))
            }
        };
        let shown = backup.display().to_string();
        // Valid JSON that this version cannot read whole (a newer or older Atlas wrote it): keep
        // every record that can be read, and leave out only the ones that cannot.
        if let Ok(value) = serde_json::from_str::<Value>(&text) {
            if let Some((config, dropped)) = recover(value) {
                self.note(ConfigNotice::Recovered {
                    backup: shown,
                    dropped,
                });
                return Ok(config);
            }
        }
        self.note(ConfigNotice::Reset {
            backup: shown.clone(),
        });
        Err(format!(
            "{} is not valid ({error}); moved to {shown}",
            self.path.display()
        ))
    }

    fn save(&self, config: &UserConfig) -> Result<(), String> {
        if self.frozen.load(Ordering::SeqCst) {
            return Err(format!(
                "{} could not be read at start; it is not overwritten",
                self.path.display()
            ));
        }
        let text = serde_json::to_string_pretty(config).map_err(|error| error.to_string())?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        // A crash never leaves a half-written config.
        atomic_file::write(&self.path, text.as_bytes()).map_err(|error| error.to_string())
    }

    fn notice(&self) -> Option<ConfigNotice> {
        self.notice.lock_or_recover().clone()
    }
}

/// The config in `value`, with every record that cannot be read on its own left out. `None` when
/// `value` is not a config object at all.
fn recover(value: Value) -> Option<(UserConfig, Vec<DroppedRecords>)> {
    let Value::Object(fields) = value else {
        return None;
    };
    let readable = |key: &str, field: Value| {
        serde_json::from_value::<UserConfig>(Value::Object(Map::from_iter([(
            key.to_owned(),
            field,
        )])))
        .is_ok()
    };
    let mut kept = Map::new();
    let mut dropped = Vec::new();
    for (key, field) in fields {
        match field {
            Value::Array(items) => {
                let total = items.len();
                let good: Vec<Value> = items
                    .into_iter()
                    .filter(|item| readable(&key, Value::Array(vec![item.clone()])))
                    .collect();
                if good.len() < total {
                    dropped.push(DroppedRecords {
                        section: key.clone(),
                        count: total - good.len(),
                    });
                }
                kept.insert(key, Value::Array(good));
            }
            other => {
                if readable(&key, other.clone()) {
                    kept.insert(key, other);
                } else {
                    dropped.push(DroppedRecords {
                        section: key,
                        count: 1,
                    });
                }
            }
        }
    }
    let config = serde_json::from_value(Value::Object(kept)).ok()?;
    Some((config, dropped))
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
                result_contract: crate::domain::result_contract::ResultContract::default(),
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
                suggested_contract: crate::domain::result_contract::ResultContract::default(),
                suggested_permission_profile: "developer".to_owned(),
            }],
            hidden_builtins: vec!["qa".to_owned()],
            workspaces: vec![Workspace {
                id: "ws-1".to_owned(),
                name: "Atlas".to_owned(),
                project_path: "/p".to_owned(),
                projects: Vec::new(),
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
                optimization: crate::application::config::OptimizationSettings::default(),
                sandbox: crate::domain::security::SandboxSettings::default(),
                presence: crate::domain::presence::PresenceSettings::default(),
                question_timeout_minutes: None,
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
                workflow_execution_id: None,
                shared_with: None,
                end_commit: None,
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
    fn records_another_version_wrote_are_left_out_and_the_rest_is_kept() {
        let path = temp_path("other-version");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let config = UserConfig {
            hidden_builtins: vec!["a".to_owned(), "b".to_owned()],
            settings: AppSettings {
                language: "en-US".to_owned(),
                ..AppSettings::default()
            },
            ..UserConfig::default()
        };
        let mut value = serde_json::to_value(&config).unwrap();
        // A string list holding a number stands for a record this version cannot read (an enum
        // variant it does not know, a field of another type).
        value["hiddenBuiltins"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!(42));
        fs::write(&path, value.to_string()).unwrap();
        let store = JsonConfigStore::new(path.clone());

        let loaded = store.load().unwrap();

        assert_eq!(loaded.hidden_builtins, ["a", "b"]);
        assert_eq!(loaded.settings.language, "en-US");
        assert!(path.with_extension("json.corrupt").exists());
        match store.notice() {
            Some(ConfigNotice::Recovered { dropped, .. }) => assert_eq!(
                dropped,
                [DroppedRecords {
                    section: "hiddenBuiltins".to_owned(),
                    count: 1
                }]
            ),
            other => panic!("expected a recovery notice, got {other:?}"),
        }
        store.save(&loaded).unwrap();
    }

    #[test]
    fn a_config_that_cannot_be_read_is_never_overwritten() {
        let path = temp_path("unreadable");
        // A folder where the file should be: reading fails with something other than NotFound.
        fs::create_dir_all(&path).unwrap();
        let store = JsonConfigStore::new(path.clone());

        assert!(store.load().is_err());

        assert!(store.save(&UserConfig::default()).is_err());
        assert!(path.is_dir());
        assert!(matches!(
            store.notice(),
            Some(ConfigNotice::Unreadable { .. })
        ));
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
