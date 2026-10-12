//! Who may talk to the mobile server: pairing codes and the sessions they are exchanged for.
//!
//! Both live in memory only. Stopping the server (or closing Atlas) ends every session, so each
//! activation is a new, explicit authorisation and no secret is ever written to disk.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;

use super::tokens;
use crate::application::support::LockExt;

/// A pairing code lives this long.
pub const PAIRING_TTL_MS: u64 = 5 * 60 * 1000;
/// A session unused for this long ends.
pub const SESSION_IDLE_MS: u64 = 24 * 60 * 60 * 1000;
/// A session ends this long after pairing, used or not.
pub const SESSION_MAX_MS: u64 = 7 * 24 * 60 * 60 * 1000;
/// Failed pairings in a row that lock pairing.
const MAX_FAILURES: u32 = 5;
const LOCK_BASE_MS: u64 = 60 * 1000;
const LOCK_MAX_MS: u64 = 15 * 60 * 1000;
/// Most devices that may be paired at once.
pub const MAX_DEVICES: usize = 8;

/// A paired phone, as the desktop lists it. Holds nothing secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: String,
    pub name: String,
    /// The address it connected from.
    pub address: String,
    pub paired_at: u64,
    pub last_seen: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthError {
    /// Not a token of this server: never issued, revoked, or from another start.
    Unknown,
    Expired,
}

struct Session {
    device: Device,
}

#[derive(Default)]
pub struct SessionStore {
    /// By digest of the token.
    sessions: Mutex<HashMap<String, Session>>,
}

impl SessionStore {
    /// Starts a session for a freshly paired device and returns its token, which is the only time
    /// it exists in clear.
    ///
    /// # Errors
    ///
    /// `None` when [`MAX_DEVICES`] are already paired.
    pub fn open(&self, name: &str, address: &str, now: u64) -> Option<(String, Device)> {
        let mut sessions = self.sessions.lock_or_recover();
        Self::sweep(&mut sessions, now);
        if sessions.len() >= MAX_DEVICES {
            return None;
        }
        let token = tokens::session_token();
        let device = Device {
            id: tokens::device_id(),
            name: clean_name(name),
            address: address.to_owned(),
            paired_at: now,
            last_seen: now,
        };
        sessions.insert(
            tokens::digest(&token),
            Session {
                device: device.clone(),
            },
        );
        Some((token, device))
    }

    /// The device a token belongs to, and a note that it was just seen.
    ///
    /// # Errors
    ///
    /// [`AuthError`] if the token is not (or no longer) valid.
    pub fn authenticate(&self, token: &str, now: u64) -> Result<Device, AuthError> {
        let key = tokens::digest(token);
        let mut sessions = self.sessions.lock_or_recover();
        let session = sessions.get_mut(&key).ok_or(AuthError::Unknown)?;
        if expired(&session.device, now) {
            sessions.remove(&key);
            return Err(AuthError::Expired);
        }
        session.device.last_seen = now;
        Ok(session.device.clone())
    }

    /// Whether the device still has a session (used by open streams to notice a revocation).
    pub fn is_active(&self, device_id: &str, now: u64) -> bool {
        self.sessions
            .lock_or_recover()
            .values()
            .any(|s| s.device.id == device_id && !expired(&s.device, now))
    }

    /// Ends one device's session. `false` if there was none.
    pub fn revoke(&self, device_id: &str) -> bool {
        let mut sessions = self.sessions.lock_or_recover();
        let before = sessions.len();
        sessions.retain(|_, s| s.device.id != device_id);
        sessions.len() != before
    }

    /// Ends every session; returns how many there were.
    pub fn revoke_all(&self) -> usize {
        let mut sessions = self.sessions.lock_or_recover();
        let count = sessions.len();
        sessions.clear();
        count
    }

    /// The paired devices, oldest first.
    pub fn devices(&self, now: u64) -> Vec<Device> {
        let mut sessions = self.sessions.lock_or_recover();
        Self::sweep(&mut sessions, now);
        let mut devices: Vec<Device> = sessions.values().map(|s| s.device.clone()).collect();
        devices.sort_by(|a, b| a.paired_at.cmp(&b.paired_at).then(a.id.cmp(&b.id)));
        devices
    }

    fn sweep(sessions: &mut HashMap<String, Session>, now: u64) {
        sessions.retain(|_, s| !expired(&s.device, now));
    }
}

fn expired(device: &Device, now: u64) -> bool {
    now.saturating_sub(device.last_seen) > SESSION_IDLE_MS
        || now.saturating_sub(device.paired_at) > SESSION_MAX_MS
}

/// The name a device gave itself: printable, short, never empty.
fn clean_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| !c.is_control())
        .take(40)
        .collect::<String>()
        .trim()
        .to_owned();
    if cleaned.is_empty() {
        "Phone".to_owned()
    } else {
        cleaned
    }
}

/// Why a pairing was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairError {
    /// Not a code that is valid now (wrong, used, expired or replaced).
    Invalid,
    /// Too many wrong codes: pairing is closed until then (milliseconds from now).
    Locked { retry_after_ms: u64 },
}

/// The code a desktop shows, with what the page needs to draw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedCode {
    /// Unformatted, as encoded in the QR.
    pub code: String,
    pub expires_at: u64,
}

#[derive(Default)]
struct Pairing {
    /// Digest of the code → when it expires. At most one is valid at a time.
    pending: HashMap<String, u64>,
    /// The code in clear, for as long as the desktop dialog needs to show it.
    shown: Option<IssuedCode>,
    failures: u32,
    lockouts: u32,
    locked_until: u64,
}

#[derive(Default)]
pub struct PairingStore {
    inner: Mutex<Pairing>,
}

impl PairingStore {
    /// A new code. Any earlier one that was not used stops working.
    pub fn issue(&self, now: u64) -> IssuedCode {
        let code = tokens::pairing_code();
        let issued = IssuedCode {
            code: code.clone(),
            expires_at: now + PAIRING_TTL_MS,
        };
        let mut inner = self.inner.lock_or_recover();
        inner.pending.clear();
        inner
            .pending
            .insert(tokens::digest(&code), issued.expires_at);
        inner.shown = Some(issued.clone());
        issued
    }

    /// The code that can still be used, if there is one.
    pub fn current(&self, now: u64) -> Option<IssuedCode> {
        let inner = self.inner.lock_or_recover();
        inner.shown.clone().filter(|c| c.expires_at > now)
    }

    /// Spends a code. A code works once: this is the only way to turn one into a session.
    ///
    /// # Errors
    ///
    /// [`PairError::Invalid`] for a code that is not valid; [`PairError::Locked`] while pairing is
    /// closed after repeated failures (a right code does not get through a lock either).
    pub fn redeem(&self, typed: &str, now: u64) -> Result<(), PairError> {
        let mut inner = self.inner.lock_or_recover();
        if inner.locked_until > now {
            return Err(PairError::Locked {
                retry_after_ms: inner.locked_until - now,
            });
        }
        let key = tokens::digest(&tokens::normalize_code(typed));
        match inner.pending.remove(&key) {
            Some(expires_at) if expires_at > now => {
                inner.shown = None;
                inner.failures = 0;
                inner.lockouts = 0;
                Ok(())
            }
            _ => {
                inner.failures += 1;
                if inner.failures >= MAX_FAILURES {
                    // Whatever a guesser was after is gone: a new code has to be asked for.
                    inner.pending.clear();
                    inner.shown = None;
                    inner.failures = 0;
                    let wait = (LOCK_BASE_MS << inner.lockouts.min(5)).min(LOCK_MAX_MS);
                    inner.lockouts += 1;
                    inner.locked_until = now + wait;
                    return Err(PairError::Locked {
                        retry_after_ms: wait,
                    });
                }
                Err(PairError::Invalid)
            }
        }
    }

    /// Drops every code (the server is stopping).
    pub fn clear(&self) {
        let mut inner = self.inner.lock_or_recover();
        inner.pending.clear();
        inner.shown = None;
        inner.failures = 0;
        inner.lockouts = 0;
        inner.locked_until = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: u64 = 1_000_000;

    #[test]
    fn a_code_works_once() {
        let store = PairingStore::default();
        let code = store.issue(T0).code;
        assert_eq!(store.redeem(&code, T0 + 1), Ok(()));
        assert_eq!(store.redeem(&code, T0 + 2), Err(PairError::Invalid));
        assert!(store.current(T0 + 2).is_none());
    }

    #[test]
    fn a_code_expires() {
        let store = PairingStore::default();
        let code = store.issue(T0).code;
        assert_eq!(
            store.redeem(&code, T0 + PAIRING_TTL_MS + 1),
            Err(PairError::Invalid)
        );
    }

    #[test]
    fn a_new_code_replaces_the_old_one() {
        let store = PairingStore::default();
        let old = store.issue(T0).code;
        let new = store.issue(T0 + 10).code;
        assert_eq!(store.redeem(&old, T0 + 20), Err(PairError::Invalid));
        assert_eq!(store.redeem(&new, T0 + 30), Ok(()));
    }

    #[test]
    fn a_code_is_accepted_as_a_person_types_it() {
        let store = PairingStore::default();
        let code = store.issue(T0).code;
        let typed = tokens::display_code(&code).to_lowercase();
        assert_eq!(store.redeem(&typed, T0 + 1), Ok(()));
    }

    #[test]
    fn repeated_failures_lock_pairing_and_burn_the_code() {
        let store = PairingStore::default();
        let code = store.issue(T0).code;
        for i in 0..4 {
            assert_eq!(
                store.redeem("WRONGWRONGWRONG1", T0 + i),
                Err(PairError::Invalid)
            );
        }
        assert!(matches!(
            store.redeem("WRONGWRONGWRONG1", T0 + 5),
            Err(PairError::Locked { .. })
        ));
        // The right code neither works during the lock...
        assert!(matches!(
            store.redeem(&code, T0 + 6),
            Err(PairError::Locked { .. })
        ));
        // ...nor after it: it was withdrawn when the lock began.
        assert_eq!(
            store.redeem(&code, T0 + LOCK_BASE_MS + 100),
            Err(PairError::Invalid)
        );
    }

    #[test]
    fn locks_grow_while_the_guessing_goes_on() {
        let store = PairingStore::default();
        let mut now = T0;
        let mut waits = Vec::new();
        for _ in 0..3 {
            let mut last = Err(PairError::Invalid);
            for _ in 0..MAX_FAILURES {
                last = store.redeem("WRONGWRONGWRONG1", now);
            }
            let Err(PairError::Locked { retry_after_ms }) = last else {
                panic!("expected a lock");
            };
            waits.push(retry_after_ms);
            now += retry_after_ms + 1;
        }
        assert_eq!(waits, vec![60_000, 120_000, 240_000]);
    }

    #[test]
    fn a_session_is_found_by_its_token_and_only_by_it() {
        let sessions = SessionStore::default();
        let (token, device) = sessions.open("Pixel", "192.168.0.9", T0).unwrap();
        assert_eq!(sessions.authenticate(&token, T0 + 5).unwrap().id, device.id);
        assert_eq!(
            sessions.authenticate("0".repeat(64).as_str(), T0),
            Err(AuthError::Unknown)
        );
        assert_eq!(sessions.authenticate("", T0), Err(AuthError::Unknown));
    }

    #[test]
    fn a_revoked_session_stops_working_at_once() {
        let sessions = SessionStore::default();
        let (token, device) = sessions.open("Pixel", "192.168.0.9", T0).unwrap();
        assert!(sessions.is_active(&device.id, T0));
        assert!(sessions.revoke(&device.id));
        assert_eq!(
            sessions.authenticate(&token, T0 + 1),
            Err(AuthError::Unknown)
        );
        assert!(!sessions.is_active(&device.id, T0 + 1));
        assert!(!sessions.revoke(&device.id));
    }

    #[test]
    fn revoking_all_ends_every_device() {
        let sessions = SessionStore::default();
        let (a, _) = sessions.open("A", "x", T0).unwrap();
        let (b, _) = sessions.open("B", "y", T0).unwrap();
        assert_eq!(sessions.revoke_all(), 2);
        assert!(sessions.authenticate(&a, T0).is_err());
        assert!(sessions.authenticate(&b, T0).is_err());
        assert_eq!(sessions.devices(T0).len(), 0);
    }

    #[test]
    fn an_idle_session_expires_and_use_extends_it() {
        let sessions = SessionStore::default();
        let (token, _) = sessions.open("Pixel", "x", T0).unwrap();
        let used = T0 + SESSION_IDLE_MS - 1;
        assert!(sessions.authenticate(&token, used).is_ok());
        assert!(sessions
            .authenticate(&token, used + SESSION_IDLE_MS - 1)
            .is_ok());
        let (idle, _) = sessions.open("Idle", "x", T0).unwrap();
        assert_eq!(
            sessions.authenticate(&idle, T0 + SESSION_IDLE_MS + 1),
            Err(AuthError::Expired)
        );
    }

    #[test]
    fn a_session_has_an_absolute_end() {
        let sessions = SessionStore::default();
        let (token, _) = sessions.open("Pixel", "x", T0).unwrap();
        let mut now = T0;
        while now < T0 + SESSION_MAX_MS {
            now += SESSION_IDLE_MS / 2;
            let _ = sessions.authenticate(&token, now);
        }
        assert_eq!(
            sessions.authenticate(&token, T0 + SESSION_MAX_MS + 1),
            Err(AuthError::Expired)
        );
    }

    #[test]
    fn the_number_of_devices_is_bounded() {
        let sessions = SessionStore::default();
        for i in 0..MAX_DEVICES {
            assert!(sessions.open(&format!("d{i}"), "x", T0).is_some());
        }
        assert!(sessions.open("one too many", "x", T0).is_none());
    }

    #[test]
    fn a_device_name_is_printable_and_short() {
        let sessions = SessionStore::default();
        let (_, device) = sessions
            .open(&format!("  \u{7}bad\nname{}", "x".repeat(100)), "a", T0)
            .unwrap();
        assert!(device.name.len() <= 40 && !device.name.contains('\n'));
        let (_, anonymous) = sessions.open("   ", "a", T0).unwrap();
        assert_eq!(anonymous.name, "Phone");
    }
}
