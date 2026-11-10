use tauri::State;

use crate::application::errors::AppError;
use crate::application::usage::{build_report, newer};
use crate::domain::usage::{AgentUsageSummary, UsageReport, UsageWindows, WorkspaceUsageSummary};
use crate::infrastructure::codex_quota;
use crate::state::AppState;

/// What Atlas observed one agent consume in one workspace. `windows` says where "today", "this
/// week" and "this month" start in the user's time zone.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn get_agent_usage(
    state: State<'_, AppState>,
    workspace_id: String,
    agent_id: String,
    windows: UsageWindows,
) -> Result<AgentUsageSummary, AppError> {
    state.usage.agent_summary(&workspace_id, &agent_id, windows)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn get_workspace_usage(
    state: State<'_, AppState>,
    workspace_id: String,
    windows: UsageWindows,
) -> WorkspaceUsageSummary {
    state.usage.workspace_summary(&workspace_id, windows)
}

/// Every execution Atlas observed that ended at or after `since` (ms since the Unix epoch), in
/// every workspace, with names for its agents, workspaces and runtimes: the usage screen.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn get_usage_report(state: State<'_, AppState>, since: u64) -> UsageReport {
    let (records, mut quotas) = state.usage.since(since);
    // Codex reports its limits only in its own session logs: the newest reading wins.
    if let Some(home) = codex_quota::codex_home() {
        let current = quotas.remove("codex");
        if let Some(quota) = newer(current, codex_quota::latest_codex_quota(&home)) {
            quotas.insert("codex".to_owned(), quota);
        }
    }
    let mut connected: Vec<String> = state
        .agents
        .list()
        .into_iter()
        .map(|a| a.runtime_id)
        .collect();
    connected.sort();
    connected.dedup();
    let workspaces = state.workspaces.list();
    build_report(
        connected,
        records,
        quotas,
        |id| state.agents.find(id).map(|a| a.name),
        |id| {
            workspaces
                .iter()
                .find(|w| w.id == id)
                .map(|w| w.name.clone())
        },
        |id| state.runtimes.find(id).map(|r| r.info().name),
    )
}
