//! A whole Atlas around a real (or stand-in) CLI, for the tests that call a model: the real guard,
//! an isolated Git worktree, the real runtime adapters and, when asked for, the macOS sandbox and
//! the Memory. Used by `sandbox_real_tests` and `memory_experiment_tests`; both are opt-in because
//! they cost tokens.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use super::*;
use crate::application::agents::CreateAgentRequest;
use crate::application::config::memory::MemoryStore;
use crate::application::config::ConfigRepository;
use crate::application::memory::MemoryService;
use crate::application::optimization::FixedFlags;
use crate::application::process::ProcessRunner;
use crate::application::projects::fake::FakeInspector;
use crate::application::security::testutil::TempDir;
use crate::application::security::{
    ApprovalBroker, AuditLog, GuardedProcessRunner, MacosSandbox, SecurityService,
};
use crate::application::sessions::SessionRegistry;
use crate::application::workspace::WorkspaceInput;
use crate::domain::memory::MemorySettings;
use crate::domain::security::{SandboxMode, SandboxSettings};

#[derive(Default)]
pub(super) struct Quiet;

impl ExecutionObserver for Quiet {
    fn on_event(&self, _event: &ExecutionEvent) {}
}

pub(super) fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(["-c", "user.email=a@b", "-c", "user.name=a"])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// What a world is made of.
pub(super) struct Setup<'a> {
    pub runtime_id: &'a str,
    pub model: &'a str,
    /// The project's files, committed as the first commit.
    pub files: &'a [(&'a str, &'a str)],
    pub sandbox: SandboxMode,
    /// Memory on with these settings (the service is wired either way, so a note can be seeded).
    pub memory: MemorySettings,
}

pub(super) struct RealWorld {
    pub service: ExecutionService,
    pub project: PathBuf,
    pub memory: Arc<MemoryService>,
    pub workspace_id: String,
    pub agent_id: String,
    pub first_commit: String,
    /// The runtimes, through the same guard and sandbox as the service.
    pub registry: Arc<RuntimeRegistry>,
    _dir: TempDir,
}

impl RealWorld {
    #[allow(clippy::too_many_lines)] // one world, built once
    pub fn build(setup: &Setup<'_>) -> Self {
        let dir = TempDir::new(&format!("real-{}", setup.runtime_id));
        let project = dir.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        git(&project, &["init", "-q", "-b", "main"]);
        for (name, content) in setup.files {
            let file = project.join(name);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, content).unwrap();
        }
        git(&project, &["add", "."]);
        git(&project, &["commit", "-q", "-m", "init"]);
        let first_commit = git(&project, &["rev-parse", "HEAD"]);

        let path = project.to_string_lossy().into_owned();
        let mode = setup.sandbox;
        let config = Arc::new(ConfigRepository::load(Box::<MemoryStore>::default()));
        let security = Arc::new(SecurityService::new(config.clone()));
        let sessions = Arc::new(SessionRegistry::default());
        let audit = Arc::new(AuditLog::default());
        let sandbox = Arc::new(MacosSandbox::new(
            Arc::new(move || SandboxSettings {
                mode,
                dev_caches: true,
            }),
            dir.path().join("sandbox"),
            crate::application::runtimes::confinement::default_confinements(),
        ));
        if mode != SandboxMode::Off {
            assert!(
                sandbox.available(),
                "the sandbox needs macOS with sandbox-exec"
            );
        }
        let runner: Arc<dyn ProcessRunner> = Arc::new(GuardedProcessRunner::new(
            Arc::new(
                crate::infrastructure::SystemProcessRunner::new().with_sessions(sessions.clone()),
            ),
            security.clone(),
            Arc::new(ApprovalBroker::new()),
            audit.clone(),
            sandbox,
            crate::application::runtimes::RUNTIME_PROGRAMS
                .map(str::to_owned)
                .to_vec(),
        ));
        let registry = Arc::new(RuntimeRegistry::with_default_runtimes(&runner));
        let registry_for_bare = registry.clone();
        let personalities = Arc::new(PersonalityService::new(config.clone()));
        let agents = Arc::new(AgentService::new(
            config.clone(),
            personalities.clone(),
            registry.clone(),
        ));
        let workspaces = Arc::new(WorkspaceService::new(
            config.clone(),
            agents.clone(),
            Arc::new(FakeInspector::with(&[(path.as_str(), &[])])),
        ));
        let workspace_id = workspaces
            .create(&WorkspaceInput {
                name: "Real".to_owned(),
                project_path: path,
                description: None,
            })
            .unwrap()
            .id;
        let agent_id = agents
            .create(CreateAgentRequest {
                permission_profile_id: Some("developer".to_owned()),
                name: "dev".to_owned(),
                personality_id: "architect".to_owned(),
                runtime_id: setup.runtime_id.to_owned(),
                model_id: setup.model.to_owned(),
                instructions: String::new(),
                worktree_isolation: Some(true),
                result_contract: None,
            })
            .unwrap()
            .id;
        config
            .modify(|c| {
                for workspace in &mut c.workspaces {
                    workspace.security = crate::domain::security::SecurityPolicy::developer();
                }
                Ok(())
            })
            .unwrap();
        let layout =
            crate::application::worktree::WorktreeLayout::new(dir.path().join("worktrees"));
        let worktrees = Arc::new(crate::application::worktree::WorktreeService::new(
            Arc::new(crate::infrastructure::GitWorktreeManager::new(
                "git".into(),
                layout.clone(),
            )),
            layout,
            config.clone(),
            security.clone(),
        ));
        let memory = Arc::new(MemoryService::new(
            Arc::new(crate::application::memory::ports::MemoryMemoryStore::default()),
            // The real probe: a note that names a file is checked against the real project.
            Arc::new(crate::infrastructure::FsProjectProbe),
        ));
        let service = ExecutionService::new(agents, personalities, registry, workspaces, audit)
            .with_optimization(Arc::new(
                FixedFlags {
                    metrics: true,
                    guardrails: true,
                    ..FixedFlags::default()
                }
                .with_memory(setup.memory),
            ))
            .with_policies(security)
            .with_worktrees(worktrees)
            .with_memory(memory.clone())
            .with_sessions(sessions);
        Self {
            service,
            project,
            memory,
            workspace_id,
            agent_id,
            first_commit,
            registry: registry_for_bare,
            _dir: dir,
        }
    }

    pub fn run(&self, prompt: &str) -> ExecutionRecord {
        self.service
            .run_step(
                self.service.next_execution_id(),
                RunAgentRequest {
                    task_id: "t".to_owned(),
                    workspace_id: self.workspace_id.clone(),
                    agent_id: self.agent_id.clone(),
                    description: prompt.to_owned(),
                    scope: None,
                },
                StepOptions::default(),
                &Quiet,
            )
            .unwrap()
    }

    /// The runtime's CLI as Atlas launches it (same arguments, same guard, same sandbox, file edits
    /// granted in the project), but asked `prompt` alone: none of Atlas's own instructions, so a
    /// model is not told to stay in its folder. What the sandbox must hold is then the only thing
    /// between the request and the disk (ADR 0038, "Não verificado"). Returns the answer (or the
    /// error) and every event the runtime reported, tool calls included.
    pub fn run_bare(
        &self,
        runtime_id: &str,
        model: &str,
        prompt: &str,
    ) -> (
        Result<String, String>,
        Vec<crate::application::runtimes::RuntimeEvent>,
    ) {
        let runtime = self
            .registry
            .find(runtime_id)
            .expect("the runtime is registered");
        let request = crate::application::runtimes::RuntimeRequest {
            model_id: model.to_owned(),
            prompt: crate::application::prompt::Prompt {
                system: "You are a helpful assistant on the owner's own computer.".to_owned(),
                harness: None,
                task_aware: false,
                skills: None,
                memories: None,
                memory_rule: None,
                rules: None,
                context: format!("Working directory: {}", self.project.display()),
                instruction: prompt.to_owned(),
            },
            working_dir: self.project.clone(),
            extra_dirs: Vec::new(),
            scope: crate::application::process::ExecutionScope {
                workspace_id: self.workspace_id.clone(),
                agent_id: self.agent_id.clone(),
                execution_id: "bare-1".to_owned(),
                task_id: "t".to_owned(),
                runtime_access: crate::domain::security::ToolAccess {
                    filesystem_write: true,
                    ..crate::domain::security::ToolAccess::NONE
                },
                isolated: false,
                project_id: None,
            },
            text_only: false,
            allow_edits: true,
            granted_commands: Vec::new(),
            mcp: None,
        };
        let events = std::sync::Mutex::new(Vec::new());
        let result = runtime.execute(&request, &|event| events.lock().unwrap().push(event));
        (
            result
                .map(|o| o.text)
                .map_err(|e| format!("{} | {e:?}", e.user_message())),
            events.into_inner().unwrap(),
        )
    }

    /// Puts the project back as it was at its first commit, so the next run starts from the same
    /// files (Atlas merges a finished execution into the project).
    pub fn reset_project(&self) {
        git(
            &self.project,
            &["reset", "-q", "--hard", &self.first_commit],
        );
        git(&self.project, &["clean", "-q", "-fd"]);
    }
}
