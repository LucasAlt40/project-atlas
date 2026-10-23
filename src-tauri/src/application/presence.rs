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
//! - the screen the person has open is only its name (`Screen`): it picks a line of the profile when
//!   nothing is running. What is on the screen never reaches it;
//! - the events only mutate [`State`] and wake the worker. What is shown is always composed from the
//!   state *at the moment it is sent*, so bursts collapse into one update.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use super::settings::SettingsService;
use super::specs::service::SpecService;
use super::support::now_ms;
use super::workflow::service::WorkflowService;
use super::workspace::WorkspaceService;
use crate::application::support::LockExt;
use crate::domain::execution::{ExecutionEvent, ExecutionEventKind};
use crate::domain::presence::{Activity, PresenceSettings, Screen, DEFAULT_CLIENT_ID};
use crate::domain::spec::SpecStatus;
use crate::domain::workflow::{WorkflowEvent, WorkflowEventKind};

/// Discord drops updates beyond about five per twenty seconds.
const MIN_INTERVAL: Duration = Duration::from_secs(4);
/// How long to wait before trying again after Discord could not be reached.
const RETRY_AFTER: Duration = Duration::from_secs(15);
/// How long a finished run stays on the profile before the workspace takes its place.
const FINISHED_WINDOW_MS: u64 = 30_000;
/// How often a specification in progress is looked at again.
const SPEC_RECHECK: Duration = Duration::from_secs(15);

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
    /// Specifications of the selected workspace that are being written or wait for the person.
    fn spec_work(&self) -> Vec<SpecWork>;
}

/// What a specification is doing right now, for the profile. The title is shown only when the person
/// allowed names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecWork {
    pub title: String,
    /// The document at hand (`proposal`, `specs`, `design`, `tasks`).
    pub stage: Option<String>,
    pub phase: SpecPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecPhase {
    /// An agent is writing a document.
    Writing,
    /// A question, a review or the last word waits for the person.
    WaitingForYou,
}

/// [`PresenceEnv`] over the real services.
pub struct AtlasEnv {
    pub settings: Arc<SettingsService>,
    pub workspaces: Arc<WorkspaceService>,
    pub workflows: Arc<WorkflowService>,
    pub specs: Arc<SpecService>,
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

    fn spec_work(&self) -> Vec<SpecWork> {
        let Some(workspace) = self.settings.get().selected_workspace_id else {
            return Vec::new();
        };
        self.specs
            .list(&workspace)
            .into_iter()
            .filter_map(|spec| {
                let phase = match spec.status {
                    SpecStatus::Generating => SpecPhase::Writing,
                    SpecStatus::WaitingForInput
                    | SpecStatus::AwaitingReview
                    | SpecStatus::Approved => SpecPhase::WaitingForYou,
                    _ => return None,
                };
                Some(SpecWork {
                    title: spec.title,
                    stage: spec.current_artifact,
                    phase,
                })
            })
            .collect()
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
    /// The screen the webview said is open.
    screen: Option<Screen>,
    /// Since when a specification has been written without a break; the profile's timer.
    writing_since: Option<u64>,
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

    /// The person opened another screen.
    pub fn set_screen(&self, screen: Screen) {
        let changed = {
            let mut state = self.shared.lock();
            let changed = state.screen != Some(screen);
            state.screen = Some(screen);
            changed
        };
        if changed {
            self.poke();
        }
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
    if !settings.enabled {
        return Desired {
            published: None,
            wake_in: None,
        };
    }
    let language = shared.env.language();
    let selected = shared.env.selected_workspace_name();
    let spec_work = shared.env.spec_work();
    let now = now_ms();
    let mut state = shared.lock();
    // The timer of a specification being written starts when it was first seen so, and stops with it.
    if spec_work.iter().any(|w| w.phase == SpecPhase::Writing) {
        state.writing_since.get_or_insert(now);
    } else {
        state.writing_since = None;
    }
    let activity = compose(
        &state,
        &settings,
        &language,
        selected.as_deref(),
        now,
        &spec_work,
    );
    let finished_in = state
        .runs
        .values()
        .filter_map(|run| run.ended_ms)
        .map(|ended| (ended + FINISHED_WINDOW_MS).saturating_sub(now))
        .filter(|left| *left > 0)
        .min()
        .map(|left| Duration::from_millis(left + 50));
    // A specification changes without any execution event telling it (a question answered, a
    // document approved): look again now and then while one is in progress.
    let spec_in = (!spec_work.is_empty()).then_some(SPEC_RECHECK);
    let wake_in = match (finished_in, spec_in) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    Desired {
        published: Some(Published {
            client_id: DEFAULT_CLIENT_ID.to_owned(),
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
                log::warn!("discord presence: {error}");
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

    fn pick_static(&self, pt: &'static str, en: &'static str) -> &'static str {
        self.pick(pt, en)
    }
}

#[allow(clippy::too_many_lines)] // one branch per thing Atlas can be doing
fn compose(
    state: &State,
    settings: &PresenceSettings,
    language: &str,
    selected_workspace: Option<&str>,
    now: u64,
    spec_work: &[SpecWork],
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

    if let Some(activity) = compose_specs(
        state,
        &w,
        names,
        selected_workspace,
        spec_work,
        SpecPhase::Writing,
    ) {
        return activity;
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

    if let Some(activity) = compose_specs(
        state,
        &w,
        names,
        selected_workspace,
        spec_work,
        SpecPhase::WaitingForYou,
    ) {
        return activity;
    }

    if let Some(activity) = compose_screen(state, &w, names, selected_workspace) {
        return activity;
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

/// The name of a specification document, in the person's language.
fn stage_name(w: &Words, stage: &str) -> &'static str {
    match stage {
        "proposal" => w.pick_static("Proposta", "Proposal"),
        "specs" => w.pick_static("Requisitos", "Requirements"),
        "design" => "Design",
        "tasks" => w.pick_static("Tarefas", "Tasks"),
        _ => w.pick_static("Documento", "Document"),
    }
}

/// A specification being written or waiting for the person (the SDD flow).
fn compose_specs(
    state: &State,
    w: &Words,
    names: bool,
    selected_workspace: Option<&str>,
    spec_work: &[SpecWork],
    phase: SpecPhase,
) -> Option<Activity> {
    let mut mine = spec_work.iter().filter(|work| work.phase == phase);
    let first = mine.next()?;
    let more = mine.count();
    let details = match (names, phase) {
        (true, _) => format!(
            "{} {}",
            w.pick("Especificação:", "Specification:"),
            first.title
        ),
        (false, SpecPhase::Writing) => w
            .pick("Escrevendo uma especificação", "Writing a specification")
            .to_owned(),
        (false, SpecPhase::WaitingForYou) => w
            .pick("Especificação (SDD)", "Specification (SDD)")
            .to_owned(),
    };
    let mut line = match phase {
        SpecPhase::Writing => match &first.stage {
            Some(document) => format!(
                "{} {}",
                w.pick("Escrevendo:", "Writing:"),
                stage_name(w, document)
            ),
            None => w.pick("Agente escrevendo", "Agent writing").to_owned(),
        },
        SpecPhase::WaitingForYou => w.pick("Aguardando você", "Waiting for you").to_owned(),
    };
    if more > 0 {
        let _ = write!(line, " (+{more})");
    }
    let (image, hint) = match phase {
        SpecPhase::Writing => ("running", w.pick("Escrevendo", "Writing")),
        SpecPhase::WaitingForYou => ("waiting", w.pick("Aguardando você", "Waiting for you")),
    };
    Some(Activity {
        details,
        state: line,
        started_at: match phase {
            SpecPhase::Writing => state.writing_since.map(|ms| ms / 1000),
            SpecPhase::WaitingForYou => Some(state.opened_ms / 1000),
        },
        large_text: workspace_hint(names, selected_workspace),
        small_image: Some(image),
        small_text: Some(hint.to_owned()),
    })
}

/// What the person is doing on a screen, when nothing is running. The screen's name only.
fn compose_screen(
    state: &State,
    w: &Words,
    names: bool,
    selected_workspace: Option<&str>,
) -> Option<Activity> {
    let (details, line) = match state.screen? {
        // The workspace has its own lines, with its name.
        Screen::Workspace => return None,
        Screen::Workflow => (
            w.pick("Workflows", "Workflows"),
            w.pick("Montando um workflow", "Building a workflow"),
        ),
        Screen::Agents => (
            w.pick("Agentes", "Agents"),
            w.pick("Configurando agentes", "Configuring agents"),
        ),
        Screen::Personalities => (
            w.pick("Personalidades", "Personalities"),
            w.pick("Ajustando personalidades", "Tuning personalities"),
        ),
        Screen::Specs => (
            w.pick("Especificações (SDD)", "Specifications (SDD)"),
            w.pick("Lendo e aprovando specs", "Reading and approving specs"),
        ),
        Screen::McpHub => (
            "MCP Hub",
            w.pick("Conectando ferramentas", "Connecting tools"),
        ),
        Screen::Settings => (
            w.pick("Configurações", "Settings"),
            w.pick("Ajustando o Atlas", "Adjusting Atlas"),
        ),
    };
    Some(Activity {
        details: details.to_owned(),
        state: line.to_owned(),
        started_at: Some(state.opened_ms / 1000),
        large_text: workspace_hint(names, selected_workspace),
        small_image: Some("idle"),
        small_text: Some(w.pick("Ocioso", "Idle").to_owned()),
    })
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
        specs: Mutex<Vec<SpecWork>>,
    }

    impl FakeEnv {
        fn new(enabled: bool) -> Arc<Self> {
            Arc::new(Self {
                settings: Mutex::new(PresenceSettings {
                    enabled,
                    show_names: true,
                }),
                language: "en-US",
                selected: Mutex::new(Some("Billing".to_owned())),
                specs: Mutex::new(Vec::new()),
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
        fn spec_work(&self) -> Vec<SpecWork> {
            self.specs.lock().unwrap().clone()
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
        assert_eq!(last(&transport).unwrap().client_id, DEFAULT_CLIENT_ID);
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

    fn spec(title: &str, stage: &str, phase: SpecPhase) -> SpecWork {
        SpecWork {
            title: title.to_owned(),
            stage: Some(stage.to_owned()),
            phase,
        }
    }

    #[test]
    fn shows_the_specification_being_written_and_then_the_one_waiting() {
        let (env, transport) = (FakeEnv::new(true), Arc::new(FakeTransport::default()));
        env.specs
            .lock()
            .unwrap()
            .push(spec("Dark mode", "design", SpecPhase::Writing));
        let service = start(&env, &transport);

        wait_for("writing", || {
            last(&transport).is_some_and(|p| {
                p.activity.details == "Specification: Dark mode"
                    && p.activity.state == "Writing: Design"
                    && p.activity.started_at.is_some()
            })
        });

        *env.specs.lock().unwrap() = vec![spec("Dark mode", "design", SpecPhase::WaitingForYou)];
        service.poke();
        wait_for("waiting", || {
            last(&transport).is_some_and(|p| {
                p.activity.state == "Waiting for you" && p.activity.small_image == Some("waiting")
            })
        });
    }

    #[test]
    fn a_workflow_run_comes_before_a_specification() {
        let (env, transport) = (FakeEnv::new(true), Arc::new(FakeTransport::default()));
        env.specs
            .lock()
            .unwrap()
            .push(spec("Dark mode", "design", SpecPhase::Writing));
        let service = start(&env, &transport);
        service.on_workflow_event(&workflow_event(WorkflowEventKind::Started, None));

        wait_for("run", || {
            last(&transport).is_some_and(|p| p.activity.details == "Workflow: Ship it")
        });
    }

    #[test]
    fn the_screen_picks_the_line_when_nothing_runs_and_never_carries_content() {
        let (env, transport) = (FakeEnv::new(true), Arc::new(FakeTransport::default()));
        let service = start(&env, &transport);
        service.set_screen(Screen::Specs);
        wait_for("specs screen", || {
            last(&transport).is_some_and(|p| {
                p.activity.details == "Specifications (SDD)"
                    && p.activity.state == "Reading and approving specs"
            })
        });
        service.set_screen(Screen::McpHub);
        wait_for("hub screen", || {
            last(&transport).is_some_and(|p| p.activity.details == "MCP Hub")
        });
        // The workspace screen keeps the workspace's own lines.
        service.set_screen(Screen::Workspace);
        wait_for("workspace", || {
            last(&transport).is_some_and(|p| p.activity.details == "Workspace: Billing")
        });
    }

    #[test]
    fn without_names_a_specification_is_generic() {
        let env = FakeEnv::new(true);
        env.settings.lock().unwrap().show_names = false;
        env.specs
            .lock()
            .unwrap()
            .push(spec("Secret title", "tasks", SpecPhase::Writing));
        let transport = Arc::new(FakeTransport::default());
        let _service = start(&env, &transport);

        wait_for("generic", || {
            last(&transport).is_some_and(|p| p.activity.details == "Writing a specification")
        });
        let shown = format!("{:?}", transport.calls.lock().unwrap());
        assert!(
            !shown.contains("Secret title") && !shown.contains("Billing"),
            "{shown}"
        );
    }
}
