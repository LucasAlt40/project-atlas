//! The decision function: given the effective policy, the project folder and a structured
//! process request, may it run? Pure apart from reading the file system to resolve paths; it
//! starts nothing and asks nobody.

use std::path::Path;

use super::classify::{
    classify_git, has_path, is_dangerous_env, is_launcher, is_network_tool, is_shell, program_stem,
    runs_inline_code, GitClass,
};
use super::paths::is_inside_physical;
use crate::application::process::ProcessSpec;
use crate::domain::security::{Permission, PermissionDecision, Reason, SecurityPolicy};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Evaluation {
    pub decision: PermissionDecision,
    /// Why the decision is not a plain "allowed".
    pub reason: Option<Reason>,
}

impl Evaluation {
    const ALLOWED: Self = Self {
        decision: PermissionDecision::Allowed,
        reason: None,
    };

    pub fn denied(reason: Reason) -> Self {
        Self {
            decision: PermissionDecision::Denied,
            reason: Some(reason),
        }
    }

    fn from_needs(needs: &[(Permission, Reason)]) -> Self {
        // The most restrictive requirement decides; the first one at that level explains it.
        let Some(lowest) = needs.iter().map(|(p, _)| *p).min() else {
            return Self::ALLOWED;
        };
        let reason = needs.iter().find(|(p, _)| *p == lowest).map(|(_, r)| *r);
        match lowest {
            Permission::Allowed => Self::ALLOWED,
            Permission::ApprovalRequired => Self {
                decision: PermissionDecision::RequiresApproval,
                reason,
            },
            Permission::Denied => Self {
                decision: PermissionDecision::Denied,
                reason,
            },
        }
    }
}

/// Checks that apply to every process, whoever asks: no shell, no launcher, no environment
/// that rewires the program, and a working directory inside the project.
fn check_common(root: &Path, spec: &ProcessSpec, require_cwd: bool) -> Option<Evaluation> {
    // Without a project folder there is no boundary to hold anyone to.
    if !root.is_dir() {
        return Some(Evaluation::denied(Reason::ProjectUnavailable));
    }
    let stem = program_stem(&spec.program);
    if is_shell(&stem) {
        return Some(Evaluation::denied(Reason::ShellNotPermitted));
    }
    if is_launcher(&stem) {
        return Some(Evaluation::denied(Reason::LauncherNotPermitted));
    }
    if spec.env.iter().any(|(key, _)| is_dangerous_env(key)) {
        return Some(Evaluation::denied(Reason::EnvironmentOverride));
    }
    match &spec.cwd {
        Some(cwd) => {
            if let Some(denied) = check_inside(root, cwd, root) {
                return Some(denied);
            }
        }
        None if require_cwd => return Some(Evaluation::denied(Reason::NoWorkingDirectory)),
        None => {}
    }
    None
}

/// `None` when `candidate` (relative to `base`) is physically inside the project.
fn check_inside(root: &Path, candidate: &Path, base: &Path) -> Option<Evaluation> {
    match is_inside_physical(root, candidate, base) {
        Ok(true) => None,
        // Unresolvable (dangling or looping link, unreadable) counts as outside: fail closed.
        Ok(false) | Err(_) => Some(Evaluation::denied(Reason::OutsideProject)),
    }
}

/// The part of an argument that may name a place, if any. Heuristic by nature: Atlas does not
/// know every tool's options, so it looks at what *looks like* a path and refuses what leaves
/// the project. A tool that builds paths from other data is outside its reach (ADR 0006).
fn path_in_argument(arg: &str) -> Option<&str> {
    let value = if arg.starts_with('-') {
        arg.split_once('=')?.1
    } else {
        arg
    };
    let bytes = value.as_bytes();
    let drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    (!value.is_empty()
        && (value.contains('/')
            || value.contains('\\')
            || value.starts_with('.')
            || value.starts_with('~')
            || value.starts_with("file:")
            || drive))
        .then_some(value)
}

fn check_arguments(root: &Path, spec: &ProcessSpec, extra: &[String]) -> Option<Evaluation> {
    let cwd = spec.cwd.as_deref().unwrap_or(root);
    for value in spec
        .args
        .iter()
        .filter_map(|a| path_in_argument(a))
        .chain(extra.iter().map(String::as_str))
    {
        // `~` is expanded by the tool, not by a shell here: it means the home folder.
        if value.starts_with('~') || value.starts_with("file:") {
            return Some(Evaluation::denied(Reason::OutsideProject));
        }
        if let Some(denied) = check_inside(root, Path::new(value), cwd) {
            return Some(denied);
        }
    }
    None
}

/// Starting a runtime's CLI for an execution. The program must be one of the runtimes Atlas
/// knows, the working directory the project; what the runtime then does inside its own process
/// is its tool's business (reported, not enforced: see `ToolAccess`).
pub fn evaluate_runtime_launch(
    root: &Path,
    spec: &ProcessSpec,
    known_programs: &[String],
) -> Evaluation {
    if let Some(denied) = check_common(root, spec, true) {
        return denied;
    }
    if has_path(&spec.program) || !known_programs.contains(&program_stem(&spec.program)) {
        return Evaluation::denied(Reason::UnknownProgram);
    }
    Evaluation::ALLOWED
}

/// A query to a runtime's tool: only a known program, nowhere in particular.
pub fn evaluate_probe(spec: &ProcessSpec, known_programs: &[String]) -> Evaluation {
    let stem = program_stem(&spec.program);
    if is_shell(&stem) {
        return Evaluation::denied(Reason::ShellNotPermitted);
    }
    if spec.cwd.is_some()
        || has_path(&spec.program)
        || !known_programs.contains(&stem)
        || spec.env.iter().any(|(key, _)| is_dangerous_env(key))
    {
        return Evaluation::denied(Reason::UnknownProgram);
    }
    Evaluation::ALLOWED
}

/// A command an agent asked Atlas to run.
pub fn evaluate_agent_request(
    policy: &SecurityPolicy,
    root: &Path,
    spec: &ProcessSpec,
) -> Evaluation {
    if let Some(denied) = check_common(root, spec, true) {
        return denied;
    }
    let stem = program_stem(&spec.program);
    let bare = !has_path(&spec.program);
    let mut needs: Vec<(Permission, Reason)> = Vec::new();

    let git = (bare && stem == "git").then(|| classify_git(&spec.args));
    if let Some(operation) = &git {
        if let Some(denied) = check_arguments(root, spec, &operation.directories) {
            return denied;
        }
        match operation.class {
            GitClass::ConfigOverride => return Evaluation::denied(Reason::GitConfigOverride),
            GitClass::Read => needs.push((policy.git.read, Reason::GitRead)),
            GitClass::Write => {
                needs.push((policy.git.write, Reason::GitWrite));
                needs.push((policy.filesystem.write, Reason::FilesystemWrite));
            }
            GitClass::Destructive => {
                needs.push((policy.git.destructive, Reason::GitDestructive));
                needs.push((policy.filesystem.write, Reason::FilesystemWrite));
            }
        }
        if operation.network {
            needs.push((policy.network.mode, Reason::NetworkAccess));
        }
    } else {
        if let Some(denied) = check_arguments(root, spec, &[]) {
            return denied;
        }
        needs.push(process_need(policy, &stem, bare));
        // Any program may change files.
        needs.push((policy.filesystem.write, Reason::FilesystemWrite));
        if is_network_tool(&stem) {
            needs.push((policy.network.mode, Reason::NetworkAccess));
        }
        if runs_inline_code(&stem, &spec.args) {
            needs.push((Permission::ApprovalRequired, Reason::InlineCode));
        }
    }
    Evaluation::from_needs(&needs)
}

/// What the process policy alone says about running this program.
fn process_need(policy: &SecurityPolicy, stem: &str, bare: bool) -> (Permission, Reason) {
    match policy.processes.mode {
        Permission::Denied => (Permission::Denied, Reason::ProcessesDenied),
        Permission::ApprovalRequired => (Permission::ApprovalRequired, Reason::ApprovalPolicy),
        Permission::Allowed if !bare => (Permission::ApprovalRequired, Reason::ProgramPath),
        Permission::Allowed if policy.processes.allowed_commands.iter().any(|c| c == stem) => {
            (Permission::Allowed, Reason::NotInAllowedList)
        }
        Permission::Allowed => (Permission::ApprovalRequired, Reason::NotInAllowedList),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;
    use crate::application::process::{ExecutionScope, ProcessContext};
    use crate::application::security::testutil::TempDir;
    use crate::domain::security::PermissionProfile;
    use std::time::Duration;

    struct Project {
        _dir: TempDir,
        root: PathBuf,
    }

    fn project() -> Project {
        let dir = TempDir::new("eval");
        let root = dir.path().join("project");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(dir.path().join("other")).unwrap();
        Project { _dir: dir, root }
    }

    fn request(root: &Path, program: &str, args: &[&str]) -> ProcessSpec {
        ProcessSpec {
            program: program.to_owned(),
            args: args.iter().map(|a| (*a).to_owned()).collect(),
            stdin: None,
            cwd: Some(root.to_path_buf()),
            env: Vec::new(),
            timeout: Duration::from_secs(1),
            context: ProcessContext::AgentRequested(ExecutionScope::for_tests()),
            terminal: None,
        }
    }

    fn eval(policy: &SecurityPolicy, p: &Project, program: &str, args: &[&str]) -> Evaluation {
        evaluate_agent_request(policy, &p.root, &request(&p.root, program, args))
    }

    fn decision(
        policy: &SecurityPolicy,
        p: &Project,
        program: &str,
        args: &[&str],
    ) -> PermissionDecision {
        eval(policy, p, program, args).decision
    }

    fn developer() -> SecurityPolicy {
        PermissionProfile::Developer.policy()
    }

    #[test]
    fn an_mcp_launch_and_probe_of_a_runtime_pass_the_guard() {
        let p = project();
        let known = vec!["claude".to_owned()];
        // What the Claude adapter hands over: a config file outside the project and the
        // references' values in the environment.
        let env = vec![("ATLAS_MCP_FILES_TOKEN".to_owned(), "x".to_owned())];
        let args = [
            "--mcp-config",
            "/var/folders/xx/atlas-mcp-1.json",
            "--strict-mcp-config",
        ];
        let launch = ProcessSpec {
            context: ProcessContext::Runtime(ExecutionScope::for_tests()),
            env: env.clone(),
            ..request(&p.root, "claude", &args)
        };
        let probe = ProcessSpec {
            context: ProcessContext::Probe,
            cwd: None,
            env,
            ..request(&p.root, "claude", &args)
        };

        assert_eq!(
            evaluate_runtime_launch(&p.root, &launch, &known),
            Evaluation::ALLOWED
        );
        assert_eq!(evaluate_probe(&probe, &known), Evaluation::ALLOWED);
    }

    #[test]
    fn listed_development_commands_run_in_the_project() {
        let p = project();
        for (program, args) in [
            ("npm", &["test"][..]),
            ("cargo", &["build", "--release"]),
            ("pnpm", &["install"]),
            ("git", &["status"]),
            ("NPM.CMD", &["test"]),
            ("python", &["script.py"]),
        ] {
            assert_eq!(
                decision(&developer(), &p, program, args),
                PermissionDecision::Allowed,
                "{program} {args:?}"
            );
        }
    }

    #[test]
    fn a_command_that_is_not_listed_asks() {
        let p = project();
        let result = eval(&developer(), &p, "make", &["all"]);

        assert_eq!(result.decision, PermissionDecision::RequiresApproval);
        assert_eq!(result.reason, Some(Reason::NotInAllowedList));
    }

    #[test]
    fn a_program_named_by_path_never_runs_unasked_even_when_the_name_is_listed() {
        let p = project();
        for program in ["./npm", "/tmp/npm", "src/cargo", r"C:\x\npm.exe"] {
            let result = eval(&developer(), &p, program, &["test"]);
            assert_eq!(
                result.decision,
                PermissionDecision::RequiresApproval,
                "{program}"
            );
            assert_eq!(result.reason, Some(Reason::ProgramPath), "{program}");
        }
    }

    #[test]
    fn approval_required_mode_asks_for_every_command() {
        let p = project();
        let mut policy = developer();
        policy.processes.mode = Permission::ApprovalRequired;

        assert_eq!(
            decision(&policy, &p, "npm", &["test"]),
            PermissionDecision::RequiresApproval
        );
    }

    #[test]
    fn denied_mode_runs_nothing_but_still_allows_reading_git() {
        let p = project();
        let policy = PermissionProfile::ReadOnly.policy();

        assert_eq!(
            decision(&policy, &p, "npm", &["test"]),
            PermissionDecision::Denied
        );
        assert_eq!(
            decision(&policy, &p, "cargo", &["build"]),
            PermissionDecision::Denied
        );
        assert_eq!(
            decision(&policy, &p, "git", &["status"]),
            PermissionDecision::Allowed
        );
        assert_eq!(
            decision(&policy, &p, "git", &["diff"]),
            PermissionDecision::Allowed
        );
        assert_eq!(
            decision(&policy, &p, "git", &["log", "-3"]),
            PermissionDecision::Allowed
        );
        assert_eq!(
            decision(&policy, &p, "git", &["commit", "-m", "x"]),
            PermissionDecision::Denied
        );
        assert_eq!(
            decision(&policy, &p, "git", &["add", "."]),
            PermissionDecision::Denied
        );
    }

    #[test]
    fn shells_are_never_run_whatever_the_policy() {
        let p = project();
        let mut permissive = SecurityPolicy::global_maximum();
        permissive
            .processes
            .allowed_commands
            .extend(["sh".into(), "bash".into(), "cmd".into()]);
        for (program, args) in [
            ("sh", &["-c", "rm -rf ."][..]),
            ("bash", &["-c", "x"]),
            ("/bin/zsh", &["-c", "x"]),
            ("cmd", &["/c", "dir"]),
            ("cmd.exe", &["/c", "dir"]),
            ("powershell", &["-Command", "x"]),
            ("pwsh.exe", &["-c", "x"]),
        ] {
            let result = eval(&permissive, &p, program, args);
            assert_eq!(result.decision, PermissionDecision::Denied, "{program}");
            assert_eq!(result.reason, Some(Reason::ShellNotPermitted), "{program}");
        }
    }

    #[test]
    fn programs_that_launch_other_programs_are_refused() {
        let p = project();
        for program in ["env", "xargs", "sudo", "start", "find", "nohup"] {
            let result = eval(&developer(), &p, program, &["sh"]);
            assert_eq!(result.decision, PermissionDecision::Denied, "{program}");
            assert_eq!(result.reason, Some(Reason::LauncherNotPermitted));
        }
    }

    #[test]
    fn injection_text_in_arguments_is_just_an_argument() {
        let p = project();
        // Nothing here is interpreted: the request is data. The decision depends on the
        // program and on arguments that name places, not on shell metacharacters.
        for hostile in [
            "; rm -rf ..",
            "& del *",
            "$(rm -rf /)",
            "`id`",
            "a | b",
            "x && y",
            "> out",
        ] {
            let result = eval(&developer(), &p, "npm", &["test", hostile]);
            assert_eq!(result.decision, PermissionDecision::Allowed, "{hostile}");
        }
        // And a "program" that smuggles arguments into its name is simply not a listed one.
        let result = eval(&developer(), &p, "npm test; rm -rf .", &[]);
        assert_ne!(result.decision, PermissionDecision::Allowed);
    }

    #[test]
    fn the_working_directory_must_be_inside_the_project() {
        let p = project();
        let other = p.root.parent().unwrap().join("other");
        let mut spec = request(&p.root, "npm", &["test"]);
        spec.cwd = Some(other);

        let result = evaluate_agent_request(&developer(), &p.root, &spec);
        assert_eq!(result.reason, Some(Reason::OutsideProject));
        assert_eq!(result.decision, PermissionDecision::Denied);

        spec.cwd = Some(p.root.join("src"));
        assert_eq!(
            evaluate_agent_request(&developer(), &p.root, &spec).decision,
            PermissionDecision::Allowed
        );

        spec.cwd = Some(p.root.join("src/not-yet-created"));
        assert_eq!(
            evaluate_agent_request(&developer(), &p.root, &spec).decision,
            PermissionDecision::Allowed
        );

        spec.cwd = None;
        assert_eq!(
            evaluate_agent_request(&developer(), &p.root, &spec).reason,
            Some(Reason::NoWorkingDirectory)
        );
    }

    #[test]
    fn arguments_that_name_places_outside_the_project_are_refused() {
        let p = project();
        let sibling = p.root.parent().unwrap().join("other");
        let sibling = sibling.to_string_lossy().into_owned();
        for args in [
            vec!["run", "/etc/passwd"],
            vec!["run", "../other/x"],
            vec!["run", "src/../../other"],
            vec!["test", "--out-dir=/tmp/x"],
            vec!["test", "--config=../../secret"],
            vec!["install", "~/.ssh/id_rsa"],
            vec!["install", "file:///etc/hosts"],
            vec!["x", sibling.as_str()],
        ] {
            let result = eval(&developer(), &p, "npm", &args);
            assert_eq!(result.decision, PermissionDecision::Denied, "{args:?}");
            assert_eq!(result.reason, Some(Reason::OutsideProject), "{args:?}");
        }
        // Places inside are fine, whether or not they exist.
        for args in [
            vec!["run", "src/main.rs"],
            vec!["run", "./src/../src/lib.rs"],
            vec!["test", "--out-dir=target/new"],
            vec!["test", "--grep", "a/b"],
        ] {
            assert_eq!(
                decision(&developer(), &p, "npm", &args),
                PermissionDecision::Allowed,
                "{args:?}"
            );
        }
    }

    #[test]
    fn git_cannot_be_pointed_at_another_directory() {
        let p = project();
        let result = eval(&developer(), &p, "git", &["-C", "../other", "status"]);
        assert_eq!(result.reason, Some(Reason::OutsideProject));
        assert_eq!(
            decision(&developer(), &p, "git", &["-C", "src", "status"]),
            PermissionDecision::Allowed
        );
    }

    #[test]
    fn git_config_overrides_are_denied() {
        let p = project();
        let result = eval(
            &developer(),
            &p,
            "git",
            &["-c", "core.sshCommand=x", "fetch"],
        );
        assert_eq!(result.decision, PermissionDecision::Denied);
        assert_eq!(result.reason, Some(Reason::GitConfigOverride));
    }

    #[test]
    fn destructive_git_asks_under_developer_and_is_denied_under_read_only() {
        let p = project();
        for args in [
            &["reset", "--hard"][..],
            &["clean", "-fd"],
            &["branch", "-D", "x"],
            &["checkout", "--", "."],
        ] {
            let result = eval(&developer(), &p, "git", args);
            assert_eq!(
                result.decision,
                PermissionDecision::RequiresApproval,
                "{args:?}"
            );
            assert_eq!(result.reason, Some(Reason::GitDestructive));
            assert_eq!(
                decision(&PermissionProfile::ReadOnly.policy(), &p, "git", args),
                PermissionDecision::Denied
            );
        }
    }

    #[test]
    fn a_force_push_is_both_destructive_and_network_so_the_stricter_rule_wins() {
        let p = project();
        let push = ["push", "--force", "origin", "main"];

        // Developer: network is off.
        let result = eval(&developer(), &p, "git", &push);
        assert_eq!(result.decision, PermissionDecision::Denied);
        assert_eq!(result.reason, Some(Reason::NetworkAccess));
        // Network on request, destructive on request: asks.
        let mut asking = developer();
        asking.network.mode = Permission::ApprovalRequired;
        assert_eq!(
            decision(&asking, &p, "git", &push),
            PermissionDecision::RequiresApproval
        );
        // Destructive denied beats network on request.
        asking.git.destructive = Permission::Denied;
        assert_eq!(
            decision(&asking, &p, "git", &push),
            PermissionDecision::Denied
        );
    }

    #[test]
    fn git_writes_are_allowed_for_developers() {
        let p = project();
        for args in [
            &["add", "."][..],
            &["commit", "-m", "x"],
            &["checkout", "-b", "f"],
        ] {
            assert_eq!(
                decision(&developer(), &p, "git", args),
                PermissionDecision::Allowed,
                "{args:?}"
            );
        }
    }

    #[test]
    fn network_is_denied_by_default_and_remote_git_follows_it() {
        let p = project();
        for (program, args) in [
            ("curl", &["https://example.com"][..]),
            ("wget", &["https://example.com/x"]),
            ("ssh", &["host"]),
            ("git", &["push", "origin", "main"]),
            ("git", &["fetch"]),
            ("git", &["clone", "https://example.com/r.git"]),
        ] {
            assert_eq!(
                decision(&developer(), &p, program, args),
                PermissionDecision::Denied,
                "{program} {args:?}"
            );
        }
        // Local git is untouched by the network policy.
        assert_eq!(
            decision(&developer(), &p, "git", &["commit", "-m", "x"]),
            PermissionDecision::Allowed
        );

        let mut asking = developer();
        asking.network.mode = Permission::ApprovalRequired;
        assert_eq!(
            decision(&asking, &p, "git", &["push", "origin", "main"]),
            PermissionDecision::RequiresApproval
        );
    }

    #[test]
    fn inline_code_asks_even_for_an_allowed_interpreter() {
        let p = project();
        let result = eval(&developer(), &p, "python", &["-c", "import os"]);
        assert_eq!(result.decision, PermissionDecision::RequiresApproval);
        assert_eq!(result.reason, Some(Reason::InlineCode));
    }

    #[test]
    fn environment_overrides_that_rewire_programs_are_denied() {
        let p = project();
        for key in [
            "LD_PRELOAD",
            "DYLD_INSERT_LIBRARIES",
            "PATH",
            "GIT_SSH_COMMAND",
            "NODE_OPTIONS",
        ] {
            let mut spec = request(&p.root, "npm", &["test"]);
            spec.env = vec![(key.to_owned(), "x".to_owned())];
            let result = evaluate_agent_request(&developer(), &p.root, &spec);
            assert_eq!(result.reason, Some(Reason::EnvironmentOverride), "{key}");
        }
        let mut spec = request(&p.root, "npm", &["test"]);
        spec.env = vec![("CI".to_owned(), "1".to_owned())];
        assert_eq!(
            evaluate_agent_request(&developer(), &p.root, &spec).decision,
            PermissionDecision::Allowed
        );
    }

    #[test]
    fn a_policy_that_denies_writing_files_stops_commands_that_could_write() {
        let p = project();
        let mut policy = developer();
        policy.filesystem.write = Permission::Denied;

        assert_eq!(
            decision(&policy, &p, "npm", &["test"]),
            PermissionDecision::Denied
        );
        assert_eq!(
            decision(&policy, &p, "git", &["commit", "-m", "x"]),
            PermissionDecision::Denied
        );
        assert_eq!(
            decision(&policy, &p, "git", &["status"]),
            PermissionDecision::Allowed
        );
    }

    #[test]
    fn runtime_launches_need_a_known_program_and_the_project_as_working_directory() {
        let p = project();
        let known = vec!["claude".to_owned(), "opencode".to_owned()];
        let mut spec = request(&p.root, "claude", &["-p"]);
        spec.context = ProcessContext::Runtime(ExecutionScope::for_tests());

        assert_eq!(
            evaluate_runtime_launch(&p.root, &spec, &known).decision,
            PermissionDecision::Allowed
        );

        for program in ["curl", "./claude", "/tmp/claude"] {
            let mut bad = spec.clone();
            bad.program = program.to_owned();
            assert_eq!(
                evaluate_runtime_launch(&p.root, &bad, &known).reason,
                Some(Reason::UnknownProgram),
                "{program}"
            );
        }
        let mut shell = spec.clone();
        shell.program = "bash".to_owned();
        assert_eq!(
            evaluate_runtime_launch(&p.root, &shell, &known).reason,
            Some(Reason::ShellNotPermitted)
        );
        let mut elsewhere = spec.clone();
        elsewhere.cwd = Some(p.root.parent().unwrap().to_path_buf());
        assert_eq!(
            evaluate_runtime_launch(&p.root, &elsewhere, &known).reason,
            Some(Reason::OutsideProject)
        );
        let mut nowhere = spec;
        nowhere.cwd = None;
        assert_eq!(
            evaluate_runtime_launch(&p.root, &nowhere, &known).reason,
            Some(Reason::NoWorkingDirectory)
        );
    }

    #[test]
    fn nothing_runs_when_the_project_folder_is_gone() {
        let p = project();
        fs::remove_dir_all(&p.root).unwrap();

        assert_eq!(
            eval(&developer(), &p, "npm", &["test"]).reason,
            Some(Reason::ProjectUnavailable)
        );
    }

    #[test]
    fn probes_are_limited_to_known_programs_without_a_working_directory() {
        let known = vec!["claude".to_owned()];
        let probe = ProcessSpec::probe("claude", &["--version"], Duration::from_secs(1));

        assert_eq!(
            evaluate_probe(&probe, &known).decision,
            PermissionDecision::Allowed
        );
        for program in ["curl", "sh", "./claude"] {
            let mut bad = probe.clone();
            bad.program = program.to_owned();
            assert_eq!(
                evaluate_probe(&bad, &known).decision,
                PermissionDecision::Denied,
                "{program}"
            );
        }
        let mut with_cwd = probe;
        with_cwd.cwd = Some(PathBuf::from("/"));
        assert_eq!(
            evaluate_probe(&with_cwd, &known).decision,
            PermissionDecision::Denied
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_out_of_the_project_cannot_be_used_as_working_directory_or_argument() {
        let p = project();
        let outside = p.root.parent().unwrap().join("other");
        std::os::unix::fs::symlink(&outside, p.root.join("escape")).unwrap();

        let mut spec = request(&p.root, "npm", &["test"]);
        spec.cwd = Some(p.root.join("escape"));
        assert_eq!(
            evaluate_agent_request(&developer(), &p.root, &spec).reason,
            Some(Reason::OutsideProject)
        );
        assert_eq!(
            eval(&developer(), &p, "npm", &["run", "escape/file.js"]).reason,
            Some(Reason::OutsideProject)
        );
    }
}
