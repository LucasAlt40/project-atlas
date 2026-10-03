//! What happens to running agents when Atlas closes (ADR 0007): they are ended, never left
//! behind. If any are still working the user is asked first.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Manager, RunEvent, Runtime};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::state::AppState;

/// How long the application waits for processes it has asked to end before it exits anyway.
const SHUTDOWN_WAIT: Duration = Duration::from_secs(4);

/// The user already agreed to end the running agents (so closing again must not ask again).
#[derive(Clone, Default)]
pub struct ExitConsent(Arc<AtomicBool>);

/// `(title, message, end-and-exit, keep-open)` in the user's language. The only text the core
/// words itself, because the webview is not there to do it when the window closes.
fn texts(language: &str, running: usize) -> (String, String, String, String) {
    if language == "en-US" {
        (
            "Agents are still running".to_owned(),
            format!(
                "{running} execution(s) are still running. Exiting will stop them and their processes."
            ),
            "Stop executions and exit".to_owned(),
            "Keep Atlas open".to_owned(),
        )
    } else {
        (
            "Agentes ainda em execução".to_owned(),
            format!(
                "{running} execução(ões) ainda estão em andamento. Sair vai interrompê-las e encerrar seus processos."
            ),
            "Cancelar execuções e sair".to_owned(),
            "Manter o Atlas aberto".to_owned(),
        )
    }
}

pub fn on_run_event<R: Runtime>(app: &AppHandle<R>, event: &RunEvent, consent: &ExitConsent) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    match event {
        RunEvent::ExitRequested { api, .. } => {
            let running = state.sessions.live_executions().len();
            if running == 0 || consent.0.load(Ordering::SeqCst) {
                return;
            }
            api.prevent_exit();
            let (title, message, stop, keep) = texts(&state.settings.get().language, running);
            let (app, consent) = (app.clone(), consent.0.clone());
            app.dialog()
                .message(message)
                .title(title)
                .kind(MessageDialogKind::Warning)
                .buttons(MessageDialogButtons::OkCancelCustom(stop, keep))
                .show(move |stop_and_exit| {
                    if stop_and_exit {
                        consent.store(true, Ordering::SeqCst);
                        app.exit(0);
                    }
                });
        }
        // Whatever way the application ends, its agents' processes end with it.
        RunEvent::Exit => state.sessions.terminate_all_and_wait(SHUTDOWN_WAIT),
        _ => {}
    }
}
