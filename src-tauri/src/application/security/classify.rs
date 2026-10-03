//! What kind of operation is a command? Deliberately shallow: names and well-known flags, not
//! a parser for every tool. The result feeds the policy; anything unrecognised is treated as
//! the *more* dangerous kind, never the less.

/// The program's identity for policy purposes: the bare, lowercased file name without a
/// Windows executable extension (`C:\x\NPM.CMD` -> `npm`).
pub fn program_stem(program: &str) -> String {
    let name = program
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(program)
        .to_lowercase();
    for extension in [".exe", ".cmd", ".bat", ".com"] {
        if let Some(stem) = name.strip_suffix(extension) {
            return stem.to_owned();
        }
    }
    name
}

/// Does the program name a location (`./npm`, `/tmp/npm`, `C:\x\npm.exe`) rather than a tool
/// looked up on the search path? A script called `npm` inside the project must not be mistaken
/// for the real one.
pub fn has_path(program: &str) -> bool {
    program.contains('/') || program.contains('\\')
}

/// Command interpreters. Running one turns "executable + arguments" into "whatever string the
/// agent builds", which defeats every other rule, so they are never run.
const SHELLS: &[&str] = &[
    "sh",
    "bash",
    "zsh",
    "fish",
    "dash",
    "ash",
    "ksh",
    "csh",
    "tcsh",
    "busybox",
    "cmd",
    "command",
    "powershell",
    "pwsh",
    "wsl",
    "conhost",
    "mshta",
    "wscript",
    "cscript",
    "osascript",
];

/// Programs whose whole purpose is to run another program (so the real program would escape the
/// per-program rules).
const LAUNCHERS: &[&str] = &[
    "env", "xargs", "nohup", "sudo", "su", "doas", "runas", "start", "exec", "eval", "time",
    "timeout", "nice", "setsid", "script", "watch", "find",
];

pub fn is_shell(stem: &str) -> bool {
    SHELLS.contains(&stem)
}

pub fn is_launcher(stem: &str) -> bool {
    LAUNCHERS.contains(&stem)
}

/// Programs whose purpose is talking to the network.
const NETWORK_TOOLS: &[&str] = &[
    "curl",
    "wget",
    "ssh",
    "scp",
    "sftp",
    "nc",
    "ncat",
    "netcat",
    "telnet",
    "ftp",
    "rsync",
    "nmap",
    "socat",
    "http",
    "httpie",
    "invoke-webrequest",
];

pub fn is_network_tool(stem: &str) -> bool {
    NETWORK_TOOLS.contains(&stem)
}

/// Does this invocation hand the interpreter source code on the command line? Such a call can
/// do anything the interpreter can, so even an allowed interpreter asks first.
pub fn runs_inline_code(stem: &str, args: &[String]) -> bool {
    let flags: &[&str] = match stem {
        "python" | "python2" | "python3" | "py" => &["-c"],
        "node" | "nodejs" | "deno" | "bun" => &["-e", "--eval", "-p", "--print"],
        "ruby" | "perl" | "php" => &["-e", "-r", "-E"],
        _ => return false,
    };
    args.iter().any(|arg| flags.contains(&arg.as_str()))
}

/// Environment variables that change what a program loads or runs. Agents may not set them.
pub fn is_dangerous_env(key: &str) -> bool {
    let upper = key.to_uppercase();
    upper.starts_with("LD_")
        || upper.starts_with("DYLD_")
        || upper.starts_with("GIT_")
        || matches!(
            upper.as_str(),
            "PATH"
                | "PATHEXT"
                | "COMSPEC"
                | "SHELL"
                | "ENV"
                | "BASH_ENV"
                | "NODE_OPTIONS"
                | "NODE_PATH"
                | "PYTHONSTARTUP"
                | "PYTHONPATH"
                | "PYTHONHOME"
                | "RUBYOPT"
                | "PERL5OPT"
                | "RUSTC_WRAPPER"
                | "CARGO_BUILD_RUSTC_WRAPPER"
                | "JAVA_TOOL_OPTIONS"
                | "_JAVA_OPTIONS"
                | "JDK_JAVA_OPTIONS"
        )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitClass {
    /// status, diff, log, show…
    Read,
    /// add, commit, checkout, branch creation…
    Write,
    /// Loses work or rewrites history.
    Destructive,
    /// Can run programs through Git's own configuration (`-c`, `--exec-path`, …).
    ConfigOverride,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitOperation {
    pub class: GitClass,
    /// Reaches a remote (push, pull, fetch, clone…): also subject to the network policy.
    pub network: bool,
    /// Directories Git was told to work in (`-C <dir>`): they must be inside the project like
    /// the working directory.
    pub directories: Vec<String>,
}

const READ_SUBCOMMANDS: &[&str] = &[
    "status",
    "diff",
    "log",
    "show",
    "blame",
    "rev-parse",
    "rev-list",
    "ls-files",
    "ls-tree",
    "shortlog",
    "describe",
    "cat-file",
    "grep",
    "whatchanged",
    "merge-base",
    "diff-tree",
    "diff-files",
    "diff-index",
    "show-ref",
    "name-rev",
    "count-objects",
    "version",
    "help",
];

const NETWORK_SUBCOMMANDS: &[&str] = &["push", "pull", "fetch", "clone", "ls-remote", "submodule"];

/// Git options that run programs. (The global `-c key=value` is handled before the subcommand:
/// after it, `-c` means other things, such as `git commit -c <commit>`.)
const CONFIG_OVERRIDES: &[&str] = &[
    "--config-env",
    "--exec-path",
    "--upload-pack",
    "--receive-pack",
    "--ext-diff",
    "--textconv",
];

/// Classifies a `git` invocation. Not a Git parser: it finds the subcommand, recognises the
/// read-only ones, the known destructive forms, and treats everything else as a write.
pub fn classify_git(args: &[String]) -> GitOperation {
    let mut directories = Vec::new();
    let mut index = 0;
    let mut config_override = false;
    // Global options come before the subcommand.
    while let Some(arg) = args.get(index) {
        if arg == "-C" {
            if let Some(dir) = args.get(index + 1) {
                directories.push(dir.clone());
            }
            index += 2;
        } else if arg == "-c" || arg == "--config-env" || arg == "--exec-path" {
            config_override = true;
            index += 2;
        } else if let Some(dir) = arg
            .strip_prefix("--git-dir=")
            .or_else(|| arg.strip_prefix("--work-tree="))
        {
            directories.push(dir.to_owned());
            index += 1;
        } else if arg == "--git-dir" || arg == "--work-tree" {
            if let Some(dir) = args.get(index + 1) {
                directories.push(dir.clone());
            }
            index += 2;
        } else if arg.starts_with("--exec-path") || arg.starts_with("--config-env") {
            config_override = true;
            index += 1;
        } else if arg.starts_with('-') {
            index += 1;
        } else {
            break;
        }
    }
    let subcommand = args.get(index).map_or("", String::as_str);
    let rest = args.get(index + 1..).unwrap_or(&[]);
    let has = |flags: &[&str]| rest.iter().any(|a| flags.contains(&a.as_str()));
    let has_prefix = |prefixes: &[&str]| {
        rest.iter()
            .any(|a| prefixes.iter().any(|p| a.starts_with(p)))
    };

    let network = NETWORK_SUBCOMMANDS.contains(&subcommand);
    let class = if config_override
        || rest.iter().any(|a| {
            CONFIG_OVERRIDES
                .iter()
                .any(|o| a == o || a.starts_with(&format!("{o}=")))
        }) {
        GitClass::ConfigOverride
    } else {
        match subcommand {
            "reset" if has(&["--hard", "--merge", "--keep"]) => GitClass::Destructive,
            "clean" if !has(&["-n", "--dry-run"]) => GitClass::Destructive,
            "push"
                if has(&[
                    "-f",
                    "--force",
                    "--force-with-lease",
                    "-d",
                    "--delete",
                    "--mirror",
                    "--prune",
                ]) || has_prefix(&["--force-with-lease=", "+", ":"]) =>
            {
                GitClass::Destructive
            }
            "branch" if has(&["-D", "-d", "--delete", "-M", "--move", "-f", "--force"]) => {
                GitClass::Destructive
            }
            "tag" if has(&["-d", "--delete", "-f", "--force"]) => GitClass::Destructive,
            "checkout" | "restore" | "switch"
                if has(&["-f", "--force", "--discard-changes", "--", "."]) =>
            {
                GitClass::Destructive
            }
            "stash" if has(&["drop", "clear"]) => GitClass::Destructive,
            "rebase" | "filter-branch" | "filter-repo" | "gc" | "prune" | "reflog"
            | "update-ref" | "replace" | "fsck" | "worktree" | "rm" => GitClass::Destructive,
            "branch" if rest.iter().all(|a| a.starts_with('-') && !has(&["-m"])) => {
                // `git branch`, `git branch -a`, `-v`, `--list`: listing.
                GitClass::Read
            }
            "tag" if rest.is_empty() || has(&["-l", "--list"]) => GitClass::Read,
            "remote" if rest.is_empty() || has(&["-v", "--verbose", "show"]) => GitClass::Read,
            "config" if has(&["--get", "--get-all", "--list", "-l"]) => GitClass::Read,
            sub if READ_SUBCOMMANDS.contains(&sub) => {
                // `--output=<file>` makes read commands write a file.
                if has_prefix(&["--output"]) {
                    GitClass::Write
                } else {
                    GitClass::Read
                }
            }
            // add, commit, merge, pull, push, checkout, switch, … and anything unknown
            // (aliases, plugins, `git foo`): not known to be harmless.
            _ => GitClass::Write,
        }
    };
    GitOperation {
        class,
        network,
        directories,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|a| (*a).to_owned()).collect()
    }

    fn git(list: &[&str]) -> GitClass {
        classify_git(&args(list)).class
    }

    #[test]
    fn program_names_are_compared_without_path_or_windows_extension() {
        assert_eq!(program_stem("npm"), "npm");
        assert_eq!(program_stem("NPM.CMD"), "npm");
        assert_eq!(program_stem(r"C:\Program Files\Git\cmd\GIT.exe"), "git");
        assert_eq!(program_stem("/usr/bin/bash"), "bash");
        assert!(has_path("./npm"));
        assert!(has_path(r"C:\x\npm.exe"));
        assert!(!has_path("npm"));
    }

    #[test]
    fn shells_and_launchers_are_recognised_on_every_platform() {
        for shell in [
            "sh",
            "bash",
            "zsh",
            "cmd",
            "cmd.exe",
            "powershell.exe",
            "pwsh",
            "/bin/sh",
            r"C:\Windows\System32\cmd.exe",
        ] {
            assert!(is_shell(&program_stem(shell)), "{shell}");
        }
        for launcher in ["env", "xargs", "sudo", "start", "find"] {
            assert!(is_launcher(launcher), "{launcher}");
        }
        assert!(!is_shell("git") && !is_launcher("cargo"));
    }

    #[test]
    fn network_tools_are_recognised() {
        assert!(is_network_tool("curl"));
        assert!(is_network_tool("ssh"));
        assert!(!is_network_tool("cargo"));
    }

    #[test]
    fn interpreters_given_inline_code_are_flagged() {
        assert!(runs_inline_code("python", &args(&["-c", "import os"])));
        assert!(runs_inline_code("node", &args(&["-e", "1"])));
        assert!(!runs_inline_code("python", &args(&["script.py"])));
        assert!(!runs_inline_code("cargo", &args(&["-c"])));
    }

    #[test]
    fn dangerous_environment_variables_are_recognised() {
        for key in [
            "LD_PRELOAD",
            "DYLD_INSERT_LIBRARIES",
            "GIT_SSH_COMMAND",
            "path",
            "NODE_OPTIONS",
        ] {
            assert!(is_dangerous_env(key), "{key}");
        }
        assert!(!is_dangerous_env("CI"));
        assert!(!is_dangerous_env("RUST_LOG"));
    }

    #[test]
    fn git_reads() {
        for list in [
            &["status"][..],
            &["diff", "HEAD~1"],
            &["log", "--oneline", "-5"],
            &["show", "abc"],
            &["branch"],
            &["branch", "-a"],
            &["tag"],
            &["remote", "-v"],
            &["config", "--get", "user.name"],
            &["-C", "sub", "status"],
        ] {
            assert_eq!(git(list), GitClass::Read, "{list:?}");
        }
    }

    #[test]
    fn git_writes() {
        for list in [
            &["add", "."][..],
            &["commit", "-m", "x"],
            &["checkout", "main"],
            &["checkout", "-b", "feature"],
            &["branch", "feature"],
            &["merge", "x"],
            &["stash"],
            &["tag", "v1"],
            // Unknown subcommands are not assumed harmless.
            &["frobnicate"],
            &["diff", "--output=out.txt"],
        ] {
            assert_eq!(git(list), GitClass::Write, "{list:?}");
        }
    }

    #[test]
    fn git_destructive_operations() {
        for list in [
            &["reset", "--hard"][..],
            &["reset", "--hard", "HEAD~3"],
            &["clean", "-fd"],
            &["push", "--force"],
            &["push", "-f", "origin", "main"],
            &["push", "origin", "+main"],
            &["push", "origin", ":old-branch"],
            &["push", "--delete", "origin", "x"],
            &["branch", "-D", "x"],
            &["branch", "--delete", "x"],
            &["checkout", "--", "."],
            &["checkout", "-f"],
            &["stash", "drop"],
            &["rebase", "main"],
            &["reflog", "expire"],
        ] {
            assert_eq!(git(list), GitClass::Destructive, "{list:?}");
        }
        // A dry run loses nothing.
        assert_eq!(git(&["clean", "-n"]), GitClass::Write);
        assert_eq!(git(&["reset", "--soft", "HEAD~1"]), GitClass::Write);
    }

    #[test]
    fn git_options_that_can_run_programs_are_flagged() {
        assert_eq!(
            git(&["-c", "core.sshCommand=evil", "fetch"]),
            GitClass::ConfigOverride
        );
        assert_eq!(git(&["diff", "--ext-diff"]), GitClass::ConfigOverride);
        assert_eq!(
            git(&["fetch", "--upload-pack=evil"]),
            GitClass::ConfigOverride
        );
    }

    #[test]
    fn git_remote_operations_are_flagged_as_network() {
        for sub in ["push", "pull", "fetch", "clone", "ls-remote"] {
            assert!(classify_git(&args(&[sub])).network, "{sub}");
        }
        assert!(!classify_git(&args(&["status"])).network);
        assert!(!classify_git(&args(&["commit", "-m", "x"])).network);
    }

    #[test]
    fn git_working_directory_overrides_are_reported_for_checking() {
        let op = classify_git(&args(&["-C", "../other", "status"]));
        assert_eq!(op.directories, ["../other"]);
        let op = classify_git(&args(&["--git-dir=/etc/x", "status"]));
        assert_eq!(op.directories, ["/etc/x"]);
    }
}
