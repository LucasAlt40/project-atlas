use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Runtime, State};

use super::events::TauriWorkflowObserver;
use crate::application::errors::{AppError, ErrorCode};
use crate::application::ide::Ide;
use crate::application::workflow::orchestrator::{AnswerDelivery, Control};
use crate::application::workflow::repair::{RepairChoice, RepairProposal};
use crate::application::workflow::service::NewWorkflow;
use crate::application::workflow::templates::{Role, TemplateInfo};
use crate::application::workflow::validation::ValidationReport;
use crate::domain::interaction::{InteractionAnswer, PendingInteraction};
use crate::domain::workflow::{
    RecoveryPlan, Workflow, WorkflowExecution, WorkflowExecutionStatus, WorkflowMode,
};
use crate::domain::worktree::ChangeSet;
use crate::state::AppState;

/// A workflow made from a template, and the roles no agent could be found for. Atlas never
/// creates an agent on its own: the user picks one for each of those nodes.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateWorkflow {
    pub workflow: Workflow,
    pub missing_roles: Vec<Role>,
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn list_workflows(state: State<'_, AppState>, workspace_id: String) -> Vec<Workflow> {
    state.workflows.list(&workspace_id)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn get_workflow(state: State<'_, AppState>, workflow_id: String) -> Option<Workflow> {
    state.workflows.get(&workflow_id)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn list_workflow_templates(state: State<'_, AppState>) -> Vec<TemplateInfo> {
    state.workflows.templates()
}

/// The template Automatic mode proposes for a task. Deterministic; no model is asked.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn select_workflow_template(state: State<'_, AppState>, task: String) -> String {
    state.workflows.select_template(&task).to_owned()
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn create_workflow(
    state: State<'_, AppState>,
    request: NewWorkflow,
) -> Result<Workflow, AppError> {
    state.workflows.create(request)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn create_workflow_from_template(
    state: State<'_, AppState>,
    workspace_id: String,
    template_id: String,
    name: Option<String>,
    mode: WorkflowMode,
) -> Result<TemplateWorkflow, AppError> {
    let built =
        state
            .workflows
            .create_from_template(&workspace_id, &template_id, name.as_deref(), mode)?;
    Ok(TemplateWorkflow {
        workflow: built.workflow,
        missing_roles: built.missing_roles,
    })
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn update_workflow(
    state: State<'_, AppState>,
    workflow: Workflow,
) -> Result<Workflow, AppError> {
    state.workflows.update(workflow)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn delete_workflow(state: State<'_, AppState>, workflow_id: String) -> Result<(), AppError> {
    state.workflows.delete(&workflow_id)
}

/// Checks a definition (possibly unsaved) and lists everything that is wrong with it.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn validate_workflow(state: State<'_, AppState>, workflow: Workflow) -> ValidationReport {
    state.workflows.validate(&workflow)
}

/// What Atlas can propose for routes that cannot match their agent's contract. Changes nothing.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn suggest_route_repairs(
    state: State<'_, AppState>,
    workflow: Workflow,
) -> Vec<RepairProposal> {
    state.workflows.route_repairs(&workflow)
}

/// Applies the mapping the user confirmed, as a new version of the saved workflow.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn repair_workflow_routes(
    state: State<'_, AppState>,
    workflow_id: String,
    choices: Vec<RepairChoice>,
) -> Result<Workflow, AppError> {
    state.workflows.repair_routes(&workflow_id, &choices)
}

/// Starts a run of the workflow on a task. Returns at once with the run; its progress arrives
/// as `workflow:*` events (and the steps' own `execution:progress` events).
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn start_workflow<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    workflow_id: String,
    task: String,
) -> Result<WorkflowExecution, AppError> {
    let execution = state.workflows.start(&workflow_id, &task)?;
    drive(&app, &state, execution.id.clone(), Drive::Fresh);
    Ok(execution)
}

/// Stops starting new steps. A step already running is left to finish.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn pause_workflow<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    execution_id: String,
) -> Result<(), AppError> {
    state.orchestrator.control(
        &execution_id,
        Control::Pause,
        &TauriWorkflowObserver { app },
    )
}

/// Resumes a paused run, picks up one the app's shutdown interrupted, or picks a failed one up at
/// its Recovery Point (see `get_workflow_recovery`).
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn resume_workflow<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    execution_id: String,
) -> Result<(), AppError> {
    let status = state
        .workflows
        .execution(&execution_id)
        .ok_or_else(|| AppError::new(ErrorCode::WorkflowExecutionNotFound))?
        .status;
    if status == WorkflowExecutionStatus::Interrupted {
        drive(&app, &state, execution_id, Drive::Interrupted);
        return Ok(());
    }
    if status == WorkflowExecutionStatus::Failed {
        // Refused here, not on the thread: the user hears why a run cannot go on.
        if let Some(problem) = state
            .orchestrator
            .recovery_plan(&execution_id)
            .and_then(|plan| plan.problem)
        {
            let reason = serde_json::to_value(problem)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default();
            return Err(AppError::new(ErrorCode::WorkflowNotRecoverable).with("reason", reason));
        }
        drive(&app, &state, execution_id, Drive::Failed);
        return Ok(());
    }
    state.orchestrator.control(
        &execution_id,
        Control::Resume,
        &TauriWorkflowObserver { app },
    )
}

/// Where a failed run would go on from: the step it stopped at, the last that completed, the
/// steps that run next and the ones kept as they are. `None` for a run that did not fail. Looks
/// only; changes nothing.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn get_workflow_recovery(
    state: State<'_, AppState>,
    execution_id: String,
) -> Option<RecoveryPlan> {
    state.orchestrator.recovery_plan(&execution_id)
}

/// Cancels the whole run: what has not started is cancelled, running steps are stopped, and
/// what completed is kept.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn cancel_workflow<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    execution_id: String,
) -> Result<(), AppError> {
    state.orchestrator.control(
        &execution_id,
        Control::Cancel,
        &TauriWorkflowObserver { app },
    )
}

/// The person's answer to a question a step asked. The webview names the run, the question and
/// the answer; the answer is checked against the question and only handed back to the agent as
/// text. It never changes a permission, a policy or the workflow, and it applies nothing to the
/// project.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn answer_workflow_interaction<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    execution_id: String,
    interaction_id: String,
    answer: InteractionAnswer,
) -> Result<(), AppError> {
    let delivery = state.orchestrator.answer_interaction(
        &execution_id,
        &interaction_id,
        answer,
        &TauriWorkflowObserver { app: app.clone() },
    )?;
    if delivery == AnswerDelivery::Recorded {
        // The app was closed since the question was asked: the run is picked up from the answer.
        drive(&app, &state, execution_id, Drive::Interrupted);
    }
    Ok(())
}

/// The questions waiting for a person in a workspace, across its runs: what "Action required"
/// counts and links to.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn list_pending_interactions(
    state: State<'_, AppState>,
    workspace_id: String,
) -> Vec<PendingInteraction> {
    state
        .workflows
        .executions(&workspace_id, None)
        .into_iter()
        .filter(|run| run.status.is_active() || run.status == WorkflowExecutionStatus::Interrupted)
        .flat_map(|run| run.interactions)
        .filter(PendingInteraction::is_pending)
        .collect()
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn get_workflow_execution(
    state: State<'_, AppState>,
    execution_id: String,
) -> Option<WorkflowExecution> {
    state.workflows.execution(&execution_id)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn list_workflow_executions(
    state: State<'_, AppState>,
    workspace_id: String,
    workflow_id: Option<String>,
) -> Vec<WorkflowExecution> {
    state
        .workflows
        .executions(&workspace_id, workflow_id.as_deref())
}

#[derive(Clone, Copy)]
enum Drive {
    Fresh,
    Interrupted,
    Failed,
}

/// Hands the run to the orchestrator on a blocking thread: a step can take minutes, and the
/// window must not wait for it.
fn drive<R: Runtime>(
    app: &AppHandle<R>,
    state: &State<'_, AppState>,
    execution_id: String,
    how: Drive,
) {
    let orchestrator = state.orchestrator.clone();
    let observer = Arc::new(TauriWorkflowObserver { app: app.clone() });
    tauri::async_runtime::spawn_blocking(move || {
        let result = match how {
            Drive::Interrupted => orchestrator.resume_interrupted(&execution_id, observer),
            Drive::Failed => orchestrator.resume_failed(&execution_id, observer),
            Drive::Fresh => orchestrator.run(&execution_id, observer),
        };
        if let Err(error) = result {
            eprintln!("workflow run {execution_id} could not be driven: {error}");
        }
    });
}

// ---- the code of a run -------------------------------------------------------------------------
//
// A run can complete while its code is only in the run's isolated worktree. None of these commands
// takes a path, a program, a branch or an approval from the webview: they name a run, and
// everything else is what Atlas stored and Git reports.

/// What the run changed in the code, from Git (while its worktree exists; the summary kept with
/// the run afterwards).
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn get_workflow_changes(
    state: State<'_, AppState>,
    execution_id: String,
) -> Result<Option<ChangeSet>, AppError> {
    state.integration.changes(&execution_id)
}

/// The real diff of the run's code, or of one file of it.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn get_workflow_diff(
    state: State<'_, AppState>,
    execution_id: String,
    file: Option<String>,
) -> Result<String, AppError> {
    state.integration.diff(&execution_id, file.as_deref())
}

/// The user's decision to apply the run's changes to the project. Goes through the worktree
/// merge with every check it has; a merge that cannot happen is reported in the run.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn apply_workflow_changes<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    execution_id: String,
) -> Result<WorkflowExecution, AppError> {
    Ok(announce(&app, state.integration.apply(&execution_id)?))
}

/// The user's decision to leave the changes in the worktree.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn keep_workflow_changes<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    execution_id: String,
) -> Result<WorkflowExecution, AppError> {
    Ok(announce(&app, state.integration.keep(&execution_id)?))
}

/// The user's confirmed decision to discard the worktree (its branch is kept).
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn discard_workflow_changes<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    execution_id: String,
) -> Result<WorkflowExecution, AppError> {
    Ok(announce(&app, state.integration.discard(&execution_id)?))
}

/// The editors Atlas can open a worktree in, found on this machine.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn list_ides(state: State<'_, AppState>) -> Vec<Ide> {
    state.integration.ides()
}

/// Opens the run's code in an editor: its worktree while the code is only there.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn open_workflow_in_ide(
    state: State<'_, AppState>,
    execution_id: String,
    ide_id: String,
) -> Result<(), AppError> {
    state.integration.open_in_ide(&execution_id, &ide_id)
}

fn announce<R: Runtime>(app: &AppHandle<R>, execution: WorkflowExecution) -> WorkflowExecution {
    use crate::application::workflow::runner::WorkflowObserver;
    if let Some(event) = execution.events.last() {
        TauriWorkflowObserver { app: app.clone() }.on_workflow_event(event);
    }
    execution
}
