//! The security audit trail: every permission decision, kept per execution.
//!
//! There is no separate database. The decisions are collected here while an execution runs and
//! then stored on the execution record (`Execution::permission_events`) and in its log. As they
//! happen they are also announced through a [`PermissionSink`] so the UI can show them live.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::domain::security::PermissionEvent;

/// Port: told about each decision the moment it is made.
pub trait PermissionSink: Send + Sync {
    fn on_permission_event(&self, event: &PermissionEvent);
}

#[derive(Default)]
pub struct AuditLog {
    events: Mutex<HashMap<String, Vec<PermissionEvent>>>,
    sink: Option<Arc<dyn PermissionSink>>,
}

impl AuditLog {
    pub fn with_sink(sink: Arc<dyn PermissionSink>) -> Self {
        Self {
            events: Mutex::default(),
            sink: Some(sink),
        }
    }

    pub fn record(&self, event: PermissionEvent) {
        if let Some(sink) = &self.sink {
            sink.on_permission_event(&event);
        }
        self.events
            .lock()
            .expect("audit lock poisoned")
            .entry(event.execution_id.clone())
            .or_default()
            .push(event);
    }

    /// Hands over (and forgets) what was recorded for an execution, oldest first.
    pub fn take(&self, execution_id: &str) -> Vec<PermissionEvent> {
        self.events
            .lock()
            .expect("audit lock poisoned")
            .remove(execution_id)
            .unwrap_or_default()
    }
}
