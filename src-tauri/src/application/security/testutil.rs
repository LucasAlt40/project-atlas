//! Helpers shared by the security tests.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// A temporary directory (canonical path, so symlinked temp roots such as macOS's `/var` do
/// not confuse comparisons) removed when dropped.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(label: &str) -> Self {
        let path = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "atlas-{label}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The real process runner behind the real guard, for the ignored tests that talk to real CLIs.
/// The scope of `ExecutionScope::for_tests` (workspace `ws-1`, agent `agent-1`, read-only
/// profile) is registered with `project` as its folder, so such tests exercise the same path as
/// the app.
pub fn guarded_system_runner(
    project: &Path,
) -> std::sync::Arc<dyn crate::application::process::ProcessRunner> {
    use std::sync::Arc;

    use super::{ApprovalBroker, AuditLog, GuardedProcessRunner, NoSandbox, SecurityService};
    use crate::application::config::memory::MemoryStore;
    use crate::application::config::ConfigRepository;
    use crate::application::runtimes::RUNTIME_PROGRAMS;
    use crate::domain::agent::Agent;
    use crate::domain::security::SecurityPolicy;
    use crate::domain::workspace::{Workspace, WorkspaceLayout};

    let config = Arc::new(ConfigRepository::load(Box::<MemoryStore>::default()));
    config
        .modify(|c| {
            c.workspaces.push(Workspace {
                id: "ws-1".to_owned(),
                name: "tests".to_owned(),
                project_path: project.to_string_lossy().into_owned(),
                description: None,
                created_at: 1,
                updated_at: 1,
                layout: WorkspaceLayout {
                    rows: 2,
                    columns: 2,
                    agent_placements: vec![],
                },
                security: SecurityPolicy::default(),
            });
            c.agents.push(Agent {
                id: "agent-1".to_owned(),
                name: "tests".to_owned(),
                personality_id: "architect".to_owned(),
                runtime_id: "x".to_owned(),
                model_id: "m".to_owned(),
                instructions: String::new(),
                permission_profile_id: None,
                worktree_isolation: false,
                result_contract: crate::domain::result_contract::ResultContract::default(),
                created_at: 1,
            });
            Ok(())
        })
        .unwrap();
    Arc::new(GuardedProcessRunner::new(
        Arc::new(crate::infrastructure::SystemProcessRunner::new()),
        Arc::new(SecurityService::new(config)),
        Arc::new(ApprovalBroker::new()),
        Arc::new(AuditLog::default()),
        Arc::new(NoSandbox),
        RUNTIME_PROGRAMS.map(str::to_owned).to_vec(),
    ))
}
