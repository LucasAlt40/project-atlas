//! Specifications (ADR 0035): thin adapters between the Specs screen and `application::specs`.
//! Nothing here takes a path, a program or an approval from the webview: a command names a spec
//! (and a document of it), and what is written, run or published is what Atlas stored and the
//! agents' turns report. Every command refuses while the module is off.

use serde::Serialize;
use tauri::{AppHandle, Runtime, State};

use super::events::TauriEventObserver;
use crate::application::errors::{AppError, ErrorCode};
use crate::application::specs::runner::PreparedSpecRun;
use crate::application::specs::service::{NewSpecRequest, SpecEvents};
use crate::application::specs::view::{ImportCandidate, SpecSummary, SpecView};
use crate::domain::spec::{DecisionKind, FindingStatus, QuestionSource};
use crate::state::AppState;

/// A spec changed: its id and its new version. The screens read it again.
pub const SPEC_CHANGED_EVENT: &str = "spec:changed";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpecChanged<'a> {
    spec_id: &'a str,
    version: u64,
}

/// Adapter: tells the webview that a spec changed.
pub struct TauriSpecEvents<R: Runtime> {
    pub app: AppHandle<R>,
}

impl<R: Runtime> SpecEvents for TauriSpecEvents<R> {
    fn changed(&self, spec_id: &str, version: u64) {
        use tauri::Emitter;
        let payload = SpecChanged { spec_id, version };
        if let Err(error) = self.app.emit(SPEC_CHANGED_EVENT, payload) {
            log::warn!("failed to emit {SPEC_CHANGED_EVENT}: {error}");
        }
    }
}

/// What a decision or an answer did, and, when it was meant to start an agent that could not
/// start, why. The decision stands either way.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecAction {
    pub view: SpecView,
    pub launch_error: Option<AppError>,
}

/// Runs a prepared turn on a blocking thread: an agent takes minutes, and the window must not
/// wait for it. Its outcome is recorded in the spec, and the screens are told.
fn launch<R: Runtime>(app: &AppHandle<R>, run: PreparedSpecRun) {
    let observer = TauriEventObserver { app: app.clone() };
    tauri::async_runtime::spawn_blocking(move || run.run(&observer));
}

fn launch_artifact<R: Runtime>(
    app: &AppHandle<R>,
    state: &State<'_, AppState>,
    spec_id: &str,
    artifact_id: &str,
) -> Option<AppError> {
    match state.spec_runner.prepare_generation(spec_id, artifact_id) {
        Ok(run) => {
            launch(app, run);
            None
        }
        Err(error) => Some(error),
    }
}

fn launch_review<R: Runtime>(
    app: &AppHandle<R>,
    state: &State<'_, AppState>,
    spec_id: &str,
) -> Option<AppError> {
    match state.spec_runner.prepare_review(spec_id) {
        Ok(run) => {
            launch(app, run);
            None
        }
        Err(error) => Some(error),
    }
}

fn action(
    state: &State<'_, AppState>,
    spec_id: &str,
    launch_error: Option<AppError>,
) -> Result<SpecAction, AppError> {
    Ok(SpecAction {
        view: state.specs.view(spec_id)?,
        launch_error,
    })
}

#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::unnecessary_wraps)] // the commands' contract: the frontend reads every result as a `Result`
#[tauri::command]
pub fn list_specs(
    state: State<'_, AppState>,
    workspace_id: String,
) -> Result<Vec<SpecSummary>, AppError> {
    Ok(state.specs.list(&workspace_id))
}

/// The person chooses which of the workspace's integrations the spec is tested and reviewed with.
/// Choosing grants nothing.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn set_spec_integrations(
    state: State<'_, AppState>,
    spec_id: String,
    connection_ids: Vec<String>,
) -> Result<SpecView, AppError> {
    state.specs.set_integrations(&spec_id, &connection_ids)
}

/// The person says to go on after the spec's token budget was reached: it is granted once more.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn raise_spec_budget(
    state: State<'_, AppState>,
    spec_id: String,
) -> Result<SpecView, AppError> {
    state.specs.raise_budget(&spec_id)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn get_spec(state: State<'_, AppState>, spec_id: String) -> Result<SpecView, AppError> {
    state.specs.view(&spec_id)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn create_spec(
    state: State<'_, AppState>,
    request: NewSpecRequest,
) -> Result<SpecView, AppError> {
    state.specs.create(request)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn delete_spec(state: State<'_, AppState>, spec_id: String) -> Result<(), AppError> {
    state.specs.delete(&spec_id)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn assign_spec_agents(
    state: State<'_, AppState>,
    spec_id: String,
    author_agent_id: String,
    reviewer_agent_id: String,
) -> Result<SpecView, AppError> {
    state
        .specs
        .assign_agents(&spec_id, &author_agent_id, &reviewer_agent_id)
}

/// The person confirms the request: from here the first document is written.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn start_spec<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    spec_id: String,
) -> Result<SpecAction, AppError> {
    state.specs.start(&spec_id)?;
    let next = state
        .specs
        .view(&spec_id)?
        .ready_to_generate
        .first()
        .cloned();
    let error = next.and_then(|artifact| launch_artifact(&app, &state, &spec_id, &artifact));
    action(&state, &spec_id, error)
}

/// Asks for a document that can be written now (the first one, or one that failed or is
/// outdated).
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn generate_spec_artifact<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    spec_id: String,
    artifact_id: String,
) -> Result<SpecAction, AppError> {
    // The reasons it cannot be done are the command's answer, not a side note.
    let run = state
        .spec_runner
        .prepare_generation(&spec_id, &artifact_id)?;
    launch(&app, run);
    action(&state, &spec_id, None)
}

/// Approve (and go on to the next document), ask for changes (the agent revises), or have it
/// written again. `expected_revision` is the one the person was looking at.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn decide_spec_artifact<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    spec_id: String,
    artifact_id: String,
    expected_revision: u32,
    decision: String,
    note: Option<String>,
) -> Result<SpecAction, AppError> {
    let kind = DecisionKind::parse(&decision)
        .ok_or_else(|| AppError::new(ErrorCode::SpecInvalid).with("reason", "decision_invalid"))?;
    let view = state.specs.decide(
        &spec_id,
        &artifact_id,
        expected_revision,
        kind,
        note.as_deref(),
    )?;
    let next = match kind {
        DecisionKind::Approve => view.ready_to_generate.first().cloned(),
        DecisionKind::Revise | DecisionKind::Redo => Some(artifact_id),
    };
    let error = next.and_then(|artifact| launch_artifact(&app, &state, &spec_id, &artifact));
    action(&state, &spec_id, error)
}

/// The person looked at an outdated document and it still holds.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn keep_spec_artifact(
    state: State<'_, AppState>,
    spec_id: String,
    artifact_id: String,
    expected_revision: u32,
) -> Result<SpecView, AppError> {
    state
        .specs
        .keep_outdated(&spec_id, &artifact_id, expected_revision)
}

/// The person's answer to a question an agent asked; the agent goes on from it.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn answer_spec_question<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    spec_id: String,
    question_id: String,
    choice: Option<String>,
    answer: Option<String>,
) -> Result<SpecAction, AppError> {
    let (source, artifact) = state.specs.answer_question(
        &spec_id,
        &question_id,
        choice.as_deref(),
        answer.as_deref(),
    )?;
    let error = match (source, artifact) {
        (QuestionSource::Artifact, Some(artifact)) => {
            launch_artifact(&app, &state, &spec_id, &artifact)
        }
        (QuestionSource::Review, _) => launch_review(&app, &state, &spec_id),
        (QuestionSource::Artifact, None) => None,
    };
    action(&state, &spec_id, error)
}

/// One answer to what a finding of the review asked.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindingAnswer {
    pub finding_id: String,
    pub answer: String,
}

/// The person answers what several findings of the review asked, all together; the documents they
/// are about are revised with every answer that concerns them.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn answer_spec_findings<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    spec_id: String,
    answers: Vec<FindingAnswer>,
) -> Result<SpecAction, AppError> {
    let pairs: Vec<(String, String)> = answers
        .into_iter()
        .map(|a| (a.finding_id, a.answer))
        .collect();
    let rewrite = state.specs.answer_findings(&spec_id, &pairs)?;
    let error = rewrite.and_then(|artifact| launch_artifact(&app, &state, &spec_id, &artifact));
    action(&state, &spec_id, error)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn resolve_spec_finding(
    state: State<'_, AppState>,
    spec_id: String,
    finding_id: String,
    status: FindingStatus,
    note: Option<String>,
) -> Result<SpecView, AppError> {
    state
        .specs
        .resolve_finding(&spec_id, &finding_id, status, note.as_deref())
}

/// The reviewing agent reads the approved documents.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn run_spec_review<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    spec_id: String,
) -> Result<SpecAction, AppError> {
    let run = state.spec_runner.prepare_review(&spec_id)?;
    launch(&app, run);
    action(&state, &spec_id, None)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn waive_spec_review(
    state: State<'_, AppState>,
    spec_id: String,
    note: String,
) -> Result<SpecView, AppError> {
    state.specs.waive_review(&spec_id, &note)
}

/// The person's last word on the spec.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn approve_spec(state: State<'_, AppState>, spec_id: String) -> Result<SpecView, AppError> {
    state.specs.approve_final(&spec_id)
}

/// Runs the checks again on every document.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn validate_spec(state: State<'_, AppState>, spec_id: String) -> Result<SpecView, AppError> {
    state.specs.revalidate(&spec_id)
}

/// Writes the spec's files to the project. `change_id` is another name, when the first is taken.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn publish_spec(
    state: State<'_, AppState>,
    spec_id: String,
    change_id: Option<String>,
) -> Result<SpecView, AppError> {
    state.specs.publish(&spec_id, change_id.as_deref())
}

/// Closes Atlas's own record of a published spec.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn archive_spec(state: State<'_, AppState>, spec_id: String) -> Result<SpecView, AppError> {
    state.specs.archive_record(&spec_id, None, "atlas")
}

/// Asks the agent's process to end. The turn then reports itself as cancelled.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn cancel_spec_run(state: State<'_, AppState>, spec_id: String) -> Result<(), AppError> {
    state.spec_runner.cancel(&spec_id)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn list_importable_specs(
    state: State<'_, AppState>,
    workspace_id: String,
    project_id: String,
) -> Result<Vec<ImportCandidate>, AppError> {
    state.specs.importable(&workspace_id, &project_id)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn import_spec(
    state: State<'_, AppState>,
    workspace_id: String,
    project_id: String,
    change_id: String,
    author_agent_id: String,
    reviewer_agent_id: Option<String>,
) -> Result<SpecView, AppError> {
    state.specs.import(
        &workspace_id,
        &project_id,
        &change_id,
        &author_agent_id,
        reviewer_agent_id.as_deref(),
    )
}

/// The specs a workflow could be started from: ready, and not archived.
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::unnecessary_wraps)] // the commands' contract: the frontend reads every result as a `Result`
#[tauri::command]
pub fn list_ready_specs(
    state: State<'_, AppState>,
    workspace_id: String,
) -> Result<Vec<SpecSummary>, AppError> {
    Ok(state.specs.ready_for_workspace(&workspace_id))
}

/// How far each run started from the spec has got: what its steps reported, and what Atlas
/// confirms because the run ended well.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn get_spec_progress(
    state: State<'_, AppState>,
    spec_id: String,
) -> Result<Vec<crate::application::specs::progress::ImplementationProgress>, AppError> {
    state.spec_work.progress(&spec_id)
}

/// Whether the `OpenSpec` command is installed and a version Atlas reads, for the spec's project.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn get_openspec_cli_status(
    state: State<'_, AppState>,
    spec_id: String,
) -> Result<crate::application::specs::cli::CliStatus, AppError> {
    state.spec_cli.status(&spec_id)
}

/// The person asks the `OpenSpec` command to look at the published change (read-only).
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn check_spec_with_cli(
    state: State<'_, AppState>,
    spec_id: String,
) -> Result<SpecView, AppError> {
    state.spec_cli.check(&spec_id)
}

/// The person asks the `OpenSpec` command to archive the published change: it merges the deltas
/// into the project's specs and moves the folder. Moves files in the project, so only on this
/// explicit request.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command(async)]
pub fn archive_spec_with_cli(
    state: State<'_, AppState>,
    spec_id: String,
) -> Result<SpecView, AppError> {
    state.spec_cli.archive(&spec_id)
}
