//! The macOS sandbox profile of one execution, made from facts and nothing else.
//!
//! Permissive by default and strict where it counts: everything is allowed (so the CLIs keep
//! their Keychain, their network and their tools), every **write** is denied except the listed
//! places, and the reading of credential stores is denied. Later rules win in the profile
//! language, so the denials that must beat an allowance (a repository's `hooks`) come last.
//!
//! Every path reaches the profile as a `-D` parameter (`(param "P3")`), never as text inside it:
//! a path with a quote in it cannot change what the profile says. The only text that is
//! interpolated, the prefixes of files, is checked first and refused if it is not plain.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Where a linked worktree keeps its Git data, found from its `.git` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitLayout {
    /// `<repo>/.git/worktrees/<id>`: the worktree's own index, HEAD and locks.
    pub worktree_dir: PathBuf,
    /// `<repo>/.git`: shared objects and refs.
    pub common_dir: PathBuf,
}

/// The branch prefix Atlas gives the worktrees it makes (`atlas/exec-7`). Only refs under it can
/// be written; `main`, tags and every other branch cannot.
pub const BRANCH_PREFIX: &str = "atlas";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileInput {
    pub worktree: PathBuf,
    /// The run may change the worktree (it is isolated, or the policy lets it write).
    pub worktree_writable: bool,
    /// Folders written to as a whole: the execution's own temporary folder, the CLI's folders,
    /// the developer caches.
    pub write_dirs: Vec<PathBuf>,
    /// Files written to, with their siblings (`file`, `file.tmp.1`…).
    pub write_file_prefixes: Vec<PathBuf>,
    /// The Git data of a linked worktree.
    pub git: Option<GitLayout>,
    /// Places inside an allowed folder that stay refused (`.git/hooks` of a project that is its
    /// own working directory).
    pub deny_write: Vec<PathBuf>,
    pub deny_read: Vec<PathBuf>,
}

/// A profile and the parameters it needs, for `sandbox-exec -D name=value -f profile`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub text: String,
    pub params: Vec<(String, String)>,
}

/// Why a profile could not be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileError(pub String);

struct Params(Vec<(String, String)>);

impl Params {
    /// The profile expression that stands for `path`.
    fn of(&mut self, path: &Path) -> String {
        let name = format!("P{}", self.0.len());
        self.0
            .push((name.clone(), path.to_string_lossy().into_owned()));
        format!("(param \"{name}\")")
    }
}

/// A prefix as a regular expression, or `None` if the path has anything but plain characters (the
/// profile is text, and a character that ends a string must not get in).
fn prefix_regex(path: &Path) -> Option<String> {
    let text = path.to_str()?;
    let mut out = String::from("^");
    for c in text.chars() {
        match c {
            c if c.is_alphanumeric() || matches!(c, '/' | '-' | '_' | ' ' | '@' | '+') => {
                out.push(c);
            }
            '.' => out.push_str("\\."),
            _ => return None,
        }
    }
    Some(out)
}

/// The profile for `input`.
///
/// # Errors
///
/// When a path cannot be placed in the profile safely.
pub fn build(input: &ProfileInput) -> Result<Profile, ProfileError> {
    let mut params = Params(Vec::new());
    let mut text = String::from("(version 1)\n(allow default)\n(deny file-write*)\n");

    text.push_str("(allow file-write*\n  (literal \"/dev/null\") (literal \"/dev/tty\") (literal \"/dev/dtracehelper\")\n  (regex #\"^/dev/tty\") (regex #\"^/dev/pty\")\n");
    if input.worktree_writable {
        let _ = writeln!(text, "  (subpath {})", params.of(&input.worktree));
    }
    for dir in &input.write_dirs {
        let _ = writeln!(text, "  (subpath {})", params.of(dir));
    }
    if let Some(git) = &input.git {
        let _ = writeln!(text, "  (subpath {})", params.of(&git.worktree_dir));
        let _ = writeln!(
            text,
            "  (subpath {})",
            params.of(&git.common_dir.join("objects"))
        );
        for under in ["refs/heads", "logs/refs/heads"] {
            let dir = git.common_dir.join(under).join(BRANCH_PREFIX);
            let _ = writeln!(text, "  (subpath {})", params.of(&dir));
        }
    }
    for prefix in &input.write_file_prefixes {
        let regex = prefix_regex(prefix).ok_or_else(|| {
            ProfileError(format!("not a plain path: {}", prefix.to_string_lossy()))
        })?;
        let _ = writeln!(text, "  (regex #\"{regex}\")");
    }
    text.push_str(")\n");

    if !input.deny_write.is_empty() {
        text.push_str("(deny file-write*\n");
        for path in &input.deny_write {
            let _ = writeln!(text, "  (subpath {})", params.of(path));
        }
        text.push_str(")\n");
    }
    if !input.deny_read.is_empty() {
        text.push_str("(deny file-read*\n");
        for path in &input.deny_read {
            let _ = writeln!(text, "  (subpath {})", params.of(path));
        }
        text.push_str(")\n");
    }
    Ok(Profile {
        text,
        params: params.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> ProfileInput {
        ProfileInput {
            worktree: PathBuf::from("/work/wt"),
            worktree_writable: true,
            write_dirs: vec![
                PathBuf::from("/tmp/atlas/exec-1"),
                PathBuf::from("/home/u/.claude"),
            ],
            write_file_prefixes: vec![PathBuf::from("/home/u/.claude.json")],
            git: Some(GitLayout {
                worktree_dir: PathBuf::from("/repo/.git/worktrees/wt"),
                common_dir: PathBuf::from("/repo/.git"),
            }),
            deny_write: Vec::new(),
            deny_read: vec![PathBuf::from("/home/u/.ssh")],
        }
    }

    fn param<'a>(profile: &'a Profile, value: &str) -> &'a str {
        &profile
            .params
            .iter()
            .find(|(_, v)| v == value)
            .unwrap_or_else(|| panic!("no parameter for {value}"))
            .0
    }

    #[test]
    fn everything_is_allowed_but_writing_and_the_credential_stores() {
        let profile = build(&input()).unwrap();

        assert!(profile
            .text
            .starts_with("(version 1)\n(allow default)\n(deny file-write*)\n"));
        assert!(profile.text.contains("(deny file-read*"));
        let ssh = param(&profile, "/home/u/.ssh");
        assert!(profile
            .text
            .contains(&format!("(subpath (param \"{ssh}\"))")));
    }

    #[test]
    fn a_path_is_a_parameter_and_never_part_of_the_profile_text() {
        let mut hostile = input();
        hostile.worktree = PathBuf::from("/work/\") (allow file-write* (subpath \"/\")) (\"");

        let profile = build(&hostile).unwrap();

        assert!(!profile.text.contains("allow file-write* (subpath \"/\")"));
        assert!(!profile.text.contains("/work/"));
        assert_eq!(
            profile.params[0].1,
            "/work/\") (allow file-write* (subpath \"/\")) (\""
        );
    }

    #[test]
    fn a_read_only_run_gets_no_write_access_to_its_worktree() {
        let mut read_only = input();
        read_only.worktree_writable = false;

        let profile = build(&read_only).unwrap();

        assert!(profile.params.iter().all(|(_, v)| v != "/work/wt"));
        assert!(build(&input())
            .unwrap()
            .params
            .iter()
            .any(|(_, v)| v == "/work/wt"));
    }

    #[cfg(unix)]
    #[test]
    fn git_is_narrow_the_worktrees_own_dir_the_objects_and_the_atlas_branches_only() {
        let profile = build(&input()).unwrap();

        let values: Vec<&str> = profile.params.iter().map(|(_, v)| v.as_str()).collect();
        assert!(values.contains(&"/repo/.git/worktrees/wt"));
        assert!(values.contains(&"/repo/.git/objects"));
        assert!(values.contains(&"/repo/.git/refs/heads/atlas"));
        assert!(values.contains(&"/repo/.git/logs/refs/heads/atlas"));
        // Not the whole `.git`: no hooks, no config, no other branch.
        assert!(!values.contains(&"/repo/.git"));
        assert!(!values
            .iter()
            .any(|v| v.contains("hooks") || v.ends_with("/config")));
    }

    #[test]
    fn the_denials_come_after_the_allowances_because_the_last_rule_wins() {
        let mut project = input();
        project.git = None;
        project.deny_write = vec![PathBuf::from("/work/wt/.git/hooks")];

        let profile = build(&project).unwrap();

        let allow = profile.text.find("(allow file-write*").unwrap();
        let deny = profile.text.rfind("(deny file-write*").unwrap();
        assert!(deny > allow);
        assert!(profile
            .params
            .iter()
            .any(|(_, v)| v == "/work/wt/.git/hooks"));
    }

    #[test]
    fn a_file_prefix_is_escaped_and_one_with_a_quote_in_it_is_refused() {
        let profile = build(&input()).unwrap();
        assert!(profile
            .text
            .contains(r#"(regex #"^/home/u/\.claude\.json")"#));

        let mut bad = input();
        bad.write_file_prefixes = vec![PathBuf::from("/home/u/\".claude.json")];
        assert!(build(&bad).is_err());
    }

    #[test]
    fn two_equal_inputs_make_the_same_profile() {
        assert_eq!(build(&input()), build(&input()));
    }
}
