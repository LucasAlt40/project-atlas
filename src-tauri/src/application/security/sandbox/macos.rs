//! The macOS sandbox: the runtime's CLI runs under `sandbox-exec` with a profile made for the
//! execution (ADR 0038). `sandbox-exec` is deprecated by Apple and still the way the system's own
//! tools confine a process; it is checked for at every start and its absence is said, never
//! hidden.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use super::cli::{self, ConfinementRegistry, RuntimeConfinement};
use super::profile::{self, GitLayout, ProfileInput};
use super::{Prepared, SandboxProvider, SandboxRefusal, SandboxRequest};
use crate::application::process::ProcessSpec;
use crate::domain::security::{Permission, SandboxMode, SandboxSettings};

const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";
/// Profiles and temporary folders of executions older than this are removed.
const KEEP: Duration = Duration::from_secs(24 * 60 * 60);

pub struct MacosSandbox {
    /// What each runtime's CLI needs (**Strategy**, found by program name).
    confinements: ConfinementRegistry,
    settings: Arc<dyn Fn() -> SandboxSettings + Send + Sync>,
    /// Where profiles and the executions' temporary folders live (Atlas's own data folder).
    work_dir: PathBuf,
    tool: PathBuf,
    home: Option<PathBuf>,
}

impl MacosSandbox {
    pub fn new(
        settings: Arc<dyn Fn() -> SandboxSettings + Send + Sync>,
        work_dir: PathBuf,
        confinements: ConfinementRegistry,
    ) -> Self {
        Self {
            confinements,
            settings,
            work_dir,
            tool: PathBuf::from(SANDBOX_EXEC),
            home: std::env::var_os("HOME").map(PathBuf::from),
        }
    }

    #[cfg(test)]
    #[must_use]
    pub fn with_tool(mut self, tool: PathBuf) -> Self {
        self.tool = tool;
        self
    }

    #[cfg(test)]
    #[must_use]
    pub fn with_home(mut self, home: PathBuf) -> Self {
        self.home = Some(home);
        self
    }

    /// Whether this system can confine a process: macOS, with `sandbox-exec` where it should be.
    pub fn available(&self) -> bool {
        cfg!(target_os = "macos") && self.tool.is_file()
    }

    /// The current settings, as the person set them.
    pub fn settings(&self) -> SandboxSettings {
        (self.settings)()
    }

    fn sweep(&self) {
        for sub in ["profiles", "tmp"] {
            let Ok(entries) = fs::read_dir(self.work_dir.join(sub)) else {
                continue;
            };
            for entry in entries.flatten() {
                let old = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| SystemTime::now().duration_since(t).ok())
                    .is_some_and(|age| age > KEEP);
                if old {
                    let _ =
                        fs::remove_dir_all(entry.path()).or_else(|_| fs::remove_file(entry.path()));
                }
            }
        }
    }

    fn confine(
        &self,
        mut spec: ProcessSpec,
        request: &SandboxRequest<'_>,
        strategy: &dyn RuntimeConfinement,
        program: &Path,
        settings: SandboxSettings,
    ) -> Result<Prepared, SandboxRefusal> {
        let failed = |why: String| SandboxRefusal::Failed(why);
        let home = self
            .home
            .clone()
            .ok_or_else(|| SandboxRefusal::Unavailable("no_home".to_owned()))?;
        let cwd = spec
            .cwd
            .clone()
            .ok_or_else(|| SandboxRefusal::Unavailable("no_working_dir".to_owned()))?;
        // The profile talks about real paths: `/tmp` is `/private/tmp`, and a symlinked project
        // folder is the folder it points to.
        let worktree = cwd
            .canonicalize()
            .map_err(|e| failed(format!("working directory: {e}")))?;
        let needs = strategy.needs();
        let uid = uid_of(&home);

        let execution = request.scope.execution_id.as_str();
        let id: String = execution
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let tmp = self.work_dir.join("tmp").join(&id);
        make_private_dir(&tmp).map_err(|e| failed(format!("temporary folder: {e}")))?;
        let tmp = tmp.canonicalize().map_err(|e| failed(e.to_string()))?;

        let mut write_dirs = vec![tmp.clone()];
        write_dirs.extend(needs.dirs.iter().map(|d| home.join(d)));
        write_dirs.extend(
            needs
                .tmp_dirs
                .iter()
                .map(|d| PathBuf::from(d.replace("{uid}", &uid))),
        );
        if settings.dev_caches {
            write_dirs.extend(cli::DEV_CACHES.iter().map(|d| home.join(d)));
        }
        let (git, deny_write) = git_layout(&worktree);
        // A CLI with a sandbox of its own cannot apply it inside ours: it hands the confinement
        // over to the profile, which then also carries what that sandbox enforced.
        let adapted = strategy.adapt(&mut spec.args);
        let input = ProfileInput {
            worktree,
            // A run in its own worktree is reviewed before anything reaches the project; one in
            // the project itself writes only if the policy lets it. A run the CLI itself held to
            // read-only stays so.
            worktree_writable: !adapted.read_only
                && (request.scope.isolated
                    || request.policy.filesystem.write == Permission::Allowed),
            write_dirs,
            write_file_prefixes: needs.file_prefixes.iter().map(|f| home.join(f)).collect(),
            git,
            deny_write,
            deny_read: cli::SECRET_STORES.iter().map(|d| home.join(d)).collect(),
        };
        let profile = profile::build(&input).map_err(|e| failed(e.0))?;

        let profiles = self.work_dir.join("profiles");
        make_private_dir(&profiles).map_err(|e| failed(format!("profile folder: {e}")))?;
        let file = profiles.join(format!("{id}-{}.sb", crate::application::support::now_ms()));
        write_private(&file, &profile.text).map_err(|e| failed(format!("profile: {e}")))?;
        self.sweep();

        let mut args = Vec::with_capacity(profile.params.len() * 2 + 3 + spec.args.len());
        for (name, value) in &profile.params {
            args.push("-D".to_owned());
            args.push(format!("{name}={value}"));
        }
        args.push("-f".to_owned());
        args.push(file.to_string_lossy().into_owned());
        args.push(program.to_string_lossy().into_owned());
        args.append(&mut spec.args);

        spec.env.retain(|(name, _)| name != "TMPDIR");
        spec.env
            .push(("TMPDIR".to_owned(), tmp.to_string_lossy().into_owned()));
        spec.program = self.tool.to_string_lossy().into_owned();
        spec.args = args;
        let mut notes = vec!["sandbox:confined".to_owned()];
        notes.extend(adapted.note);
        Ok(Prepared { spec, notes })
    }
}

impl SandboxProvider for MacosSandbox {
    fn prepare(
        &self,
        spec: ProcessSpec,
        request: &SandboxRequest<'_>,
    ) -> Result<Prepared, SandboxRefusal> {
        let settings = self.settings();
        if settings.mode == SandboxMode::Off {
            return Ok(Prepared {
                spec,
                notes: Vec::new(),
            });
        }
        // Nothing to confine, and nothing to say: the process port reports a missing program.
        let Some(program) = request.program_path.as_deref() else {
            return Ok(Prepared {
                spec,
                notes: Vec::new(),
            });
        };
        let cannot = |why: &str| -> Result<Prepared, SandboxRefusal> {
            if settings.mode == SandboxMode::Required {
                Err(SandboxRefusal::Unavailable(why.to_owned()))
            } else {
                Ok(Prepared {
                    spec: spec.clone(),
                    notes: vec![format!("sandbox:unavailable:{why}")],
                })
            }
        };
        if !self.available() {
            return cannot(if cfg!(target_os = "macos") {
                "no_sandbox_exec"
            } else {
                "platform"
            });
        }
        let Some(strategy) = self.confinements.find(&spec.program) else {
            return cannot("unknown_runtime");
        };
        match self.confine(spec.clone(), request, strategy, program, settings) {
            Ok(prepared) => Ok(prepared),
            Err(SandboxRefusal::Unavailable(why)) => cannot(&why),
            // A profile that could not be made is never skipped quietly, in either mode.
            Err(failed) => Err(failed),
        }
    }
}

/// The numeric id of the user, from who owns their home (no system call needed).
fn uid_of(home: &Path) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let Ok(meta) = fs::metadata(home) {
            return meta.uid().to_string();
        }
    }
    let _ = home;
    String::new()
}

fn make_private_dir(dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn write_private(file: &Path, text: &str) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut out = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(file)?;
        out.write_all(text.as_bytes())
    }
    #[cfg(not(unix))]
    {
        fs::write(file, text)
    }
}

/// The Git data of a working directory, and what must stay unwritable inside it.
///
/// A linked worktree's `.git` is a file naming `<repo>/.git/worktrees/<id>`: a commit there
/// writes into the main repository's `.git`, so exactly the narrow set the profile lists is
/// opened. A project that is its own working directory has a `.git` folder inside the writable
/// folder: its `hooks` (code that runs outside the profile) and `config` are closed again.
fn git_layout(worktree: &Path) -> (Option<GitLayout>, Vec<PathBuf>) {
    let dot_git = worktree.join(".git");
    if dot_git.is_dir() {
        return (None, vec![dot_git.join("hooks"), dot_git.join("config")]);
    }
    let Ok(text) = fs::read_to_string(&dot_git) else {
        return (None, Vec::new());
    };
    let Some(named) = text.trim().strip_prefix("gitdir:").map(str::trim) else {
        return (None, Vec::new());
    };
    let worktree_dir = {
        let path = Path::new(named);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            worktree.join(path)
        }
    };
    let Ok(worktree_dir) = worktree_dir.canonicalize() else {
        return (None, Vec::new());
    };
    let common_dir = fs::read_to_string(worktree_dir.join("commondir"))
        .ok()
        .and_then(|rel| worktree_dir.join(rel.trim()).canonicalize().ok())
        .or_else(|| {
            worktree_dir
                .parent()
                .and_then(Path::parent)
                .map(Path::to_path_buf)
        });
    match common_dir {
        Some(common_dir) => (
            Some(GitLayout {
                worktree_dir,
                common_dir,
            }),
            Vec::new(),
        ),
        None => (None, Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    use super::*;
    use crate::application::process::ExecutionScope;
    use crate::domain::security::SecurityPolicy;

    struct World {
        base: PathBuf,
        home: PathBuf,
        program: PathBuf,
    }

    fn world(name: &str) -> World {
        let base = std::env::temp_dir()
            .join(format!("atlas-sbx-{name}-{}", std::process::id()))
            .canonicalize_or_self();
        let _ = fs::remove_dir_all(&base);
        let home = base.join("home");
        for dir in [".claude", ".ssh", ".aws"] {
            fs::create_dir_all(home.join(dir)).unwrap();
        }
        fs::write(home.join(".ssh/id_test"), "secret").unwrap();
        let bin = base.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let program = bin.join("claude");
        fs::write(&program, "#!/bin/sh\nexec /bin/sh \"$@\"\n").unwrap();
        make_executable(&program);
        World {
            base: base.canonicalize().unwrap(),
            home,
            program,
        }
    }

    trait OrSelf {
        fn canonicalize_or_self(self) -> PathBuf;
    }

    impl OrSelf for PathBuf {
        fn canonicalize_or_self(self) -> PathBuf {
            fs::create_dir_all(&self).ok();
            self.canonicalize().unwrap_or(self)
        }
    }

    fn make_executable(path: &Path) {
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn settings(mode: SandboxMode, dev_caches: bool) -> SandboxSettings {
        SandboxSettings { mode, dev_caches }
    }

    fn provider(w: &World, settings: SandboxSettings) -> MacosSandbox {
        MacosSandbox::new(
            Arc::new(move || settings),
            w.base.join("data"),
            crate::application::runtimes::confinement::default_confinements(),
        )
        .with_home(w.home.clone())
    }

    fn spec(program: &str, cwd: &Path, args: &[&str]) -> ProcessSpec {
        ProcessSpec {
            program: program.to_owned(),
            args: args.iter().map(|a| (*a).to_owned()).collect(),
            stdin: None,
            cwd: Some(cwd.to_path_buf()),
            env: vec![
                ("KEEP".to_owned(), "1".to_owned()),
                ("TMPDIR".to_owned(), "/elsewhere".to_owned()),
            ],
            timeout: Duration::from_secs(30),
            context: crate::application::process::ProcessContext::Probe,
            terminal: None,
        }
    }

    fn prepare(
        sandbox: &MacosSandbox,
        w: &World,
        spec: ProcessSpec,
        policy: &SecurityPolicy,
        isolated: bool,
    ) -> Result<Prepared, SandboxRefusal> {
        let mut scope = ExecutionScope::for_tests();
        scope.isolated = isolated;
        sandbox.prepare(
            spec,
            &SandboxRequest {
                policy,
                scope: &scope,
                program_path: Some(w.program.clone()),
            },
        )
    }

    fn run(prepared: &Prepared) -> String {
        let spec = &prepared.spec;
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .current_dir(spec.cwd.as_ref().unwrap());
        for (k, v) in &spec.env {
            command.env(k, v);
        }
        let out = command.output().unwrap();
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    }

    // ── what the provider decides, anywhere ──────────────────────────────────────────

    #[test]
    fn with_the_sandbox_off_the_request_goes_through_untouched_and_says_nothing() {
        let w = world("off");
        let given = spec("claude", &w.base, &["-c", "true"]);

        let prepared = prepare(
            &provider(&w, settings(SandboxMode::Off, true)),
            &w,
            given.clone(),
            &SecurityPolicy::developer(),
            true,
        )
        .unwrap();

        assert_eq!(
            (prepared.spec.program, prepared.spec.args),
            (given.program, given.args)
        );
        assert_eq!(prepared.notes, Vec::<String>::new());
    }

    #[test]
    fn a_program_that_is_not_installed_is_left_for_the_process_port_to_report() {
        let w = world("missing");
        let scope = ExecutionScope::for_tests();
        let policy = SecurityPolicy::developer();

        let prepared = provider(&w, settings(SandboxMode::Required, true))
            .prepare(
                spec("claude", &w.base, &[]),
                &SandboxRequest {
                    policy: &policy,
                    scope: &scope,
                    program_path: None,
                },
            )
            .unwrap();

        assert_eq!(
            (prepared.spec.program.as_str(), prepared.notes),
            ("claude", Vec::new())
        );
    }

    #[test]
    fn auto_runs_unconfined_and_says_why_while_required_refuses_when_there_is_no_sandbox() {
        let w = world("none");
        let nothing = w.base.join("no-such-sandbox-exec");
        let policy = SecurityPolicy::developer();
        let given = spec("claude", &w.base, &[]);

        let auto = prepare(
            &provider(&w, settings(SandboxMode::Auto, true)).with_tool(nothing.clone()),
            &w,
            given.clone(),
            &policy,
            true,
        )
        .unwrap();
        let required = prepare(
            &provider(&w, settings(SandboxMode::Required, true)).with_tool(nothing),
            &w,
            given,
            &policy,
            true,
        );

        assert_eq!(auto.spec.program, "claude");
        assert!(auto.notes[0].starts_with("sandbox:unavailable"));
        assert!(matches!(required, Err(SandboxRefusal::Unavailable(_))));
    }

    // ── a real `sandbox-exec` (macOS only; no model, no cost) ────────────────────────────

    #[cfg(target_os = "macos")]
    mod real {
        use super::*;

        const PROBE: &str = r#"
touch "$WT/inside"; echo worktree=$?
touch "$OUT/outside"; echo outside=$?
touch "$H/.claude/session"; echo cli_dir=$?
touch "$H/.claude.json.tmp.7"; echo file_sibling=$?
touch "$H/.claudeXjson"; echo file_not_a_sibling=$?
cat "$H/.ssh/id_test" >/dev/null 2>&1; echo read_ssh=$?
touch "$TMPDIR/t"; echo own_tmp=$?
touch /private/tmp/atlas_sbx_probe; echo global_tmp=$?
mkdir -p "$H/.npm" 2>/dev/null; touch "$H/.npm/c" 2>/dev/null; echo dev_cache=$?
touch "$H/.aws/x" 2>/dev/null; echo aws_write=$?
"#;

        fn probe(
            w: &World,
            wt: &Path,
            out: &Path,
            s: SandboxSettings,
            policy: &SecurityPolicy,
            isolated: bool,
        ) -> std::collections::BTreeMap<String, String> {
            fs::create_dir_all(out).unwrap();
            let mut given = spec("claude", wt, &["-c", PROBE]);
            given
                .env
                .push(("WT".to_owned(), wt.to_string_lossy().into_owned()));
            given
                .env
                .push(("OUT".to_owned(), out.to_string_lossy().into_owned()));
            given
                .env
                .push(("H".to_owned(), w.home.to_string_lossy().into_owned()));
            let prepared = prepare(&provider(w, s), w, given, policy, isolated).unwrap();
            assert_eq!(prepared.notes, ["sandbox:confined"]);
            run(&prepared)
                .lines()
                .filter_map(|l| l.split_once('='))
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect()
        }

        #[test]
        fn the_cli_may_write_its_worktree_its_own_folders_and_its_temporary_folder_and_nothing_else(
        ) {
            let w = world("confine");
            let wt = w.base.join("wt");
            fs::create_dir_all(&wt).unwrap();

            let r = probe(
                &w,
                &wt,
                &w.base.join("elsewhere"),
                settings(SandboxMode::Required, true),
                &SecurityPolicy::developer(),
                true,
            );

            assert_eq!(r["worktree"], "0");
            assert_eq!(r["cli_dir"], "0");
            assert_eq!(r["file_sibling"], "0");
            assert_eq!(r["own_tmp"], "0");
            assert_eq!(r["dev_cache"], "0");
            // Everything else is refused, including what only differs by a character.
            assert_eq!(r["outside"], "1");
            assert_eq!(r["file_not_a_sibling"], "1");
            assert_eq!(r["global_tmp"], "1");
            assert_eq!(r["aws_write"], "1");
            assert_eq!(r["read_ssh"], "1");
        }

        #[test]
        fn without_the_developer_caches_a_confined_process_cannot_write_them() {
            let w = world("nocache");
            let wt = w.base.join("wt");
            fs::create_dir_all(&wt).unwrap();

            let r = probe(
                &w,
                &wt,
                &w.base.join("elsewhere"),
                settings(SandboxMode::Required, false),
                &SecurityPolicy::developer(),
                true,
            );

            assert_eq!(r["dev_cache"], "1");
            assert_eq!(r["cli_dir"], "0");
        }

        #[test]
        fn a_read_only_run_in_the_project_itself_cannot_change_the_project() {
            let w = world("readonly");
            let wt = w.base.join("project");
            fs::create_dir_all(&wt).unwrap();

            let r = probe(
                &w,
                &wt,
                &w.base.join("elsewhere"),
                settings(SandboxMode::Required, true),
                &SecurityPolicy::read_only(),
                false,
            );

            assert_eq!(r["worktree"], "1");
            assert_eq!(r["cli_dir"], "0");
        }

        fn git(dir: &Path, args: &[&str]) -> String {
            let out = Command::new("git")
                .args(["-c", "user.email=a@b", "-c", "user.name=a"])
                .args(args)
                .current_dir(dir)
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).trim().to_owned()
        }

        const GIT_PROBE: &str = r#"
echo one > f.txt; git add f.txt; git -c user.email=a@b -c user.name=a commit -qm one; echo commit=$?
git update-ref refs/heads/main HEAD 2>/dev/null; echo main_ref=$?
git config core.hooksPath /x 2>/dev/null; echo config=$?
echo '#!/bin/sh' > "$REPO/.git/hooks/post-commit" 2>/dev/null; echo hook=$?
git branch -q atlas/other 2>/dev/null; echo atlas_branch=$?
git branch -q other 2>/dev/null; echo other_branch=$?
"#;

        fn git_results(
            w: &World,
            wt: &Path,
            repo: &Path,
        ) -> std::collections::BTreeMap<String, String> {
            let mut given = spec("claude", wt, &["-c", GIT_PROBE]);
            given
                .env
                .push(("REPO".to_owned(), repo.to_string_lossy().into_owned()));
            let prepared = prepare(
                &provider(w, settings(SandboxMode::Required, true)),
                w,
                given,
                &SecurityPolicy::developer(),
                true,
            )
            .unwrap();
            run(&prepared)
                .lines()
                .filter_map(|l| l.split_once('='))
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect()
        }

        #[test]
        fn in_a_linked_worktree_a_commit_works_but_hooks_config_and_other_branches_stay_closed() {
            let w = world("git-wt");
            let repo = w.base.join("repo");
            fs::create_dir_all(&repo).unwrap();
            git(&repo, &["init", "-q", "-b", "main"]);
            git(&repo, &["commit", "-q", "--allow-empty", "-m", "init"]);
            let wt = w.base.join("wt");
            git(
                &repo,
                &[
                    "worktree",
                    "add",
                    "-q",
                    wt.to_str().unwrap(),
                    "-b",
                    "atlas/exec-1",
                ],
            );

            let r = git_results(&w, &wt, &repo);

            assert_eq!(r["commit"], "0");
            assert_eq!(
                git(&repo, &["log", "--format=%s", "atlas/exec-1", "-1"]),
                "one"
            );
            assert_eq!(r["atlas_branch"], "0");
            assert_ne!(r["main_ref"], "0");
            assert_ne!(r["config"], "0");
            assert_ne!(r["hook"], "0");
            assert_ne!(r["other_branch"], "0");
            assert!(!repo.join(".git/hooks/post-commit").exists());
            assert!(
                !repo.join("f.txt").exists(),
                "the main working tree is untouched"
            );
        }

        #[test]
        fn in_a_project_that_is_its_own_working_directory_hooks_and_config_are_closed_again() {
            let w = world("git-own");
            let repo = w.base.join("repo");
            fs::create_dir_all(&repo).unwrap();
            git(&repo, &["init", "-q", "-b", "main"]);
            git(&repo, &["commit", "-q", "--allow-empty", "-m", "init"]);

            let r = git_results(&w, &repo, &repo);

            assert_eq!(r["commit"], "0");
            assert_ne!(r["config"], "0");
            assert_ne!(r["hook"], "0");
            assert!(!repo.join(".git/hooks/post-commit").exists());
        }

        #[test]
        fn a_confined_spec_runs_the_program_it_was_given_with_its_arguments_and_the_new_temporary_folder(
        ) {
            let w = world("shape");
            let wt = w.base.join("wt");
            fs::create_dir_all(&wt).unwrap();
            let prepared = prepare(
                &provider(&w, settings(SandboxMode::Auto, true)),
                &w,
                spec("claude", &wt, &["-c", "echo \"$KEEP:$TMPDIR\""]),
                &SecurityPolicy::developer(),
                true,
            )
            .unwrap();

            assert_eq!(prepared.spec.program, "/usr/bin/sandbox-exec");
            assert_eq!(
                prepared.spec.args.iter().rev().nth(1).map(String::as_str),
                Some("-c")
            );
            let output = run(&prepared);
            let (keep, tmp) = output.trim().split_once(':').unwrap();
            assert_eq!(keep, "1");
            assert!(tmp.contains("/data/tmp/"), "{tmp}");
            assert_eq!(
                fs::metadata(tmp).unwrap().permissions().mode() & 0o777,
                0o700
            );
            let profiles: Vec<_> = fs::read_dir(w.base.join("data/profiles"))
                .unwrap()
                .flatten()
                .collect();
            assert_eq!(profiles.len(), 1);
            assert_eq!(
                profiles[0].metadata().unwrap().permissions().mode() & 0o777,
                0o600
            );
        }

        #[test]
        fn codex_runs_without_a_second_sandbox_and_a_read_only_one_cannot_write_the_worktree() {
            let w = world("codex");
            let codex = w.base.join("bin/codex");
            fs::write(
                &codex,
                "#!/bin/sh\ntouch \"$WT/x\"; echo touched=$?; echo \"$@\"\n",
            )
            .unwrap();
            make_executable(&codex);
            let wt = w.base.join("wt");
            fs::create_dir_all(&wt).unwrap();
            let ask = |mode: &str| {
                let mut given = spec("codex", &wt, &["exec", "--sandbox", mode, "--", "p"]);
                given
                    .env
                    .push(("WT".to_owned(), wt.to_string_lossy().into_owned()));
                let scope = ExecutionScope {
                    isolated: true,
                    ..ExecutionScope::for_tests()
                };
                let policy = SecurityPolicy::developer();
                let prepared = provider(&w, settings(SandboxMode::Required, true))
                    .prepare(
                        given,
                        &SandboxRequest {
                            policy: &policy,
                            scope: &scope,
                            program_path: Some(codex.clone()),
                        },
                    )
                    .unwrap();
                let output = run(&prepared);
                let _ = fs::remove_file(wt.join("x"));
                (prepared, output)
            };

            let (writing, wrote) = ask("workspace-write");
            let (reading, read) = ask("read-only");

            assert!(wrote.contains("touched=0") && read.contains("touched=1"));
            for output in [&wrote, &read] {
                assert!(output.contains("--sandbox danger-full-access"), "{output}");
            }
            assert!(writing
                .notes
                .contains(&"sandbox:replaces_cli_sandbox:codex".to_owned()));
            assert!(reading.notes.contains(&"sandbox:confined".to_owned()));
        }
    }
}
