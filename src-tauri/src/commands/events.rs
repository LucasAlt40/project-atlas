use tauri::{AppHandle, Emitter, Runtime};

use crate::application::chat::ChatObserver;
use crate::application::executions::ExecutionObserver;
use crate::application::security::PermissionSink;
use crate::application::sessions::SessionSink;
use crate::application::support::now_ms;
use crate::application::workflow::runner::WorkflowObserver;
use crate::domain::conversation::Message;
use crate::domain::execution::{ExecutionEvent, ExecutionEventKind};
use crate::domain::security::PermissionEvent;
use crate::domain::terminal::{SessionStatusEvent, TerminalChunk};
use crate::domain::workflow::WorkflowEvent;

/// Progress of an execution; the payload's `kind` says which step. One channel for every
/// execution: listeners tell executions apart by `executionId` and `agentId`.
pub const EXECUTION_PROGRESS_EVENT: &str = "execution:progress";
/// A piece of a process's raw terminal output (`TerminalChunk`), as it is written. Separate from
/// `execution:progress` on purpose: the chat is what the agent *says*, this is what the
/// process *does*.
pub const EXECUTION_OUTPUT_EVENT: &str = "execution:output";
/// A process changed state (`SessionStatusEvent`): running, interrupting, terminating, exited.
pub const EXECUTION_STATUS_EVENT: &str = "execution:status";
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

/// Adapter: tells the webview about a workflow run. Each event goes out under its own name
/// (`workflow:node_started`…) carrying ids, not state: the UI reads the run again. The steps'
/// executions report through the same observer as any execution (`execution:progress`,
/// `conversation:message`).
pub struct TauriWorkflowObserver<R: Runtime> {
    pub app: AppHandle<R>,
}

impl<R: Runtime> ExecutionObserver for TauriWorkflowObserver<R> {
    fn on_event(&self, event: &ExecutionEvent) {
        TauriEventObserver {
            app: self.app.clone(),
        }
        .on_event(event);
    }
}

impl<R: Runtime> ChatObserver for TauriWorkflowObserver<R> {
    fn on_message(&self, message: &Message) {
        TauriEventObserver {
            app: self.app.clone(),
        }
        .on_message(message);
    }
}

impl<R: Runtime> WorkflowObserver for TauriWorkflowObserver<R> {
    fn on_workflow_event(&self, event: &WorkflowEvent) {
        let name = event.kind.wire_name();
        if let Err(error) = self.app.emit(&name, event) {
            eprintln!("failed to emit {name}: {error}");
        }
    }
}

/// Adapter: forwards what the live process sessions report (output, state, user actions) to the
/// webview. Tauri drops listeners with the window, so nothing is emitted to a closed UI.
pub struct TauriSessionSink<R: Runtime> {
    pub app: AppHandle<R>,
}

impl<R: Runtime> TauriSessionSink<R> {
    fn emit<S: serde::Serialize + Clone>(&self, name: &str, payload: &S) {
        if let Err(error) = self.app.emit(name, payload) {
            eprintln!("failed to emit {name}: {error}");
        }
    }
}

impl<R: Runtime> SessionSink for TauriSessionSink<R> {
    fn on_output(&self, chunk: &TerminalChunk) {
        self.emit(EXECUTION_OUTPUT_EVENT, chunk);
    }

    fn on_status(&self, event: &SessionStatusEvent) {
        self.emit(EXECUTION_STATUS_EVENT, event);
    }

    fn on_activity(&self, event: &ExecutionEvent) {
        self.emit(EXECUTION_PROGRESS_EVENT, event);
    }
}

/// Adapter: announces permission decisions as `execution:progress` events (kind `permission`),
/// so they appear in the live activity of the execution they belong to, with the same ids.
pub struct TauriPermissionSink<R: Runtime> {
    pub app: AppHandle<R>,
}

impl<R: Runtime> PermissionSink for TauriPermissionSink<R> {
    fn on_permission_event(&self, event: &PermissionEvent) {
        let progress = permission_progress(event);
        if let Err(error) = self.app.emit(EXECUTION_PROGRESS_EVENT, &progress) {
            eprintln!("failed to emit permission event: {error}");
        }
    }
}

/// The `snake_case` wire name of a unit enum variant.
fn wire(value: &impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// The `execution:progress` form of a permission decision.
pub fn permission_progress(event: &PermissionEvent) -> ExecutionEvent {
    let mut metadata = std::collections::BTreeMap::from([
        ("decision".to_owned(), wire(&event.decision)),
        ("action".to_owned(), wire(&event.action)),
        ("source".to_owned(), wire(&event.source)),
        ("target".to_owned(), event.target.clone()),
    ]);
    if let Some(reason) = event.reason {
        metadata.insert("reason".to_owned(), reason.as_str().to_owned());
    }
    if let Some(cwd) = &event.cwd {
        metadata.insert("cwd".to_owned(), cwd.clone());
    }
    if let Some(id) = &event.approval_id {
        metadata.insert("approvalId".to_owned(), id.clone());
    }
    if !event.notes.is_empty() {
        metadata.insert("notes".to_owned(), event.notes.join(","));
    }
    ExecutionEvent {
        execution_id: event.execution_id.clone(),
        workspace_id: event.workspace_id.clone(),
        task_id: event.task_id.clone(),
        agent_id: event.agent_id.clone(),
        kind: ExecutionEventKind::Permission,
        message: format!("Permission {}: {}", wire(&event.decision), event.target),
        timestamp: if event.timestamp == 0 {
            now_ms()
        } else {
            event.timestamp
        },
        metadata,
    }
}

/// Adapter: tells the webview what the agent analysing a project is doing.
pub struct TauriHarnessProgress<R: Runtime> {
    pub app: AppHandle<R>,
}

impl<R: Runtime> crate::application::harness::semantic::HarnessProgress
    for TauriHarnessProgress<R>
{
    fn report(&self, update: &crate::application::harness::semantic::ProgressUpdate) {
        if let Err(error) = self.app.emit("harness:progress", update) {
            eprintln!("failed to emit harness:progress: {error}");
        }
    }
}
