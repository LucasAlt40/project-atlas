//! What the sandbox needs to know about a runtime's CLI, as one interface with one implementation
//! per runtime (**Strategy**) and a registry that finds the right one by program name
//! (**Registry**). The knowledge itself, from the spike of 2026-10-15
//! (`docs/architecture/spikes/macos-sandbox.md`), lives next to each runtime's adapter
//! (`runtimes/confinement.rs`), so the sandbox has no `match` on runtime names: adding a runtime
//! is one `impl RuntimeConfinement` and one line in its factory.
//!
//! Paths are relative to the user's home. A program no strategy claims is not a known runtime: it
//! is not confined, and a required sandbox refuses it.

use std::sync::Arc;

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

/// What one runtime tells the sandbox about its CLI.
pub trait RuntimeConfinement: Send + Sync {
    /// The bare name of the program this strategy is for (`claude`).
    fn program(&self) -> &'static str;

    /// What the CLI writes when it runs.
    fn needs(&self) -> CliNeeds;

    /// How its arguments change when the operating system already confines it. Most CLIs need no
    /// change; one that applies a sandbox of its own (Codex) must give it up, because a second
    /// one cannot be applied inside ours. Only called when the process *is* confined.
    fn adapt(&self, _args: &mut [String]) -> Adapted {
        Adapted::default()
    }
}

/// The strategies Atlas knows, found by the program a request starts.
#[derive(Clone, Default)]
pub struct ConfinementRegistry {
    strategies: Vec<Arc<dyn RuntimeConfinement>>,
}

impl ConfinementRegistry {
    pub fn new(strategies: Vec<Arc<dyn RuntimeConfinement>>) -> Self {
        Self { strategies }
    }

    /// The strategy for `program`, a bare name or a path (`.exe` is ignored).
    pub fn find(&self, program: &str) -> Option<&dyn RuntimeConfinement> {
        let name = program.rsplit(['/', '\\']).next().unwrap_or(program);
        let name = name.strip_suffix(".exe").unwrap_or(name);
        self.strategies
            .iter()
            .find(|s| s.program() == name)
            .map(AsRef::as_ref)
    }
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

    struct Plain;

    impl RuntimeConfinement for Plain {
        fn program(&self) -> &'static str {
            "plain"
        }

        fn needs(&self) -> CliNeeds {
            CliNeeds {
                dirs: &[".plain"],
                file_prefixes: &[],
                tmp_dirs: &[],
            }
        }
    }

    #[test]
    fn the_registry_finds_a_strategy_by_bare_name_path_or_exe_and_nothing_else() {
        let registry = ConfinementRegistry::new(vec![Arc::new(Plain)]);

        for program in ["plain", "/opt/homebrew/bin/plain", "C:\\bin\\plain.exe"] {
            assert_eq!(
                registry.find(program).map(RuntimeConfinement::program),
                Some("plain")
            );
        }
        assert!(registry.find("rm").is_none());
        assert!(ConfinementRegistry::default().find("plain").is_none());
    }

    #[test]
    fn a_strategy_that_says_nothing_about_its_arguments_leaves_them_alone() {
        let mut args = vec!["--sandbox".to_owned(), "read-only".to_owned()];

        assert_eq!(Plain.adapt(&mut args), Adapted::default());
        assert_eq!(args, ["--sandbox", "read-only"]);
    }

    #[test]
    fn what_the_shared_lists_name_never_overlaps() {
        assert!(!DEV_CACHES
            .iter()
            .any(|cache| SECRET_STORES.iter().any(|s| cache.starts_with(s))));
    }
}
