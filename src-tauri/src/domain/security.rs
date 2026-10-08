//! The vocabulary of Atlas's security boundary: what a policy can say, what a decision is, and
//! what is recorded about each decision. No behavior lives here beyond combining policies.

use serde::{Deserialize, Serialize};

/// How much of something is granted. Ordered from least to most: combining two policies takes
/// the lower value, so a layer can only ever *restrict* what the layer above granted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    Denied,
    ApprovalRequired,
    Allowed,
}

impl Permission {
    #[must_use]
    pub fn restrict(self, other: Self) -> Self {
        self.min(other)
    }
}

/// Where agents may reach on disk. Only the project folder exists today; the enum is the seam
/// where "project plus these folders" would be added.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilesystemScope {
    ProjectOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilesystemPolicy {
    pub scope: FilesystemScope,
    /// Changing, creating or deleting files. Reading inside the scope is always allowed;
    /// outside it nothing is.
    pub write: Permission,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessPolicy {
    /// `Allowed` runs the commands in `allowed_commands` and asks about every other one;
    /// `ApprovalRequired` asks about every command; `Denied` runs none. Shells are never run.
    pub mode: Permission,
    /// Bare program names (no extension, no path) that `Allowed` mode runs without asking.
    pub allowed_commands: Vec<String>,
}

/// Arbitrary network access requested by an agent. The connection the runtime itself makes to
/// its model provider is Atlas-managed and not governed by this.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkPolicy {
    pub mode: Permission,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPolicy {
    /// status, diff, log, show…
    pub read: Permission,
    /// add, commit, checkout, branch…
    pub write: Permission,
    /// reset --hard, clean, force push, branch deletion…
    pub destructive: Permission,
}

/// Whether an agent may be given MCP tools at all. This is the policy half; the other is a grant
/// (which connections, which tools): both are needed, and nothing is exposed without a grant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpPolicy {
    /// `Allowed`: a grant is enough. `ApprovalRequired`: a person is asked before each step that
    /// would be given MCP tools. `Denied`: none, whatever a grant says.
    pub mode: Permission,
}

fn mcp_not_restricted() -> McpPolicy {
    // Older stored policies have no such setting. They do not restrict it: nothing is exposed
    // without a grant, and the read-only profile denies it.
    McpPolicy {
        mode: Permission::Allowed,
    }
}

/// What may be done in one workspace. A workspace stores one; profiles and runtimes can only
/// narrow it (see [`SecurityPolicy::restrict`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityPolicy {
    pub filesystem: FilesystemPolicy,
    pub processes: ProcessPolicy,
    pub network: NetworkPolicy,
    pub git: GitPolicy,
    #[serde(default = "mcp_not_restricted")]
    pub mcp: McpPolicy,
}

/// Development tools a workspace runs without asking. Bare names: `npm.cmd` and `npm.exe`
/// match `npm`. Every one of them can run arbitrary code (npm scripts, build scripts), which
/// is why the list is short and is the thing to review, not a sandbox.
pub const DEFAULT_DEVELOPMENT_COMMANDS: &[&str] = &[
    "git", "npm", "pnpm", "yarn", "cargo", "dotnet", "java", "mvn", "gradle", "python",
];

impl SecurityPolicy {
    /// The most Atlas will ever grant, whatever a config file or a profile says: arbitrary
    /// network and destructive Git operations are never automatic.
    pub fn global_maximum() -> Self {
        Self {
            filesystem: FilesystemPolicy {
                scope: FilesystemScope::ProjectOnly,
                write: Permission::Allowed,
            },
            processes: ProcessPolicy {
                mode: Permission::Allowed,
                allowed_commands: DEFAULT_DEVELOPMENT_COMMANDS
                    .iter()
                    .map(|c| (*c).to_owned())
                    .collect(),
            },
            network: NetworkPolicy {
                mode: Permission::ApprovalRequired,
            },
            git: GitPolicy {
                read: Permission::Allowed,
                write: Permission::Allowed,
                destructive: Permission::ApprovalRequired,
            },
            mcp: McpPolicy {
                mode: Permission::Allowed,
            },
        }
    }

    /// Nothing but reading the project and Git history.
    pub fn read_only() -> Self {
        Self {
            filesystem: FilesystemPolicy {
                scope: FilesystemScope::ProjectOnly,
                write: Permission::Denied,
            },
            processes: ProcessPolicy {
                mode: Permission::Denied,
                allowed_commands: Vec::new(),
            },
            network: NetworkPolicy {
                mode: Permission::Denied,
            },
            git: GitPolicy {
                read: Permission::Allowed,
                write: Permission::Denied,
                destructive: Permission::Denied,
            },
            // An MCP tool can do what its server can; a read-only agent is not given any.
            mcp: McpPolicy {
                mode: Permission::Denied,
            },
        }
    }

    /// Edit files, run development commands, non-destructive Git; destructive Git asks; no
    /// arbitrary network.
    pub fn developer() -> Self {
        Self {
            network: NetworkPolicy {
                mode: Permission::Denied,
            },
            ..Self::global_maximum()
        }
    }

    /// The combination of two policies: every setting is the lower of the two, and a command
    /// is runnable without asking only if both lists name it. Never grants more than either.
    #[must_use]
    pub fn restrict(&self, other: &Self) -> Self {
        Self {
            filesystem: FilesystemPolicy {
                scope: self.filesystem.scope,
                write: self.filesystem.write.restrict(other.filesystem.write),
            },
            processes: ProcessPolicy {
                mode: self.processes.mode.restrict(other.processes.mode),
                allowed_commands: self
                    .processes
                    .allowed_commands
                    .iter()
                    .filter(|c| other.processes.allowed_commands.contains(c))
                    .cloned()
                    .collect(),
            },
            network: NetworkPolicy {
                mode: self.network.mode.restrict(other.network.mode),
            },
            git: GitPolicy {
                read: self.git.read.restrict(other.git.read),
                write: self.git.write.restrict(other.git.write),
                destructive: self.git.destructive.restrict(other.git.destructive),
            },
            mcp: McpPolicy {
                mode: self.mcp.mode.restrict(other.mcp.mode),
            },
        }
    }
}

/// A new workspace starts with developer-level permissions as its ceiling; each agent's
/// profile narrows it further.
impl Default for SecurityPolicy {
    fn default() -> Self {
        Self::developer()
    }
}

/// A reusable bundle of permissions an agent points to (instead of carrying its own policy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionProfile {
    ReadOnly,
    Developer,
}

impl PermissionProfile {
    pub const ALL: [Self; 2] = [Self::ReadOnly, Self::Developer];

    /// What agents without a (valid) profile get: the most restrictive one.
    pub const DEFAULT: Self = Self::ReadOnly;

    pub fn id(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::Developer => "developer",
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.id() == id)
    }

    /// An unset or unknown id falls back to the most restrictive profile, never to a wider
    /// one.
    pub fn from_id_or_default(id: Option<&str>) -> Self {
        id.and_then(Self::parse).unwrap_or(Self::DEFAULT)
    }

    pub fn policy(self) -> SecurityPolicy {
        match self {
            Self::ReadOnly => SecurityPolicy::read_only(),
            Self::Developer => SecurityPolicy::developer(),
        }
    }
}

/// What a runtime's own tools can do when Atlas launches it the way it does today. These are
/// facts about the external tool, not permissions: a runtime reporting `true` means Atlas
/// cannot stop that tool from doing it (only warn).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct ToolAccess {
    pub filesystem_write: bool,
    pub process_execution: bool,
    /// Arbitrary network access from the runtime's tools (not its provider connection).
    pub network: bool,
}

impl ToolAccess {
    pub const NONE: Self = Self {
        filesystem_write: false,
        process_execution: false,
        network: false,
    };

    /// The runtime layer of the policy: a runtime whose tools cannot do something is a
    /// restriction on what an agent running through it can be granted.
    pub fn ceiling(self) -> SecurityPolicy {
        let level = |open: bool| {
            if open {
                Permission::Allowed
            } else {
                Permission::Denied
            }
        };
        let mut policy = SecurityPolicy::global_maximum();
        policy.filesystem.write = level(self.filesystem_write);
        policy.processes.mode = level(self.process_execution);
        policy.network.mode = level(self.network);
        policy.git.write = level(self.filesystem_write);
        policy.git.destructive = level(self.filesystem_write);
        policy
    }

    /// The capabilities that exceed what `policy` grants, as stable names. Empty when the
    /// runtime stays within it.
    pub fn exceeding(self, policy: &SecurityPolicy) -> Vec<&'static str> {
        let mut names = Vec::new();
        if self.filesystem_write && policy.filesystem.write != Permission::Allowed {
            names.push("filesystem_write");
        }
        if self.process_execution && policy.processes.mode != Permission::Allowed {
            names.push("process_execution");
        }
        if self.network && policy.network.mode != Permission::Allowed {
            names.push("network");
        }
        names
    }
}

/// The answer to "may this happen?".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionDecision {
    Allowed,
    Denied,
    RequiresApproval,
}

/// Why a decision was not a plain "allowed". Stable codes: the UI words each one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    UnknownScope,
    ProjectUnavailable,
    ShellNotPermitted,
    LauncherNotPermitted,
    EnvironmentOverride,
    NoWorkingDirectory,
    OutsideProject,
    ProcessesDenied,
    NotInAllowedList,
    ApprovalPolicy,
    ProgramPath,
    InlineCode,
    GitRead,
    GitWrite,
    GitDestructive,
    GitConfigOverride,
    NetworkAccess,
    FilesystemWrite,
    UnknownProgram,
    SandboxRefused,
    ApprovalRejected,
    ApprovalTimeout,
    /// The context to be sent has a blocking problem.
    ContextInvalid,
    /// The context to be sent has a problem a person should look at.
    ContextNeedsReview,
    /// Something that looked like a secret was taken out of context before it was sent.
    SecretsRedacted,
    /// Edits were not granted: the agent does not work in an isolated worktree.
    WriteNotIsolated,
    /// Edits were not granted: the runtime cannot be launched with file-editing tools.
    RuntimeCannotEdit,
    /// The runtime listed an MCP tool nobody authorized for this step.
    McpToolNotAuthorized,
    /// A required MCP connection could not be given to the step.
    McpConnectionUnavailable,
}

impl Reason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::McpToolNotAuthorized => "mcp_tool_not_authorized",
            Self::McpConnectionUnavailable => "mcp_connection_unavailable",
            Self::UnknownScope => "unknown_scope",
            Self::ProjectUnavailable => "project_unavailable",
            Self::ShellNotPermitted => "shell_not_permitted",
            Self::LauncherNotPermitted => "launcher_not_permitted",
            Self::EnvironmentOverride => "environment_override",
            Self::NoWorkingDirectory => "no_working_directory",
            Self::OutsideProject => "outside_project",
            Self::ProcessesDenied => "processes_denied",
            Self::NotInAllowedList => "not_in_allowed_list",
            Self::ApprovalPolicy => "approval_policy",
            Self::ProgramPath => "program_path",
            Self::InlineCode => "inline_code",
            Self::GitRead => "git_read",
            Self::GitWrite => "git_write",
            Self::GitDestructive => "git_destructive",
            Self::GitConfigOverride => "git_config_override",
            Self::NetworkAccess => "network_access",
            Self::FilesystemWrite => "filesystem_write",
            Self::UnknownProgram => "unknown_program",
            Self::SandboxRefused => "sandbox_refused",
            Self::ApprovalRejected => "approval_rejected",
            Self::ApprovalTimeout => "approval_timeout",
            Self::ContextInvalid => "context_invalid",
            Self::ContextNeedsReview => "context_needs_review",
            Self::SecretsRedacted => "secrets_redacted",
            Self::WriteNotIsolated => "write_not_isolated",
            Self::RuntimeCannotEdit => "runtime_cannot_edit",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionAction {
    /// Atlas starts the runtime's CLI for an execution.
    LaunchRuntime,
    /// A command an agent asked Atlas to run.
    RunProcess,
    /// Atlas weighed the context an agent is about to receive before starting it.
    ReviewContext,
    /// Atlas decided whether an execution may be given file-editing tools.
    EditFiles,
    /// Atlas weighed what a step hands to the next one.
    ShareResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionOutcome {
    Allowed,
    Denied,
    ApprovalRequested,
    Approved,
    Rejected,
    /// Allowed, changed by a deterministic rule (secrets redacted).
    Transformed,
}

/// Who decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionSource {
    /// Atlas's own policy.
    Policy,
    /// The user, through the approval UI.
    User,
    /// Nobody answered in time.
    Timeout,
}

/// One entry of an execution's security audit trail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionEvent {
    /// Milliseconds since the Unix epoch.
    pub timestamp: u64,
    pub execution_id: String,
    pub workspace_id: String,
    pub task_id: String,
    pub agent_id: String,
    pub action: PermissionAction,
    /// The program and arguments, for display. Never executed from this text.
    pub target: String,
    pub cwd: Option<String>,
    pub decision: PermissionOutcome,
    pub source: DecisionSource,
    pub reason: Option<Reason>,
    /// Links `approval_requested` with its `approved` / `rejected`.
    pub approval_id: Option<String>,
    /// Facts worth keeping next to the decision (for example capabilities Atlas could not
    /// restrict).
    pub notes: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restricting_never_grants_more_than_either_side() {
        let max = SecurityPolicy::global_maximum();
        let read_only = SecurityPolicy::read_only();

        assert_eq!(max.restrict(&read_only), read_only);
        assert_eq!(read_only.restrict(&max), read_only);
        let dev = SecurityPolicy::developer();
        assert_eq!(dev.restrict(&max), dev);
    }

    #[test]
    fn allowed_commands_are_intersected() {
        let mut a = SecurityPolicy::developer();
        a.processes.allowed_commands = vec!["git".into(), "npm".into()];
        let mut b = SecurityPolicy::developer();
        b.processes.allowed_commands = vec!["npm".into(), "curl".into()];

        assert_eq!(a.restrict(&b).processes.allowed_commands, ["npm"]);
    }

    #[test]
    fn unknown_profiles_fall_back_to_read_only() {
        assert_eq!(
            PermissionProfile::from_id_or_default(None),
            PermissionProfile::ReadOnly
        );
        assert_eq!(
            PermissionProfile::from_id_or_default(Some("root")),
            PermissionProfile::ReadOnly
        );
        assert_eq!(
            PermissionProfile::from_id_or_default(Some("developer")),
            PermissionProfile::Developer
        );
    }

    #[test]
    fn a_tool_that_cannot_write_or_run_commands_caps_the_policy() {
        let ceiling = ToolAccess::NONE.ceiling();

        assert_eq!(ceiling.filesystem.write, Permission::Denied);
        assert_eq!(ceiling.processes.mode, Permission::Denied);
        assert_eq!(ceiling.git.write, Permission::Denied);
        assert_eq!(ceiling.git.read, Permission::Allowed);
    }

    #[test]
    fn reports_what_a_runtime_can_do_beyond_the_policy() {
        let open = ToolAccess {
            filesystem_write: true,
            process_execution: true,
            network: true,
        };

        assert_eq!(
            open.exceeding(&SecurityPolicy::read_only()),
            ["filesystem_write", "process_execution", "network"]
        );
        assert_eq!(
            ToolAccess::NONE.exceeding(&SecurityPolicy::read_only()),
            Vec::<&str>::new()
        );
    }
}
