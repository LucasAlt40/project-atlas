//! The port for secrets. The domain does not know whether a secret is in the macOS Keychain, the
//! Windows Credential Manager, the Linux Secret Service or a test's memory: it asks for one by
//! (connection, name) and gets a [`Secret`] or nothing.

use crate::domain::mcp::Secret;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialError {
    /// There is no credential store on this machine, or it is locked or refused.
    Unavailable(String),
}

pub trait CredentialStore: Send + Sync {
    /// # Errors
    ///
    /// Fails if the store cannot be reached.
    fn put(&self, connection_id: &str, name: &str, secret: &Secret) -> Result<(), CredentialError>;

    /// `Ok(None)`: nothing is stored under that name.
    ///
    /// # Errors
    ///
    /// Fails if the store cannot be reached.
    fn get(&self, connection_id: &str, name: &str) -> Result<Option<Secret>, CredentialError>;

    /// Forgets it. Forgetting what is not there is not an error.
    ///
    /// # Errors
    ///
    /// Fails if the store cannot be reached.
    fn delete(&self, connection_id: &str, name: &str) -> Result<(), CredentialError>;
}

#[cfg(test)]
pub mod memory {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::{CredentialError, CredentialStore};
    use crate::domain::mcp::Secret;

    /// A store in memory, for tests. `locked` makes every call fail, like a locked keychain.
    #[derive(Default)]
    pub struct MemoryCredentials {
        pub items: Mutex<HashMap<(String, String), String>>,
        pub locked: std::sync::atomic::AtomicBool,
    }

    impl MemoryCredentials {
        fn check(&self) -> Result<(), CredentialError> {
            if self.locked.load(std::sync::atomic::Ordering::SeqCst) {
                Err(CredentialError::Unavailable("locked".to_owned()))
            } else {
                Ok(())
            }
        }
    }

    impl CredentialStore for MemoryCredentials {
        fn put(&self, id: &str, name: &str, secret: &Secret) -> Result<(), CredentialError> {
            self.check()?;
            self.items
                .lock()
                .unwrap()
                .insert((id.to_owned(), name.to_owned()), secret.expose().to_owned());
            Ok(())
        }

        fn get(&self, id: &str, name: &str) -> Result<Option<Secret>, CredentialError> {
            self.check()?;
            Ok(self
                .items
                .lock()
                .unwrap()
                .get(&(id.to_owned(), name.to_owned()))
                .map(|v| Secret::new(v.clone())))
        }

        fn delete(&self, id: &str, name: &str) -> Result<(), CredentialError> {
            self.check()?;
            self.items
                .lock()
                .unwrap()
                .remove(&(id.to_owned(), name.to_owned()));
            Ok(())
        }
    }
}
