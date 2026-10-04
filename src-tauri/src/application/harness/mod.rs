//! Project Harness: what Atlas knows about a project, kept in the project's own `.atlas/`.
//!
//! ```text
//! ProjectScanner -> ScanSnapshot -> DeterministicAnalyzer -> findings
//!                        └-> RepositorySampler -> EvidenceBundle -> SemanticAnalyzer (opt-in) -> findings
//! findings -> ConsistencyChecker -> conflicts
//! findings + conflicts -> human review (diff, evidence, corrections, user knowledge)
//!   -> HarnessGenerator -> HarnessStore -> <project>/.atlas/
//! HarnessStore -> HarnessContextBuilder -> PromptBuilder -> Runtime      (what agents are told)
//! ```
//!
//! Evidence > inference > guess; unknown > invented. Nothing here executes project code, and
//! nothing knows a runtime's internals or a worktree. The project is the source of truth: Atlas
//! keeps no copy of the Harness in its own config.

pub mod analyzer;
pub mod consistency;
pub mod context;
pub mod fingerprint;
pub mod generator;
pub mod knowledge;
pub mod manifest;
pub mod negative;
pub mod sampler;
pub mod secrets;
pub mod semantic;
pub mod snapshot;
pub mod task_context;
pub mod taxonomy;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use self::analyzer::{AnalysisInput, AnalysisOutput, DeterministicAnalyzer, ProjectAnalyzer};
use self::consistency::ConsistencyInput;
use self::context::{HarnessContextBuilder, TaskContextLoad};
use self::fingerprint::{drift, Drift};
use self::generator::{read_user_files, user_files, ExistingUserFiles};
use self::knowledge::{apply_choices, diff, stamp_verification};
use self::manifest::{build_manifest, parse_knowledge, parse_manifest, summarize};
use self::sampler::{Purpose, SampleFile, SampleLimits};
use self::semantic::{Mode, SemanticAnalyzer, SemanticModelFactory};
use self::snapshot::ScanSnapshot;
use super::errors::{AppError, ErrorCode};
use super::support::{new_id, now_ms};
use super::workspace::WorkspaceService;
use crate::domain::harness::UserKnowledge;
use crate::domain::harness::{
    AnalysisInfo, Conflict, Finding, HarnessFile, HarnessKnowledge, HarnessManifest, HarnessStatus,
    HarnessSummary, InitMode, InitializeInput, InitializeOutcome, Origin, ProjectAnalysis,
    ProjectFingerprint, RefreshOutcome, SemanticReport, SemanticRequest, SemanticStatus,
    WriteReport, KNOWLEDGE_VERSION,
};
use crate::domain::task_context::{PreviewStatus, TaskContextPreview, TaskContextRequest};

/// Port: reads a project folder safely and reports what it saw. Implemented in `infrastructure/`.
///
/// Implementations must stay inside the folder, never follow symlinks, skip generated folders,
/// respect limits (depth, entries, file size), never read secrets and never run anything.
pub trait ProjectScanner: Send + Sync {
    /// # Errors
    ///
    /// `UnsafeProjectPath`, `ProjectNotFound`, `ProjectNotDirectory` or `ProjectAnalysisFailed`.
    fn scan(&self, project_path: &str) -> Result<ScanSnapshot, AppError>;

    /// Whether a project-relative path exists inside the project: a regular file or folder,
    /// reached without a symlink and without leaving the root. Never reads content.
    fn exists(&self, project_path: &str, relative: &str) -> bool;

    /// The text of the given files (relative paths the scan listed), each cut to
    /// `max_bytes`. A path that is unsafe, a symlink, not a regular file or that looks like a
    /// secret is simply not returned.
    fn read_files(
        &self,
        project_path: &str,
        paths: &[String],
        max_bytes: usize,
    ) -> Vec<(String, String)>;
}

/// Port: the `.atlas/` folder of a project.
pub trait HarnessStore: Send + Sync {
    /// The text of `.atlas/project.yaml`, or `None` if there is none.
    ///
    /// # Errors
    ///
    /// `HarnessInvalid` if it exists but cannot be read safely (a symlink, not a file).
    fn read_manifest(&self, project_path: &str) -> Result<Option<String>, AppError>;
    fn has_atlas_dir(&self, project_path: &str) -> bool;
    /// `.atlas/context/<name>.md`, if present and readable.
    fn read_context(&self, project_path: &str, name: &str) -> Option<String>;
    /// Any file under `.atlas/` (relative path), if present.
    ///
    /// # Errors
    ///
    /// `HarnessInvalid` if the path is unsafe or the file cannot be read safely.
    fn read_file(&self, project_path: &str, path: &str) -> Result<Option<String>, AppError>;
    /// Writes files under `.atlas/`. Anything it would replace is copied to a backup first, and
    /// nothing outside `.atlas/` is touched.
    ///
    /// # Errors
    ///
    /// `HarnessGenerationFailed` if a file cannot be written safely.
    fn write(&self, project_path: &str, files: &[HarnessFile]) -> Result<WriteReport, AppError>;
}

/// What an analysis found, kept between "analyze" and "initialize" so the Harness is built from
/// exactly what the user reviewed (a model is not asked twice, and the webview cannot supply
/// findings of its own).
#[derive(Clone)]
struct AnalysisState {
    project_name: String,
    deterministic: Vec<Finding>,
    semantic: Vec<Finding>,
    documents: Vec<SampleFile>,
    info: AnalysisInfo,
    /// What a model drafted for the user's business fields (only on the analysis that asked).
    suggestions: UserKnowledge,
}

impl AnalysisState {
    /// Deterministic facts first: where both say the same thing, the facts' version stands.
    /// A statement that points at no real evidence is never kept, whoever made it.
    fn raw_findings(&self) -> Vec<Finding> {
        let mut all = self.deterministic.clone();
        for f in &self.semantic {
            if !all.iter().any(|d| d.id == f.id) {
                all.push(f.clone());
            }
        }
        all.retain(|f| f.origin == Origin::Generated || f.has_valid_evidence());
        stamp_verification(&mut all, self.info.analyzed_at);
        all
    }
}

/// Use case: analyse a workspace's project, create or update its Harness, report its state.
pub struct HarnessService {
    workspaces: Arc<WorkspaceService>,
    scanner: Arc<dyn ProjectScanner>,
    store: Arc<dyn HarnessStore>,
    analyzer: DeterministicAnalyzer,
    semantic: Option<Arc<dyn SemanticModelFactory>>,
    limits: SampleLimits,
    pending: Mutex<HashMap<String, AnalysisState>>,
    /// Chooses what a task is told of the Harness. Shared with the execution service so that a
    /// preview shows what a run would use.
    context: Option<Arc<HarnessContextBuilder>>,
}

/// The user's choices about the findings, as stored in the manifest.
#[derive(Default, Clone)]
struct Choices {
    excluded: Vec<String>,
    corrections: BTreeMap<String, String>,
    confirmed: Vec<String>,
}

impl HarnessService {
    pub fn new(
        workspaces: Arc<WorkspaceService>,
        scanner: Arc<dyn ProjectScanner>,
        store: Arc<dyn HarnessStore>,
    ) -> Self {
        Self {
            workspaces,
            scanner,
            store,
            analyzer: DeterministicAnalyzer::default(),
            semantic: None,
            limits: SampleLimits::default(),
            pending: Mutex::new(HashMap::new()),
            context: None,
        }
    }

    /// The builder that previews a task's context.
    #[must_use]
    pub fn with_context_builder(mut self, builder: Arc<HarnessContextBuilder>) -> Self {
        self.context = Some(builder);
        self
    }

    /// What an agent would be told for `request.task`, chosen from the workspace's Harness, with
    /// the reason for every choice. Reads only what a run reads; changes nothing.
    ///
    /// # Errors
    ///
    /// Fails if the workspace does not exist.
    pub fn preview_task_context(
        &self,
        request: &TaskContextRequest,
    ) -> Result<TaskContextPreview, AppError> {
        let path = self.project_path(&request.workspace_id)?;
        let Some(builder) = &self.context else {
            return Ok(TaskContextPreview {
                status: PreviewStatus::Missing,
                context: None,
            });
        };
        Ok(match builder.build_for_task(&path, &request.task) {
            TaskContextLoad::Ready(context) => TaskContextPreview {
                status: PreviewStatus::Ready,
                context: Some(*context),
            },
            TaskContextLoad::Missing => TaskContextPreview {
                status: PreviewStatus::Missing,
                context: None,
            },
            TaskContextLoad::Invalid => TaskContextPreview {
                status: PreviewStatus::Invalid,
                context: None,
            },
        })
    }

    /// Lets an analysis ask a model (opt-in, per request) to read the selected evidence.
    #[must_use]
    pub fn with_semantic(mut self, factory: Arc<dyn SemanticModelFactory>) -> Self {
        self.semantic = Some(factory);
        self
    }

    #[cfg(test)]
    pub fn workspaces_for_tests(&self) -> Arc<WorkspaceService> {
        self.workspaces.clone()
    }

    #[cfg(test)]
    #[must_use]
    pub fn with_limits(mut self, limits: SampleLimits) -> Self {
        self.limits = limits;
        self
    }

    fn project_path(&self, workspace_id: &str) -> Result<String, AppError> {
        self.workspaces
            .get(workspace_id)
            .map(|w| w.project_path)
            .ok_or_else(|| AppError::new(ErrorCode::WorkspaceNotFound))
    }

    /// The state of the project's Harness right now.
    ///
    /// # Errors
    ///
    /// Fails if the workspace does not exist.
    pub fn get(&self, workspace_id: &str) -> Result<HarnessSummary, AppError> {
        let path = self.project_path(workspace_id)?;
        Ok(self.summary_at(&path))
    }

    fn knowledge_text(&self, path: &str) -> Option<String> {
        self.store
            .read_file(path, "knowledge/findings.yaml")
            .ok()
            .flatten()
    }

    /// The project's relevant state right now. `None` when it cannot be read: staleness is then
    /// simply not claimed.
    fn current_fingerprint(&self, path: &str) -> Option<ProjectFingerprint> {
        self.scanner
            .scan(path)
            .ok()
            .map(|snapshot| fingerprint::compute(&snapshot))
    }

    fn summary_at(&self, path: &str) -> HarnessSummary {
        let has_dir = self.store.has_atlas_dir(path);
        match self.store.read_manifest(path) {
            Ok(text) => summarize(
                text.as_deref(),
                self.knowledge_text(path).as_deref(),
                has_dir,
                self.current_fingerprint(path).as_ref(),
            ),
            Err(_) => HarnessSummary {
                problem: Some("harness_invalid".to_owned()),
                status: HarnessStatus::NeedsReview,
                ..HarnessSummary::not_initialized(has_dir)
            },
        }
    }

    fn previous(&self, path: &str) -> (Option<HarnessManifest>, Option<HarnessKnowledge>) {
        let manifest = self
            .store
            .read_manifest(path)
            .ok()
            .flatten()
            .and_then(|t| parse_manifest(&t).ok());
        let knowledge = manifest
            .as_ref()
            .and_then(|_| self.knowledge_text(path))
            .and_then(|t| parse_knowledge(&t).ok());
        (manifest, knowledge)
    }

    fn user_files(&self, path: &str) -> ExistingUserFiles {
        let read = |name: &str| self.store.read_context(path, name);
        ExistingUserFiles {
            business: read("business"),
            constraints: read("constraints"),
            decisions: read("decisions"),
        }
    }

    /// Scans, runs the rules, samples evidence and, if asked, the model. A model that fails is
    /// reported in the result; it does not fail the analysis.
    fn gather(
        &self,
        workspace_id: &str,
        path: &str,
        semantic: Option<&SemanticRequest>,
        previous: Option<&HarnessKnowledge>,
    ) -> Result<(ScanSnapshot, AnalysisState), AppError> {
        let snapshot = self.scanner.scan(path)?;
        let deterministic = self
            .analyzer
            .analyze_project(&AnalysisInput {
                snapshot: &snapshot,
                evidence: None,
                deterministic: &[],
                exists: None,
            })?
            .findings;

        // Code samples are only read when a model that cannot explore is going to see them.
        let purpose = if semantic.is_some_and(|r| r.restricted) {
            Purpose::Semantic
        } else {
            Purpose::Documents
        };
        let selection = sampler::select(&snapshot, self.limits, purpose);
        let read = self
            .scanner
            .read_files(path, &selection.to_read, self.limits.max_file_bytes);
        let bundle = sampler::complete(selection, read, self.limits);

        let mut info = AnalysisInfo {
            partial: snapshot.truncated || (semantic.is_some() && bundle.partial),
            scanned_entries: snapshot.entries.len(),
            sampled_files: bundle.file_paths(),
            semantic: SemanticReport::default(),
            analyzed_at: now_ms(),
            fingerprint: Some(fingerprint::compute(&snapshot)),
        };
        let mut found = Vec::new();
        let mut suggestions = UserKnowledge::default();
        if let Some(request) = semantic {
            let result = self.run_semantic(
                workspace_id,
                path,
                request,
                &snapshot,
                &bundle,
                &deterministic,
            );
            info.semantic = semantic::report(&result, &bundle, !request.restricted);
            if let Ok(output) = result {
                suggestions = output.suggestions.unwrap_or_default();
                found = output.findings;
            }
        } else if let Some(previous) = previous {
            // No new model run: keep what an earlier one proposed, where its evidence still
            // exists. What no longer has any is dropped, and shows up in the diff as removed.
            info.semantic = previous.analysis.semantic.clone();
            found = carried_over(previous, &snapshot);
        }
        // Whatever a model proposed must point at evidence that exists.
        let proposed = found.len();
        found.retain(Finding::has_valid_evidence);
        info.semantic.rejected += proposed - found.len();
        if info.semantic.status == SemanticStatus::NotRun {
            info.semantic.sent_files = Vec::new();
        }
        Ok((
            snapshot.clone(),
            AnalysisState {
                project_name: snapshot.root_name,
                deterministic,
                semantic: found,
                documents: bundle.files,
                info,
                suggestions,
            },
        ))
    }

    fn run_semantic(
        &self,
        workspace_id: &str,
        path: &str,
        request: &SemanticRequest,
        snapshot: &ScanSnapshot,
        bundle: &sampler::EvidenceBundle,
        deterministic: &[Finding],
    ) -> Result<AnalysisOutput, AppError> {
        let factory = self
            .semantic
            .as_ref()
            .ok_or_else(|| AppError::new(ErrorCode::SemanticUnsupported))?;
        let model = factory.model_for(workspace_id, &request.agent_id, path, request.restricted)?;
        let mode = if request.restricted {
            Mode::Restricted
        } else {
            Mode::Explore {
                instructions: request.instructions.clone(),
            }
        };
        let exists = |relative: &str| self.scanner.exists(path, relative);
        SemanticAnalyzer::new(model, mode).analyze_project(&AnalysisInput {
            snapshot,
            evidence: Some(bundle),
            deterministic,
            exists: Some(&exists),
        })
    }

    fn conflicts(
        state: &AnalysisState,
        previous: Option<&HarnessKnowledge>,
        corrections: &BTreeMap<String, String>,
    ) -> Vec<Conflict> {
        consistency::check(&ConsistencyInput {
            deterministic: &state.deterministic,
            semantic: &state.semantic,
            documents: &state.documents,
            previous,
            corrections,
        })
    }

    /// Reads the project and reports what it found, with evidence, conflicts and — when a
    /// Harness already exists — how this differs from it. Writes nothing. With `semantic`, a
    /// model is shown the selected evidence (listed in the result) and nothing else.
    ///
    /// # Errors
    ///
    /// Fails if the workspace or its folder does not exist, the path is unsafe, or the scan fails.
    pub fn analyze(
        &self,
        workspace_id: &str,
        semantic: Option<&SemanticRequest>,
    ) -> Result<ProjectAnalysis, AppError> {
        let path = self.project_path(workspace_id)?;
        let (manifest, knowledge) = self.previous(&path);
        let (_, state) = self.gather(workspace_id, &path, semantic, knowledge.as_ref())?;
        let choices = manifest.as_ref().map(choices_of).unwrap_or_default();
        let conflicts = Self::conflicts(&state, knowledge.as_ref(), &choices.corrections);
        let raw = state.raw_findings();

        let (user, unmanaged) = read_user_files(&self.user_files(&path));
        let analysis = ProjectAnalysis {
            project_name: state.project_name.clone(),
            path: path.clone(),
            gaps: negative::gaps(
                &apply_choices(
                    &raw,
                    &choices.excluded,
                    &choices.corrections,
                    &choices.confirmed,
                ),
                &state.info,
            ),
            findings: raw.clone(),
            conflicts,
            partial: state.info.partial,
            scanned_entries: state.info.scanned_entries,
            analysis: state.info.clone(),
            existing: Some(self.summary_at(&path)),
            diff: knowledge.as_ref().map(|k| diff(&k.findings, &raw)),
            previous_corrections: choices.corrections,
            previous_excluded: choices.excluded,
            previous_confirmed: choices.confirmed,
            user,
            suggested_user: state.suggestions.clone(),
            unmanaged_user_files: unmanaged,
        };
        self.pending
            .lock()
            .expect("pending analyses lock")
            .insert(workspace_id.to_owned(), state);
        Ok(analysis)
    }

    /// Creates or updates `.atlas/` from the analysis the user reviewed and their decisions.
    ///
    /// # Errors
    ///
    /// `ProjectAlreadyInitialized` when creating over an existing manifest, `HarnessInvalid` /
    /// `HarnessNotInitialized` when "use existing" has nothing valid to use, and the analysis and
    /// writing errors.
    #[allow(clippy::needless_pass_by_value)]
    pub fn initialize(
        &self,
        workspace_id: &str,
        input: InitializeInput,
    ) -> Result<InitializeOutcome, AppError> {
        let path = self.project_path(workspace_id)?;
        let existing = self.store.read_manifest(&path)?;
        match (input.mode, &existing) {
            (InitMode::UseExisting, None) => {
                return Err(AppError::new(ErrorCode::HarnessNotInitialized));
            }
            (InitMode::UseExisting, Some(text)) => {
                parse_manifest(text)?;
                return Ok(self.outcome(&path, WriteReport::default(), Vec::new()));
            }
            (InitMode::Create, Some(_)) => {
                return Err(AppError::new(ErrorCode::ProjectAlreadyInitialized));
            }
            _ => {}
        }
        // An unreadable manifest is replaced, but only after being backed up by the store.
        let (previous_manifest, previous_knowledge) = self.previous(&path);
        let state = self.take_state(workspace_id, &path, previous_knowledge.as_ref())?;
        let choices = Choices {
            excluded: input.excluded.clone(),
            corrections: input.corrections.clone(),
            confirmed: input.confirmed.clone(),
        };
        self.apply(
            &path,
            &state,
            previous_manifest.as_ref(),
            previous_knowledge.as_ref(),
            choices,
            &input.user_knowledge(),
        )
    }

    /// The analysis the user just reviewed, or a fresh deterministic one.
    fn take_state(
        &self,
        workspace_id: &str,
        path: &str,
        previous: Option<&HarnessKnowledge>,
    ) -> Result<AnalysisState, AppError> {
        if let Some(state) = self
            .pending
            .lock()
            .expect("pending analyses lock")
            .remove(workspace_id)
        {
            return Ok(state);
        }
        Ok(self.gather(workspace_id, path, None, previous)?.1)
    }

    fn apply(
        &self,
        path: &str,
        state: &AnalysisState,
        previous_manifest: Option<&HarnessManifest>,
        previous_knowledge: Option<&HarnessKnowledge>,
        choices: Choices,
        wanted_user: &crate::domain::harness::UserKnowledge,
    ) -> Result<InitializeOutcome, AppError> {
        let raw = state.raw_findings();
        let knowledge = HarnessKnowledge {
            version: KNOWLEDGE_VERSION,
            analysis: state.info.clone(),
            findings: raw.clone(),
            conflicts: Self::conflicts(state, previous_knowledge, &choices.corrections),
        };
        let effective = apply_choices(
            &raw,
            &choices.excluded,
            &choices.corrections,
            &choices.confirmed,
        );
        let manifest = build_manifest(
            previous_manifest.map_or_else(|| new_id("prj"), |m| m.project.id.clone()),
            &state.project_name,
            previous_manifest.map_or_else(now_ms, |m| m.project.initialized_at),
            &effective,
            choices.corrections,
            choices.excluded,
            choices.confirmed,
        );
        let mut files = generator::generate(&manifest, &knowledge, &effective)?;
        let (user_files, left) = user_files(&self.user_files(path), wanted_user);
        files.extend(user_files);
        let report = self.store.write(path, &files)?;
        Ok(self.outcome(path, report, left))
    }

    /// Re-analyses and compares with the Harness's stored knowledge. Without `confirm` nothing is
    /// written: the result is the diff and the conflicts, for the user to review. With it, the
    /// generated files are updated (backed up first), the user's earlier corrections stay, and
    /// their own files are left exactly as they are.
    ///
    /// # Errors
    ///
    /// `HarnessNotInitialized` or `HarnessInvalid` if there is no usable Harness to refresh.
    pub fn refresh(&self, workspace_id: &str, confirm: bool) -> Result<RefreshOutcome, AppError> {
        let path = self.project_path(workspace_id)?;
        let Some(text) = self.store.read_manifest(&path)? else {
            return Err(AppError::new(ErrorCode::HarnessNotInitialized));
        };
        let manifest = parse_manifest(&text)?;
        let (_, knowledge) = self.previous(&path);
        let (_, state) = self.gather(workspace_id, &path, None, knowledge.as_ref())?;
        let choices = choices_of(&manifest);
        let raw = state.raw_findings();
        let changes = knowledge
            .as_ref()
            .map(|k| diff(&k.findings, &raw))
            .unwrap_or_default();
        let conflicts = Self::conflicts(&state, knowledge.as_ref(), &choices.corrections);
        let staleness = knowledge.as_ref().and_then(|k| {
            match drift(&k.analysis, state.info.fingerprint.as_ref()) {
                Drift::Changed(s) => Some(s),
                _ => None,
            }
        });
        let applied = if confirm {
            // The user's own files are passed as they are, so nothing of theirs changes.
            let (current, _) = read_user_files(&self.user_files(&path));
            Some(self.apply(
                &path,
                &state,
                Some(&manifest),
                knowledge.as_ref(),
                choices,
                &current,
            )?)
        } else {
            None
        };
        Ok(RefreshOutcome {
            staleness,
            diff: changes,
            conflicts,
            applied,
        })
    }

    fn outcome(
        &self,
        path: &str,
        report: WriteReport,
        left_untouched: Vec<String>,
    ) -> InitializeOutcome {
        InitializeOutcome {
            summary: self.summary_at(path),
            written: report.written,
            backed_up: report.backed_up,
            left_untouched,
        }
    }
}

fn choices_of(manifest: &HarnessManifest) -> Choices {
    Choices {
        excluded: manifest.excluded.clone(),
        corrections: manifest.corrections.clone(),
        confirmed: manifest.confirmed.clone(),
    }
}

/// What an earlier model run proposed, kept where its evidence still exists in the project.
fn carried_over(previous: &HarnessKnowledge, snapshot: &ScanSnapshot) -> Vec<Finding> {
    previous
        .findings
        .iter()
        .filter(|f| f.by_model)
        .filter_map(|f| {
            let mut kept = f.clone();
            kept.evidence.retain(|e| snapshot.has_path(&e.source));
            (!kept.evidence.is_empty()).then_some(kept)
        })
        .collect()
}

#[cfg(test)]
pub mod fake {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use super::{HarnessStore, ProjectScanner};
    use crate::application::errors::AppError;
    use crate::application::harness::snapshot::ScanSnapshot;
    use crate::domain::harness::{HarnessFile, WriteReport};

    /// Returns one snapshot for every folder; a test can change it between analyses.
    pub struct FakeScanner(pub Mutex<ScanSnapshot>);

    impl FakeScanner {
        pub fn new(snapshot: ScanSnapshot) -> Self {
            Self(Mutex::new(snapshot))
        }

        pub fn set(&self, snapshot: ScanSnapshot) {
            *self.0.lock().unwrap() = snapshot;
        }
    }

    impl ProjectScanner for FakeScanner {
        fn scan(&self, _: &str) -> Result<ScanSnapshot, AppError> {
            Ok(self.0.lock().unwrap().clone())
        }

        fn exists(&self, _: &str, relative: &str) -> bool {
            let relative = relative.trim_end_matches('/');
            self.0
                .lock()
                .unwrap()
                .entries
                .iter()
                .any(|e| e.path == relative)
        }

        fn read_files(&self, _: &str, paths: &[String], _: usize) -> Vec<(String, String)> {
            let snapshot = self.0.lock().unwrap();
            paths
                .iter()
                .filter_map(|p| Some((p.clone(), snapshot.files.get(p)?.clone())))
                .collect()
        }
    }

    /// `.atlas/` in memory: relative path -> content.
    #[derive(Default)]
    pub struct MemoryHarnessStore {
        pub files: Mutex<BTreeMap<String, String>>,
    }

    impl MemoryHarnessStore {
        pub fn with(files: &[(&str, &str)]) -> Self {
            Self {
                files: Mutex::new(
                    files
                        .iter()
                        .map(|(p, c)| ((*p).to_owned(), (*c).to_owned()))
                        .collect(),
                ),
            }
        }

        pub fn get(&self, path: &str) -> Option<String> {
            self.files.lock().unwrap().get(path).cloned()
        }
    }

    impl HarnessStore for MemoryHarnessStore {
        fn read_manifest(&self, _: &str) -> Result<Option<String>, AppError> {
            Ok(self.get("project.yaml"))
        }

        fn has_atlas_dir(&self, _: &str) -> bool {
            !self.files.lock().unwrap().is_empty()
        }

        fn read_context(&self, _: &str, name: &str) -> Option<String> {
            self.get(&format!("context/{name}.md"))
        }

        fn read_file(&self, _: &str, path: &str) -> Result<Option<String>, AppError> {
            Ok(self.get(path))
        }

        fn write(&self, _: &str, files: &[HarnessFile]) -> Result<WriteReport, AppError> {
            let mut store = self.files.lock().unwrap();
            let mut report = WriteReport::default();
            for file in files {
                match store.get(&file.path).cloned() {
                    Some(_) if file.keep_if_exists => {
                        report.skipped.push(file.path.clone());
                        continue;
                    }
                    Some(old) if old == file.content => continue,
                    Some(old) => {
                        store.insert(format!("backups/{}", file.path), old);
                        report.backed_up.push(file.path.clone());
                    }
                    None => {}
                }
                store.insert(file.path.clone(), file.content.clone());
                report.written.push(file.path.clone());
            }
            Ok(report)
        }
    }
}
