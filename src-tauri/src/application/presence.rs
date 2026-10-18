//! Discord Rich Presence (proof of concept): turns what Atlas is doing into the two lines Discord
//! shows under the person's name, like the VS Code extension does.
//!
//! ```text
//! workflow / execution events ─┐
//! workspace selection ─────────┼─> State ──(worker thread: compose, dedupe, throttle)──> PresenceTransport
//! settings (on/off, names) ────┘                                                    (Discord IPC socket)
//! ```
//!
//! Rules this module keeps:
//!
//! - nothing leaves the machine unless the person switched it on, and the names (workspace,
//!   workflow, step) are a separate choice. The text of prompts, files and answers never reaches it;
//! - it only observes. A failing Discord (closed, no application id, a refused frame) never reaches
//!   the run: the worker retries on a schedule and the events are not held up;
//! - the events only mutate [`State`] and wake the worker. What is shown is always composed from the
//!   state *at the moment it is sent*, so bursts collapse into one update.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use super::settings::SettingsService;
use super::support::now_ms;
use super::workflow::service::WorkflowService;
use super::workspace::WorkspaceService;
use crate::application::support::LockExt;
use crate::domain::execution::{ExecutionEvent, ExecutionEventKind};
use crate::domain::presence::{Activity, PresenceSettings};
use crate::domain::workflow::{WorkflowEvent, WorkflowEventKind};

/// Discord drops updates beyond about five per twenty seconds.
const MIN_INTERVAL: Duration = Duration::from_secs(4);
/// How long to wait before trying again after Discord could not be reached.
const RETRY_AFTER: Duration = Duration::from_secs(15);
/// How long a finished run stays on the profile before the workspace takes its place.
const FINISHED_WINDOW_MS: u64 = 30_000;

/// What is sent: the Discord application and the activity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Published {
    pub client_id: String,
    pub activity: Activity,
}

/// Port: delivers the activity to Discord. Implemented in `infrastructure/`; tests use a fake.
pub trait PresenceTransport: Send + Sync {
    /// Shows `published`, or removes the activity (`None`).
    ///
    /// # Errors
    ///
    /// Why Discord could not be reached or refused it. The worker tries again later.
    fn publish(&self, published: Option<&Published>) -> Result<(), String>;
}

/// Delivers nowhere. For tests that build a whole state.
#[cfg(test)]
pub struct NullTransport;

#[cfg(test)]
impl PresenceTransport for NullTransport {
    fn publish(&self, _: Option<&Published>) -> Result<(), String> {
        Ok(())
    }
}

/// Port: what the composer needs to know about the rest of Atlas.
pub trait PresenceEnv: Send + Sync {
    fn settings(&self) -> PresenceSettings;
    fn language(&self) -> String;
    fn selected_workspace_name(&self) -> Option<String>;
    fn workspace_name(&self, workspace_id: &str) -> Option<String>;
    fn workflow_name(&self, workflow_id: &str) -> Option<String>;
    fn node_label(&self, workflow_id: &str, node_id: &str) -> Option<String>;
}

/// [`PresenceEnv`] over the real services.
pub struct AtlasEnv {
    pub settings: Arc<SettingsService>,
    pub workspaces: Arc<WorkspaceService>,
    pub workflows: Arc<WorkflowService>,
}

impl PresenceEnv for AtlasEnv {
    fn settings(&self) -> PresenceSettings {
        self.settings.get().presence
    }

    fn language(&self) -> String {
        self.settings.get().language
    }

    fn selected_workspace_name(&self) -> Option<String> {
        let id = self.settings.get().selected_workspace_id?;
        self.workspace_name(&id)
    }

    fn workspace_name(&self, workspace_id: &str) -> Option<String> {
        self.workspaces.get(workspace_id).map(|w| w.name)
    }

    fn workflow_name(&self, workflow_id: &str) -> Option<String> {
        self.workflows.get(workflow_id).map(|w| w.name)
    }

    fn node_label(&self, workflow_id: &str, node_id: &str) -> Option<String> {
        self.workflows
            .get(workflow_id)?
            .nodes
            .iter()
            .find(|n| n.id == node_id)
            .map(|n| n.label().to_owned())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Running,
    /// A step waits for the person (an approval or a question).
    Waiting,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl Phase {
    fn is_final(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone)]
struct Run {
    workspace_name: Option<String>,
    workflow_name: Option<String>,
    node_label: Option<String>,
    phase: Phase,
    started_ms: u64,
    ended_ms: Option<u64>,
    /// Orders the runs by their last news: the latest is the one shown.
    seq: u64,
}

#[derive(Debug, Default)]
struct State {
    runs: BTreeMap<String, Run>,
    /// Executions outside a run's reach: a chat with an agent, a spec turn… (workflow steps are
    /// in here too; the run they belong to is what gets shown).
    executions: BTreeSet<String>,
    seq: u64,
    opened_ms: u64,
}

enum Msg {
    Poke,
}

struct Shared {
    state: Mutex<State>,
    env: Arc<dyn PresenceEnv>,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock_or_recover()
    }
}

/// Use case: keeps the Discord profile in step with Atlas.
pub struct PresenceService {
    shared: Arc<Shared>,
    wake: Sender<Msg>,
}

impl PresenceService {
    /// Starts the worker thread. It stops, clearing the profile, when the service is dropped.
    pub fn start(env: Arc<dyn PresenceEnv>, transport: Arc<dyn PresenceTransport>) -> Self {
        Self::start_with(env, transport, MIN_INTERVAL, RETRY_AFTER)
    }

    fn start_with(
        env: Arc<dyn PresenceEnv>,
        transport: Arc<dyn PresenceTransport>,
        min_interval: Duration,
        retry_after: Duration,
    ) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                opened_ms: now_ms(),
                ..State::default()
            }),
            env,
        });
        let (wake, inbox) = mpsc::channel();
        let worker = shared.clone();
        thread::Builder::new()
            .name("atlas-presence".to_owned())
            .spawn(move || {
                run_worker(
                    &worker,
                    &inbox,
                    transport.as_ref(),
                    min_interval,
                    retry_after,
                );
            })
            .ok();
        let service = Self { shared, wake };
        service.poke();
        service
    }

    /// Something that decides what is shown changed (the selected workspace, the settings).
    pub fn poke(&self) {
        // The worker is gone only when the process is closing.
        let _ = self.wake.send(Msg::Poke);
    }

    pub fn on_workflow_event(&self, event: &WorkflowEvent) {
        use WorkflowEventKind as K;
        let env = &self.shared.env;
        {
            let mut state = self.shared.lock();
            state.seq += 1;
            let seq = state.seq;
            let id = event.execution_id.clone();
            if event.kind == K::Started {
                state.runs.insert(
                    id.clone(),
                    Run {
                        workspace_name: env.workspace_name(&event.workspace_id),
                        workflow_name: env.workflow_name(&event.workflow_id),
                        node_label: None,
                        phase: Phase::Running,
                        started_ms: event.timestamp,
                        ended_ms: None,
                        seq,
                    },
                );
            } else if let Some(run) = state.runs.get_mut(&id) {
                let phase = match event.kind {
                    K::NodeStarted
                    | K::NodeApprovalResolved
                    | K::NodeInputResolved
                    | K::InteractionAnswered
                    | K::Resumed => Some(Phase::Running),
                    K::NodeWaitingApproval | K::NodeWaitingForInput | K::InteractionDetected => {
                        Some(Phase::Waiting)
                    }
                    K::Paused => Some(Phase::Paused),
                    K::Completed => Some(Phase::Completed),
                    K::Failed | K::Interrupted => Some(Phase::Failed),
                    K::Cancelled => Some(Phase::Cancelled),
                    _ => None,
                };
                if event.kind == K::NodeStarted {
                    if let Some(node) = &event.node_id {
                        run.node_label = env.node_label(&event.workflow_id, node);
                    }
                }
                if let Some(phase) = phase {
                    // A finished run stays finished: late events do not bring it back.
                    if !run.phase.is_final() {
                        run.phase = phase;
                        if phase.is_final() {
                            run.ended_ms = Some(event.timestamp.max(run.started_ms));
                        }
                        run.seq = seq;
                    }
                }
            }
        }
        self.poke();
    }

    pub fn on_execution_event(&self, event: &ExecutionEvent) {
        let changed = {
            let mut state = self.shared.lock();
            match event.kind {
                ExecutionEventKind::Started => state.executions.insert(event.execution_id.clone()),
                ExecutionEventKind::Completed
                | ExecutionEventKind::Failed
                | ExecutionEventKind::Cancelled
                | ExecutionEventKind::ProcessExited => state.executions.remove(&event.execution_id),
                _ => false,
            }
        };
        if changed {
            self.poke();
        }
    }
}

/// What to show now, and when it will have changed by itself (a finished run leaving the profile).
struct Desired {
    published: Option<Published>,
    wake_in: Option<Duration>,
}

fn desired(shared: &Shared) -> Desired {
    let settings = shared.env.settings();
    let client_id = settings.resolved_client_id();
    if !settings.enabled {
        return Desired {
            published: None,
            wake_in: None,
        };
    }
    let language = shared.env.language();
    let selected = shared.env.selected_workspace_name();
    let now = now_ms();
    let state = shared.lock();
    let activity = compose(&state, &settings, &language, selected.as_deref(), now);
    let wake_in = state
        .runs
        .values()
        .filter_map(|run| run.ended_ms)
        .map(|ended| (ended + FINISHED_WINDOW_MS).saturating_sub(now))
        .filter(|left| *left > 0)
        .min()
        .map(|left| Duration::from_millis(left + 50));
    Desired {
        published: Some(Published {
            client_id,
            activity,
        }),
        wake_in,
    }
}

fn run_worker(
    shared: &Shared,
    inbox: &mpsc::Receiver<Msg>,
    transport: &dyn PresenceTransport,
    min_interval: Duration,
    retry_after: Duration,
) {
    // `Some(x)`: Discord shows `x`. `None`: unknown (a send failed). Starts as "nothing shown", so
    // that a person who never enables the feature never makes Atlas touch Discord.
    let mut shown: Option<Option<Published>> = Some(None);
    let mut last_sent: Option<Instant> = None;
    let mut timeout: Option<Duration> = None;
    loop {
        let received = match timeout {
            Some(wait) => inbox.recv_timeout(wait),
            None => inbox.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        if matches!(received, Err(RecvTimeoutError::Disconnected)) {
            // Atlas is closing: take the activity down while the socket still works.
            if shown != Some(None) {
                let _ = transport.publish(None);
            }
            return;
        }
        // Several wake-ups in a row are one: only the state matters.
        while inbox.try_recv().is_ok() {}
        let want = desired(shared);
        timeout = want.wake_in;
        if shown.as_ref() == Some(&want.published) {
            continue;
        }
        if let Some(at) = last_sent {
            let waited = at.elapsed();
            if waited < min_interval {
                timeout = Some(min_interval.saturating_sub(waited));
                continue;
            }
        }
        last_sent = Some(Instant::now());
        match transport.publish(want.published.as_ref()) {
            Ok(()) => shown = Some(want.published),
            Err(error) => {
                eprintln!("discord presence: {error}");
                shown = None;
                timeout = Some(retry_after.min(timeout.unwrap_or(retry_after)));
            }
        }
    }
}

/// Words of the profile in the person's language.
struct Words {
    english: bool,
}

impl Words {
    fn pick<'a>(&self, pt: &'a str, en: &'a str) -> &'a str {
        if self.english {
            en
        } else {
            pt
        }
    }
}

#[allow(clippy::too_many_lines)] // one branch per thing Atlas can be doing
fn compose(
    state: &State,
    settings: &PresenceSettings,
    language: &str,
    selected_workspace: Option<&str>,
    now: u64,
) -> Activity {
    let w = Words {
        english: language == "en-US",
    };
    let names = settings.show_names;
    let secs = |ms: u64| Some(ms / 1000);

    let active = state
        .runs
        .values()
        .filter(|run| !run.phase.is_final())
        .max_by_key(|run| run.seq);
    let finished = state
        .runs
        .values()
        .filter(|run| run.ended_ms.is_some_and(|at| now < at + FINISHED_WINDOW_MS))
        .max_by_key(|run| run.seq);

    if let Some(run) = active {
        let others = state.runs.values().filter(|r| !r.phase.is_final()).count() - 1;
        let workflow = match (&run.workflow_name, names) {
            (Some(name), true) => format!("Workflow: {name}"),
            _ => w
                .pick("Executando um workflow", "Running a workflow")
                .to_owned(),
        };
        let mut line = match run.phase {
            Phase::Waiting => w.pick("Aguardando você", "Waiting for you").to_owned(),
            Phase::Paused => w.pick("Pausado", "Paused").to_owned(),
            _ => match (&run.node_label, names) {
                (Some(step), true) => format!("{} {step}", w.pick("Passo:", "Step:")),
                _ => w.pick("Em andamento", "In progress").to_owned(),
            },
        };
        if others > 0 {
            let _ = write!(line, " (+{others})");
        }
        let (image, hint) = match run.phase {
            Phase::Waiting => ("waiting", w.pick("Aguardando você", "Waiting for you")),
            Phase::Paused => ("paused", w.pick("Pausado", "Paused")),
            _ => ("running", w.pick("Executando", "Running")),
        };
        return Activity {
            details: workflow,
            state: line,
            started_at: secs(run.started_ms),
            large_text: workspace_hint(names, run.workspace_name.as_deref()),
            small_image: Some(image),
            small_text: Some(hint.to_owned()),
        };
    }

    if let Some(run) = finished {
        let (headline, image) = match run.phase {
            Phase::Completed => (w.pick("Workflow concluído", "Workflow completed"), "done"),
            Phase::Failed => (w.pick("Workflow falhou", "Workflow failed"), "failed"),
            _ => (w.pick("Workflow cancelado", "Workflow cancelled"), "failed"),
        };
        let name = match (&run.workflow_name, names) {
            (Some(name), true) => name.clone(),
            _ => "Atlas".to_owned(),
        };
        return Activity {
            details: headline.to_owned(),
            state: name,
            started_at: None,
            large_text: workspace_hint(names, run.workspace_name.as_deref()),
            small_image: Some(image),
            small_text: Some(headline.to_owned()),
        };
    }

    let working = state.executions.len();
    if working > 0 {
        let count = if working == 1 {
            w.pick("1 agente trabalhando", "1 agent working").to_owned()
        } else if w.english {
            format!("{working} agents working")
        } else {
            format!("{working} agentes trabalhando")
        };
        return Activity {
            details: w
                .pick("Orquestrando agentes", "Orchestrating agents")
                .to_owned(),
            state: count,
            started_at: secs(state.opened_ms),
            large_text: workspace_hint(names, selected_workspace),
            small_image: Some("running"),
            small_text: Some(w.pick("Executando", "Running").to_owned()),
        };
    }

    match (selected_workspace, names) {
        (Some(name), true) => Activity {
            details: format!("Workspace: {name}"),
            state: w
                .pick("Navegando no workspace", "Browsing the workspace")
                .to_owned(),
            started_at: secs(state.opened_ms),
            large_text: None,
            small_image: Some("idle"),
            small_text: Some(w.pick("Ocioso", "Idle").to_owned()),
        },
        (Some(_), false) => Activity {
            details: w.pick("Em um workspace", "In a workspace").to_owned(),
            state: w.pick("Navegando", "Browsing").to_owned(),
            started_at: secs(state.opened_ms),
            large_text: None,
            small_image: Some("idle"),
            small_text: Some(w.pick("Ocioso", "Idle").to_owned()),
        },
        (None, _) => Activity {
            details: "Atlas".to_owned(),
            state: w.pick("Na tela inicial", "On the home screen").to_owned(),
            started_at: secs(state.opened_ms),
            large_text: None,
            small_image: None,
            small_text: None,
        },
    }
}

fn workspace_hint(names: bool, workspace: Option<&str>) -> Option<String> {
    match (workspace, names) {
        (Some(name), true) => Some(format!("Workspace: {name}")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    struct FakeEnv {
        settings: Mutex<PresenceSettings>,
        language: &'static str,
        selected: Mutex<Option<String>>,
    }

    impl FakeEnv {
        fn new(enabled: bool) -> Arc<Self> {
            Arc::new(Self {
                settings: Mutex::new(PresenceSettings {
                    enabled,
                    show_names: true,
                    client_id: Some("123".to_owned()),
                }),
                language: "en-US",
                selected: Mutex::new(Some("Billing".to_owned())),
            })
        }
    }

    impl PresenceEnv for FakeEnv {
        fn settings(&self) -> PresenceSettings {
            self.settings.lock().unwrap().clone()
        }
        fn language(&self) -> String {
            self.language.to_owned()
        }
        fn selected_workspace_name(&self) -> Option<String> {
            self.selected.lock().unwrap().clone()
        }
        fn workspace_name(&self, _: &str) -> Option<String> {
            Some("Billing".to_owned())
        }
        fn workflow_name(&self, _: &str) -> Option<String> {
            Some("Ship it".to_owned())
        }
        fn node_label(&self, _: &str, node: &str) -> Option<String> {
            Some(format!("step {node}"))
        }
    }

    #[derive(Default)]
    struct FakeTransport {
        calls: Mutex<Vec<Option<Published>>>,
        fail: AtomicBool,
    }

    impl PresenceTransport for FakeTransport {
        fn publish(&self, published: Option<&Published>) -> Result<(), String> {
            if self.fail.load(Ordering::SeqCst) {
                return Err("discord is closed".to_owned());
            }
            self.calls.lock().unwrap().push(published.cloned());
            Ok(())
        }
    }

    fn workflow_event(kind: WorkflowEventKind, node: Option<&str>) -> WorkflowEvent {
        WorkflowEvent {
            kind,
            workflow_id: "wf".to_owned(),
            execution_id: "run-1".to_owned(),
            workspace_id: "ws".to_owned(),
            node_id: node.map(str::to_owned),
            message: "never shown".to_owned(),
            timestamp: now_ms(),
            metadata: BTreeMap::new(),
        }
    }

    fn start(env: &Arc<FakeEnv>, transport: &Arc<FakeTransport>) -> PresenceService {
        PresenceService::start_with(
            env.clone(),
            transport.clone(),
            Duration::from_millis(10),
            Duration::from_millis(30),
        )
    }

    fn last(transport: &FakeTransport) -> Option<Published> {
        transport.calls.lock().unwrap().last().cloned().flatten()
    }

    fn wait_for(what: &str, check: impl Fn() -> bool) {
        for _ in 0..200 {
            if check() {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("timed out waiting for {what}");
    }

    #[test]
    fn disabled_never_touches_discord() {
        let (env, transport) = (FakeEnv::new(false), Arc::new(FakeTransport::default()));
        let service = start(&env, &transport);
        service.on_workflow_event(&workflow_event(WorkflowEventKind::Started, None));
        thread::sleep(Duration::from_millis(80));

        assert!(transport.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn shows_the_workspace_then_the_run_then_the_result() {
        let (env, transport) = (FakeEnv::new(true), Arc::new(FakeTransport::default()));
        let service = start(&env, &transport);

        wait_for("workspace", || {
            last(&transport).is_some_and(|p| p.activity.details == "Workspace: Billing")
        });
        service.on_workflow_event(&workflow_event(WorkflowEventKind::Started, None));
        service.on_workflow_event(&workflow_event(WorkflowEventKind::NodeStarted, Some("n1")));
        wait_for("step", || {
            last(&transport).is_some_and(|p| {
                p.activity.details == "Workflow: Ship it" && p.activity.state == "Step: step n1"
            })
        });
        service.on_workflow_event(&workflow_event(
            WorkflowEventKind::NodeWaitingApproval,
            Some("n1"),
        ));
        wait_for("waiting", || {
            last(&transport).is_some_and(|p| p.activity.state == "Waiting for you")
        });
        service.on_workflow_event(&workflow_event(WorkflowEventKind::Completed, None));
        wait_for("done", || {
            last(&transport).is_some_and(|p| p.activity.details == "Workflow completed")
        });
        assert_eq!(last(&transport).unwrap().client_id, "123");
    }

    #[test]
    fn hides_the_names_when_asked() {
        let env = FakeEnv::new(true);
        env.settings.lock().unwrap().show_names = false;
        let transport = Arc::new(FakeTransport::default());
        let service = start(&env, &transport);
        service.on_workflow_event(&workflow_event(WorkflowEventKind::Started, None));
        service.on_workflow_event(&workflow_event(WorkflowEventKind::NodeStarted, Some("n1")));

        wait_for("generic text", || {
            last(&transport).is_some_and(|p| p.activity.details == "Running a workflow")
        });
        let shown = format!("{:?}", transport.calls.lock().unwrap());
        for secret in ["Billing", "Ship it", "step n1"] {
            assert!(!shown.contains(secret), "{secret} leaked: {shown}");
        }
    }

    #[test]
    fn retries_after_discord_was_closed_and_clears_on_disable() {
        let (env, transport) = (FakeEnv::new(true), Arc::new(FakeTransport::default()));
        transport.fail.store(true, Ordering::SeqCst);
        let service = start(&env, &transport);
        thread::sleep(Duration::from_millis(60));
        assert!(transport.calls.lock().unwrap().is_empty());

        transport.fail.store(false, Ordering::SeqCst);
        wait_for("retry", || last(&transport).is_some());

        env.settings.lock().unwrap().enabled = false;
        service.poke();
        wait_for("clear", || {
            matches!(transport.calls.lock().unwrap().last(), Some(None))
        });
    }

    #[test]
    fn a_burst_of_events_is_one_update() {
        let (env, transport) = (FakeEnv::new(true), Arc::new(FakeTransport::default()));
        let service = start(&env, &transport);
        wait_for("first", || last(&transport).is_some());
        let before = transport.calls.lock().unwrap().len();

        for i in 0..50 {
            service.on_workflow_event(&workflow_event(
                WorkflowEventKind::NodeStarted,
                Some(&format!("n{i}")),
            ));
        }
        service.on_workflow_event(&workflow_event(WorkflowEventKind::Started, None));
        wait_for("run", || {
            last(&transport).is_some_and(|p| p.activity.details == "Workflow: Ship it")
        });

        assert!(transport.calls.lock().unwrap().len() - before <= 2);
    }

    #[test]
    fn dropping_the_service_clears_the_profile() {
        let (env, transport) = (FakeEnv::new(true), Arc::new(FakeTransport::default()));
        let service = start(&env, &transport);
        wait_for("shown", || last(&transport).is_some());
        drop(service);

        wait_for("cleared", || {
            matches!(transport.calls.lock().unwrap().last(), Some(None))
        });
    }
}
