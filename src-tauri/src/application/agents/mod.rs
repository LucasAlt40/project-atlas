use std::sync::Arc;

use serde::Deserialize;

use super::config::ConfigRepository;
use super::errors::{AppError, ErrorCode};
use super::personalities::PersonalityService;
use super::runtimes::RuntimeRegistry;
use super::support::{new_id, now_ms};
use crate::domain::agent::Agent;
use crate::domain::result_contract::{ContractError, ResultContract};
use crate::domain::security::PermissionProfile;

const MAX_NAME_LEN: usize = 80;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAgentRequest {
    pub name: String,
    pub personality_id: String,
    pub runtime_id: String,
    pub model_id: String,
    pub instructions: String,
    /// Whether executions run in their own Git worktree. Left out, a new agent gets `true` and
    /// an edited agent keeps what it had.
    #[serde(default)]
    pub worktree_isolation: Option<bool>,
    /// What the agent promises to say at the end of a step. Left out, a new agent is `general`
    /// and an edited agent keeps what it had.
    #[serde(default)]
    pub result_contract: Option<ResultContract>,
    /// The permission profile (`developer`, `read_only`). Left out, a new agent starts from what
    /// its personality suggests and an edited agent keeps what it had.
    #[serde(default)]
    pub permission_profile_id: Option<String>,
}

/// Use case: create agents from a personality + provider + model + instructions, and
/// remember them.
pub struct AgentService {
    config: Arc<ConfigRepository>,
    personalities: Arc<PersonalityService>,
    runtimes: Arc<RuntimeRegistry>,
}

impl AgentService {
    pub fn new(
        config: Arc<ConfigRepository>,
        personalities: Arc<PersonalityService>,
        runtimes: Arc<RuntimeRegistry>,
    ) -> Self {
        Self {
            config,
            personalities,
            runtimes,
        }
    }

    pub fn list(&self) -> Vec<Agent> {
        self.config.agents()
    }

    pub fn find(&self, id: &str) -> Option<Agent> {
        self.list().into_iter().find(|agent| agent.id == id)
    }

    /// # Errors
    ///
    /// Fails if the name or model is empty, the personality or runtime is unknown, the
    /// runtime cannot run tasks yet, or the agent cannot be saved.
    pub fn create(&self, request: CreateAgentRequest) -> Result<Agent, AppError> {
        let (name, model_id) = self.validate(&request)?;
        let result_contract = match request.result_contract.clone() {
            Some(contract) => Self::checked_contract(contract)?,
            None => ResultContract::general(),
        };
        let profile = self.starting_profile(&request)?;
        let agent = Agent {
            id: new_id("agent"),
            name,
            personality_id: request.personality_id,
            runtime_id: request.runtime_id,
            model_id,
            instructions: request.instructions.trim().to_owned(),
            // Always explicit: an unset profile means the most restrictive one.
            permission_profile_id: Some(profile.id().to_owned()),
            worktree_isolation: request.worktree_isolation.unwrap_or(true),
            result_contract,
            created_at: now_ms(),
        };
        self.config.add_agent(agent.clone())?;
        Ok(agent)
    }

    /// Changes everything about an agent except its identity and creation time. Its
    /// conversation and workspace position are unaffected.
    ///
    /// # Errors
    ///
    /// Fails if the agent does not exist, the new settings are invalid, or saving fails.
    pub fn update(&self, id: &str, request: CreateAgentRequest) -> Result<Agent, AppError> {
        let (name, model_id) = self.validate(&request)?;
        let result_contract = request
            .result_contract
            .clone()
            .map(Self::checked_contract)
            .transpose()?;
        let profile = request
            .permission_profile_id
            .as_deref()
            .map(Self::checked_profile)
            .transpose()?;
        self.config.modify(|config| {
            let agent = config
                .agents
                .iter_mut()
                .find(|a| a.id == id)
                .ok_or_else(|| AppError::new(ErrorCode::AgentNotFound))?;
            agent.name = name;
            agent.personality_id = request.personality_id;
            agent.runtime_id = request.runtime_id;
            agent.model_id = model_id;
            request
                .instructions
                .trim()
                .clone_into(&mut agent.instructions);
            if let Some(isolation) = request.worktree_isolation {
                agent.worktree_isolation = isolation;
            }
            if let Some(contract) = result_contract {
                agent.result_contract = contract;
            }
            if let Some(profile) = profile {
                agent.permission_profile_id = Some(profile.id().to_owned());
            }
            Ok(agent.clone())
        })
    }

    /// Points the agent at another permission profile. Profiles are defined by Atlas; the agent
    /// only names one, and the workspace's policy still bounds what it grants.
    ///
    /// # Errors
    ///
    /// Fails if the profile is unknown, the agent does not exist, or saving fails.
    pub fn set_permission_profile(&self, id: &str, profile_id: &str) -> Result<Agent, AppError> {
        let profile = PermissionProfile::parse(profile_id)
            .ok_or_else(|| AppError::new(ErrorCode::PermissionProfileInvalid))?;
        self.config.modify(|config| {
            let agent = config
                .agents
                .iter_mut()
                .find(|a| a.id == id)
                .ok_or_else(|| AppError::new(ErrorCode::AgentNotFound))?;
            agent.permission_profile_id = Some(profile.id().to_owned());
            Ok(agent.clone())
        })
    }

    /// Sets what the agent promises to say at the end of a step. The contract only describes the
    /// result a workflow can route on: it grants nothing.
    ///
    /// # Errors
    ///
    /// Fails if the contract is malformed, the agent does not exist, or saving fails.
    pub fn set_result_contract(
        &self,
        id: &str,
        contract: ResultContract,
    ) -> Result<Agent, AppError> {
        let contract = Self::checked_contract(contract)?;
        self.config.modify(|config| {
            let agent = config
                .agents
                .iter_mut()
                .find(|a| a.id == id)
                .ok_or_else(|| AppError::new(ErrorCode::AgentNotFound))?;
            agent.result_contract = contract;
            Ok(agent.clone())
        })
    }

    /// Deletes the agent's configuration only; see `AgentLifecycle` (in `application::lifecycle`) for the clean-up that
    /// goes with it.
    ///
    /// # Errors
    ///
    /// Fails if the agent does not exist or saving fails.
    pub fn delete(&self, id: &str) -> Result<(), AppError> {
        self.config.modify(|config| {
            let before = config.agents.len();
            config.agents.retain(|a| a.id != id);
            if config.agents.len() == before {
                return Err(AppError::new(ErrorCode::AgentNotFound));
            }
            Ok(())
        })
    }

    fn checked_contract(contract: ResultContract) -> Result<ResultContract, AppError> {
        contract.normalized().map_err(|error| {
            AppError::new(ErrorCode::ResultContractInvalid).with(
                "reason",
                match error {
                    ContractError::InvalidId => "invalid_id",
                    ContractError::DuplicateId => "duplicate_id",
                    ContractError::EmptyLabel => "empty_label",
                    ContractError::TooMany => "too_many",
                    ContractError::WrongOutcomeCount => "wrong_outcome_count",
                },
            )
        })
    }

    fn checked_profile(id: &str) -> Result<PermissionProfile, AppError> {
        PermissionProfile::parse(id)
            .ok_or_else(|| AppError::new(ErrorCode::PermissionProfileInvalid))
    }

    /// The profile a new agent starts with: the one asked for, else what its personality
    /// suggests, else the developer profile. Never "unset", which means read only.
    fn starting_profile(
        &self,
        request: &CreateAgentRequest,
    ) -> Result<PermissionProfile, AppError> {
        if let Some(id) = &request.permission_profile_id {
            return Self::checked_profile(id);
        }
        Ok(self
            .personalities
            .find(&request.personality_id)
            .and_then(|p| PermissionProfile::parse(&p.suggested_permission_profile))
            .unwrap_or(PermissionProfile::Developer))
    }

    /// Checks a create/update request and returns the cleaned name and model id.
    fn validate(&self, request: &CreateAgentRequest) -> Result<(String, String), AppError> {
        let name = request.name.trim();
        if name.is_empty() {
            return Err(AppError::new(ErrorCode::NameRequired));
        }
        if name.chars().count() > MAX_NAME_LEN {
            return Err(AppError::new(ErrorCode::NameTooLong).with("max", MAX_NAME_LEN.to_string()));
        }
        if self.personalities.find(&request.personality_id).is_none() {
            return Err(AppError::new(ErrorCode::PersonalityRequired));
        }
        let runtime = self
            .runtimes
            .find(&request.runtime_id)
            .ok_or_else(|| AppError::new(ErrorCode::RuntimeRequired))?;
        if !runtime.info().capabilities.non_interactive_execution {
            return Err(AppError::new(ErrorCode::RuntimeNotSupported));
        }
        let model_id = request.model_id.trim();
        if model_id.is_empty() {
            return Err(AppError::new(ErrorCode::ModelRequired));
        }
        if model_id.starts_with('-')
            || model_id
                .chars()
                .any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(AppError::new(ErrorCode::ModelInvalid));
        }
        Ok((name.to_owned(), model_id.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::config::memory::MemoryStore;
    use crate::application::process::fake::{ok, FakeProcessRunner};
    use crate::application::process::ProcessRunner;

    /// The real default registry (`OpenCode`, Claude, detect-only runtimes) over a fake runner.
    fn service() -> AgentService {
        let config = Arc::new(ConfigRepository::load(Box::<MemoryStore>::default()));
        let runner: Arc<dyn ProcessRunner> = Arc::new(FakeProcessRunner::new(&[], |_| ok("")));
        AgentService::new(
            config.clone(),
            Arc::new(PersonalityService::new(config)),
            Arc::new(RuntimeRegistry::with_default_runtimes(&runner)),
        )
    }

    fn request(runtime_id: &str, model_id: &str) -> CreateAgentRequest {
        CreateAgentRequest {
            name: " Architecture Expert ".to_owned(),
            personality_id: "architect".to_owned(),
            runtime_id: runtime_id.to_owned(),
            model_id: model_id.to_owned(),
            instructions: "  Be brief. ".to_owned(),
            worktree_isolation: None,
            result_contract: None,
            permission_profile_id: None,
        }
    }

    #[test]
    fn the_same_personality_can_use_opencode_or_claude_without_changing_the_agent() {
        let service = service();

        let open_code = service
            .create(request("opencode", " opencode/big-pickle "))
            .unwrap();
        let claude = service.create(request("claude", "sonnet")).unwrap();

        assert_eq!(open_code.personality_id, claude.personality_id);
        assert_eq!(
            (open_code.runtime_id.as_str(), open_code.model_id.as_str()),
            ("opencode", "opencode/big-pickle")
        );
        assert_eq!(
            (claude.runtime_id.as_str(), claude.model_id.as_str()),
            ("claude", "sonnet")
        );
        assert_eq!(open_code.name, "Architecture Expert");
        assert_eq!(open_code.instructions, "Be brief.");
        assert_ne!(open_code.id, claude.id);
        assert_eq!(service.list(), [open_code.clone(), claude]);
        assert_eq!(service.find(&open_code.id), Some(open_code));
    }

    #[test]
    fn a_new_agent_starts_from_its_personalitys_suggested_profile_unless_one_is_chosen() {
        let service = service();
        let create = |personality: &str, profile: Option<&str>| {
            let mut r = request("claude", "sonnet");
            r.personality_id = personality.to_owned();
            r.permission_profile_id = profile.map(str::to_owned);
            service.create(r).map(|a| a.permission_profile_id)
        };

        assert_eq!(create("developer", None), Ok(Some("developer".to_owned())));
        assert_eq!(
            create("architecture-validator", None),
            Ok(Some("read_only".to_owned()))
        );
        // The user's choice wins over the suggestion, both ways.
        assert_eq!(
            create("architecture-validator", Some("developer")),
            Ok(Some("developer".to_owned()))
        );
        assert_eq!(
            create("developer", Some("read_only")),
            Ok(Some("read_only".to_owned()))
        );
        assert!(create("developer", Some("root"))
            .unwrap_err()
            .is(ErrorCode::PermissionProfileInvalid));
    }

    #[test]
    fn editing_an_agent_changes_its_profile_only_when_one_is_given() {
        let service = service();
        let agent = service.create(request("claude", "sonnet")).unwrap();
        let mut edit = request("claude", "sonnet");

        let kept = service.update(&agent.id, edit.clone()).unwrap();
        edit.permission_profile_id = Some("read_only".to_owned());
        let changed = service.update(&agent.id, edit).unwrap();

        assert_eq!(kept.permission_profile_id.as_deref(), Some("developer"));
        assert_eq!(changed.permission_profile_id.as_deref(), Some("read_only"));
    }

    #[test]
    fn rejects_invalid_requests() {
        let service = service();
        let bad = |change: fn(&mut CreateAgentRequest)| {
            let mut r = request("claude", "sonnet");
            change(&mut r);
            service.create(r)
        };

        assert!(bad(|r| r.name = " ".into())
            .unwrap_err()
            .is(ErrorCode::NameRequired));
        assert!(bad(|r| r.personality_id = "nope".into())
            .unwrap_err()
            .is(ErrorCode::PersonalityRequired));
        assert!(bad(|r| r.runtime_id = "nope".into())
            .unwrap_err()
            .is(ErrorCode::RuntimeRequired));
        assert!(bad(|r| r.model_id = " ".into())
            .unwrap_err()
            .is(ErrorCode::ModelRequired));
        assert!(bad(|r| r.model_id = "--help".into())
            .unwrap_err()
            .is(ErrorCode::ModelInvalid));
        assert!(bad(|r| r.model_id = "a b".into())
            .unwrap_err()
            .is(ErrorCode::ModelInvalid));
        // Detected but not runnable yet.
        assert!(bad(|r| r.runtime_id = "codex".into())
            .unwrap_err()
            .is(ErrorCode::RuntimeNotSupported));
        assert_eq!(service.list(), []);
    }

    #[test]
    fn agents_saved_before_runtimes_existed_still_load() {
        let old = r#"{"id":"a","name":"A","personalityId":"architect","providerId":"opencode",
            "modelId":"m","instructions":"","createdAt":1}"#;

        let agent: crate::domain::agent::Agent = serde_json::from_str(old).unwrap();

        assert_eq!(agent.runtime_id, "opencode");
    }

    #[test]
    fn new_agents_isolate_their_executions_in_worktrees_unless_told_otherwise() {
        let service = service();

        let default = service.create(request("claude", "sonnet")).unwrap();
        let off = service
            .create(CreateAgentRequest {
                permission_profile_id: None,
                worktree_isolation: Some(false),
                result_contract: None,
                ..request("claude", "sonnet")
            })
            .unwrap();

        assert!(default.worktree_isolation);
        assert!(!off.worktree_isolation);
    }

    #[test]
    fn editing_changes_isolation_only_when_asked_to() {
        let service = service();
        let agent = service.create(request("claude", "sonnet")).unwrap();

        let kept = service
            .update(&agent.id, request("claude", "sonnet"))
            .unwrap();
        let off = service
            .update(
                &agent.id,
                CreateAgentRequest {
                    permission_profile_id: None,
                    worktree_isolation: Some(false),
                    result_contract: None,
                    ..request("claude", "sonnet")
                },
            )
            .unwrap();
        let still_off = service
            .update(&agent.id, request("claude", "sonnet"))
            .unwrap();

        assert!(kept.worktree_isolation);
        assert!(!off.worktree_isolation);
        assert!(!still_off.worktree_isolation);
    }

    #[test]
    fn agents_saved_before_isolation_existed_are_isolated() {
        let old = r#"{"id":"a","name":"A","personalityId":"architect","runtimeId":"claude",
            "modelId":"m","instructions":"","createdAt":1}"#;

        let agent: crate::domain::agent::Agent = serde_json::from_str(old).unwrap();

        assert!(agent.worktree_isolation);
    }

    #[test]
    fn edits_an_agent_keeping_its_identity() {
        let service = service();
        let created = service
            .create(request("opencode", "opencode/big-pickle"))
            .unwrap();

        let updated = service
            .update(
                &created.id,
                CreateAgentRequest {
                    permission_profile_id: None,
                    name: " Renamed ".to_owned(),
                    personality_id: "qa".to_owned(),
                    runtime_id: "claude".to_owned(),
                    model_id: "sonnet".to_owned(),
                    instructions: " New ".to_owned(),
                    worktree_isolation: None,
                    result_contract: None,
                },
            )
            .unwrap();

        assert_eq!(
            (updated.id.as_str(), updated.created_at),
            (created.id.as_str(), created.created_at)
        );
        assert_eq!(updated.name, "Renamed");
        assert_eq!(
            (updated.personality_id.as_str(), updated.runtime_id.as_str()),
            ("qa", "claude")
        );
        assert_eq!(
            (updated.model_id.as_str(), updated.instructions.as_str()),
            ("sonnet", "New")
        );
        assert_eq!(service.list(), [updated]);
    }

    #[test]
    fn rejects_invalid_edits_and_unknown_agents_without_changing_the_agent() {
        let service = service();
        let created = service.create(request("claude", "sonnet")).unwrap();

        assert!(service
            .update(&created.id, request("codex", "m"))
            .unwrap_err()
            .is(ErrorCode::RuntimeNotSupported));
        assert!(service
            .update(
                &created.id,
                CreateAgentRequest {
                    permission_profile_id: None,
                    name: " ".into(),
                    ..request("claude", "sonnet")
                }
            )
            .unwrap_err()
            .is(ErrorCode::NameRequired));
        assert!(matches!(
            service.update("ghost", request("claude", "sonnet")),
            Err(e) if e.is(ErrorCode::AgentNotFound)
        ));
        assert_eq!(service.find(&created.id), Some(created));
    }

    #[test]
    fn deletes_an_agent_and_reports_unknown_ones() {
        let service = service();
        let created = service.create(request("claude", "sonnet")).unwrap();

        service.delete(&created.id).unwrap();

        assert_eq!(service.list(), []);
        assert!(matches!(
            service.delete(&created.id),
            Err(e) if e.is(ErrorCode::AgentNotFound)
        ));
    }

    #[test]
    fn a_contract_is_saved_with_the_agent_and_kept_when_an_edit_says_nothing_about_it() {
        use crate::domain::result_contract::ContractKind;
        let service = service();
        let mut create = request("claude", "sonnet");
        create.result_contract = Some(ResultContract::preset(ContractKind::Validation));
        let agent = service.create(create).unwrap();
        assert_eq!(agent.result_contract.kind, ContractKind::Validation);
        assert_eq!(agent.result_contract.outcomes.len(), 2);

        // An edit without a contract changes nothing about it...
        let edited = service
            .update(&agent.id, request("claude", "sonnet"))
            .unwrap();
        assert_eq!(edited.result_contract, agent.result_contract);
        // ... and one with a contract replaces it.
        let mut change = request("claude", "sonnet");
        change.result_contract = Some(ResultContract::general());
        let general = service.update(&agent.id, change).unwrap();
        assert!(!general.result_contract.requires_outcome());

        // A malformed contract is refused and nothing is saved.
        let mut bad = request("claude", "sonnet");
        bad.result_contract = Some(ResultContract {
            kind: ContractKind::Custom,
            outcomes: vec![],
        });
        let error = service.create(bad).unwrap_err();
        assert!(error.is(ErrorCode::ResultContractInvalid));
        assert_eq!(service.list().len(), 1);
    }
}
