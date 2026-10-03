//! Atlas's security boundary for agent execution. See ADR 0006 for what it does and does not
//! control.
//!
//! The runtime decides *how* to talk to a model; this module decides *what the execution may
//! do*. The pieces:
//!
//! - [`paths`]: "is this path inside the project?", symlink-aware
//! - [`classify`]: what kind of command is this (shell, Git read/write/destructive, network…)
//! - [`evaluator`]: policy + request -> allowed / denied / requires approval
//! - [`service`]: which policy applies to this workspace, agent and runtime
//! - [`approvals`]: the explicit, UI-only way to let a "requires approval" command run
//! - [`audit`]: every decision, per execution
//! - [`guard`]: the `ProcessRunner` wrapper that enforces all of it
//! - [`sandbox`]: the seam for a future OS-level sandbox

pub mod approvals;
pub mod audit;
mod classify;
mod evaluator;
pub mod guard;
mod paths;
pub mod sandbox;
pub mod service;

#[cfg(test)]
mod end_to_end;
#[cfg(all(test, unix))]
mod live_shell;
#[cfg(test)]
pub mod testutil;

pub use approvals::ApprovalBroker;
pub use audit::{AuditLog, PermissionSink};
pub use guard::GuardedProcessRunner;
pub use sandbox::NoSandbox;
pub use service::{PolicyResolver, SecurityOverview, SecurityService};
