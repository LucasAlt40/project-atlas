//! Looks for contradictions between everything Atlas knows: deterministic findings, semantic
//! findings, what the documentation says, the existing Harness and the user's corrections.
//! It only *records* them. It never picks a side, except that a value the user chose is the
//! resolution.

use std::collections::BTreeMap;

use super::sampler::SampleFile;
use crate::domain::harness::{
    Claim, Conflict, Evidence, Finding, FindingCategory, HarnessKnowledge, Origin,
};

pub struct ConsistencyInput<'a> {
    pub deterministic: &'a [Finding],
    pub semantic: &'a [Finding],
    /// Sampled files; only the documentation among them (README, docs/, *.md) is read.
    pub documents: &'a [SampleFile],
    /// The knowledge stored in the existing Harness, if any.
    pub previous: Option<&'a HarnessKnowledge>,
    /// The user's corrections (stored ones and those made in this review): they resolve.
    pub corrections: &'a BTreeMap<String, String>,
}

/// How documentation names the technologies whose version it might state.
const DOC_NAMES: &[(&str, &str)] = &[
    ("angular", "angular"),
    ("react", "react"),
    ("vue", "vue"),
    ("nextjs", "next.js"),
    ("nestjs", "nestjs"),
    ("dotnet", ".net"),
    ("spring_boot", "spring boot"),
    ("django", "django"),
    ("nodejs", "node.js"),
    ("tauri", "tauri"),
];

pub fn check(input: &ConsistencyInput<'_>) -> Vec<Conflict> {
    let mut conflicts = Vec::new();
    semantic_vs_deterministic(input, &mut conflicts);
    documentation_vs_facts(input, &mut conflicts);
    user_vs_new_facts(input, &mut conflicts);
    conflicts.sort_by(|a, b| a.finding_id.cmp(&b.finding_id));
    conflicts.dedup_by(|a, b| a.finding_id == b.finding_id);
    for conflict in &mut conflicts {
        if conflict.resolution.is_none() {
            conflict.resolution = input.corrections.get(&conflict.finding_id).cloned();
        }
    }
    conflicts
}

fn claim(f: &Finding) -> Claim {
    Claim {
        value: f.label.clone(),
        choice: f.value.clone(),
        origin: f.origin,
        evidence: f.evidence.clone(),
    }
}

fn semantic_vs_deterministic(input: &ConsistencyInput<'_>, out: &mut Vec<Conflict>) {
    for semantic in input.semantic {
        let Some(fact) = input
            .deterministic
            .iter()
            .find(|d| d.id == semantic.id && d.origin != Origin::Generated)
        else {
            continue;
        };
        if !fact.value.eq_ignore_ascii_case(&semantic.value) {
            out.push(Conflict {
                finding_id: fact.id.clone(),
                label: fact.label.clone(),
                claims: vec![claim(fact), claim(semantic)],
                resolution: None,
            });
        }
    }
}

fn major(value: &str) -> &str {
    value.split('.').next().unwrap_or(value)
}

/// The first version number written right after `needle`, e.g. `angular 18` or `.NET v8.0`.
fn version_after(text: &str, needle: &str) -> Option<String> {
    let mut from = 0;
    while let Some(at) = text[from..].find(needle) {
        let rest = text[from + at + needle.len()..].trim_start_matches([' ', '\t']);
        let rest = rest.strip_prefix('v').unwrap_or(rest);
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if !digits.is_empty() {
            return Some(digits);
        }
        from += at + needle.len();
    }
    None
}

fn is_document(path: &str) -> bool {
    let lower = path.to_lowercase();
    std::path::Path::new(&lower)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("md"))
        || lower.contains("readme")
}

fn documentation_vs_facts(input: &ConsistencyInput<'_>, out: &mut Vec<Conflict>) {
    for fact in input.deterministic.iter().filter(|f| {
        f.origin == Origin::Fact
            && matches!(
                f.category,
                FindingCategory::Framework | FindingCategory::Runtime | FindingCategory::Language
            )
    }) {
        let Some((_, name)) = DOC_NAMES.iter().find(|(key, _)| *key == fact.key) else {
            continue;
        };
        if !fact
            .value
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
        {
            continue;
        }
        for document in input.documents.iter().filter(|d| is_document(&d.path)) {
            let Some(claimed) = version_after(&document.content.to_lowercase(), name) else {
                continue;
            };
            if claimed != major(&fact.value) {
                out.push(Conflict {
                    finding_id: fact.id.clone(),
                    label: fact.label.clone(),
                    claims: vec![
                        claim(fact),
                        Claim {
                            value: format!("{name} {claimed}"),
                            choice: claimed.clone(),
                            origin: Origin::Inference,
                            evidence: vec![Evidence::new(
                                &document.path,
                                Some(&format!("mentions {name} {claimed}")),
                            )],
                        },
                    ],
                    resolution: None,
                });
                break;
            }
        }
    }
}

/// The user corrected a value, and the repository has since changed to a third one: the user's
/// word stands, but the disagreement is shown so it is not forgotten.
fn user_vs_new_facts(input: &ConsistencyInput<'_>, out: &mut Vec<Conflict>) {
    let Some(previous) = input.previous else {
        return;
    };
    for (id, corrected) in input.corrections {
        let (Some(new), Some(old)) = (
            input.deterministic.iter().find(|f| &f.id == id),
            previous.findings.iter().find(|f| &f.id == id),
        ) else {
            continue;
        };
        if new.value != old.value && !new.value.eq_ignore_ascii_case(corrected) {
            out.push(Conflict {
                finding_id: id.clone(),
                label: new.label.clone(),
                claims: vec![
                    Claim {
                        value: corrected.clone(),
                        choice: corrected.clone(),
                        origin: Origin::UserCorrected,
                        evidence: vec![Evidence::new("user", Some("correction"))],
                    },
                    claim(new),
                ],
                resolution: Some(corrected.clone()),
            });
        }
    }
}

#[cfg(test)]
#[allow(clippy::assert_is_empty)]
mod tests {
    use super::*;
    use crate::domain::harness::{AnalysisInfo, Confidence, KNOWLEDGE_VERSION};

    fn fact(key: &str, value: &str, category: FindingCategory) -> Finding {
        Finding::new(
            category,
            key,
            value,
            Confidence::High,
            Origin::Fact,
            "package.json",
            key,
        )
    }

    fn doc(path: &str, text: &str) -> SampleFile {
        SampleFile {
            path: path.to_owned(),
            content: text.to_owned(),
            truncated: false,
        }
    }

    fn check_with(
        deterministic: &[Finding],
        semantic: &[Finding],
        documents: &[SampleFile],
        previous: Option<&HarnessKnowledge>,
        corrections: &BTreeMap<String, String>,
    ) -> Vec<Conflict> {
        check(&ConsistencyInput {
            deterministic,
            semantic,
            documents,
            previous,
            corrections,
        })
    }

    #[test]
    fn documentation_that_names_another_version_is_a_conflict_with_both_sides_and_their_evidence() {
        let facts = [fact("angular", "21", FindingCategory::Framework)];
        let docs = [doc("README.md", "# App\nBuilt on Angular 18, see docs.")];

        let conflicts = check_with(&facts, &[], &docs, None, &BTreeMap::new());

        assert_eq!(conflicts.len(), 1);
        let c = &conflicts[0];
        assert_eq!(c.finding_id, "framework:angular");
        assert_eq!(c.claims[0].value, "Angular 21");
        assert_eq!(c.claims[0].origin, Origin::Fact);
        assert_eq!(c.claims[1].value, "angular 18");
        assert_eq!(c.claims[1].evidence[0].source, "README.md");
        assert_eq!(c.resolution, None);
    }

    #[test]
    fn agreeing_documentation_and_non_documents_are_no_conflict() {
        let facts = [fact("angular", "21", FindingCategory::Framework)];
        let agree = [doc("README.md", "Angular 21 and v21")];
        let source_file = [doc("src/app.ts", "// angular 3")];

        assert!(check_with(&facts, &[], &agree, None, &BTreeMap::new()).is_empty());
        assert!(check_with(&facts, &[], &source_file, None, &BTreeMap::new()).is_empty());
    }

    #[test]
    fn a_version_after_a_v_prefix_and_a_dotnet_style_name_are_read() {
        let facts = [fact("dotnet", "10.0", FindingCategory::Runtime)];

        let conflicts = check_with(
            &facts,
            &[],
            &[doc("docs/setup.md", "Requires .NET v8.0 SDK")],
            None,
            &BTreeMap::new(),
        );

        assert_eq!(conflicts[0].claims[1].value, ".net 8");
    }

    #[test]
    fn a_model_saying_something_else_than_a_fact_is_a_conflict() {
        let facts = [fact("clean", "true", FindingCategory::Architecture)];
        let model = [Finding::new(
            FindingCategory::Architecture,
            "clean",
            "layered",
            Confidence::Medium,
            Origin::Inference,
            "src",
            "",
        )];

        let conflicts = check_with(&facts, &model, &[], None, &BTreeMap::new());

        assert_eq!(conflicts[0].finding_id, "architecture:clean");
        assert_eq!(conflicts[0].claims[1].origin, Origin::Inference);
    }

    #[test]
    fn the_users_choice_resolves_a_conflict_without_deleting_it() {
        let facts = [fact("angular", "21", FindingCategory::Framework)];
        let docs = [doc("README.md", "Angular 18")];
        let corrections: BTreeMap<String, String> =
            [("framework:angular".to_owned(), "21".to_owned())].into();

        let conflicts = check_with(&facts, &[], &docs, None, &corrections);

        assert_eq!(conflicts[0].resolution.as_deref(), Some("21"));
        assert_eq!(conflicts[0].claims.len(), 2);
    }

    #[test]
    fn a_user_correction_that_the_repository_has_since_moved_past_is_recorded_as_resolved_by_the_user(
    ) {
        let previous = HarnessKnowledge {
            version: KNOWLEDGE_VERSION,
            analysis: AnalysisInfo::default(),
            findings: vec![fact("angular", "18", FindingCategory::Framework)],
            conflicts: vec![],
        };
        let now = [fact("angular", "21", FindingCategory::Framework)];
        let corrections: BTreeMap<String, String> =
            [("framework:angular".to_owned(), "19".to_owned())].into();

        let conflicts = check_with(&now, &[], &[], Some(&previous), &corrections);

        assert_eq!(conflicts[0].resolution.as_deref(), Some("19"));
        assert_eq!(conflicts[0].claims[0].origin, Origin::UserCorrected);
        // When the repository still says what it said before, the correction is not in conflict.
        let same = [fact("angular", "18", FindingCategory::Framework)];
        assert!(check_with(&same, &[], &[], Some(&previous), &corrections).is_empty());
    }
}
