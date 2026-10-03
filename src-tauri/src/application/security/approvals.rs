//! Approvals: the only way an operation that "requires approval" ever runs.
//!
//! An approval is answered by [`ApprovalBroker::resolve`], which the UI reaches through the
//! `resolve_approval` command. Nothing a model writes can reach it: model output is data in a
//! conversation, never an input here.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::application::support::now_ms;
use crate::domain::security::{PermissionAction, Reason};

/// What the user is shown while a command waits: who wants to run what, where, and why Atlas
/// did not just allow it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingApproval {
    pub id: String,
    pub execution_id: String,
    pub workspace_id: String,
    pub task_id: String,
    pub agent_id: String,
    pub agent_name: String,
    pub action: PermissionAction,
    pub executable: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    /// Why Atlas asks (a stable code the UI words).
    pub reason: Reason,
    /// Milliseconds since the Unix epoch.
    pub requested_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    Approved,
    Rejected,
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalError {
    /// Nothing is waiting under that id (already answered, timed out, or never existed).
    Unknown(String),
}

struct Slot {
    request: PendingApproval,
    answer: Option<Answer>,
}

/// Holds the operations that are waiting for the user. A waiting operation blocks its own
/// execution thread, nothing else, and is released only by an explicit answer or by timing out
/// (which counts as a rejection).
#[derive(Default)]
pub struct ApprovalBroker {
    slots: Mutex<BTreeMap<String, Slot>>,
    changed: Condvar,
    next_id: AtomicU64,
}

impl ApprovalBroker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a request and returns its id. The request is visible to
    /// [`Self::pending`] from now on; [`Self::wait`] blocks for the answer.
    pub fn open(&self, build: impl FnOnce(String) -> PendingApproval) -> PendingApproval {
        let id = format!(
            "approval-{}",
            self.next_id.fetch_add(1, Ordering::Relaxed) + 1
        );
        let mut request = build(id.clone());
        request.id.clone_from(&id);
        request.requested_at = now_ms();
        self.slots.lock().expect("approvals lock poisoned").insert(
            id,
            Slot {
                request: request.clone(),
                answer: None,
            },
        );
        request
    }

    /// Blocks until the request is answered or `timeout` passes, then forgets the request.
    pub fn wait(&self, id: &str, timeout: Duration) -> Answer {
        let deadline = Instant::now() + timeout;
        let mut slots = self.slots.lock().expect("approvals lock poisoned");
        loop {
            if let Some(answer) = slots.get(id).and_then(|slot| slot.answer) {
                slots.remove(id);
                return answer;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() || !slots.contains_key(id) {
                slots.remove(id);
                return Answer::TimedOut;
            }
            slots = self
                .changed
                .wait_timeout(slots, remaining)
                .expect("approvals lock poisoned")
                .0;
        }
    }

    /// The user's answer. `approve = false` rejects.
    ///
    /// # Errors
    ///
    /// Fails if nothing is waiting under `id`.
    pub fn resolve(&self, id: &str, approve: bool) -> Result<PendingApproval, ApprovalError> {
        let mut slots = self.slots.lock().expect("approvals lock poisoned");
        let slot = slots
            .get_mut(id)
            .filter(|slot| slot.answer.is_none())
            .ok_or_else(|| ApprovalError::Unknown(id.to_owned()))?;
        slot.answer = Some(if approve {
            Answer::Approved
        } else {
            Answer::Rejected
        });
        let request = slot.request.clone();
        self.changed.notify_all();
        Ok(request)
    }

    /// Requests still waiting, oldest first, optionally for one workspace.
    pub fn pending(&self, workspace_id: Option<&str>) -> Vec<PendingApproval> {
        let slots = self.slots.lock().expect("approvals lock poisoned");
        let mut pending: Vec<_> = slots
            .values()
            .filter(|slot| slot.answer.is_none())
            .map(|slot| slot.request.clone())
            .filter(|request| workspace_id.is_none_or(|id| request.workspace_id == id))
            .collect();
        pending.sort_by_key(|request| request.requested_at);
        pending
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;

    use super::*;

    fn request(workspace: &str) -> impl FnOnce(String) -> PendingApproval + '_ {
        move |id| PendingApproval {
            id,
            execution_id: "exec-1".to_owned(),
            workspace_id: workspace.to_owned(),
            task_id: "task-1".to_owned(),
            agent_id: "agent-1".to_owned(),
            agent_name: "Dev".to_owned(),
            action: PermissionAction::RunProcess,
            executable: "make".to_owned(),
            args: vec!["all".to_owned()],
            cwd: Some("/atlas".to_owned()),
            reason: Reason::NotInAllowedList,
            requested_at: 0,
        }
    }

    #[test]
    fn a_request_waits_until_it_is_answered() {
        let broker = Arc::new(ApprovalBroker::new());
        let opened = broker.open(request("ws"));
        assert_eq!(broker.pending(None).len(), 1);

        let waiter = {
            let broker = broker.clone();
            let id = opened.id.clone();
            thread::spawn(move || broker.wait(&id, Duration::from_secs(10)))
        };
        thread::sleep(Duration::from_millis(50));
        assert!(!waiter.is_finished(), "must not proceed before an answer");

        broker.resolve(&opened.id, true).unwrap();

        assert_eq!(waiter.join().unwrap(), Answer::Approved);
        assert_eq!(broker.pending(None).len(), 0);
    }

    #[test]
    fn rejecting_releases_the_waiter_with_a_rejection() {
        let broker = Arc::new(ApprovalBroker::new());
        let opened = broker.open(request("ws"));
        broker.resolve(&opened.id, false).unwrap();

        // Answered before anyone waits: the answer is not lost.
        assert_eq!(
            broker.wait(&opened.id, Duration::from_secs(1)),
            Answer::Rejected
        );
    }

    #[test]
    fn no_answer_in_time_counts_as_a_timeout_and_the_request_is_gone() {
        let broker = ApprovalBroker::new();
        let opened = broker.open(request("ws"));

        assert_eq!(
            broker.wait(&opened.id, Duration::from_millis(30)),
            Answer::TimedOut
        );
        assert_eq!(broker.pending(None).len(), 0);
        assert_eq!(
            broker.resolve(&opened.id, true),
            Err(ApprovalError::Unknown(opened.id))
        );
    }

    #[test]
    fn a_request_can_be_answered_only_once() {
        let broker = ApprovalBroker::new();
        let opened = broker.open(request("ws"));

        assert!(broker.resolve(&opened.id, true).is_ok());
        assert!(broker.resolve(&opened.id, false).is_err());
        assert!(broker.resolve("approval-999", true).is_err());
    }

    #[test]
    fn pending_requests_can_be_listed_per_workspace() {
        let broker = ApprovalBroker::new();
        broker.open(request("a"));
        broker.open(request("b"));
        broker.open(request("a"));

        assert_eq!(broker.pending(Some("a")).len(), 2);
        assert_eq!(broker.pending(Some("b")).len(), 1);
        assert_eq!(broker.pending(None).len(), 3);
    }
}
