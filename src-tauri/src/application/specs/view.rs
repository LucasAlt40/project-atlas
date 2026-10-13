//! What the screens are given of a spec: the record itself, and what follows from it that the
//! screens must not work out for themselves (the status, what each document is up to, what keeps
//! the spec from being ready). One place decides these, so the list, the detail and the workflow
//! start never disagree.

use std::collections::BTreeMap;

use serde::Serialize;

use super::framework::{FileOutline, SpecFramework, TaskItem};
use crate::domain::spec::rules::{self, ArtifactPhase, Readiness};
use crate::domain::spec::{
    FindingStatus, Publication, QuestionStatus, ReviewRun, Spec, SpecStatus,
};
use crate::domain::usage::{UsageRecord, UsageSource};

#[allow(clippy::struct_excessive_bools)] // what the screen shows, one fact per field
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecView {
    pub spec: Spec,
    pub status: SpecStatus,
    pub phases: BTreeMap<String, ArtifactPhase>,
    pub readiness: Readiness,
    pub current_revisions: BTreeMap<String, u32>,
    /// The documents that could be written now.
    pub ready_to_generate: Vec<String>,
    /// The review, the checks and the person's approval each read these very revisions.
    pub review_current: bool,
    pub consistency_current: bool,
    pub final_approval_current: bool,
    pub publication_current: bool,
    /// An agent is working on the spec now.
    pub busy: bool,
    pub pending_questions: u32,
    pub open_findings: u32,
    /// The tasks of the latest task list, as the document says them.
    pub tasks: Vec<TaskItem>,
    /// The spec files of the latest `specs` revision, laid out for reading.
    pub outline: Vec<FileOutline>,
    /// The project and the two agents are still there.
    pub project_available: bool,
    pub author_available: bool,
    pub reviewer_available: bool,
    /// What the agents' runs for this spec used, as the runtimes reported it.
    pub usage: SpecUsage,
    pub budget: SpecBudget,
}

/// The token budget of the spec (`optimization.specs.maxTokens`) and where it stands.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecBudget {
    /// What the runs may use together, now: the configured budget, once more for every time the
    /// person said to go on. `None`: no limit.
    pub limit: Option<u64>,
    /// What the runs used so far, as the runtimes reported it; `None` while nothing was.
    pub spent: Option<u64>,
    pub steps: u32,
    /// No new run starts until the person says to go on.
    pub reached: bool,
    /// More than four fifths of it is used.
    pub near: bool,
}

impl SpecBudget {
    pub fn of(spec: &Spec, usage: &SpecUsage, configured: Option<u64>) -> Self {
        let limit = configured.map(|max| max.saturating_mul(u64::from(spec.budget_steps) + 1));
        let spent = usage.total_tokens();
        let (reached, near) = match (limit, spent) {
            (Some(limit), Some(spent)) => (
                spent >= limit,
                spent.saturating_mul(5) >= limit.saturating_mul(4),
            ),
            _ => (false, false),
        };
        Self {
            limit,
            spent,
            steps: spec.budget_steps,
            reached,
            near,
        }
    }
}

/// What one run of an agent for the spec used. A field is `None` when the runtime did not say:
/// that is not zero.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunUsage {
    pub run_id: String,
    pub runtime_id: String,
    pub model_id: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
    pub cost: Option<f64>,
    pub currency: Option<String>,
    /// `runtime_reported`, or `atlas_calculated` when Atlas worked it out itself.
    pub source: Option<UsageSource>,
}

/// The sum over the runs that reported something, and how many that was out of how many runs.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecUsage {
    pub runs: Vec<RunUsage>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
    pub cost: Option<f64>,
    pub currency: Option<String>,
    /// Runs the spec made, and how many of them have a usage record with numbers in it.
    pub runs_total: u32,
    pub runs_measured: u32,
}

impl SpecUsage {
    /// Every token the spec has used so far that a runtime reported (input and output), for the
    /// budget. `None` while nothing was reported.
    pub fn total_tokens(&self) -> Option<u64> {
        match (self.input_tokens, self.output_tokens) {
            (None, None) => None,
            (a, b) => Some(a.unwrap_or(0) + b.unwrap_or(0)),
        }
    }
}

fn add(total: &mut Option<u64>, value: Option<u64>) {
    if let Some(value) = value {
        *total = Some(total.unwrap_or(0) + value);
    }
}

/// The usage of the spec's runs, from the ledger Atlas keeps for every execution it observed.
pub fn usage_of(spec: &Spec, records: &[UsageRecord]) -> SpecUsage {
    let mut usage = SpecUsage {
        runs_total: count(spec.runs.len()),
        ..SpecUsage::default()
    };
    let mut currency_conflict = false;
    for run in &spec.runs {
        let Some(record) = records.iter().rev().find(|r| r.execution_id == run.id) else {
            continue;
        };
        let metrics = record.metrics.as_ref();
        usage.runs.push(RunUsage {
            run_id: run.id.clone(),
            runtime_id: record.runtime_id.clone(),
            model_id: record.model_id.clone(),
            input_tokens: metrics.and_then(|m| m.input_tokens),
            output_tokens: metrics.and_then(|m| m.output_tokens),
            cached_input_tokens: metrics.and_then(|m| m.cached_input_tokens),
            cost: metrics.and_then(|m| m.cost),
            currency: metrics.and_then(|m| m.currency.clone()),
            source: metrics.map(|m| m.source),
        });
        let Some(metrics) = metrics.filter(|m| m.has_tokens()) else {
            continue;
        };
        usage.runs_measured += 1;
        add(&mut usage.input_tokens, metrics.input_tokens);
        add(&mut usage.output_tokens, metrics.output_tokens);
        add(&mut usage.cached_input_tokens, metrics.cached_input_tokens);
        if let Some(cost) = metrics.cost {
            usage.cost = Some(usage.cost.unwrap_or(0.0) + cost);
            match &usage.currency {
                None => usage.currency.clone_from(&metrics.currency),
                Some(seen) if Some(seen) != metrics.currency.as_ref() => currency_conflict = true,
                Some(_) => {}
            }
        }
    }
    if currency_conflict {
        // Amounts in different currencies are not added: no sum is better than a wrong one.
        usage.cost = None;
        usage.currency = None;
    }
    usage
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecSummary {
    pub id: String,
    pub title: String,
    /// The start of what the person asked for.
    pub request: String,
    pub workspace_id: String,
    pub project_id: String,
    pub change_id: String,
    pub status: SpecStatus,
    /// The documents approved so far, out of `stage_total`.
    pub approved: u32,
    pub stage_total: u32,
    /// The first document that is not approved yet; none when all are.
    pub current_artifact: Option<String>,
    pub pending_questions: u32,
    pub open_findings: u32,
    pub author_agent_id: String,
    pub implementations: u32,
    pub imported: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

pub struct Availability {
    pub project: bool,
    pub author: bool,
    pub reviewer: bool,
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

pub fn build(
    spec: &Spec,
    framework: &dyn SpecFramework,
    available: &Availability,
    records: &[UsageRecord],
    budget: Option<u64>,
) -> SpecView {
    let current = spec.current_revisions();
    let phases = spec
        .artifacts
        .iter()
        .map(|a| (a.id.clone(), rules::artifact_phase(spec, a)))
        .collect();
    let tasks = spec
        .artifact("tasks")
        .and_then(|a| {
            a.approved_revision
                .and_then(|n| a.revision(n))
                .or_else(|| a.latest())
        })
        .map(|r| framework.tasks(&r.files))
        .unwrap_or_default();
    let review_current = spec.ai_review.as_ref().is_some_and(|r| {
        r.revisions == current
            && matches!(
                r.run,
                ReviewRun::Completed { .. } | ReviewRun::Waived { .. }
            )
    });
    let consistency_current = spec
        .consistency
        .as_ref()
        .is_some_and(|c| c.revisions == current);
    let final_approval_current = spec
        .final_approval
        .as_ref()
        .is_some_and(|a| a.revisions == current);
    let publication_current = matches!(
        &spec.publication,
        Publication::Published { revisions, .. } if *revisions == current
    );
    let outline = spec
        .artifact("specs")
        .and_then(|a| a.latest())
        .map(|r| framework.outline(&r.files))
        .unwrap_or_default();
    let usage = usage_of(spec, records);
    SpecView {
        budget: SpecBudget::of(spec, &usage, budget),
        outline,
        status: rules::status(spec),
        phases,
        readiness: rules::readiness(spec),
        ready_to_generate: rules::ready_to_generate(spec),
        review_current,
        consistency_current,
        final_approval_current,
        publication_current,
        busy: rules::has_active_run(spec),
        pending_questions: count(
            spec.questions
                .iter()
                .filter(|q| q.status == QuestionStatus::Pending)
                .count(),
        ),
        open_findings: count(
            spec.findings
                .iter()
                .filter(|f| f.status == FindingStatus::Open)
                .count(),
        ),
        tasks,
        project_available: available.project,
        author_available: available.author,
        reviewer_available: available.reviewer,
        usage,
        current_revisions: current,
        spec: spec.clone(),
    }
}

pub fn summarize(spec: &Spec) -> SpecSummary {
    let approved = spec.artifacts.iter().filter(|a| a.is_approved()).count();
    let current = spec
        .artifacts
        .iter()
        .find(|a| !a.is_approved())
        .map(|a| a.id.clone());
    let request: String = spec.request.trim().chars().take(240).collect();
    SpecSummary {
        id: spec.id.clone(),
        title: spec.title.clone(),
        request,
        workspace_id: spec.workspace_id.clone(),
        project_id: spec.project_id.clone(),
        change_id: spec.change_id.clone(),
        status: rules::status(spec),
        approved: count(approved),
        stage_total: count(spec.artifacts.len()),
        current_artifact: current,
        pending_questions: count(
            spec.questions
                .iter()
                .filter(|q| q.status == QuestionStatus::Pending)
                .count(),
        ),
        open_findings: count(
            spec.findings
                .iter()
                .filter(|f| f.status == FindingStatus::Open)
                .count(),
        ),
        author_agent_id: spec.author_agent_id.clone(),
        implementations: count(spec.implementations.len()),
        imported: matches!(
            spec.origin,
            crate::domain::spec::SpecOrigin::Imported { .. }
        ),
        created_at: spec.created_at,
        updated_at: spec.updated_at,
    }
}

/// A change already in the project that could become a spec.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCandidate {
    pub change_id: String,
    /// Which documents it has.
    pub artifacts: Vec<String>,
    /// What it lacks or has wrong, as codes: `missing:design`, `unrecognized:notes.txt`…
    pub problems: Vec<String>,
    /// The spec that already is this change, if any.
    pub imported_as: Option<String>,
}
