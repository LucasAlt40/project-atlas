//! Negative knowledge: what the Harness does *not* hold, said honestly.
//!
//! Two different statements, never mixed:
//!
//! - **not found**: a bounded search (the analyzed manifests, configuration and folder layout)
//!   came back empty. It says where Atlas looked, and never claims the thing does not exist.
//! - **unknown**: nothing Atlas read could establish it (production deployment, whether a
//!   command works, whether an inferred architecture is still enforced).
//!
//! Pure functions over the effective findings; nothing is searched here and nothing runs.

use crate::domain::harness::{AnalysisInfo, Finding, FindingCategory, Gap, GapKind, Origin};

/// Categories the analysis actively looks for: if there is none, that is a *not found*.
const PROBED: &[(FindingCategory, &str, &str)] = &[
    (FindingCategory::Database, "database", "database technology"),
    (FindingCategory::Testing, "testing", "test framework"),
    (FindingCategory::Ci, "ci", "CI/CD configuration"),
    (
        FindingCategory::Infrastructure,
        "infrastructure",
        "container or infrastructure configuration",
    ),
];

pub fn gaps(effective: &[Finding], analysis: &AnalysisInfo) -> Vec<Gap> {
    let mut gaps = Vec::new();
    let scope = if analysis.partial {
        "the analyzed manifests, configuration and folder layout (the analysis was partial)"
    } else {
        "the analyzed manifests, configuration and folder layout"
    };
    for (category, subject, noun) in PROBED {
        if !effective.iter().any(|f| f.category == *category) {
            gaps.push(Gap {
                kind: GapKind::NotFound,
                subject: (*subject).to_owned(),
                statement: format!(
                    "No {noun} was found in {scope}. This is not proof that the project has none."
                ),
            });
        }
    }
    if effective
        .iter()
        .any(|f| f.category == FindingCategory::Build)
    {
        gaps.push(Gap {
            kind: GapKind::Unknown,
            subject: "commands".to_owned(),
            statement: "Build and test commands were read from manifests but never executed; \
                        they are not verified to work."
                .to_owned(),
        });
    }
    if effective.iter().any(|f| {
        f.category == FindingCategory::Architecture
            && f.origin == Origin::Inference
            && !f.origin.is_user()
    }) {
        gaps.push(Gap {
            kind: GapKind::Unknown,
            subject: "architecture_enforcement".to_owned(),
            statement: "Whether the inferred architecture is still actively enforced is not \
                        known."
                .to_owned(),
        });
    }
    gaps.push(Gap {
        kind: GapKind::Unknown,
        subject: "deployment".to_owned(),
        statement: "Production deployment and runtime infrastructure were not verified.".to_owned(),
    });
    gaps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::harness::Confidence;

    fn finding(category: FindingCategory, key: &str, origin: Origin) -> Finding {
        Finding::new(category, key, "true", Confidence::High, origin, "src", "")
    }

    #[test]
    fn a_probe_that_found_nothing_is_not_found_and_never_says_it_does_not_exist() {
        let gaps = gaps(&[], &AnalysisInfo::default());

        let db = gaps.iter().find(|g| g.subject == "database").unwrap();
        assert_eq!(db.kind, GapKind::NotFound);
        assert!(db.statement.contains("analyzed manifests"));
        assert!(db.statement.contains("not proof"));
        assert!(!db.statement.contains("does not use"));
    }

    #[test]
    fn a_found_category_has_no_not_found() {
        let findings = [finding(
            FindingCategory::Database,
            "postgresql",
            Origin::Fact,
        )];

        let gaps = gaps(&findings, &AnalysisInfo::default());

        assert!(gaps.iter().all(|g| g.subject != "database"));
    }

    #[test]
    fn unknowns_are_a_different_kind_from_not_found() {
        let findings = [
            finding(FindingCategory::Build, "test", Origin::Fact),
            finding(FindingCategory::Architecture, "layered", Origin::Inference),
        ];

        let gaps = gaps(&findings, &AnalysisInfo::default());

        for subject in ["commands", "architecture_enforcement", "deployment"] {
            let gap = gaps.iter().find(|g| g.subject == subject).unwrap();
            assert_eq!(gap.kind, GapKind::Unknown, "{subject}");
        }
    }

    #[test]
    fn a_partial_analysis_says_so_in_what_was_searched() {
        let partial = AnalysisInfo {
            partial: true,
            ..AnalysisInfo::default()
        };

        let gaps = gaps(&[], &partial);

        assert!(gaps
            .iter()
            .filter(|g| g.kind == GapKind::NotFound)
            .all(|g| g.statement.contains("partial")));
    }
}
