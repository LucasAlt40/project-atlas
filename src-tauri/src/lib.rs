//! Project Atlas core.
//!
//! Dependency direction: `commands` -> `application` -> `domain`.
//! `platform` and `infrastructure` implement ports declared by `application`; they are
//! wired up only here, in the composition root.

mod application;
mod commands;
mod domain;
mod infrastructure;
mod platform;
mod state;

use std::path::Path;
use std::sync::Arc;

use application::agents::AgentService;
use application::app_info::AppInfoService;
use application::chat::ChatService;
use application::config::ConfigRepository;
use application::executions::ExecutionService;
use application::harness::context::HarnessContextBuilder;
use application::harness::semantic::RuntimeSemanticFactory;
use application::harness::{HarnessService, HarnessStore, ProjectScanner};
use application::history::ConversationHistory;
use application::lifecycle::AgentLifecycle;
use application::personalities::PersonalityService;
use application::process::ProcessRunner;
use application::runtimes::{RuntimeRegistry, RUNTIME_PROGRAMS};
use application::security::{
    ApprovalBroker, AuditLog, GuardedProcessRunner, NoSandbox, PermissionSink, SecurityOverview,
    SecurityService,
};
use application::sessions::{SessionRegistry, SessionSink};
use application::settings::SettingsService;
use application::usage::UsageLedger;
use application::usage_reports::UsageReporter;
use application::workflow::chat_runner::ChatStepRunner;
use application::workflow::integration::IntegrationService;
use application::workflow::orchestrator::Orchestrator;
use application::workflow::service::WorkflowService;
use application::workspace::WorkspaceService;
use application::worktree::{WorktreeLayout, WorktreeService};
use infrastructure::{
    FsHarnessStore, FsProjectInspector, FsProjectScanner, FsSkillStore, GitWorktreeManager,
    JsonConfigStore, SystemIdeLauncher, SystemProcessRunner,
};
use platform::OsPlatform;
use state::AppState;
use tauri::Manager;

/// Builds and runs the Tauri application.
///
/// # Panics
///
/// Panics if the Tauri runtime fails to build.
#[allow(clippy::too_many_lines)] // the command list
pub fn run() {
    let consent = commands::exit::ExitConsent::default();
    tauri::Builder::default()
        // Native folder picker only (granted as `dialog:allow-open` in the capability file).
        .plugin(tauri_plugin_dialog::init())
        // System notifications (granted as `notification:default`): an agent waiting for the person.
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let sink = Arc::new(commands::events::TauriPermissionSink {
                app: app.handle().clone(),
            });
            let sessions: Arc<dyn SessionSink> = Arc::new(commands::events::TauriSessionSink {
                app: app.handle().clone(),
            });
            let progress: Arc<dyn application::harness::semantic::HarnessProgress> =
                Arc::new(commands::events::TauriHarnessProgress {
                    app: app.handle().clone(),
                });
            let live: Arc<dyn application::live_workspace::LiveSink> =
                Arc::new(commands::events::TauriLiveSink {
                    app: app.handle().clone(),
                });
            app.manage(build_state(&data_dir, sink, sessions, Some(progress), live));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::get_app_info,
            commands::personalities::list_personalities,
            commands::personalities::create_personality,
            commands::personalities::update_personality,
            commands::personalities::delete_personality,
            commands::personalities::restore_default_personalities,
            commands::runtimes::list_runtimes,
            commands::agents::list_agents,
            commands::agents::create_agent,
            commands::agents::update_agent,
            commands::agents::set_agent_result_contract,
            commands::agents::delete_agent,
            commands::workspace::list_workspaces,
            commands::workspace::create_workspace,
            commands::workspace::update_workspace,
            commands::workspace::delete_workspace,
            commands::workspace::add_agent_to_workspace,
            commands::workspace::remove_agent_from_workspace,
            commands::workspace::get_project_context,
            commands::settings::get_settings,
            commands::settings::set_language,
            commands::settings::select_workspace,
            commands::chat::send_message,
            commands::chat::list_messages,
            commands::chat::list_executions,
            commands::usage::get_agent_usage,
            commands::usage::get_workspace_usage,
            commands::security::get_workspace_security,
            commands::security::get_agent_permissions,
            commands::security::set_agent_permission_profile,
            commands::security::list_pending_approvals,
            commands::security::resolve_approval,
            commands::terminal::get_execution_terminal,
            commands::terminal::execution_interrupt,
            commands::terminal::execution_terminate,
            commands::terminal::execution_terminal_input,
            commands::terminal::execution_terminal_resize,
            commands::worktree::list_execution_worktrees,
            commands::worktree::merge_execution,
            commands::harness::analyze_project,
            commands::harness::initialize_project,
            commands::harness::get_project_harness,
            commands::harness::refresh_project_harness,
            commands::harness::preview_task_context,
            commands::workflow::list_workflows,
            commands::workflow::get_workflow,
            commands::workflow::list_workflow_templates,
            commands::workflow::select_workflow_template,
            commands::workflow::create_workflow,
            commands::workflow::create_workflow_from_template,
            commands::workflow::update_workflow,
            commands::workflow::delete_workflow,
            commands::workflow::validate_workflow,
            commands::workflow::suggest_route_repairs,
            commands::workflow::repair_workflow_routes,
            commands::workflow::start_workflow,
            commands::workflow::pause_workflow,
            commands::workflow::resume_workflow,
            commands::workflow::cancel_workflow,
            commands::workflow::answer_workflow_interaction,
            commands::workflow::list_pending_interactions,
            commands::workflow::get_workflow_execution,
            commands::workflow::get_workflow_recovery,
            commands::workflow::list_workflow_executions,
            commands::workflow::get_workflow_changes,
            commands::workflow::get_workflow_diff,
            commands::workflow::apply_workflow_changes,
            commands::workflow::keep_workflow_changes,
            commands::workflow::discard_workflow_changes,
            commands::workflow::list_ides,
            commands::workflow::open_workflow_in_ide,
            commands::live_workspace::get_live_workspace,
            commands::live_workspace::refresh_live_workspace,
            commands::live_workspace::get_live_file,
            commands::live_workspace::get_live_diff,
        ])
        .build(context())
        .expect("error while building Project Atlas")
        .run(move |app, event| commands::exit::on_run_event(app, &event, &consent));
}

#[allow(clippy::too_many_lines)]
fn build_state(
    data_dir: &Path,
    permission_sink: Arc<dyn PermissionSink>,
    session_sink: Arc<dyn SessionSink>,
    harness_progress: Option<Arc<dyn application::harness::semantic::HarnessProgress>>,
    live_sink: Arc<dyn application::live_workspace::LiveSink>,
) -> AppState {
    let app_info = AppInfoService::new(env!("CARGO_PKG_VERSION").to_owned(), Arc::new(OsPlatform));
    let config = Arc::new(ConfigRepository::load(Box::new(JsonConfigStore::new(
        data_dir.join("config.json"),
    ))));
    // Every process the core starts goes through the guard: the system runner is never handed
    // to a runtime directly. The policy it applies is derived from stored workspaces and agents.
    let security = Arc::new(SecurityService::new(config.clone()));
    let approvals = Arc::new(ApprovalBroker::new());
    let audit = Arc::new(AuditLog::with_sink(permission_sink));
    // The runner publishes each terminal process it starts here; the same registry is what the
    // terminal commands control. A session exists only for a process the guard let start.
    let sessions = Arc::new(SessionRegistry::with_sink(session_sink));
    let runner: Arc<dyn ProcessRunner> = Arc::new(GuardedProcessRunner::new(
        Arc::new(SystemProcessRunner::new().with_sessions(sessions.clone())),
        security.clone(),
        approvals.clone(),
        audit.clone(),
        Arc::new(NoSandbox),
        RUNTIME_PROGRAMS.map(str::to_owned).to_vec(),
    ));
    let runtimes = Arc::new(RuntimeRegistry::with_default_runtimes(&runner));
    let personalities = Arc::new(PersonalityService::new(config.clone()));
    let agents = Arc::new(AgentService::new(
        config.clone(),
        personalities.clone(),
        runtimes.clone(),
    ));
    let workspaces = Arc::new(WorkspaceService::new(
        config.clone(),
        agents.clone(),
        Arc::new(FsProjectInspector),
    ));
    // Conversations and ended executions come back after a restart. A run the previous session
    // never saw end is answered as interrupted, and new ids continue after the stored ones.
    let history = Arc::new(ConversationHistory::new(config.clone()));
    history.recover_interrupted();
    // Isolated executions work in Git worktrees kept in the app's data folder. Git is run by
    // Atlas itself, not by an agent, so it does not go through the guard; the manager is the
    // only thing that runs it (see ADR 0009). What a worktree's execution may do is still
    // decided by the guard, with the worktree as its project.
    let layout = WorktreeLayout::new(data_dir.join("worktrees"));
    let git = runner
        .locate("git")
        .unwrap_or_else(|| std::path::PathBuf::from("git"));
    let worktrees = Arc::new(WorktreeService::new(
        Arc::new(GitWorktreeManager::new(git, layout.clone())),
        layout,
        config.clone(),
        security.clone(),
    ));
    // A worktree whose execution the previous session never saw end is kept, and said to have
    // been cut short; one whose merge succeeded but whose folder could not be removed is
    // tried again.
    worktrees.recover_interrupted();
    worktrees.cleanup_pending();
    // New ids continue after every stored execution *and* every worktree still around, so a
    // branch or folder is never reused.
    // The Harness lives in the project's own `.atlas/`; Atlas keeps no copy of it. The context
    // builder hands agents its text, and nothing else (runtimes, worktrees) knows about it.
    let harness_store: Arc<dyn HarnessStore> = Arc::new(FsHarnessStore);
    let scanner: Arc<dyn ProjectScanner> = Arc::new(FsProjectScanner::default());
    // One builder serves the runs and the previews, so that a preview is what a run would use.
    let context_builder = Arc::new(
        HarnessContextBuilder::new(harness_store.clone())
            .with_scanner(scanner.clone())
            .with_fingerprint_cache(std::time::Duration::from_secs(5)),
    );
    let harness = Arc::new(
        HarnessService::new(workspaces.clone(), scanner.clone(), harness_store.clone())
            .with_context_builder(context_builder.clone())
            .with_semantic(Arc::new(RuntimeSemanticFactory {
                agents: agents.clone(),
                runtimes: runtimes.clone(),
                progress: harness_progress,
            })),
    );
    let first_id = worktrees.next_execution_number(history.next_execution_number());
    let executions = Arc::new(
        ExecutionService::new(
            agents.clone(),
            personalities.clone(),
            runtimes.clone(),
            workspaces.clone(),
            audit,
        )
        .with_sessions(sessions.clone())
        .with_worktrees(worktrees.clone())
        .with_policies(security.clone())
        .with_harness(context_builder)
        .with_optimization(config.clone())
        .with_skills(Arc::new(
            application::optimization::skills::SkillService::new(
                Arc::new(FsSkillStore),
                Some(data_dir.join("skills")),
            ),
        ))
        .with_first_id(first_id),
    );
    let ledger = Arc::new(UsageLedger::new(config.clone()));
    let config_for_overview = config.clone();
    // Workflow definitions and runs live in the same config store. A run the previous session
    // never saw end is marked interrupted: nothing is resumed or called finished on its own.
    let workflows = Arc::new(WorkflowService::new(
        config.clone(),
        agents.clone(),
        workspaces.clone(),
    ));
    workflows.recover_interrupted();
    let settings = Arc::new(SettingsService::new(config));
    let chat = ChatService::with_history(
        agents.clone(),
        executions.clone(),
        workspaces.clone(),
        ledger.clone(),
        history,
    );
    let lifecycle = AgentLifecycle::new(
        agents.clone(),
        chat.clone(),
        workspaces.clone(),
        ledger.clone(),
        settings.clone(),
    )
    .with_workflows(workflows.clone());
    // A workflow step is an execution like any other: it goes through the chat layer, so
    // through the agent's worktree policy, the task context, the runtimes and the guard.
    let live = Arc::new(application::live_workspace::LiveWorkspaceService::new(
        worktrees.clone(),
        workflows.clone(),
        Arc::new(infrastructure::NotifyWatcher),
        live_sink,
    ));
    let orchestrator = Arc::new(Orchestrator::new(
        workflows.clone(),
        Arc::new(
            ChatStepRunner::new(chat.clone(), sessions.clone(), approvals.clone())
                .with_live(live.clone())
                .with_shared_worktrees(worktrees.clone(), workspaces.clone(), executions),
        ),
    ));
    // What becomes of a run's code (review, apply, keep, discard, open in an editor): Git and the
    // worktree service decide; the editor is launched by Atlas from a fixed list.
    let integration = Arc::new(IntegrationService::new(
        workflows.clone(),
        worktrees.clone(),
        workspaces.clone(),
        Arc::new(SystemIdeLauncher::system()),
    ));
    let usage = UsageReporter::new(ledger, chat.clone(), agents.clone());
    let overview = Arc::new(SecurityOverview::new(
        security,
        config_for_overview,
        runtimes.clone(),
    ));
    AppState {
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
    }
}

/// The generated Tauri context (config + capabilities). Defined once because the macro
/// embeds symbols that must not be duplicated; tests reuse it to exercise the real ACL.
fn context<R: tauri::Runtime>() -> tauri::Context<R> {
    tauri::generate_context!()
}
