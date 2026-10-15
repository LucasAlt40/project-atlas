use std::sync::Arc;

use crate::application::agents::AgentService;
use crate::application::app_info::AppInfoService;
use crate::application::chat::ChatService;
use crate::application::harness::HarnessService;
use crate::application::lifecycle::AgentLifecycle;
use crate::application::live_workspace::LiveWorkspaceService;
use crate::application::mcp::McpService;
use crate::application::personalities::PersonalityService;
use crate::application::portability::PortabilityService;
use crate::application::remote::RemoteAccess;
use crate::application::runtimes::RuntimeRegistry;
use crate::application::security::{ApprovalBroker, SecurityOverview};
use crate::application::sessions::SessionRegistry;
use crate::application::settings::SettingsService;
use crate::application::specs::runner::SpecRunner;
use crate::application::specs::service::SpecService;
use crate::application::usage_reports::UsageReporter;
use crate::application::workflow::integration::IntegrationService;
use crate::application::workflow::orchestrator::Orchestrator;
use crate::application::workflow::service::WorkflowService;
use crate::application::workspace::WorkspaceService;
use crate::application::worktree::WorktreeService;

/// Application services shared with command handlers via Tauri's managed state.
/// Services are `Arc`s because they are shared with each other and moved onto blocking threads.
pub struct AppState {
    pub app_info: AppInfoService,
    pub personalities: Arc<PersonalityService>,
    pub runtimes: Arc<RuntimeRegistry>,
    pub agents: Arc<AgentService>,
    pub workspaces: Arc<WorkspaceService>,
    pub settings: Arc<SettingsService>,
    pub chat: ChatService,
    pub lifecycle: AgentLifecycle,
    pub usage: UsageReporter,
    pub security: Arc<SecurityOverview>,
    pub approvals: Arc<ApprovalBroker>,
    /// The live processes of executions that run in a terminal.
    pub sessions: Arc<SessionRegistry>,
    /// The Git worktrees of isolated executions.
    pub worktrees: Arc<WorktreeService>,
    /// Project analysis and the `.atlas/` Harness.
    pub harness: Arc<HarnessService>,
    /// Workflow definitions and the record of their runs.
    pub workflows: Arc<WorkflowService>,
    /// Drives workflow runs through the chat/execution layer.
    pub orchestrator: Arc<Orchestrator>,
    /// What becomes of the code of a run: review, apply, keep, discard, open in an editor.
    pub integration: Arc<IntegrationService>,
    /// The files of a run's worktree as they change while its agents work.
    pub live: Arc<LiveWorkspaceService>,
    /// The workspace's MCP connections, their grants and their secrets.
    pub mcp: Arc<McpService>,
    pub mcp_catalog: Arc<crate::application::mcp::McpCatalog>,
    /// The notes of earlier work (ADR 0037).
    pub memory: Arc<crate::application::memory::MemoryService>,
    /// Export and import of configurations as files.
    pub portability: Arc<PortabilityService>,
    /// Watching and answering runs from a phone on the local network. Off until the person
    /// switches it on.
    pub remote: Arc<RemoteAccess>,
    /// Specifications (ADR 0035): their records and the checks, and the agents' turns on them.
    pub specs: Arc<SpecService>,
    pub spec_runner: Arc<SpecRunner>,
    pub spec_work: Arc<crate::application::specs::implementation::SpecImplementation>,
    /// The optional `OpenSpec` command (a second look at a published change, and the archive).
    pub spec_cli: Arc<crate::application::specs::cli_service::SpecCliService>,
}

#[cfg(test)]
impl AppState {
    /// A fully wired state with an in-memory config, a fake workspace folder (`/atlas`) and a
    /// scripted runtime (`fake`, answering "fake answer") that can run tasks.
    #[allow(clippy::too_many_lines)]
    pub fn for_tests(app_info: AppInfoService) -> Self {
        use crate::application::config::memory::MemoryStore;
        use crate::application::config::ConfigRepository;
        use crate::application::executions::ExecutionService;
        use crate::application::projects::fake::FakeInspector;
        use crate::application::runtimes::fake::FakeRuntime;
        use crate::application::security::{AuditLog, SecurityService};
        use crate::application::usage::UsageLedger;

        let config = Arc::new(ConfigRepository::load(Box::<MemoryStore>::default()));
        let personalities = Arc::new(PersonalityService::new(config.clone()));
        let runtimes = Arc::new(RuntimeRegistry::new(vec![Arc::new(FakeRuntime::new(
            "fake",
            Ok("fake answer"),
        ))]));
        let mcp = Arc::new(McpService::new(
            config.clone(),
            Arc::new(crate::application::mcp::MemoryCredentials::default()),
            runtimes.clone(),
        ));
        let agents = Arc::new(AgentService::new(
            config.clone(),
            personalities.clone(),
            runtimes.clone(),
        ));
        let workspaces = Arc::new(WorkspaceService::new(
            config.clone(),
            agents.clone(),
            Arc::new(FakeInspector::with(&[
                ("/atlas", &["Rust"]),
                ("/other", &[]),
            ])),
        ));
        let security = Arc::new(SecurityService::new(config.clone()));
        let layout = crate::application::worktree::WorktreeLayout::new(
            std::env::temp_dir().join(format!("atlas-test-state-{}", std::process::id())),
        );
        let worktrees = Arc::new(WorktreeService::new(
            Arc::new(crate::infrastructure::GitWorktreeManager::new(
                "git".into(),
                layout.clone(),
            )),
            layout,
            config.clone(),
            security.clone(),
        ));
        let harness_store: Arc<dyn crate::application::harness::HarnessStore> =
            Arc::new(crate::application::harness::fake::MemoryHarnessStore::default());
        let context_builder = Arc::new(
            crate::application::harness::context::HarnessContextBuilder::new(harness_store.clone()),
        );
        let harness = Arc::new(
            HarnessService::new(
                workspaces.clone(),
                Arc::new(crate::application::harness::fake::FakeScanner::new(
                    crate::application::harness::snapshot::ScanSnapshot::default(),
                )),
                harness_store,
            )
            .with_context_builder(context_builder.clone()),
        );
        let executions = Arc::new(
            ExecutionService::new(
                agents.clone(),
                personalities.clone(),
                runtimes.clone(),
                workspaces.clone(),
                Arc::new(AuditLog::default()),
            )
            .with_worktrees(worktrees.clone())
            .with_policies(security.clone())
            .with_harness(context_builder),
        );
        let overview = Arc::new(SecurityOverview::new(
            security,
            config.clone(),
            runtimes.clone(),
        ));
        let ledger = Arc::new(UsageLedger::new(config.clone()));
        let workflows = Arc::new(WorkflowService::new(
            config.clone(),
            agents.clone(),
            workspaces.clone(),
        ));
        let portability = Arc::new(PortabilityService::new(
            config.clone(),
            runtimes.clone(),
            workspaces.clone(),
        ));
        let settings = Arc::new(SettingsService::new(config));
        let chat = ChatService::new(
            agents.clone(),
            executions.clone(),
            workspaces.clone(),
            ledger.clone(),
        );
        let specs = Arc::new(SpecService::new(
            Arc::new(crate::application::specs::ports::MemorySpecStore::default()),
            Arc::new(crate::application::specs::ports::MemoryProjectFiles::default()),
            Arc::new(crate::application::specs::openspec::OpenSpecAdapter),
            workspaces.clone(),
            agents.clone(),
            Arc::new(crate::application::specs::service::NoEvents),
        ));
        let lifecycle = AgentLifecycle::new(
            agents.clone(),
            chat.clone(),
            workspaces.clone(),
            ledger.clone(),
            settings.clone(),
        )
        .with_workflows(workflows.clone())
        .with_specs(specs.clone());
        let approvals = Arc::new(ApprovalBroker::new());
        let sessions = Arc::new(crate::application::sessions::SessionRegistry::default());
        let spec_runner = Arc::new(SpecRunner::new(
            specs.clone(),
            chat.clone(),
            sessions.clone(),
        ));
        let spec_cli = Arc::new(crate::application::specs::cli_service::SpecCliService::new(
            specs.clone(),
            workflows.clone(),
            crate::application::specs::cli::OpenSpecCli::new(Arc::new(
                crate::application::process::fake::FakeProcessRunner::new(&[], |_| {
                    Err(crate::application::process::ProcessError::NotFound)
                }),
            )),
            Arc::new(crate::application::specs::ports::MemoryProjectFiles::default()),
        ));
        let spec_work = Arc::new(
            crate::application::specs::implementation::SpecImplementation::new(
                specs.clone(),
                workflows.clone(),
            ),
        );
        let live = Arc::new(LiveWorkspaceService::new(
            worktrees.clone(),
            workflows.clone(),
            Arc::new(crate::infrastructure::NotifyWatcher),
            Arc::new(crate::application::live_workspace::NoLiveSink),
        ));
        let orchestrator = Arc::new(Orchestrator::new(
            workflows.clone(),
            Arc::new(
                crate::application::workflow::chat_runner::ChatStepRunner::new(
                    chat.clone(),
                    sessions.clone(),
                    approvals.clone(),
                )
                .with_live(live.clone())
                .with_shared_worktrees(
                    worktrees.clone(),
                    workspaces.clone(),
                    executions.clone(),
                ),
            ),
        ));
        let integration = Arc::new(IntegrationService::new(
            workflows.clone(),
            worktrees.clone(),
            workspaces.clone(),
            Arc::new(crate::application::ide::fake::FakeIde::default()),
        ));
        let usage = UsageReporter::new(ledger, chat.clone(), agents.clone());
        let remote = Arc::new(RemoteAccess::new(
            Arc::new(crate::application::remote::gateway::RemoteGateway::new(
                workflows.clone(),
                workspaces.clone(),
                agents.clone(),
                settings.clone(),
                orchestrator.clone(),
            )),
            Arc::new(crate::application::remote::network::fake::Fixed(vec![(
                "lo0",
                "127.0.0.1",
            )])),
        ));
        Self {
            app_info,
            personalities,
            runtimes,
            agents,
            workspaces,
            settings,
            chat,
            lifecycle,
            usage,
            security: overview,
            approvals,
            sessions,
            worktrees,
            harness,
            workflows,
            orchestrator,
            integration,
            live,
            mcp,
            mcp_catalog: Arc::new(crate::application::mcp::McpCatalog::new(Arc::new(
                crate::application::process::fake::FakeProcessRunner::new(&[], |_| {
                    Err(crate::application::process::ProcessError::NotFound)
                }),
            ))),
            memory: Arc::new(crate::application::memory::MemoryService::new(
                Arc::new(crate::application::memory::ports::MemoryMemoryStore::default()),
                Arc::new(crate::application::memory::ports::FakeProbe::default()),
            )),
            portability,
            remote,
            specs,
            spec_runner,
            spec_work,
            spec_cli,
        }
    }
}
