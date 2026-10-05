//! The state of one worktree's live changes, and the only code that changes it.
//!
//! Two ways to learn what the worktree holds, both read from Git (never from an agent):
//!
//! - **incremental**: for the paths an event named, compare just those with the baseline and
//!   replace what the state had for them;
//! - **full**: compare the whole worktree with the baseline and replace the state.
//!
//! The tracker has no threads, timers or events of its own: it is told what to look at, so every
//! rule here is testable with a real repository and no clock.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::application::support::now_ms;
use crate::application::worktree::{LiveTarget, LiveTargetError, WorktreeService};
use crate::domain::live_workspace::{
    LivePhase, LiveWorkspaceState, LiveWorkspaceUpdate, ObservationMode, WorktreeAvailability,
};
use crate::domain::worktree::{FileChange, FileChangeStatus, MAX_CHANGESET_FILES};

/// More paths than this in one batch is no longer "a few files changed": the whole worktree is
/// read instead (also what a lost or overflowed event queue looks like).
pub const MAX_INCREMENTAL_PATHS: usize = 200;

/// Whether the worktree may be read.
enum Gate {
    Open,
    Closed(Option<LiveWorkspaceUpdate>),
}

pub struct LiveTracker {
    worktrees: Arc<WorktreeService>,
    target: LiveTarget,
    files: BTreeMap<String, FileChange>,
    /// What each file looked like on disk when it was read (size, modification time). Two reads
    /// with the same counts of lines can still be different text: this is what tells them apart.
    stamps: BTreeMap<String, Stamp>,
    state: LiveWorkspaceState,
}

type Stamp = Option<(u64, u128)>;

impl LiveTracker {
    /// A tracker with nothing read yet. `revision` continues a previous one, so the numbers a UI
    /// holds stay meaningful when the run's worktree is picked up again.
    pub fn new(worktrees: Arc<WorktreeService>, target: LiveTarget, revision: u64) -> Self {
        let state = LiveWorkspaceState {
            run_id: target.run_id.clone(),
            worktree_execution_id: target.execution_id.clone(),
            branch: target.branch.clone(),
            baseline_revision: target.baseline.clone(),
            current_revision: None,
            availability: WorktreeAvailability::Available,
            phase: LivePhase::Idle,
            observation: ObservationMode::Stopped,
            files: Vec::new(),
            files_changed: 0,
            additions: 0,
            deletions: 0,
            revision,
            updated_at: now_ms(),
            last_reconciled_at: None,
        };
        Self {
            worktrees,
            target,
            files: BTreeMap::new(),
            stamps: BTreeMap::new(),
            state,
        }
    }

    pub fn state(&self) -> &LiveWorkspaceState {
        &self.state
    }

    /// Whether the worktree can still be looked at.
    pub fn is_available(&self) -> bool {
        self.state.availability == WorktreeAvailability::Available
    }

    /// Looks at the whole worktree again. `None` when nothing the UI shows changed.
    pub fn reconcile(&mut self) -> Option<LiveWorkspaceUpdate> {
        if let Gate::Closed(update) = self.revalidate() {
            return update;
        }
        let Ok(found) = self.worktrees.live_changes(&self.target, None) else {
            // Git could not read it just now (the agent's own Git holds a lock, say). The state
            // stays what it was and the next look tries again; it is not called wrong.
            return None;
        };
        let next: BTreeMap<String, FileChange> =
            found.into_iter().map(|c| (c.path.clone(), c)).collect();
        let stamps: BTreeMap<String, Stamp> = next
            .keys()
            .map(|path| (path.clone(), self.stamp(path)))
            .collect();
        let head = self.worktrees.live_head(&self.target);
        let changed =
            next != self.files || stamps != self.stamps || head != self.state.current_revision;
        self.state.last_reconciled_at = Some(now_ms());
        if !changed {
            return None;
        }
        self.files = next;
        self.stamps = stamps;
        self.state.current_revision = head;
        Some(self.commit(true, Vec::new()))
    }

    /// Looks only at `paths` (relative to the worktree) and what is under them. `None` when
    /// nothing the UI shows changed.
    pub fn apply_paths(&mut self, paths: &[String]) -> Option<LiveWorkspaceUpdate> {
        let paths = collapse(paths);
        if paths.is_empty() {
            return None;
        }
        if paths.len() > MAX_INCREMENTAL_PATHS {
            return self.reconcile();
        }
        if let Gate::Closed(update) = self.revalidate() {
            return update;
        }
        let Ok(found) = self.worktrees.live_changes(&self.target, Some(&paths)) else {
            return self.reconcile();
        };
        let under = |name: &str| paths.iter().any(|p| is_at_or_under(name, p));
        // What the state had for these paths is replaced by what Git says now. A rename pairs two
        // paths, and a path alone cannot tell whether the other half is still there: read all.
        let before: Vec<FileChange> = self
            .files
            .values()
            .filter(|c| under(&c.path) || c.old_path.as_deref().is_some_and(under))
            .cloned()
            .collect();
        if before.iter().any(|c| c.status == FileChangeStatus::Renamed) {
            return self.reconcile();
        }
        let old_stamps: BTreeMap<String, Stamp> = before
            .iter()
            .map(|old| (old.path.clone(), self.stamps.remove(&old.path).flatten()))
            .collect();
        for old in &before {
            self.files.remove(&old.path);
        }
        let mut changed = Vec::new();
        for change in found {
            let stamp = self.stamp(&change.path);
            let same = before.contains(&change) && old_stamps.get(&change.path) == Some(&stamp);
            self.stamps.insert(change.path.clone(), stamp);
            self.files.insert(change.path.clone(), change.clone());
            if !same {
                changed.push(change);
            }
        }
        let removed: Vec<String> = before
            .iter()
            .filter(|old| !self.files.contains_key(&old.path))
            .map(|old| old.path.clone())
            .collect();
        let head = self.worktrees.live_head(&self.target);
        let head_moved = head != self.state.current_revision;
        if changed.is_empty() && removed.is_empty() && !head_moved {
            return None;
        }
        self.state.current_revision = head;
        Some(self.commit_delta(changed, removed))
    }

    /// The file as it is on disk: its size and when it was last written.
    fn stamp(&self, path: &str) -> Stamp {
        let metadata = self.target.path.join(path).symlink_metadata().ok()?;
        let modified = metadata
            .modified()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_nanos();
        Some((metadata.len(), modified))
    }

    pub fn set_phase(&mut self, phase: LivePhase) -> Option<LiveWorkspaceUpdate> {
        if self.state.phase == phase {
            return None;
        }
        self.state.phase = phase;
        Some(self.commit_delta(Vec::new(), Vec::new()))
    }

    pub fn set_observation(&mut self, observation: ObservationMode) -> Option<LiveWorkspaceUpdate> {
        if self.state.observation == observation {
            return None;
        }
        self.state.observation = observation;
        Some(self.commit_delta(Vec::new(), Vec::new()))
    }

    /// Checks the worktree is still the folder Atlas made. `Closed` when it is not (the caller
    /// returns what it holds: the state says so and nothing is read), with the update that
    /// announced it if this is the first time.
    fn revalidate(&mut self) -> Gate {
        let availability = match self.worktrees.live_target(&self.target.execution_id) {
            Ok(target) => {
                self.target = target;
                return Gate::Open;
            }
            Err(LiveTargetError::Invalid) => WorktreeAvailability::Invalid,
            Err(LiveTargetError::Missing | LiveTargetError::Unknown) => {
                WorktreeAvailability::Missing
            }
        };
        if self.state.availability == availability {
            return Gate::Closed(None);
        }
        // The last files stay: they are the last thing known. Nothing new can be learned.
        self.state.availability = availability;
        self.state.observation = ObservationMode::Stopped;
        Gate::Closed(Some(self.commit_delta(Vec::new(), Vec::new())))
    }

    fn commit_delta(
        &mut self,
        changed: Vec<FileChange>,
        removed: Vec<String>,
    ) -> LiveWorkspaceUpdate {
        self.commit_with(false, changed, removed)
    }

    fn commit(&mut self, full: bool, removed: Vec<String>) -> LiveWorkspaceUpdate {
        let changed = self.files.values().cloned().collect();
        self.commit_with(full, changed, removed)
    }

    fn commit_with(
        &mut self,
        full: bool,
        changed: Vec<FileChange>,
        removed: Vec<String>,
    ) -> LiveWorkspaceUpdate {
        let all: Vec<&FileChange> = self.files.values().collect();
        self.state.files = all
            .iter()
            .take(MAX_CHANGESET_FILES)
            .map(|c| (*c).clone())
            .collect();
        self.state.files_changed = u32::try_from(all.len()).unwrap_or(u32::MAX);
        self.state.additions = all.iter().filter_map(|c| c.additions).sum();
        self.state.deletions = all.iter().filter_map(|c| c.deletions).sum();
        self.state.revision += 1;
        self.state.updated_at = now_ms();
        LiveWorkspaceUpdate {
            run_id: self.state.run_id.clone(),
            worktree_execution_id: self.state.worktree_execution_id.clone(),
            revision: self.state.revision,
            full,
            changed,
            removed,
            current_revision: self.state.current_revision.clone(),
            availability: self.state.availability,
            phase: self.state.phase,
            observation: self.state.observation,
            files_changed: self.state.files_changed,
            additions: self.state.additions,
            deletions: self.state.deletions,
            updated_at: self.state.updated_at,
        }
    }
}

/// `name` is `path`, or is inside the folder `path`.
fn is_at_or_under(name: &str, path: &str) -> bool {
    name == path
        || name
            .strip_prefix(path)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// The distinct paths, without any that is inside another of them (reading a folder reads what
/// is in it).
fn collapse(paths: &[String]) -> Vec<String> {
    let mut sorted: Vec<String> = paths.to_vec();
    sorted.sort();
    sorted.dedup();
    let mut kept: Vec<String> = Vec::new();
    for path in sorted {
        if !kept.iter().any(|k| is_at_or_under(&path, k)) {
            kept.push(path);
        }
    }
    kept
}
