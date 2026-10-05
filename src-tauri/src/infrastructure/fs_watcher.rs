//! [`WorktreeWatcher`] over the platform's filesystem events (`FSEvents`, `inotify`, `ReadDirectoryChangesW`).
//!
//! It only reports *where* something happened. It reads no file and runs no Git: that is the
//! observer's job, after the events settle. A platform that cannot watch the folder says so, and
//! the observer looks on a schedule instead.

use std::path::Path;

use notify::{recommended_watcher, Event, EventKind, RecursiveMode, Watcher};

use crate::application::live_workspace::{WatchSignal, WorktreeWatcher};

#[derive(Default)]
pub struct NotifyWatcher;

impl WorktreeWatcher for NotifyWatcher {
    fn watch(
        &self,
        root: &Path,
        on_signal: Box<dyn Fn(WatchSignal) + Send + Sync>,
    ) -> Result<Box<dyn Send>, String> {
        let mut watcher = recommended_watcher(move |result: notify::Result<Event>| match result {
            Ok(event) if event.need_rescan() => on_signal(WatchSignal::Rescan),
            // Reading a file is not changing it.
            Ok(event) if matches!(event.kind, EventKind::Access(_)) => {}
            Ok(event) => on_signal(WatchSignal::Paths(event.paths)),
            // The queue overflowed or the watch broke: events may have been lost.
            Err(_) => on_signal(WatchSignal::Rescan),
        })
        .map_err(|e| e.to_string())?;
        watcher
            .watch(root, RecursiveMode::Recursive)
            .map_err(|e| e.to_string())?;
        Ok(Box::new(watcher))
    }
}
