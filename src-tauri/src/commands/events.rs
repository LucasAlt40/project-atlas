use tauri::{AppHandle, Emitter, Runtime};

use crate::application::chat::ChatObserver;
use crate::application::executions::ExecutionObserver;
use crate::domain::conversation::Message;
use crate::domain::execution::ExecutionEvent;

/// Progress of an execution; the payload's `kind` says which step. One channel for every
/// execution: listeners tell executions apart by `executionId` and `agentId`.
pub const EXECUTION_PROGRESS_EVENT: &str = "execution:progress";
/// A message was added to a conversation (today: the assistant's answer).
pub const CONVERSATION_MESSAGE_EVENT: &str = "conversation:message";

/// Adapter: forwards what the core reports to the webview as Tauri events.
pub struct TauriEventObserver<R: Runtime> {
    pub app: AppHandle<R>,
}

impl<R: Runtime> TauriEventObserver<R> {
    fn emit<S: serde::Serialize + Clone>(&self, name: &str, payload: &S) {
        // Events are best-effort: the conversation can always be re-read with a command.
        if let Err(error) = self.app.emit(name, payload) {
            eprintln!("failed to emit {name}: {error}");
        }
    }
}

impl<R: Runtime> ExecutionObserver for TauriEventObserver<R> {
    fn on_event(&self, event: &ExecutionEvent) {
        self.emit(EXECUTION_PROGRESS_EVENT, event);
    }
}

impl<R: Runtime> ChatObserver for TauriEventObserver<R> {
    fn on_message(&self, message: &Message) {
        self.emit(CONVERSATION_MESSAGE_EVENT, message);
    }
}
