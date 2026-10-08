//! The OS credential store (macOS Keychain, Windows Credential Manager, Linux Secret Service)
//! behind the `CredentialStore` port. The only place that knows a library is involved.

use keyring::Entry;

use crate::application::mcp::{CredentialError, CredentialStore};
use crate::domain::mcp::Secret;

/// One credential per (connection, name), under one service name, so they are Atlas's and easy to
/// tell from anything else the user keeps there.
pub struct KeyringCredentialStore {
    service: String,
}

impl KeyringCredentialStore {
    pub fn new() -> Self {
        Self {
            service: "project-atlas.mcp".to_owned(),
        }
    }

    fn entry(&self, connection_id: &str, name: &str) -> Result<Entry, CredentialError> {
        Entry::new(&self.service, &format!("{connection_id}/{name}")).map_err(|e| unavailable(&e))
    }
}

impl Default for KeyringCredentialStore {
    fn default() -> Self {
        Self::new()
    }
}

/// The store's own message is kept for the technical details; it never contains the secret.
fn unavailable(error: &keyring::Error) -> CredentialError {
    CredentialError::Unavailable(error.to_string())
}

impl CredentialStore for KeyringCredentialStore {
    fn put(&self, connection_id: &str, name: &str, secret: &Secret) -> Result<(), CredentialError> {
        self.entry(connection_id, name)?
            .set_password(secret.expose())
            .map_err(|e| unavailable(&e))
    }

    fn get(&self, connection_id: &str, name: &str) -> Result<Option<Secret>, CredentialError> {
        match self.entry(connection_id, name)?.get_password() {
            Ok(value) => Ok(Some(Secret::new(value))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(unavailable(&error)),
        }
    }

    fn delete(&self, connection_id: &str, name: &str) -> Result<(), CredentialError> {
        match self.entry(connection_id, name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(unavailable(&error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes and removes one credential in the machine's real store. Ignored: it touches the
    /// user's Keychain (and may ask for permission), so it runs only when asked for:
    /// `cargo test real_keyring -- --ignored`.
    #[test]
    #[ignore = "writes to the OS credential store"]
    fn real_keyring_round_trips_and_forgets_a_secret() {
        let store = KeyringCredentialStore {
            service: "project-atlas.mcp.test".to_owned(),
        };
        let secret = Secret::new("atlas-test-value".to_owned());

        store.put("conn-test", "TOKEN", &secret).unwrap();
        assert_eq!(store.get("conn-test", "TOKEN").unwrap(), Some(secret));
        store.delete("conn-test", "TOKEN").unwrap();
        assert_eq!(store.get("conn-test", "TOKEN").unwrap(), None);
        // Forgetting what is not there is not an error.
        store.delete("conn-test", "TOKEN").unwrap();
    }
}
