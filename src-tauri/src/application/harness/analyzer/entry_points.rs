use super::SnapshotAnalyzer;
use crate::application::harness::snapshot::{depth, file_name, parent_of, ScanSnapshot};
use crate::domain::harness::{Confidence, Evidence, Finding, FindingCategory, Origin};

/// Where the program starts. The file existing is a fact; what it is the entry *of* is read
/// from its neighbours (`angular.json` next to `main.ts`), and is only an inference when it
/// rests on the name alone.
pub struct EntryPointAnalyzer;

const MAX_ENTRY_DEPTH: usize = 3;

impl SnapshotAnalyzer for EntryPointAnalyzer {
    fn analyze(&self, s: &ScanSnapshot) -> Vec<Finding> {
        let mut out = Vec::new();
        for entry in s
            .entries
            .iter()
            .filter(|e| !e.is_dir && depth(&e.path) <= MAX_ENTRY_DEPTH)
        {
            if let Some(finding) = classify(s, &entry.path) {
                out.push(finding);
            }
        }
        out
    }
}

/// Is any file named like a project marker in this folder or one of its parents?
fn marker_above(s: &ScanSnapshot, path: &str, marker: impl Fn(&str) -> bool) -> Option<String> {
    let mut dir = parent_of(path);
    loop {
        if let Some(found) = s
            .entries
            .iter()
            .find(|e| !e.is_dir && parent_of(&e.path) == dir && marker(file_name(&e.path)))
        {
            return Some(found.path.clone());
        }
        if dir.is_empty() {
            return None;
        }
        dir = parent_of(dir);
    }
}

fn entry(path: &str, role: &str, confidence: Confidence, evidence: &[&str], why: &str) -> Finding {
    let origin = if confidence == Confidence::High {
        Origin::Fact
    } else {
        Origin::Inference
    };
    Finding::new(
        FindingCategory::EntryPoint,
        path,
        role,
        confidence,
        origin,
        path,
        "",
    )
    .with_evidence(evidence.iter().map(|e| Evidence::new(e, None)).collect())
    .with_reason(why)
    .with_label(&format!("{role} entry: {path}"))
}

fn classify(s: &ScanSnapshot, path: &str) -> Option<Finding> {
    let name = file_name(path);
    match name {
        "main.ts" => marker_above(s, path, |n| n == "angular.json").map(|angular| {
            entry(
                path,
                "Frontend (Angular)",
                Confidence::High,
                &[path, &angular],
                "main.ts bootstraps the app declared in angular.json",
            )
        }),
        "main.tsx" | "main.jsx" => Some(entry(
            path,
            "Frontend",
            Confidence::Medium,
            &[path],
            "main.tsx/main.jsx is the usual React entry file name",
        )),
        "Program.cs" => Some(match marker_above(s, path, |n| n.ends_with(".csproj")) {
            Some(project) => entry(
                path,
                "Backend (.NET)",
                Confidence::High,
                &[path, &project],
                "Program.cs next to a .NET project file",
            ),
            None => entry(
                path,
                "Application",
                Confidence::Medium,
                &[path],
                "Program.cs is the usual .NET entry file name",
            ),
        }),
        "main.rs" => marker_above(s, path, |n| n == "Cargo.toml").map(|cargo| {
            entry(
                path,
                "Binary (Rust)",
                Confidence::High,
                &[path, &cargo],
                "main.rs inside a Cargo package",
            )
        }),
        "main.go" => marker_above(s, path, |n| n == "go.mod").map(|module| {
            entry(
                path,
                "Application (Go)",
                Confidence::High,
                &[path, &module],
                "main.go inside a Go module",
            )
        }),
        "manage.py" => Some(entry(
            path,
            "Application (Django)",
            Confidence::Medium,
            &[path],
            "manage.py is Django's command entry",
        )),
        "server.ts" | "server.js" | "app.ts" | "app.js" | "index.ts" | "index.js"
            if path.starts_with("src/") && depth(path) == 1 =>
        {
            marker_above(s, path, |n| n == "package.json").map(|manifest| {
                entry(
                    path,
                    "Application (Node.js)",
                    Confidence::Medium,
                    &[path, &manifest],
                    "Conventional Node entry file name next to package.json",
                )
            })
        }
        _ => None,
    }
}
