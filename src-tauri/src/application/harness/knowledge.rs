//! The Harness knowledge model, as pure functions: the user's choices applied to findings,
//! the health of a Harness, the diff between two analyses, and the user's own files.
//!
//! Precedence, strongest first: what the user said, deterministic facts, semantic inferences.
//! None of it outranks Atlas's security policy, which is not knowledge at all.

use std::collections::BTreeMap;

use super::fingerprint::{touches, Drift};
use crate::domain::harness::{
    label_for, Confidence, Evidence, Finding, FindingCategory, FindingChange, Gap, HarnessDiff,
    HarnessHealth, HarnessKnowledge, HarnessManifest, HealthState, KnowledgeStats, Origin,
    Staleness, UserKnowledge, Verification, VerificationMethod, VerificationStatus,
};

/// Applies the user's review to raw findings: unticked ones go, corrections replace the value
/// (and become the user's word), confirmations promote an inference to user-confirmed.
pub fn apply_choices(
    findings: &[Finding],
    excluded: &[String],
    corrections: &BTreeMap<String, String>,
    confirmed: &[String],
) -> Vec<Finding> {
    findings
        .iter()
        .filter(|f| !excluded.contains(&f.id))
        .cloned()
        .map(|mut f| {
            let correction = corrections
                .get(&f.id)
                .map(|v| v.trim())
                .filter(|v| !v.is_empty());
            if let Some(value) = correction {
                f.original_origin = Some(f.original_origin.unwrap_or(f.origin));
                // What was verified was Atlas's value, not the user's.
                f.verification = Verification::default();
                f.reason = Some(format!("Corrected by the user (Atlas found: {})", f.value));
                f.evidence.push(Evidence::new("user", Some("correction")));
                f.value = value.to_owned();
                f.label = label_for(&f.key, value);
                f.confidence = Confidence::High;
                f.origin = Origin::UserCorrected;
            } else if confirmed.contains(&f.id) && f.origin == Origin::Inference {
                // The user vouches for it; it does not become a fact, and stays recognisable as
                // what Atlas inferred.
                f.original_origin = Some(Origin::Inference);
                f.origin = Origin::UserConfirmed;
                f.evidence.push(Evidence::new("user", Some("confirmation")));
            }
            f
        })
        .collect()
}

/// Inferences that matter enough to ask the user about.
fn needs_confirmation(f: &Finding) -> bool {
    f.origin == Origin::Inference
        && f.confidence >= Confidence::Medium
        && matches!(
            f.category,
            FindingCategory::Architecture
                | FindingCategory::Module
                | FindingCategory::Convention
                | FindingCategory::Integration
        )
}

/// Stamps the time a fact was read from the repository onto findings that do not carry one
/// (fresh analyses, and knowledge written before V0.7.2, whose facts were read the same way).
pub fn stamp_verification(findings: &mut [Finding], analyzed_at: u64) {
    for f in findings {
        if f.origin == Origin::Fact && f.verification.is_unverified() {
            f.verification = Verification::verified(VerificationMethod::RepositoryFile, None);
        }
        if f.verification.status == VerificationStatus::Verified
            && f.verification.verified_at.is_none()
            && analyzed_at > 0
        {
            f.verification.verified_at = Some(analyzed_at);
        }
    }
}

/// Marks what the project's changes put in doubt. A finding is affected when the changed files
/// or folders are where its evidence lives. The user's own words are never marked: they are
/// not claims about the repository. Nothing is removed.
pub fn mark_stale(findings: &mut [Finding], staleness: &Staleness) {
    for f in findings {
        if f.origin.is_user() || f.origin == Origin::Generated {
            continue;
        }
        if f.evidence.iter().any(|e| touches(&e.source, staleness)) {
            f.verification.status = VerificationStatus::Stale;
        }
    }
}

/// The findings as the user left them, with what the project's changes put in doubt marked.
pub fn effective_findings(
    knowledge: &HarnessKnowledge,
    manifest: &HarnessManifest,
    drift: &Drift,
) -> Vec<Finding> {
    let mut effective = apply_choices(
        &knowledge.findings,
        &manifest.excluded,
        &manifest.corrections,
        &manifest.confirmed,
    );
    if let Drift::Changed(staleness) = drift {
        mark_stale(&mut effective, staleness);
    }
    effective
}

/// Counts for a glance. `stale` findings count only as stale, not as verified.
pub fn stats(effective: &[Finding], gaps: &[Gap]) -> KnowledgeStats {
    let mut stats = KnowledgeStats {
        unknown: gaps.len(),
        ..KnowledgeStats::default()
    };
    for f in effective
        .iter()
        .filter(|f| f.origin != Origin::Generated && f.category != FindingCategory::Environment)
    {
        stats.findings += 1;
        if f.verification.status == VerificationStatus::Stale {
            stats.stale += 1;
        } else if f.origin.is_user() {
            stats.user += 1;
        } else if f.verification.status == VerificationStatus::Verified {
            stats.verified += 1;
        } else {
            stats.inferred += 1;
        }
    }
    stats
}

/// How far the Harness can be trusted.
///
/// Precedence, strongest first: **Stale > Conflicted > Partial > `NeedsReview` > Healthy**. A stale
/// Harness outranks the rest because its other judgements (its conflicts, its gaps) describe a
/// project that no longer exists as analysed; refreshing is the first step and may dissolve them.
/// Every reason is still listed. `Healthy` means: not stale, no unresolved conflict, a complete
/// analysis and nothing important waiting for the user's confirmation.
pub fn health(
    knowledge: Option<&HarnessKnowledge>,
    manifest: &HarnessManifest,
    drift: &Drift,
) -> HarnessHealth {
    let Some(knowledge) = knowledge else {
        return HarnessHealth {
            state: HealthState::NeedsReview,
            reasons: vec!["legacy_harness".to_owned()],
        };
    };
    let effective = apply_choices(
        &knowledge.findings,
        &manifest.excluded,
        &manifest.corrections,
        &manifest.confirmed,
    );
    let mut reasons = Vec::new();
    let mut state = HealthState::Healthy;
    let mut raise = |new: HealthState, reason: &str| {
        reasons.push(reason.to_owned());
        let rank = |s: HealthState| match s {
            HealthState::Stale => 4,
            HealthState::Conflicted => 3,
            HealthState::Partial => 2,
            HealthState::NeedsReview => 1,
            HealthState::Healthy => 0,
        };
        if rank(new) > rank(state) {
            state = new;
        }
    };
    match drift {
        Drift::Changed(_) => raise(HealthState::Stale, "project_changed"),
        // Knowledge from before fingerprints: nothing says it still holds.
        Drift::Unknown => raise(HealthState::NeedsReview, "no_fingerprint"),
        Drift::Current | Drift::Unchecked => {}
    }
    let unresolved = knowledge
        .conflicts
        .iter()
        .any(|c| c.resolution.is_none() && !manifest.corrections.contains_key(&c.finding_id));
    if unresolved {
        raise(HealthState::Conflicted, "unresolved_conflicts");
    }
    if knowledge.analysis.partial {
        raise(HealthState::Partial, "partial_analysis");
    }
    if effective.iter().any(needs_confirmation) {
        raise(HealthState::NeedsReview, "unconfirmed_inferences");
    }
    HarnessHealth { state, reasons }
}

/// What a new analysis changes compared with the Harness's stored knowledge.
pub fn diff(old: &[Finding], new: &[Finding]) -> HarnessDiff {
    let old_by_id: BTreeMap<&str, &Finding> = old.iter().map(|f| (f.id.as_str(), f)).collect();
    let new_by_id: BTreeMap<&str, &Finding> = new.iter().map(|f| (f.id.as_str(), f)).collect();
    let mut result = HarnessDiff::default();
    for finding in new {
        match old_by_id.get(finding.id.as_str()) {
            None => result.added.push(FindingChange {
                id: finding.id.clone(),
                label: finding.label.clone(),
                before: None,
                after: Some(finding.label.clone()),
            }),
            Some(previous) if previous.value != finding.value => {
                result.changed.push(FindingChange {
                    id: finding.id.clone(),
                    label: finding.label.clone(),
                    before: Some(previous.label.clone()),
                    after: Some(finding.label.clone()),
                });
            }
            Some(_) => result.unchanged.push(finding.label.clone()),
        }
    }
    for finding in old {
        if !new_by_id.contains_key(finding.id.as_str()) {
            result.removed.push(FindingChange {
                id: finding.id.clone(),
                label: finding.label.clone(),
                before: Some(finding.label.clone()),
                after: None,
            });
        }
    }
    result
}

// ---- The user's own files ----

pub const BUSINESS_FILE: &str = "context/business.md";
pub const CONSTRAINTS_FILE: &str = "context/constraints.md";
pub const DECISIONS_FILE: &str = "context/decisions.md";

const NOT_PROVIDED: &str = "_Not provided._";
const NOTICE: &str =
    "> Written by you. Atlas never edits or \"corrects\" this file unless you change it in the review.";
/// Placeholders V0.7 wrote; they mean "empty".
const LEGACY_PLACEHOLDERS: &[&str] = &[
    "The project's purpose has not been provided.",
    "No business rules have been provided.",
    "No constraints have been provided.",
];

fn shown(text: &str) -> &str {
    if text.is_empty() {
        NOT_PROVIDED
    } else {
        text
    }
}

fn cleaned(text: &str) -> String {
    let text = text.trim();
    if text == NOT_PROVIDED || LEGACY_PLACEHOLDERS.contains(&text) {
        String::new()
    } else {
        text.to_owned()
    }
}

pub fn render_business(user: &UserKnowledge) -> String {
    format!(
        "# Business Context\n\n{NOTICE}\n\n## Purpose\n\n{}\n\n## Users\n\n{}\n\n\
         ## Important Concepts\n\n{}\n\n## Business Rules\n\n{}\n",
        shown(&user.purpose),
        shown(&user.users),
        shown(&user.concepts),
        shown(&user.business_rules),
    )
}

pub fn render_body_file(title: &str, text: &str) -> String {
    format!("# {title}\n\n{NOTICE}\n\n{}\n", shown(text))
}

/// The four business fields, or `None` when the file has structure Atlas would lose by
/// rewriting it (an unknown heading, text outside any section).
pub fn parse_business(text: &str) -> Option<UserKnowledge> {
    let mut user = UserKnowledge::default();
    let mut current: Option<&'static str> = None;
    let mut buffers: BTreeMap<&'static str, String> = BTreeMap::new();
    for line in text.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            current = Some(match heading.trim().to_lowercase().as_str() {
                "purpose" => "purpose",
                "users" => "users",
                "important concepts" | "concepts" => "concepts",
                "business rules" => "rules",
                _ => return None,
            });
        } else if line.starts_with("# ")
            || line.starts_with("> ")
            || line.trim() == "_Provided by the user._"
        {
            // Title, notice and V0.7's attribution line.
        } else if let Some(section) = current {
            let buffer = buffers.entry(section).or_default();
            buffer.push_str(line);
            buffer.push('\n');
        } else if !line.trim().is_empty() {
            return None;
        }
    }
    let get = |k: &str| cleaned(buffers.get(k).map_or("", String::as_str));
    user.purpose = get("purpose");
    user.users = get("users");
    user.concepts = get("concepts");
    user.business_rules = get("rules");
    Some(user)
}

/// The body of a free-text user file, without its title and notice.
pub fn parse_body_file(text: &str) -> String {
    let body: Vec<&str> = text
        .lines()
        .filter(|l| {
            !(l.starts_with("# ") || l.starts_with("> ") || l.trim() == "_Provided by the user._")
        })
        .collect();
    cleaned(&body.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(
        key: &str,
        value: &str,
        origin: Origin,
        confidence: Confidence,
        cat: FindingCategory,
    ) -> Finding {
        Finding::new(cat, key, value, confidence, origin, "src", "")
    }

    #[test]
    fn precedence_user_over_facts_and_inferences_with_provenance_kept() {
        let findings = [
            finding(
                "angular",
                "18",
                Origin::Fact,
                Confidence::High,
                FindingCategory::Framework,
            ),
            finding(
                "layered",
                "possible",
                Origin::Inference,
                Confidence::Medium,
                FindingCategory::Architecture,
            ),
            finding(
                "vitest",
                "3",
                Origin::Fact,
                Confidence::High,
                FindingCategory::Testing,
            ),
        ];
        let corrections: BTreeMap<String, String> =
            [("framework:angular".to_owned(), "19".to_owned())].into();

        let out = apply_choices(
            &findings,
            &["testing:vitest".to_owned()],
            &corrections,
            &["architecture:layered".to_owned()],
        );

        assert_eq!(out.len(), 2);
        assert_eq!(
            (out[0].origin, out[0].value.as_str(), out[0].confidence),
            (Origin::UserCorrected, "19", Confidence::High)
        );
        assert_eq!(out[0].label, "Angular 19");
        assert!(out[0].reason.as_ref().unwrap().contains("Atlas found: 18"));
        assert_eq!(out[1].origin, Origin::UserConfirmed);
        // A confirmation never turns a fact into something else.
        let facts = apply_choices(
            &findings,
            &[],
            &BTreeMap::new(),
            &["framework:angular".to_owned()],
        );
        assert_eq!(facts[0].origin, Origin::Fact);
    }

    #[test]
    fn the_diff_lists_added_removed_changed_and_unchanged() {
        let old = [
            finding(
                "angular",
                "18",
                Origin::Fact,
                Confidence::High,
                FindingCategory::Framework,
            ),
            finding(
                "jest",
                "29",
                Origin::Fact,
                Confidence::High,
                FindingCategory::Testing,
            ),
            finding(
                "docker",
                "true",
                Origin::Fact,
                Confidence::High,
                FindingCategory::Infrastructure,
            ),
        ];
        let new = [
            finding(
                "angular",
                "21",
                Origin::Fact,
                Confidence::High,
                FindingCategory::Framework,
            ),
            finding(
                "vitest",
                "3",
                Origin::Fact,
                Confidence::High,
                FindingCategory::Testing,
            ),
            finding(
                "docker",
                "true",
                Origin::Fact,
                Confidence::High,
                FindingCategory::Infrastructure,
            ),
        ];

        let d = diff(&old, &new);

        assert_eq!(
            (
                d.changed[0].before.as_deref(),
                d.changed[0].after.as_deref()
            ),
            (Some("Angular 18"), Some("Angular 21"))
        );
        assert_eq!(d.added[0].label, "Vitest 3");
        assert_eq!(d.removed[0].label, "Jest 29");
        assert_eq!(d.unchanged, ["Docker"]);
    }

    #[test]
    fn business_files_round_trip_and_unknown_structure_is_refused() {
        let user = UserKnowledge {
            purpose: "ERP".to_owned(),
            business_rules: "Never delete an invoice.\n\nEver.".to_owned(),
            ..UserKnowledge::default()
        };

        let text = render_business(&user);
        let parsed = parse_business(&text).unwrap();

        assert_eq!(parsed, user);
        assert!(text.contains("_Not provided._"));
        assert_eq!(parse_business("# B\n\n## Glossary\n\nx"), None);
        assert_eq!(parse_business("# B\n\nloose text"), None);
    }

    #[test]
    fn v07_business_files_and_placeholders_are_understood() {
        let v07 = "# Business Context\n\n## Purpose\n\nERP\n\n_Provided by the user._\n\n## Business rules\n\nNo business rules have been provided.\n\n";

        let parsed = parse_business(v07).unwrap();

        assert_eq!(parsed.purpose, "ERP");
        assert_eq!(parsed.business_rules, "");
        assert_eq!(
            parse_body_file("# Constraints\n\nNo constraints have been provided.\n"),
            ""
        );
        assert_eq!(
            parse_body_file(&render_body_file("Decisions", "Keep it.")),
            "Keep it."
        );
    }

    #[test]
    fn health_ranks_conflicts_over_partial_over_review_and_lists_every_reason() {
        use crate::domain::harness::{AnalysisInfo, Claim, Conflict, KNOWLEDGE_VERSION};
        let manifest = crate::application::harness::manifest::build_manifest(
            "p".to_owned(),
            "x",
            1,
            &[],
            BTreeMap::new(),
            vec![],
            vec![],
        );
        let knowledge = HarnessKnowledge {
            version: KNOWLEDGE_VERSION,
            analysis: AnalysisInfo {
                partial: true,
                ..AnalysisInfo::default()
            },
            findings: vec![finding(
                "layered",
                "possible",
                Origin::Inference,
                Confidence::Medium,
                FindingCategory::Architecture,
            )],
            conflicts: vec![Conflict {
                finding_id: "framework:angular".to_owned(),
                label: "Angular".to_owned(),
                claims: vec![Claim {
                    value: "1".to_owned(),
                    choice: "1".to_owned(),
                    origin: Origin::Fact,
                    evidence: vec![],
                }],
                resolution: None,
            }],
        };

        let h = health(Some(&knowledge), &manifest, &Drift::Current);

        assert_eq!(h.state, HealthState::Conflicted);
        assert_eq!(
            h.reasons,
            [
                "unresolved_conflicts",
                "partial_analysis",
                "unconfirmed_inferences"
            ]
        );
        // Low-confidence inferences do not demand review.
        let weak = HarnessKnowledge {
            findings: vec![finding(
                "layered",
                "possible",
                Origin::Inference,
                Confidence::Low,
                FindingCategory::Architecture,
            )],
            conflicts: vec![],
            analysis: AnalysisInfo::default(),
            ..knowledge
        };
        assert_eq!(
            health(Some(&weak), &manifest, &Drift::Current).state,
            HealthState::Healthy
        );
    }
}
