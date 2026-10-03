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
use application::lifecycle::AgentLifecycle;
use application::personalities::PersonalityService;
use application::process::ProcessRunner;
use application::runtimes::RuntimeRegistry;
use application::settings::SettingsService;
use application::usage::UsageLedger;
use application::usage_reports::UsageReporter;
use application::workspace::WorkspaceService;
use infrastructure::{FsProjectInspector, JsonConfigStore, SystemProcessRunner};
use platform::OsPlatform;
use state::AppState;
use tauri::Manager;

/// Builds and runs the Tauri application.
///
/// # Panics
///
/// Panics if the Tauri runtime fails to start.
pub fn run() {
    tauri::Builder::default()
        // Native folder picker only (granted as `dialog:allow-open` in the capability file).
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            app.manage(build_state(&data_dir));
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
            commands::usage::get_agent_usage,
            commands::usage::get_workspace_usage,
        ])
        .run(context())
        .expect("error while running Project Atlas");
}

fn build_state(data_dir: &Path) -> AppState {
    let app_info = AppInfoService::new(env!("CARGO_PKG_VERSION").to_owned(), Arc::new(OsPlatform));
    let config = Arc::new(ConfigRepository::load(Box::new(JsonConfigStore::new(
        data_dir.join("config.json"),
    ))));
    let runner: Arc<dyn ProcessRunner> = Arc::new(SystemProcessRunner::new());
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
    let executions = Arc::new(ExecutionService::new(
        agents.clone(),
        personalities.clone(),
        runtimes.clone(),
        workspaces.clone(),
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
    }
}

/// The generated Tauri context (config + capabilities). Defined once because the macro
/// embeds symbols that must not be duplicated; tests reuse it to exercise the real ACL.
fn context<R: tauri::Runtime>() -> tauri::Context<R> {
    tauri::generate_context!()
}
