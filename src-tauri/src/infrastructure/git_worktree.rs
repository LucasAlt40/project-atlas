//! [`WorktreeManager`] over the `git` program.
//!
//! How Git is run, for every call:
//!
//! - as a program with a list of arguments, never through a shell or a command line;
//! - every value that becomes an argument is one Atlas derived (a branch of the form
//!   `atlas/exec-NNNNNN`, a full commit id, a path inside the worktree folder) or a branch name
//!   Git itself reported, and refs are always spelled `refs/heads/...`, so nothing can read as
//!   an option;
//! - with hooks off (`core.hooksPath` is an empty folder): Atlas's own commits and merges
//!   never run scripts that came from the repository;
//! - without a terminal prompt, an editor, a pager or any inherited `GIT_*` location variable;
//! - with a time limit, so a stuck Git cannot hold an execution for ever;
//! - one write operation at a time (they share the repository's locks).
//!
//! Nothing here resets, cleans, stashes, forces or deletes anything that is not fully merged.

use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use crate::application::worktree::{
    Assessment, BaseInfo, BaseReadiness, MergeOutcome, NewWorktree, WorktreeError, WorktreeLayout,
    WorktreeManager,
};
use crate::domain::worktree::MAX_LISTED_FILES;

const GIT_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
const POLL: Duration = Duration::from_millis(10);
/// Location variables that would send Git somewhere other than the folder it is run in.
const GIT_LOCATION_VARIABLES: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
    "GIT_PREFIX",
    "GIT_CEILING_DIRECTORIES",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_EXEC_PATH",
    "GIT_EXTERNAL_DIFF",
    "GIT_SSH_COMMAND",
    "GIT_ASKPASS",
];

struct Output {
    code: Option<i32>,
    stdout: Vec<u8>,
    stderr: String,
}

impl Output {
    fn ok(&self) -> bool {
        self.code == Some(0)
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).trim().to_owned()
    }
}

pub struct GitWorktreeManager {
    git: PathBuf,
    layout: WorktreeLayout,
    /// An empty folder used as `core.hooksPath`.
    no_hooks: PathBuf,
    /// Held for every operation that writes to a repository.
    write_lock: Mutex<()>,
}

impl GitWorktreeManager {
    pub fn new(git: PathBuf, layout: WorktreeLayout) -> Self {
        let no_hooks = layout.root().join(".no-hooks");
        Self {
            git,
            layout,
            no_hooks,
            write_lock: Mutex::new(()),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ()> {
        self.write_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Runs Git in `cwd` and waits (bounded) for it.
    fn run<I, S>(
        &self,
        cwd: &Path,
        args: I,
        identity: Option<&Identity>,
    ) -> Result<Output, WorktreeError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        // Without this a missing folder reads as a missing `git` (both are "not found").
        if !cwd.is_dir() {
            return Err(WorktreeError::Git(format!(
                "not a folder: {}",
                cwd.display()
            )));
        }
        fs::create_dir_all(&self.no_hooks).map_err(|e| WorktreeError::Git(e.to_string()))?;
        let mut command = Command::new(&self.git);
        command
            .arg("-c")
            .arg(format!("core.hooksPath={}", self.no_hooks.display()))
            .args(["-c", "core.fsmonitor=false"])
            .args(["-c", "commit.gpgsign=false"])
            .args(["-c", "merge.autoStash=false"])
            .args(["-c", "core.quotePath=false"])
            .args(["--no-pager", "--no-optional-locks"])
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_EDITOR", "true")
            .env("GIT_MERGE_AUTOEDIT", "no")
            .env("LC_ALL", "C");
        for variable in GIT_LOCATION_VARIABLES {
            command.env_remove(variable);
        }
        if let Some(identity) = identity {
            command
                .env("GIT_AUTHOR_NAME", &identity.name)
                .env("GIT_AUTHOR_EMAIL", &identity.email)
                .env("GIT_COMMITTER_NAME", &identity.name)
                .env("GIT_COMMITTER_EMAIL", &identity.email);
        }
        let mut child = command.spawn().map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                WorktreeError::GitUnavailable
            } else {
                WorktreeError::Git(error.to_string())
            }
        })?;
        let stdout = child
            .stdout
            .take()
            .map(|s| thread::spawn(move || capture(s)));
        let stderr = child
            .stderr
            .take()
            .map(|s| thread::spawn(move || capture(s)));
        let deadline = Instant::now() + GIT_TIMEOUT;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(WorktreeError::Git("timed out".to_owned()));
                }
                Ok(None) => thread::sleep(POLL),
                Err(error) => return Err(WorktreeError::Git(error.to_string())),
            }
        };
        let take = |handle: Option<thread::JoinHandle<Vec<u8>>>| {
            handle.and_then(|h| h.join().ok()).unwrap_or_default()
        };
        Ok(Output {
            code: status.code(),
            stdout: take(stdout),
            stderr: String::from_utf8_lossy(&take(stderr)).trim().to_owned(),
        })
    }

    /// Like [`Self::run`], but a failing exit status is an error carrying Git's message.
    fn run_ok<I, S>(&self, cwd: &Path, args: I) -> Result<Output, WorktreeError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = self.run(cwd, args, None)?;
        if output.ok() {
            Ok(output)
        } else {
            Err(WorktreeError::Git(first_line(&output.stderr)))
        }
    }

    /// The author for Atlas's own commits when Git has none configured.
    fn identity(&self, cwd: &Path) -> Option<Identity> {
        let configured = |key: &str| {
            self.run(cwd, ["config", "--get", key], None)
                .ok()
                .filter(Output::ok)
                .map(|o| o.text())
                .filter(|v| !v.is_empty())
        };
        if configured("user.name").is_some() && configured("user.email").is_some() {
            None
        } else {
            Some(Identity {
                name: "Atlas".to_owned(),
                email: "atlas@localhost".to_owned(),
            })
        }
    }

    fn current_branch(&self, cwd: &Path) -> Result<Option<String>, WorktreeError> {
        let output = self.run(cwd, ["symbolic-ref", "--quiet", "--short", "HEAD"], None)?;
        Ok(output.ok().then(|| output.text()).filter(|b| !b.is_empty()))
    }

    /// Uncommitted changes to files Git tracks. Untracked files do not count: they are not
    /// work Git could lose, and a merge that would overwrite one stops by itself.
    fn is_dirty(&self, cwd: &Path) -> Result<bool, WorktreeError> {
        Ok(!self
            .run_ok(
                cwd,
                ["status", "--porcelain=v1", "-z", "--untracked-files=no"],
            )?
            .stdout
            .is_empty())
    }

    fn require_execution_branch(branch: &str) -> Result<(), WorktreeError> {
        if WorktreeLayout::is_execution_branch(branch) {
            Ok(())
        } else {
            Err(WorktreeError::InvalidIdentifier(branch.to_owned()))
        }
    }

    /// A branch name Git reported for the checkout (or the user chose): safe as a ref.
    fn require_valid_base(&self, cwd: &Path, branch: &str) -> Result<(), WorktreeError> {
        let valid = !branch.starts_with('-')
            && self
                .run(cwd, ["check-ref-format", "--branch", branch], None)?
                .ok();
        if valid {
            Ok(())
        } else {
            Err(WorktreeError::InvalidIdentifier(branch.to_owned()))
        }
    }

    fn require_commit_id(commit: &str) -> Result<(), WorktreeError> {
        if matches!(commit.len(), 40 | 64) && commit.bytes().all(|b| b.is_ascii_hexdigit()) {
            Ok(())
        } else {
            Err(WorktreeError::InvalidIdentifier(commit.to_owned()))
        }
    }

    /// `path` must be inside the worktree folder, spelled without `..`.
    fn require_inside_root(&self, path: &Path) -> Result<(), WorktreeError> {
        let inside = path.starts_with(self.layout.root())
            && !path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir));
        if inside {
            Ok(())
        } else {
            Err(WorktreeError::OutsideRoot(path.display().to_string()))
        }
    }

    /// Whether the checkout is on `base_branch` and clean. The caller holds the write lock.
    fn base_readiness(
        &self,
        toplevel: &Path,
        base_branch: &str,
    ) -> Result<BaseReadiness, WorktreeError> {
        self.require_valid_base(toplevel, base_branch)?;
        if self.current_branch(toplevel)?.as_deref() != Some(base_branch) {
            return Ok(BaseReadiness::BranchChanged);
        }
        // A merge in progress or any local change: not ours to build on.
        let merging = self.ref_exists(toplevel, "MERGE_HEAD")?;
        if merging || self.is_dirty(toplevel)? {
            return Ok(BaseReadiness::Dirty);
        }
        Ok(BaseReadiness::Ready)
    }

    fn ref_exists(&self, cwd: &Path, reference: &str) -> Result<bool, WorktreeError> {
        Ok(self
            .run(cwd, ["rev-parse", "--verify", "--quiet", reference], None)?
            .ok())
    }
}

struct Identity {
    name: String,
    email: String,
}

fn capture(mut stream: impl Read) -> Vec<u8> {
    let mut buffer = Vec::new();
    let _ = (&mut stream)
        .take(MAX_OUTPUT_BYTES as u64)
        .read_to_end(&mut buffer);
    let _ = std::io::copy(&mut stream, &mut std::io::sink());
    buffer
}

fn first_line(text: &str) -> String {
    text.lines()
        .next()
        .unwrap_or("")
        .chars()
        .take(300)
        .collect()
}

fn lines(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .filter(|l| !l.is_empty())
        .collect()
}

/// NUL-separated names, as `-z` prints them.
fn nul_separated(output: &Output) -> Vec<String> {
    output
        .stdout
        .split(|b| *b == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).into_owned())
        .collect()
}

fn head(branch: &str) -> String {
    format!("refs/heads/{branch}")
}

impl WorktreeManager for GitWorktreeManager {
    fn is_git_repository(&self, project: &Path) -> bool {
        project.is_dir()
            && self
                .run(project, ["rev-parse", "--is-inside-work-tree"], None)
                .is_ok_and(|o| o.ok() && o.text() == "true")
    }

    fn inspect_base(&self, project: &Path) -> Result<BaseInfo, WorktreeError> {
        if !self.is_git_repository(project) {
            if !project.is_dir() {
                return Err(WorktreeError::GitRepositoryRequired);
            }
            // Not a repository and no `git` look the same to the user: say which.
            return match self.run(project, ["--version"], None) {
                Ok(_) => Err(WorktreeError::GitRepositoryRequired),
                Err(error) => Err(error),
            };
        }
        let toplevel = PathBuf::from(
            self.run_ok(project, ["rev-parse", "--show-toplevel"])?
                .text(),
        );
        let subdir = PathBuf::from(self.run_ok(project, ["rev-parse", "--show-prefix"])?.text());
        let branch = self.current_branch(&toplevel)?.ok_or_else(|| {
            WorktreeError::BaseUnavailable("the project is on a detached HEAD".to_owned())
        })?;
        let commit = self.run(
            &toplevel,
            ["rev-parse", "--verify", "--quiet", "HEAD^{commit}"],
            None,
        )?;
        if !commit.ok() {
            return Err(WorktreeError::BaseUnavailable(
                "the repository has no commits yet".to_owned(),
            ));
        }
        self.require_valid_base(&toplevel, &branch)?;
        Ok(BaseInfo {
            dirty: self.is_dirty(&toplevel)?,
            toplevel,
            subdir,
            branch,
            commit: commit.text(),
        })
    }

    fn create(&self, new: &NewWorktree<'_>) -> Result<(), WorktreeError> {
        Self::require_execution_branch(new.branch)?;
        Self::require_commit_id(new.base_commit)?;
        self.require_inside_root(new.path)?;
        let _write = self.lock();
        if new.path.symlink_metadata().is_ok() {
            return Err(WorktreeError::Collision(new.path.display().to_string()));
        }
        if self.ref_exists(new.toplevel, &head(new.branch))? {
            return Err(WorktreeError::Collision(new.branch.to_owned()));
        }
        if let Some(parent) = new.path.parent() {
            fs::create_dir_all(parent).map_err(|e| WorktreeError::Git(e.to_string()))?;
        }
        self.run_ok(
            new.toplevel,
            [
                OsStr::new("worktree"),
                OsStr::new("add"),
                OsStr::new("--quiet"),
                OsStr::new("--no-track"),
                OsStr::new("-b"),
                OsStr::new(new.branch),
                new.path.as_os_str(),
                OsStr::new(new.base_commit),
            ],
        )?;
        Ok(())
    }

    fn get_branch(&self, path: &Path) -> Result<String, WorktreeError> {
        self.current_branch(path)?
            .ok_or_else(|| WorktreeError::BaseUnavailable("detached HEAD".to_owned()))
    }

    fn get_status(&self, path: &Path) -> Result<Vec<String>, WorktreeError> {
        let output = self.run_ok(
            path,
            ["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        )?;
        // Each entry is `XY path`; a rename adds the old path as a separate entry.
        Ok(nul_separated(&output)
            .into_iter()
            .filter_map(|entry| entry.get(3..).map(str::to_owned))
            .filter(|path| !path.is_empty())
            .collect())
    }

    fn get_diff(
        &self,
        toplevel: &Path,
        base_branch: &str,
        branch: &str,
        max_bytes: usize,
    ) -> Result<String, WorktreeError> {
        Self::require_execution_branch(branch)?;
        self.require_valid_base(toplevel, base_branch)?;
        let range = format!("{}...{}", head(base_branch), head(branch));
        let output = self.run_ok(
            toplevel,
            ["diff", "--no-color", "--no-ext-diff", &range, "--"],
        )?;
        let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
        if text.len() > max_bytes {
            let mut cut = max_bytes;
            while !text.is_char_boundary(cut) {
                cut -= 1;
            }
            text.truncate(cut);
        }
        Ok(text)
    }

    fn commit_all(&self, path: &Path, message: &str) -> Result<Option<String>, WorktreeError> {
        self.require_inside_root(path)?;
        let _write = self.lock();
        self.run_ok(path, ["add", "--all"])?;
        // Exit 0: nothing staged. Exit 1: something is.
        let staged = self.run(path, ["diff", "--cached", "--quiet"], None)?;
        if staged.ok() {
            return Ok(None);
        }
        let identity = self.identity(path);
        let commit = self.run(
            path,
            ["commit", "--no-verify", "--quiet", "--message", message],
            identity.as_ref(),
        )?;
        if !commit.ok() {
            return Err(WorktreeError::Git(first_line(&commit.stderr)));
        }
        Ok(Some(self.run_ok(path, ["rev-parse", "HEAD"])?.text()))
    }

    fn assess(
        &self,
        toplevel: &Path,
        base_branch: &str,
        branch: &str,
    ) -> Result<Assessment, WorktreeError> {
        Self::require_execution_branch(branch)?;
        self.require_valid_base(toplevel, base_branch)?;
        let (base, work) = (head(base_branch), head(branch));
        let counts = self.run_ok(
            toplevel,
            [
                "rev-list",
                "--left-right",
                "--count",
                &format!("{base}...{work}"),
            ],
        )?;
        let text = counts.text();
        let mut numbers = text
            .split_whitespace()
            .map(|n| n.parse::<u32>().unwrap_or(0));
        let (behind, ahead) = (numbers.next().unwrap_or(0), numbers.next().unwrap_or(0));
        let files = nul_separated(&self.run_ok(
            toplevel,
            [
                "diff",
                "--name-only",
                "-z",
                &format!("{base}...{work}"),
                "--",
            ],
        )?);
        let files_changed = u32::try_from(files.len()).unwrap_or(u32::MAX);
        let mut assessment = Assessment {
            files: files.into_iter().take(MAX_LISTED_FILES).collect(),
            files_changed,
            commits_ahead: ahead,
            commits_behind: behind,
            conflicts: Vec::new(),
            mergeable: None,
        };
        // A trial merge in Git's object store: no checkout, index or ref is touched.
        let trial = self.run(
            toplevel,
            [
                "merge-tree",
                "--write-tree",
                "--name-only",
                "--no-messages",
                &base,
                &work,
            ],
            None,
        )?;
        match trial.code {
            Some(0) => assessment.mergeable = Some(true),
            Some(1) => {
                assessment.mergeable = Some(false);
                // First line is the tree id; then the conflicted files, then a blank line.
                assessment.conflicts = lines(&trial)
                    .into_iter()
                    .skip(1)
                    .take(MAX_LISTED_FILES)
                    .collect();
            }
            // Older Git (no `--write-tree`) or an error: unknown, never assumed clean.
            _ => {}
        }
        Ok(assessment)
    }

    fn base_ready(
        &self,
        toplevel: &Path,
        base_branch: &str,
    ) -> Result<BaseReadiness, WorktreeError> {
        // Behind the same lock as merges: never look at a checkout in the middle of one.
        let _write = self.lock();
        self.base_readiness(toplevel, base_branch)
    }

    fn merge(
        &self,
        toplevel: &Path,
        base_branch: &str,
        branch: &str,
        message: &str,
    ) -> Result<MergeOutcome, WorktreeError> {
        Self::require_execution_branch(branch)?;
        self.require_valid_base(toplevel, base_branch)?;
        let _write = self.lock();
        match self.base_readiness(toplevel, base_branch)? {
            BaseReadiness::Ready => {}
            BaseReadiness::Dirty => {
                return Err(WorktreeError::NotMergeable(
                    "the project checkout has uncommitted changes".to_owned(),
                ))
            }
            BaseReadiness::BranchChanged => {
                return Err(WorktreeError::NotMergeable(
                    "the project checkout is no longer on the base branch".to_owned(),
                ))
            }
        }
        let identity = self.identity(toplevel);
        let merged = self.run(
            toplevel,
            [
                "merge",
                "--no-ff",
                "--no-edit",
                "--no-verify",
                "--no-stat",
                "--message",
                message,
                &head(branch),
            ],
            identity.as_ref(),
        )?;
        if merged.ok() {
            return Ok(MergeOutcome::Merged {
                commit: self.run_ok(toplevel, ["rev-parse", "HEAD"])?.text(),
            });
        }
        // Stopped half-way: only a merge *we* started has a MERGE_HEAD (the checkout was
        // clean). Name the conflicts, then undo exactly that attempt.
        if self.ref_exists(toplevel, "MERGE_HEAD")? {
            let files =
                lines(&self.run(toplevel, ["diff", "--name-only", "--diff-filter=U"], None)?);
            self.run_ok(toplevel, ["merge", "--abort"])?;
            return Ok(MergeOutcome::Conflict {
                files: files.into_iter().take(MAX_LISTED_FILES).collect(),
            });
        }
        Err(WorktreeError::Git(first_line(&merged.stderr)))
    }

    fn remove(&self, toplevel: &Path, path: &Path) -> Result<(), WorktreeError> {
        self.require_inside_root(path)?;
        let _write = self.lock();
        let metadata = path.symlink_metadata();
        if metadata.is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(WorktreeError::OutsideRoot(path.display().to_string()));
        }
        // Never `--force`: a worktree Git considers unclean is kept.
        self.run_ok(
            toplevel,
            [
                OsStr::new("worktree"),
                OsStr::new("remove"),
                path.as_os_str(),
            ],
        )?;
        Ok(())
    }

    fn delete_merged_branch(&self, toplevel: &Path, branch: &str) -> Result<(), WorktreeError> {
        Self::require_execution_branch(branch)?;
        let _write = self.lock();
        if !self.ref_exists(toplevel, &head(branch))? {
            return Ok(());
        }
        self.run_ok(toplevel, ["branch", "--delete", "--", branch])?;
        Ok(())
    }
}
