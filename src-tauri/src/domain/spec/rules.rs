//! The rules that move a specification from one state to another. Pure: no clock (callers pass
//! `now`), no storage, no agent. Each function checks that the move is allowed, makes it, and
//! refuses with a [`TransitionError`] when it is not, leaving the record as it was.
//!
//! The rules that matter most:
//! * a run's result is used only while the run is still the one the artifact is waiting for, so
//!   a late or repeated answer never overwrites a newer revision;
//! * an approval names the revision it was given on, and is worth nothing for another;
//! * a document is *approved* only if the person approved its latest revision and nothing it was
//!   written from has changed since;
//! * a spec is *ready* only when every condition holds at once, and the agent's say-so is none
//!   of them.

use std::collections::BTreeMap;

use super::model::{
    AiReview, Archived, ArtifactKind, ArtifactValidation, ConsistencyValidation, DecisionKind,
    Feedback, FinalApproval, Finding, FindingStatus, FrameworkKind, Generation, GenerationMode,
    ImplementationLink, IssueSeverity, Outdated, Publication, Question, QuestionSource,
    QuestionStatus, Review, ReviewRun, Revision, RevisionOrigin, RunKind, RunOutcome, Spec,
    SpecArtifact, SpecFile, SpecOrigin, SpecRun, SpecStatus, MAX_REVISIONS_KEPT, MAX_RUNS_KEPT,
};
use crate::domain::interaction::InteractionOption;

/// Why a move was refused. The service turns these into the errors the interface shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionError {
    ArtifactNotFound,
    QuestionNotFound,
    FindingNotFound,
    /// The spec was archived: nothing moves any more.
    Archived,
    NotStarted,
    /// Another run of this spec is going: one at a time, so two agents never write over each
    /// other and the person is never asked twice.
    RunInProgress,
    /// The documents this one is written from are not all approved yet (their ids).
    DependenciesNotApproved(Vec<String>),
    /// The artifact is in a state where this does not apply (`reason` is a stable code).
    WrongState(&'static str),
    /// The person looked at another revision than the current one.
    RevisionOutdated {
        current: u32,
    },
    /// A change request or a risk acceptance needs the person's words.
    NoteRequired,
    /// The run is no longer the one the artifact waits for: its result is dropped.
    Late,
    /// Approving needs the checks to have run on this revision and found no errors.
    ValidationBlocking,
    /// The final approval or the publication is asked for while conditions are not met.
    NotReady,
}

/// What a document is up to, for the interface: a single word that follows from the three
/// separate states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactPhase {
    /// The documents it is written from are not approved yet.
    Blocked,
    /// Can be written now; waiting for the person to ask.
    Ready,
    Generating,
    WaitingForInput,
    AwaitingReview,
    ChangesRequested,
    Approved,
    /// Approved or written once, but something it was written from changed since.
    Outdated,
    Failed,
}

/// Something that keeps a spec from being ready.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Blocker {
    /// Stable code the interface translates.
    pub code: String,
    #[serde(default)]
    pub artifact_id: Option<String>,
    #[serde(default)]
    pub count: Option<u32>,
}

impl Blocker {
    fn new(code: &str) -> Self {
        Self {
            code: code.to_owned(),
            artifact_id: None,
            count: None,
        }
    }

    fn of(code: &str, artifact: &str) -> Self {
        Self {
            code: code.to_owned(),
            artifact_id: Some(artifact.to_owned()),
            count: None,
        }
    }

    fn counted(code: &str, count: usize) -> Self {
        Self {
            code: code.to_owned(),
            artifact_id: None,
            count: Some(u32::try_from(count).unwrap_or(u32::MAX)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Readiness {
    pub ready: bool,
    pub blockers: Vec<Blocker>,
}

/// Blockers that mean "not run yet" rather than "run and found something".
const NOT_RUN: [&str; 6] = [
    "artifact_not_validated",
    "consistency_not_run",
    "review_not_run",
    "review_running",
    "review_outdated",
    "final_approval_missing",
];

/// What it takes to create a spec.
pub struct NewSpec {
    pub id: String,
    pub title: String,
    pub request: String,
    pub context: String,
    pub workspace_id: String,
    pub project_id: String,
    pub framework: FrameworkKind,
    pub framework_schema: String,
    pub change_id: String,
    pub author_agent_id: String,
    pub reviewer_agent_id: String,
    pub origin: SpecOrigin,
}

/// A spec with its artifacts and none of them started. `graph` is the framework's: each kind and
/// the kinds it is written from.
pub fn create(new: NewSpec, graph: &[(ArtifactKind, Vec<ArtifactKind>)], now: u64) -> Spec {
    let artifacts = graph
        .iter()
        .map(|(kind, depends_on)| SpecArtifact {
            id: kind.id().to_owned(),
            kind: *kind,
            depends_on: depends_on.iter().map(|d| d.id().to_owned()).collect(),
            revisions: Vec::new(),
            generation: Generation::Idle,
            review: Review::None,
            approved_revision: None,
            outdated: None,
            validation: None,
            feedback: Vec::new(),
            agent_id: None,
            created_at: now,
            updated_at: now,
        })
        .collect();
    Spec {
        id: new.id,
        title: new.title,
        request: new.request,
        context: new.context,
        workspace_id: new.workspace_id,
        project_id: new.project_id,
        framework: new.framework,
        framework_schema: new.framework_schema,
        change_id: new.change_id,
        author_agent_id: new.author_agent_id,
        reviewer_agent_id: new.reviewer_agent_id,
        started: false,
        artifacts,
        questions: Vec::new(),
        findings: Vec::new(),
        ai_review: None,
        consistency: None,
        final_approval: None,
        publication: Publication::NotPublished,
        archived: None,
        cli_check: None,
        implementations: Vec::new(),
        budget_steps: 0,
        runs: Vec::new(),
        origin: new.origin,
        version: 1,
        created_at: now,
        updated_at: now,
    }
}

fn touch(spec: &mut Spec, now: u64) {
    spec.version += 1;
    spec.updated_at = now;
}

fn open(spec: &Spec) -> Result<(), TransitionError> {
    if spec.archived.is_some() {
        return Err(TransitionError::Archived);
    }
    Ok(())
}

fn trimmed(note: Option<&str>) -> Option<String> {
    note.map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_owned)
}

// ---- runs ----------------------------------------------------------------------------------------

/// Whether an agent is working on the spec right now.
pub fn has_active_run(spec: &Spec) -> bool {
    spec.artifacts
        .iter()
        .any(|a| matches!(a.generation, Generation::Running { .. }))
        || matches!(
            spec.ai_review.as_ref().map(|r| &r.run),
            Some(ReviewRun::Running { .. })
        )
}

fn record_run(spec: &mut Spec, run: SpecRun) {
    spec.runs.push(run);
    if spec.runs.len() > MAX_RUNS_KEPT {
        let extra = spec.runs.len() - MAX_RUNS_KEPT;
        spec.runs.drain(..extra);
    }
}

fn end_run(spec: &mut Spec, run_id: &str, outcome: RunOutcome, now: u64) {
    if let Some(run) = spec.runs.iter_mut().find(|r| r.id == run_id) {
        if run.outcome.is_none() {
            run.outcome = Some(outcome);
            run.ended_at = Some(now);
        }
    }
}

/// The person pressed start. Nothing ran before, and only this lets anything run.
pub fn start(spec: &mut Spec, now: u64) -> Result<(), TransitionError> {
    open(spec)?;
    if !spec.started {
        spec.started = true;
        touch(spec, now);
    }
    Ok(())
}

/// The artifacts that could be written now: started, every document they are written from is
/// approved, and they are not written or waiting yet.
pub fn ready_to_generate(spec: &Spec) -> Vec<String> {
    if !spec.started || spec.archived.is_some() {
        return Vec::new();
    }
    spec.artifacts
        .iter()
        .filter(|a| can_generate(spec, a).is_ok())
        .map(|a| a.id.clone())
        .collect()
}

fn unapproved_dependencies(spec: &Spec, artifact: &SpecArtifact) -> Vec<String> {
    artifact
        .depends_on
        .iter()
        .filter(|id| !spec.artifact(id).is_some_and(SpecArtifact::is_approved))
        .cloned()
        .collect()
}

fn can_generate(spec: &Spec, artifact: &SpecArtifact) -> Result<GenerationMode, TransitionError> {
    match artifact.generation {
        Generation::Running { .. } => return Err(TransitionError::RunInProgress),
        Generation::WaitingForInput { .. } => {
            return Err(TransitionError::WrongState("waiting_for_input"))
        }
        Generation::Idle | Generation::Failed { .. } => {}
    }
    let missing = unapproved_dependencies(spec, artifact);
    if !missing.is_empty() {
        return Err(TransitionError::DependenciesNotApproved(missing));
    }
    match (artifact.latest(), artifact.review) {
        (None, _) => Ok(GenerationMode::Initial),
        (Some(_), Review::ChangesRequested) => Ok(
            match artifact
                .feedback
                .iter()
                .rev()
                .find(|f| Some(f.revision) == artifact.latest_number())
                .map(|f| f.kind)
            {
                Some(DecisionKind::Redo) => GenerationMode::Regenerate,
                _ => GenerationMode::Revise,
            },
        ),
        // Its sources changed: bring it up to date, keeping what is still right.
        (Some(_), _) if artifact.outdated.is_some() => Ok(GenerationMode::Revise),
        // A document waiting for review, or approved and current: nothing to write.
        (Some(_), _) => Err(TransitionError::WrongState("already_written")),
    }
}

/// How the artifact would be written now, or why it cannot be. Changes nothing: the screen and the
/// runner ask before they start anything.
pub fn generation_mode(spec: &Spec, artifact_id: &str) -> Result<GenerationMode, TransitionError> {
    open(spec)?;
    if !spec.started {
        return Err(TransitionError::NotStarted);
    }
    if has_active_run(spec) {
        return Err(TransitionError::RunInProgress);
    }
    let artifact = spec
        .artifact(artifact_id)
        .ok_or(TransitionError::ArtifactNotFound)?;
    can_generate(spec, artifact)
}

/// The agent is about to write the artifact. Returns how: from nothing, from feedback, or again.
pub fn begin_generation(
    spec: &mut Spec,
    artifact_id: &str,
    run_id: &str,
    agent_id: &str,
    now: u64,
) -> Result<GenerationMode, TransitionError> {
    let mode = generation_mode(spec, artifact_id)?;
    if let Some(artifact) = spec.artifact_mut(artifact_id) {
        artifact.generation = Generation::Running {
            run_id: run_id.to_owned(),
            started_at: now,
            mode,
        };
        artifact.agent_id = Some(agent_id.to_owned());
        artifact.updated_at = now;
    }
    record_run(
        spec,
        SpecRun {
            id: run_id.to_owned(),
            kind: RunKind::Generate,
            artifact_id: Some(artifact_id.to_owned()),
            agent_id: agent_id.to_owned(),
            started_at: now,
            ended_at: None,
            outcome: None,
        },
    );
    touch(spec, now);
    Ok(mode)
}

/// The mode the artifact's current run was started in, while it is the run `run_id`.
fn running_mode(artifact: &SpecArtifact, run_id: &str) -> Result<GenerationMode, TransitionError> {
    match &artifact.generation {
        Generation::Running {
            run_id: current,
            mode,
            ..
        } if current == run_id => Ok(*mode),
        _ => Err(TransitionError::Late),
    }
}

/// The agent's document arrives. Used only if the run is still the artifact's current one.
/// Returns the new revision's number.
pub fn complete_generation(
    spec: &mut Spec,
    artifact_id: &str,
    run_id: &str,
    files: Vec<SpecFile>,
    now: u64,
) -> Result<u32, TransitionError> {
    let artifact = spec
        .artifact(artifact_id)
        .ok_or(TransitionError::ArtifactNotFound)?;
    let mode = match running_mode(artifact, run_id) {
        Ok(mode) => mode,
        Err(error) => {
            end_run(spec, run_id, RunOutcome::Superseded, now);
            return Err(error);
        }
    };
    let based_on: BTreeMap<String, u32> = artifact
        .depends_on
        .iter()
        .filter_map(|id| {
            let dep = spec.artifact(id)?;
            dep.approved_revision
                .or_else(|| dep.latest_number())
                .map(|n| (id.clone(), n))
        })
        .collect();
    let first = artifact.revisions.is_empty();
    // A document that says the same thing as the one before it gives its dependents nothing new to
    // be written from: they are not called out of date for it.
    let unchanged = artifact
        .latest()
        .is_some_and(|previous| same_content(&previous.files, &files));
    let number = artifact.latest_number().unwrap_or(0) + 1;
    let agent_id = artifact.agent_id.clone();
    let origin = match mode {
        GenerationMode::Initial => RevisionOrigin::Generated,
        GenerationMode::Revise => RevisionOrigin::Revised,
        GenerationMode::Regenerate => RevisionOrigin::Regenerated,
    };
    if let Some(artifact) = spec.artifact_mut(artifact_id) {
        artifact.revisions.push(Revision {
            number,
            files,
            origin,
            run_id: Some(run_id.to_owned()),
            agent_id,
            based_on,
            created_at: now,
        });
        prune_revisions(artifact);
        artifact.generation = Generation::Idle;
        artifact.review = Review::AwaitingReview;
        artifact.outdated = None;
        artifact.validation = None;
        artifact.updated_at = now;
    }
    end_run(spec, run_id, RunOutcome::Completed, now);
    if !first && !unchanged {
        mark_dependents_outdated(spec, artifact_id, number, now);
    }
    touch(spec, now);
    Ok(number)
}

/// Whether two sets of files say the same thing: the same files, and in each the same lines, with
/// line endings and trailing blanks not counting. Anything else, a word, counts as a change.
fn same_content(before: &[SpecFile], after: &[SpecFile]) -> bool {
    fn lines(file: &SpecFile) -> Vec<&str> {
        file.content.trim().lines().map(str::trim_end).collect()
    }
    before.len() == after.len()
        && before.iter().all(|a| {
            after
                .iter()
                .any(|b| a.path == b.path && lines(a) == lines(b))
        })
}

/// Keeps the history bounded without losing an approved revision or the latest one.
fn prune_revisions(artifact: &mut SpecArtifact) {
    while artifact.revisions.len() > MAX_REVISIONS_KEPT {
        let keep = artifact.approved_revision;
        let latest = artifact.latest_number();
        let Some(position) = artifact
            .revisions
            .iter()
            .position(|r| Some(r.number) != keep && Some(r.number) != latest)
        else {
            break;
        };
        artifact.revisions.remove(position);
    }
}

/// Marks every document written (directly or not) from `changed` as outdated. It does not touch
/// their approval: they are simply no longer approved until the person looks again.
fn mark_dependents_outdated(spec: &mut Spec, changed: &str, revision: u32, now: u64) {
    let mut frontier = vec![changed.to_owned()];
    let mut seen = vec![changed.to_owned()];
    while let Some(source) = frontier.pop() {
        let dependents: Vec<String> = spec
            .artifacts
            .iter()
            .filter(|a| a.depends_on.contains(&source) && !seen.contains(&a.id))
            .map(|a| a.id.clone())
            .collect();
        for id in dependents {
            seen.push(id.clone());
            frontier.push(id.clone());
            if let Some(artifact) = spec.artifact_mut(&id) {
                if !artifact.revisions.is_empty() {
                    artifact.outdated = Some(Outdated {
                        because: changed.to_owned(),
                        revision,
                        at: now,
                    });
                    artifact.updated_at = now;
                }
            }
        }
    }
}

/// The run ended without a document (a failure, a cancel, the app closing).
pub fn fail_generation(
    spec: &mut Spec,
    artifact_id: &str,
    run_id: &str,
    message: &str,
    outcome: RunOutcome,
    now: u64,
) -> Result<(), TransitionError> {
    let artifact = spec
        .artifact(artifact_id)
        .ok_or(TransitionError::ArtifactNotFound)?;
    if running_mode(artifact, run_id).is_err() {
        end_run(spec, run_id, RunOutcome::Superseded, now);
        return Err(TransitionError::Late);
    }
    if let Some(artifact) = spec.artifact_mut(artifact_id) {
        // A cancel is not a failure: the person asked, and nothing is wrong.
        artifact.generation = if outcome == RunOutcome::Cancelled {
            Generation::Idle
        } else {
            Generation::Failed {
                message: message.to_owned(),
                at: now,
                interrupted: outcome == RunOutcome::Interrupted,
            }
        };
        artifact.updated_at = now;
    }
    end_run(spec, run_id, outcome, now);
    touch(spec, now);
    Ok(())
}

/// What a question needs to be recorded.
pub struct AskedQuestion {
    pub id: String,
    pub question: String,
    pub context: String,
    pub options: Vec<InteractionOption>,
}

/// The agent stopped to ask the person. The run is over; the answer starts another.
pub fn wait_for_input(
    spec: &mut Spec,
    artifact_id: &str,
    run_id: &str,
    asked: AskedQuestion,
    now: u64,
) -> Result<(), TransitionError> {
    let artifact = spec
        .artifact(artifact_id)
        .ok_or(TransitionError::ArtifactNotFound)?;
    if running_mode(artifact, run_id).is_err() {
        end_run(spec, run_id, RunOutcome::Superseded, now);
        return Err(TransitionError::Late);
    }
    let question_id = asked.id.clone();
    spec.questions.push(question_from(
        asked,
        QuestionSource::Artifact,
        Some(artifact_id),
        None,
        run_id,
        now,
    ));
    if let Some(artifact) = spec.artifact_mut(artifact_id) {
        artifact.generation = Generation::WaitingForInput { question_id };
        artifact.updated_at = now;
    }
    end_run(spec, run_id, RunOutcome::WaitingForInput, now);
    touch(spec, now);
    Ok(())
}

fn question_from(
    asked: AskedQuestion,
    source: QuestionSource,
    artifact_id: Option<&str>,
    finding_id: Option<&str>,
    run_id: &str,
    now: u64,
) -> Question {
    Question {
        id: asked.id,
        source,
        artifact_id: artifact_id.map(str::to_owned),
        finding_id: finding_id.map(str::to_owned),
        question: asked.question,
        context: asked.context,
        options: asked.options,
        status: QuestionStatus::Pending,
        choice: None,
        answer: None,
        run_id: run_id.to_owned(),
        asked_at: now,
        answered_at: None,
    }
}

/// The person's answer to a question an agent asked. For a document, the artifact goes back to
/// idle so the next run can use the answer; for the review, the review can be run again.
pub fn answer_question(
    spec: &mut Spec,
    question_id: &str,
    choice: Option<&str>,
    answer: Option<&str>,
    now: u64,
) -> Result<(), TransitionError> {
    open(spec)?;
    let question = spec
        .question(question_id)
        .ok_or(TransitionError::QuestionNotFound)?;
    if question.status != QuestionStatus::Pending {
        return Err(TransitionError::WrongState("question_answered"));
    }
    let (source, artifact_id) = (question.source, question.artifact_id.clone());
    if let Some(question) = spec.questions.iter_mut().find(|q| q.id == question_id) {
        question.status = QuestionStatus::Answered;
        question.choice = choice.map(str::to_owned);
        question.answer = answer.map(str::to_owned);
        question.answered_at = Some(now);
    }
    match (source, artifact_id) {
        (QuestionSource::Artifact, Some(id)) => {
            if let Some(artifact) = spec.artifact_mut(&id) {
                if matches!(&artifact.generation, Generation::WaitingForInput { question_id: q } if q == question_id)
                {
                    artifact.generation = Generation::Idle;
                    artifact.updated_at = now;
                }
            }
        }
        (QuestionSource::Review, _) => {
            if let Some(review) = spec.ai_review.as_mut() {
                if matches!(&review.run, ReviewRun::WaitingForInput { question_id: q } if q == question_id)
                {
                    // Ready to be run again, and nothing is waiting any more.
                    review.run = ReviewRun::Failed {
                        message: "answered".to_owned(),
                        at: now,
                        interrupted: false,
                    };
                }
            }
        }
        (QuestionSource::Artifact, None) => {}
    }
    touch(spec, now);
    Ok(())
}

/// The answers given to the questions of this artifact that the next run should know, oldest
/// first.
pub fn answered_questions<'a>(spec: &'a Spec, artifact_id: &str) -> Vec<&'a Question> {
    spec.questions
        .iter()
        .filter(|q| {
            q.status == QuestionStatus::Answered
                && q.artifact_id.as_deref() == Some(artifact_id)
                && q.source == QuestionSource::Artifact
        })
        .collect()
}

// ---- the person's decisions ------------------------------------------------------------------------

/// The person's answer about the artifact's latest revision: approve it, ask for changes, or
/// have it written again. `expected_revision` is the one they were looking at.
pub fn decide(
    spec: &mut Spec,
    artifact_id: &str,
    expected_revision: u32,
    kind: DecisionKind,
    note: Option<&str>,
    now: u64,
) -> Result<(), TransitionError> {
    open(spec)?;
    let artifact = spec
        .artifact(artifact_id)
        .ok_or(TransitionError::ArtifactNotFound)?;
    let latest = artifact
        .latest_number()
        .ok_or(TransitionError::WrongState("no_revision"))?;
    if latest != expected_revision {
        return Err(TransitionError::RevisionOutdated { current: latest });
    }
    if !matches!(
        artifact.generation,
        Generation::Idle | Generation::Failed { .. }
    ) {
        return Err(TransitionError::WrongState("run_in_progress"));
    }
    let note = trimmed(note);
    match kind {
        DecisionKind::Approve => {
            if artifact.outdated.is_some() {
                return Err(TransitionError::WrongState("outdated"));
            }
            if artifact.review == Review::Approved {
                return Err(TransitionError::WrongState("already_approved"));
            }
            let unapproved = unapproved_dependencies(spec, artifact);
            if !unapproved.is_empty() {
                return Err(TransitionError::DependenciesNotApproved(unapproved));
            }
            // Approval needs the checks to have looked at this very revision and found no error.
            match &artifact.validation {
                Some(v) if v.revision == latest && v.errors() == 0 => {}
                _ => return Err(TransitionError::ValidationBlocking),
            }
        }
        DecisionKind::Revise if note.is_none() => return Err(TransitionError::NoteRequired),
        DecisionKind::Revise | DecisionKind::Redo => {}
    }
    if let Some(artifact) = spec.artifact_mut(artifact_id) {
        artifact.feedback.push(Feedback {
            id: format!("fb-{artifact_id}-{}", artifact.feedback.len() + 1),
            revision: latest,
            kind,
            note,
            at: now,
        });
        match kind {
            DecisionKind::Approve => {
                artifact.review = Review::Approved;
                artifact.approved_revision = Some(latest);
            }
            DecisionKind::Revise | DecisionKind::Redo => {
                artifact.review = Review::ChangesRequested;
            }
        }
        artifact.updated_at = now;
    }
    touch(spec, now);
    Ok(())
}

/// The person looked at an outdated document and decided it still holds. It goes back to waiting
/// for review: nothing is approved on its behalf.
pub fn keep_outdated(
    spec: &mut Spec,
    artifact_id: &str,
    expected_revision: u32,
    now: u64,
) -> Result<(), TransitionError> {
    open(spec)?;
    let artifact = spec
        .artifact(artifact_id)
        .ok_or(TransitionError::ArtifactNotFound)?;
    if artifact.latest_number() != Some(expected_revision) {
        return Err(TransitionError::RevisionOutdated {
            current: artifact.latest_number().unwrap_or(0),
        });
    }
    if artifact.outdated.is_none() {
        return Err(TransitionError::WrongState("not_outdated"));
    }
    if !matches!(
        artifact.generation,
        Generation::Idle | Generation::Failed { .. }
    ) {
        return Err(TransitionError::WrongState("run_in_progress"));
    }
    if let Some(artifact) = spec.artifact_mut(artifact_id) {
        artifact.outdated = None;
        artifact.review = Review::AwaitingReview;
        artifact.updated_at = now;
    }
    touch(spec, now);
    Ok(())
}

// ---- checks --------------------------------------------------------------------------------------

/// Records what the checks found in a revision. A result for a revision that is no longer the
/// latest is refused: it must not stand in for the newer one.
pub fn record_validation(
    spec: &mut Spec,
    artifact_id: &str,
    validation: ArtifactValidation,
    now: u64,
) -> Result<(), TransitionError> {
    let artifact = spec
        .artifact_mut(artifact_id)
        .ok_or(TransitionError::ArtifactNotFound)?;
    if artifact.latest_number() != Some(validation.revision) {
        return Err(TransitionError::Late);
    }
    artifact.validation = Some(validation);
    artifact.updated_at = now;
    touch(spec, now);
    Ok(())
}

/// Records what the checks across the documents found, if they read the current revisions.
pub fn record_consistency(
    spec: &mut Spec,
    consistency: ConsistencyValidation,
    now: u64,
) -> Result<(), TransitionError> {
    if consistency.revisions != spec.current_revisions() {
        return Err(TransitionError::Late);
    }
    spec.consistency = Some(consistency);
    touch(spec, now);
    Ok(())
}

// ---- the reviewing agent -----------------------------------------------------------------------------

fn final_review_stage(spec: &Spec) -> Result<(), TransitionError> {
    if spec.artifacts.iter().all(SpecArtifact::is_approved) {
        Ok(())
    } else {
        Err(TransitionError::WrongState("artifacts_not_approved"))
    }
}

/// The reviewing agent is about to read the approved documents.
pub fn begin_review(
    spec: &mut Spec,
    review_id: &str,
    run_id: &str,
    agent_id: &str,
    now: u64,
) -> Result<(), TransitionError> {
    open(spec)?;
    if has_active_run(spec) {
        return Err(TransitionError::RunInProgress);
    }
    final_review_stage(spec)?;
    if spec
        .questions
        .iter()
        .any(|q| q.status == QuestionStatus::Pending && q.source == QuestionSource::Review)
    {
        return Err(TransitionError::WrongState("waiting_for_input"));
    }
    spec.ai_review = Some(AiReview {
        id: review_id.to_owned(),
        agent_id: agent_id.to_owned(),
        revisions: spec.current_revisions(),
        run: ReviewRun::Running {
            run_id: run_id.to_owned(),
            started_at: now,
        },
        started_at: now,
    });
    record_run(
        spec,
        SpecRun {
            id: run_id.to_owned(),
            kind: RunKind::Review,
            artifact_id: None,
            agent_id: agent_id.to_owned(),
            started_at: now,
            ended_at: None,
            outcome: None,
        },
    );
    touch(spec, now);
    Ok(())
}

fn running_review(spec: &Spec, run_id: &str) -> Result<(), TransitionError> {
    match spec.ai_review.as_ref().map(|r| &r.run) {
        Some(ReviewRun::Running {
            run_id: current, ..
        }) if current == run_id => Ok(()),
        _ => Err(TransitionError::Late),
    }
}

/// What the reviewing agent found, merged into the findings kept: the same problem found again
/// stays one finding with the person's decision on it; one that an artifact has been revised
/// past, and that is not reported again, is resolved.
pub fn complete_review(
    spec: &mut Spec,
    run_id: &str,
    found: Vec<Finding>,
    summary: &str,
    now: u64,
) -> Result<(), TransitionError> {
    if let Err(error) = running_review(spec, run_id) {
        end_run(spec, run_id, RunOutcome::Superseded, now);
        return Err(error);
    }
    let reported: Vec<String> = found.iter().map(|f| f.id.clone()).collect();
    for finding in found {
        match spec.findings.iter_mut().find(|f| f.id == finding.id) {
            Some(existing) => {
                // Same problem: keep the person's decision, refresh what the agent said.
                existing.description = finding.description;
                existing.evidence = finding.evidence;
                existing.rationale = finding.rationale;
                existing.suggestion = finding.suggestion;
                existing.question = finding.question;
                existing.severity = finding.severity;
                existing.artifact_revision = finding.artifact_revision;
                existing.review_id = finding.review_id;
            }
            None => spec.findings.push(finding),
        }
    }
    let artifacts: Vec<(String, Option<u32>)> = spec
        .artifacts
        .iter()
        .map(|a| (a.id.clone(), a.latest_number()))
        .collect();
    for finding in &mut spec.findings {
        let moved_on = match (&finding.artifact_id, finding.artifact_revision) {
            (Some(id), Some(found_at)) => artifacts
                .iter()
                .find(|(a, _)| a == id)
                .and_then(|(_, latest)| *latest)
                .is_some_and(|latest| latest > found_at),
            _ => false,
        };
        if finding.status == FindingStatus::Open && moved_on && !reported.contains(&finding.id) {
            finding.status = FindingStatus::Resolved;
            finding.resolution_note = Some("not reported by the later review".to_owned());
            finding.resolved_at = Some(now);
        }
    }
    if let Some(review) = spec.ai_review.as_mut() {
        review.run = ReviewRun::Completed {
            at: now,
            summary: summary.to_owned(),
        };
    }
    end_run(spec, run_id, RunOutcome::Completed, now);
    touch(spec, now);
    Ok(())
}

/// The review run ended without a review.
pub fn fail_review(
    spec: &mut Spec,
    run_id: &str,
    message: &str,
    outcome: RunOutcome,
    now: u64,
) -> Result<(), TransitionError> {
    if let Err(error) = running_review(spec, run_id) {
        end_run(spec, run_id, RunOutcome::Superseded, now);
        return Err(error);
    }
    if let Some(review) = spec.ai_review.as_mut() {
        review.run = ReviewRun::Failed {
            message: message.to_owned(),
            at: now,
            interrupted: outcome == RunOutcome::Interrupted,
        };
    }
    end_run(spec, run_id, outcome, now);
    touch(spec, now);
    Ok(())
}

/// The reviewing agent stopped to ask the person.
pub fn review_wait_for_input(
    spec: &mut Spec,
    run_id: &str,
    asked: AskedQuestion,
    now: u64,
) -> Result<(), TransitionError> {
    if let Err(error) = running_review(spec, run_id) {
        end_run(spec, run_id, RunOutcome::Superseded, now);
        return Err(error);
    }
    let question_id = asked.id.clone();
    spec.questions.push(question_from(
        asked,
        QuestionSource::Review,
        None,
        None,
        run_id,
        now,
    ));
    if let Some(review) = spec.ai_review.as_mut() {
        review.run = ReviewRun::WaitingForInput { question_id };
    }
    end_run(spec, run_id, RunOutcome::WaitingForInput, now);
    touch(spec, now);
    Ok(())
}

/// The person goes without the reviewing agent, and says so.
pub fn waive_review(spec: &mut Spec, note: Option<&str>, now: u64) -> Result<(), TransitionError> {
    open(spec)?;
    if has_active_run(spec) {
        return Err(TransitionError::RunInProgress);
    }
    final_review_stage(spec)?;
    let note = trimmed(note).ok_or(TransitionError::NoteRequired)?;
    spec.ai_review = Some(AiReview {
        id: format!("review-waived-{now}"),
        agent_id: spec.reviewer_agent_id.clone(),
        revisions: spec.current_revisions(),
        run: ReviewRun::Waived { note, at: now },
        started_at: now,
    });
    touch(spec, now);
    Ok(())
}

/// The person's decision on a finding. Accepting a risk needs their words, so the acceptance is
/// on record and is theirs.
pub fn resolve_finding(
    spec: &mut Spec,
    finding_id: &str,
    status: FindingStatus,
    note: Option<&str>,
    now: u64,
) -> Result<(), TransitionError> {
    open(spec)?;
    let note = trimmed(note);
    if status == FindingStatus::AcceptedRisk && note.is_none() {
        return Err(TransitionError::NoteRequired);
    }
    let finding = spec
        .findings
        .iter_mut()
        .find(|f| f.id == finding_id)
        .ok_or(TransitionError::FindingNotFound)?;
    finding.status = status;
    finding.resolution_note = note;
    finding.resolved_at = (status != FindingStatus::Open).then_some(now);
    touch(spec, now);
    Ok(())
}

// ---- final approval, publication, archive -------------------------------------------------------------

fn blockers(spec: &Spec) -> Vec<Blocker> {
    let mut out = Vec::new();
    let current = spec.current_revisions();
    for artifact in &spec.artifacts {
        let Some(latest) = artifact.latest_number() else {
            out.push(Blocker::of("artifact_not_written", &artifact.id));
            continue;
        };
        if artifact.outdated.is_some() {
            out.push(Blocker::of("artifact_outdated", &artifact.id));
            continue;
        }
        if !artifact.is_approved() {
            out.push(Blocker::of("artifact_not_approved", &artifact.id));
            continue;
        }
        match &artifact.validation {
            Some(v) if v.revision == latest => {
                if v.errors() > 0 {
                    out.push(Blocker::of("artifact_has_errors", &artifact.id));
                }
            }
            _ => out.push(Blocker::of("artifact_not_validated", &artifact.id)),
        }
    }
    match &spec.consistency {
        Some(c) if c.revisions == current => {
            let errors = c
                .issues
                .iter()
                .filter(|i| i.severity == IssueSeverity::Error)
                .count();
            if errors > 0 {
                out.push(Blocker::counted("consistency_errors", errors));
            }
        }
        _ => out.push(Blocker::new("consistency_not_run")),
    }
    let pending = spec
        .questions
        .iter()
        .filter(|q| q.status == QuestionStatus::Pending)
        .count();
    if pending > 0 {
        out.push(Blocker::counted("questions_pending", pending));
    }
    match &spec.ai_review {
        None => out.push(Blocker::new("review_not_run")),
        Some(review) if review.revisions != current => out.push(Blocker::new("review_outdated")),
        Some(review) => match &review.run {
            ReviewRun::Running { .. } => out.push(Blocker::new("review_running")),
            ReviewRun::Failed { .. } => out.push(Blocker::new("review_failed")),
            // A question is counted above; the review is run again once it is answered.
            ReviewRun::WaitingForInput { .. }
            | ReviewRun::Completed { .. }
            | ReviewRun::Waived { .. } => {}
        },
    }
    let open_findings = spec.findings.iter().filter(|f| f.blocks()).count();
    if open_findings > 0 {
        out.push(Blocker::counted("findings_open", open_findings));
    }
    match &spec.publication {
        Publication::InProgress { .. } => out.push(Blocker::new("publication_in_progress")),
        Publication::Failed { .. } => out.push(Blocker::new("publication_failed")),
        Publication::Published { revisions, .. } if *revisions != current => {
            out.push(Blocker::new("publication_outdated"));
        }
        Publication::Published { .. } | Publication::NotPublished => {}
    }
    out
}

/// What keeps the spec from being approved or published, leaving out what only says the project's
/// files are behind: publishing again is the remedy for those, so they cannot also stand in its way.
fn blockers_before_publishing(spec: &Spec) -> Vec<Blocker> {
    blockers(spec)
        .into_iter()
        .filter(|b| {
            !matches!(
                b.code.as_str(),
                "publication_in_progress" | "publication_failed" | "publication_outdated"
            )
        })
        .collect()
}

/// Everything that keeps the spec from being ready for implementation, or nothing.
pub fn readiness(spec: &Spec) -> Readiness {
    let mut list = blockers(spec);
    let approved_now =
        matches!(&spec.final_approval, Some(a) if a.revisions == spec.current_revisions());
    if !approved_now {
        list.push(Blocker::new("final_approval_missing"));
    }
    Readiness {
        ready: list.is_empty(),
        blockers: list,
    }
}

/// The person's last word. Needs everything else to hold; it names the revisions it was given
/// on, so changing any of them voids it.
pub fn final_approve(
    spec: &mut Spec,
    warnings: Vec<String>,
    now: u64,
) -> Result<(), TransitionError> {
    open(spec)?;
    if has_active_run(spec) {
        return Err(TransitionError::RunInProgress);
    }
    if !blockers_before_publishing(spec).is_empty() {
        return Err(TransitionError::NotReady);
    }
    spec.final_approval = Some(FinalApproval {
        revisions: spec.current_revisions(),
        acknowledged_warnings: warnings,
        at: now,
    });
    touch(spec, now);
    Ok(())
}

/// Where the files will be written: the spec has to be ready, or an earlier attempt left off.
pub fn begin_publication(
    spec: &mut Spec,
    change_id: &str,
    now: u64,
) -> Result<(), TransitionError> {
    open(spec)?;
    let resuming = matches!(
        spec.publication,
        Publication::InProgress { .. } | Publication::Failed { .. }
    );
    // What is behind in the project is what this is about to fix; everything else must hold.
    let others = blockers_before_publishing(spec).len();
    let approved_now =
        matches!(&spec.final_approval, Some(a) if a.revisions == spec.current_revisions());
    if others > 0 || !approved_now {
        return Err(TransitionError::NotReady);
    }
    let written = match &spec.publication {
        Publication::InProgress { written, .. } | Publication::Failed { written, .. }
            if resuming =>
        {
            written.clone()
        }
        _ => Vec::new(),
    };
    spec.publication = Publication::InProgress {
        started_at: now,
        change_id: change_id.to_owned(),
        written,
        revisions: spec.current_revisions(),
    };
    touch(spec, now);
    Ok(())
}

pub fn publication_progress(spec: &mut Spec, file: &str, now: u64) {
    if let Publication::InProgress { written, .. } = &mut spec.publication {
        if !written.iter().any(|w| w == file) {
            written.push(file.to_owned());
        }
        touch(spec, now);
    }
}

pub fn complete_publication(spec: &mut Spec, root: &str, files: Vec<String>, now: u64) {
    if let Publication::InProgress {
        change_id,
        revisions,
        ..
    } = &spec.publication
    {
        spec.publication = Publication::Published {
            change_id: change_id.clone(),
            root: root.to_owned(),
            files,
            revisions: revisions.clone(),
            at: now,
        };
        touch(spec, now);
    }
}

pub fn fail_publication(spec: &mut Spec, message: &str, now: u64) {
    if let Publication::InProgress {
        change_id, written, ..
    } = &spec.publication
    {
        spec.publication = Publication::Failed {
            message: message.to_owned(),
            at: now,
            change_id: change_id.clone(),
            written: written.clone(),
        };
        touch(spec, now);
    }
}

/// Closes the spec. Only a published one, so what was agreed is in the project before it is put
/// away; nothing is deleted from Atlas's record.
pub fn archive(
    spec: &mut Spec,
    path: Option<String>,
    method: &str,
    now: u64,
) -> Result<(), TransitionError> {
    open(spec)?;
    if !matches!(spec.publication, Publication::Published { .. }) {
        return Err(TransitionError::WrongState("not_published"));
    }
    spec.archived = Some(Archived {
        at: now,
        path,
        method: method.to_owned(),
    });
    touch(spec, now);
    Ok(())
}

/// What the `OpenSpec` command said of the published change.
pub fn record_cli_check(spec: &mut Spec, check: super::model::CliCheck, now: u64) {
    spec.cli_check = Some(check);
    touch(spec, now);
}

/// A workflow run that implements the spec. Only a spec that is ready can be implemented.
pub fn link_implementation(
    spec: &mut Spec,
    link: ImplementationLink,
    now: u64,
) -> Result<(), TransitionError> {
    open(spec)?;
    if !readiness(spec).ready {
        return Err(TransitionError::NotReady);
    }
    spec.implementations.push(link);
    touch(spec, now);
    Ok(())
}

// ---- recovery -----------------------------------------------------------------------------------------

/// The app ended while a run was going: its result will never come. The artifacts and the review
/// say so, nothing is guessed, and what was written before stays. Returns whether anything
/// changed.
pub fn recover_interrupted(spec: &mut Spec, now: u64) -> bool {
    let mut changed = false;
    let mut interrupted: Vec<String> = Vec::new();
    for artifact in &mut spec.artifacts {
        if let Generation::Running { run_id, .. } = &artifact.generation {
            interrupted.push(run_id.clone());
            artifact.generation = Generation::Failed {
                message: "interrupted".to_owned(),
                at: now,
                interrupted: true,
            };
            changed = true;
        }
    }
    if let Some(review) = spec.ai_review.as_mut() {
        if let ReviewRun::Running { run_id, .. } = &review.run {
            interrupted.push(run_id.clone());
            review.run = ReviewRun::Failed {
                message: "interrupted".to_owned(),
                at: now,
                interrupted: true,
            };
            changed = true;
        }
    }
    for run in &mut spec.runs {
        if run.outcome.is_none() {
            run.outcome = Some(RunOutcome::Interrupted);
            run.ended_at = Some(now);
            changed = true;
        }
    }
    let _ = interrupted;
    if changed {
        touch(spec, now);
    }
    changed
}

// ---- views --------------------------------------------------------------------------------------------

pub fn artifact_phase(spec: &Spec, artifact: &SpecArtifact) -> ArtifactPhase {
    match &artifact.generation {
        Generation::Running { .. } => return ArtifactPhase::Generating,
        Generation::WaitingForInput { .. } => return ArtifactPhase::WaitingForInput,
        Generation::Failed { .. } => return ArtifactPhase::Failed,
        Generation::Idle => {}
    }
    if artifact.latest().is_none() {
        return if unapproved_dependencies(spec, artifact).is_empty() {
            ArtifactPhase::Ready
        } else {
            ArtifactPhase::Blocked
        };
    }
    if artifact.outdated.is_some() {
        return ArtifactPhase::Outdated;
    }
    match artifact.review {
        Review::ChangesRequested => ArtifactPhase::ChangesRequested,
        Review::Approved if artifact.is_approved() => ArtifactPhase::Approved,
        Review::None | Review::AwaitingReview | Review::Approved => ArtifactPhase::AwaitingReview,
    }
}

/// How the spec is doing, from its documents and checks alone.
pub fn status(spec: &Spec) -> SpecStatus {
    if spec.archived.is_some() {
        return SpecStatus::Archived;
    }
    let readiness = readiness(spec);
    if readiness.ready {
        return if matches!(spec.publication, Publication::Published { .. }) {
            SpecStatus::Published
        } else {
            SpecStatus::ReadyForImplementation
        };
    }
    if !spec.started {
        return SpecStatus::Draft;
    }
    let phases: Vec<ArtifactPhase> = spec
        .artifacts
        .iter()
        .map(|a| artifact_phase(spec, a))
        .collect();
    let reviewing = |f: fn(&ReviewRun) -> bool| spec.ai_review.as_ref().is_some_and(|r| f(&r.run));
    if phases.contains(&ArtifactPhase::Generating)
        || reviewing(|r| matches!(r, ReviewRun::Running { .. }))
    {
        return SpecStatus::Generating;
    }
    if spec
        .questions
        .iter()
        .any(|q| q.status == QuestionStatus::Pending)
    {
        return SpecStatus::WaitingForInput;
    }
    if phases.contains(&ArtifactPhase::Failed)
        || reviewing(|r| matches!(r, ReviewRun::Failed { message, .. } if message != "answered"))
    {
        return SpecStatus::Failed;
    }
    if phases.contains(&ArtifactPhase::ChangesRequested) {
        return SpecStatus::ChangesRequested;
    }
    if phases.contains(&ArtifactPhase::AwaitingReview) {
        return SpecStatus::AwaitingReview;
    }
    if phases.contains(&ArtifactPhase::Outdated) {
        return SpecStatus::Blocked;
    }
    if phases.iter().all(|p| *p == ArtifactPhase::Approved) {
        let remaining: Vec<&Blocker> = readiness
            .blockers
            .iter()
            .filter(|b| b.code != "final_approval_missing")
            .collect();
        if remaining.is_empty() {
            return SpecStatus::Approved;
        }
        return if remaining.iter().all(|b| NOT_RUN.contains(&b.code.as_str())) {
            SpecStatus::AwaitingValidation
        } else {
            SpecStatus::Blocked
        };
    }
    SpecStatus::AwaitingGeneration
}

/// A stable id for a finding, from what makes it the same problem: where it is and what it is
/// about. The agent's wording of the description may change between reviews; this does not.
pub fn finding_id(
    category: &str,
    artifact_id: Option<&str>,
    reference: &str,
    title: &str,
) -> String {
    fn norm(text: &str) -> String {
        text.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    }
    let key = format!(
        "{}|{}|{}|{}",
        norm(category),
        artifact_id.unwrap_or(""),
        norm(reference),
        norm(title)
    );
    // FNV-1a: enough to tell findings apart, and stable across runs and platforms.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in key.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("f-{hash:016x}")
}

#[cfg(test)]
pub(crate) mod tests_support {
    use super::*;

    pub fn graph() -> Vec<(ArtifactKind, Vec<ArtifactKind>)> {
        vec![
            (ArtifactKind::Proposal, vec![]),
            (ArtifactKind::Specs, vec![ArtifactKind::Proposal]),
            (
                ArtifactKind::Design,
                vec![ArtifactKind::Proposal, ArtifactKind::Specs],
            ),
            (
                ArtifactKind::Tasks,
                vec![ArtifactKind::Specs, ArtifactKind::Design],
            ),
        ]
    }

    pub fn spec() -> Spec {
        create(
            NewSpec {
                id: "spec-1".to_owned(),
                title: "Dark mode".to_owned(),
                request: "Add a dark mode".to_owned(),
                context: String::new(),
                workspace_id: "w1".to_owned(),
                project_id: "p1".to_owned(),
                framework: FrameworkKind::Openspec,
                framework_schema: "spec-driven/1".to_owned(),
                change_id: "add-dark-mode".to_owned(),
                author_agent_id: "a1".to_owned(),
                reviewer_agent_id: "a2".to_owned(),
                origin: SpecOrigin::Created,
            },
            &graph(),
            1,
        )
    }

    pub fn file(text: &str) -> Vec<SpecFile> {
        vec![SpecFile {
            path: "doc.md".to_owned(),
            content: text.to_owned(),
        }]
    }

    pub fn valid(artifact: &str, revision: u32) -> ArtifactValidation {
        let _ = artifact;
        ArtifactValidation {
            revision,
            validator: "t/1".to_owned(),
            issues: Vec::new(),
            at: 1,
        }
    }

    /// Generates, validates and approves one artifact.
    pub fn write_and_approve(spec: &mut Spec, id: &str, now: u64) -> u32 {
        write_and_approve_with(spec, id, "text", now)
    }

    /// The same, with the document's text.
    pub fn write_and_approve_with(spec: &mut Spec, id: &str, text: &str, now: u64) -> u32 {
        let run = format!("run-{id}-{now}");
        begin_generation(spec, id, &run, "a1", now).unwrap();
        let path = match id {
            "proposal" => "proposal.md",
            "design" => "design.md",
            "tasks" => "tasks.md",
            _ => "specs/cap/spec.md",
        };
        let files = vec![SpecFile {
            path: path.to_owned(),
            content: text.to_owned(),
        }];
        let n = complete_generation(spec, id, &run, files, now + 1).unwrap();
        record_validation(spec, id, valid(id, n), now + 2).unwrap();
        decide(spec, id, n, DecisionKind::Approve, None, now + 3).unwrap();
        n
    }

    pub fn all_approved() -> Spec {
        let mut s = spec();
        start(&mut s, 2).unwrap();
        for (i, id) in ["proposal", "specs", "design", "tasks"].iter().enumerate() {
            write_and_approve(&mut s, id, 10 + (i as u64) * 10);
        }
        s
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::assert_is_empty)]
    use super::tests_support::{all_approved, file, spec, valid, write_and_approve};
    use super::*;
    use crate::domain::spec::model::{FindingSeverity, ValidationIssue};

    fn make_ready(s: &mut Spec) {
        let current = s.current_revisions();
        record_consistency(
            s,
            ConsistencyValidation {
                revisions: current,
                validator: "t/1".to_owned(),
                issues: Vec::new(),
                at: 100,
            },
            100,
        )
        .unwrap();
        begin_review(s, "rev-1", "run-review", "a2", 101).unwrap();
        complete_review(s, "run-review", Vec::new(), "fine", 102).unwrap();
        final_approve(s, Vec::new(), 103).unwrap();
    }

    #[test]
    fn nothing_can_run_until_the_person_starts_it() {
        let mut s = spec();

        assert_eq!(
            begin_generation(&mut s, "proposal", "r1", "a1", 2),
            Err(TransitionError::NotStarted)
        );
        assert_eq!(status(&s), SpecStatus::Draft);
        assert!(ready_to_generate(&s).is_empty());

        start(&mut s, 3).unwrap();

        assert_eq!(ready_to_generate(&s), vec!["proposal".to_owned()]);
        assert_eq!(status(&s), SpecStatus::AwaitingGeneration);
    }

    #[test]
    fn a_document_waits_for_the_ones_it_is_written_from() {
        let mut s = spec();
        start(&mut s, 2).unwrap();

        let refused = begin_generation(&mut s, "specs", "r1", "a1", 3);

        assert_eq!(
            refused,
            Err(TransitionError::DependenciesNotApproved(vec![
                "proposal".to_owned()
            ]))
        );
        assert_eq!(
            artifact_phase(&s, s.artifact("specs").unwrap()),
            ArtifactPhase::Blocked
        );
    }

    #[test]
    fn generating_leaves_a_draft_to_review_not_an_approved_document() {
        let mut s = spec();
        start(&mut s, 2).unwrap();
        let mode = begin_generation(&mut s, "proposal", "r1", "a1", 3).unwrap();
        assert_eq!(mode, GenerationMode::Initial);
        assert_eq!(status(&s), SpecStatus::Generating);

        let n = complete_generation(&mut s, "proposal", "r1", file("# Proposal"), 4).unwrap();

        let a = s.artifact("proposal").unwrap();
        assert_eq!(n, 1);
        assert_eq!(a.review, Review::AwaitingReview);
        assert!(!a.is_approved());
        assert_eq!(status(&s), SpecStatus::AwaitingReview);
    }

    #[test]
    fn one_run_at_a_time_per_spec() {
        let mut s = spec();
        start(&mut s, 2).unwrap();
        begin_generation(&mut s, "proposal", "r1", "a1", 3).unwrap();

        assert_eq!(
            begin_generation(&mut s, "proposal", "r2", "a1", 4),
            Err(TransitionError::RunInProgress)
        );
    }

    #[test]
    fn a_late_or_repeated_result_never_overwrites_a_newer_revision() {
        let mut s = spec();
        start(&mut s, 2).unwrap();
        begin_generation(&mut s, "proposal", "r1", "a1", 3).unwrap();
        complete_generation(&mut s, "proposal", "r1", file("first"), 4).unwrap();

        // The same run answers again, and an old run nobody waits for answers too.
        let again = complete_generation(&mut s, "proposal", "r1", file("again"), 5);
        let stray = complete_generation(&mut s, "proposal", "r0", file("stray"), 6);

        assert_eq!(again, Err(TransitionError::Late));
        assert_eq!(stray, Err(TransitionError::Late));
        let a = s.artifact("proposal").unwrap();
        assert_eq!(a.revisions.len(), 1);
        assert_eq!(a.revisions[0].files[0].content, "first");
    }

    #[test]
    fn approval_needs_the_checks_to_have_run_on_that_revision_without_errors() {
        let mut s = spec();
        start(&mut s, 2).unwrap();
        begin_generation(&mut s, "proposal", "r1", "a1", 3).unwrap();
        let n = complete_generation(&mut s, "proposal", "r1", file("x"), 4).unwrap();

        assert_eq!(
            decide(&mut s, "proposal", n, DecisionKind::Approve, None, 5),
            Err(TransitionError::ValidationBlocking)
        );

        let mut with_error = valid("proposal", n);
        with_error.issues.push(ValidationIssue {
            code: "why_missing".to_owned(),
            severity: IssueSeverity::Error,
            file: None,
            line: None,
            message: String::new(),
            params: BTreeMap::new(),
        });
        record_validation(&mut s, "proposal", with_error, 5).unwrap();
        assert_eq!(
            decide(&mut s, "proposal", n, DecisionKind::Approve, None, 6),
            Err(TransitionError::ValidationBlocking)
        );

        record_validation(&mut s, "proposal", valid("proposal", n), 7).unwrap();
        decide(&mut s, "proposal", n, DecisionKind::Approve, None, 8).unwrap();
        assert!(s.artifact("proposal").unwrap().is_approved());
    }

    #[test]
    fn a_validation_of_an_older_revision_cannot_stand_in_for_the_newer_one() {
        let mut s = spec();
        start(&mut s, 2).unwrap();
        begin_generation(&mut s, "proposal", "r1", "a1", 3).unwrap();
        complete_generation(&mut s, "proposal", "r1", file("one"), 4).unwrap();
        record_validation(&mut s, "proposal", valid("proposal", 1), 5).unwrap();
        decide(&mut s, "proposal", 1, DecisionKind::Revise, Some("more"), 6).unwrap();
        begin_generation(&mut s, "proposal", "r2", "a1", 7).unwrap();
        complete_generation(&mut s, "proposal", "r2", file("two"), 8).unwrap();

        // The result for revision 1 arrives after revision 2 exists.
        assert_eq!(
            record_validation(&mut s, "proposal", valid("proposal", 1), 9),
            Err(TransitionError::Late)
        );
        assert_eq!(
            decide(&mut s, "proposal", 2, DecisionKind::Approve, None, 10),
            Err(TransitionError::ValidationBlocking)
        );
    }

    #[test]
    fn an_approval_is_for_the_revision_the_person_looked_at() {
        let mut s = spec();
        start(&mut s, 2).unwrap();
        begin_generation(&mut s, "proposal", "r1", "a1", 3).unwrap();
        let n = complete_generation(&mut s, "proposal", "r1", file("x"), 4).unwrap();
        record_validation(&mut s, "proposal", valid("proposal", n), 5).unwrap();

        assert_eq!(
            decide(&mut s, "proposal", n + 1, DecisionKind::Approve, None, 6),
            Err(TransitionError::RevisionOutdated { current: n })
        );
        assert_eq!(
            s.artifact("proposal").unwrap().review,
            Review::AwaitingReview
        );
    }

    #[test]
    fn changes_need_words_and_send_the_document_back_to_be_revised() {
        let mut s = spec();
        start(&mut s, 2).unwrap();
        begin_generation(&mut s, "proposal", "r1", "a1", 3).unwrap();
        let n = complete_generation(&mut s, "proposal", "r1", file("x"), 4).unwrap();

        assert_eq!(
            decide(&mut s, "proposal", n, DecisionKind::Revise, Some("   "), 5),
            Err(TransitionError::NoteRequired)
        );
        decide(
            &mut s,
            "proposal",
            n,
            DecisionKind::Revise,
            Some("Add the risks"),
            6,
        )
        .unwrap();

        assert_eq!(status(&s), SpecStatus::ChangesRequested);
        let mode = begin_generation(&mut s, "proposal", "r2", "a1", 7).unwrap();
        assert_eq!(mode, GenerationMode::Revise);
        complete_generation(&mut s, "proposal", "r2", file("y"), 8).unwrap();
        let a = s.artifact("proposal").unwrap();
        assert_eq!(a.revisions.len(), 2, "the first revision is kept");
        assert_eq!(a.revisions[1].origin, RevisionOrigin::Revised);
        assert_eq!(a.feedback[0].note.as_deref(), Some("Add the risks"));
    }

    #[test]
    fn redo_writes_again_and_keeps_the_history() {
        let mut s = spec();
        start(&mut s, 2).unwrap();
        begin_generation(&mut s, "proposal", "r1", "a1", 3).unwrap();
        let n = complete_generation(&mut s, "proposal", "r1", file("x"), 4).unwrap();
        decide(&mut s, "proposal", n, DecisionKind::Redo, None, 5).unwrap();

        let mode = begin_generation(&mut s, "proposal", "r2", "a1", 6).unwrap();
        complete_generation(&mut s, "proposal", "r2", file("z"), 7).unwrap();

        assert_eq!(mode, GenerationMode::Regenerate);
        let a = s.artifact("proposal").unwrap();
        assert_eq!(a.revisions[1].origin, RevisionOrigin::Regenerated);
        assert_eq!(a.revisions[0].files[0].content, "x");
    }

    #[test]
    fn changing_a_requirement_after_the_design_was_approved_marks_what_follows_outdated() {
        let mut s = all_approved();
        // The person asks for changes to the approved specs.
        decide(
            &mut s,
            "specs",
            1,
            DecisionKind::Revise,
            Some("Add an error case"),
            100,
        )
        .unwrap();
        begin_generation(&mut s, "specs", "r-new", "a1", 101).unwrap();
        complete_generation(&mut s, "specs", "r-new", file("new"), 102).unwrap();

        let design = s.artifact("design").unwrap();
        let tasks = s.artifact("tasks").unwrap();
        assert!(design.outdated.is_some() && tasks.outdated.is_some());
        assert!(
            !design.is_approved(),
            "it is not approved on its own say-so"
        );
        assert_eq!(artifact_phase(&s, design), ArtifactPhase::Outdated);
        // The proposal, which the specs depend on, is untouched.
        assert!(s.artifact("proposal").unwrap().is_approved());
    }

    #[test]
    fn an_outdated_document_can_be_kept_but_has_to_be_approved_again() {
        let mut s = all_approved();
        decide(&mut s, "specs", 1, DecisionKind::Redo, None, 100).unwrap();
        begin_generation(&mut s, "specs", "r-new", "a1", 101).unwrap();
        complete_generation(&mut s, "specs", "r-new", file("new"), 102).unwrap();
        let n = s.artifact("design").unwrap().latest_number().unwrap();

        keep_outdated(&mut s, "design", n, 103).unwrap();

        let design = s.artifact("design").unwrap();
        assert!(design.outdated.is_none());
        assert_eq!(design.review, Review::AwaitingReview);
        assert!(!design.is_approved());
    }

    #[test]
    fn an_outdated_document_is_brought_up_to_date_by_revising_it() {
        let mut s = all_approved();
        decide(&mut s, "specs", 1, DecisionKind::Redo, None, 100).unwrap();
        begin_generation(&mut s, "specs", "r-new", "a1", 101).unwrap();
        complete_generation(&mut s, "specs", "r-new", file("new"), 102).unwrap();
        // The specs have to be approved again before the design may be rewritten from them.
        assert!(begin_generation(&mut s, "design", "r-d", "a1", 103).is_err());
        record_validation(&mut s, "specs", valid("specs", 2), 104).unwrap();
        decide(&mut s, "specs", 2, DecisionKind::Approve, None, 105).unwrap();

        let mode = begin_generation(&mut s, "design", "r-d", "a1", 106).unwrap();

        assert_eq!(mode, GenerationMode::Revise);
        complete_generation(&mut s, "design", "r-d", file("d2"), 107).unwrap();
        assert!(s.artifact("design").unwrap().outdated.is_none());
    }

    #[test]
    fn a_question_pauses_the_artifact_and_the_answer_lets_it_go_on() {
        let mut s = spec();
        start(&mut s, 2).unwrap();
        begin_generation(&mut s, "proposal", "r1", "a1", 3).unwrap();
        wait_for_input(
            &mut s,
            "proposal",
            "r1",
            AskedQuestion {
                id: "q1".to_owned(),
                question: "Which users?".to_owned(),
                context: String::new(),
                options: Vec::new(),
            },
            4,
        )
        .unwrap();
        assert_eq!(status(&s), SpecStatus::WaitingForInput);
        assert_eq!(
            begin_generation(&mut s, "proposal", "r2", "a1", 5),
            Err(TransitionError::WrongState("waiting_for_input"))
        );

        answer_question(&mut s, "q1", None, Some("Admins"), 6).unwrap();

        assert_eq!(answered_questions(&s, "proposal").len(), 1);
        assert_eq!(
            begin_generation(&mut s, "proposal", "r2", "a1", 7).unwrap(),
            GenerationMode::Initial
        );
        // A question is answered once.
        assert_eq!(
            answer_question(&mut s, "q1", None, Some("again"), 8),
            Err(TransitionError::WrongState("question_answered"))
        );
    }

    #[test]
    fn a_failed_run_keeps_what_was_written_before() {
        let mut s = spec();
        start(&mut s, 2).unwrap();
        begin_generation(&mut s, "proposal", "r1", "a1", 3).unwrap();
        let n = complete_generation(&mut s, "proposal", "r1", file("good"), 4).unwrap();
        decide(&mut s, "proposal", n, DecisionKind::Redo, None, 5).unwrap();
        begin_generation(&mut s, "proposal", "r2", "a1", 6).unwrap();

        fail_generation(
            &mut s,
            "proposal",
            "r2",
            "runtime failed",
            RunOutcome::Failed,
            7,
        )
        .unwrap();

        let a = s.artifact("proposal").unwrap();
        assert_eq!(a.revisions.len(), 1);
        assert_eq!(a.revisions[0].files[0].content, "good");
        assert!(matches!(a.generation, Generation::Failed { .. }));
        assert_eq!(status(&s), SpecStatus::Failed);
        // And it can be tried again.
        assert!(begin_generation(&mut s, "proposal", "r3", "a1", 8).is_ok());
    }

    #[test]
    fn an_interrupted_run_is_reported_as_such_after_a_restart() {
        let mut s = spec();
        start(&mut s, 2).unwrap();
        begin_generation(&mut s, "proposal", "r1", "a1", 3).unwrap();

        assert!(recover_interrupted(&mut s, 9));

        assert!(matches!(
            s.artifact("proposal").unwrap().generation,
            Generation::Failed {
                interrupted: true,
                ..
            }
        ));
        assert_eq!(s.runs[0].outcome, Some(RunOutcome::Interrupted));
        // A result of that run that turns up afterwards is dropped.
        assert_eq!(
            complete_generation(&mut s, "proposal", "r1", file("late"), 10),
            Err(TransitionError::Late)
        );
        assert!(!recover_interrupted(&mut s, 11), "nothing left to recover");
    }

    #[test]
    fn a_spec_is_not_ready_because_the_documents_are_approved() {
        let s = all_approved();

        let readiness = readiness(&s);

        assert!(!readiness.ready);
        let codes: Vec<&str> = readiness.blockers.iter().map(|b| b.code.as_str()).collect();
        assert!(codes.contains(&"consistency_not_run"));
        assert!(codes.contains(&"review_not_run"));
        assert!(codes.contains(&"final_approval_missing"));
        assert_eq!(status(&s), SpecStatus::AwaitingValidation);
    }

    #[test]
    fn the_final_approval_needs_everything_else() {
        let mut s = all_approved();

        assert_eq!(
            final_approve(&mut s, Vec::new(), 100),
            Err(TransitionError::NotReady)
        );

        make_ready(&mut s);

        assert!(readiness(&s).ready);
        assert_eq!(status(&s), SpecStatus::ReadyForImplementation);
    }

    #[test]
    fn changing_any_document_voids_the_final_approval_the_review_and_the_checks() {
        let mut s = all_approved();
        make_ready(&mut s);
        decide(
            &mut s,
            "tasks",
            1,
            DecisionKind::Revise,
            Some("split task 2"),
            200,
        )
        .unwrap();
        begin_generation(&mut s, "tasks", "r-t", "a1", 201).unwrap();
        complete_generation(&mut s, "tasks", "r-t", file("tasks v2"), 202).unwrap();

        let r = readiness(&s);

        assert!(!r.ready);
        let codes: Vec<&str> = r.blockers.iter().map(|b| b.code.as_str()).collect();
        assert!(codes.contains(&"artifact_not_approved"));
        assert!(codes.contains(&"consistency_not_run"));
        assert!(codes.contains(&"review_outdated"));
        assert!(codes.contains(&"final_approval_missing"));
        assert_ne!(status(&s), SpecStatus::ReadyForImplementation);
    }

    fn finding(id: &str, severity: FindingSeverity, question: Option<&str>) -> Finding {
        Finding {
            id: id.to_owned(),
            category: "ambiguity".to_owned(),
            severity,
            title: id.to_owned(),
            description: "d".to_owned(),
            artifact_id: Some("specs".to_owned()),
            reference: String::new(),
            evidence: String::new(),
            rationale: String::new(),
            question: question.map(str::to_owned),
            suggestion: None,
            artifact_revision: Some(1),
            status: FindingStatus::Open,
            resolution_note: None,
            review_id: "rev-1".to_owned(),
            created_at: 1,
            resolved_at: None,
        }
    }

    #[test]
    fn serious_findings_and_questions_block_until_the_person_decides() {
        let mut s = all_approved();
        let current = s.current_revisions();
        record_consistency(
            &mut s,
            ConsistencyValidation {
                revisions: current,
                validator: "t/1".to_owned(),
                issues: Vec::new(),
                at: 1,
            },
            100,
        )
        .unwrap();
        begin_review(&mut s, "rev-1", "run-review", "a2", 101).unwrap();
        complete_review(
            &mut s,
            "run-review",
            vec![
                finding("high", FindingSeverity::High, None),
                finding("ask", FindingSeverity::Low, Some("Which currency?")),
                finding("minor", FindingSeverity::Low, None),
            ],
            "found some",
            102,
        )
        .unwrap();

        assert_eq!(
            final_approve(&mut s, Vec::new(), 103),
            Err(TransitionError::NotReady)
        );
        let blockers = readiness(&s).blockers;
        assert!(blockers
            .iter()
            .any(|b| b.code == "findings_open" && b.count == Some(2)));
        assert_eq!(status(&s), SpecStatus::Blocked);

        // Accepting a risk needs the person's words.
        assert_eq!(
            resolve_finding(&mut s, "high", FindingStatus::AcceptedRisk, None, 104),
            Err(TransitionError::NoteRequired)
        );
        resolve_finding(
            &mut s,
            "high",
            FindingStatus::AcceptedRisk,
            Some("Handled by ops"),
            105,
        )
        .unwrap();
        resolve_finding(&mut s, "ask", FindingStatus::Resolved, Some("EUR"), 106).unwrap();

        final_approve(&mut s, Vec::new(), 107).unwrap();
        assert!(readiness(&s).ready);
    }

    #[test]
    fn a_finding_found_again_stays_one_finding_with_the_persons_decision() {
        let mut s = all_approved();
        let current = s.current_revisions();
        record_consistency(
            &mut s,
            ConsistencyValidation {
                revisions: current,
                validator: "t/1".to_owned(),
                issues: Vec::new(),
                at: 1,
            },
            100,
        )
        .unwrap();
        begin_review(&mut s, "rev-1", "run-1", "a2", 101).unwrap();
        complete_review(
            &mut s,
            "run-1",
            vec![finding("f", FindingSeverity::High, None)],
            "",
            102,
        )
        .unwrap();
        resolve_finding(
            &mut s,
            "f",
            FindingStatus::Dismissed,
            Some("not a problem"),
            103,
        )
        .unwrap();

        begin_review(&mut s, "rev-2", "run-2", "a2", 104).unwrap();
        complete_review(
            &mut s,
            "run-2",
            vec![finding("f", FindingSeverity::High, None)],
            "",
            105,
        )
        .unwrap();

        assert_eq!(s.findings.len(), 1);
        assert_eq!(s.findings[0].status, FindingStatus::Dismissed);
    }

    #[test]
    fn a_finding_the_documents_have_moved_past_is_resolved_when_not_reported_again() {
        let mut s = all_approved();
        let current = s.current_revisions();
        record_consistency(
            &mut s,
            ConsistencyValidation {
                revisions: current,
                validator: "t/1".to_owned(),
                issues: Vec::new(),
                at: 1,
            },
            100,
        )
        .unwrap();
        begin_review(&mut s, "rev-1", "run-1", "a2", 101).unwrap();
        complete_review(
            &mut s,
            "run-1",
            vec![finding("f", FindingSeverity::High, None)],
            "",
            102,
        )
        .unwrap();
        // The specs are revised (revision 2) and approved again.
        decide(&mut s, "specs", 1, DecisionKind::Revise, Some("fix f"), 110).unwrap();
        begin_generation(&mut s, "specs", "rs", "a1", 111).unwrap();
        complete_generation(&mut s, "specs", "rs", file("fixed"), 112).unwrap();
        record_validation(&mut s, "specs", valid("specs", 2), 113).unwrap();
        decide(&mut s, "specs", 2, DecisionKind::Approve, None, 114).unwrap();
        for (i, id) in ["design", "tasks"].iter().enumerate() {
            let n = s.artifact(id).unwrap().latest_number().unwrap();
            keep_outdated(&mut s, id, n, 115 + i as u64).unwrap();
            record_validation(&mut s, id, valid(id, n), 120 + i as u64).unwrap();
            decide(&mut s, id, n, DecisionKind::Approve, None, 125 + i as u64).unwrap();
        }

        begin_review(&mut s, "rev-2", "run-2", "a2", 130).unwrap();
        complete_review(&mut s, "run-2", Vec::new(), "clean", 131).unwrap();

        assert_eq!(s.findings[0].status, FindingStatus::Resolved);
    }

    #[test]
    fn the_review_can_be_waived_only_in_writing_and_is_voided_by_a_change() {
        let mut s = all_approved();

        assert_eq!(
            waive_review(&mut s, None, 100),
            Err(TransitionError::NoteRequired)
        );
        waive_review(&mut s, Some("No time for it"), 101).unwrap();

        assert!(readiness(&s)
            .blockers
            .iter()
            .all(|b| b.code != "review_not_run" && b.code != "review_outdated"));
    }

    #[test]
    fn a_review_needs_every_document_approved() {
        let mut s = spec();
        start(&mut s, 2).unwrap();
        write_and_approve(&mut s, "proposal", 10);

        assert_eq!(
            begin_review(&mut s, "rev", "run", "a2", 20),
            Err(TransitionError::WrongState("artifacts_not_approved"))
        );
    }

    #[test]
    fn a_publication_left_half_way_is_picked_up_again_not_repeated() {
        let mut s = all_approved();
        make_ready(&mut s);
        begin_publication(&mut s, "add-dark-mode", 200).unwrap();
        publication_progress(&mut s, "proposal.md", 201);
        fail_publication(&mut s, "disk full", 202);
        assert!(!readiness(&s).ready, "a failed publication is not coherent");

        begin_publication(&mut s, "add-dark-mode", 203).unwrap();
        assert!(matches!(
            &s.publication,
            Publication::InProgress { written, .. } if written == &vec!["proposal.md".to_owned()]
        ));
        complete_publication(
            &mut s,
            "openspec/changes/add-dark-mode",
            vec!["proposal.md".to_owned()],
            204,
        );

        assert!(readiness(&s).ready);
        assert_eq!(status(&s), SpecStatus::Published);
    }

    #[test]
    fn nothing_is_published_before_the_spec_is_ready() {
        let mut s = all_approved();

        assert_eq!(
            begin_publication(&mut s, "add-dark-mode", 100),
            Err(TransitionError::NotReady)
        );
        assert_eq!(s.publication, Publication::NotPublished);
    }

    #[test]
    fn a_change_after_publishing_makes_the_publication_outdated() {
        let mut s = all_approved();
        make_ready(&mut s);
        begin_publication(&mut s, "add-dark-mode", 200).unwrap();
        complete_publication(&mut s, "openspec/changes/add-dark-mode", vec![], 201);
        decide(&mut s, "tasks", 1, DecisionKind::Redo, None, 210).unwrap();
        begin_generation(&mut s, "tasks", "rt", "a1", 211).unwrap();
        complete_generation(&mut s, "tasks", "rt", file("t2"), 212).unwrap();

        assert!(readiness(&s)
            .blockers
            .iter()
            .any(|b| b.code == "publication_outdated"));
    }

    #[test]
    fn only_a_ready_spec_can_be_given_to_an_implementation_and_only_a_published_one_archived() {
        let mut s = all_approved();
        let link = ImplementationLink {
            workflow_execution_id: "wfx-1".to_owned(),
            workflow_id: "wf-1".to_owned(),
            workflow_name: "Feature".to_owned(),
            revisions: s.current_revisions(),
            started_at: 300,
        };
        assert_eq!(
            link_implementation(&mut s, link.clone(), 300),
            Err(TransitionError::NotReady)
        );
        make_ready(&mut s);
        link_implementation(&mut s, link, 301).unwrap();
        assert_eq!(s.implementations.len(), 1);

        assert_eq!(
            archive(&mut s, None, "atlas", 302),
            Err(TransitionError::WrongState("not_published"))
        );
        begin_publication(&mut s, "add-dark-mode", 303).unwrap();
        complete_publication(&mut s, "openspec/changes/add-dark-mode", vec![], 304);
        archive(&mut s, None, "atlas", 305).unwrap();

        assert_eq!(status(&s), SpecStatus::Archived);
        // And nothing moves any more.
        assert_eq!(
            decide(&mut s, "proposal", 1, DecisionKind::Redo, None, 306),
            Err(TransitionError::Archived)
        );
    }

    #[test]
    fn the_same_problem_gets_the_same_id_however_it_is_worded() {
        let a = finding_id(
            "Ambiguity",
            Some("specs"),
            "Requirement:  Login ",
            "Unclear   timeout",
        );
        let b = finding_id(
            "ambiguity",
            Some("specs"),
            "requirement: login",
            "unclear timeout",
        );
        let c = finding_id(
            "ambiguity",
            Some("design"),
            "requirement: login",
            "unclear timeout",
        );

        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn the_version_grows_with_every_change() {
        let mut s = spec();
        let v0 = s.version;
        start(&mut s, 2).unwrap();
        begin_generation(&mut s, "proposal", "r1", "a1", 3).unwrap();

        assert!(s.version > v0 + 1);
        assert_eq!(s.updated_at, 3);
    }

    #[test]
    fn a_cancelled_run_leaves_the_artifact_idle_not_failed() {
        let mut s = spec();
        start(&mut s, 2).unwrap();
        begin_generation(&mut s, "proposal", "r1", "a1", 3).unwrap();

        fail_generation(
            &mut s,
            "proposal",
            "r1",
            "cancelled",
            RunOutcome::Cancelled,
            4,
        )
        .unwrap();

        assert_eq!(s.artifact("proposal").unwrap().generation, Generation::Idle);
        assert_eq!(status(&s), SpecStatus::AwaitingGeneration);
    }
}
