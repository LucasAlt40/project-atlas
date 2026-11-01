//! Which commands an execution may run, when its runtime can be given a short list of them alone
//! (`ModelRuntime::runs_granted_commands`): the project's test commands, and Git's reading
//! commands so a reviewer can see what changed. A pure function of the agent's policy: the policy
//! stays the one authority, this only reads it.
//!
//! Running a test runs the project's code, which the agent may have just edited, so the list is
//! short and each command still needs its program allowed by the policy (`npm` for `npm test`),
//! the same names that run without asking everywhere else; the Git commands also need the
//! policy's `git.read`. The runtime runs them inside a sandbox of its own (writes in the working
//! directory only, none at all for a run that reads, no network), which is what bounds them; the
//! list is what keeps the agent to them.

use crate::domain::security::{Permission, SecurityPolicy};

/// The test commands Atlas knows, as typed: the program first. Arguments after them are allowed
/// (`npm test -- --grep x`), nothing before them.
pub const TEST_COMMANDS: &[&str] = &[
    "npm test",
    "npm run test",
    "pnpm test",
    "pnpm run test",
    "yarn test",
    "yarn run test",
    "node --test",
    "cargo test",
    "dotnet test",
    "mvn test",
    "gradle test",
    "python -m pytest",
    "python3 -m pytest",
    "pytest",
    "go test",
];

/// Git's commands that only read. A validator that tried `git status` to check the scope of a change
/// was refused before they were here (`bench/`, 2026-10-31d).
pub const GIT_READ_COMMANDS: &[&str] = &["git status", "git diff", "git log", "git show"];

/// The commands of [`TEST_COMMANDS`] and [`GIT_READ_COMMANDS`] this policy lets an agent run: none
/// unless processes are `Allowed`, and then only those whose program is in its
/// `allowed_commands`; the Git ones also need `git.read` to be `Allowed`.
pub fn granted(policy: &SecurityPolicy) -> Vec<String> {
    if policy.processes.mode != Permission::Allowed {
        return Vec::new();
    }
    let runs = |command: &&&str| {
        let program = command.split(' ').next().unwrap_or_default();
        policy
            .processes
            .allowed_commands
            .iter()
            .any(|allowed| allowed == program)
    };
    let git = if policy.git.read == Permission::Allowed {
        GIT_READ_COMMANDS
    } else {
        &[]
    };
    TEST_COMMANDS
        .iter()
        .chain(git)
        .filter(runs)
        .map(|command| (*command).to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_policy_grants_the_tests_of_the_tools_it_already_allows() {
        let granted = granted(&SecurityPolicy::global_maximum());
        assert!(granted.contains(&"npm test".to_owned()));
        assert!(granted.contains(&"cargo test".to_owned()));
        assert!(granted.contains(&"python -m pytest".to_owned()));
        // `node`, `pytest` and `go` are not among the default development commands.
        assert!(!granted.contains(&"node --test".to_owned()));
        assert!(!granted.contains(&"pytest".to_owned()));
        assert!(!granted.contains(&"go test".to_owned()));
    }

    #[test]
    fn a_policy_that_does_not_run_processes_grants_none() {
        let mut policy = SecurityPolicy::global_maximum();
        policy.processes.mode = Permission::ApprovalRequired;
        assert_eq!(granted(&policy), Vec::<String>::new());
        policy.processes.mode = Permission::Denied;
        assert_eq!(granted(&policy), Vec::<String>::new());
        assert_eq!(granted(&SecurityPolicy::read_only()), Vec::<String>::new());
    }

    #[test]
    fn git_reading_commands_come_with_the_policy_that_reads_git() {
        let mut policy = SecurityPolicy::global_maximum();
        let granted_now = granted(&policy);
        for command in GIT_READ_COMMANDS {
            assert!(granted_now.contains(&(*command).to_owned()), "{command}");
        }
        // Nothing that writes to Git is ever among them.
        assert!(!granted_now
            .iter()
            .any(|c| c.contains("commit") || c.contains("push")));

        policy.git.read = Permission::ApprovalRequired;
        assert!(!granted(&policy).iter().any(|c| c.starts_with("git ")));
        policy.git.read = Permission::Allowed;
        policy.processes.allowed_commands.retain(|c| c != "git");
        assert!(!granted(&policy).iter().any(|c| c.starts_with("git ")));
    }

    #[test]
    fn only_the_programs_the_policy_names_count() {
        let mut policy = SecurityPolicy::global_maximum();
        policy.processes.allowed_commands = vec!["cargo".to_owned()];
        assert_eq!(granted(&policy), ["cargo test"]);
        policy.processes.allowed_commands.clear();
        assert_eq!(granted(&policy), Vec::<String>::new());
    }
}
