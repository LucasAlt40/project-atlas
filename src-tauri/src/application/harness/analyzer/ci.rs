use super::SnapshotAnalyzer;
use crate::application::harness::snapshot::{file_name, ScanSnapshot};
use crate::domain::harness::{Confidence, Finding, FindingCategory, Origin};

pub struct CiAnalyzer;

impl SnapshotAnalyzer for CiAnalyzer {
    fn analyze(&self, s: &ScanSnapshot) -> Vec<Finding> {
        let ci = |key: &str, source: &str, evidence: &str| {
            Finding::new(
                FindingCategory::Ci,
                key,
                "true",
                Confidence::High,
                Origin::Fact,
                source,
                evidence,
            )
        };
        let mut out = Vec::new();
        let workflows: Vec<&str> = s
            .entries
            .iter()
            .filter(|e| !e.is_dir && e.path.starts_with(".github/workflows/"))
            .map(|e| file_name(&e.path))
            .collect();
        if !workflows.is_empty() {
            out.push(ci(
                "github_actions",
                ".github/workflows/",
                &workflows
                    .iter()
                    .take(5)
                    .copied()
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        }
        for (file, key) in [
            (".gitlab-ci.yml", "gitlab_ci"),
            ("azure-pipelines.yml", "azure_pipelines"),
            ("Jenkinsfile", "jenkins"),
            (".circleci/config.yml", "circleci"),
        ] {
            if s.has_path(file) {
                out.push(ci(key, file, file));
            }
        }
        out
    }
}
