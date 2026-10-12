//! What the phone is told while it watches: a numbered stream of *hints*.
//!
//! An event says that something about a run changed, never what the run now is. The page reacts by
//! reading the run again, so the store stays the source of truth and a lost, repeated or late event
//! can only cause one more read. The hub keeps the last events so a phone that dropped for a moment
//! can pick up where it was; one that missed more than that is told to read everything again.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tokio::sync::watch;

use super::tokens;
use crate::application::support::{now_ms, LockExt};
use crate::domain::workflow::{WorkflowEvent, WorkflowEventKind};

/// Events kept for catching up.
pub const RING: usize = 512;
/// Agent activity (a stream of small events) is announced at most this often.
const ACTIVITY_EVERY: Duration = Duration::from_millis(400);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteEvent {
    pub seq: u64,
    /// `workflow:*` name of the engine's event (`interaction_detected`, `node_started`…), or
    /// `activity` for what an agent is doing, or `revoked`.
    pub kind: String,
    pub run_id: Option<String>,
    pub workspace_id: Option<String>,
    pub at: u64,
}

/// What a stream starting from a cursor gets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Replay {
    /// The events after the cursor, in order (possibly none).
    Events(Vec<RemoteEvent>),
    /// The cursor is from another start or older than what is kept: read everything again.
    Resync,
}

struct Inner {
    epoch: String,
    seq: u64,
    ring: VecDeque<RemoteEvent>,
    last_activity: Option<std::time::Instant>,
}

pub struct EventHub {
    inner: Mutex<Inner>,
    wake: watch::Sender<u64>,
}

impl Default for EventHub {
    fn default() -> Self {
        Self::new()
    }
}

impl EventHub {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                epoch: tokens::epoch(),
                seq: 0,
                ring: VecDeque::new(),
                last_activity: None,
            }),
            wake: watch::channel(0).0,
        }
    }

    /// Starts a new stream: cursors of the previous one no longer mean anything.
    pub fn reset(&self) -> String {
        let mut inner = self.inner.lock_or_recover();
        inner.epoch = tokens::epoch();
        inner.seq = 0;
        inner.ring.clear();
        inner.last_activity = None;
        let epoch = inner.epoch.clone();
        drop(inner);
        self.wake.send_replace(0);
        epoch
    }

    pub fn epoch(&self) -> String {
        self.inner.lock_or_recover().epoch.clone()
    }

    pub fn seq(&self) -> u64 {
        self.inner.lock_or_recover().seq
    }

    /// A run's state changed.
    pub fn publish_workflow(&self, event: &WorkflowEvent) {
        // Guardrail notes and handoffs change nothing a phone shows differently.
        if matches!(
            event.kind,
            WorkflowEventKind::GuardrailTransformed | WorkflowEventKind::HandoffCreated
        ) {
            return;
        }
        let kind = event
            .kind
            .wire_name()
            .trim_start_matches("workflow:")
            .to_owned();
        self.push(
            &kind,
            Some(event.execution_id.clone()),
            Some(event.workspace_id.clone()),
        );
    }

    /// An agent did something worth showing in a run's log. Throttled: output arrives in streams.
    pub fn publish_activity(&self, workspace_id: &str) {
        {
            let mut inner = self.inner.lock_or_recover();
            let now = std::time::Instant::now();
            if inner
                .last_activity
                .is_some_and(|last| now.duration_since(last) < ACTIVITY_EVERY)
            {
                return;
            }
            inner.last_activity = Some(now);
        }
        self.push("activity", None, Some(workspace_id.to_owned()));
    }

    /// Wakes the open streams so they notice a revoked session now rather than at the next beat.
    pub fn publish_revoked(&self) {
        self.push("revoked", None, None);
    }

    fn push(&self, kind: &str, run_id: Option<String>, workspace_id: Option<String>) {
        let mut inner = self.inner.lock_or_recover();
        inner.seq += 1;
        let event = RemoteEvent {
            seq: inner.seq,
            kind: kind.to_owned(),
            run_id,
            workspace_id,
            at: now_ms(),
        };
        let seq = event.seq;
        inner.ring.push_back(event);
        while inner.ring.len() > RING {
            inner.ring.pop_front();
        }
        drop(inner);
        self.wake.send_replace(seq);
    }

    /// What a stream that last saw `after` (of `epoch`) has missed.
    pub fn replay(&self, epoch: Option<&str>, after: u64) -> Replay {
        let inner = self.inner.lock_or_recover();
        if epoch != Some(inner.epoch.as_str()) || after > inner.seq {
            return Replay::Resync;
        }
        let oldest = inner.ring.front().map_or(inner.seq + 1, |e| e.seq);
        // Nothing is missing when the cursor is at the end, or right before the oldest kept.
        if after + 1 < oldest && after < inner.seq {
            return Replay::Resync;
        }
        Replay::Events(
            inner
                .ring
                .iter()
                .filter(|e| e.seq > after)
                .cloned()
                .collect(),
        )
    }

    /// A receiver that changes whenever an event is published.
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.wake.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn event(kind: WorkflowEventKind, run: &str) -> WorkflowEvent {
        WorkflowEvent {
            kind,
            workflow_id: "wf".into(),
            execution_id: run.into(),
            workspace_id: "ws".into(),
            node_id: None,
            message: "secret-looking message".into(),
            timestamp: 1,
            metadata: BTreeMap::new(),
        }
    }

    #[test]
    fn events_are_numbered_and_carry_ids_not_state() {
        let hub = EventHub::new();
        hub.publish_workflow(&event(WorkflowEventKind::Started, "r1"));
        hub.publish_workflow(&event(WorkflowEventKind::InteractionDetected, "r1"));
        let Replay::Events(events) = hub.replay(Some(&hub.epoch()), 0) else {
            panic!("expected events");
        };
        assert_eq!(events.iter().map(|e| e.seq).collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(events[1].kind, "interaction_detected");
        assert_eq!(events[1].run_id.as_deref(), Some("r1"));
        let wire = serde_json::to_string(&events[1]).unwrap();
        assert!(
            !wire.contains("secret-looking"),
            "an event never carries text"
        );
    }

    #[test]
    fn a_stream_resumes_after_its_cursor_without_repeating() {
        let hub = EventHub::new();
        for _ in 0..5 {
            hub.publish_workflow(&event(WorkflowEventKind::NodeStarted, "r1"));
        }
        let epoch = hub.epoch();
        let Replay::Events(events) = hub.replay(Some(&epoch), 3) else {
            panic!("expected events");
        };
        assert_eq!(events.iter().map(|e| e.seq).collect::<Vec<_>>(), vec![4, 5]);
        assert_eq!(hub.replay(Some(&epoch), 5), Replay::Events(vec![]));
    }

    #[test]
    fn a_cursor_from_another_start_or_from_the_future_means_read_everything() {
        let hub = EventHub::new();
        hub.publish_workflow(&event(WorkflowEventKind::Started, "r1"));
        assert_eq!(hub.replay(Some("old-epoch"), 1), Replay::Resync);
        assert_eq!(hub.replay(None, 0), Replay::Resync);
        assert_eq!(hub.replay(Some(&hub.epoch()), 99), Replay::Resync);
        let before = hub.epoch();
        let after = hub.reset();
        assert_ne!(before, after);
        assert_eq!(hub.replay(Some(&before), 0), Replay::Resync);
    }

    #[test]
    fn a_phone_that_missed_more_than_is_kept_is_told_to_resync() {
        let hub = EventHub::new();
        for _ in 0..(RING + 10) {
            hub.publish_workflow(&event(WorkflowEventKind::NodeStarted, "r1"));
        }
        assert_eq!(hub.replay(Some(&hub.epoch()), 3), Replay::Resync);
        let tail = u64::try_from(RING).unwrap() + 10 - 5;
        assert!(matches!(
            hub.replay(Some(&hub.epoch()), tail),
            Replay::Events(e) if e.len() == 5
        ));
    }

    #[test]
    fn a_burst_of_activity_is_one_event() {
        let hub = EventHub::new();
        for _ in 0..50 {
            hub.publish_activity("ws");
        }
        assert_eq!(hub.seq(), 1);
    }

    #[test]
    fn publishing_wakes_the_listeners() {
        let hub = EventHub::new();
        let mut rx = hub.subscribe();
        hub.publish_workflow(&event(WorkflowEventKind::Failed, "r1"));
        assert!(rx.has_changed().unwrap());
        assert_eq!(*rx.borrow_and_update(), 1);
    }
}
