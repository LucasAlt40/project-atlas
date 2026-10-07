use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// A process-unique id such as `agent-1790000000000-3`.
pub fn new_id(prefix: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    format!(
        "{prefix}-{}-{}",
        now_ms(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

/// Locking that survives a panic elsewhere. A poisoned lock only records that another thread
/// panicked while holding it; the data is still there, and refusing it would turn one failed
/// run into a failure of every command that touches the same state until the app restarts.
pub trait LockExt<T> {
    fn lock_or_recover(&self) -> std::sync::MutexGuard<'_, T>;
}

impl<T> LockExt<T> for std::sync::Mutex<T> {
    fn lock_or_recover(&self) -> std::sync::MutexGuard<'_, T> {
        self.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn a_lock_poisoned_by_a_panic_is_still_usable() {
        let shared = Arc::new(Mutex::new(1));
        let poisoner = shared.clone();
        let _ = std::thread::spawn(move || {
            let _guard = poisoner.lock().unwrap();
            panic!("a run failed while holding the lock");
        })
        .join();
        assert!(shared.lock().is_err(), "the lock should be poisoned");

        *shared.lock_or_recover() += 1;

        assert_eq!(*shared.lock_or_recover(), 2);
    }
}
