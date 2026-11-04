//! Use case: specifications. Keeps them (one record each, in Atlas's own data), moves them from
//! state to state through the rules of `domain::spec::rules`, and does what needs the outside:
//! reading and writing the project's `openspec/` folder, and asking the agents' service who exists.
//! It never runs an agent: that is [`super::runner`], which reports back what happened.
//!
//! Every change goes through [`SpecService::modify`]: made on a copy, saved, and only then
//! shown; if the save fails the spec is what it was.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use super::framework::{
    ChangeFolder, ChangeIdProblem, Documents, ImportProblem, ProjectSpecs, SpecFramework, TaskItem,
};
use super::ports::{FilesError, ProjectFiles, SpecStore};
use super::view::{self, Availability, ImportCandidate, IntegrationNote, SpecSummary, SpecView};
use crate::application::agents::AgentService;
use crate::application::errors::{AppError, ErrorCode};
use crate::application::support::{new_id, now_ms, LockExt};
use crate::application::workspace::WorkspaceService;
use crate::domain::interaction::MAX_ANSWER;
use crate::domain::spec::rules::{self, AskedQuestion, NewSpec, TransitionError};
use crate::domain::spec::{
    ArtifactKind, ArtifactValidation, ConsistencyValidation, DecisionKind, Finding, FindingStatus,
    FrameworkKind, GenerationMode, ImplementationLink, IssueSeverity, Publication, QuestionSource,
    Review, Revision, RevisionOrigin, RunOutcome, Spec, SpecFile, SpecOrigin, MAX_CONTEXT,
    MAX_NOTE, MAX_REQUEST, MAX_TITLE,
};

/// Told every time a spec changes, so a screen that shows it can read it again.
pub trait SpecEvents: Send + Sync {
    fn changed(&self, spec_id: &str, version: u64);
}

/// For tests.
#[cfg(test)]
pub struct NoEvents;

#[cfg(test)]
impl SpecEvents for NoEvents {
    fn changed(&self, _spec_id: &str, _version: u64) {}
}

/// What a person gives to start a spec.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSpecRequest {
    pub workspace_id: String,
    /// The project the spec is for; the workspace's primary project when absent.
    #[serde(default)]
    pub project_id: Option<String>,
    pub title: String,
    pub request: String,
    #[serde(default)]
    pub context: String,
    pub author_agent_id: String,
    /// The agent that reviews; the author again when absent.
    #[serde(default)]
    pub reviewer_agent_id: Option<String>,
    /// The workspace's integrations (connection ids) to test and review the spec with.
    #[serde(default)]
    pub integrations: Vec<String>,
}

/// How a generation ended, as the runner reports it.
pub enum GenerationEnd {
    Document(Vec<SpecFile>),
    Question(AskedQuestion),
    Failed {
        message: String,
        outcome: RunOutcome,
    },
}

/// How a review ended.
pub enum ReviewEnd {
    Report {
        summary: String,
        findings: Vec<Finding>,
    },
    Question(AskedQuestion),
    Failed {
        message: String,
        outcome: RunOutcome,
    },
}

/// What an implementing run is given of a ready spec.
#[derive(Debug, Clone)]
pub struct ImplementationContext {
    pub spec_id: String,
    /// The project the spec is for: where the run works.
    pub project_id: String,
    /// The workspace's other projects, which the run may read.
    pub sibling_project_ids: Vec<String>,
    pub title: String,
    pub change_id: String,
    /// The task, short: what the run is for.
    pub task: String,
    /// What the documents say, bounded, for the agent's brief.
    pub digest: String,
    pub revisions: BTreeMap<String, u32>,
    pub tasks: Vec<TaskItem>,
    /// Where the files are in the project, when published.
    pub published_root: Option<String>,
    /// The integrations chosen for testing and review, switched on now.
    pub integrations: Vec<IntegrationNote>,
}

/// Longest digest an implementing agent is given.
pub const DIGEST_CHARS: usize = 24_000;

pub struct SpecService {
    store: Arc<dyn SpecStore>,
    files: Arc<dyn ProjectFiles>,
    framework: Arc<dyn SpecFramework>,
    workspaces: Arc<WorkspaceService>,
    agents: Arc<AgentService>,
    events: Arc<dyn SpecEvents>,
    specs: Mutex<BTreeMap<String, Spec>>,
    launching: Arc<Mutex<BTreeSet<String>>>,
    /// What Atlas observed of every execution; the spec's own runs are looked up in it.
    usage: Option<Arc<crate::application::usage::UsageLedger>>,
    /// The configured token budget of a spec, read when it is needed: it can be changed any time.
    budget: Option<Arc<dyn Fn() -> Option<u64> + Send + Sync>>,
    /// The workspace's integrations, to look up the ones a spec names.
    mcp: Option<Arc<crate::application::mcp::McpService>>,
}

/// Held while a run is being set up for a spec: a second launch for the same spec is refused
/// until this is dropped.
pub struct LaunchGuard {
    launching: Arc<Mutex<BTreeSet<String>>>,
    spec_id: String,
}

impl Drop for LaunchGuard {
    fn drop(&mut self) {
        self.launching.lock_or_recover().remove(&self.spec_id);
    }
}

pub fn app_error(error: TransitionError) -> AppError {
    match error {
        TransitionError::ArtifactNotFound => AppError::new(ErrorCode::SpecArtifactNotFound),
        TransitionError::QuestionNotFound => AppError::new(ErrorCode::SpecQuestionNotFound),
        TransitionError::FindingNotFound => AppError::new(ErrorCode::SpecFindingNotFound),
        TransitionError::Archived => state("archived"),
        TransitionError::NotStarted => state("not_started"),
        TransitionError::RunInProgress => AppError::new(ErrorCode::SpecRunInProgress),
        TransitionError::DependenciesNotApproved(ids) => {
            AppError::new(ErrorCode::SpecDependenciesNotApproved).with("artifacts", ids.join(","))
        }
        TransitionError::WrongState(reason) => state(reason),
        TransitionError::RevisionOutdated { current } => {
            AppError::new(ErrorCode::SpecRevisionOutdated).with("current", current.to_string())
        }
        TransitionError::NoteRequired => AppError::new(ErrorCode::SpecNoteRequired),
        TransitionError::Late => state("late"),
        TransitionError::ValidationBlocking => AppError::new(ErrorCode::SpecValidationBlocking),
        TransitionError::NotReady => AppError::new(ErrorCode::SpecNotReady),
    }
}

fn state(reason: &str) -> AppError {
    AppError::new(ErrorCode::SpecStateInvalid).with("reason", reason)
}

fn invalid(reason: &str) -> AppError {
    AppError::new(ErrorCode::SpecInvalid).with("reason", reason)
}

fn change_id_error(problem: ChangeIdProblem) -> AppError {
    let reason = match problem {
        ChangeIdProblem::Empty => "empty",
        ChangeIdProblem::Format => "format",
        ChangeIdProblem::TooLong => "too_long",
    };
    AppError::new(ErrorCode::SpecChangeIdInvalid).with("reason", reason)
}

fn clip_chars(text: &str, max: usize) -> String {
    text.trim().chars().take(max).collect()
}

impl SpecService {
    pub fn new(
        store: Arc<dyn SpecStore>,
        files: Arc<dyn ProjectFiles>,
        framework: Arc<dyn SpecFramework>,
        workspaces: Arc<WorkspaceService>,
        agents: Arc<AgentService>,
        events: Arc<dyn SpecEvents>,
    ) -> Self {
        let (loaded, unreadable) = store.load_all();
        for problem in unreadable {
            log::warn!("a spec record could not be read and was left as it is: {problem}");
        }
        Self {
            store,
            files,
            framework,
            workspaces,
            agents,
            events,
            specs: Mutex::new(loaded.into_iter().map(|s| (s.id.clone(), s)).collect()),
            launching: Arc::new(Mutex::new(BTreeSet::new())),
            usage: None,
            budget: None,
            mcp: None,
        }
    }

    /// Where the integrations a spec names are looked up.
    #[must_use]
    pub fn with_mcp(mut self, mcp: Arc<crate::application::mcp::McpService>) -> Self {
        self.mcp = Some(mcp);
        self
    }

    /// Where the token budget of a spec is read from.
    #[must_use]
    pub fn with_budget(mut self, budget: Arc<dyn Fn() -> Option<u64> + Send + Sync>) -> Self {
        self.budget = Some(budget);
        self
    }

    /// Lets the spec's views say what its runs used.
    #[must_use]
    pub fn with_usage(mut self, usage: Arc<crate::application::usage::UsageLedger>) -> Self {
        self.usage = Some(usage);
        self
    }

    pub fn framework(&self) -> &Arc<dyn SpecFramework> {
        &self.framework
    }

    // ---- reading ---------------------------------------------------------------------------------

    pub fn get(&self, id: &str) -> Option<Spec> {
        self.specs.lock_or_recover().get(id).cloned()
    }

    fn require(&self, id: &str) -> Result<Spec, AppError> {
        self.get(id)
            .ok_or_else(|| AppError::new(ErrorCode::SpecNotFound))
    }

    pub fn list(&self, workspace_id: &str) -> Vec<SpecSummary> {
        let mut specs: Vec<SpecSummary> = self
            .specs
            .lock_or_recover()
            .values()
            .filter(|s| s.workspace_id == workspace_id)
            .map(view::summarize)
            .collect();
        specs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(a.id.cmp(&b.id)));
        specs
    }

    fn availability(&self, spec: &Spec) -> Availability {
        let project = self
            .workspaces
            .get(&spec.workspace_id)
            .is_some_and(|w| w.project(&spec.project_id).is_some());
        let usable = |agent_id: &str| self.agents.find(agent_id).is_some();
        Availability {
            project,
            author: usable(&spec.author_agent_id),
            reviewer: usable(&spec.reviewer_agent_id),
        }
    }

    pub fn view_of(&self, spec: &Spec) -> SpecView {
        let runs: Vec<&str> = spec.runs.iter().map(|r| r.id.as_str()).collect();
        let records = self
            .usage
            .as_ref()
            .map(|u| u.records_of(&runs))
            .unwrap_or_default();
        view::build(
            spec,
            self.framework.as_ref(),
            &self.availability(spec),
            &records,
            self.budget.as_ref().and_then(|read| read()),
            self.integrations_of(spec),
        )
    }

    /// # Errors
    ///
    /// When there is no such spec.
    pub fn view(&self, id: &str) -> Result<SpecView, AppError> {
        Ok(self.view_of(&self.require(id)?))
    }

    /// The root folder of the spec's project, if the workspace still has it.
    pub fn project_root(&self, spec: &Spec) -> Option<String> {
        self.workspaces
            .get(&spec.workspace_id)?
            .project(&spec.project_id)
            .map(|p| p.path.clone())
    }

    /// The folders of every project of the spec's workspace, the spec's own project first. A spec
    /// belongs to the workspace, so publishing writes it to all of them.
    pub fn project_roots(&self, spec: &Spec) -> Vec<String> {
        let Some(workspace) = self.workspaces.get(&spec.workspace_id) else {
            return Vec::new();
        };
        let mut roots: Vec<String> = workspace
            .project(&spec.project_id)
            .map(|p| p.path.clone())
            .into_iter()
            .collect();
        roots.extend(
            workspace
                .projects
                .iter()
                .filter(|p| p.id != spec.project_id)
                .map(|p| p.path.clone()),
        );
        roots
    }

    /// The other projects of the spec's workspace. A request often spans several (a backend and
    /// its frontend), so the agents may read them all; the spec's own project is where it lives.
    pub fn sibling_project_ids(&self, spec: &Spec) -> Vec<String> {
        self.workspaces
            .get(&spec.workspace_id)
            .map(|w| {
                w.projects
                    .iter()
                    .filter(|p| p.id != spec.project_id)
                    .map(|p| p.id.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The names of the workspace's projects, the spec's own first, each with its role if it has one.
    pub fn workspace_project_names(&self, spec: &Spec) -> Vec<String> {
        let Some(workspace) = self.workspaces.get(&spec.workspace_id) else {
            return Vec::new();
        };
        let mut projects: Vec<_> = workspace.projects.iter().collect();
        projects.sort_by_key(|p| p.id != spec.project_id);
        projects
            .into_iter()
            .map(|p| match p.role {
                Some(role) => format!(
                    "{} ({})",
                    p.name,
                    serde_json::to_value(role)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_owned))
                        .unwrap_or_default()
                ),
                None => p.name.clone(),
            })
            .collect()
    }

    pub fn project_specs(&self, spec: &Spec) -> ProjectSpecs {
        ProjectSpecs {
            capabilities: self
                .project_root(spec)
                .map(|root| self.files.main_specs(&root))
                .unwrap_or_default(),
        }
    }

    /// The integrations the spec names that its workspace still has.
    pub fn integrations_of(&self, spec: &Spec) -> Vec<view::IntegrationInfo> {
        let Some(mcp) = &self.mcp else {
            return Vec::new();
        };
        let overview = mcp.overview(&spec.workspace_id);
        spec.integrations
            .iter()
            .filter_map(|id| overview.connections.iter().find(|c| c.id == *id))
            .map(|c| view::IntegrationInfo {
                id: c.id.clone(),
                name: c.name.clone(),
                enabled: c.enabled,
                tools: c
                    .discovery
                    .as_ref()
                    .map(|d| d.tools.clone())
                    .unwrap_or_default(),
            })
            .collect()
    }

    /// What an agent is told of the integrations: only the ones that are switched on.
    pub fn integration_notes(&self, spec: &Spec) -> Vec<IntegrationNote> {
        self.integrations_of(spec)
            .into_iter()
            .filter(|i| i.enabled)
            .map(|i| IntegrationNote {
                name: i.name,
                tools: i.tools,
            })
            .collect()
    }

    /// The ids that are connections of the workspace, once each.
    fn known_connections(
        &self,
        workspace_id: &str,
        ids: &[String],
    ) -> Result<Vec<String>, AppError> {
        let Some(mcp) = &self.mcp else {
            return if ids.is_empty() {
                Ok(Vec::new())
            } else {
                Err(invalid("integration_invalid"))
            };
        };
        let overview = mcp.overview(workspace_id);
        let mut out: Vec<String> = Vec::new();
        for id in ids {
            if !overview.connections.iter().any(|c| c.id == *id) {
                return Err(invalid("integration_invalid"));
            }
            if !out.contains(id) {
                out.push(id.clone());
            }
        }
        Ok(out)
    }

    /// The person chooses which of the workspace's integrations this spec is tested and reviewed
    /// with. It grants nothing: whether a step gets the tools is the MCP hub's.
    ///
    /// # Errors
    ///
    /// When a connection is not the workspace's.
    pub fn set_integrations(
        &self,
        id: &str,
        connection_ids: &[String],
    ) -> Result<SpecView, AppError> {
        let spec = self.require(id)?;
        let chosen = self.known_connections(&spec.workspace_id, connection_ids)?;
        self.modify(id, |spec| {
            spec.integrations = chosen;
            spec.version += 1;
            spec.updated_at = now_ms();
            Ok(())
        })?;
        self.view(id)
    }

    /// Refuses a new run when what the spec's runs used reached its budget; the person can say to
    /// go on ([`Self::raise_budget`]).
    ///
    /// # Errors
    ///
    /// [`ErrorCode::SpecBudgetReached`], with what was used and the limit.
    pub fn check_budget(&self, id: &str) -> Result<(), AppError> {
        let view = self.view(id)?;
        match (view.budget.reached, view.budget.spent, view.budget.limit) {
            (true, Some(spent), Some(limit)) => Err(AppError::new(ErrorCode::SpecBudgetReached)
                .with("spent", spent.to_string())
                .with("limit", limit.to_string())),
            _ => Ok(()),
        }
    }

    /// The person says to go on after the budget was reached: it is granted once more.
    ///
    /// # Errors
    ///
    /// When there is no such spec or saving fails.
    pub fn raise_budget(&self, id: &str) -> Result<SpecView, AppError> {
        self.modify(id, |spec| {
            spec.budget_steps = spec.budget_steps.saturating_add(1);
            Ok(())
        })?;
        self.view(id)
    }

    // ---- changing --------------------------------------------------------------------------------

    /// Applies `change` to a copy of the spec, saves it, and only then keeps it: when the save
    /// fails the spec is as it was. Tells the screens afterwards.
    fn modify<T>(
        &self,
        id: &str,
        change: impl FnOnce(&mut Spec) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let (result, version) = {
            let mut specs = self.specs.lock_or_recover();
            let current = specs
                .get(id)
                .ok_or_else(|| AppError::new(ErrorCode::SpecNotFound))?;
            let mut copy = current.clone();
            let result = change(&mut copy)?;
            self.store.save(&copy).map_err(AppError::storage)?;
            let version = copy.version;
            specs.insert(id.to_owned(), copy);
            (result, version)
        };
        self.events.changed(id, version);
        Ok(result)
    }

    fn transition<T>(
        &self,
        id: &str,
        change: impl FnOnce(&mut Spec) -> Result<T, TransitionError>,
    ) -> Result<T, AppError> {
        self.modify(id, |spec| change(spec).map_err(app_error))
    }

    /// Reads the project again for the checks: the main specs a MODIFIED requirement is checked
    /// against. Done before the change, outside the lock.
    fn refresh_checks(&self, spec: &mut Spec, project: &ProjectSpecs) {
        let validator = self.framework.validator();
        let now = now_ms();
        let due: Vec<(String, ArtifactKind, u32, Vec<SpecFile>)> = spec
            .artifacts
            .iter()
            .filter_map(|a| {
                let latest = a.latest()?;
                let fresh = a
                    .validation
                    .as_ref()
                    .is_some_and(|v| v.revision == latest.number && v.validator == validator);
                (!fresh).then(|| (a.id.clone(), a.kind, latest.number, latest.files.clone()))
            })
            .collect();
        for (id, kind, revision, files) in due {
            let issues = self.framework.validate_artifact(kind, &files, project);
            let _ = rules::record_validation(
                spec,
                &id,
                ArtifactValidation {
                    revision,
                    validator: validator.to_owned(),
                    issues,
                    at: now,
                },
                now,
            );
        }
        if spec.artifacts.iter().all(|a| a.latest().is_some()) {
            let current = spec.current_revisions();
            let stale = spec
                .consistency
                .as_ref()
                .is_none_or(|c| c.revisions != current || c.validator != validator);
            if stale {
                let documents = latest_documents(spec);
                let issues = self.framework.validate_consistency(&documents);
                let _ = rules::record_consistency(
                    spec,
                    ConsistencyValidation {
                        revisions: current,
                        validator: validator.to_owned(),
                        issues,
                        at: now,
                    },
                    now,
                );
            }
        }
    }

    /// Runs the checks again on everything, whatever was there: when the person asks.
    ///
    /// # Errors
    ///
    /// When there is no such spec or the save fails.
    pub fn revalidate(&self, id: &str) -> Result<SpecView, AppError> {
        let spec = self.require(id)?;
        let project = self.project_specs(&spec);
        self.modify(id, |spec| {
            for artifact in &mut spec.artifacts {
                artifact.validation = None;
            }
            spec.consistency = None;
            self.refresh_checks(spec, &project);
            Ok(())
        })?;
        self.view(id)
    }

    /// At start-up: a run the last session never saw end is reported as interrupted, and checks
    /// made with other rules are made again. Nothing is resumed on its own.
    pub fn recover(&self) {
        let ids: Vec<String> = self.specs.lock_or_recover().keys().cloned().collect();
        for id in ids {
            let Some(spec) = self.get(&id) else { continue };
            let project = self.project_specs(&spec);
            let _ = self.modify(&id, |spec| {
                rules::recover_interrupted(spec, now_ms());
                self.refresh_checks(spec, &project);
                Ok(())
            });
        }
    }

    // ---- creating --------------------------------------------------------------------------------

    /// Any agent of the app can write or review a spec; it need not be placed in the workspace.
    fn usable_agent(&self, agent_id: &str) -> bool {
        self.agents.find(agent_id).is_some()
    }

    /// A change id from the title that no other spec of the project has and no folder of the
    /// project's `openspec/changes/` has.
    fn free_change_id(&self, workspace_id: &str, project_roots: &[String], title: &str) -> String {
        let base = self.framework.suggest_change_id(title);
        let taken: BTreeSet<String> = self
            .specs
            .lock_or_recover()
            .values()
            .filter(|s| s.workspace_id == workspace_id)
            .map(|s| s.change_id.clone())
            .collect();
        let root = self.framework.changes_root();
        let used = |id: &str| {
            taken.contains(id)
                || project_roots
                    .iter()
                    .any(|project_root| self.files.change_exists(project_root, root, id))
        };
        if !used(&base) {
            return base;
        }
        (2..100)
            .map(|n| format!("{base}-{n}"))
            .find(|candidate| !used(candidate))
            .unwrap_or_else(|| format!("{base}-{}", now_ms()))
    }

    /// Records the request. Nothing runs: the person starts it when they have looked.
    ///
    /// # Errors
    ///
    /// When the request, the workspace, the project or an agent is not acceptable.
    pub fn create(&self, request: NewSpecRequest) -> Result<SpecView, AppError> {
        let title = clip_chars(&request.title, MAX_TITLE + 1);
        if title.is_empty() {
            return Err(invalid("title_required"));
        }
        if title.chars().count() > MAX_TITLE {
            return Err(invalid("title_too_long"));
        }
        let text = request.request.trim();
        if text.is_empty() {
            return Err(invalid("request_required"));
        }
        if text.chars().count() > MAX_REQUEST {
            return Err(invalid("request_too_long"));
        }
        if request.context.trim().chars().count() > MAX_CONTEXT {
            return Err(invalid("context_too_long"));
        }
        let workspace = self
            .workspaces
            .get(&request.workspace_id)
            .ok_or_else(|| AppError::new(ErrorCode::WorkspaceNotFound))?;
        let project = match request.project_id.as_deref() {
            Some(id) => workspace.project(id),
            None => workspace.primary_project(),
        }
        .ok_or_else(|| invalid("project_invalid"))?
        .clone();
        let reviewer = request
            .reviewer_agent_id
            .clone()
            .unwrap_or_else(|| request.author_agent_id.clone());
        for agent in [&request.author_agent_id, &reviewer] {
            if !self.usable_agent(agent) {
                return Err(invalid("agent_invalid"));
            }
        }
        let integrations = self.known_connections(&workspace.id, &request.integrations)?;
        let roots: Vec<String> = workspace.projects.iter().map(|p| p.path.clone()).collect();
        let change_id = self.free_change_id(&workspace.id, &roots, &title);
        let now = now_ms();
        let mut spec = rules::create(
            NewSpec {
                id: new_id("spec"),
                title,
                request: text.to_owned(),
                context: request.context.trim().to_owned(),
                workspace_id: workspace.id.clone(),
                project_id: project.id,
                framework: self.framework.kind(),
                framework_schema: self.framework.schema().to_owned(),
                change_id,
                author_agent_id: request.author_agent_id,
                reviewer_agent_id: reviewer,
                origin: SpecOrigin::Created,
            },
            &self.framework.graph(),
            now,
        );
        spec.integrations = integrations;
        self.store.save(&spec).map_err(AppError::storage)?;
        let id = spec.id.clone();
        let version = spec.version;
        self.specs.lock_or_recover().insert(id.clone(), spec);
        self.events.changed(&id, version);
        self.view(&id)
    }

    /// Changes who writes and who reviews.
    ///
    /// # Errors
    ///
    /// When an agent does not exist, or a run is going.
    pub fn assign_agents(
        &self,
        id: &str,
        author: &str,
        reviewer: &str,
    ) -> Result<SpecView, AppError> {
        self.require(id)?;
        for agent in [author, reviewer] {
            if !self.usable_agent(agent) {
                return Err(invalid("agent_invalid"));
            }
        }
        self.modify(id, |spec| {
            if rules::has_active_run(spec) {
                return Err(AppError::new(ErrorCode::SpecRunInProgress));
            }
            author.clone_into(&mut spec.author_agent_id);
            reviewer.clone_into(&mut spec.reviewer_agent_id);
            spec.version += 1;
            spec.updated_at = now_ms();
            Ok(())
        })?;
        self.view(id)
    }

    /// Forgets the record (the project's files are not touched). Not while an agent works on it or
    /// an implementation was started from it.
    ///
    /// # Errors
    ///
    /// When the spec is in use.
    pub fn delete(&self, id: &str) -> Result<(), AppError> {
        let mut specs = self.specs.lock_or_recover();
        let spec = specs
            .get(id)
            .ok_or_else(|| AppError::new(ErrorCode::SpecNotFound))?;
        if rules::has_active_run(spec) || !spec.implementations.is_empty() {
            return Err(AppError::new(ErrorCode::SpecInUse));
        }
        self.store.remove(id).map_err(AppError::storage)?;
        specs.remove(id);
        drop(specs);
        self.events.changed(id, 0);
        Ok(())
    }

    /// The workspace is going away: so are its specs' records.
    pub fn discard_workspace(&self, workspace_id: &str) {
        let ids: Vec<String> = self
            .specs
            .lock_or_recover()
            .values()
            .filter(|s| s.workspace_id == workspace_id)
            .map(|s| s.id.clone())
            .collect();
        for id in ids {
            if self.store.remove(&id).is_ok() {
                self.specs.lock_or_recover().remove(&id);
                self.events.changed(&id, 0);
            }
        }
    }

    // ---- runs --------------------------------------------------------------------------------------

    /// Claims the right to start a run for the spec. A second claim fails until the first is
    /// dropped, so two clicks never start two agents.
    ///
    /// # Errors
    ///
    /// When another run is being started.
    pub fn launch_guard(&self, id: &str) -> Result<LaunchGuard, AppError> {
        if !self.launching.lock_or_recover().insert(id.to_owned()) {
            return Err(AppError::new(ErrorCode::SpecRunInProgress));
        }
        Ok(LaunchGuard {
            launching: self.launching.clone(),
            spec_id: id.to_owned(),
        })
    }

    /// The person pressed start.
    ///
    /// # Errors
    ///
    /// When there is no such spec.
    pub fn start(&self, id: &str) -> Result<(), AppError> {
        self.transition(id, |spec| rules::start(spec, now_ms()))
    }

    /// Checks that the artifact can be written now and says how, without changing anything.
    ///
    /// # Errors
    ///
    /// Why it cannot.
    pub fn plan_generation(&self, id: &str, artifact_id: &str) -> Result<GenerationMode, AppError> {
        let spec = self.require(id)?;
        rules::generation_mode(&spec, artifact_id).map_err(app_error)
    }

    /// Marks the artifact as being written by the run `run_id`.
    ///
    /// # Errors
    ///
    /// Why it cannot.
    pub fn begin_generation(
        &self,
        id: &str,
        artifact_id: &str,
        run_id: &str,
        agent_id: &str,
    ) -> Result<GenerationMode, AppError> {
        self.transition(id, |spec| {
            rules::begin_generation(spec, artifact_id, run_id, agent_id, now_ms())
        })
    }

    /// What the run produced. A result of a run that is no longer the artifact's is dropped and
    /// reported as late.
    ///
    /// # Errors
    ///
    /// [`TransitionError::Late`] as an error, when the result is not used.
    pub fn finish_generation(
        &self,
        id: &str,
        artifact_id: &str,
        run_id: &str,
        end: GenerationEnd,
    ) -> Result<(), AppError> {
        let spec = self.require(id)?;
        let project = self.project_specs(&spec);
        self.modify(id, |spec| {
            let now = now_ms();
            match end {
                GenerationEnd::Document(files) => {
                    rules::complete_generation(spec, artifact_id, run_id, files, now)
                        .map_err(app_error)?;
                    self.refresh_checks(spec, &project);
                }
                GenerationEnd::Question(asked) => {
                    rules::wait_for_input(spec, artifact_id, run_id, asked, now)
                        .map_err(app_error)?;
                }
                GenerationEnd::Failed { message, outcome } => {
                    rules::fail_generation(spec, artifact_id, run_id, &message, outcome, now)
                        .map_err(app_error)?;
                }
            }
            Ok(())
        })
    }

    /// Marks the review as being made by the run `run_id`.
    ///
    /// # Errors
    ///
    /// Why it cannot.
    pub fn begin_review(&self, id: &str, run_id: &str, agent_id: &str) -> Result<(), AppError> {
        self.transition(id, |spec| {
            rules::begin_review(spec, &new_id("review"), run_id, agent_id, now_ms())
        })
    }

    /// Checks that a review could start now, without changing anything.
    ///
    /// # Errors
    ///
    /// Why it cannot.
    pub fn plan_review(&self, id: &str) -> Result<(), AppError> {
        let mut copy = self.require(id)?;
        let agent = copy.reviewer_agent_id.clone();
        rules::begin_review(&mut copy, "check", "check", &agent, now_ms()).map_err(app_error)
    }

    /// What the review run produced.
    ///
    /// # Errors
    ///
    /// When the result is not used (late) or the save fails.
    pub fn finish_review(&self, id: &str, run_id: &str, end: ReviewEnd) -> Result<(), AppError> {
        self.transition(id, |spec| {
            let now = now_ms();
            match end {
                ReviewEnd::Report { summary, findings } => {
                    rules::complete_review(spec, run_id, findings, &summary, now)
                }
                ReviewEnd::Question(asked) => {
                    rules::review_wait_for_input(spec, run_id, asked, now)
                }
                ReviewEnd::Failed { message, outcome } => {
                    rules::fail_review(spec, run_id, &message, outcome, now)
                }
            }
        })
    }

    // ---- the person's decisions -----------------------------------------------------------------------

    /// Approve, ask for changes, or have the document written again.
    ///
    /// # Errors
    ///
    /// Why it cannot: another revision, no note, errors in the checks…
    pub fn decide(
        &self,
        id: &str,
        artifact_id: &str,
        expected_revision: u32,
        kind: DecisionKind,
        note: Option<&str>,
    ) -> Result<SpecView, AppError> {
        if note.is_some_and(|n| n.trim().chars().count() > MAX_NOTE) {
            return Err(invalid("note_too_long"));
        }
        self.transition(id, |spec| {
            rules::decide(spec, artifact_id, expected_revision, kind, note, now_ms())
        })?;
        self.view(id)
    }

    /// The person looked at an outdated document and it still holds.
    ///
    /// # Errors
    ///
    /// Why it cannot.
    pub fn keep_outdated(
        &self,
        id: &str,
        artifact_id: &str,
        expected_revision: u32,
    ) -> Result<SpecView, AppError> {
        self.transition(id, |spec| {
            rules::keep_outdated(spec, artifact_id, expected_revision, now_ms())
        })?;
        self.view(id)
    }

    /// The person's answer to a question an agent asked. Returns where it came from, so the
    /// caller can go on with that run.
    ///
    /// # Errors
    ///
    /// When the answer is empty, too long, not one of the options, or the question is not open.
    pub fn answer_question(
        &self,
        id: &str,
        question_id: &str,
        choice: Option<&str>,
        answer: Option<&str>,
    ) -> Result<(QuestionSource, Option<String>), AppError> {
        let spec = self.require(id)?;
        let question = spec
            .question(question_id)
            .ok_or_else(|| AppError::new(ErrorCode::SpecQuestionNotFound))?;
        let answer = answer.map(str::trim).filter(|a| !a.is_empty());
        let choice = choice.map(str::trim).filter(|c| !c.is_empty());
        if answer.is_none() && choice.is_none() {
            return Err(invalid("answer_required"));
        }
        if answer.is_some_and(|a| a.chars().count() > MAX_ANSWER) {
            return Err(invalid("answer_too_long"));
        }
        if let Some(choice) = choice {
            let allowed = question.options.is_empty()
                || question
                    .options
                    .iter()
                    .any(|o| o.id == choice || o.label == choice);
            if !allowed {
                return Err(invalid("answer_invalid"));
            }
        }
        let (source, artifact) = (question.source, question.artifact_id.clone());
        self.transition(id, |spec| {
            rules::answer_question(spec, question_id, choice, answer, now_ms())
        })?;
        Ok((source, artifact))
    }

    /// The person's decision on a finding of the review.
    ///
    /// # Errors
    ///
    /// When accepting a risk without saying why, or the finding is not there.
    pub fn resolve_finding(
        &self,
        id: &str,
        finding_id: &str,
        status: FindingStatus,
        note: Option<&str>,
    ) -> Result<SpecView, AppError> {
        if note.is_some_and(|n| n.trim().chars().count() > MAX_NOTE) {
            return Err(invalid("note_too_long"));
        }
        self.transition(id, |spec| {
            rules::resolve_finding(spec, finding_id, status, note, now_ms())
        })?;
        self.view(id)
    }

    /// The person answers what several findings asked, all at once. Each document the findings
    /// are about goes back to its agent with every question and answer that concerns it, and the
    /// findings are closed. Returns the first artifact (in the order of the documents) that needs
    /// writing again, if any: the others wait, asked for changes, for their turn.
    ///
    /// # Errors
    ///
    /// When there is nothing to answer, an answer is empty or too long, a finding is not there,
    /// or a document cannot be sent back now. Nothing is changed then.
    pub fn answer_findings(
        &self,
        id: &str,
        answers: &[(String, String)],
    ) -> Result<Option<String>, AppError> {
        if answers.is_empty() {
            return Err(invalid("answer_required"));
        }
        for (_, answer) in answers {
            if answer.trim().is_empty() {
                return Err(invalid("answer_required"));
            }
            if answer.trim().chars().count() > MAX_ANSWER {
                return Err(invalid("answer_too_long"));
            }
        }
        self.modify(id, |spec| {
            let now = now_ms();
            // What each document is told, in the order the documents come.
            let mut notes: Vec<(String, Vec<String>)> = Vec::new();
            let mut closing: Vec<(String, String)> = Vec::new();
            for (finding_id, answer) in answers {
                let answer = answer.trim();
                let finding = spec
                    .findings
                    .iter()
                    .find(|f| f.id == *finding_id)
                    .cloned()
                    .ok_or_else(|| AppError::new(ErrorCode::SpecFindingNotFound))?;
                if let Some(artifact_id) = finding.artifact_id.as_deref() {
                    let line = format!(
                        "A review of the specification raised: {}\nIt asked: {}\nThe person answered: {answer}",
                        finding.title,
                        finding.question.as_deref().unwrap_or(&finding.description),
                    );
                    match notes.iter_mut().find(|(a, _)| a == artifact_id) {
                        Some((_, lines)) => lines.push(line),
                        None => notes.push((artifact_id.to_owned(), vec![line])),
                    }
                }
                closing.push((finding_id.clone(), answer.to_owned()));
            }
            notes.sort_by_key(|(artifact_id, _)| {
                spec.artifacts
                    .iter()
                    .position(|a| a.id == *artifact_id)
                    .unwrap_or(usize::MAX)
            });
            let mut rewrite = None;
            for (artifact_id, lines) in &notes {
                let revision = spec
                    .artifact(artifact_id)
                    .and_then(crate::domain::spec::SpecArtifact::latest_number);
                let Some(revision) = revision else { continue };
                let note = format!(
                    "{}\nChange the document so that it reflects {}.",
                    lines.join("\n\n"),
                    if lines.len() == 1 { "this answer" } else { "these answers" },
                );
                rules::decide(spec, artifact_id, revision, DecisionKind::Revise, Some(&note), now)
                    .map_err(app_error)?;
                rewrite.get_or_insert_with(|| artifact_id.clone());
            }
            for (finding_id, answer) in &closing {
                rules::resolve_finding(spec, finding_id, FindingStatus::Resolved, Some(answer), now)
                    .map_err(app_error)?;
            }
            Ok(rewrite)
        })
    }

    /// The person goes without the reviewing agent, in writing.
    ///
    /// # Errors
    ///
    /// When no note was given or the documents are not all approved.
    pub fn waive_review(&self, id: &str, note: &str) -> Result<SpecView, AppError> {
        if note.trim().chars().count() > MAX_NOTE {
            return Err(invalid("note_too_long"));
        }
        self.transition(id, |spec| rules::waive_review(spec, Some(note), now_ms()))?;
        self.view(id)
    }

    /// The person's last word. Records the warnings that were showing.
    ///
    /// # Errors
    ///
    /// When something still keeps the spec from being ready.
    pub fn approve_final(&self, id: &str) -> Result<SpecView, AppError> {
        self.transition(id, |spec| {
            let warnings = current_warnings(spec);
            rules::final_approve(spec, warnings, now_ms())
        })?;
        self.view(id)
    }

    // ---- publishing --------------------------------------------------------------------------------

    /// Writes the spec's files to `<project>/openspec/changes/<change id>/` of every project of
    /// the workspace. Never overwrites a
    /// change that is not this spec's; picks up where a failed attempt stopped; confirms what it
    /// wrote by reading it back before it says so.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::SpecChangeExists`] when the folder belongs to something else (the caller can
    /// offer another id), or [`ErrorCode::SpecPublishFailed`] when writing stopped.
    pub fn publish(&self, id: &str, change_id: Option<&str>) -> Result<SpecView, AppError> {
        let spec = self.require(id)?;
        let roots = self.project_roots(&spec);
        if roots.is_empty() {
            return Err(invalid("project_invalid"));
        }
        let target = change_id.unwrap_or(&spec.change_id).trim().to_owned();
        self.framework
            .check_change_id(&target)
            .map_err(change_id_error)?;
        let changes_root = self.framework.changes_root();
        // The folder is this spec's own only if an earlier publication of it named it.
        let own = match &spec.publication {
            Publication::InProgress { change_id, .. }
            | Publication::Failed { change_id, .. }
            | Publication::Published { change_id, .. } => *change_id == target,
            Publication::NotPublished => false,
        };
        if !own
            && roots
                .iter()
                .any(|root| self.files.change_exists(root, changes_root, &target))
        {
            return Err(AppError::new(ErrorCode::SpecChangeExists).with("change", target));
        }
        self.transition(id, |spec| {
            rules::begin_publication(spec, &target, now_ms())?;
            target.clone_into(&mut spec.change_id);
            Ok(())
        })?;
        let documents = latest_documents(&self.require(id)?);
        let planned = self
            .framework
            .publish_files(&documents, now_ms())
            .map_err(|_| state("document_missing"))?;
        let mut written = Vec::new();
        for file in &planned {
            let relative = format!("{changes_root}/{target}/{}", file.path);
            for root in &roots {
                let outcome = match self.files.read(root, &relative) {
                    Some(existing) if existing == file.content => Ok(()),
                    Some(_) if own => self.files.replace(root, &relative, &file.content),
                    Some(_) => Err(FilesError::Exists),
                    None => self.files.create(root, &relative, &file.content),
                };
                if let Err(error) = outcome {
                    return Err(self.publication_failed(id, &relative, &error, &target));
                }
            }
            self.transition(id, |spec| {
                rules::publication_progress(spec, &file.path, now_ms());
                Ok(())
            })?;
            written.push(file.path.clone());
        }
        // Said done only when it can be read back as it was written.
        for file in &planned {
            let relative = format!("{changes_root}/{target}/{}", file.path);
            for root in &roots {
                if self.files.read(root, &relative).as_deref() != Some(file.content.as_str()) {
                    return Err(self.publication_failed(
                        id,
                        &relative,
                        &FilesError::Io("the file read back differs".to_owned()),
                        &target,
                    ));
                }
            }
        }
        self.transition(id, |spec| {
            rules::complete_publication(
                spec,
                &format!("{changes_root}/{target}"),
                written.clone(),
                now_ms(),
            );
            Ok(())
        })?;
        self.view(id)
    }

    fn publication_failed(
        &self,
        id: &str,
        relative: &str,
        error: &FilesError,
        target: &str,
    ) -> AppError {
        let message = match error {
            FilesError::Exists => format!("{relative} already exists and is not this spec's"),
            FilesError::Unsafe(path) => format!("{path} is not a path Atlas may write to"),
            FilesError::NoProject => "the project folder is not there".to_owned(),
            FilesError::Io(detail) => format!("{relative}: {detail}"),
        };
        let _ = self.transition(id, |spec| {
            rules::fail_publication(spec, &message, now_ms());
            Ok(())
        });
        match error {
            FilesError::Exists => AppError::new(ErrorCode::SpecChangeExists).with("change", target),
            _ => AppError::new(ErrorCode::SpecPublishFailed).with_detail(message),
        }
    }

    /// Keeps what the `OpenSpec` command said of the published change.
    ///
    /// # Errors
    ///
    /// When there is no such spec or the save fails.
    pub fn record_cli_check(
        &self,
        id: &str,
        check: crate::domain::spec::CliCheck,
    ) -> Result<(), AppError> {
        self.transition(id, |spec| {
            rules::record_cli_check(spec, check, now_ms());
            Ok(())
        })
    }

    /// Closes Atlas's own record of a published spec. The project's files are not moved: that is
    /// the `OpenSpec` command's job (`archive`), done separately when it is installed.
    ///
    /// # Errors
    ///
    /// When the spec was never published.
    pub fn archive_record(
        &self,
        id: &str,
        path: Option<String>,
        method: &str,
    ) -> Result<SpecView, AppError> {
        self.transition(id, |spec| rules::archive(spec, path, method, now_ms()))?;
        self.view(id)
    }

    // ---- importing -----------------------------------------------------------------------------------

    fn workspace_project_root(
        &self,
        workspace_id: &str,
        project_id: &str,
    ) -> Result<String, AppError> {
        let workspace = self
            .workspaces
            .get(workspace_id)
            .ok_or_else(|| AppError::new(ErrorCode::WorkspaceNotFound))?;
        workspace
            .project(project_id)
            .map(|p| p.path.clone())
            .ok_or_else(|| invalid("project_invalid"))
    }

    fn problem_codes(problems: &[ImportProblem]) -> Vec<String> {
        problems
            .iter()
            .map(|p| match p {
                ImportProblem::Missing(kind) => format!("missing:{}", kind.id()),
                ImportProblem::Unrecognized(path) => format!("unrecognized:{path}"),
                ImportProblem::SchemaUnsupported(schema) => format!("schema:{schema}"),
                ImportProblem::TooLarge(path) => format!("too_large:{path}"),
            })
            .collect()
    }

    /// The changes in the project's `openspec/changes/`, read without changing anything.
    ///
    /// # Errors
    ///
    /// When the workspace or project is not known.
    pub fn importable(
        &self,
        workspace_id: &str,
        project_id: &str,
    ) -> Result<Vec<ImportCandidate>, AppError> {
        let root = self.workspace_project_root(workspace_id, project_id)?;
        let changes_root = self.framework.changes_root();
        let known: BTreeMap<String, String> = self
            .specs
            .lock_or_recover()
            .values()
            .filter(|s| s.workspace_id == workspace_id && s.project_id == project_id)
            .map(|s| (s.change_id.clone(), s.id.clone()))
            .collect();
        let mut out = Vec::new();
        for change in self.files.change_ids(&root, changes_root) {
            let folder = self
                .files
                .read_change(&root, changes_root, &change)
                .unwrap_or_default();
            let imported = self.framework.import(&folder);
            out.push(ImportCandidate {
                imported_as: known.get(&change).cloned(),
                artifacts: imported
                    .artifacts
                    .keys()
                    .map(|k| k.id().to_owned())
                    .collect(),
                problems: Self::problem_codes(&imported.problems),
                change_id: change,
            });
        }
        Ok(out)
    }

    /// Turns a change that is already in the project into a spec. Reads the folder and changes
    /// nothing in it; nothing is approved on its behalf.
    ///
    /// # Errors
    ///
    /// When the change cannot be read, is already a spec of the workspace, or an agent does not
    /// exist.
    pub fn import(
        &self,
        workspace_id: &str,
        project_id: &str,
        change_id: &str,
        author_agent_id: &str,
        reviewer_agent_id: Option<&str>,
    ) -> Result<SpecView, AppError> {
        let root = self.workspace_project_root(workspace_id, project_id)?;
        let reviewer = reviewer_agent_id.unwrap_or(author_agent_id);
        for agent in [author_agent_id, reviewer] {
            if !self.usable_agent(agent) {
                return Err(invalid("agent_invalid"));
            }
        }
        self.framework
            .check_change_id(change_id)
            .map_err(change_id_error)?;
        let already = self.specs.lock_or_recover().values().any(|s| {
            s.workspace_id == workspace_id
                && s.project_id == project_id
                && s.change_id == change_id
                && s.archived.is_none()
        });
        if already {
            return Err(AppError::new(ErrorCode::SpecAlreadyImported));
        }
        let changes_root = self.framework.changes_root();
        let folder: ChangeFolder = self
            .files
            .read_change(&root, changes_root, change_id)
            .map_err(|e| {
                AppError::new(ErrorCode::SpecImportFailed).with("reason", format!("{e:?}"))
            })?;
        let imported = self.framework.import(&folder);
        if imported.artifacts.is_empty() {
            return Err(
                AppError::new(ErrorCode::SpecImportFailed).with("reason", "nothing_to_read")
            );
        }
        let now = now_ms();
        let why = imported
            .artifacts
            .get(&ArtifactKind::Proposal)
            .and_then(|files| files.iter().find(|f| f.path == "proposal.md"))
            .map(|f| why_of(&f.content))
            .unwrap_or_default();
        let title = title_of(change_id);
        let mut spec = rules::create(
            NewSpec {
                id: new_id("spec"),
                title,
                request: if why.is_empty() {
                    format!("Imported from {changes_root}/{change_id}")
                } else {
                    why
                },
                context: String::new(),
                workspace_id: workspace_id.to_owned(),
                project_id: project_id.to_owned(),
                framework: FrameworkKind::Openspec,
                framework_schema: self.framework.schema().to_owned(),
                change_id: change_id.to_owned(),
                author_agent_id: author_agent_id.to_owned(),
                reviewer_agent_id: reviewer.to_owned(),
                origin: SpecOrigin::Imported {
                    path: format!("{changes_root}/{change_id}"),
                    at: now,
                },
            },
            &self.framework.graph(),
            now,
        );
        spec.started = true;
        for (kind, files) in imported.artifacts {
            if let Some(artifact) = spec.artifact_mut(kind.id()) {
                artifact.revisions.push(Revision {
                    number: 1,
                    files,
                    origin: RevisionOrigin::Imported,
                    run_id: None,
                    agent_id: None,
                    based_on: BTreeMap::new(),
                    created_at: now,
                });
                // Read, not approved: the person decides what to make of it.
                artifact.review = Review::AwaitingReview;
            }
        }
        let paths: Vec<String> = folder.files.iter().map(|f| f.path.clone()).collect();
        let current = spec.current_revisions();
        spec.publication = Publication::Published {
            change_id: change_id.to_owned(),
            root: format!("{changes_root}/{change_id}"),
            files: paths,
            revisions: current,
            at: now,
        };
        let project = ProjectSpecs {
            capabilities: self.files.main_specs(&root),
        };
        self.refresh_checks(&mut spec, &project);
        self.store.save(&spec).map_err(AppError::storage)?;
        let id = spec.id.clone();
        let version = spec.version;
        self.specs.lock_or_recover().insert(id.clone(), spec);
        self.events.changed(&id, version);
        self.view(&id)
    }

    // ---- implementing --------------------------------------------------------------------------------

    /// The specs of a workspace a workflow could be started from: ready, and not archived.
    pub fn ready_for_workspace(&self, workspace_id: &str) -> Vec<SpecSummary> {
        self.specs
            .lock_or_recover()
            .values()
            .filter(|s| {
                s.workspace_id == workspace_id && s.archived.is_none() && rules::readiness(s).ready
            })
            .map(view::summarize)
            .collect()
    }

    /// What an implementing run is given. Only a ready spec of the workspace has one.
    ///
    /// # Errors
    ///
    /// When the spec is not the workspace's or is not ready.
    pub fn implementation_context(
        &self,
        id: &str,
        workspace_id: &str,
    ) -> Result<ImplementationContext, AppError> {
        let spec = self.require(id)?;
        if spec.workspace_id != workspace_id {
            return Err(AppError::new(ErrorCode::SpecNotFound));
        }
        if spec.archived.is_some() {
            return Err(state("archived"));
        }
        if !rules::readiness(&spec).ready {
            return Err(AppError::new(ErrorCode::SpecNotReady));
        }
        let documents = latest_documents(&spec);
        let digest = self.framework.digest(&documents, DIGEST_CHARS);
        let tasks = documents
            .get("tasks")
            .map(|files| self.framework.tasks(files))
            .unwrap_or_default();
        let published_root = match &spec.publication {
            Publication::Published { root, .. } => Some(root.clone()),
            _ => None,
        };
        Ok(ImplementationContext {
            spec_id: spec.id.clone(),
            project_id: spec.project_id.clone(),
            sibling_project_ids: self.sibling_project_ids(&spec),
            title: spec.title.clone(),
            change_id: spec.change_id.clone(),
            task: format!(
                "Implement the approved specification «{}» (change {}), following its task list.",
                spec.title.trim(),
                spec.change_id
            ),
            digest,
            revisions: spec.current_revisions(),
            tasks,
            published_root,
            integrations: self.integration_notes(&spec),
        })
    }

    /// Records that a workflow run was started from the spec, with the revisions it was given.
    ///
    /// # Errors
    ///
    /// When the spec is not ready any more.
    pub fn link_implementation(&self, id: &str, link: ImplementationLink) -> Result<(), AppError> {
        self.transition(id, |spec| rules::link_implementation(spec, link, now_ms()))
    }
}

/// The latest document of every artifact that has one.
pub fn latest_documents(spec: &Spec) -> Documents {
    spec.artifacts
        .iter()
        .filter_map(|a| a.latest().map(|r| (a.id.clone(), r.files.clone())))
        .collect()
}

/// The warnings showing now, as `code` or `code@file`, for the record of the final approval.
fn current_warnings(spec: &Spec) -> Vec<String> {
    let mut out = Vec::new();
    let mut note = |code: &str, file: &Option<String>| {
        let item = file
            .as_ref()
            .map_or_else(|| code.to_owned(), |f| format!("{code}@{f}"));
        if !out.contains(&item) {
            out.push(item);
        }
    };
    for artifact in &spec.artifacts {
        if let Some(validation) = &artifact.validation {
            for issue in validation
                .issues
                .iter()
                .filter(|i| i.severity == IssueSeverity::Warning)
            {
                note(&issue.code, &issue.file);
            }
        }
    }
    if let Some(consistency) = &spec.consistency {
        for issue in consistency
            .issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Warning)
        {
            note(&issue.code, &issue.file);
        }
    }
    out
}

/// The `## Why` of a proposal, in one paragraph.
fn why_of(proposal: &str) -> String {
    let sections = crate::application::specs::openspec::markdown::sections(proposal, 2);
    crate::application::specs::openspec::markdown::find_section(&sections, "Why")
        .map(|s| s.full_text.split_whitespace().collect::<Vec<_>>().join(" "))
        .map(|t| clip_chars(&t, MAX_REQUEST))
        .unwrap_or_default()
}

/// A readable title from a change id: `add-dark-mode` → `Add dark mode`.
fn title_of(change_id: &str) -> String {
    let spaced = change_id.replace('-', " ");
    let mut chars = spaced.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().collect::<String>() + chars.as_str()
    })
}
