use std::collections::BTreeMap;

use super::{npm, SnapshotAnalyzer};
use crate::application::harness::sampler::is_test_file;
use crate::application::harness::snapshot::{file_name, ScanSnapshot};
use crate::domain::harness::{Confidence, Evidence, Finding, FindingCategory, Origin};

/// What there is to say about testing, kept apart on purpose: a framework being installed is
/// not the same as tests existing, and neither means coverage.
///
/// The analyzers elsewhere report the framework (`testing:vitest`) and the declared command
/// (`build:test_node`); this one adds the test files, the folders and the coverage setup.
pub struct TestingAnalyzer;

const COVERAGE_DEPS: &[&str] = &[
    "@vitest/coverage-v8",
    "@vitest/coverage-istanbul",
    "c8",
    "nyc",
    "istanbul",
];

const PATTERNS: &[&str] = &[
    ".spec.ts",
    ".test.ts",
    ".spec.tsx",
    ".test.tsx",
    ".spec.js",
    ".test.js",
    "Tests.cs",
    "_test.go",
    "Test.java",
];

impl SnapshotAnalyzer for TestingAnalyzer {
    fn analyze(&self, s: &ScanSnapshot) -> Vec<Finding> {
        let mut out = Vec::new();
        test_files(s, &mut out);
        folders(s, &mut out);
        coverage(s, &mut out);
        out
    }
}

fn test_files(s: &ScanSnapshot, out: &mut Vec<Finding>) {
    let files: Vec<&str> = s
        .entries
        .iter()
        .filter(|e| !e.is_dir && is_test_file(&file_name(&e.path).to_lowercase()))
        .map(|e| e.path.as_str())
        .collect();
    if files.is_empty() {
        return;
    }
    let samples = |list: &[&str]| -> Vec<Evidence> {
        list.iter()
            .take(3)
            .map(|p| Evidence::new(p, None))
            .collect()
    };
    out.push(
        Finding::new(
            FindingCategory::Testing,
            "test_files",
            &files.len().to_string(),
            Confidence::High,
            Origin::Fact,
            "",
            "",
        )
        .with_evidence(samples(&files))
        .with_label(&if files.len() == 1 {
            "1 test file found".to_owned()
        } else {
            format!("{} test files found", files.len())
        }),
    );

    // The naming convention is whichever pattern most test files share, if several do.
    let mut counts: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for file in &files {
        if let Some(pattern) = PATTERNS.iter().find(|p| file.ends_with(*p)) {
            counts.entry(pattern).or_default().push(file);
        }
    }
    if let Some((pattern, list)) = counts.iter().max_by_key(|(_, list)| list.len()) {
        if list.len() >= 2 {
            let shown = format!("*{pattern}");
            out.push(
                Finding::new(
                    FindingCategory::Convention,
                    "test_file_pattern",
                    &shown,
                    Confidence::High,
                    Origin::Fact,
                    "",
                    "",
                )
                .with_evidence(samples(list))
                .with_label(&format!("Tests are named {shown}")),
            );
        }
    }
}

fn folders(s: &ScanSnapshot, out: &mut Vec<Finding>) {
    for (names, key, label) in [
        (
            &["e2e", "cypress"][..],
            "e2e_tests",
            "End-to-end tests folder",
        ),
        (
            &["integration", "integration-tests", "integrationtests"][..],
            "integration_tests",
            "Integration tests folder",
        ),
        (
            &["unit", "unit-tests", "unittests"][..],
            "unit_tests",
            "Unit tests folder",
        ),
    ] {
        if let Some(dir) = s.all_dirs().find(|d| {
            d.matches('/').count() <= 3 && names.contains(&file_name(d).to_lowercase().as_str())
        }) {
            out.push(
                Finding::new(
                    FindingCategory::Testing,
                    key,
                    dir,
                    Confidence::High,
                    Origin::Fact,
                    dir,
                    "",
                )
                .with_label(label),
            );
        }
    }
    if let Some(config) = s
        .entries
        .iter()
        .find(|e| !e.is_dir && e.path.starts_with("playwright.config."))
    {
        out.push(
            Finding::new(
                FindingCategory::Testing,
                "e2e_tests",
                &config.path,
                Confidence::High,
                Origin::Fact,
                &config.path,
                "",
            )
            .with_label("End-to-end tests (Playwright)"),
        );
    }
}

/// Coverage is only reported when something configures it, never from the test framework alone.
fn coverage(s: &ScanSnapshot, out: &mut Vec<Finding>) {
    for (path, json) in npm::manifests(s) {
        if let Some((name, field)) = COVERAGE_DEPS
            .iter()
            .find_map(|n| npm::dependency(&json, n).map(|(f, _)| (*n, f)))
        {
            out.push(
                Finding::new(
                    FindingCategory::Testing,
                    "coverage_config",
                    name,
                    Confidence::High,
                    Origin::Fact,
                    path,
                    &field,
                )
                .with_label(&format!("Coverage tooling: {name}")),
            );
            return;
        }
    }
    for file in [
        ".nycrc",
        ".coveragerc",
        "codecov.yml",
        ".codecov.yml",
        "jest.config.js",
    ] {
        if s.has_path(file) && file != "jest.config.js" {
            out.push(
                Finding::new(
                    FindingCategory::Testing,
                    "coverage_config",
                    file,
                    Confidence::High,
                    Origin::Fact,
                    file,
                    "",
                )
                .with_label(&format!("Coverage configuration: {file}")),
            );
            return;
        }
    }
    if s.files
        .iter()
        .any(|(p, t)| p.ends_with(".csproj") && t.contains("coverlet"))
    {
        out.push(
            Finding::new(
                FindingCategory::Testing,
                "coverage_config",
                "coverlet",
                Confidence::High,
                Origin::Fact,
                "",
                "",
            )
            .with_evidence(vec![Evidence::new(
                s.files
                    .keys()
                    .find(|p| p.ends_with(".csproj"))
                    .map_or("", String::as_str),
                None,
            )])
            .with_label("Coverage tooling: coverlet"),
        );
    }
}
