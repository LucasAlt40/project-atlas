//! A small fixed-window counter, to keep a client from hammering what changes things.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::application::support::LockExt;

pub struct RateLimiter {
    limit: u32,
    window_ms: u64,
    /// key → (window start, hits in it)
    hits: Mutex<HashMap<String, (u64, u32)>>,
}

impl RateLimiter {
    pub fn new(limit: u32, window_ms: u64) -> Self {
        Self {
            limit,
            window_ms,
            hits: Mutex::new(HashMap::new()),
        }
    }

    /// Counts one hit; `false` when the key is over its limit for this window.
    pub fn allow(&self, key: &str, now: u64) -> bool {
        let mut hits = self.hits.lock_or_recover();
        if hits.len() > 1024 {
            let window = self.window_ms;
            hits.retain(|_, (start, _)| now.saturating_sub(*start) < window);
        }
        let entry = hits.entry(key.to_owned()).or_insert((now, 0));
        if now.saturating_sub(entry.0) >= self.window_ms {
            *entry = (now, 0);
        }
        entry.1 += 1;
        entry.1 <= self.limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_over_its_limit_waits_for_the_next_window() {
        let limiter = RateLimiter::new(3, 1000);
        assert!(limiter.allow("a", 0));
        assert!(limiter.allow("a", 1));
        assert!(limiter.allow("a", 2));
        assert!(!limiter.allow("a", 3));
        assert!(limiter.allow("b", 3), "keys do not share a budget");
        assert!(limiter.allow("a", 1000));
    }
}
