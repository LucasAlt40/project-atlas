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

/// What the machine has, as far as Atlas can see without starting anything.
pub struct Host {
    runner: Arc<dyn ProcessRunner>,
    chrome_paths: Vec<PathBuf>,
}

impl Host {
    fn has_program(&self, program: &str) -> bool {
        self.runner.locate(program).is_some()
    }

    fn chrome_found(&self) -> bool {
        self.chrome_paths.iter().any(|p| p.exists()) || self.has_program("google-chrome")
    }
}

/// One entry of the catalogue (the strategy): what it is, what it needs, and the connection it
/// becomes. Adding an integration is one `impl` and one arm of [`McpPresetFactory::create`].
pub trait McpPreset: Send + Sync {
    fn id(&self) -> &'static str;

    /// How it is shown, with what the machine has.
    fn describe(&self, host: &Host) -> McpPresetInfo;

    /// The name and configuration of the connection it becomes.
    ///
    /// # Errors
    ///
    /// The entry cannot be added yet.
    fn connection(&self) -> Result<(&'static str, McpTransport), AppError>;
}

/// The Chrome `DevTools` server: STDIO, pinned, headless and isolated.
struct ChromeDevTools;

impl ChromeDevTools {
    fn transport() -> McpTransport {
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
}

impl McpPreset for ChromeDevTools {
    fn id(&self) -> &'static str {
        "chrome-devtools"
    }

    fn describe(&self, host: &Host) -> McpPresetInfo {
        let McpTransport::Stdio {
            executable, args, ..
        } = Self::transport()
        else {
            unreachable!("the DevTools entry is STDIO")
        };
        McpPresetInfo {
            id: "chrome-devtools",
            connection_name: "chrome-devtools",
            availability: PresetAvailability::Ready,
            risk: PresetRisk::High,
            source_url: "https://github.com/ChromeDevTools/chrome-devtools-mcp",
            pinned_version: Some(DEVTOOLS_VERSION),
            command: Some(format!("{executable} {}", args.join(" "))),
            requirements: vec![
                check(Requirement::Node, host.has_program("node")),
                check(Requirement::Npx, host.has_program("npx")),
                check(Requirement::Chrome, host.chrome_found()),
            ],
        }
    }

    fn connection(&self) -> Result<(&'static str, McpTransport), AppError> {
        Ok(("chrome-devtools", Self::transport()))
    }
}

/// Figma's official server is remote (HTTP) and needs OAuth: listed for what it is, not addable.
struct Figma;

impl McpPreset for Figma {
    fn id(&self) -> &'static str {
        "figma"
    }

    fn describe(&self, _host: &Host) -> McpPresetInfo {
        McpPresetInfo {
            id: "figma",
            connection_name: "figma",
            availability: PresetAvailability::NeedsHttpAndOauth,
            risk: PresetRisk::High,
            source_url: "https://developers.figma.com/docs/figma-mcp-server",
            pinned_version: None,
            command: None,
            requirements: Vec::new(),
        }
    }

    fn connection(&self) -> Result<(&'static str, McpTransport), AppError> {
        Err(AppError::new(ErrorCode::McpPresetUnavailable))
    }
}

/// Chooses the entries (the factory).
pub struct McpPresetFactory;

impl McpPresetFactory {
    /// Every entry, in the order they are shown.
    pub fn all() -> Vec<Box<dyn McpPreset>> {
        vec![Box::new(ChromeDevTools), Box::new(Figma)]
    }

    pub fn create(id: &str) -> Option<Box<dyn McpPreset>> {
        Self::all().into_iter().find(|p| p.id() == id)
    }
}

pub struct McpCatalog {
    presets: Vec<Box<dyn McpPreset>>,
    host: Host,
}

impl McpCatalog {
    pub fn new(runner: Arc<dyn ProcessRunner>) -> Self {
        Self {
            presets: McpPresetFactory::all(),
            host: Host {
                runner,
                chrome_paths: default_chrome_paths(),
            },
        }
    }

    #[cfg(test)]
    pub fn with_chrome_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.host.chrome_paths = paths;
        self
    }

    pub fn list(&self) -> Vec<McpPresetInfo> {
        self.presets
            .iter()
            .map(|p| p.describe(&self.host))
            .collect()
    }

    /// The name and configuration of an entry, to be added as a connection.
    ///
    /// # Errors
    ///
    /// The entry is unknown, or cannot be added yet.
    pub fn connection_for(preset_id: &str) -> Result<(&'static str, McpTransport), AppError> {
        McpPresetFactory::create(preset_id)
            .ok_or_else(|| AppError::new(ErrorCode::McpPresetUnavailable))?
            .connection()
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
