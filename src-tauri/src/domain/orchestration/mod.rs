//! What the steps of a workflow run share: structured results, artifacts, decisions and state.

mod artifact;
mod decision;
mod result;
mod state;

pub use artifact::{Artifact, ArtifactType};
pub use decision::Decision;
pub use result::{
    AgentResult, ArtifactDraft, DecisionDraft, Finding, ResultStatus, ValidationSummary,
};
pub use state::{OverlapWarning, SharedExecutionState, ValidationEntry};
