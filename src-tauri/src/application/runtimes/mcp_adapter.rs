//! How a runtime's CLI takes MCP servers, as one interface with one implementation per CLI
//! (**Strategy**), and the one place that chooses the implementation (**Factory**).
//!
//! Every CLI needs the same three things from Atlas, in its own dialect: to be handed the servers of
//! one run (arguments, environment, temporary files), to be asked about one server without a model
//! (a probe), and to have the tool names it reports told back to the servers they belong to. A
//! runtime holds an `Arc<dyn McpAdapter>` and does not know which CLI's dialect it speaks.
//!
//! Adding a runtime is one `impl McpAdapter` and one arm of [`McpAdapterFactory::create`]; a runtime
//! that cannot take servers (Antigravity) has no adapter at all.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use super::claude_mcp::ClaudeMcpAdapter;
use super::codex_mcp::CodexMcpAdapter;
use super::gemini_mcp::GeminiMcpAdapter;
use super::opencode_mcp::OpenCodeMcpAdapter;
use super::{RuntimeError, RuntimeRequest};
use crate::application::mcp::{LaunchServer, McpLaunch, McpProbe};
use crate::application::process::{ProcessContext, ProcessOutput, ProcessRunner, ProcessSpec};
use crate::domain::mcp::{McpFeatures, McpServerStatus, McpSupport, ReportedMcpTool};

/// A private file of one run: readable by the user alone (on Unix), holding references where a
/// secret goes and never the secret, and deleted when this is dropped however the run ended.
pub struct ConfigFile {
    path: PathBuf,
}

impl ConfigFile {
    /// Writes `text` to a new file in the system's temporary directory.
    ///
    /// # Errors
    ///
    /// Fails if the file cannot be created or written.
    pub fn write_text(text: &str) -> std::io::Result<Self> {
        use std::io::Write;
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "atlas-mcp-{}-{}.json",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&path)?;
        // From here the guard owns the file: a failed write still deletes it.
        let guard = Self { path };
        file.write_all(text.as_bytes())?;
        Ok(guard)
    }

    pub fn path(&self) -> &str {
        self.path.to_str().unwrap_or_default()
    }
}

impl Drop for ConfigFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn io_error(error: &std::io::Error) -> RuntimeError {
    RuntimeError::ExecutionFailed(format!("the MCP config could not be written: {error}"))
}

/// What one run needs to be given its servers. Keep it alive until the run has ended: it owns the
/// temporary files.
#[derive(Default)]
pub struct McpPreparation {
    /// Arguments to add to the CLI's command line (where they go is the runtime's business).
    pub args: Vec<String>,
    /// Variables to add to the CLI's environment: secrets, and where its config is.
    pub env: Vec<(String, String)>,
    _files: Vec<ConfigFile>,
}

impl McpPreparation {
    /// For an adapter that wrote files: what it returns.
    pub fn with_files(
        args: Vec<String>,
        env: Vec<(String, String)>,
        files: Vec<ConfigFile>,
    ) -> Self {
        Self {
            args,
            env,
            _files: files,
        }
    }

    /// The servers of a run, as a text-only run or a step with none has them: nothing.
    pub fn empty() -> Self {
        Self::default()
    }
}

/// How to look at one server through a CLI, without a model.
pub struct ProbePlan {
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    /// Idle limit: a first `npx` downloads before it prints anything.
    pub timeout: Duration,
    /// Turns what the CLI printed into what it said of the server (given its name).
    pub read: fn(&ProcessOutput, &str) -> Result<McpProbe, RuntimeError>,
    _files: Vec<ConfigFile>,
}

impl ProbePlan {
    pub fn new(
        args: Vec<String>,
        env: Vec<(String, String)>,
        timeout: Duration,
        read: fn(&ProcessOutput, &str) -> Result<McpProbe, RuntimeError>,
        files: Vec<ConfigFile>,
    ) -> Self {
        Self {
            args,
            env,
            timeout,
            read,
            _files: files,
        }
    }
}

/// The probe of a CLI whose `mcp list` starts each server and says whether it connected: the
/// arguments, and what a listed server (or the lack of one) makes of the answer. It lists no tools.
pub(super) fn list_probe_arguments() -> Vec<String> {
    vec!["mcp".to_owned(), "list".to_owned()]
}

pub(super) fn listed_probe(status: Option<McpServerStatus>) -> Result<McpProbe, RuntimeError> {
    let status = status.ok_or_else(|| {
        RuntimeError::UnexpectedResponse("the CLI did not list the server".to_owned())
    })?;
    Ok(McpProbe {
        status,
        tools: Vec::new(),
    })
}

/// A CLI's way of taking MCP servers (the strategy).
pub trait McpAdapter: Send + Sync {
    /// The program to start.
    fn program(&self) -> &'static str;

    /// What this CLI can do with MCP: how it holds a server to named tools, what a probe shows,
    /// whether it loads only Atlas's servers. Measured, see the adapter's documentation.
    fn features(&self) -> McpFeatures;

    /// The arguments, environment and temporary files that give the CLI `launch`'s servers.
    ///
    /// # Errors
    ///
    /// A config file cannot be written, or the servers cannot be given together.
    fn prepare(&self, launch: &McpLaunch) -> Result<McpPreparation, RuntimeError>;

    /// How to look at one server. `None` when the CLI cannot say anything without a model.
    fn probe_plan(&self, _server: &LaunchServer) -> Option<Result<ProbePlan, RuntimeError>> {
        None
    }

    /// Which of the tool names a run listed or used are tools of the servers that were launched.
    fn classify(&self, reported: &[String], launched: &[&str]) -> Vec<ReportedMcpTool>;

    /// Starts `server` through the CLI only to see what it reports. No model is asked anything.
    ///
    /// # Errors
    ///
    /// The CLI is missing, has no probe, cannot run, or did not say what was needed.
    fn probe(
        &self,
        runner: &dyn ProcessRunner,
        server: &LaunchServer,
    ) -> Result<McpProbe, RuntimeError> {
        if runner.locate(self.program()).is_none() {
            return Err(RuntimeError::NotInstalled);
        }
        let plan = self.probe_plan(server).unwrap_or_else(|| {
            Err(RuntimeError::Unavailable(
                "This runtime cannot look at an MCP server without a model.".to_owned(),
            ))
        })?;
        let spec = ProcessSpec {
            program: self.program().to_owned(),
            args: plan.args.clone(),
            stdin: None,
            cwd: None,
            env: plan.env.clone(),
            timeout: plan.timeout,
            context: ProcessContext::Probe,
            terminal: None,
        };
        // The exit code is not the answer: a CLI told to use a model that does not exist fails on
        // purpose, after saying what it loaded.
        let output = runner
            .run(&spec, &|_| {})
            .map_err(|error| RuntimeError::ExecutionFailed(format!("{error:?}")))?;
        (plan.read)(&output, &server.name)
    }
}

/// The dialects Atlas has an adapter for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpDialect {
    Claude,
    Gemini,
    OpenCode,
    Codex,
}

/// Chooses the adapter for a dialect (the factory).
pub struct McpAdapterFactory;

impl McpAdapterFactory {
    pub fn create(dialect: McpDialect) -> Arc<dyn McpAdapter> {
        match dialect {
            McpDialect::Claude => Arc::new(ClaudeMcpAdapter),
            McpDialect::Gemini => Arc::new(GeminiMcpAdapter),
            McpDialect::OpenCode => Arc::new(OpenCodeMcpAdapter),
            McpDialect::Codex => Arc::new(CodexMcpAdapter),
        }
    }

    /// What a runtime declares of MCP: with an adapter, `Supported` and what it can do; without,
    /// `Unsupported` (measured: nothing to give servers through).
    pub fn declared(adapter: Option<&Arc<dyn McpAdapter>>) -> (McpSupport, McpFeatures) {
        adapter.map_or((McpSupport::Unsupported, McpFeatures::default()), |a| {
            (McpSupport::Supported, a.features())
        })
    }
}

/// The servers of a step, when it has any to give: never to a text-only run, and none is no
/// preparation at all.
pub fn servers_of(request: &RuntimeRequest) -> Option<&McpLaunch> {
    request
        .mcp
        .as_ref()
        .filter(|m| !request.text_only && !m.is_empty())
}

/// Prepares a step's servers, or nothing when it has none.
///
/// # Errors
///
/// As [`McpAdapter::prepare`].
pub fn prepare_for(
    adapter: &dyn McpAdapter,
    request: &RuntimeRequest,
) -> Result<McpPreparation, RuntimeError> {
    servers_of(request).map_or_else(|| Ok(McpPreparation::empty()), |m| adapter.prepare(m))
}

pub(super) fn write_config(text: &str) -> Result<ConfigFile, RuntimeError> {
    ConfigFile::write_text(text).map_err(|e| io_error(&e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_factory_gives_each_dialect_its_own_program_and_measured_features() {
        use crate::domain::mcp::{McpProbeKind, McpToolFilter};
        let of = |d| McpAdapterFactory::create(d);

        assert_eq!(of(McpDialect::Claude).program(), "claude");
        assert_eq!(of(McpDialect::Gemini).program(), "gemini");
        assert_eq!(of(McpDialect::OpenCode).program(), "opencode");
        assert_eq!(of(McpDialect::Codex).program(), "codex");
        let claude = of(McpDialect::Claude).features();
        assert_eq!(claude.tool_filter, McpToolFilter::DenyList);
        assert_eq!(claude.probe, McpProbeKind::Tools);
        assert!(claude.strict);
        for dialect in [McpDialect::Gemini, McpDialect::OpenCode, McpDialect::Codex] {
            let features = of(dialect).features();
            assert_eq!(
                features.tool_filter,
                McpToolFilter::AllowList,
                "{dialect:?}"
            );
            assert!(!features.strict, "{dialect:?}");
        }
        assert_eq!(of(McpDialect::Codex).features().probe, McpProbeKind::None);
    }

    #[test]
    fn a_runtime_without_an_adapter_declares_itself_unsupported() {
        let (support, features) = McpAdapterFactory::declared(None);

        assert_eq!(support, McpSupport::Unsupported);
        assert_eq!(features, McpFeatures::default());
        let adapter = McpAdapterFactory::create(McpDialect::Codex);
        assert_eq!(
            McpAdapterFactory::declared(Some(&adapter)).0,
            McpSupport::Supported
        );
    }

    #[test]
    fn a_dialect_that_cannot_probe_says_so_without_starting_anything() {
        use crate::application::process::fake::FakeProcessRunner;
        let runner = FakeProcessRunner::new(&["codex"], |_| unreachable!("nothing is started"));
        let server = LaunchServer {
            name: "files".to_owned(),
            executable: "node".to_owned(),
            args: vec![],
            env: vec![],
        };

        let error = McpAdapterFactory::create(McpDialect::Codex)
            .probe(&runner, &server)
            .unwrap_err();

        assert!(matches!(error, RuntimeError::Unavailable(_)), "{error:?}");
    }

    #[test]
    fn two_config_files_never_share_a_path() {
        let (a, b) = (
            ConfigFile::write_text("{}").unwrap(),
            ConfigFile::write_text("{}").unwrap(),
        );

        assert_ne!(a.path(), b.path());
    }

    #[cfg(unix)]
    #[test]
    fn a_config_file_is_private_and_goes_with_its_guard() {
        use std::os::unix::fs::PermissionsExt;
        let file = ConfigFile::write_text("{}").unwrap();
        let path = PathBuf::from(file.path());

        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        drop(file);
        assert!(!path.exists());
    }
}
