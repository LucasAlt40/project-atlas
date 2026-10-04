mod builtin;

use std::sync::Arc;

use serde::Deserialize;

use super::config::ConfigRepository;
use super::errors::{AppError, ErrorCode};
use super::support::new_id;
use crate::domain::personality::{PersonalityProfile, PersonalitySource};
use crate::domain::result_contract::ResultContract;

use builtin::builtin_personalities;

const MAX_NAME_LEN: usize = 80;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePersonalityRequest {
    pub name: String,
    pub description: String,
    pub system_instructions: String,
    pub tags: Vec<String>,
}

/// Use case: built-in presets (which the user can edit or remove) plus custom personalities.
pub struct PersonalityService {
    config: Arc<ConfigRepository>,
}

impl PersonalityService {
    pub fn new(config: Arc<ConfigRepository>) -> Self {
        Self { config }
    }

    /// Built-ins first (as edited by the user, without the ones they removed), then custom
    /// ones in creation order.
    pub fn list(&self) -> Vec<PersonalityProfile> {
        let config = self.config.snapshot();
        let mut all: Vec<_> = builtin_personalities()
            .into_iter()
            .filter(|p| !config.hidden_builtins.contains(&p.id))
            .map(|p| {
                config
                    .builtin_overrides
                    .iter()
                    .find(|o| o.id == p.id)
                    .cloned()
                    .unwrap_or(p)
            })
            .collect();
        all.extend(config.personalities);
        all
    }

    pub fn find(&self, id: &str) -> Option<PersonalityProfile> {
        self.list().into_iter().find(|p| p.id == id)
    }

    /// # Errors
    ///
    /// Fails if the name or instructions are empty or the personality cannot be saved.
    pub fn create(
        &self,
        request: &CreatePersonalityRequest,
    ) -> Result<PersonalityProfile, AppError> {
        let fields = Fields::from_request(request)?;
        let personality = PersonalityProfile {
            id: new_id("custom"),
            name: fields.name,
            description: fields.description,
            system_instructions: fields.instructions,
            behavior: Vec::new(),
            tags: fields.tags,
            source: PersonalitySource::Custom,
            suggested_contract: ResultContract::general(),
        };
        self.config.add_personality(personality.clone())?;
        Ok(personality)
    }

    /// Replaces a personality's name, description, instructions and tags. Editing a built-in
    /// keeps a changed copy next to it (the built-in itself ships with the app); its behaviour
    /// summary is dropped because it described the original instructions. Agents using the
    /// personality pick up the change on their next run.
    ///
    /// # Errors
    ///
    /// Fails if the fields are invalid, the personality does not exist, or saving fails.
    pub fn update(
        &self,
        id: &str,
        request: &CreatePersonalityRequest,
    ) -> Result<PersonalityProfile, AppError> {
        let fields = Fields::from_request(request)?;
        let current = self.find(id).ok_or_else(not_found)?;
        let updated = PersonalityProfile {
            id: current.id.clone(),
            name: fields.name,
            description: fields.description,
            system_instructions: fields.instructions,
            behavior: Vec::new(),
            tags: fields.tags,
            source: current.source,
            suggested_contract: current.suggested_contract.clone(),
        };
        self.config.modify(|config| {
            match current.source {
                PersonalitySource::Custom => {
                    let slot = config
                        .personalities
                        .iter_mut()
                        .find(|p| p.id == id)
                        .ok_or_else(not_found)?;
                    *slot = updated.clone();
                }
                PersonalitySource::Builtin => {
                    config.builtin_overrides.retain(|p| p.id != id);
                    config.builtin_overrides.push(updated.clone());
                }
            }
            Ok(())
        })?;
        Ok(updated)
    }

    /// Removes a personality. A built-in is hidden (it can be brought back with
    /// [`Self::restore_defaults`]). A personality that agents still use cannot be removed.
    ///
    /// # Errors
    ///
    /// Fails if the personality does not exist, is used by agents, or saving fails.
    pub fn delete(&self, id: &str) -> Result<(), AppError> {
        let current = self.find(id).ok_or_else(not_found)?;
        let users: Vec<_> = self
            .config
            .agents()
            .into_iter()
            .filter(|agent| agent.personality_id == id)
            .map(|agent| agent.name)
            .collect();
        if !users.is_empty() {
            return Err(AppError::new(ErrorCode::PersonalityInUse)
                .with("name", current.name)
                .with("agents", users.join(", ")));
        }
        self.config.modify(|config| {
            match current.source {
                PersonalitySource::Custom => config.personalities.retain(|p| p.id != id),
                PersonalitySource::Builtin => {
                    config.builtin_overrides.retain(|p| p.id != id);
                    config.hidden_builtins.push(id.to_owned());
                }
            }
            Ok(())
        })
    }

    /// Brings back every removed built-in personality and undoes edits to them. Custom
    /// personalities are untouched.
    ///
    /// # Errors
    ///
    /// Fails if saving fails.
    pub fn restore_defaults(&self) -> Result<Vec<PersonalityProfile>, AppError> {
        self.config.modify(|config| {
            config.builtin_overrides.clear();
            config.hidden_builtins.clear();
            Ok(())
        })?;
        Ok(self.list())
    }
}

fn not_found() -> AppError {
    AppError::new(ErrorCode::PersonalityNotFound)
}

/// The validated, cleaned-up fields of a create/update request.
struct Fields {
    name: String,
    description: String,
    instructions: String,
    tags: Vec<String>,
}

impl Fields {
    fn from_request(request: &CreatePersonalityRequest) -> Result<Self, AppError> {
        let name = request.name.trim();
        if name.is_empty() {
            return Err(AppError::new(ErrorCode::NameRequired));
        }
        if name.chars().count() > MAX_NAME_LEN {
            return Err(AppError::new(ErrorCode::NameTooLong).with("max", MAX_NAME_LEN.to_string()));
        }
        let instructions = request.system_instructions.trim();
        if instructions.is_empty() {
            return Err(AppError::new(ErrorCode::InstructionsRequired));
        }
        let mut tags: Vec<String> = Vec::new();
        for tag in request.tags.iter().map(|t| t.trim().to_lowercase()) {
            if !tag.is_empty() && !tags.contains(&tag) {
                tags.push(tag);
            }
        }
        Ok(Self {
            name: name.to_owned(),
            description: request.description.trim().to_owned(),
            instructions: instructions.to_owned(),
            tags,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::config::memory::MemoryStore;

    fn service() -> PersonalityService {
        PersonalityService::new(Arc::new(ConfigRepository::load(
            Box::<MemoryStore>::default(),
        )))
    }

    fn request(name: &str, instructions: &str) -> CreatePersonalityRequest {
        CreatePersonalityRequest {
            name: name.to_owned(),
            description: " Clean Architecture ".to_owned(),
            system_instructions: instructions.to_owned(),
            tags: vec![
                " DDD ".to_owned(),
                "ddd".to_owned(),
                String::new(),
                ".NET".to_owned(),
            ],
        }
    }

    #[test]
    fn ships_architect_developer_and_qa_with_visible_instructions() {
        let all = service().list();

        let ids: Vec<_> = all.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "architect",
                "developer",
                "qa",
                "architecture-validator",
                "bug-fixer"
            ]
        );
        assert!(all.iter().all(|p| p.source == PersonalitySource::Builtin));
        assert!(all
            .iter()
            .all(|p| !p.system_instructions.is_empty() && !p.behavior.is_empty()));
    }

    #[test]
    fn creates_and_lists_custom_personalities() {
        let service = service();

        let created = service
            .create(&request(" Senior .NET Architect ", "You are..."))
            .unwrap();

        assert_eq!(created.name, "Senior .NET Architect");
        assert_eq!(created.description, "Clean Architecture");
        assert_eq!(created.tags, ["ddd", ".net"]);
        assert_eq!(created.source, PersonalitySource::Custom);
        assert_eq!(service.list().last(), Some(&created));
        assert_eq!(service.find(&created.id), Some(created));
    }

    #[test]
    fn rejects_blank_name_or_instructions() {
        let service = service();

        assert!(service
            .create(&request("  ", "x"))
            .unwrap_err()
            .is(ErrorCode::NameRequired));
        assert!(service
            .create(&request("N", " "))
            .unwrap_err()
            .is(ErrorCode::InstructionsRequired));
        assert_eq!(service.list().len(), 5);
    }

    fn input(name: &str) -> CreatePersonalityRequest {
        request(name, "New instructions")
    }

    fn service_with_agent(personality_id: &str) -> PersonalityService {
        use crate::domain::agent::Agent;
        let config = Arc::new(ConfigRepository::load(Box::<MemoryStore>::default()));
        config
            .add_agent(Agent {
                id: "a1".to_owned(),
                name: "My agent".to_owned(),
                personality_id: personality_id.to_owned(),
                runtime_id: "x".to_owned(),
                model_id: "m".to_owned(),
                instructions: String::new(),
                permission_profile_id: None,
                worktree_isolation: false,
                result_contract: crate::domain::result_contract::ResultContract::default(),
                created_at: 1,
            })
            .unwrap();
        PersonalityService::new(config)
    }

    #[test]
    fn edits_a_custom_personality_in_place() {
        let service = service();
        let created = service.create(&request("Old", "Old text")).unwrap();

        let updated = service.update(&created.id, &input("  New name ")).unwrap();

        assert_eq!(updated.id, created.id);
        assert_eq!(updated.name, "New name");
        assert_eq!(updated.system_instructions, "New instructions");
        assert_eq!(updated.source, PersonalitySource::Custom);
        assert_eq!(service.find(&created.id), Some(updated));
        assert_eq!(service.list().len(), 6);
    }

    #[test]
    fn editing_a_builtin_saves_a_changed_copy_in_the_same_place() {
        let service = service();

        let updated = service.update("developer", &input("My Developer")).unwrap();

        assert_eq!(updated.source, PersonalitySource::Builtin);
        assert!(
            updated.behavior.is_empty(),
            "the old behaviour summary no longer applies"
        );
        let names: Vec<_> = service.list().into_iter().map(|p| p.name).collect();
        assert_eq!(
            names,
            [
                "Architect",
                "My Developer",
                "QA",
                "Architecture & Code Validator",
                "Bug Fixer"
            ]
        );
        // Editing again replaces the copy instead of stacking another one.
        service.update("developer", &input("Again")).unwrap();
        assert_eq!(service.list().len(), 5);
        assert_eq!(service.find("developer").unwrap().name, "Again");
    }

    #[test]
    fn rejects_invalid_or_unknown_edits_without_changing_anything() {
        let service = service();

        assert!(service
            .update("qa", &request(" ", "x"))
            .unwrap_err()
            .is(ErrorCode::NameRequired));
        assert!(service
            .update("qa", &request("n", " "))
            .unwrap_err()
            .is(ErrorCode::InstructionsRequired));
        assert!(service
            .update("ghost", &input("n"))
            .unwrap_err()
            .is(ErrorCode::PersonalityNotFound));
        assert_eq!(service.find("qa").unwrap().name, "QA");
    }

    #[test]
    fn deletes_a_custom_personality() {
        let service = service();
        let created = service.create(&request("Mine", "x")).unwrap();

        service.delete(&created.id).unwrap();

        assert!(service.find(&created.id).is_none());
        assert_eq!(service.list().len(), 5);
        assert!(service
            .delete(&created.id)
            .unwrap_err()
            .is(ErrorCode::PersonalityNotFound));
    }

    #[test]
    fn deleting_a_builtin_hides_it_and_restore_brings_every_default_back() {
        let service = service();
        service.update("architect", &input("Edited")).unwrap();

        service.delete("qa").unwrap();
        service.delete("architect").unwrap();
        let ids: Vec<_> = service.list().into_iter().map(|p| p.id).collect();
        assert_eq!(ids, ["developer", "architecture-validator", "bug-fixer"]);

        let restored = service.restore_defaults().unwrap();

        let names: Vec<_> = restored.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Architect",
                "Developer",
                "QA",
                "Architecture & Code Validator",
                "Bug Fixer"
            ],
            "removed ones are back and edits are undone"
        );
    }

    #[test]
    fn restoring_defaults_keeps_custom_personalities() {
        let service = service();
        let mine = service.create(&request("Mine", "x")).unwrap();
        service.delete("qa").unwrap();

        service.restore_defaults().unwrap();

        assert!(service.find(&mine.id).is_some());
        assert_eq!(service.list().len(), 6);
    }

    #[test]
    fn refuses_to_delete_a_personality_that_agents_use() {
        let service = service_with_agent("qa");

        let error = service.delete("qa").unwrap_err();

        assert!(error.is(ErrorCode::PersonalityInUse));
        assert_eq!(error.params["name"], "QA");
        assert_eq!(error.params["agents"], "My agent");
        assert!(service.find("qa").is_some());
        // An unused one can still go.
        service.delete("developer").unwrap();
    }
}
