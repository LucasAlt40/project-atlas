//! The Live Workspace: what the files of a workflow run's isolated worktree look like *while the
//! agents are working*, observed for real.
//!
//! ```text
//! worktree folder --(filesystem events)--> debounce --> affected paths
//!        |                                                    |
//!        |  (slow full check; polling if events are           v
//!        |   unavailable)                              LiveTracker  <-- Git: files vs baseline
//!        v                                                    |
//!   full reconcile ----------------------------------------> state + LiveWorkspaceUpdate
//!                                                             |
//!                                           LiveSink (Tauri event) / state() (a command)
//! ```
//!
//! Rules this module keeps:
//!
//! - it observes the run's worktree and nothing else; the folder comes from what Atlas stored
//!   ([`WorktreeService::live_target`]), never from a path a caller sends;
//! - the files, read through Git, are the only source of the state: no agent text feeds it;
//! - it only reads. It takes no lock an agent's own Git would wait for, and never commits,
//!   merges, stages or writes in the worktree;
//! - one event is never "the final state": events only say *where to look*, after the files have
//!   been quiet for a moment. The whole worktree is read again whenever the run changes phase,
//!   ends, or anything looks wrong.

mod tracker;

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub use tracker::LiveTracker;

use super::errors::{AppError, ErrorCode};
use super::workflow::service::WorkflowService;
use super::worktree::{LiveTargetError, WorktreeError, WorktreeService};
use crate::domain::live_workspace::{
    is_plain_relative_path, LiveFile, LivePhase, LiveWorkspaceState, LiveWorkspaceUpdate,
    ObservationMode, WorktreeAvailability,
};
use crate::domain::workflow::{IntegrationStatus, WorkflowExecutionStatus};

/// What the filesystem reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchSignal {
    /// These paths (absolute) were touched in some way. Which way does not matter: the files are
    /// read to find out.
    Paths(Vec<PathBuf>),
    /// Events may have been lost (queue overflow, watcher error): look at everything.
    Rescan,
}

/// Port: filesystem events for a folder. Implemented in `infrastructure/` over the platform's
/// facility; tests use one they can drive.
pub trait WorktreeWatcher: Send + Sync {
    /// Starts reporting changes under `root` (recursively) to `on_signal`, until the returned
    /// guard is dropped.
    ///
    /// # Errors
    ///
    /// Why it cannot watch (the folder is gone, the platform's limit is reached…). The caller
    /// then falls back to looking on a schedule.
    fn watch(
        &self,
        root: &Path,
        on_signal: Box<dyn Fn(WatchSignal) + Send + Sync>,
    ) -> Result<Box<dyn Send>, String>;
}

/// Port: where updates go (the webview, as a Tauri event). Best effort: the state can always be
/// read again.
pub trait LiveSink: Send + Sync {
    fn on_update(&self, update: &LiveWorkspaceUpdate);
}

/// Discards updates. For when nobody listens.
#[cfg(test)]
pub struct NoLiveSink;

#[cfg(test)]
impl LiveSink for NoLiveSink {
    fn on_update(&self, _update: &LiveWorkspaceUpdate) {}
}

/// How long things wait. Defaults suit a person watching; tests shorten them.
#[derive(Debug, Clone, Copy)]
pub struct LiveConfig {
    /// A batch of events is read once no event has arrived for this long: a file written in
    /// several pieces, or saved several times in a row, is read once, after it settles.
    pub quiet: Duration,
    /// …but never later than this after the first event of the batch, so a worktree that is
    /// written to without a pause still shows its progress.
    pub max_wait: Duration,
    /// With events working: how often everything is compared anyway, in case an event was lost.
    pub safety_running: Duration,
    pub safety_idle: Duration,
    /// Without events: the first interval of the schedule, and the slowest it gets (it doubles
    /// each time nothing changed, and starts again when something does).
    pub poll_min: Duration,
    pub poll_max: Duration,
}

impl Default for LiveConfig {
    fn default() -> Self {
        Self {
            quiet: Duration::from_millis(150),
            max_wait: Duration::from_millis(1000),
            safety_running: Duration::from_secs(10),
            safety_idle: Duration::from_secs(30),
            poll_min: Duration::from_millis(500),
            poll_max: Duration::from_secs(5),
        }
    }
}

/// Why the Live Workspace cannot be given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveError {
    NotFound,
    Missing,
    Invalid,
    /// The observer did not answer in time.
    Unresponsive,
    /// A path that is not a plain path inside the worktree.
    InvalidPath,
    /// Git or the filesystem failed.
    Failed(String),
}

impl From<WorktreeError> for LiveError {
    fn from(error: WorktreeError) -> Self {
        match error {
            WorktreeError::InvalidIdentifier(_) | WorktreeError::OutsideRoot(_) => {
                Self::InvalidPath
            }
            other => Self::Failed(other.to_string()),
        }
    }
}

impl From<LiveTargetError> for LiveError {
    fn from(error: LiveTargetError) -> Self {
        match error {
            LiveTargetError::Unknown => Self::NotFound,
            LiveTargetError::Missing => Self::Missing,
            LiveTargetError::Invalid => Self::Invalid,
        }
    }
}

impl From<LiveError> for AppError {
    fn from(error: LiveError) -> Self {
        let detail = match &error {
            LiveError::Failed(why) => Some(why.clone()),
            _ => None,
        };
        let app = AppError::new(match error {
            LiveError::NotFound | LiveError::Missing => ErrorCode::WorktreeNotFound,
            LiveError::Invalid | LiveError::InvalidPath => ErrorCode::WorktreeInvalidState,
            LiveError::Unresponsive => ErrorCode::WorktreeBusy,
            LiveError::Failed(_) => ErrorCode::WorktreeFailed,
        });
        match detail {
            Some(detail) => app.with_detail(detail),
            None => app,
        }
    }
}

enum Command {
    Signal(WatchSignal),
    Phase(LivePhase),
    /// Compare everything now, then say so.
    Reconcile(Sender<()>),
    /// The run is over: compare everything, record the final phase, stop observing.
    Finish(LivePhase, Sender<()>),
}

struct Session {
    commands: Sender<Command>,
    state: Arc<Mutex<LiveWorkspaceState>>,
    thread: Option<JoinHandle<()>>,
}

impl Session {
    fn alive(&self) -> bool {
        self.thread.as_ref().is_some_and(|t| !t.is_finished())
    }
}

pub struct LiveWorkspaceService {
    worktrees: Arc<WorktreeService>,
    workflows: Arc<WorkflowService>,
    watcher: Arc<dyn WorktreeWatcher>,
    sink: Arc<dyn LiveSink>,
    config: LiveConfig,
    sessions: Mutex<HashMap<String, Session>>,
    /// The last state of runs whose observation ended, so it can still be read (and its
    /// revision continued if the run's worktree is picked up again).
    finished: Mutex<HashMap<String, LiveWorkspaceState>>,
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl LiveWorkspaceService {
    pub fn new(
        worktrees: Arc<WorktreeService>,
        workflows: Arc<WorkflowService>,
        watcher: Arc<dyn WorktreeWatcher>,
        sink: Arc<dyn LiveSink>,
    ) -> Self {
        Self {
            worktrees,
            workflows,
            watcher,
            sink,
            config: LiveConfig::default(),
            sessions: Mutex::new(HashMap::new()),
            finished: Mutex::new(HashMap::new()),
        }
    }

    #[cfg(test)]
    #[must_use]
    pub fn with_config(mut self, config: LiveConfig) -> Self {
        self.config = config;
        self
    }

    /// Starts observing a workflow's worktree (or returns what is already being observed). The
    /// worktree may already hold changes (a run being resumed): the first look reads them all.
    ///
    /// # Errors
    ///
    /// If the worktree is not one that may be observed.
    pub fn start(&self, primary_id: &str) -> Result<LiveWorkspaceState, LiveError> {
        let mut sessions = locked(&self.sessions);
        if let Some(session) = sessions.get(primary_id) {
            if session.alive() {
                return Ok(locked(&session.state).clone());
            }
        }
        let target = self.worktrees.live_target(primary_id)?;
        let previous = sessions
            .remove(primary_id)
            .map(|s| locked(&s.state).clone())
            .or_else(|| locked(&self.finished).remove(primary_id));
        let (commands, inbox) = mpsc::channel();
        // The watcher starts first: whatever happens during the first look is queued, and
        // reading a path twice is harmless.
        let forward = commands.clone();
        let guard = self
            .watcher
            .watch(
                &target.path,
                Box::new(move |signal| {
                    let _ = forward.send(Command::Signal(signal));
                }),
            )
            .ok();
        let mut tracker = LiveTracker::new(
            self.worktrees.clone(),
            target.clone(),
            previous.map_or(0, |p| p.revision),
        );
        tracker.set_observation(if guard.is_some() {
            ObservationMode::Events
        } else {
            ObservationMode::Polling
        });
        if let Some(update) = tracker.reconcile() {
            self.sink.on_update(&update);
        }
        let state = Arc::new(Mutex::new(tracker.state().clone()));
        let snapshot = tracker.state().clone();
        let worker = Worker {
            tracker,
            shared: state.clone(),
            sink: self.sink.clone(),
            config: self.config,
            roots: std::iter::once(target.path.clone())
                .chain(target.path.canonicalize().ok())
                .collect(),
            guard,
        };
        let thread = thread::Builder::new()
            .name(format!("live-workspace-{primary_id}"))
            .spawn(move || worker.run(&inbox))
            .ok();
        sessions.insert(
            primary_id.to_owned(),
            Session {
                commands,
                state,
                thread,
            },
        );
        Ok(snapshot)
    }

    /// Tells the observer what the run is doing. Waiting, idle and cancelled phases also read the
    /// whole worktree: the agent stopped, so what is there is what it left.
    pub fn set_phase(&self, primary_id: &str, phase: LivePhase) {
        if let Some(session) = locked(&self.sessions).get(primary_id) {
            let _ = session.commands.send(Command::Phase(phase));
        }
    }

    /// The run is over. Reads the whole worktree one last time, records `phase`, and stops
    /// observing. The returned state is what the review starts from. Blocks until done.
    pub fn finish(&self, primary_id: &str, phase: LivePhase) -> Option<LiveWorkspaceState> {
        let session = locked(&self.sessions).remove(primary_id);
        let Some(mut session) = session else {
            // Never observed (or already over): one look is all there is.
            return self.snapshot(primary_id, phase);
        };
        let (done, wait) = mpsc::channel();
        if session.commands.send(Command::Finish(phase, done)).is_ok() {
            let _ = wait.recv_timeout(Duration::from_secs(60));
        }
        if let Some(thread) = session.thread.take() {
            let _ = thread.join();
        }
        let last = locked(&session.state).clone();
        locked(&self.finished).insert(primary_id.to_owned(), last.clone());
        Some(last)
    }

    /// Compares the whole worktree with the baseline now (the user asked for a refresh, or
    /// something looked wrong) and returns the state.
    ///
    /// # Errors
    ///
    /// If the worktree cannot be observed, or the observer does not answer.
    pub fn refresh(&self, primary_id: &str) -> Result<LiveWorkspaceState, LiveError> {
        {
            let sessions = locked(&self.sessions);
            if let Some(session) = sessions.get(primary_id).filter(|s| s.alive()) {
                let (done, wait) = mpsc::channel();
                session
                    .commands
                    .send(Command::Reconcile(done))
                    .map_err(|_| LiveError::Unresponsive)?;
                let state = session.state.clone();
                drop(sessions);
                wait.recv_timeout(Duration::from_secs(60))
                    .map_err(|_| LiveError::Unresponsive)?;
                return Ok(locked(&state).clone());
            }
        }
        self.snapshot(primary_id, LivePhase::Idle)
            .ok_or(LiveError::NotFound)
    }

    /// The state of a worktree: what the observer holds, or what it last held, or (for a run
    /// nobody is observing, as after a restart) one look at the worktree.
    pub fn state(&self, primary_id: &str) -> Option<LiveWorkspaceState> {
        if let Some(session) = locked(&self.sessions).get(primary_id) {
            return Some(locked(&session.state).clone());
        }
        let mut last = locked(&self.finished).get(primary_id)?.clone();
        // The run is over, but its folder may not be (applied and removed, discarded): say so.
        match self.worktrees.live_target(primary_id) {
            Ok(_) => {}
            Err(LiveTargetError::Invalid) => last.availability = WorktreeAvailability::Invalid,
            Err(_) => last.availability = WorktreeAvailability::Missing,
        }
        Some(last)
    }

    /// The state for a workflow run, by the run's id: the worktree is the one Atlas stored for it.
    /// `None` when the run has no code worktree.
    pub fn state_for_run(&self, run_id: &str) -> Option<LiveWorkspaceState> {
        let (primary, phase) = self.primary_of(run_id)?;
        self.state(&primary)
            .or_else(|| self.snapshot(&primary, phase))
            .or_else(|| self.remembered(run_id))
    }

    /// [`Self::refresh`], by the run's id.
    ///
    /// # Errors
    ///
    /// If the run has no worktree, or it cannot be observed.
    pub fn refresh_for_run(&self, run_id: &str) -> Result<LiveWorkspaceState, LiveError> {
        let (primary, _) = self.primary_of(run_id).ok_or(LiveError::NotFound)?;
        self.refresh(&primary)
    }

    /// One file of the run's worktree as it is now. The run is named and the folder is the one
    /// Atlas stored for it; the path must be a plain relative one.
    ///
    /// # Errors
    ///
    /// If the run has no worktree that can be read, or the path is not allowed.
    pub fn file_for_run(&self, run_id: &str, path: &str) -> Result<LiveFile, LiveError> {
        if !is_plain_relative_path(path) {
            return Err(LiveError::InvalidPath);
        }
        let target = self.target_of(run_id)?;
        Ok(self.worktrees.live_file(&target, path)?)
    }

    /// The diff of the run's worktree as it is now against the workflow's baseline.
    ///
    /// # Errors
    ///
    /// As [`Self::file_for_run`].
    pub fn diff_for_run(&self, run_id: &str, path: Option<&str>) -> Result<String, LiveError> {
        if path.is_some_and(|p| !is_plain_relative_path(p)) {
            return Err(LiveError::InvalidPath);
        }
        let target = self.target_of(run_id)?;
        Ok(self.worktrees.live_diff(&target, path)?)
    }

    fn target_of(
        &self,
        run_id: &str,
    ) -> Result<crate::application::worktree::LiveTarget, LiveError> {
        // By the worktree's own record: the run's id is all that is needed, and all the webview has.
        let primary = self
            .worktrees
            .primary_of_run(run_id)
            .ok_or(LiveError::NotFound)?;
        Ok(self.worktrees.live_target(&primary)?)
    }

    /// The worktree of a run, and the phase its record implies when nothing is observing it.
    fn primary_of(&self, run_id: &str) -> Option<(String, LivePhase)> {
        let run = self.workflows.execution(run_id)?;
        let primary = run.integration.worktree_execution_id.clone()?;
        let phase = if run.integration.status == IntegrationStatus::InProgress {
            LivePhase::Idle
        } else if run.status == WorkflowExecutionStatus::Completed {
            LivePhase::Ended
        } else {
            LivePhase::Stopped
        };
        Some((primary, phase))
    }

    /// One look at a worktree nobody observes. Not stored: observing it is [`Self::start`].
    fn snapshot(&self, primary_id: &str, phase: LivePhase) -> Option<LiveWorkspaceState> {
        let target = self.worktrees.live_target(primary_id).ok()?;
        let revision = locked(&self.finished)
            .get(primary_id)
            .map_or(0, |s| s.revision);
        let mut tracker = LiveTracker::new(self.worktrees.clone(), target, revision);
        tracker.reconcile();
        tracker.set_phase(phase);
        let state = tracker.state().clone();
        if phase.is_final() {
            locked(&self.finished).insert(primary_id.to_owned(), state.clone());
        }
        Some(state)
    }

    /// What is known of a run whose worktree cannot be read any more (applied and removed,
    /// discarded, gone): the record, saying so. The files are those of the run's change set.
    fn remembered(&self, run_id: &str) -> Option<LiveWorkspaceState> {
        let run = self.workflows.execution(run_id)?;
        let integration = &run.integration;
        let changes = run.changes.as_ref();
        let files = changes.map(|c| c.files.clone()).unwrap_or_default();
        Some(LiveWorkspaceState {
            run_id: run.id.clone(),
            worktree_execution_id: integration.worktree_execution_id.clone()?,
            branch: integration.branch.clone()?,
            baseline_revision: integration.base_revision.clone()?,
            current_revision: integration.current_revision.clone(),
            availability: WorktreeAvailability::Missing,
            phase: if run.status == WorkflowExecutionStatus::Completed {
                LivePhase::Ended
            } else {
                LivePhase::Stopped
            },
            observation: ObservationMode::Stopped,
            files_changed: changes.map_or(0, |c| c.files_changed),
            additions: changes.map_or(0, |c| c.additions),
            deletions: changes.map_or(0, |c| c.deletions),
            files,
            revision: 0,
            updated_at: integration.updated_at,
            last_reconciled_at: None,
        })
    }
}

/// The thread that watches one worktree.
struct Worker {
    tracker: LiveTracker,
    shared: Arc<Mutex<LiveWorkspaceState>>,
    sink: Arc<dyn LiveSink>,
    config: LiveConfig,
    roots: Vec<PathBuf>,
    /// Held so the watcher lives as long as the thread; dropped to stop it.
    guard: Option<Box<dyn Send>>,
}

/// Paths touched since the last look.
#[derive(Default)]
struct Batch {
    paths: BTreeSet<String>,
    rescan: bool,
    first: Option<Instant>,
    last: Option<Instant>,
}

impl Batch {
    fn is_empty(&self) -> bool {
        self.paths.is_empty() && !self.rescan
    }

    /// When the batch is due: once it has been quiet for `quiet`, or `max_wait` after it began.
    fn due(&self, config: &LiveConfig) -> Option<Instant> {
        let (first, last) = (self.first?, self.last?);
        Some((last + config.quiet).min(first + config.max_wait))
    }

    fn touch(&mut self, now: Instant) {
        self.first.get_or_insert(now);
        self.last = Some(now);
    }
}

impl Worker {
    fn run(mut self, inbox: &mpsc::Receiver<Command>) {
        let mut batch = Batch::default();
        let mut poll = self.config.poll_min;
        let mut next_check = Instant::now() + self.check_interval(poll);
        loop {
            let now = Instant::now();
            // A batch that keeps growing is still read once it has waited long enough.
            if batch.due(&self.config).is_some_and(|due| due <= now) {
                self.flush(&mut batch);
                if !self.tracker.is_available() {
                    return self.stop_observing();
                }
                continue;
            }
            let deadline = batch.due(&self.config).unwrap_or(next_check);
            match inbox.recv_timeout(deadline.saturating_duration_since(now)) {
                Ok(Command::Signal(signal)) => self.collect(&mut batch, signal),
                Ok(Command::Phase(phase)) => {
                    let update = self.tracker.set_phase(phase);
                    self.publish(update);
                    // The agent stopped: what is in the worktree is what it left.
                    if !matches!(phase, LivePhase::Running) {
                        self.flush_all(&mut batch);
                    }
                }
                Ok(Command::Reconcile(done)) => {
                    self.flush_all(&mut batch);
                    let _ = done.send(());
                }
                Ok(Command::Finish(phase, done)) => {
                    self.flush_all(&mut batch);
                    let update = self.tracker.set_phase(phase);
                    self.publish(update);
                    self.stop_observing();
                    let _ = done.send(());
                    return;
                }
                Err(RecvTimeoutError::Timeout) => {
                    if batch.is_empty() {
                        // Nothing was reported for a while: look anyway. With events working it
                        // is a safety net; without them it is the way things are noticed, and
                        // looks less often the longer nothing changes.
                        let changed = self.flush_all(&mut batch);
                        poll = if changed {
                            self.config.poll_min
                        } else {
                            (poll * 2).min(self.config.poll_max)
                        };
                        next_check = Instant::now() + self.check_interval(poll);
                    }
                }
                Err(RecvTimeoutError::Disconnected) => return self.stop_observing(),
            }
            if !self.tracker.is_available() {
                return self.stop_observing();
            }
        }
    }

    fn check_interval(&self, poll: Duration) -> Duration {
        let phase = self.tracker.state().phase;
        match self.tracker.state().observation {
            ObservationMode::Polling => {
                if phase == LivePhase::Running {
                    poll
                } else {
                    poll.max(self.config.poll_max)
                }
            }
            _ => {
                if phase == LivePhase::Running {
                    self.config.safety_running
                } else {
                    self.config.safety_idle
                }
            }
        }
    }

    fn collect(&self, batch: &mut Batch, signal: WatchSignal) {
        let now = Instant::now();
        match signal {
            WatchSignal::Rescan => {
                batch.rescan = true;
                batch.touch(now);
            }
            WatchSignal::Paths(paths) => {
                let mut any = false;
                for path in paths {
                    if let Some(relative) = relative_to(&self.roots, &path) {
                        batch.paths.insert(relative);
                        any = true;
                    }
                }
                if any {
                    batch.touch(now);
                }
            }
        }
    }

    /// Reads what the batch names (everything, if events may have been lost).
    fn flush(&mut self, batch: &mut Batch) -> bool {
        let taken = std::mem::take(batch);
        let update = if taken.rescan {
            self.tracker.reconcile()
        } else {
            let paths: Vec<String> = taken.paths.into_iter().collect();
            self.tracker.apply_paths(&paths)
        };
        self.publish(update)
    }

    /// Reads the whole worktree, whatever was pending.
    fn flush_all(&mut self, batch: &mut Batch) -> bool {
        *batch = Batch::default();
        let update = self.tracker.reconcile();
        self.publish(update)
    }

    fn publish(&self, update: Option<LiveWorkspaceUpdate>) -> bool {
        *locked(&self.shared) = self.tracker.state().clone();
        match update {
            Some(update) => {
                self.sink.on_update(&update);
                true
            }
            None => false,
        }
    }

    fn stop_observing(&mut self) {
        self.guard = None;
        let update = self.tracker.set_observation(ObservationMode::Stopped);
        self.publish(update);
    }
}

/// `path` relative to the worktree, unless it is outside it, is the folder itself, or is Git's
/// own (`.git` is a file in a worktree and Git's changes to it are not changes to the work).
/// `roots` are the folder as Atlas knows it and as the platform may report it (a real path
/// behind a link).
fn relative_to(roots: &[PathBuf], path: &Path) -> Option<String> {
    let relative = roots.iter().find_map(|root| path.strip_prefix(root).ok())?;
    let text = relative.to_str()?;
    if text.is_empty() || text == ".git" || text.starts_with(".git/") {
        return None;
    }
    Some(text.replace('\\', "/"))
}

#[cfg(test)]
pub mod tests;
