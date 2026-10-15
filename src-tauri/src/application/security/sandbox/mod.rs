//! The seam where a real sandbox goes, and the one that exists (ADR 0038).
//!
//! Atlas's permission layer decides *whether* a process may start. It does not confine what the
//! process then does: a command that is allowed runs with the user's own rights, and the tools a
//! runtime's CLI has inside its own process (`Edit`, `Write`, `Bash`…) never pass through Atlas.
//! A [`SandboxProvider`] rewraps the approved request so the *operating system* enforces the
//! policy that was just evaluated, on the CLI and on everything it starts. The guard calls it for
//! every runtime process it lets through.
//!
//! - [`NoSandbox`] changes nothing (Windows, Linux, and the macOS default).
//! - [`MacosSandbox`] runs the CLI under `sandbox-exec` with a profile Atlas generates for the
//!   execution ([`profile`]), from what each CLI needs to touch ([`cli`]).

pub mod cli;
mod macos;
pub mod profile;

use std::path::PathBuf;

pub use macos::MacosSandbox;

use crate::application::process::{ExecutionScope, ProcessSpec};
use crate::domain::security::SecurityPolicy;

/// What the provider is told about the process, besides the request itself.
pub struct SandboxRequest<'a> {
    pub policy: &'a SecurityPolicy,
    pub scope: &'a ExecutionScope,
    /// Where the program is installed, as the process port found it. `None`: it is not installed,
    /// and the port will say so; there is nothing to confine.
    pub program_path: Option<PathBuf>,
}

/// A request ready to start, and what to tell the person about how it will.
pub struct Prepared {
    pub spec: ProcessSpec,
    /// Facts for the audit trail (`sandbox:confined`, `sandbox:unavailable:<why>`…). Empty: the
    /// sandbox is off and has nothing to say.
    pub notes: Vec<String>,
}

/// Why a request could not be prepared. Either way the process does not start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxRefusal {
    /// A sandbox is required and this system, runtime or execution cannot have one.
    Unavailable(String),
    /// The sandbox exists but its profile could not be made.
    Failed(String),
}

pub trait SandboxProvider: Send + Sync {
    /// Prepares an approved request for execution under `request.policy`, for example by wrapping
    /// the program in the OS's sandbox launcher.
    ///
    /// # Errors
    ///
    /// Fails if the sandbox cannot confine the process as it must; the process then does not
    /// start.
    fn prepare(
        &self,
        spec: ProcessSpec,
        request: &SandboxRequest<'_>,
    ) -> Result<Prepared, SandboxRefusal>;
}

/// No OS-level confinement. Policy enforcement only. Production uses [`MacosSandbox`], whose `Off`
/// mode is the same; this one is for tests that have no sandbox to configure.
#[cfg(test)]
pub struct NoSandbox;

#[cfg(test)]
impl SandboxProvider for NoSandbox {
    fn prepare(
        &self,
        spec: ProcessSpec,
        _request: &SandboxRequest<'_>,
    ) -> Result<Prepared, SandboxRefusal> {
        Ok(Prepared {
            spec,
            notes: Vec::new(),
        })
    }
}
