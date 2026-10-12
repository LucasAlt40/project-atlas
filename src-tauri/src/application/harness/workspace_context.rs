//! The context of a workspace as a whole, and starting the Harness of all its projects.
//!
//! Every project keeps its own Harness in its own `.atlas/`. What relates them (which projects
//! exist, what each is, what the user says depends on what, the integrations each Harness found)
//! belongs to the workspace, not to any repository, so it is kept by Atlas next to its
//! configuration and never written into a project. It has two parts: a **generated** one, rebuilt
//! from the projects' Harnesses, and the user's **notes** (cross-cutting decisions, contracts,
//! conventions), which regenerating never touches.
//!
//! Only references are copied from a project's Harness: a name, a stack, a status, a few
//! evidenced integration facts with their source. The Harness itself stays where it is and is
//! read, per task, when a project is in an execution's scope.

use std::collections::BTreeMap;
use std::fmt::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use serde::Serialize;

use super::knowledge::apply_choices;
use super::manifest::parse_manifest;
use super::HarnessService;
use crate::application::errors::{AppError, ErrorCode};
use crate::application::runtimes::sha256_hex;
use crate::application::support::now_ms;
use crate::domain::harness::{
    FindingCategory, HarnessStatus, HarnessSummary, InitMode, InitializeInput, Origin,
};
use crate::domain::workspace::{Workspace, WorkspaceProject};

/// The most integration facts of one project that go into the workspace context.
const MAX_FACTS_PER_PROJECT: usize = 6;
/// The most projects whose Harness is started at the same time.
const MAX_PARALLEL_PROJECTS: usize = 3;
/// The first line of a generated context. `fingerprint` is what it was made from.
const MARKER: &str = "<!-- atlas:workspace-context v1";

/// The two parts of a workspace's context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextFile {
    /// Rebuilt by Atlas from the projects' Harnesses.
    Generated,
    /// Written by the user; Atlas only reads it.
    Notes,
}

/// Port: where a workspace's context is kept. Implemented in `infrastructure/`, outside every
/// project folder.
pub trait WorkspaceContextStore: Send + Sync {
    fn read(&self, workspace_id: &str, file: ContextFile) -> Option<String>;

    /// # Errors
    ///
    /// `HarnessGenerationFailed` if the text cannot be stored.
    fn write(&self, workspace_id: &str, file: ContextFile, text: &str) -> Result<(), AppError>;

    /// Forgets everything kept for the workspace (it was deleted). Never fails the caller.
    fn remove(&self, workspace_id: &str);
}

/// Where each project stands, for the workspace screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectHarnessState {
    /// The folder is gone: fix the path or remove the project.
    FolderMissing,
    NotInitialized,
    /// `.atlas/` is there but cannot be trusted as it is.
    NeedsReview,
    /// Initialized, and the project changed in relevant ways since the analysis.
    Outdated,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHarnessInfo {
    pub project_id: String,
    pub state: ProjectHarnessState,
    pub summary: HarnessSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceContextInfo {
    pub exists: bool,
    /// When the generated part was made.
    pub generated_at: Option<u64>,
    /// A project, its role, its dependencies or its Harness changed since it was generated.
    pub outdated: bool,
    pub has_notes: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceHarnessOverview {
    pub projects: Vec<ProjectHarnessInfo>,
    pub context: WorkspaceContextInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectInitStatus {
    Initialized,
    /// It had a valid Harness already: it was not analysed again.
    AlreadyInitialized,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInitResult {
    pub project_id: String,
    pub status: ProjectInitStatus,
    pub error: Option<AppError>,
}

/// What starting the Harness of the workspace's projects came to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInitReport {
    pub projects: Vec<ProjectInitResult>,
    /// Every requested project has a Harness. Never true while one failed: the screen does not
    /// call the workspace ready.
    pub complete: bool,
    /// The workspace context after this run, when it could be made.
    pub context: Option<WorkspaceContextInfo>,
}

/// Progress of one project, as it happens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInitUpdate {
    pub workspace_id: String,
    pub project_id: String,
    pub state: ProjectInitProgress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectInitProgress {
    Running,
    Initialized,
    AlreadyInitialized,
    Failed,
}

/// What the context says of one project.
struct Digest {
    project: WorkspaceProject,
    summary: Option<HarnessSummary>,
    exists: bool,
    facts: Vec<String>,
}

impl HarnessService {
    /// Where each project of the workspace stands, and whether its context is current.
    ///
    /// # Errors
    ///
    /// Fails if the workspace does not exist.
    pub fn workspace_overview(
        &self,
        workspace_id: &str,
    ) -> Result<WorkspaceHarnessOverview, AppError> {
        let workspace = self.workspace(workspace_id)?;
        let digests = self.digests(&workspace);
        let projects = digests
            .iter()
            .map(|d| {
                let summary = d
                    .summary
                    .clone()
                    .unwrap_or_else(|| HarnessSummary::not_initialized(false));
                ProjectHarnessInfo {
                    project_id: d.project.id.clone(),
                    state: state_of(d.exists, &summary),
                    summary,
                }
            })
            .collect();
        Ok(WorkspaceHarnessOverview {
            projects,
            context: self.context_info(&workspace, &digests),
        })
    }

    /// Rebuilds the generated part of the workspace's context from the projects' Harnesses. The
    /// user's notes are untouched. A workspace with a single project has no such context.
    ///
    /// # Errors
    ///
    /// Fails if the workspace does not exist or the text cannot be stored.
    pub fn build_workspace_context(
        &self,
        workspace_id: &str,
    ) -> Result<WorkspaceContextInfo, AppError> {
        let workspace = self.workspace(workspace_id)?;
        let digests = self.digests(&workspace);
        if let Some(store) = &self.workspace_store {
            if workspace.projects.len() > 1 {
                let text = render(&workspace, &digests, now_ms());
                store.write(workspace_id, ContextFile::Generated, &text)?;
            }
        }
        Ok(self.context_info(&workspace, &digests))
    }

    /// Forgets what Atlas kept about the workspace (it was deleted). The projects are not touched.
    pub fn forget_workspace(&self, workspace_id: &str) {
        if let Some(store) = &self.workspace_store {
            store.remove(workspace_id);
        }
    }

    /// The user's notes about the workspace as a whole.
    pub fn workspace_notes(&self, workspace_id: &str) -> String {
        self.workspace_store
            .as_ref()
            .and_then(|store| store.read(workspace_id, ContextFile::Notes))
            .unwrap_or_default()
    }

    /// # Errors
    ///
    /// Fails if the workspace does not exist or the notes cannot be stored.
    pub fn save_workspace_notes(&self, workspace_id: &str, notes: &str) -> Result<(), AppError> {
        self.workspace(workspace_id)?;
        let store = self
            .workspace_store
            .as_ref()
            .ok_or_else(|| AppError::new(ErrorCode::HarnessGenerationFailed))?;
        store.write(workspace_id, ContextFile::Notes, notes.trim())
    }

    /// Starts the Harness of the workspace's projects (or of the ones named), then rebuilds the
    /// workspace context.
    ///
    /// Projects are analysed one by one by the same rules as a single project, a few at a time.
    /// One that fails does not stop the others, and a project that already has a valid Harness is
    /// left exactly as it is, so running this again after a failure, or after adding a project,
    /// only does the work that is missing. No model is asked: this is the deterministic analysis,
    /// and a project's findings can be reviewed or refreshed on its own afterwards.
    ///
    /// # Errors
    ///
    /// Fails if the workspace does not exist or a named project is not in it. Failures of single
    /// projects are in the report.
    pub fn initialize_workspace(
        &self,
        workspace_id: &str,
        project_ids: Option<&[String]>,
        ignore_in_git: bool,
        progress: &(dyn Fn(ProjectInitUpdate) + Sync),
    ) -> Result<WorkspaceInitReport, AppError> {
        let workspace = self.workspace(workspace_id)?;
        let targets: Vec<WorkspaceProject> = match project_ids {
            None => workspace.projects.clone(),
            Some(ids) => ids
                .iter()
                .map(|id| {
                    workspace
                        .project(id)
                        .cloned()
                        .ok_or_else(|| AppError::new(ErrorCode::WorkspaceProjectNotFound))
                })
                .collect::<Result<_, _>>()?,
        };
        let next = AtomicUsize::new(0);
        let results: Mutex<BTreeMap<usize, ProjectInitResult>> = Mutex::new(BTreeMap::new());
        std::thread::scope(|scope| {
            for _ in 0..MAX_PARALLEL_PROJECTS.min(targets.len()) {
                scope.spawn(|| loop {
                    let index = next.fetch_add(1, Ordering::SeqCst);
                    let Some(project) = targets.get(index) else {
                        break;
                    };
                    let update = |state| {
                        progress(ProjectInitUpdate {
                            workspace_id: workspace_id.to_owned(),
                            project_id: project.id.clone(),
                            state,
                        });
                    };
                    update(ProjectInitProgress::Running);
                    let result = self.initialize_one(workspace_id, project, ignore_in_git);
                    update(match result.status {
                        ProjectInitStatus::Initialized => ProjectInitProgress::Initialized,
                        ProjectInitStatus::AlreadyInitialized => {
                            ProjectInitProgress::AlreadyInitialized
                        }
                        ProjectInitStatus::Failed => ProjectInitProgress::Failed,
                    });
                    results.lock().expect("results lock").insert(index, result);
                });
            }
        });
        let projects: Vec<ProjectInitResult> = results
            .into_inner()
            .expect("results lock")
            .into_values()
            .collect();
        let complete = projects
            .iter()
            .all(|r| r.status != ProjectInitStatus::Failed);
        // The context is built from whatever has a Harness, even after a partial run: it says
        // which projects have none yet.
        let context = self.build_workspace_context(workspace_id).ok();
        Ok(WorkspaceInitReport {
            projects,
            complete,
            context,
        })
    }

    fn initialize_one(
        &self,
        workspace_id: &str,
        project: &WorkspaceProject,
        ignore_in_git: bool,
    ) -> ProjectInitResult {
        let result = |status, error| ProjectInitResult {
            project_id: project.id.clone(),
            status,
            error,
        };
        match self.store.read_manifest(&project.path) {
            Err(error) => return result(ProjectInitStatus::Failed, Some(error)),
            Ok(Some(text)) => {
                return match parse_manifest(&text) {
                    Ok(_) => result(ProjectInitStatus::AlreadyInitialized, None),
                    Err(error) => result(ProjectInitStatus::Failed, Some(error)),
                };
            }
            Ok(None) => {}
        }
        let outcome = self
            .analyze_in(workspace_id, Some(&project.id), None)
            .and_then(|_| {
                self.initialize_in(
                    workspace_id,
                    Some(&project.id),
                    InitializeInput {
                        mode: InitMode::Create,
                        ignore_in_git,
                        ..InitializeInput::default()
                    },
                )
            });
        match outcome {
            Ok(_) => result(ProjectInitStatus::Initialized, None),
            Err(error) => result(ProjectInitStatus::Failed, Some(error)),
        }
    }

    fn workspace(&self, workspace_id: &str) -> Result<Workspace, AppError> {
        self.workspaces
            .get(workspace_id)
            .ok_or_else(|| AppError::new(ErrorCode::WorkspaceNotFound))
    }

    fn digests(&self, workspace: &Workspace) -> Vec<Digest> {
        workspace
            .projects
            .iter()
            .map(|project| {
                let exists = self.scanner.folder_exists(&project.path);
                // A folder that is not there has no Harness to read.
                let summary = exists.then(|| self.summary_at(&project.path));
                let facts = if exists {
                    self.integration_facts(project)
                } else {
                    Vec::new()
                };
                Digest {
                    project: project.clone(),
                    summary,
                    exists,
                    facts,
                }
            })
            .collect()
    }

    /// Evidenced facts about how the project connects to anything else, from its own Harness:
    /// what it was told, never what it might mean for another project.
    fn integration_facts(&self, project: &WorkspaceProject) -> Vec<String> {
        let (manifest, knowledge) = self.previous(&project.path);
        let (Some(manifest), Some(knowledge)) = (manifest, knowledge) else {
            return Vec::new();
        };
        apply_choices(
            &knowledge.findings,
            &manifest.excluded,
            &manifest.corrections,
            &manifest.confirmed,
        )
        .into_iter()
        .filter(|f| {
            matches!(
                f.category,
                FindingCategory::Integration | FindingCategory::Database | FindingCategory::Data
            ) && matches!(
                f.origin,
                Origin::Fact | Origin::UserConfirmed | Origin::UserCorrected
            )
        })
        .take(MAX_FACTS_PER_PROJECT)
        .map(|f| {
            let source = f
                .evidence
                .first()
                .map(|e| format!(" ({}:{})", project.name, e.source))
                .unwrap_or_default();
            format!("{}: {}{source}", f.label, f.value)
        })
        .collect()
    }

    fn context_info(&self, workspace: &Workspace, digests: &[Digest]) -> WorkspaceContextInfo {
        let Some(store) = &self.workspace_store else {
            return WorkspaceContextInfo {
                exists: false,
                generated_at: None,
                outdated: false,
                has_notes: false,
            };
        };
        let generated = store.read(&workspace.id, ContextFile::Generated);
        let (stamp, generated_at) = generated.as_deref().map(parse_marker).unwrap_or_default();
        WorkspaceContextInfo {
            exists: generated.is_some(),
            generated_at,
            outdated: generated.is_some()
                && stamp.as_deref() != Some(&fingerprint(workspace, digests)),
            has_notes: store
                .read(&workspace.id, ContextFile::Notes)
                .is_some_and(|n| !n.trim().is_empty()),
        }
    }
}

fn state_of(exists: bool, summary: &HarnessSummary) -> ProjectHarnessState {
    if !exists {
        return ProjectHarnessState::FolderMissing;
    }
    match summary.status {
        HarnessStatus::NotInitialized => ProjectHarnessState::NotInitialized,
        HarnessStatus::NeedsReview => ProjectHarnessState::NeedsReview,
        HarnessStatus::Initialized if summary.staleness.is_some() => ProjectHarnessState::Outdated,
        HarnessStatus::Initialized => ProjectHarnessState::Ready,
    }
}

/// What the generated context depends on: when any of it changes, the context is out of date.
fn fingerprint(workspace: &Workspace, digests: &[Digest]) -> String {
    let mut lines = vec![workspace.name.clone()];
    for d in digests {
        let p = &d.project;
        lines.push(format!(
            "{}|{}|{}|{:?}|{}|{}|{:?}|{}",
            p.id,
            p.name,
            p.path,
            p.role,
            p.depends_on.join(","),
            d.exists,
            d.summary
                .as_ref()
                .map(|s| (s.status, s.analyzed_at, s.staleness.is_some())),
            d.facts.join(";")
        ));
    }
    sha256_hex(lines.join("\n").as_bytes())
}

/// The fingerprint and time in the first line of a generated context.
fn parse_marker(text: &str) -> (Option<String>, Option<u64>) {
    let Some(line) = text.lines().next().filter(|l| l.starts_with(MARKER)) else {
        return (None, None);
    };
    let value = |key: &str| {
        line.split_whitespace()
            .find_map(|part| part.strip_prefix(key))
            .map(str::to_owned)
    };
    (
        value("fingerprint="),
        value("generated=").and_then(|v| v.parse().ok()),
    )
}

fn render(workspace: &Workspace, digests: &[Digest], generated_at: u64) -> String {
    let mut out = format!(
        "{MARKER} fingerprint={} generated={generated_at} -->\n# Workspace: {}\n",
        fingerprint(workspace, digests),
        workspace.name
    );
    if let Some(description) = &workspace.description {
        out.push_str(description);
        out.push('\n');
    }
    out.push_str("\n## Projects\n");
    for d in digests {
        let p = &d.project;
        let role = p
            .role
            .and_then(|r| serde_json::to_value(r).ok())
            .and_then(|v| v.as_str().map(str::to_owned));
        let _ = write!(
            out,
            "\n### {}{}\n- Folder: {}\n",
            p.name,
            role.map(|r| format!(" — {r}")).unwrap_or_default(),
            p.path
        );
        match (&d.summary, d.exists) {
            (_, false) => out.push_str("- Harness: the folder was not found\n"),
            (Some(s), true) => {
                if !s.stack.is_empty() {
                    let _ = writeln!(out, "- Stack: {}", s.stack.join(", "));
                }
                let _ = writeln!(
                    out,
                    "- Harness: {}",
                    match (s.status, s.staleness.is_some()) {
                        (HarnessStatus::NotInitialized, _) => "not initialized",
                        (HarnessStatus::NeedsReview, _) => "needs review",
                        (HarnessStatus::Initialized, true) =>
                            "initialized, the project changed since",
                        (HarnessStatus::Initialized, false) => "initialized",
                    }
                );
            }
            (None, true) => {}
        }
        for fact in &d.facts {
            let _ = writeln!(out, "- Integration or data: {fact}");
        }
    }
    let declared: Vec<String> = digests
        .iter()
        .flat_map(|d| {
            d.project.depends_on.iter().filter_map(|id| {
                digests
                    .iter()
                    .find(|other| other.project.id == *id)
                    .map(|other| format!("- {} depends on {}", d.project.name, other.project.name))
            })
        })
        .collect();
    if !declared.is_empty() {
        out.push_str("\n## Relationships declared by the user\n");
        out.push_str(&declared.join("\n"));
        out.push('\n');
    }
    out.push_str(
        "\n## Reading this\nOnly what each project's Harness found, with its source, and what \
         the user declared, is listed. Anything about how the projects communicate that is not \
         listed here is not known: check the projects' code before assuming it.\n",
    );
    out
}

/// The generated context without its marker line, followed by the user's notes: what a prompt
/// is told about the workspace as a whole.
pub fn told(generated: Option<&str>, notes: Option<&str>) -> Option<String> {
    let body = generated
        .map(|text| {
            if text.starts_with(MARKER) {
                text.split_once('\n').map_or("", |(_, rest)| rest)
            } else {
                text
            }
            .trim()
            .to_owned()
        })
        .filter(|t| !t.is_empty());
    let notes = notes.map(str::trim).filter(|n| !n.is_empty());
    match (body, notes) {
        (None, None) => None,
        (Some(body), None) => Some(body),
        (None, Some(notes)) => Some(format!("Notes from the user:\n{notes}")),
        (Some(body), Some(notes)) => Some(format!("{body}\n\nNotes from the user:\n{notes}")),
    }
}

/// An in-memory store, for tests.
#[cfg(test)]
#[derive(Default)]
pub struct MemoryContextStore {
    files: Mutex<BTreeMap<(String, bool), String>>,
}

#[cfg(test)]
impl WorkspaceContextStore for MemoryContextStore {
    fn read(&self, workspace_id: &str, file: ContextFile) -> Option<String> {
        self.files
            .lock()
            .unwrap()
            .get(&(workspace_id.to_owned(), file == ContextFile::Notes))
            .cloned()
    }

    fn write(&self, workspace_id: &str, file: ContextFile, text: &str) -> Result<(), AppError> {
        self.files.lock().unwrap().insert(
            (workspace_id.to_owned(), file == ContextFile::Notes),
            text.to_owned(),
        );
        Ok(())
    }

    fn remove(&self, workspace_id: &str) {
        self.files
            .lock()
            .unwrap()
            .retain(|(id, _), _| id != workspace_id);
    }
}
