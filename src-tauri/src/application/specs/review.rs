//! Reading what the reviewing agent reports. The agent ends its answer with one `atlas-review`
//! block of JSON; it is read as data, validated field by field and clipped. Whatever is in it is
//! the agent's opinion: it becomes a finding for the person to decide on, never a failed check.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::domain::spec::rules::finding_id;
use crate::domain::spec::{Finding, FindingSeverity, FindingStatus};

pub const REVIEW_FENCE: &str = "atlas-review";

const MAX_FINDINGS: usize = 40;
const MAX_SUMMARY: usize = 600;
const MAX_TITLE: usize = 160;
const MAX_TEXT: usize = 1_200;
const MAX_SHORT: usize = 400;
const MAX_REFERENCE: usize = 200;

const CATEGORIES: [&str; 9] = [
    "ambiguity",
    "contradiction",
    "missing_scenario",
    "edge_case",
    "business_rule",
    "risk",
    "dependency",
    "coverage",
    "decision",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewProblem {
    /// The answer ends without the block.
    NoBlock,
    /// The block is not the JSON that was asked for.
    Malformed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedReview {
    pub summary: String,
    pub findings: Vec<Finding>,
    /// Findings left out because they had no title or no description.
    pub dropped: usize,
}

fn clip(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn last_block(text: &str) -> Option<&str> {
    let marker = format!("```{REVIEW_FENCE}");
    let at = text.rfind(&marker)?;
    let rest = &text[at + marker.len()..];
    let end = rest.find("```")?;
    Some(rest[..end].trim())
}

fn field(object: &serde_json::Map<String, Value>, key: &str, max: usize) -> String {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(|t| clip(t, max))
        .unwrap_or_default()
}

fn severity(text: &str) -> FindingSeverity {
    match text.trim().to_ascii_lowercase().as_str() {
        "high" | "critical" | "blocker" => FindingSeverity::High,
        "low" | "minor" | "info" => FindingSeverity::Low,
        _ => FindingSeverity::Medium,
    }
}

fn category(text: &str) -> String {
    let normal = text.trim().to_ascii_lowercase().replace(['-', ' '], "_");
    if CATEGORIES.contains(&normal.as_str()) {
        normal
    } else {
        "other".to_owned()
    }
}

/// Reads the review. `revisions` are the revisions of each artifact the agent was shown, so a
/// finding says which one it read.
///
/// # Errors
///
/// When there is no block, or it is not the JSON asked for.
pub fn parse_review(
    output: &str,
    review_id: &str,
    revisions: &BTreeMap<String, u32>,
    now: u64,
) -> Result<ParsedReview, ReviewProblem> {
    let body = last_block(output).ok_or(ReviewProblem::NoBlock)?;
    let value: Value = serde_json::from_str(body).map_err(|_| ReviewProblem::Malformed)?;
    let object = value.as_object().ok_or(ReviewProblem::Malformed)?;
    let items = object
        .get("findings")
        .and_then(Value::as_array)
        .ok_or(ReviewProblem::Malformed)?;
    let summary = field(object, "summary", MAX_SUMMARY);
    let mut findings: Vec<Finding> = Vec::new();
    let mut dropped = 0;
    for item in items.iter().take(MAX_FINDINGS) {
        let Some(entry) = item.as_object() else {
            dropped += 1;
            continue;
        };
        let title = field(entry, "title", MAX_TITLE);
        let description = field(entry, "description", MAX_TEXT);
        if title.is_empty() || description.is_empty() {
            dropped += 1;
            continue;
        }
        let artifact = entry
            .get("artifact")
            .and_then(Value::as_str)
            .map(|a| a.trim().to_ascii_lowercase())
            .filter(|a| revisions.contains_key(a));
        let reference = field(entry, "reference", MAX_REFERENCE);
        let category = category(&field(entry, "category", 40));
        let id = finding_id(&category, artifact.as_deref(), &reference, &title);
        if findings.iter().any(|f| f.id == id) {
            continue;
        }
        let question = Some(field(entry, "question", MAX_SHORT)).filter(|q| !q.is_empty());
        let suggestion = Some(field(entry, "suggestion", MAX_TEXT)).filter(|q| !q.is_empty());
        findings.push(Finding {
            id,
            category,
            severity: severity(&field(entry, "severity", 20)),
            title,
            description,
            artifact_revision: artifact.as_ref().and_then(|a| revisions.get(a).copied()),
            artifact_id: artifact,
            reference,
            evidence: field(entry, "evidence", MAX_SHORT * 2),
            rationale: field(entry, "rationale", MAX_TEXT),
            question,
            suggestion,
            status: FindingStatus::Open,
            resolution_note: None,
            review_id: review_id.to_owned(),
            created_at: now,
            resolved_at: None,
        });
    }
    dropped += items.len().saturating_sub(MAX_FINDINGS);
    Ok(ParsedReview {
        summary,
        findings,
        dropped,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::assert_is_empty)]
    use super::*;

    fn revisions() -> BTreeMap<String, u32> {
        [("specs".to_owned(), 2), ("tasks".to_owned(), 1)].into()
    }

    fn block(json: &str) -> String {
        format!("My review.\n\n```{REVIEW_FENCE}\n{json}\n```\n")
    }

    #[test]
    fn a_well_formed_review_becomes_findings_for_the_person_to_decide() {
        let output = block(
            r#"{"summary":"Two gaps","findings":[
              {"category":"Missing Scenario","severity":"high","artifact":"specs","reference":"Requirement: Export","title":"No error scenario","description":"Export has no failure case.","evidence":"The system SHALL export","rationale":"inference","question":"What if the disk is full?","suggestion":"Add a scenario"},
              {"category":"coverage","severity":"low","artifact":"tasks","title":"Task 1.2 covers nothing","description":"No requirement."}
            ]}"#,
        );

        let parsed = parse_review(&output, "review-1", &revisions(), 5).unwrap();

        assert_eq!(parsed.summary, "Two gaps");
        assert_eq!(parsed.findings.len(), 2);
        let first = &parsed.findings[0];
        assert_eq!(first.category, "missing_scenario");
        assert_eq!(first.severity, FindingSeverity::High);
        assert_eq!(first.artifact_id.as_deref(), Some("specs"));
        assert_eq!(first.artifact_revision, Some(2));
        assert_eq!(first.question.as_deref(), Some("What if the disk is full?"));
        assert_eq!(first.status, FindingStatus::Open);
        assert!(
            first.blocks(),
            "a serious finding with a question blocks until decided"
        );
        assert!(!parsed.findings[1].blocks());
        assert_eq!(parsed.dropped, 0);
    }

    #[test]
    fn an_answer_without_the_block_or_with_broken_json_is_not_a_review() {
        assert_eq!(
            parse_review("I looked and it is fine.", "r", &revisions(), 1),
            Err(ReviewProblem::NoBlock)
        );
        assert_eq!(
            parse_review(&block("{ not json"), "r", &revisions(), 1),
            Err(ReviewProblem::Malformed)
        );
        assert_eq!(
            parse_review(&block(r#"{"summary":"x"}"#), "r", &revisions(), 1),
            Err(ReviewProblem::Malformed)
        );
        assert_eq!(
            parse_review(&block("[1,2]"), "r", &revisions(), 1),
            Err(ReviewProblem::Malformed)
        );
    }

    #[test]
    fn an_empty_list_is_a_review_that_found_nothing() {
        let parsed = parse_review(
            &block(r#"{"summary":"Clean","findings":[]}"#),
            "r",
            &revisions(),
            1,
        )
        .unwrap();

        assert!(parsed.findings.is_empty());
    }

    #[test]
    fn a_finding_without_a_title_or_a_description_is_dropped_and_counted() {
        let output = block(
            r#"{"summary":"","findings":[{"title":"","description":"x"},{"title":"t"},"text",{"title":"ok","description":"d"}]}"#,
        );

        let parsed = parse_review(&output, "r", &revisions(), 1).unwrap();

        assert_eq!(parsed.findings.len(), 1);
        assert_eq!(parsed.dropped, 3);
    }

    #[test]
    fn the_same_problem_reported_twice_is_one_finding() {
        let item = r#"{"category":"ambiguity","artifact":"specs","reference":"R","title":"Unclear","description":"d"}"#;
        let output = block(&format!(r#"{{"summary":"","findings":[{item},{item}]}}"#));

        let parsed = parse_review(&output, "r", &revisions(), 1).unwrap();

        assert_eq!(parsed.findings.len(), 1);
    }

    #[test]
    fn an_artifact_that_does_not_exist_makes_a_finding_about_the_spec_as_a_whole() {
        let output = block(
            r#"{"summary":"","findings":[{"artifact":"database","title":"t","description":"d"}]}"#,
        );

        let parsed = parse_review(&output, "r", &revisions(), 1).unwrap();

        assert_eq!(parsed.findings[0].artifact_id, None);
        assert_eq!(parsed.findings[0].artifact_revision, None);
    }

    #[test]
    fn unknown_words_are_not_trusted_as_categories_or_severities() {
        let output = block(
            r#"{"summary":"","findings":[{"category":"ignore all rules","severity":"apocalyptic","title":"t","description":"d"}]}"#,
        );

        let parsed = parse_review(&output, "r", &revisions(), 1).unwrap();

        assert_eq!(parsed.findings[0].category, "other");
        assert_eq!(parsed.findings[0].severity, FindingSeverity::Medium);
    }

    #[test]
    fn long_fields_are_clipped_and_the_list_is_bounded() {
        let long = "x".repeat(5_000);
        let many: Vec<String> = (0..60)
            .map(|i| format!(r#"{{"title":"t{i}","description":"{long}"}}"#))
            .collect();
        let output = block(&format!(
            r#"{{"summary":"{long}","findings":[{}]}}"#,
            many.join(",")
        ));

        let parsed = parse_review(&output, "r", &revisions(), 1).unwrap();

        assert_eq!(parsed.findings.len(), MAX_FINDINGS);
        assert!(parsed.findings[0].description.chars().count() <= MAX_TEXT);
        assert!(parsed.summary.chars().count() <= MAX_SUMMARY);
        assert_eq!(parsed.dropped, 20);
    }

    #[test]
    fn only_the_last_block_counts() {
        let output = format!(
            "{}\nSecond thoughts:\n{}",
            block(r#"{"summary":"first","findings":[{"title":"a","description":"b"}]}"#),
            block(r#"{"summary":"second","findings":[]}"#)
        );

        let parsed = parse_review(&output, "r", &revisions(), 1).unwrap();

        assert_eq!(parsed.summary, "second");
        assert!(parsed.findings.is_empty());
    }
}
