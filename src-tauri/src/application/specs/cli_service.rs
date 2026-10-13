//! What the person can ask of the `OpenSpec` command about a spec (ADR 0035): a second look at the
//! published change, and the archive. Both only on the person's explicit say-so, both only for a
//! spec whose files are in the project as Atlas last wrote them, and both leave Atlas working the
//! same when the command is missing.

use std::sync::Arc;

use super::cli::{CliStatus, OpenSpecCli};
use super::ports::ProjectFiles;
use super::service::SpecService;
use super::view::SpecView;
use crate::application::errors::{AppError, ErrorCode};
use crate::application::support::now_ms;
use crate::application::workflow::service::WorkflowService;
use crate::domain::spec::rules;
use crate::domain::spec::{CliCheck, Publication, Spec};

pub struct SpecCliService {
    specs: Arc<SpecService>,
    workflows: Arc<WorkflowService>,
    cli: OpenSpecCli,
    files: Arc<dyn ProjectFiles>,
}

fn state(reason: &str) -> AppError {
    AppError::new(ErrorCode::SpecStateInvalid).with("reason", reason)
}

impl SpecCliService {
    pub fn new(
        specs: Arc<SpecService>,
        workflows: Arc<WorkflowService>,
        cli: OpenSpecCli,
        files: Arc<dyn ProjectFiles>,
    ) -> Self {
        Self {
            specs,
            workflows,
            cli,
            files,
        }
    }

    fn load(&self, spec_id: &str) -> Result<(Spec, String), AppError> {
        let spec = self
            .specs
            .get(spec_id)
            .ok_or_else(|| AppError::new(ErrorCode::SpecNotFound))?;
        let root = self.specs.project_root(&spec).ok_or_else(|| {
            AppError::new(ErrorCode::SpecInvalid).with("reason", "project_invalid")
        })?;
        Ok((spec, root))
    }

    /// Whether the command is installed and usable, for the spec's project.
    ///
    /// # Errors
    ///
    /// When there is no such spec or its project is gone.
    pub fn status(&self, spec_id: &str) -> Result<CliStatus, AppError> {
        let (_, root) = self.load(spec_id)?;
        Ok(self.cli.status(&root))
    }

    /// The published change must be in the project as Atlas wrote it: that is what is looked at.
    fn published(spec: &Spec) -> Result<String, AppError> {
        match &spec.publication {
            Publication::Published {
                change_id,
                revisions,
                ..
            } if *revisions == spec.current_revisions() => Ok(change_id.clone()),
            Publication::Published { .. } => Err(state("publication_outdated")),
            _ => Err(state("not_published")),
        }
    }

    /// Asks the command to validate the published change. Read-only; kept as a second opinion.
    ///
    /// # Errors
    ///
    /// When the spec is not published as it stands, or the command is not usable.
    pub fn check(&self, spec_id: &str) -> Result<SpecView, AppError> {
        let (spec, root) = self.load(spec_id)?;
        let change_id = Self::published(&spec)?;
        let result = self.cli.validate(&root, &change_id)?;
        self.specs.record_cli_check(
            spec_id,
            CliCheck {
                revisions: spec.current_revisions(),
                cli_version: result.version,
                valid: result.valid,
                issues: result.issues,
                at: now_ms(),
            },
        )?;
        self.specs.view(spec_id)
    }

    /// Whether anything built from the spec is still going, or the last build did not end well.
    fn implementation_settled(&self, spec: &Spec) -> Result<(), AppError> {
        let mut last = None;
        for link in &spec.implementations {
            let Some(run) = self.workflows.execution(&link.workflow_execution_id) else {
                continue;
            };
            if run.status.is_active() {
                return Err(state("implementation_running"));
            }
            last = Some(run.status);
        }
        match last {
            Some(crate::domain::workflow::WorkflowExecutionStatus::Completed) | None => Ok(()),
            Some(_) => Err(state("implementation_not_completed")),
        }
    }

    /// Has the command merge the change into the project's specs and move it to the archive. Only
    /// a spec that is ready, published as it stands and not being built; the command validates
    /// first and refuses what is invalid. Atlas keeps the spec and everything run from it.
    ///
    /// # Errors
    ///
    /// Why it cannot, or what the command said when it refused.
    pub fn archive(&self, spec_id: &str) -> Result<SpecView, AppError> {
        let (spec, root) = self.load(spec_id)?;
        if spec.archived.is_some() {
            return Err(state("archived"));
        }
        if !rules::readiness(&spec).ready {
            return Err(AppError::new(ErrorCode::SpecNotReady));
        }
        let change_id = Self::published(&spec)?;
        self.implementation_settled(&spec)?;
        let result = self.cli.archive(&root, &change_id)?;
        // Said only if it can be seen: the folder is where the command said it put it.
        let path = result.archived_as.and_then(|name| {
            self.files
                .change_exists(&root, "openspec/changes/archive", &name)
                .then(|| format!("openspec/changes/archive/{name}"))
        });
        self.specs.archive_record(spec_id, path, "cli")
    }
}
