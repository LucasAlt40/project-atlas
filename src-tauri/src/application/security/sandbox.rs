//! The seam where a real sandbox goes.
//!
//! Atlas's permission layer decides *whether* a process may start. It does not (yet) confine what
//! the process then does: a command that is allowed runs with the user's own rights. A stronger
//! boundary — a macOS sandbox profile, Windows `AppContainer`, Linux namespaces, a container or a
//! VM — would be a [`SandboxProvider`] that rewraps the approved request so the OS enforces the
//! policy that was just evaluated. The guard already calls this hook for every process it lets
//! through; the only provider today is [`NoSandbox`], which changes nothing.

use crate::application::process::ProcessSpec;
use crate::domain::security::SecurityPolicy;

pub trait SandboxProvider: Send + Sync {
    /// Prepares an approved request for execution under `policy`, for example by wrapping the
    /// program in the OS's sandbox launcher.
    ///
    /// # Errors
    ///
    /// Fails if the sandbox cannot confine the process as the policy requires; the process
    /// then does not start.
    fn prepare(&self, spec: ProcessSpec, policy: &SecurityPolicy) -> Result<ProcessSpec, String>;
}

/// No OS-level confinement. Policy enforcement only.
pub struct NoSandbox;

impl SandboxProvider for NoSandbox {
    fn prepare(&self, spec: ProcessSpec, _policy: &SecurityPolicy) -> Result<ProcessSpec, String> {
        Ok(spec)
    }
}
