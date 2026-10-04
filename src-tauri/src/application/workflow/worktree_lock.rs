//! Who may use a run's shared worktree right now. The worktree is one place the whole run reads
//! and writes, so a step that can change it must have it to itself: readers never see code half
//! way through an edit, and one writer's commits and change set never include another's.
//!
//! Which steps write is not decided here (nor by who the agent is): the runner says, from the
//! runtime's capability and the agent's policy. Readers share the worktree with each other.
//! The lock belongs to the run: it lives in the orchestrator's loop for that run alone.
//!
//! There is nothing to forget to release. Each round the lock is reconciled with the steps
//! really in flight, so a step that ended in any way (completed, failed, cancelled, panicked,
//! timed out, or gone with an interrupted run) no longer holds it.

use std::collections::{BTreeMap, BTreeSet};

use super::runner::StepAccess;

#[derive(Debug, Default)]
pub struct WorktreeLock {
    holders: BTreeMap<String, StepAccess>,
    /// A writer is waiting in this round: readers that come after it do not slip in front.
    writer_waiting: bool,
}

impl WorktreeLock {
    /// Starts a round of dispatching: drops every holder that is no longer in flight.
    pub fn begin_round(&mut self, in_flight: &BTreeSet<String>) {
        self.holders.retain(|node, _| in_flight.contains(node));
        self.writer_waiting = false;
    }

    /// Takes the lock for a step, or says it must wait.
    pub fn try_acquire(&mut self, node_id: &str, access: StepAccess) -> bool {
        // A step that already holds the lock (one resuming after the person answered) keeps it.
        if self.holders.get(node_id) == Some(&access) {
            return true;
        }
        match access {
            StepAccess::Write => {
                if self.holders.is_empty() {
                    self.holders.insert(node_id.to_owned(), access);
                    true
                } else {
                    self.writer_waiting = true;
                    false
                }
            }
            StepAccess::Read => {
                if self.writer_waiting || self.holders.values().any(|a| *a == StepAccess::Write) {
                    false
                } else {
                    self.holders.insert(node_id.to_owned(), access);
                    true
                }
            }
        }
    }

    pub fn release(&mut self, node_id: &str) {
        self.holders.remove(node_id);
    }

    #[cfg(test)]
    pub fn holders(&self) -> Vec<(&str, StepAccess)> {
        self.holders.iter().map(|(n, a)| (n.as_str(), *a)).collect()
    }
}

#[cfg(test)]
#[allow(clippy::assert_is_empty)]
mod tests {
    use super::*;

    #[test]
    fn a_writer_that_asked_a_person_keeps_its_place_until_it_goes_on() {
        let mut lock = WorktreeLock::default();
        assert!(lock.try_acquire("dev", StepAccess::Write));

        // It waits for an answer: still in flight, so nothing else gets in.
        lock.begin_round(&BTreeSet::from(["dev".to_owned()]));
        assert!(!lock.try_acquire("other", StepAccess::Write));
        assert!(!lock.try_acquire("reader", StepAccess::Read));

        // The answer came; it is about to continue and holds the lock through the round.
        lock.begin_round(&BTreeSet::from(["dev".to_owned()]));
        assert!(lock.try_acquire("dev", StepAccess::Write));
        assert_eq!(lock.holders(), [("dev", StepAccess::Write)]);
    }

    #[test]
    fn a_writer_has_the_worktree_to_itself() {
        let mut lock = WorktreeLock::default();

        assert!(lock.try_acquire("a", StepAccess::Write));
        assert!(!lock.try_acquire("b", StepAccess::Write));
        assert!(!lock.try_acquire("v", StepAccess::Read));

        lock.release("a");
        lock.begin_round(&BTreeSet::new());
        assert!(lock.try_acquire("b", StepAccess::Write));
    }

    #[test]
    fn readers_share_and_keep_a_writer_waiting_until_they_are_done() {
        let mut lock = WorktreeLock::default();

        assert!(lock.try_acquire("r1", StepAccess::Read));
        assert!(lock.try_acquire("r2", StepAccess::Read));
        assert!(!lock.try_acquire("w", StepAccess::Write));
        // The waiting writer is not overtaken by a reader that comes after it.
        assert!(!lock.try_acquire("r3", StepAccess::Read));

        let still: BTreeSet<String> = ["r1".to_owned()].into();
        lock.begin_round(&still);
        assert_eq!(lock.holders(), [("r1", StepAccess::Read)]);
        assert!(!lock.try_acquire("w", StepAccess::Write));
        lock.begin_round(&BTreeSet::new());
        assert!(lock.try_acquire("w", StepAccess::Write));
    }

    #[test]
    fn a_holder_that_is_no_longer_in_flight_never_keeps_the_lock() {
        let mut lock = WorktreeLock::default();
        assert!(lock.try_acquire("w", StepAccess::Write));

        // The step ended some way nobody reported: the next round frees it.
        lock.begin_round(&BTreeSet::new());

        assert!(lock.holders().is_empty());
        assert!(lock.try_acquire("other", StepAccess::Write));
    }
}
