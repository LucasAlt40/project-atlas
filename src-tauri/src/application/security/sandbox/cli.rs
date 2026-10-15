//! What each runtime's CLI needs to write, from the spike of 2026-10-15
//! (`docs/architecture/spikes/macos-sandbox.md`): what it creates or changes when it starts.
//! Paths are relative to the user's home. A CLI that is not here is not confined (it is not a
//! known runtime), and a required sandbox refuses it.

/// One CLI's needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CliNeeds {
    /// Folders it writes in (the folder and everything in it).
    pub dirs: &'static [&'static str],
    /// Files it writes, with the siblings it makes beside them (`.claude.json.tmp.1234`).
    pub file_prefixes: &'static [&'static str],
    /// Folders outside the home and the execution's own temporary folder, `{uid}` being the
    /// user's numeric id.
    pub tmp_dirs: &'static [&'static str],
}

/// The needs of the CLI called `program` (a bare name or a path).
pub fn needs(program: &str) -> Option<CliNeeds> {
    let name = program.rsplit(['/', '\\']).next().unwrap_or(program);
    let name = name.strip_suffix(".exe").unwrap_or(name);
    Some(match name {
        "claude" => CliNeeds {
            dirs: &[".claude", "Library/Caches/claude-cli-nodejs"],
            file_prefixes: &[".claude.json"],
            tmp_dirs: &["/private/tmp/claude-{uid}"],
        },
        "codex" => CliNeeds {
            dirs: &[".codex"],
            file_prefixes: &[],
            tmp_dirs: &[],
        },
        "opencode" => CliNeeds {
            dirs: &[
                ".local/share/opencode",
                ".local/state/opencode",
                ".cache/opencode",
            ],
            file_prefixes: &[],
            tmp_dirs: &[],
        },
        // Both keep their state under `~/.gemini`. `agy` also updates its own binary in
        // `~/.local/bin`: that stays refused on purpose, the sandbox keeps a CLI from replacing
        // itself.
        "gemini" | "agy" => CliNeeds {
            dirs: &[".gemini"],
            file_prefixes: &[],
            tmp_dirs: &[],
        },
        _ => return None,
    })
}

/// What changes in a CLI's arguments when the operating system already confines it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Adapted {
    /// The CLI's own restriction was lifted, and it was a read-only one: Atlas's profile must
    /// then keep the worktree unwritable, because nothing else will.
    pub read_only: bool,
    /// What was changed, for the audit trail.
    pub note: Option<String>,
}

/// A CLI that applies its own sandbox to the commands it runs (Codex, with `--sandbox`) cannot do
/// so inside ours: `sandbox_apply` is refused when a process is already confined (measured with
/// `codex sandbox`, at no cost), and every shell command of the run would fail. Inside Atlas's
/// profile its own sandbox is replaced by `danger-full-access`, which only means "do not add a
/// second sandbox": the confinement is the one around the whole process. A read-only run stays
/// read-only because the profile makes the worktree unwritable (`Adapted::read_only`).
///
/// Only called when the process *is* confined; unconfined, the CLI keeps its own sandbox.
pub fn adapt_to_confinement(program: &str, args: &mut [String]) -> Adapted {
    let name = program.rsplit(['/', '\\']).next().unwrap_or(program);
    if name != "codex" {
        return Adapted::default();
    }
    let Some(at) = args.iter().position(|a| a == "--sandbox") else {
        return Adapted::default();
    };
    let Some(mode) = args.get_mut(at + 1) else {
        return Adapted::default();
    };
    let read_only = mode == "read-only";
    if !read_only && mode != "workspace-write" {
        return Adapted::default();
    }
    "danger-full-access".clone_into(mode);
    Adapted {
        read_only,
        note: Some("sandbox:replaces_cli_sandbox:codex".to_owned()),
    }
}

/// Caches of the common developer tools. Without them `npm ci`, `cargo test` and `pip install`
/// fail inside the sandbox; with them a process can poison a cache other projects use, which is
/// why the person chooses (`SandboxSettings::dev_caches`).
pub const DEV_CACHES: &[&str] = &[
    ".npm",
    ".cache",
    "Library/Caches",
    ".cargo/registry",
    ".cargo/git",
    ".pnpm-store",
    "Library/pnpm",
    ".local/share/pnpm",
    ".yarn",
    ".bun/install/cache",
    ".gradle",
    ".m2/repository",
    ".nuget/packages",
    "go/pkg/mod",
];

/// Where credentials live. A confined process cannot read them, whatever it is told to do.
/// `~/.npmrc` is left readable on purpose: `npm` needs it for private registries (a residual
/// risk the ADR names).
pub const SECRET_STORES: &[&str] = &[
    ".ssh",
    ".aws",
    ".gnupg",
    ".kube",
    ".docker",
    ".netrc",
    ".config/gh",
    ".config/gcloud",
    "Library/Application Support/Google/Chrome",
    "Library/Safari",
    "Library/Messages",
    "Library/Mail",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|a| (*a).to_owned()).collect()
    }

    #[test]
    fn codex_gives_up_its_own_sandbox_inside_ours_and_a_read_only_run_is_remembered() {
        let mut writing = args(&[
            "exec",
            "--json",
            "--sandbox",
            "workspace-write",
            "--model",
            "m",
            "--",
            "p",
        ]);
        let mut reading = args(&["exec", "--sandbox", "read-only", "--", "p"]);

        let a = adapt_to_confinement("/opt/homebrew/bin/codex", &mut writing);
        let b = adapt_to_confinement("codex", &mut reading);

        assert_eq!(writing[3], "danger-full-access");
        assert_eq!(reading[2], "danger-full-access");
        assert!(!a.read_only && b.read_only);
        assert_eq!(
            a.note.as_deref(),
            Some("sandbox:replaces_cli_sandbox:codex")
        );
        // Only that value changed.
        assert_eq!(writing[..3], args(&["exec", "--json", "--sandbox"])[..]);
        assert_eq!(writing[4..], args(&["--model", "m", "--", "p"])[..]);
    }

    #[test]
    fn every_other_cli_and_any_other_codex_mode_is_left_as_it_is() {
        let mut claude = args(&["-p", "--sandbox", "read-only"]);
        let mut other = args(&["exec", "--sandbox", "danger-full-access"]);
        let mut none = args(&["exec", "--json"]);

        assert_eq!(
            adapt_to_confinement("claude", &mut claude),
            Adapted::default()
        );
        assert_eq!(
            adapt_to_confinement("codex", &mut other),
            Adapted::default()
        );
        assert_eq!(adapt_to_confinement("codex", &mut none), Adapted::default());
        assert_eq!(claude[2], "read-only");
        assert_eq!(other[2], "danger-full-access");
    }

    #[test]
    fn every_known_runtime_has_its_needs_and_an_unknown_program_has_none() {
        for program in crate::application::runtimes::RUNTIME_PROGRAMS {
            assert!(needs(program).is_some(), "{program}");
        }
        assert_eq!(needs("rm"), None);
        assert!(needs("/opt/homebrew/bin/claude").is_some());
    }

    #[test]
    fn what_a_cli_may_write_never_reaches_a_credential_store_or_its_own_binary() {
        for program in crate::application::runtimes::RUNTIME_PROGRAMS {
            let wanted = needs(program).unwrap();
            for path in wanted.dirs.iter().chain(wanted.file_prefixes) {
                assert!(!path.starts_with(".local/bin"), "{program}: {path}");
                assert!(
                    !SECRET_STORES
                        .iter()
                        .any(|secret| path.starts_with(secret) || secret.starts_with(path)),
                    "{program}: {path}"
                );
            }
        }
        assert!(!DEV_CACHES
            .iter()
            .any(|cache| SECRET_STORES.iter().any(|s| cache.starts_with(s))));
    }
}
