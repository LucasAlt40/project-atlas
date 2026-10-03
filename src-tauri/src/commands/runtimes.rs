use tauri::State;

use crate::domain::runtime::RuntimeStatus;
use crate::state::AppState;

/// Inspects the local machine for supported AI runtimes. Starts short-lived child processes
/// (fixed programs and arguments chosen by the runtime adapters), so it runs off the async
/// executor.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub async fn list_runtimes(state: State<'_, AppState>) -> Result<Vec<RuntimeStatus>, String> {
    let runtimes = state.runtimes.clone();
    tauri::async_runtime::spawn_blocking(move || runtimes.inspect_all())
        .await
        .map_err(|error| error.to_string())
}
