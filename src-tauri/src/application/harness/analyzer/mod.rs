//! Project analysis: structured, evidenced findings.
//!
//! ```text
//! ProjectAnalyzer
//! ├── DeterministicAnalyzer   snapshot -> findings, no model, no execution
//! └── SemanticAnalyzer        selected evidence -> model -> validated findings (see `semantic`)
//! ```

mod build;
mod ci;
mod dependencies;
mod entry_points;
mod git;
mod npm;
mod stack;
mod structure;
mod testing;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;

use super::sampler::EvidenceBundle;
use super::snapshot::ScanSnapshot;
use crate::application::errors::AppError;
use crate::domain::harness::Finding;

/// Everything an analyzer may look at. The semantic analyzer gets the sampled evidence and what
/// the deterministic analysis already found; it gets no path and no filesystem.
pub struct AnalysisInput<'a> {
    pub snapshot: &'a ScanSnapshot,
    pub evidence: Option<&'a EvidenceBundle>,
    pub deterministic: &'a [Finding],
    /// Whether a project-relative path exists (safely: no links, no escaping the root). Lets a
    /// model that explored the project cite what it read.
    pub exists: Option<&'a dyn Fn(&str) -> bool>,
}

/// What an analysis step produced.
#[derive(Debug, Default)]
pub struct AnalysisOutput {
    pub findings: Vec<Finding>,
    /// Statements dropped for lacking valid evidence (semantic analysis only).
    pub rejected: usize,
    /// Business fields a model drafted from the documentation. Suggestions, not knowledge.
    pub suggestions: Option<crate::domain::harness::UserKnowledge>,
}

/// An analysis step: input in, findings out.
pub trait ProjectAnalyzer: Send + Sync {
    /// # Errors
    ///
    /// An analyzer that depends on something fallible (a model) reports why it could not run.
    fn analyze_project(&self, input: &AnalysisInput<'_>) -> Result<AnalysisOutput, AppError>;
}

/// One rule-based look at a snapshot (Git, stack, structure, CI…).
pub trait SnapshotAnalyzer: Send + Sync {
    fn analyze(&self, snapshot: &ScanSnapshot) -> Vec<Finding>;
}

/// Runs the Git, stack, structure and CI analyzers and merges their findings.
pub struct DeterministicAnalyzer {
    analyzers: Vec<Box<dyn SnapshotAnalyzer>>,
}

impl Default for DeterministicAnalyzer {
    fn default() -> Self {
        Self {
            analyzers: vec![
                Box::new(git::GitAnalyzer),
                Box::new(stack::StackAnalyzer),
                Box::new(structure::StructureAnalyzer),
                Box::new(ci::CiAnalyzer),
                Box::new(build::BuildAnalyzer),
                Box::new(entry_points::EntryPointAnalyzer),
                Box::new(dependencies::DependencyAnalyzer),
                Box::new(testing::TestingAnalyzer),
            ],
        }
    }
}

impl ProjectAnalyzer for DeterministicAnalyzer {
    fn analyze_project(&self, input: &AnalysisInput<'_>) -> Result<AnalysisOutput, AppError> {
        Ok(AnalysisOutput {
            findings: self.analyze(input.snapshot),
            rejected: 0,
            suggestions: None,
        })
    }
}

impl DeterministicAnalyzer {
    pub fn analyze(&self, snapshot: &ScanSnapshot) -> Vec<Finding> {
        let mut seen = BTreeSet::new();
        let mut findings: Vec<Finding> = self
            .analyzers
            .iter()
            .flat_map(|a| a.analyze(snapshot))
            // The first analyzer to report an id wins; the others cannot contradict it.
            .filter(|f| seen.insert(f.id.clone()))
            .collect();
        findings.sort_by(|a, b| (a.category, &a.key).cmp(&(b.category, &b.key)));
        findings
    }
}
