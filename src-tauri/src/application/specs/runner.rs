//! Runs the agents of a spec. An agent's turn on a document is an execution like any other: it
//! goes through `ChatService` (so through `ExecutionService`, the agent's policy, the project's
//! Task Context, the runtime registry and the guard), read-only, and its answer is read for a
//! question to the person. This module builds the prompt, starts the turn, and turns the answer
//! into what the service records: a document, a question, a review, or a failure. It starts no
//! process, reads no file and approves nothing.

use std::fmt::Write;
use std::sync::Arc;

use super::framework::{OutputProblem, SpecFramework};
use super::prompts::{self, StepPrompt};
use super::review::{parse_review, ReviewProblem};
use super::service::{GenerationEnd, LaunchGuard, ReviewEnd, SpecService};
use crate::application::chat::{
    ChatError, ChatObserver, ChatService, PendingRun, RunSummary, SpecStepRequest,
};
use crate::application::errors::{AppError, ErrorCode};
use crate::application::sessions::{SessionRegistry, SessionTarget};
use crate::application::support::{new_id, now_ms};
use crate::domain::execution::ExecutionStatus;
use crate::domain::interaction::{InteractionKind, MAX_OPTIONS};
use crate::domain::scope::ScopeRequest;
use crate::domain::spec::rules::AskedQuestion;
use crate::domain::spec::{
    ArtifactKind, Generation, GenerationMode, ReviewRun, RunOutcome, Spec, SpecFile,
};

/// What a prepared run is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunTarget {
    Artifact(String),
    Review,
}

pub struct SpecRunner {
    specs: Arc<SpecService>,
    chat: ChatService,
    sessions: Arc<SessionRegistry>,
}

/// A run that has been recorded and not started: [`Self::run`] does the agent's turn (it
/// blocks, so a command runs it on a blocking thread) and reports what happened.
pub struct PreparedSpecRun {
    specs: Arc<SpecService>,
    framework: Arc<dyn SpecFramework>,
    spec_id: String,
    target: RunTarget,
    execution_id: String,
    pending: PendingRun,
    _guard: LaunchGuard,
}

fn chat_error(error: &ChatError) -> AppError {
    AppError::from(error)
}

fn problem_text(problem: &OutputProblem) -> String {
    match problem {
        OutputProblem::NoDocument => "output_no_document".to_owned(),
        OutputProblem::Unterminated(path) => format!("output_cut_off|{path}"),
        OutputProblem::UnexpectedPath(path) => format!("output_unexpected_file|{path}"),
        OutputProblem::UnsafePath(path) => format!("output_unsafe_path|{path}"),
        OutputProblem::DuplicatePath(path) => format!("output_duplicate_file|{path}"),
        OutputProblem::TooManyFiles => "output_too_many_files".to_owned(),
        OutputProblem::TooLarge(path) => format!("output_too_large|{path}"),
        OutputProblem::EmptyDocument(path) => format!("output_empty_document|{path}"),
        OutputProblem::MissingDocument(path) => format!("output_missing_document|{path}"),
        OutputProblem::Edit(problem) => {
            let (code, detail) = problem.code();
            if detail.is_empty() {
                code.to_owned()
            } else {
                format!("{code}|{detail}")
            }
        }
    }
}

fn failed(message: impl Into<String>, outcome: RunOutcome) -> (String, RunOutcome) {
    (message.into(), outcome)
}

/// Why a run ended without an answer, as the end it will be recorded as.
fn without_answer(summary: &RunSummary) -> (String, RunOutcome) {
    match summary.status {
        ExecutionStatus::Cancelled => failed("cancelled", RunOutcome::Cancelled),
        ExecutionStatus::WaitingForInput => failed("no_question", RunOutcome::Failed),
        _ => failed(
            format!(
                "run_failed|{}",
                summary
                    .failure
                    .clone()
                    .unwrap_or_else(|| "the run failed".to_owned())
                    .chars()
                    .take(400)
                    .collect::<String>()
            ),
            RunOutcome::Failed,
        ),
    }
}

/// The question an agent stopped to ask. Only a clarification has options Atlas shows; any other
/// kind is shown as the free-text question it is.
fn question_of(summary: &RunSummary) -> Option<AskedQuestion> {
    let detection = summary.interaction.as_ref()?;
    if detection.question.trim().is_empty() {
        return None;
    }
    let options = if detection.kind == Some(InteractionKind::Clarification) {
        detection
            .options
            .iter()
            .take(MAX_OPTIONS)
            .cloned()
            .collect()
    } else {
        Vec::new()
    };
    Some(AskedQuestion {
        id: new_id("question"),
        question: detection.question.clone(),
        context: detection.context.clone(),
        options,
    })
}

/// What a finished turn on a document means.
pub fn interpret_generation(
    framework: &dyn SpecFramework,
    kind: ArtifactKind,
    summary: &RunSummary,
    current: &[SpecFile],
) -> GenerationEnd {
    match summary.status {
        ExecutionStatus::Completed => {
            let text = summary.result.as_deref().unwrap_or_default();
            match framework.parse_revision(kind, text, current) {
                Ok(files) => GenerationEnd::Document(files),
                Err(problem) => GenerationEnd::Failed {
                    message: problem_text(&problem),
                    outcome: RunOutcome::Failed,
                },
            }
        }
        ExecutionStatus::WaitingForInput => question_of(summary).map_or_else(
            || {
                let (message, outcome) = without_answer(summary);
                GenerationEnd::Failed { message, outcome }
            },
            GenerationEnd::Question,
        ),
        _ => {
            let (message, outcome) = without_answer(summary);
            GenerationEnd::Failed { message, outcome }
        }
    }
}

/// What a finished review turn means.
pub fn interpret_review(spec: &Spec, summary: &RunSummary, review_id: &str) -> ReviewEnd {
    match summary.status {
        ExecutionStatus::Completed => {
            let text = summary.result.as_deref().unwrap_or_default();
            match parse_review(text, review_id, &spec.current_revisions(), now_ms()) {
                Ok(parsed) => {
                    let mut text = parsed.summary;
                    if parsed.dropped > 0 {
                        let _ = write!(
                            text,
                            " ({} finding(s) left out: they had no title or description.)",
                            parsed.dropped
                        );
                    }
                    ReviewEnd::Report {
                        summary: text,
                        findings: parsed.findings,
                    }
                }
                Err(ReviewProblem::NoBlock) => ReviewEnd::Failed {
                    message: "review_no_report".to_owned(),
                    outcome: RunOutcome::Failed,
                },
                Err(ReviewProblem::Malformed) => ReviewEnd::Failed {
                    message: "review_malformed".to_owned(),
                    outcome: RunOutcome::Failed,
                },
            }
        }
        ExecutionStatus::WaitingForInput => question_of(summary).map_or_else(
            || {
                let (message, outcome) = without_answer(summary);
                ReviewEnd::Failed { message, outcome }
            },
            ReviewEnd::Question,
        ),
        _ => {
            let (message, outcome) = without_answer(summary);
            ReviewEnd::Failed { message, outcome }
        }
    }
}

impl SpecRunner {
    pub fn new(specs: Arc<SpecService>, chat: ChatService, sessions: Arc<SessionRegistry>) -> Self {
        Self {
            specs,
            chat,
            sessions,
        }
    }

    fn step(
        &self,
        spec: &Spec,
        agent_id: &str,
        prompt: StepPrompt,
    ) -> Result<(String, PendingRun), AppError> {
        let (sent, pending) = self
            .chat
            .send_spec_step(SpecStepRequest {
                workspace_id: spec.workspace_id.clone(),
                agent_id: agent_id.to_owned(),
                display_task: prompt.display_task,
                instruction: prompt.instruction,
                context_query: prompt.context_query,
                // The agent works in the spec's project and may read the workspace's others.
                scope: Some(ScopeRequest {
                    working_project_id: Some(spec.project_id.clone()),
                    read_project_ids: self.specs.sibling_project_ids(spec),
                }),
            })
            .map_err(|e| chat_error(&e))?;
        Ok((sent.execution_id, pending))
    }

    /// Records that the artifact is being written and readies the turn. Nothing runs yet.
    ///
    /// # Errors
    ///
    /// When the artifact cannot be written now, the agent is gone or busy, or another run is
    /// being started for the spec.
    pub fn prepare_generation(
        &self,
        spec_id: &str,
        artifact_id: &str,
    ) -> Result<PreparedSpecRun, AppError> {
        let guard = self.specs.launch_guard(spec_id)?;
        let spec = self
            .specs
            .get(spec_id)
            .ok_or_else(|| AppError::new(ErrorCode::SpecNotFound))?;
        self.specs.check_budget(spec_id)?;
        let mode = self.specs.plan_generation(spec_id, artifact_id)?;
        let agent = spec.author_agent_id.clone();
        if !self.specs.view_of(&spec).author_available {
            return Err(AppError::new(ErrorCode::SpecInvalid).with("reason", "agent_invalid"));
        }
        let framework = self.specs.framework().clone();
        let project = self.specs.project_specs(&spec);
        let prompt = prompts::generation_prompt(
            &spec,
            artifact_id,
            mode,
            framework.as_ref(),
            &project,
            &self.specs.integration_notes(&spec),
        )
        .ok_or_else(|| AppError::new(ErrorCode::SpecArtifactNotFound))?;
        let mut prompt = prompt;
        prompt
            .instruction
            .push_str(&prompts::workspace_projects_note(
                &self.specs.workspace_project_names(&spec),
            ));
        let (execution_id, pending) = self.step(&spec, &agent, prompt)?;
        // If the spec changed under us, the turn is dropped before it ever starts.
        self.specs
            .begin_generation(spec_id, artifact_id, &execution_id, &agent)?;
        Ok(PreparedSpecRun {
            specs: self.specs.clone(),
            framework,
            spec_id: spec_id.to_owned(),
            target: RunTarget::Artifact(artifact_id.to_owned()),
            execution_id,
            pending,
            _guard: guard,
        })
    }

    /// As [`Self::prepare_generation`], for the review of the approved documents.
    ///
    /// # Errors
    ///
    /// When the documents are not all approved, the agent is gone or busy, or a run is going.
    pub fn prepare_review(&self, spec_id: &str) -> Result<PreparedSpecRun, AppError> {
        let guard = self.specs.launch_guard(spec_id)?;
        let spec = self
            .specs
            .get(spec_id)
            .ok_or_else(|| AppError::new(ErrorCode::SpecNotFound))?;
        self.specs.check_budget(spec_id)?;
        self.specs.plan_review(spec_id)?;
        let agent = spec.reviewer_agent_id.clone();
        if !self.specs.view_of(&spec).reviewer_available {
            return Err(AppError::new(ErrorCode::SpecInvalid).with("reason", "agent_invalid"));
        }
        let framework = self.specs.framework().clone();
        let prompt = prompts::review_prompt(
            &spec,
            framework.as_ref(),
            &self.specs.integration_notes(&spec),
        );
        let (execution_id, pending) = self.step(&spec, &agent, prompt)?;
        self.specs.begin_review(spec_id, &execution_id, &agent)?;
        Ok(PreparedSpecRun {
            specs: self.specs.clone(),
            framework,
            spec_id: spec_id.to_owned(),
            target: RunTarget::Review,
            execution_id,
            pending,
            _guard: guard,
        })
    }

    /// Asks the agent's process to end, the same control the person has in the terminal. The run
    /// then reports itself as cancelled.
    ///
    /// # Errors
    ///
    /// When nothing is running for the spec, or the process cannot be reached.
    pub fn cancel(&self, spec_id: &str) -> Result<(), AppError> {
        let spec = self
            .specs
            .get(spec_id)
            .ok_or_else(|| AppError::new(ErrorCode::SpecNotFound))?;
        let (execution_id, agent_id) = active_run(&spec).ok_or_else(|| {
            AppError::new(ErrorCode::SpecStateInvalid).with("reason", "nothing_running")
        })?;
        self.sessions
            .terminate(&SessionTarget {
                execution_id,
                workspace_id: spec.workspace_id.clone(),
                agent_id,
            })
            .map_err(|_| AppError::new(ErrorCode::ExecutionNotRunning))
    }
}

/// The execution an agent is running for the spec, and the agent.
fn active_run(spec: &Spec) -> Option<(String, String)> {
    for artifact in &spec.artifacts {
        if let Generation::Running { run_id, .. } = &artifact.generation {
            return Some((run_id.clone(), spec.author_agent_id.clone()));
        }
    }
    match spec.ai_review.as_ref().map(|r| &r.run) {
        Some(ReviewRun::Running { run_id, .. }) => {
            Some((run_id.clone(), spec.reviewer_agent_id.clone()))
        }
        _ => None,
    }
}

impl PreparedSpecRun {
    #[cfg(test)]
    pub fn execution_id(&self) -> &str {
        &self.execution_id
    }

    /// Does the agent's turn, blocking until it ends, and records the outcome.
    pub fn run(self, observer: &dyn ChatObserver) {
        let summary = self.pending.run(observer);
        let result = match &self.target {
            RunTarget::Artifact(artifact_id) => {
                let Some(spec) = self.specs.get(&self.spec_id) else {
                    return;
                };
                let Some(artifact) = spec.artifact(artifact_id) else {
                    return;
                };
                // Edits are applied to the document being revised, and only to that one.
                let revising = matches!(
                    artifact.generation,
                    Generation::Running {
                        mode: GenerationMode::Revise,
                        ..
                    }
                );
                let current = artifact
                    .latest()
                    .filter(|_| revising)
                    .map(|r| r.files.clone())
                    .unwrap_or_default();
                let end = interpret_generation(
                    self.framework.as_ref(),
                    artifact.kind,
                    &summary,
                    &current,
                );
                self.specs
                    .finish_generation(&self.spec_id, artifact_id, &self.execution_id, end)
            }
            RunTarget::Review => {
                let Some(spec) = self.specs.get(&self.spec_id) else {
                    return;
                };
                let review_id = spec
                    .ai_review
                    .as_ref()
                    .map(|r| r.id.clone())
                    .unwrap_or_default();
                let end = interpret_review(&spec, &summary, &review_id);
                self.specs
                    .finish_review(&self.spec_id, &self.execution_id, end)
            }
        };
        // A result that is not used (the run was superseded) or a save that failed is not the
        // agent's to answer for; the state says what it is.
        if let Err(error) = result {
            eprintln!(
                "the result of run {} for spec {} was not recorded: {error}",
                self.execution_id, self.spec_id
            );
        }
    }
}
