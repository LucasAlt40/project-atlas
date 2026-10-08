//! The integrations Atlas knows about by name: what they are, where they come from, what they need
//! and what they risk. An entry is **a suggestion, not a connection**: adding one creates an
//! ordinary connection that is switched off, granted to nobody and not started. Every figure here
//! was read from the project's own documentation (see the platform document, section 11).

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;

use crate::application::errors::{AppError, ErrorCode};
use crate::application::process::ProcessRunner;
use crate::domain::mcp::McpTransport;

/// The version `npx` is told to run. Never `@latest`: what runs is what was looked at, and a new
/// version is a change someone makes on purpose here. Node `^20.19 || ^22.12 || >=23`, Apache-2.0.
pub const DEVTOOLS_VERSION: &str = "1.10.1";

/// Whether an entry can be added today.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PresetAvailability {
    /// It can be added as a connection (STDIO).
    Ready,
    /// It exists but needs the HTTP transport and OAuth, which Atlas does not have.
    NeedsHttpAndOauth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PresetRisk {
    /// Full access to something the user owns (a whole browser, files of a design account).
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Requirement {
    Node,
    Npx,
    Chrome,
}

/// What Atlas could see of one requirement on this machine. Atlas starts nothing to find out, so
/// "not found" means not found where it looks, not that it is missing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequirementCheck {
    pub requirement: Requirement,
    pub found: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpPresetInfo {
    pub id: &'static str,
    /// What the connection is called if added: how agents and the audit trail name it.
    pub connection_name: &'static str,
    pub availability: PresetAvailability,
    pub risk: PresetRisk,
    pub source_url: &'static str,
    pub pinned_version: Option<&'static str>,
    /// What would be started, as shown to the user (never run through a shell).
    pub command: Option<String>,
    pub requirements: Vec<RequirementCheck>,
}

fn check(requirement: Requirement, found: bool) -> RequirementCheck {
    RequirementCheck { requirement, found }
}

fn devtools_transport() -> McpTransport {
    McpTransport::Stdio {
        executable: "npx".to_owned(),
        args: [
            "-y".to_owned(),
            format!("chrome-devtools-mcp@{DEVTOOLS_VERSION}"),
            // An own profile and no window: not the user's browser; and no data sent to Google.
            "--isolated".to_owned(),
            "--headless".to_owned(),
            "--no-usage-statistics".to_owned(),
            "--no-performance-crux".to_owned(),
        ]
        .to_vec(),
        env: Vec::new(),
    }
}

pub struct McpCatalog {
    runner: Arc<dyn ProcessRunner>,
    chrome_paths: Vec<PathBuf>,
}

impl McpCatalog {
    pub fn new(runner: Arc<dyn ProcessRunner>) -> Self {
        Self {
            runner,
            chrome_paths: default_chrome_paths(),
        }
    }

    #[cfg(test)]
    pub fn with_chrome_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.chrome_paths = paths;
        self
    }

    pub fn list(&self) -> Vec<McpPresetInfo> {
        let McpTransport::Stdio {
            executable, args, ..
        } = devtools_transport()
        else {
            unreachable!("the DevTools entry is STDIO")
        };
        vec![
            McpPresetInfo {
                id: "chrome-devtools",
                connection_name: "chrome-devtools",
                availability: PresetAvailability::Ready,
                risk: PresetRisk::High,
                source_url: "https://github.com/ChromeDevTools/chrome-devtools-mcp",
                pinned_version: Some(DEVTOOLS_VERSION),
                command: Some(format!("{executable} {}", args.join(" "))),
                requirements: vec![
                    check(Requirement::Node, self.runner.locate("node").is_some()),
                    check(Requirement::Npx, self.runner.locate("npx").is_some()),
                    check(Requirement::Chrome, self.chrome_found()),
                ],
            },
            McpPresetInfo {
                id: "figma",
                connection_name: "figma",
                availability: PresetAvailability::NeedsHttpAndOauth,
                risk: PresetRisk::High,
                source_url: "https://developers.figma.com/docs/figma-mcp-server",
                pinned_version: None,
                command: None,
                requirements: Vec::new(),
            },
        ]
    }

    fn chrome_found(&self) -> bool {
        self.chrome_paths.iter().any(|p| p.exists())
            || self.runner.locate("google-chrome").is_some()
    }

    /// The name and configuration of an entry, to be added as a connection.
    ///
    /// # Errors
    ///
    /// The entry is unknown, or cannot be added yet.
    pub fn connection_for(preset_id: &str) -> Result<(&'static str, McpTransport), AppError> {
        match preset_id {
            "chrome-devtools" => Ok(("chrome-devtools", devtools_transport())),
            // Figma is listed but needs HTTP and OAuth: it falls here with every unknown id.
            _ => Err(AppError::new(ErrorCode::McpPresetUnavailable)),
        }
    }
}

fn default_chrome_paths() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        paths.push("/Applications/Google Chrome.app".into());
    } else if cfg!(windows) {
        for var in ["PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"] {
            if let Some(base) = std::env::var_os(var) {
                paths.push(PathBuf::from(base).join("Google/Chrome/Application/chrome.exe"));
            }
        }
    } else {
        for fixed in [
            "/usr/bin/google-chrome",
            "/usr/bin/google-chrome-stable",
            "/opt/google/chrome/chrome",
        ] {
            paths.push(fixed.into());
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::mcp::validate::validate;
    use crate::application::process::fake::FakeProcessRunner;
    use crate::domain::mcp::tests::connection;
    use crate::domain::mcp::McpConnection;

    fn catalog(installed: &[&str], chrome: Vec<PathBuf>) -> McpCatalog {
        let runner = Arc::new(FakeProcessRunner::new(installed, |_| {
            unreachable!("nothing is started")
        }));
        McpCatalog::new(runner).with_chrome_paths(chrome)
    }

    #[test]
    fn the_devtools_entry_is_pinned_private_and_passes_the_same_checks_as_any_connection() {
        let (name, transport) = McpCatalog::connection_for("chrome-devtools").unwrap();
        let McpTransport::Stdio {
            executable,
            args,
            env,
        } = &transport
        else {
            panic!("STDIO")
        };

        assert_eq!(executable, "npx");
        assert_eq!(env.len(), 0);
        // Never `@latest`: the version is the one that was looked at.
        assert!(args.contains(&format!("chrome-devtools-mcp@{DEVTOOLS_VERSION}")));
        assert!(!args.iter().any(|a| a.contains("latest")));
        for flag in [
            "--isolated",
            "--headless",
            "--no-usage-statistics",
            "--no-performance-crux",
        ] {
            assert!(args.contains(&flag.to_owned()), "{flag}");
        }
        let as_connection = McpConnection {
            name: name.to_owned(),
            transport,
            ..connection("x")
        };
        assert_eq!(validate(&as_connection), Ok(()));
    }

    #[test]
    fn figma_is_listed_for_what_it_is_and_cannot_be_added() {
        let list = catalog(&[], vec![]).list();
        let figma = list.iter().find(|p| p.id == "figma").unwrap();

        assert_eq!(figma.availability, PresetAvailability::NeedsHttpAndOauth);
        assert_eq!(figma.command, None);
        assert!(McpCatalog::connection_for("figma")
            .unwrap_err()
            .is(ErrorCode::McpPresetUnavailable));
        assert!(McpCatalog::connection_for("ghost")
            .unwrap_err()
            .is(ErrorCode::McpPresetUnavailable));
    }

    #[test]
    fn requirements_are_looked_for_and_nothing_is_started_to_find_them() {
        let here = std::env::temp_dir();
        let found = |list: Vec<McpPresetInfo>| {
            list[0]
                .requirements
                .iter()
                .map(|r| r.found)
                .collect::<Vec<_>>()
        };

        assert_eq!(
            found(catalog(&["node", "npx"], vec![here]).list()),
            [true, true, true]
        );
        assert_eq!(
            found(catalog(&["node"], vec![PathBuf::from("/nonexistent/chrome")]).list()),
            [true, false, false]
        );
    }
}
