//! What each runtime's CLI needs from the OS sandbox (ADR 0038): one implementation of
//! [`RuntimeConfinement`] per CLI (**Strategy**) and the one place that lists them (**Factory**).
//!
//! The facts come from the spike of 2026-10-15 (`docs/architecture/spikes/macos-sandbox.md`): what
//! each CLI creates or changes when it runs. They live here, beside the adapters, because they are
//! facts about the CLIs; the sandbox only asks. Adding a runtime is one `impl RuntimeConfinement`
//! and one line of [`default_confinements`]; a test fails until it is there.

use std::sync::Arc;

use crate::application::security::sandbox::cli::{
    Adapted, CliNeeds, ConfinementRegistry, RuntimeConfinement,
};

struct ClaudeConfinement;

impl RuntimeConfinement for ClaudeConfinement {
    fn program(&self) -> &'static str {
        "claude"
    }

    fn needs(&self) -> CliNeeds {
        CliNeeds {
            dirs: &[".claude", "Library/Caches/claude-cli-nodejs"],
            file_prefixes: &[".claude.json"],
            tmp_dirs: &["/private/tmp/claude-{uid}"],
        }
    }

    /// A run given the project's test commands carries the CLI's own sandbox in its `--settings`
    /// (`claude::command_settings`). That sandbox is Seatbelt too, and a second Seatbelt profile is
    /// refused inside ours (measured for Codex, not yet for Claude), so inside Atlas's profile it
    /// is turned off: Atlas's profile confines the whole process and every command it starts.
    /// The permission rules stay, so `Bash` still runs the test commands and nothing else. What
    /// is lost is the CLI's network block: Atlas's profile leaves the network open (ADR 0038),
    /// as it does for Codex.
    fn adapt(&self, args: &mut [String]) -> Adapted {
        let Some(at) = args.iter().position(|a| a == "--settings") else {
            return Adapted::default();
        };
        let Some(value) = args.get_mut(at + 1) else {
            return Adapted::default();
        };
        let Ok(mut settings) = serde_json::from_str::<serde_json::Value>(value) else {
            return Adapted::default();
        };
        let Some(sandbox) = settings
            .get_mut("sandbox")
            .and_then(serde_json::Value::as_object_mut)
        else {
            return Adapted::default();
        };
        if sandbox.get("enabled") != Some(&serde_json::Value::Bool(true)) {
            return Adapted::default();
        }
        // A run that only reads had its folders refused for writing by the CLI's sandbox; Atlas's
        // profile must then keep the worktree unwritable, because nothing else will.
        let read_only = sandbox
            .get("filesystem")
            .and_then(|f| f.get("denyWrite"))
            .and_then(serde_json::Value::as_array)
            .is_some_and(|dirs| !dirs.is_empty());
        sandbox.insert("enabled".to_owned(), serde_json::Value::Bool(false));
        sandbox.insert(
            "failIfUnavailable".to_owned(),
            serde_json::Value::Bool(false),
        );
        *value = settings.to_string();
        Adapted {
            read_only,
            note: Some("sandbox:replaces_cli_sandbox:claude".to_owned()),
        }
    }
}

struct OpenCodeConfinement;

impl RuntimeConfinement for OpenCodeConfinement {
    fn program(&self) -> &'static str {
        "opencode"
    }

    fn needs(&self) -> CliNeeds {
        CliNeeds {
            dirs: &[
                ".local/share/opencode",
                ".local/state/opencode",
                ".cache/opencode",
            ],
            file_prefixes: &[],
            tmp_dirs: &[],
        }
    }
}

struct CodexConfinement;

impl RuntimeConfinement for CodexConfinement {
    fn program(&self) -> &'static str {
        "codex"
    }

    fn needs(&self) -> CliNeeds {
        CliNeeds {
            dirs: &[".codex"],
            file_prefixes: &[],
            tmp_dirs: &[],
        }
    }

    /// Codex applies its own seatbelt to the commands it runs (`--sandbox`), and that **fails
    /// inside ours**: `sandbox_apply` is refused when a process is already confined (measured with
    /// `codex sandbox`, at no cost), so every shell command of the run would fail. Inside Atlas's
    /// profile its own sandbox is replaced by `danger-full-access`, which only means "do not add a
    /// second sandbox": the confinement is the one around the whole process. A run Codex would
    /// have held read-only stays read-only because the profile makes the worktree unwritable
    /// (`Adapted::read_only`).
    fn adapt(&self, args: &mut [String]) -> Adapted {
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
}

/// Gemini and Antigravity both keep their state under `~/.gemini`. Antigravity also updates its
/// own binary in `~/.local/bin`: that stays refused on purpose, the sandbox keeps a CLI from
/// replacing itself.
struct GeminiConfinement;

impl RuntimeConfinement for GeminiConfinement {
    fn program(&self) -> &'static str {
        "gemini"
    }

    fn needs(&self) -> CliNeeds {
        CliNeeds {
            dirs: &[".gemini"],
            file_prefixes: &[],
            tmp_dirs: &[],
        }
    }
}

struct AntigravityConfinement;

impl RuntimeConfinement for AntigravityConfinement {
    fn program(&self) -> &'static str {
        "agy"
    }

    fn needs(&self) -> CliNeeds {
        CliNeeds {
            dirs: &[".gemini"],
            file_prefixes: &[],
            tmp_dirs: &[],
        }
    }
}

/// The GitHub Copilot CLI writes its state in `~/.copilot` and a cache in `~/Library/Caches/copilot`
/// (measured on 2026-10-15 with a run that fails on a model that does not exist, at no cost, and
/// confirmed under the profile: without these two it fails at once and says nothing).
struct CopilotConfinement;

impl RuntimeConfinement for CopilotConfinement {
    fn program(&self) -> &'static str {
        "copilot"
    }

    fn needs(&self) -> CliNeeds {
        CliNeeds {
            dirs: &[".copilot", "Library/Caches/copilot"],
            file_prefixes: &[],
            tmp_dirs: &[],
        }
    }
}

/// The strategies of every runtime Atlas knows (**Factory**).
pub fn default_confinements() -> ConfinementRegistry {
    ConfinementRegistry::new(vec![
        Arc::new(OpenCodeConfinement),
        Arc::new(ClaudeConfinement),
        Arc::new(CodexConfinement),
        Arc::new(GeminiConfinement),
        Arc::new(AntigravityConfinement),
        Arc::new(CopilotConfinement),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::runtimes::RUNTIME_PROGRAMS;
    use crate::application::security::sandbox::cli::{DEV_CACHES, SECRET_STORES};

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|a| (*a).to_owned()).collect()
    }

    #[test]
    fn inside_atlas_sandbox_claude_gives_up_its_own_and_keeps_its_permission_rules() {
        let registry = default_confinements();
        let claude = registry.find("claude").unwrap();
        let settings = r#"{"permissions":{"allow":["Bash(npm test)"]},"sandbox":{"enabled":true,"failIfUnavailable":true,"allowUnsandboxedCommands":false}}"#;
        let mut launch = args(&["-p", "--settings", settings, "--model", "haiku"]);

        let adapted = claude.adapt(&mut launch);

        assert_eq!(
            adapted.note.as_deref(),
            Some("sandbox:replaces_cli_sandbox:claude")
        );
        assert!(!adapted.read_only);
        let now: serde_json::Value = serde_json::from_str(&launch[2]).unwrap();
        assert_eq!(now["sandbox"]["enabled"], false);
        assert_eq!(now["sandbox"]["failIfUnavailable"], false);
        assert_eq!(now["sandbox"]["allowUnsandboxedCommands"], false);
        assert_eq!(now["permissions"]["allow"][0], "Bash(npm test)");
        assert_eq!(launch[3..], args(&["--model", "haiku"]));

        // A run that only reads had its folders refused for writing: Atlas's profile must then
        // keep the worktree unwritable.
        let reading = r#"{"sandbox":{"enabled":true,"filesystem":{"denyWrite":["/wt"]}}}"#;
        let mut launch = args(&["-p", "--settings", reading]);
        assert!(claude.adapt(&mut launch).read_only);

        // A launch without the test settings is left alone.
        let mut plain = args(&["-p", "--tools", "Read,Grep,Glob"]);
        assert_eq!(claude.adapt(&mut plain), Adapted::default());
        assert_eq!(plain, args(&["-p", "--tools", "Read,Grep,Glob"]));
    }

    #[test]
    fn every_runtime_atlas_can_start_has_a_strategy_so_a_new_one_cannot_be_forgotten() {
        let registry = default_confinements();

        for program in RUNTIME_PROGRAMS {
            assert!(
                registry.find(program).is_some(),
                "{program} has no strategy"
            );
        }
        assert!(registry.find("rm").is_none());
        assert!(registry.find("/opt/homebrew/bin/claude").is_some());
    }

    #[test]
    fn what_a_cli_may_write_never_reaches_a_credential_store_or_its_own_binary() {
        let registry = default_confinements();
        for program in RUNTIME_PROGRAMS {
            let wanted = registry.find(program).unwrap().needs();
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

    #[test]
    fn codex_gives_up_its_own_sandbox_inside_ours_and_a_read_only_run_is_remembered() {
        let codex = default_confinements();
        let codex = codex.find("codex").unwrap();
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

        let a = codex.adapt(&mut writing);
        let b = codex.adapt(&mut reading);

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
        let registry = default_confinements();
        let mut claude = args(&["-p", "--sandbox", "read-only"]);
        let mut other = args(&["exec", "--sandbox", "danger-full-access"]);
        let mut none = args(&["exec", "--json"]);

        assert_eq!(
            registry.find("claude").unwrap().adapt(&mut claude),
            Adapted::default()
        );
        let codex = registry.find("codex").unwrap();
        assert_eq!(codex.adapt(&mut other), Adapted::default());
        assert_eq!(codex.adapt(&mut none), Adapted::default());
        assert_eq!(claude[2], "read-only");
        assert_eq!(other[2], "danger-full-access");
    }
}
