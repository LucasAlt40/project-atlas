//! What a connection's configuration may be. Checked when it is saved and again before a step is
//! given it: a configuration edited by hand in the file is held to the same rules.
//!
//! The one thing this refuses on principle is a shell. Atlas starts a process from an executable
//! and a list of arguments, kept apart; it never joins them into a string for a shell to read, and
//! it will not be handed `sh -c "…"` as a way to do the same.

use crate::domain::mcp::{McpConnection, McpEnv, McpTransport};

pub const MAX_NAME: usize = 32;
const MAX_FIELD: usize = 4096;
const MAX_ARGS: usize = 64;
const MAX_ENV: usize = 64;

/// Why a configuration is not accepted: a code the UI words, never the offending text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invalid {
    Name,
    HttpNotSupported,
    Executable,
    Shell,
    Argument,
    Expansion,
    EnvName,
    EnvDangerous,
    EnvDuplicate,
    TooMany,
}

impl Invalid {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::HttpNotSupported => "http_not_supported",
            Self::Executable => "executable",
            Self::Shell => "shell",
            Self::Argument => "argument",
            Self::Expansion => "expansion",
            Self::EnvName => "env_name",
            Self::EnvDangerous => "env_dangerous",
            Self::EnvDuplicate => "env_duplicate",
            Self::TooMany => "too_many",
        }
    }
}

const SHELLS: &[&str] = &[
    "sh",
    "bash",
    "zsh",
    "fish",
    "dash",
    "ksh",
    "csh",
    "tcsh",
    "cmd",
    "powershell",
    "pwsh",
    "wsl",
    "env",
    "xargs",
];

/// Variables that change what a program loads or runs, whatever the program is.
const DANGEROUS_ENV: &[&str] = &[
    "PATH",
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_LIBRARY_PATH",
    "NODE_OPTIONS",
    "PYTHONPATH",
    "PYTHONSTARTUP",
    "RUBYOPT",
    "PERL5OPT",
    "BASH_ENV",
    "ENV",
    "SHELL",
    "HOME",
    "GIT_SSH_COMMAND",
    "GIT_ASKPASS",
];

/// The program's name without its folder (either kind of separator) or extension, lowercase.
fn stem(executable: &str) -> String {
    let file = executable.rsplit(['/', '\\']).next().unwrap_or("");
    let name = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    name.to_lowercase()
}

fn plain(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= MAX_FIELD
        && !text.chars().any(|c| c == '\0' || c == '\n' || c == '\r')
}

/// `[a-z0-9_-]`, 1 to 32 characters, starting with a letter or digit: how the server is named to
/// agents and in the audit trail (and so in tool names).
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME
        && !name.starts_with(['-', '_'])
        // A runtime joins server and tool with `__`: a name holding one is ambiguous.
        && !name.contains("__")
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// What a name comes to once a runtime and the environment have had their say: `web-2` and `web_2`
/// are told apart by a person and are the same to the variable a secret travels in
/// (`ATLAS_MCP_WEB_2_…`), so two connections may not share it.
pub fn name_key(name: &str) -> String {
    name.to_lowercase().replace('-', "_")
}

fn valid_env_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with(|c: char| c.is_ascii_digit())
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// # Errors
///
/// The first thing wrong with the configuration.
pub fn validate(connection: &McpConnection) -> Result<(), Invalid> {
    if !valid_name(&connection.name) {
        return Err(Invalid::Name);
    }
    let McpTransport::Stdio {
        executable,
        args,
        env,
    } = &connection.transport
    else {
        return Err(Invalid::HttpNotSupported);
    };
    if !plain(executable) || executable.trim() != executable || executable.contains(' ') {
        return Err(Invalid::Executable);
    }
    let name = stem(executable);
    if SHELLS.contains(&name.as_str()) {
        return Err(Invalid::Shell);
    }
    if args.len() > MAX_ARGS || env.len() > MAX_ENV {
        return Err(Invalid::TooMany);
    }
    if args.iter().any(|a| a.len() > MAX_FIELD || a.contains('\0')) {
        return Err(Invalid::Argument);
    }
    // The CLIs expand references from Atlas's environment in a server's command, arguments and
    // values (`${VAR}` for Claude, `$VAR` for Gemini, `{env:VAR}` and `{file:path}` for OpenCode): a
    // configuration could name a variable that holds another connection's secret. Atlas writes
    // those references itself, for its own secrets, and nothing else may.
    let expands = |text: &str| {
        text.contains("${")
            || text.contains("{env:")
            || text.contains("{file:")
            || text.match_indices('$').any(|(at, _)| {
                text[at + 1..].starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
            })
    };
    if expands(executable)
        || args.iter().any(|a| expands(a))
        || env.iter().any(|e| {
            matches!(&e.value, crate::domain::mcp::McpEnvValue::Plain { value } if expands(value))
        })
    {
        return Err(Invalid::Expansion);
    }
    check_env(env)
}

fn check_env(env: &[McpEnv]) -> Result<(), Invalid> {
    let mut seen: Vec<&str> = Vec::new();
    for var in env {
        if !valid_env_name(&var.name) {
            return Err(Invalid::EnvName);
        }
        if DANGEROUS_ENV.contains(&var.name.as_str()) || var.name.starts_with("ATLAS_") {
            return Err(Invalid::EnvDangerous);
        }
        if seen.contains(&var.name.as_str()) {
            return Err(Invalid::EnvDuplicate);
        }
        seen.push(&var.name);
        if let crate::domain::mcp::McpEnvValue::Plain { value } = &var.value {
            if value.len() > MAX_FIELD || value.contains('\0') {
                return Err(Invalid::Argument);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::mcp::tests::connection;
    use crate::domain::mcp::McpEnvValue;

    fn with(transport: McpTransport) -> McpConnection {
        McpConnection {
            transport,
            ..connection("files")
        }
    }

    fn stdio(executable: &str, args: &[&str], env: Vec<McpEnv>) -> McpTransport {
        McpTransport::Stdio {
            executable: executable.to_owned(),
            args: args.iter().map(|a| (*a).to_owned()).collect(),
            env,
        }
    }

    fn env(name: &str) -> McpEnv {
        McpEnv {
            name: name.to_owned(),
            value: McpEnvValue::Plain {
                value: "x".to_owned(),
            },
        }
    }

    #[test]
    fn a_plain_executable_with_its_arguments_is_accepted() {
        assert_eq!(validate(&connection("files")), Ok(()));
        assert_eq!(
            validate(&with(stdio(
                "/usr/local/bin/mcp-server",
                &["--root", "/tmp/a b"],
                vec![env("REGION")]
            ))),
            Ok(())
        );
    }

    #[test]
    fn names_are_short_lowercase_words() {
        for good in ["files", "chrome-devtools", "a1", "my_server"] {
            assert!(valid_name(good), "{good}");
        }
        for bad in [
            "",
            "Files",
            "-x",
            "_x",
            "a b",
            "a.b",
            "a__b",
            "é",
            &"x".repeat(33),
        ] {
            assert!(!valid_name(bad), "{bad}");
        }
        assert_eq!(validate(&connection("Bad Name")), Err(Invalid::Name));
    }

    #[test]
    fn a_shell_is_never_the_executable_however_it_is_spelled() {
        for shell in [
            "sh",
            "/bin/bash",
            "zsh",
            "cmd.exe",
            "C:\\Windows\\System32\\cmd.exe",
            "powershell",
            "pwsh.exe",
            "/usr/bin/env",
            "xargs",
        ] {
            assert_eq!(
                validate(&with(stdio(shell, &["-c", "echo hi"], vec![]))),
                Err(Invalid::Shell),
                "{shell}"
            );
        }
    }

    #[test]
    fn the_executable_is_a_path_or_a_name_and_nothing_to_be_split() {
        for bad in ["", " node", "node ", "node server.js", "a\nb", "a\0b"] {
            assert_eq!(
                validate(&with(stdio(bad, &[], vec![]))),
                Err(Invalid::Executable),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn arguments_are_kept_apart_and_may_hold_anything_but_a_nul() {
        assert_eq!(
            validate(&with(stdio(
                "node",
                &["a; rm -rf /", "$(x)", "`y`"],
                vec![]
            ))),
            Ok(())
        );
        assert_eq!(
            validate(&with(stdio("node", &["a\0b"], vec![]))),
            Err(Invalid::Argument)
        );
    }

    #[test]
    fn environment_names_are_checked_and_the_dangerous_ones_refused() {
        assert_eq!(
            validate(&with(stdio("node", &[], vec![env("lower")]))),
            Err(Invalid::EnvName)
        );
        for dangerous in ["PATH", "NODE_OPTIONS", "LD_PRELOAD", "ATLAS_MCP_X", "HOME"] {
            assert_eq!(
                validate(&with(stdio("node", &[], vec![env(dangerous)]))),
                Err(Invalid::EnvDangerous),
                "{dangerous}"
            );
        }
        assert_eq!(
            validate(&with(stdio("node", &[], vec![env("A"), env("A")]))),
            Err(Invalid::EnvDuplicate)
        );
    }

    #[test]
    fn a_reference_to_the_environment_is_never_accepted_in_a_configuration() {
        let leak = McpEnv {
            name: "STOLEN".to_owned(),
            value: McpEnvValue::Plain {
                value: "${ATLAS_MCP_OTHER_TOKEN}".to_owned(),
            },
        };
        for transport in [
            stdio("node", &["--key=${ATLAS_MCP_OTHER_TOKEN}"], vec![]),
            stdio("node", &[], vec![leak]),
            stdio("${HOME}/bin/x", &[], vec![]),
            stdio("node", &["--key=$ATLAS_MCP_OTHER_TOKEN"], vec![]),
            stdio("node", &["{env:ATLAS_MCP_OTHER_TOKEN}"], vec![]),
            stdio("node", &["{file:/etc/passwd}"], vec![]),
        ] {
            assert_eq!(validate(&with(transport)), Err(Invalid::Expansion));
        }
    }

    #[test]
    fn http_is_part_of_the_contract_and_refused() {
        assert_eq!(
            validate(&with(McpTransport::Http {
                url: "https://example.test/mcp".to_owned()
            })),
            Err(Invalid::HttpNotSupported)
        );
    }

    #[test]
    fn a_huge_configuration_is_refused() {
        let args: Vec<String> = (0..65).map(|i| i.to_string()).collect();
        let transport = McpTransport::Stdio {
            executable: "node".to_owned(),
            args,
            env: vec![],
        };

        assert_eq!(validate(&with(transport)), Err(Invalid::TooMany));
    }
}
