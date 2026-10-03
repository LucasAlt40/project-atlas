use std::sync::Arc;

use crate::application::agents::AgentService;
use crate::application::app_info::AppInfoService;
use crate::application::chat::ChatService;
use crate::application::harness::HarnessService;
use crate::application::lifecycle::AgentLifecycle;
use crate::application::personalities::PersonalityService;
use crate::application::runtimes::RuntimeRegistry;
use crate::application::security::{ApprovalBroker, SecurityOverview};
use crate::application::sessions::SessionRegistry;
use crate::application::settings::SettingsService;
use crate::application::usage_reports::UsageReporter;
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
}

#[cfg(test)]
impl AppState {
    /// A fully wired state with an in-memory config, a fake workspace folder (`/atlas`) and a
    /// scripted runtime (`fake`, answering "fake answer") that can run tasks.
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
        let harness = Arc::new(HarnessService::new(
            workspaces.clone(),
            Arc::new(crate::application::harness::fake::FakeScanner::new(
                crate::application::harness::snapshot::ScanSnapshot::default(),
            )),
            harness_store.clone(),
        ));
        let executions = Arc::new(
            ExecutionService::new(
                agents.clone(),
                personalities.clone(),
                runtimes.clone(),
                workspaces.clone(),
                Arc::new(AuditLog::default()),
            )
            .with_worktrees(worktrees.clone())
            .with_harness(Arc::new(
                crate::application::harness::context::HarnessContextBuilder::new(harness_store),
            )),
        );
        let overview = Arc::new(SecurityOverview::new(
            security,
            config.clone(),
            runtimes.clone(),
        ));
        let ledger = Arc::new(UsageLedger::new(config.clone()));
        let settings = Arc::new(SettingsService::new(config));
        let chat = ChatService::new(
            agents.clone(),
            executions,
            workspaces.clone(),
            ledger.clone(),
        );
        let lifecycle = AgentLifecycle::new(
            agents.clone(),
            chat.clone(),
            workspaces.clone(),
            ledger.clone(),
            settings.clone(),
        );
        let usage = UsageReporter::new(ledger, chat.clone(), agents.clone());
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
            approvals: Arc::new(ApprovalBroker::new()),
            sessions: Arc::default(),
            worktrees,
            harness,
        }
    }
}
