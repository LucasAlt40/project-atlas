//! The Live Workspace against real Git repositories and real worktrees. The filesystem watcher
//! is the platform's in the tests about real writes, and a scripted one where the point is the
//! order and timing of events (debounce, lost events, no watcher at all).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::*;
use crate::application::agents::AgentService;
use crate::application::personalities::PersonalityService;
use crate::application::projects::fake::FakeInspector;
use crate::application::runtimes::RuntimeRegistry;
use crate::application::workflow::service::WorkflowService;
use crate::application::workspace::WorkspaceService;
use crate::application::worktree::tests::{git, Env, WORKSPACE};
use crate::application::worktree::RunOutcome;
use crate::domain::security::Permission;
use crate::domain::worktree::FileChangeStatus;
use crate::infrastructure::NotifyWatcher;

fn fast() -> LiveConfig {
    LiveConfig {
        quiet: Duration::from_millis(60),
        max_wait: Duration::from_millis(400),
        safety_running: Duration::from_millis(500),
        safety_idle: Duration::from_millis(500),
        poll_min: Duration::from_millis(30),
        poll_max: Duration::from_millis(120),
    }
}

#[derive(Default)]
struct Recorder(Mutex<Vec<LiveWorkspaceUpdate>>);

impl Recorder {
    fn all(&self) -> Vec<LiveWorkspaceUpdate> {
        locked(&self.0).clone()
    }

    fn count(&self) -> usize {
        locked(&self.0).len()
    }
}

impl LiveSink for Recorder {
    fn on_update(&self, update: &LiveWorkspaceUpdate) {
        locked(&self.0).push(update.clone());
    }
}

/// A watcher the test drives: it keeps the callback and lets the test report events (or none).
type Callback = Box<dyn Fn(WatchSignal) + Send + Sync>;

#[derive(Default)]
struct ScriptedWatcher {
    callback: Mutex<Option<Arc<Callback>>>,
    /// `Some(reason)`: refuses to watch, as a platform out of watches would.
    refuse: Mutex<Option<String>>,
}

impl ScriptedWatcher {
    fn send(&self, signal: WatchSignal) {
        let callback = locked(&self.callback).clone();
        (callback.expect("a worktree is being watched"))(signal);
    }

    fn touch(&self, root: &Path, names: &[&str]) {
        self.send(WatchSignal::Paths(
            names.iter().map(|n| root.join(n)).collect(),
        ));
    }
}

impl WorktreeWatcher for Arc<ScriptedWatcher> {
    fn watch(
        &self,
        _root: &Path,
        on_signal: Box<dyn Fn(WatchSignal) + Send + Sync>,
    ) -> Result<Box<dyn Send>, String> {
        if let Some(reason) = locked(&self.refuse).clone() {
            return Err(reason);
        }
        *locked(&self.callback) = Some(Arc::new(on_signal));
        Ok(Box::new(()))
    }
}

struct Live {
    env: Env,
    service: Arc<LiveWorkspaceService>,
    sink: Arc<Recorder>,
    /// The run's worktree folder (where the agents write).
    dir: PathBuf,
    scripted: Arc<ScriptedWatcher>,
}

const PRIMARY: &str = "exec-1";

/// A service over `env`'s worktrees with the platform's real watcher, reporting to `sink`. Other
/// layers' tests use it to follow the real thing end to end.
pub fn service_over(env: &Env, sink: Arc<dyn LiveSink>) -> Arc<LiveWorkspaceService> {
    let (service, _) = build(env, Arc::new(NotifyWatcher), sink);
    service
}

fn build(
    env: &Env,
    watcher: Arc<dyn WorktreeWatcher>,
    sink: Arc<dyn LiveSink>,
) -> (Arc<LiveWorkspaceService>, Arc<WorkflowService>) {
    let personalities = Arc::new(PersonalityService::new(env.config.clone()));
    let agents = Arc::new(AgentService::new(
        env.config.clone(),
        personalities,
        Arc::new(RuntimeRegistry::new(vec![])),
    ));
    let workspaces = Arc::new(WorkspaceService::new(
        env.config.clone(),
        agents.clone(),
        Arc::new(FakeInspector::with(&[])),
    ));
    let workflows = Arc::new(WorkflowService::new(env.config.clone(), agents, workspaces));
    let service = Arc::new(
        LiveWorkspaceService::new(env.service.clone(), workflows.clone(), watcher, sink)
            .with_config(fast()),
    );
    (service, workflows)
}

fn live_over(env: Env, scripted: Option<Arc<ScriptedWatcher>>) -> Live {
    let sink = Arc::new(Recorder::default());
    let scripted_handle = scripted.clone().unwrap_or_default();
    let watcher: Arc<dyn WorktreeWatcher> = match scripted {
        Some(scripted) => Arc::new(scripted),
        None => Arc::new(NotifyWatcher),
    };
    let (service, _) = build(&env, watcher, sink.clone());
    let prepared = env.prepare(PRIMARY);
    env.service.mark_workflow(PRIMARY, "wfx-1").unwrap();
    Live {
        env,
        service,
        sink,
        dir: prepared.working_dir,
        scripted: scripted_handle,
    }
}

/// With the platform's real filesystem events.
fn live() -> Live {
    live_over(Env::new(Permission::Allowed), None)
}

/// With events the test sends itself.
fn scripted() -> Live {
    live_over(
        Env::new(Permission::Allowed),
        Some(Arc::new(ScriptedWatcher::default())),
    )
}

fn write(dir: &Path, name: &str, text: &str) {
    if let Some(parent) = Path::new(name).parent() {
        fs::create_dir_all(dir.join(parent)).unwrap();
    }
    fs::write(dir.join(name), text).unwrap();
}

fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

impl Live {
    fn state(&self) -> LiveWorkspaceState {
        self.service.state(PRIMARY).expect("observed")
    }

    fn file(&self, path: &str) -> Option<FileChange> {
        self.state().files.into_iter().find(|f| f.path == path)
    }

    fn status_of(&self, path: &str) -> Option<FileChangeStatus> {
        self.file(path).map(|f| f.status)
    }

    fn wait_for_file(&self, path: &str, status: FileChangeStatus) {
        wait_until(&format!("{path} to be {status:?}"), || {
            self.status_of(path) == Some(status)
        });
    }

    fn paths(&self) -> Vec<String> {
        self.state().files.into_iter().map(|f| f.path).collect()
    }
}

use crate::domain::worktree::FileChange;

// ---- what is seen ---------------------------------------------------------------------------

#[test]
fn a_created_file_shows_up_as_added_with_its_lines_and_is_announced() {
    let l = live();
    l.service.start(PRIMARY).unwrap();

    write(
        &l.dir,
        "src/foo.ts",
        "export const x = 1;\nexport const y = 2;\n",
    );

    l.wait_for_file("src/foo.ts", FileChangeStatus::Added);
    let file = l.file("src/foo.ts").unwrap();
    assert_eq!((file.additions, file.deletions), (Some(2), Some(0)));
    let update = l
        .sink
        .all()
        .into_iter()
        .find(|u| u.changed.iter().any(|c| c.path == "src/foo.ts"))
        .expect("an update named the file");
    assert!(!update.full);
    assert_eq!(update.worktree_execution_id, PRIMARY);
    // Nothing in the project's own checkout.
    assert!(!l.env.project.path().join("src/foo.ts").exists());
}

#[test]
fn a_modified_file_shows_up_as_modified_against_the_baseline() {
    let l = live();
    l.service.start(PRIMARY).unwrap();

    write(&l.dir, "README.md", "# Project\nmore\n");

    l.wait_for_file("README.md", FileChangeStatus::Modified);
    let file = l.file("README.md").unwrap();
    assert_eq!((file.additions, file.deletions), (Some(1), Some(0)));
}

#[test]
fn a_deleted_file_shows_up_as_deleted() {
    let l = live();
    l.service.start(PRIMARY).unwrap();

    fs::remove_file(l.dir.join("README.md")).unwrap();

    l.wait_for_file("README.md", FileChangeStatus::Deleted);
}

#[test]
fn a_file_changed_back_to_the_baseline_leaves_the_changes() {
    let l = live();
    l.service.start(PRIMARY).unwrap();
    write(&l.dir, "README.md", "# changed\n");
    l.wait_for_file("README.md", FileChangeStatus::Modified);

    write(&l.dir, "README.md", "# Project\n");

    wait_until("the file to stop being a change", || {
        l.file("README.md").is_none()
    });
    let last = l.sink.all().into_iter().last().unwrap();
    assert!(last.removed.contains(&"README.md".to_owned()));
}

#[test]
fn a_file_saved_many_times_in_a_row_ends_as_the_last_version() {
    let l = live();
    l.service.start(PRIMARY).unwrap();

    for n in 1..=12 {
        write(&l.dir, "src/a.ts", &"line\n".repeat(n));
    }

    wait_until("the last version", || {
        l.file("src/a.ts").is_some_and(|f| f.additions == Some(12))
    });
}

#[test]
fn several_files_changed_at_once_are_all_seen_with_their_own_status() {
    let l = live();
    l.service.start(PRIMARY).unwrap();

    write(&l.dir, "a/new.ts", "n\n");
    write(&l.dir, "b/deep/er/new.ts", "n\n");
    write(&l.dir, "shared.txt", "line 1\nCHANGED\nline 3\n");
    fs::remove_file(l.dir.join("README.md")).unwrap();

    wait_until("all four", || l.paths().len() == 4);
    assert_eq!(l.status_of("a/new.ts"), Some(FileChangeStatus::Added));
    assert_eq!(
        l.status_of("b/deep/er/new.ts"),
        Some(FileChangeStatus::Added)
    );
    assert_eq!(l.status_of("shared.txt"), Some(FileChangeStatus::Modified));
    assert_eq!(l.status_of("README.md"), Some(FileChangeStatus::Deleted));
}

#[test]
fn a_burst_of_files_is_read_in_a_few_batches_not_one_per_event() {
    let l = live();
    l.service.start(PRIMARY).unwrap();
    let before = l.sink.count();

    for n in 0..80 {
        write(&l.dir, &format!("gen/file-{n:02}.txt"), "x\n");
    }

    wait_until("all 80", || l.state().files_changed == 80);
    let updates = l.sink.count() - before;
    assert!(
        updates < 40,
        "80 writes made {updates} updates: events were not coalesced"
    );
}

#[test]
fn a_rename_is_one_rename_once_the_whole_worktree_is_read() {
    let l = live();
    l.service.start(PRIMARY).unwrap();

    fs::create_dir_all(l.dir.join("docs")).unwrap();
    fs::rename(l.dir.join("README.md"), l.dir.join("docs/README.md")).unwrap();
    l.service.refresh(PRIMARY).unwrap();

    let moved = l.file("docs/README.md").expect("the new name");
    assert_eq!(moved.status, FileChangeStatus::Renamed);
    assert_eq!(moved.old_path.as_deref(), Some("README.md"));
    assert!(l.file("README.md").is_none(), "not also a deletion");
}

#[test]
fn the_agents_own_commits_move_the_revision_but_not_the_changes() {
    let l = live();
    let started = l.service.start(PRIMARY).unwrap();
    write(&l.dir, "src/foo.ts", "x\n");
    l.wait_for_file("src/foo.ts", FileChangeStatus::Added);

    // A step ends: its work is saved on the run's branch.
    l.env
        .service
        .attach(WORKSPACE, "agent-2", "exec-2", PRIMARY)
        .unwrap();
    l.env
        .service
        .finish_step("exec-2", RunOutcome::Completed)
        .unwrap();
    l.service.refresh(PRIMARY).unwrap();

    let state = l.state();
    assert_eq!(state.baseline_revision, started.baseline_revision);
    assert_eq!(state.baseline_revision, l.env.main_branch_head());
    assert_ne!(
        state.current_revision.as_deref(),
        Some(state.baseline_revision.as_str())
    );
    // Still compared with the baseline, not with the new commit.
    assert_eq!(l.status_of("src/foo.ts"), Some(FileChangeStatus::Added));
    // And the project never moved.
    assert_eq!(l.env.main_branch_head(), state.baseline_revision);
}

// ---- starting -------------------------------------------------------------------------------

#[test]
fn what_the_worktree_already_holds_is_read_when_observation_starts() {
    let l = live();
    write(&l.dir, "src/early.ts", "already here\n");
    fs::remove_file(l.dir.join("README.md")).unwrap();

    let state = l.service.start(PRIMARY).unwrap();

    assert_eq!(state.observation, ObservationMode::Events);
    assert_eq!(state.availability, WorktreeAvailability::Available);
    assert_eq!(state.phase, LivePhase::Idle);
    assert_eq!(state.files_changed, 2);
    assert!(state.revision >= 1);
    let first = &l.sink.all()[0];
    assert!(first.full, "the first look is of everything");
    assert_eq!(first.changed.len(), 2);
}

#[test]
fn starting_twice_observes_once() {
    let l = live();
    l.service.start(PRIMARY).unwrap();
    l.service.start(PRIMARY).unwrap();

    write(&l.dir, "src/foo.ts", "x\n");

    l.wait_for_file("src/foo.ts", FileChangeStatus::Added);
    let announced = l
        .sink
        .all()
        .iter()
        .filter(|u| u.changed.iter().any(|c| c.path == "src/foo.ts"))
        .count();
    assert_eq!(announced, 1);
}

#[test]
fn only_a_workflows_primary_worktree_can_be_observed() {
    let l = live();
    // An ordinary execution's worktree, a lease, and an id that does not exist.
    l.env.prepare("exec-9");
    l.env
        .service
        .attach(WORKSPACE, "agent-2", "exec-2", PRIMARY)
        .unwrap();

    assert_eq!(l.service.start("exec-9").unwrap_err(), LiveError::NotFound);
    assert_eq!(l.service.start("exec-2").unwrap_err(), LiveError::NotFound);
    assert_eq!(l.service.start("nope").unwrap_err(), LiveError::NotFound);
    assert!(l.service.state("exec-9").is_none());
}

#[test]
fn a_worktree_that_is_not_what_atlas_made_is_not_read() {
    let l = live();
    git(&l.dir, &["checkout", "--quiet", "-b", "somewhere-else"]);

    assert_eq!(l.service.start(PRIMARY).unwrap_err(), LiveError::Invalid);
}

#[test]
fn the_projects_own_checkout_is_never_what_is_observed() {
    let l = live();
    l.service.start(PRIMARY).unwrap();

    write(l.env.project.path(), "in-the-project.txt", "mine\n");
    write(
        l.env.project.path(),
        "README.md",
        "# edited in the project\n",
    );
    std::thread::sleep(Duration::from_millis(400));
    l.service.refresh(PRIMARY).unwrap();

    assert_eq!(l.state().files, Vec::<FileChange>::new());
}

// ---- debounce, lost events, no events -------------------------------------------------------

#[test]
fn events_are_read_only_after_the_files_have_been_quiet() {
    let l = scripted();
    let mut config = fast();
    config.quiet = Duration::from_millis(300);
    config.max_wait = Duration::from_secs(5);
    let service = Arc::new(
        LiveWorkspaceService::new(
            l.env.service.clone(),
            l.service.workflows.clone(),
            Arc::new(l.scripted.clone()),
            l.sink.clone(),
        )
        .with_config(config),
    );
    service.start(PRIMARY).unwrap();
    let before = l.sink.count();
    write(&l.dir, "a.txt", "1\n");

    // Events keep coming for a while: nothing is read yet.
    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(250) {
        l.scripted.touch(&l.dir, &["a.txt"]);
        std::thread::sleep(Duration::from_millis(25));
    }
    assert_eq!(l.sink.count(), before, "read before the files were quiet");

    // Once they are, it is read once, however many events there were.
    wait_until("the batch", || l.sink.count() > before);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(
        l.sink.count(),
        before + 1,
        "{:#?}\n{}\n{}",
        l.sink.all()[before..]
            .iter()
            .map(|u| (u.revision, u.full, u.current_revision.clone()))
            .collect::<Vec<_>>(),
        git(&l.dir, &["log", "--oneline", "--all"]),
        l.env.main_branch_head()
    );
    assert_eq!(service.state(PRIMARY).unwrap().files[0].path, "a.txt");
}

#[test]
fn a_worktree_written_without_a_pause_is_still_read_after_the_longest_wait() {
    let l = scripted();
    l.service.start(PRIMARY).unwrap();
    let before = l.sink.count();

    // Writes and events with no pause longer than `quiet`, for longer than `max_wait`.
    let started = Instant::now();
    let mut n = 0;
    while started.elapsed() < Duration::from_millis(900) && l.sink.count() == before {
        write(&l.dir, &format!("f{n}.txt"), "x\n");
        l.scripted.touch(&l.dir, &[&format!("f{n}.txt")]);
        n += 1;
        std::thread::sleep(Duration::from_millis(20));
    }

    assert!(
        l.sink.count() > before,
        "progress was shown while writing went on"
    );
    assert!(started.elapsed() < Duration::from_millis(900));
}

#[test]
fn an_event_for_a_file_that_did_not_really_change_changes_nothing() {
    let l = scripted();
    l.service.start(PRIMARY).unwrap();
    let before = l.state();

    // The file exists and is as in the baseline; another path does not exist at all.
    l.scripted
        .touch(&l.dir, &["README.md", "never-existed.txt", ".git/index"]);
    std::thread::sleep(Duration::from_millis(300));

    assert_eq!(l.state().revision, before.revision);
    assert_eq!(l.sink.count(), 1, "only the first look was announced");
}

#[test]
fn a_change_no_event_reported_is_found_by_the_safety_look() {
    let l = scripted();
    l.service.start(PRIMARY).unwrap();

    write(&l.dir, "silent.txt", "no event for this\n");

    l.wait_for_file("silent.txt", FileChangeStatus::Added);
}

#[test]
fn a_lost_event_queue_makes_the_whole_worktree_be_read() {
    let l = scripted();
    l.service.start(PRIMARY).unwrap();
    write(&l.dir, "one.txt", "1\n");
    write(&l.dir, "two.txt", "2\n");

    l.scripted.send(WatchSignal::Rescan);

    wait_until("both files", || l.paths() == ["one.txt", "two.txt"]);
    assert!(l.sink.all().last().unwrap().full);
}

#[test]
fn more_paths_than_a_few_files_is_read_as_the_whole_worktree() {
    let l = scripted();
    l.service.start(PRIMARY).unwrap();
    let names: Vec<String> = (0..260).map(|n| format!("m/{n}.txt")).collect();
    for name in &names {
        write(&l.dir, name, "x\n");
    }

    l.scripted.send(WatchSignal::Paths(
        names.iter().map(|n| l.dir.join(n)).collect(),
    ));

    wait_until("all of them", || l.state().files_changed == 260);
    assert!(l.sink.all().last().unwrap().full);
}

#[test]
fn without_filesystem_events_the_worktree_is_looked_at_on_a_schedule_that_slows_down() {
    let l = scripted();
    *locked(&l.scripted.refuse) = Some("out of watches".to_owned());

    let state = l.service.start(PRIMARY).unwrap();
    assert_eq!(state.observation, ObservationMode::Polling);
    write(&l.dir, "polled.txt", "found by looking\n");
    l.wait_for_file("polled.txt", FileChangeStatus::Added);

    // Quiet for a while: it is still being looked at, and nothing is announced for nothing.
    let announced = l.sink.count();
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(l.sink.count(), announced);
    write(&l.dir, "later.txt", "x\n");
    l.wait_for_file("later.txt", FileChangeStatus::Added);
}

// ---- the run's phases -----------------------------------------------------------------------

#[test]
fn updates_are_numbered_one_after_another() {
    let l = live();
    l.service.start(PRIMARY).unwrap();
    for n in 0..4 {
        write(&l.dir, &format!("n{n}.txt"), "x\n");
        l.wait_for_file(&format!("n{n}.txt"), FileChangeStatus::Added);
    }
    l.service.set_phase(PRIMARY, LivePhase::Running);
    wait_until("the phase", || l.state().phase == LivePhase::Running);

    let revisions: Vec<u64> = l.sink.all().iter().map(|u| u.revision).collect();
    assert!(
        revisions.windows(2).all(|w| w[1] == w[0] + 1),
        "{revisions:?}"
    );
    assert_eq!(revisions.last(), Some(&l.state().revision));
}

#[test]
fn waiting_for_a_person_keeps_the_worktree_visible_and_reads_what_the_agent_left() {
    let l = scripted();
    l.service.start(PRIMARY).unwrap();
    l.service.set_phase(PRIMARY, LivePhase::Running);
    // The agent writes, then asks a question; no event was seen for the write.
    write(&l.dir, "half.ts", "// started\n");

    l.service.set_phase(PRIMARY, LivePhase::WaitingForInput);

    wait_until("the file", || l.status_of("half.ts").is_some());
    assert_eq!(l.state().phase, LivePhase::WaitingForInput);
    assert_eq!(l.state().observation, ObservationMode::Events);
    // Still watched while it waits: an edit during the wait (the user's IDE, say) is seen.
    l.scripted.touch(&l.dir, &["during-wait.ts"]);
    write(&l.dir, "during-wait.ts", "typed by someone\n");
    l.scripted.touch(&l.dir, &["during-wait.ts"]);
    l.wait_for_file("during-wait.ts", FileChangeStatus::Added);
}

#[test]
fn resuming_after_a_wait_goes_on_in_the_same_worktree_with_the_same_baseline() {
    let l = live();
    let started = l.service.start(PRIMARY).unwrap();
    l.service.set_phase(PRIMARY, LivePhase::WaitingForInput);
    write(&l.dir, "before.ts", "1\n");
    l.wait_for_file("before.ts", FileChangeStatus::Added);

    l.service.set_phase(PRIMARY, LivePhase::Running);
    write(&l.dir, "after.ts", "2\n");

    l.wait_for_file("after.ts", FileChangeStatus::Added);
    let state = l.state();
    assert_eq!(state.phase, LivePhase::Running);
    assert_eq!(state.baseline_revision, started.baseline_revision);
    assert!(
        l.file("before.ts").is_some(),
        "what came before is still there"
    );
}

#[test]
fn observation_picked_up_again_after_a_restart_keeps_the_numbers_going() {
    let l = live();
    l.service.start(PRIMARY).unwrap();
    write(&l.dir, "x.ts", "1\n");
    l.wait_for_file("x.ts", FileChangeStatus::Added);
    let last = l.service.finish(PRIMARY, LivePhase::Stopped).unwrap();

    // The run is resumed: the same worktree is reopened and observed again.
    let again = l.service.start(PRIMARY).unwrap();

    assert!(again.revision >= last.revision);
    assert_eq!(again.baseline_revision, last.baseline_revision);
    assert!(again.files.iter().any(|f| f.path == "x.ts"));
    assert_eq!(again.observation, ObservationMode::Events);
}

#[test]
fn a_cancelled_run_keeps_what_the_worktree_holds_and_stops_being_watched() {
    let l = live();
    l.service.start(PRIMARY).unwrap();
    l.service.set_phase(PRIMARY, LivePhase::Running);
    write(&l.dir, "partial.ts", "half a change\n");
    l.service.set_phase(PRIMARY, LivePhase::Cancelled);
    wait_until("the file", || l.status_of("partial.ts").is_some());

    let last = l.service.finish(PRIMARY, LivePhase::Stopped).unwrap();

    assert_eq!(last.phase, LivePhase::Stopped);
    assert_eq!(last.observation, ObservationMode::Stopped);
    assert_eq!(last.availability, WorktreeAvailability::Available);
    assert!(last.files.iter().any(|f| f.path == "partial.ts"));
}

// ---- the end --------------------------------------------------------------------------------

#[test]
fn the_final_state_is_what_the_worktree_holds_whatever_events_were_seen_and_equals_the_change_set()
{
    let l = scripted();
    l.service.start(PRIMARY).unwrap();
    // The agent worked and no event reached the observer at all.
    write(&l.dir, "src/foo.ts", "export const x = 1;\n");
    write(&l.dir, "shared.txt", "line 1\nline 2 changed\nline 3\n");
    fs::remove_file(l.dir.join("README.md")).unwrap();
    // One step ends (its work is committed on the run's branch), then another leaves more.
    l.env
        .service
        .attach(WORKSPACE, "agent-2", "exec-2", PRIMARY)
        .unwrap();
    write(&l.dir, "src/bar.ts", "export const y = 2;\n");
    l.env
        .service
        .finish_step("exec-2", RunOutcome::Completed)
        .unwrap();
    write(&l.dir, "src/uncommitted.ts", "not in a commit yet\n");

    let last = l.service.finish(PRIMARY, LivePhase::Ended).unwrap();

    assert_eq!(last.phase, LivePhase::Ended);
    assert_eq!(last.observation, ObservationMode::Stopped);
    // The run is closed as the real flow does, and the review's change set is read.
    let closed = l
        .env
        .service
        .close_shared(
            PRIMARY,
            RunOutcome::Completed,
            crate::domain::worktree::Validation::NotRun,
        )
        .unwrap();
    assert_eq!(
        closed.status,
        crate::domain::worktree::WorktreeStatus::Completed
    );
    let change_set = l.env.service.change_set(PRIMARY).unwrap();
    let mut from_set: Vec<(String, FileChangeStatus)> = change_set
        .files
        .iter()
        .map(|f| (f.path.clone(), f.status))
        .collect();
    from_set.sort_by(|a, b| a.0.cmp(&b.0));
    let live_files: Vec<(String, FileChangeStatus)> = last
        .files
        .iter()
        .map(|f| (f.path.clone(), f.status))
        .collect();
    assert_eq!(live_files, from_set, "the live state is the change set");
    assert_eq!(last.additions, change_set.additions);
    assert_eq!(last.deletions, change_set.deletions);
    assert_eq!(last.baseline_revision, change_set.base_revision);
}

#[test]
fn after_the_end_nothing_more_is_observed_and_the_final_state_can_still_be_read() {
    let l = live();
    l.service.start(PRIMARY).unwrap();
    write(&l.dir, "a.ts", "1\n");
    l.wait_for_file("a.ts", FileChangeStatus::Added);
    let last = l.service.finish(PRIMARY, LivePhase::Ended).unwrap();
    let announced = l.sink.count();

    write(&l.dir, "b.ts", "2\n");
    std::thread::sleep(Duration::from_millis(400));

    assert_eq!(l.sink.count(), announced);
    assert_eq!(l.state().files, last.files);
    assert_eq!(l.state().phase, LivePhase::Ended);
}

#[test]
fn a_worktree_that_is_removed_is_reported_missing_and_its_last_files_are_kept() {
    let l = live();
    l.service.start(PRIMARY).unwrap();
    write(&l.dir, "kept.ts", "1\n");
    l.wait_for_file("kept.ts", FileChangeStatus::Added);

    fs::remove_dir_all(&l.dir).unwrap();
    let state = l.service.refresh(PRIMARY).unwrap();

    assert_eq!(state.availability, WorktreeAvailability::Missing);
    assert_eq!(state.observation, ObservationMode::Stopped);
    assert!(state.files.iter().any(|f| f.path == "kept.ts"));
    // It cannot be started again, and says why.
    wait_until("the observer to end", || {
        locked(&l.service.sessions)
            .get(PRIMARY)
            .is_none_or(|s| !s.alive())
    });
    assert_eq!(l.service.start(PRIMARY).unwrap_err(), LiveError::Missing);
    assert_eq!(l.state().availability, WorktreeAvailability::Missing);
}

#[test]
fn a_worktree_removed_after_the_end_is_reported_missing_when_read_later() {
    let l = live();
    l.service.start(PRIMARY).unwrap();
    write(&l.dir, "a.ts", "1\n");
    l.wait_for_file("a.ts", FileChangeStatus::Added);
    l.service.finish(PRIMARY, LivePhase::Ended).unwrap();

    // The changes were applied and the worktree folder removed.
    l.env.service.close_shared(
        PRIMARY,
        RunOutcome::Completed,
        crate::domain::worktree::Validation::NotRun,
    );
    l.env.service.merge(PRIMARY).unwrap();

    let state = l.service.state(PRIMARY).unwrap();
    assert_eq!(state.availability, WorktreeAvailability::Missing);
    assert!(
        state.files.iter().any(|f| f.path == "a.ts"),
        "the last files are kept"
    );
}

#[test]
fn the_state_exists_without_anyone_listening() {
    // No sink that does anything, no UI: the state is read on demand all the same.
    let env = Env::new(Permission::Allowed);
    let l = live_over(env, None);
    write(&l.dir, "x.ts", "1\n");

    let state = l.service.refresh(PRIMARY).unwrap();

    assert_eq!(state.files_changed, 1);
    assert_eq!(state.phase, LivePhase::Idle);
    assert_eq!(
        state.observation,
        ObservationMode::Stopped,
        "looked at, not watched"
    );
}

#[test]
fn observing_never_takes_a_lock_an_agents_git_would_wait_for() {
    let l = live();
    l.service.start(PRIMARY).unwrap();
    let writing = {
        let dir = l.dir.clone();
        std::thread::spawn(move || {
            for n in 0..40 {
                write(&dir, &format!("w/{n}.txt"), "x\n");
                git(&dir, &["add", "--all"]);
                if n % 5 == 0 {
                    git(&dir, &["commit", "--quiet", "-m", &format!("step {n}")]);
                }
            }
        })
    };

    // The agent's own Git never fails because the observer is reading at the same time.
    writing
        .join()
        .expect("the agent's git commands all succeeded");

    wait_until("all of it", || l.state().files_changed == 40);
    let commits: u32 = git(&l.dir, &["rev-list", "--count", "HEAD"])
        .parse()
        .unwrap();
    assert!(commits >= 8, "the agent's commits all happened: {commits}");
}

// ---- the viewer: one file, one diff ---------------------------------------------------------

use crate::domain::live_workspace::LiveFileKind;

const RUN: &str = "wfx-1";

#[test]
fn a_file_is_read_from_the_worktree_as_it_is_now() {
    let l = live();
    write(&l.dir, "src/foo.ts", "export const x = 1;\n");

    let file = l.service.file_for_run(RUN, "src/foo.ts").unwrap();

    assert_eq!(file.kind, LiveFileKind::Text);
    assert_eq!(file.content.as_deref(), Some("export const x = 1;\n"));
    assert_eq!(file.size, Some(20));
    assert!(!file.truncated);
    // The project's own copy (if any) is not what is read.
    write(l.env.project.path(), "src/foo.ts", "THE PROJECT'S\n");
    write(&l.dir, "src/foo.ts", "changed again\n");
    assert_eq!(
        l.service
            .file_for_run(RUN, "src/foo.ts")
            .unwrap()
            .content
            .as_deref(),
        Some("changed again\n")
    );
}

#[test]
fn a_deleted_file_is_reported_deleted_not_as_an_error() {
    let l = live();
    fs::remove_file(l.dir.join("README.md")).unwrap();

    let file = l.service.file_for_run(RUN, "README.md").unwrap();
    let missing = l.service.file_for_run(RUN, "never/was/here.ts").unwrap();

    assert_eq!(file.kind, LiveFileKind::Deleted);
    assert_eq!(file.content, None);
    assert_eq!(missing.kind, LiveFileKind::Deleted);
}

#[test]
fn binary_files_big_files_folders_and_links_are_not_sent_as_text() {
    let l = live();
    fs::write(l.dir.join("image.bin"), [0u8, 159, 146, 150, 0]).unwrap();
    write(&l.dir, "big.txt", &"0123456789\n".repeat(200_000));
    fs::create_dir_all(l.dir.join("folder")).unwrap();
    let outside = l.env.data.path().join("secret.txt");
    fs::write(&outside, "do not read me\n").unwrap();
    std::os::unix::fs::symlink(&outside, l.dir.join("link.txt")).unwrap();

    let binary = l.service.file_for_run(RUN, "image.bin").unwrap();
    let big = l.service.file_for_run(RUN, "big.txt").unwrap();
    let folder = l.service.file_for_run(RUN, "folder").unwrap();
    let link = l.service.file_for_run(RUN, "link.txt").unwrap();

    assert_eq!((binary.kind, binary.content), (LiveFileKind::Binary, None));
    assert_eq!(big.kind, LiveFileKind::Text);
    assert!(big.truncated);
    assert_eq!(big.content.unwrap().len(), 1024 * 1024);
    assert_eq!(folder.kind, LiveFileKind::NotFile);
    assert_eq!((link.kind, link.content), (LiveFileKind::Symlink, None));
}

#[test]
fn paths_that_leave_the_worktree_or_are_not_plain_are_refused() {
    let l = live();
    let outside = l.env.data.path().join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.txt"), "no\n").unwrap();
    std::os::unix::fs::symlink(&outside, l.dir.join("linked-dir")).unwrap();

    for bad in [
        "../secret.txt",
        "a/../../secret.txt",
        "/etc/passwd",
        "C:\\Windows\\win.ini",
        "\\\\server\\share",
        ".git/config",
        ".git",
        "",
        "a//b",
        "./README.md",
        "bad\0name",
        "bad\nname",
    ] {
        assert_eq!(
            l.service.file_for_run(RUN, bad).unwrap_err(),
            LiveError::InvalidPath,
            "{bad:?}"
        );
        assert_eq!(
            l.service.diff_for_run(RUN, Some(bad)).unwrap_err(),
            LiveError::InvalidPath,
            "{bad:?}"
        );
    }
}

#[test]
fn a_file_behind_a_link_to_another_folder_is_never_read() {
    let l = live();
    let outside = l.env.data.path().join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.txt"), "no\n").unwrap();
    std::os::unix::fs::symlink(&outside, l.dir.join("linked-dir")).unwrap();

    assert_eq!(
        l.service
            .file_for_run(RUN, "linked-dir/secret.txt")
            .unwrap_err(),
        LiveError::InvalidPath
    );
}

#[test]
fn a_run_with_no_worktree_or_a_gone_one_has_no_file_to_read() {
    let l = live();

    assert_eq!(
        l.service.file_for_run("wfx-nope", "README.md").unwrap_err(),
        LiveError::NotFound
    );
    assert_eq!(
        l.service.diff_for_run("wfx-nope", None).unwrap_err(),
        LiveError::NotFound
    );
    fs::remove_dir_all(&l.dir).unwrap();
    assert_eq!(
        l.service.file_for_run(RUN, "README.md").unwrap_err(),
        LiveError::Missing
    );
}

#[test]
fn the_diff_is_the_worktree_as_it_is_now_against_the_baseline() {
    let l = live();
    write(&l.dir, "README.md", "# Project\nmore\n");
    write(&l.dir, "src/new.ts", "export {};\n");
    fs::remove_file(l.dir.join("shared.txt")).unwrap();

    let all = l.service.diff_for_run(RUN, None).unwrap();
    let one = l.service.diff_for_run(RUN, Some("README.md")).unwrap();
    let added = l.service.diff_for_run(RUN, Some("src/new.ts")).unwrap();
    let removed = l.service.diff_for_run(RUN, Some("shared.txt")).unwrap();

    assert!(all.contains("+more") && all.contains("+export {};") && all.contains("-line 2"));
    assert!(one.contains("+more") && !one.contains("export {}"));
    assert!(added.contains("new file mode") && added.contains("+export {};"));
    assert!(removed.contains("deleted file mode") && removed.contains("-line 1"));
    // Nothing was committed, staged or written to get it.
    assert_eq!(git(&l.dir, &["diff", "--cached", "--name-only"]), "");
}

#[test]
fn a_worktree_with_no_changes_has_an_empty_diff() {
    let l = live();

    assert_eq!(l.service.diff_for_run(RUN, None).unwrap(), "");
    assert_eq!(l.service.diff_for_run(RUN, Some("README.md")).unwrap(), "");
}

#[test]
fn the_diff_stays_against_the_baseline_after_the_agents_commits() {
    let l = live();
    write(&l.dir, "src/foo.ts", "committed work\n");
    l.env
        .service
        .attach(WORKSPACE, "agent-2", "exec-2", PRIMARY)
        .unwrap();
    l.env
        .service
        .finish_step("exec-2", RunOutcome::Completed)
        .unwrap();
    write(&l.dir, "src/foo.ts", "committed work\nand more\n");

    let diff = l.service.diff_for_run(RUN, Some("src/foo.ts")).unwrap();

    assert!(
        diff.contains("+committed work") && diff.contains("+and more"),
        "{diff}"
    );
}

#[test]
fn an_edit_that_keeps_the_same_counts_is_still_announced() {
    let l = live();
    // The safety look is far away: it is the event that must carry this.
    let slow = LiveConfig {
        safety_running: Duration::from_secs(60),
        safety_idle: Duration::from_secs(60),
        ..fast()
    };
    let service = Arc::new(
        LiveWorkspaceService::new(
            l.env.service.clone(),
            l.service.workflows.clone(),
            Arc::new(NotifyWatcher),
            l.sink.clone(),
        )
        .with_config(slow),
    );
    service.start(PRIMARY).unwrap();
    write(&l.dir, "src/a.ts", "const a = 1;\n");
    wait_until("the file", || {
        service
            .state(PRIMARY)
            .is_some_and(|s| s.files.iter().any(|f| f.path == "src/a.ts"))
    });
    let before = l.sink.count();
    std::thread::sleep(Duration::from_millis(30));

    // Same size, same number of lines, different text.
    write(&l.dir, "src/a.ts", "const a = 2;\n");

    wait_until("the second version to be announced", || {
        l.sink.count() > before
            && l.sink.all()[before..]
                .iter()
                .any(|u| !u.full && u.changed.iter().any(|c| c.path == "src/a.ts"))
    });
}
