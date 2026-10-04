//! `.atlas/project.yaml`: reading, validating and building the manifest, and applying what the
//! user decided in review to the findings.

use std::collections::BTreeMap;

use super::fingerprint::{drift, Drift};
use super::knowledge::{self, stamp_verification};
use super::negative;
use crate::application::errors::{AppError, ErrorCode};
use crate::domain::harness::{
    Finding, FindingCategory, HarnessKnowledge, HarnessManifest, HarnessStatus, HarnessSummary,
    ManifestContext, ManifestHarness, ManifestProject, ManifestRepository, ManifestStack,
    ProjectFingerprint, HARNESS_VERSION, KNOWLEDGE_VERSION,
};

/// A manifest bigger than this is not a manifest.
pub const MAX_MANIFEST_BYTES: usize = 256 * 1024;

/// A knowledge file bigger than this is not one.
pub const MAX_KNOWLEDGE_BYTES: usize = 1024 * 1024;

const KNOWLEDGE_HEADER: &str =
    "# Atlas knowledge: what the analysis found, with its evidence. Generated;\n\
# replaced on update after you review the diff. Your own words live in context/.\n";

const HEADER: &str = "# Atlas Project Harness manifest. Structural metadata only; what people\n\
# should read lives in context/. Safe to commit.\n";

/// # Errors
///
/// `HarnessInvalid` if the text is not a manifest this version of Atlas understands.
pub fn parse_manifest(text: &str) -> Result<HarnessManifest, AppError> {
    if text.len() > MAX_MANIFEST_BYTES {
        return Err(
            AppError::new(ErrorCode::HarnessInvalid).with_detail("project.yaml is too large")
        );
    }
    let manifest: HarnessManifest = serde_norway::from_str(text)
        .map_err(|e| AppError::new(ErrorCode::HarnessInvalid).with_detail(e.to_string()))?;
    if manifest.version != HARNESS_VERSION || manifest.harness.version != HARNESS_VERSION {
        return Err(AppError::new(ErrorCode::HarnessInvalid)
            .with("version", manifest.version.to_string())
            .with_detail(format!("unsupported manifest version {}", manifest.version)));
    }
    Ok(manifest)
}

/// # Errors
///
/// `HarnessGenerationFailed` if the manifest cannot be serialised.
pub fn render_manifest(manifest: &HarnessManifest) -> Result<String, AppError> {
    let body = serde_norway::to_string(manifest).map_err(|e| {
        AppError::new(ErrorCode::HarnessGenerationFailed).with_detail(e.to_string())
    })?;
    Ok(format!("{HEADER}{body}"))
}

/// # Errors
///
/// `HarnessInvalid` if the text is not knowledge this version of Atlas understands.
pub fn parse_knowledge(text: &str) -> Result<HarnessKnowledge, AppError> {
    if text.len() > MAX_KNOWLEDGE_BYTES {
        return Err(AppError::new(ErrorCode::HarnessInvalid).with_detail("knowledge is too large"));
    }
    let mut knowledge: HarnessKnowledge = serde_norway::from_str(text)
        .map_err(|e| AppError::new(ErrorCode::HarnessInvalid).with_detail(e.to_string()))?;
    // Facts written before V0.7.2 carry no verification: they were read from the repository.
    stamp_verification(&mut knowledge.findings, knowledge.analysis.analyzed_at);
    if knowledge.version != KNOWLEDGE_VERSION {
        return Err(
            AppError::new(ErrorCode::HarnessInvalid).with_detail(format!(
                "unsupported knowledge version {}",
                knowledge.version
            )),
        );
    }
    Ok(knowledge)
}

/// # Errors
///
/// `HarnessGenerationFailed` if the knowledge cannot be serialised.
pub fn render_knowledge(knowledge: &HarnessKnowledge) -> Result<String, AppError> {
    let body = serde_norway::to_string(knowledge).map_err(|e| {
        AppError::new(ErrorCode::HarnessGenerationFailed).with_detail(e.to_string())
    })?;
    Ok(format!("{KNOWLEDGE_HEADER}{body}"))
}

/// What a manifest (and its knowledge), or their absence, say about the project's Harness.
/// `current` is the project's fingerprint now, if it could be read: with it the summary can say
/// whether the Harness is stale.
pub fn summarize(
    manifest: Option<&str>,
    knowledge_text: Option<&str>,
    has_atlas_dir: bool,
    current: Option<&ProjectFingerprint>,
) -> HarnessSummary {
    let Some(text) = manifest else {
        return HarnessSummary::not_initialized(has_atlas_dir);
    };
    match parse_manifest(text) {
        Ok(m) => {
            // Knowledge that cannot be read counts as missing: the Harness still works, but
            // is marked as needing a look.
            let parsed = knowledge_text.and_then(|t| parse_knowledge(t).ok());
            let state = parsed
                .as_ref()
                .map_or(Drift::Unchecked, |k| drift(&k.analysis, current));
            let (summary_stats, staleness) = parsed.as_ref().map_or((None, None), |k| {
                let effective = knowledge::effective_findings(k, &m, &state);
                let gaps = negative::gaps(&effective, &k.analysis);
                let staleness = match &state {
                    Drift::Changed(s) => Some(s.clone()),
                    _ => None,
                };
                (Some(knowledge::stats(&effective, &gaps)), staleness)
            });
            HarnessSummary {
                analyzed_at: parsed.as_ref().map(|k| k.analysis.analyzed_at),
                staleness,
                stats: summary_stats,
                status: HarnessStatus::Initialized,
                project_name: Some(m.project.name.clone()),
                stack: stack_labels(&m.stack),
                version: Some(m.version),
                initialized_at: Some(m.project.initialized_at),
                has_atlas_dir,
                problem: None,
                health: Some(knowledge::health(parsed.as_ref(), &m, &state)),
            }
        }
        Err(_) => HarnessSummary {
            status: HarnessStatus::NeedsReview,
            has_atlas_dir,
            problem: Some("harness_invalid".to_owned()),
            ..HarnessSummary::not_initialized(has_atlas_dir)
        },
    }
}

pub fn stack_labels(stack: &ManifestStack) -> Vec<String> {
    [
        &stack.languages,
        &stack.frameworks,
        &stack.runtimes,
        &stack.databases,
        &stack.infrastructure,
    ]
    .into_iter()
    .flatten()
    .cloned()
    .collect()
}

/// The manifest for the findings as the user left them.
pub fn build_manifest(
    project_id: String,
    name: &str,
    initialized_at: u64,
    findings: &[Finding],
    corrections: BTreeMap<String, String>,
    excluded: Vec<String>,
    confirmed: Vec<String>,
) -> HarnessManifest {
    let labels = |category: FindingCategory| -> Vec<String> {
        findings
            .iter()
            .filter(|f| f.category == category)
            .map(|f| f.label.clone())
            .collect()
    };
    let is_git = findings.iter().any(|f| f.id == "repository:git");
    HarnessManifest {
        version: HARNESS_VERSION,
        project: ManifestProject {
            id: project_id,
            name: name.to_owned(),
            initialized_at,
        },
        repository: ManifestRepository {
            kind: if is_git { "git" } else { "none" }.to_owned(),
            root: ".".to_owned(),
        },
        stack: ManifestStack {
            languages: labels(FindingCategory::Language),
            frameworks: labels(FindingCategory::Framework),
            runtimes: labels(FindingCategory::Runtime),
            databases: labels(FindingCategory::Database),
            infrastructure: labels(FindingCategory::Infrastructure),
        },
        context: ManifestContext { generated: true },
        harness: ManifestHarness {
            version: HARNESS_VERSION,
        },
        corrections,
        excluded,
        confirmed,
    }
}
