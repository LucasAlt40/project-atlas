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
    ApplyOutcome, Assessment, BaseInfo, BaseReadiness, MergeOutcome, NewWorktree, WorktreeError,
    WorktreeLayout, WorktreeManager,
};
use crate::domain::live_workspace::{is_plain_relative_path, LiveFile, LiveFileKind};
use crate::domain::worktree::{
    FileChange, FileChangeStatus, MAX_CHANGESET_FILES, MAX_LISTED_FILES,
};

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
        self.run_with(cwd, args, identity, None)
    }

    /// [`Self::run`], optionally feeding `input` to Git's standard input.
    fn run_with<I, S>(
        &self,
        cwd: &Path,
        args: I,
        identity: Option<&Identity>,
        input: Option<Vec<u8>>,
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
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
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
        // Written from its own thread: a large patch must not wait on Git's output.
        let feeder = input.and_then(|bytes| {
            child.stdin.take().map(|mut stdin| {
                thread::spawn(move || {
                    use std::io::Write;
                    let _ = stdin.write_all(&bytes);
                })
            })
        });
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
        if let Some(feeder) = feeder {
            let _ = feeder.join();
        }
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

    /// A file name to look at: relative, inside the repository, and not something Git would read
    /// as an option or as a pathspec with magic in it.
    fn require_plain_path(file: &str) -> Result<(), WorktreeError> {
        let bad = file.is_empty()
            || file.len() > 400
            || file.starts_with(['/', '-', ':', '~'])
            || file.contains(['\0', '\n', '\\', '*', '?', '[', ']'])
            || file.split('/').any(|part| part == ".." || part == ".");
        if bad {
            Err(WorktreeError::InvalidIdentifier(file.to_owned()))
        } else {
            Ok(())
        }
    }

    /// A path to look at inside a worktree: relative, inside it, and nothing Git would read as an
    /// option. (Unlike [`Self::require_plain_path`], a name with `*` or `[` in it is a name:
    /// callers use literal pathspecs.)
    fn require_relative_path(file: &str) -> Result<(), WorktreeError> {
        let bad = file.is_empty()
            || file.len() > 1000
            || file.starts_with(['/', '-'])
            || file.contains(['\0', '\n'])
            || file.split('/').any(|part| part == ".." || part == ".");
        if bad {
            Err(WorktreeError::InvalidIdentifier(file.to_owned()))
        } else {
            Ok(())
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

    /// What the branch changed since it forked from the base: the files, and the patch (binary
    /// safe, without rename detection) that brings them.
    fn branch_patch(
        &self,
        toplevel: &Path,
        base_branch: &str,
        branch: &str,
    ) -> Result<(Vec<String>, Vec<u8>), WorktreeError> {
        let fork = self
            .run_ok(toplevel, ["merge-base", &head(base_branch), &head(branch)])?
            .text();
        Self::require_commit_id(&fork)?;
        let files = nul_separated(&self.run_ok(
            toplevel,
            [
                "diff",
                "--name-only",
                "-z",
                "--no-renames",
                "--no-ext-diff",
                &fork,
                &head(branch),
                "--",
            ],
        )?);
        let patch = self
            .run_ok(
                toplevel,
                [
                    "diff",
                    "--binary",
                    "--full-index",
                    "--no-color",
                    "--no-ext-diff",
                    "--no-renames",
                    &fork,
                    &head(branch),
                    "--",
                ],
            )?
            .stdout;
        Ok((files, patch))
    }

    /// After an apply: the history is exactly where it was, and every file shows as changed.
    fn verify_applied(
        &self,
        toplevel: &Path,
        before: &Snapshot,
        files: &[String],
    ) -> Result<(), WorktreeError> {
        let after = self.snapshot(toplevel)?;
        if after.head != before.head {
            return Err(WorktreeError::Git(
                "the project's HEAD moved while applying".to_owned(),
            ));
        }
        let dirty: std::collections::HashSet<&str> =
            after.dirty.iter().map(String::as_str).collect();
        if let Some(missing) = files.iter().find(|f| !dirty.contains(f.as_str())) {
            return Err(WorktreeError::Git(format!(
                "applied, but {missing} does not show as changed"
            )));
        }
        Ok(())
    }

    /// Git does not see a rename it was not told about (a moved file is a deletion and a new
    /// untracked file): a deleted file and a new one with exactly the same content are one
    /// rename. Only on a full read, and only for a bounded number of candidates.
    fn pair_renames(&self, path: &Path, baseline: &str, changes: &mut Vec<FileChange>) {
        const MAX_CANDIDATES: usize = 200;
        let deleted: Vec<usize> = (0..changes.len())
            .filter(|i| changes[*i].status == FileChangeStatus::Deleted)
            .take(MAX_CANDIDATES)
            .collect();
        let added: Vec<usize> = (0..changes.len())
            .filter(|i| {
                changes[*i].status == FileChangeStatus::Added && changes[*i].old_path.is_none()
            })
            .take(MAX_CANDIDATES)
            .collect();
        if deleted.is_empty() || added.is_empty() {
            return;
        }
        let ask = |args: &[&str], lines: Vec<String>| -> Option<Vec<String>> {
            let input = lines.join("\n").into_bytes();
            let out = self
                .run_with(path, args.iter().copied(), None, Some(input))
                .ok()
                .filter(Output::ok)?;
            Some(
                String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .map(str::to_owned)
                    .collect(),
            )
        };
        // Names with a newline in them cannot go through a line-based question: left alone.
        let plain = |i: &usize| !changes[*i].path.contains('\n');
        let deleted: Vec<usize> = deleted.into_iter().filter(plain).collect();
        let added: Vec<usize> = added.into_iter().filter(plain).collect();
        let Some(old_ids) = ask(
            &["cat-file", "--batch-check=%(objectname) %(objecttype)"],
            deleted
                .iter()
                .map(|i| format!("{baseline}:{}", changes[*i].path))
                .collect(),
        ) else {
            return;
        };
        let Some(new_ids) = ask(
            &["hash-object", "--stdin-paths"],
            added.iter().map(|i| changes[*i].path.clone()).collect(),
        ) else {
            return;
        };
        if old_ids.len() != deleted.len() || new_ids.len() != added.len() {
            return;
        }
        let mut taken = std::collections::HashSet::new();
        let mut renames = Vec::new();
        for (d, old) in deleted.iter().zip(&old_ids) {
            let Some(id) = old.strip_suffix(" blob") else {
                continue;
            };
            let found = added
                .iter()
                .zip(&new_ids)
                .find(|(a, new)| *new == id && !taken.contains(*a));
            if let Some((a, _)) = found {
                taken.insert(*a);
                renames.push((*d, *a));
            }
        }
        for (d, a) in &renames {
            let (old_path, new) = (changes[*d].path.clone(), changes[*a].path.clone());
            changes[*a] = FileChange {
                path: new,
                old_path: Some(old_path),
                status: FileChangeStatus::Renamed,
                additions: Some(0),
                deletions: Some(0),
                binary: false,
            };
        }
        let gone: std::collections::HashSet<usize> = renames.iter().map(|(d, _)| *d).collect();
        let mut index = 0;
        changes.retain(|_| {
            let keep = !gone.contains(&index);
            index += 1;
            keep
        });
    }

    /// The commit and the changed paths of a checkout.
    fn snapshot(&self, cwd: &Path) -> Result<Snapshot, WorktreeError> {
        let head = self.run_ok(cwd, ["rev-parse", "HEAD"])?.text();
        let status = self.run_ok(
            cwd,
            ["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        )?;
        let mut dirty = Vec::new();
        let mut fields = status.stdout.split(|b| *b == 0).filter(|f| !f.is_empty());
        while let Some(entry) = fields.next() {
            let text = String::from_utf8_lossy(entry);
            let (code, path) = text.split_at(3.min(text.len()));
            if code.starts_with(['R', 'C']) {
                // A rename lists the old path as the next field.
                if let Some(old) = fields.next() {
                    dirty.push(String::from_utf8_lossy(old).into_owned());
                }
            }
            dirty.push(path.to_owned());
        }
        dirty.sort();
        Ok(Snapshot { head, dirty })
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

/// Where the project's checkout stands: its commit and every path with changes (untracked
/// files included).
#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    head: String,
    dirty: Vec<String>,
}

/// The paths a failed `git apply --check` named.
fn conflicted_paths(stderr: &str) -> Vec<String> {
    let mut paths: Vec<String> = stderr
        .lines()
        .filter_map(|line| line.strip_prefix("error: "))
        .filter_map(|rest| {
            if let Some(rest) = rest.strip_prefix("patch failed: ") {
                rest.rsplit_once(':').map(|(path, _)| path.to_owned())
            } else {
                rest.split_once(": ").map(|(path, _)| path.to_owned())
            }
        })
        .collect();
    paths.sort();
    paths.dedup();
    paths.truncate(MAX_LISTED_FILES);
    paths
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

/// NUL-separated fields exactly as printed, empty ones kept (`--numstat -z` has them).
fn nul_separated_raw(output: &Output) -> Vec<String> {
    let mut parts: Vec<String> = output
        .stdout
        .split(|b| *b == 0)
        .map(|part| String::from_utf8_lossy(part).into_owned())
        .collect();
    // The output ends with a NUL, which leaves one empty field after it.
    if parts.last().is_some_and(String::is_empty) {
        parts.pop();
    }
    parts
}

/// Joins `--name-status -z` (what happened to each path) with `--numstat -z` (how many lines).
fn parse_changes(status: &[String], numstat: &[String]) -> Vec<FileChange> {
    let mut counts: std::collections::HashMap<String, (Option<u32>, Option<u32>)> =
        std::collections::HashMap::new();
    let mut index = 0;
    while index < numstat.len() {
        let mut fields = numstat[index].splitn(3, '\t');
        let (added, deleted, path) = (fields.next(), fields.next(), fields.next());
        let number = |text: Option<&str>| text.and_then(|t| t.parse::<u32>().ok());
        let (adds, dels) = (number(added), number(deleted));
        match path {
            // A rename: the path is empty here and the old and the new one follow.
            Some("") => {
                if let Some(new) = numstat.get(index + 2) {
                    counts.insert(new.clone(), (adds, dels));
                }
                index += 3;
            }
            Some(path) => {
                counts.insert(path.to_owned(), (adds, dels));
                index += 1;
            }
            None => index += 1,
        }
    }
    let mut changes = Vec::new();
    let mut index = 0;
    while index < status.len() && changes.len() < MAX_CHANGESET_FILES {
        let code = status[index].as_str();
        let renamed = code.starts_with('R') || code.starts_with('C');
        let (old_path, path) = if renamed {
            (
                status.get(index + 1).cloned(),
                status.get(index + 2).cloned(),
            )
        } else {
            (None, status.get(index + 1).cloned())
        };
        index += if renamed { 3 } else { 2 };
        let Some(path) = path else { break };
        let kind = match code.chars().next() {
            Some('A' | 'C') => FileChangeStatus::Added,
            Some('D') => FileChangeStatus::Deleted,
            Some('R') => FileChangeStatus::Renamed,
            _ => FileChangeStatus::Modified,
        };
        let (additions, deletions) = counts.get(&path).copied().unwrap_or((None, None));
        changes.push(FileChange {
            binary: additions.is_none() && deletions.is_none(),
            path,
            old_path,
            status: kind,
            additions,
            deletions,
        });
    }
    changes
}

/// A file that is new in the worktree and not yet known to Git, with its line count when it is
/// a small text file. `None` if it is not there any more.
fn new_file_change(root: &Path, file: String) -> Option<FileChange> {
    const MAX_COUNTED: u64 = 1024 * 1024;
    let full = root.join(&file);
    let metadata = full.symlink_metadata().ok()?;
    let (additions, binary) = if metadata.file_type().is_symlink() {
        (Some(1), false)
    } else if metadata.is_file() && metadata.len() <= MAX_COUNTED {
        let bytes = fs::read(&full).ok()?;
        if bytes.contains(&0) {
            (None, true)
        } else {
            let pieces = bytes.split(|b| *b == b'\n').count();
            let lines = if bytes.is_empty() {
                0
            } else if bytes.ends_with(b"\n") {
                pieces - 1
            } else {
                pieces
            };
            (Some(u32::try_from(lines).unwrap_or(u32::MAX)), false)
        }
    } else {
        (None, false)
    };
    Some(FileChange {
        path: file,
        old_path: None,
        status: FileChangeStatus::Added,
        additions,
        deletions: additions.map(|_| 0),
        binary,
    })
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

    fn head_commit(&self, path: &Path) -> Result<String, WorktreeError> {
        Ok(self.run_ok(path, ["rev-parse", "HEAD"])?.text())
    }

    fn changed_files(
        &self,
        path: &Path,
        from: &str,
        to: &str,
    ) -> Result<Vec<FileChange>, WorktreeError> {
        Self::require_commit_id(from)?;
        Self::require_commit_id(to)?;
        if from == to {
            return Ok(Vec::new());
        }
        let status = self.run_ok(
            path,
            [
                "diff",
                "--name-status",
                "-z",
                "-M",
                "--no-ext-diff",
                from,
                to,
                "--",
            ],
        )?;
        let numstat = self.run_ok(
            path,
            [
                "diff",
                "--numstat",
                "-z",
                "-M",
                "--no-ext-diff",
                from,
                to,
                "--",
            ],
        )?;
        Ok(parse_changes(
            &nul_separated_raw(&status),
            &nul_separated_raw(&numstat),
        ))
    }

    fn diff_between(
        &self,
        path: &Path,
        from: &str,
        to: &str,
        file: Option<&str>,
        max_bytes: usize,
    ) -> Result<String, WorktreeError> {
        Self::require_commit_id(from)?;
        Self::require_commit_id(to)?;
        let mut args: Vec<&str> = vec!["diff", "--no-color", "--no-ext-diff", "-M", from, to, "--"];
        if let Some(file) = file {
            Self::require_plain_path(file)?;
            args.push(file);
        }
        let output = self.run_ok(path, args)?;
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

    fn working_changes(
        &self,
        path: &Path,
        baseline: &str,
        only: Option<&[String]>,
    ) -> Result<Vec<FileChange>, WorktreeError> {
        Self::require_commit_id(baseline)?;
        self.require_inside_root(path)?;
        if let Some(only) = only {
            if only.is_empty() {
                return Ok(Vec::new());
            }
            for file in only {
                Self::require_relative_path(file)?;
            }
        }
        let specs: Vec<&str> =
            only.map_or_else(Vec::new, |o| o.iter().map(String::as_str).collect());
        // `--literal-pathspecs`: a file called `*.ts` is that file, not a pattern.
        let tracked = |kind: &str| -> Result<Output, WorktreeError> {
            let mut args = vec![
                "--literal-pathspecs",
                "diff",
                kind,
                "-z",
                "-M",
                "--no-ext-diff",
                baseline,
                "--",
            ];
            args.extend(&specs);
            self.run_ok(path, args)
        };
        let mut changes = parse_changes(
            &nul_separated_raw(&tracked("--name-status")?),
            &nul_separated_raw(&tracked("--numstat")?),
        );
        let mut args = vec![
            "--literal-pathspecs",
            "ls-files",
            "--others",
            "--exclude-standard",
            "-z",
            "--",
        ];
        args.extend(&specs);
        let untracked = nul_separated(&self.run_ok(path, args)?);
        for file in untracked {
            if changes.len() >= MAX_CHANGESET_FILES {
                break;
            }
            // Gone again between Git's listing and the read: not a change any more.
            if let Some(change) = new_file_change(path, file) {
                changes.push(change);
            }
        }
        if only.is_none() {
            self.pair_renames(path, baseline, &mut changes);
        }
        changes.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(changes)
    }

    fn working_diff(
        &self,
        path: &Path,
        baseline: &str,
        file: Option<&str>,
        max_bytes: usize,
    ) -> Result<String, WorktreeError> {
        Self::require_commit_id(baseline)?;
        self.require_inside_root(path)?;
        if let Some(file) = file {
            Self::require_relative_path(file)?;
        }
        let specs: Vec<&str> = file.into_iter().collect();
        let mut args = vec![
            "--literal-pathspecs",
            "diff",
            "--no-color",
            "--no-ext-diff",
            "-M",
            baseline,
            "--",
        ];
        args.extend(&specs);
        let mut text = String::from_utf8_lossy(&self.run_ok(path, args)?.stdout).into_owned();
        // New files Git does not know yet are changes too: shown as what they add.
        let mut args = vec![
            "--literal-pathspecs",
            "ls-files",
            "--others",
            "--exclude-standard",
            "-z",
            "--",
        ];
        args.extend(&specs);
        for new in nul_separated(&self.run_ok(path, args)?)
            .into_iter()
            .take(MAX_CHANGESET_FILES)
        {
            if text.len() >= max_bytes {
                break;
            }
            // `--no-index` exits 1 when the files differ, which is the point.
            let shown = self.run(
                path,
                [
                    "diff",
                    "--no-index",
                    "--no-color",
                    "--no-ext-diff",
                    "--",
                    "/dev/null",
                    new.as_str(),
                ],
                None,
            )?;
            if matches!(shown.code, Some(0 | 1)) {
                text.push_str(&String::from_utf8_lossy(&shown.stdout));
            }
        }
        if text.len() > max_bytes {
            let mut cut = max_bytes;
            while !text.is_char_boundary(cut) {
                cut -= 1;
            }
            text.truncate(cut);
        }
        Ok(text)
    }

    fn read_file(
        &self,
        path: &Path,
        file: &str,
        max_bytes: usize,
    ) -> Result<LiveFile, WorktreeError> {
        self.require_inside_root(path)?;
        if !is_plain_relative_path(file) {
            return Err(WorktreeError::InvalidIdentifier(file.to_owned()));
        }
        let gone = |kind| LiveFile {
            path: file.to_owned(),
            kind,
            content: None,
            size: None,
            truncated: false,
        };
        // Walk down from the worktree: a link anywhere on the way could lead out of it.
        let parts: Vec<&str> = file.split('/').collect();
        let mut current = path.to_path_buf();
        for (index, part) in parts.iter().enumerate() {
            current.push(part);
            let metadata = match current.symlink_metadata() {
                Ok(m) => m,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(gone(LiveFileKind::Deleted))
                }
                // A file where a folder was expected, and the like: it is not there.
                Err(e) if e.kind() == std::io::ErrorKind::NotADirectory => {
                    return Ok(gone(LiveFileKind::Deleted))
                }
                Err(e) => return Err(WorktreeError::Git(e.to_string())),
            };
            let last = index + 1 == parts.len();
            if metadata.file_type().is_symlink() {
                if last {
                    return Ok(gone(LiveFileKind::Symlink));
                }
                return Err(WorktreeError::OutsideRoot(file.to_owned()));
            }
            if last && !metadata.is_file() {
                return Ok(gone(LiveFileKind::NotFile));
            }
            if !last && !metadata.is_dir() {
                return Ok(gone(LiveFileKind::Deleted));
            }
        }
        let size = current.metadata().map(|m| m.len()).ok();
        let mut bytes = Vec::new();
        let opened = fs::File::open(&current);
        let Ok(opened) = opened else {
            // Removed between the look and the read.
            return Ok(gone(LiveFileKind::Deleted));
        };
        opened
            .take(max_bytes as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| WorktreeError::Git(e.to_string()))?;
        let truncated = bytes.len() > max_bytes;
        bytes.truncate(max_bytes);
        if bytes.contains(&0) {
            return Ok(LiveFile {
                path: file.to_owned(),
                kind: LiveFileKind::Binary,
                content: None,
                size,
                truncated: false,
            });
        }
        // A cut in the middle of a character is not a reason to show a replacement mark.
        let mut text = String::from_utf8_lossy(&bytes).into_owned();
        if truncated {
            while text.ends_with('\u{FFFD}') {
                text.pop();
            }
        }
        Ok(LiveFile {
            path: file.to_owned(),
            kind: LiveFileKind::Text,
            content: Some(text),
            size,
            truncated,
        })
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

    fn apply_to_working_tree(
        &self,
        toplevel: &Path,
        base_branch: &str,
        branch: &str,
    ) -> Result<ApplyOutcome, WorktreeError> {
        Self::require_execution_branch(branch)?;
        self.require_valid_base(toplevel, base_branch)?;
        let _write = self.lock();
        // Prepare: the checkout is on the base branch and no operation of the user's is half-done.
        if self.current_branch(toplevel)?.as_deref() != Some(base_branch) {
            return Err(WorktreeError::NotMergeable(
                "the project checkout is no longer on the base branch".to_owned(),
            ));
        }
        for marker in ["MERGE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD"] {
            if self.ref_exists(toplevel, marker)? {
                return Err(WorktreeError::NotMergeable(
                    "the project checkout is in the middle of a Git operation".to_owned(),
                ));
            }
        }
        // Snapshot: where the project is before anything is written.
        let before = self.snapshot(toplevel)?;
        // The changes are what the branch did since it forked from the base, whatever the base
        // has done since: the same set a merge would bring, as a patch.
        let fork = self
            .run_ok(toplevel, ["merge-base", &head(base_branch), &head(branch)])?
            .text();
        Self::require_commit_id(&fork)?;
        let files = nul_separated(&self.run_ok(
            toplevel,
            [
                "diff",
                "--name-only",
                "-z",
                "--no-renames",
                "--no-ext-diff",
                &fork,
                &head(branch),
                "--",
            ],
        )?);
        if files.is_empty() {
            return Ok(ApplyOutcome::Applied { files });
        }
        // Detect conflicts with the user's own work: a file the workflow changes that the
        // working tree already has changes (or an untracked file) in is never overwritten.
        let touched: std::collections::HashSet<&str> =
            before.dirty.iter().map(String::as_str).collect();
        let overlap: Vec<String> = files
            .iter()
            .filter(|f| touched.contains(f.as_str()))
            .take(MAX_LISTED_FILES)
            .cloned()
            .collect();
        if !overlap.is_empty() {
            return Ok(ApplyOutcome::Conflict { files: overlap });
        }
        let patch = self
            .run_ok(
                toplevel,
                [
                    "diff",
                    "--binary",
                    "--full-index",
                    "--no-color",
                    "--no-ext-diff",
                    "--no-renames",
                    &fork,
                    &head(branch),
                    "--",
                ],
            )?
            .stdout;
        // Validate: the whole patch is checked against the working tree before a byte is written.
        let check = self.run_with(
            toplevel,
            ["apply", "--check", "--whitespace=nowarn", "-"],
            None,
            Some(patch.clone()),
        )?;
        if !check.ok() {
            let mut failed = conflicted_paths(&check.stderr);
            if failed.is_empty() {
                failed = files.iter().take(MAX_LISTED_FILES).cloned().collect();
            }
            return Ok(ApplyOutcome::Conflict { files: failed });
        }
        // Revalidate: nothing may have moved since the snapshot.
        if self.snapshot(toplevel)? != before {
            return Err(WorktreeError::NotMergeable(
                "the project changed while the changes were being prepared".to_owned(),
            ));
        }
        // Apply: working tree only. `git apply` writes all of the patch or none of it, and never
        // touches the index, a ref or the history.
        let applied = self.run_with(
            toplevel,
            ["apply", "--whitespace=nowarn", "-"],
            None,
            Some(patch),
        )?;
        if !applied.ok() {
            return Err(WorktreeError::Git(first_line(&applied.stderr)));
        }
        self.verify_applied(toplevel, &before, &files)?;
        Ok(ApplyOutcome::Applied { files })
    }

    fn revert_from_working_tree(
        &self,
        toplevel: &Path,
        base_branch: &str,
        branch: &str,
        expected_head: &str,
    ) -> Result<ApplyOutcome, WorktreeError> {
        Self::require_execution_branch(branch)?;
        self.require_valid_base(toplevel, base_branch)?;
        Self::require_commit_id(expected_head)?;
        let _write = self.lock();
        if self.current_branch(toplevel)?.as_deref() != Some(base_branch) {
            return Err(WorktreeError::NotMergeable(
                "the project checkout is no longer on the base branch".to_owned(),
            ));
        }
        for marker in ["MERGE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD"] {
            if self.ref_exists(toplevel, marker)? {
                return Err(WorktreeError::NotMergeable(
                    "the project checkout is in the middle of a Git operation".to_owned(),
                ));
            }
        }
        let before = self.snapshot(toplevel)?;
        // With a new commit the applied changes may be part of it: they are no longer only
        // uncommitted files, and taking them out would rewrite what the user committed.
        if before.head != expected_head {
            return Err(WorktreeError::NotMergeable(
                "the project has new commits since the changes were applied".to_owned(),
            ));
        }
        let (files, patch) = self.branch_patch(toplevel, base_branch, branch)?;
        if files.is_empty() {
            return Ok(ApplyOutcome::Applied { files });
        }
        // The patch, reversed, is checked against the working tree as a whole: any applied file the
        // user changed since no longer matches it, and then nothing at all is written.
        let check = self.run_with(
            toplevel,
            ["apply", "-R", "--check", "--whitespace=nowarn", "-"],
            None,
            Some(patch.clone()),
        )?;
        if !check.ok() {
            let mut failed = conflicted_paths(&check.stderr);
            if failed.is_empty() {
                failed = files.iter().take(MAX_LISTED_FILES).cloned().collect();
            }
            return Ok(ApplyOutcome::Conflict { files: failed });
        }
        if self.snapshot(toplevel)? != before {
            return Err(WorktreeError::NotMergeable(
                "the project changed while the changes were being taken back".to_owned(),
            ));
        }
        let reverted = self.run_with(
            toplevel,
            ["apply", "-R", "--whitespace=nowarn", "-"],
            None,
            Some(patch),
        )?;
        if !reverted.ok() {
            return Err(WorktreeError::Git(first_line(&reverted.stderr)));
        }
        let after = self.snapshot(toplevel)?;
        if after.head != before.head {
            return Err(WorktreeError::Git(
                "the project's HEAD moved while taking the changes back".to_owned(),
            ));
        }
        let dirty: std::collections::HashSet<&str> =
            after.dirty.iter().map(String::as_str).collect();
        if let Some(left) = files.iter().find(|f| dirty.contains(f.as_str())) {
            return Err(WorktreeError::Git(format!(
                "taken back, but {left} still differs from the commit"
            )));
        }
        Ok(ApplyOutcome::Applied { files })
    }

    fn restore(&self, toplevel: &Path, path: &Path, branch: &str) -> Result<(), WorktreeError> {
        Self::require_execution_branch(branch)?;
        self.require_inside_root(path)?;
        let _write = self.lock();
        if path.symlink_metadata().is_ok() {
            return Err(WorktreeError::Collision(path.display().to_string()));
        }
        if !self.ref_exists(toplevel, &head(branch))? {
            return Err(WorktreeError::NotFound(branch.to_owned()));
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| WorktreeError::Git(e.to_string()))?;
        }
        // The branch by name (it is one Atlas generated): spelled as a ref, Git would detach.
        self.run_ok(
            toplevel,
            [
                OsStr::new("worktree"),
                OsStr::new("add"),
                path.as_os_str(),
                OsStr::new(branch),
            ],
        )?;
        Ok(())
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
