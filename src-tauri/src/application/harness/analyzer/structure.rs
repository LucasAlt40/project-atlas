use std::collections::BTreeMap;

use serde_json::Value;

use super::SnapshotAnalyzer;
use crate::application::harness::snapshot::{depth, file_name, parent_of, ScanSnapshot};
use crate::domain::harness::{Confidence, Evidence, Finding, FindingCategory, Origin};

const FRONTEND_DIRS: &[&str] = &["frontend", "client", "web", "webapp", "ui"];
const BACKEND_DIRS: &[&str] = &["backend", "server", "api", "services"];
const LAYERED_DIRS: &[&str] = &[
    "controllers",
    "services",
    "repositories",
    "models",
    "handlers",
];
const MANIFESTS: &[&str] = &[
    "package.json",
    "Cargo.toml",
    "pom.xml",
    "go.mod",
    "pyproject.toml",
];

/// Reads the shape of the folder tree. Folder names alone are weak evidence, so a pattern is
/// only reported when several of its folders sit side by side, never above medium confidence,
/// and "unknown" is reported when nothing fits.
pub struct StructureAnalyzer;

impl SnapshotAnalyzer for StructureAnalyzer {
    fn analyze(&self, s: &ScanSnapshot) -> Vec<Finding> {
        let mut out = Vec::new();
        monorepo(s, &mut out);
        frontend_backend(s, &mut out);
        layers(s, &mut out);
        feature_based(s, &mut out);
        tests_directory(s, &mut out);
        let has_pattern = out
            .iter()
            .any(|f| f.category == FindingCategory::Architecture);
        if !has_pattern {
            out.push(
                Finding::new(
                    FindingCategory::Architecture,
                    "unknown",
                    "unknown",
                    Confidence::Low,
                    Origin::Generated,
                    "directory structure",
                    "",
                )
                .with_reason("No recognisable architectural pattern; Atlas does not guess one."),
            );
        }
        out
    }
}

/// An architectural finding. Facts are declared (a workspace file); everything else is an
/// inference that carries its reason and every folder or file it rests on.
fn pattern(
    key: &str,
    confidence: Confidence,
    reason: &str,
    evidence: &[impl AsRef<str>],
    origin: Origin,
) -> Finding {
    Finding::new(
        FindingCategory::Architecture,
        key,
        if origin == Origin::Fact {
            "true"
        } else {
            "possible"
        },
        confidence,
        origin,
        "directory structure",
        "",
    )
    .with_evidence(
        evidence
            .iter()
            .map(|e| Evidence::new(e.as_ref(), None))
            .collect(),
    )
    .with_reason(reason)
}

/// `parent/name` for each name (`parent` empty means the root).
fn join(parent: &str, names: &[&str]) -> Vec<String> {
    names
        .iter()
        .map(|n| {
            if parent.is_empty() {
                (*n).to_owned()
            } else {
                format!("{parent}/{n}")
            }
        })
        .collect()
}

fn monorepo(s: &ScanSnapshot, out: &mut Vec<Finding>) {
    for file in ["pnpm-workspace.yaml", "lerna.json", "nx.json", "turbo.json"] {
        if s.has_path(file) {
            out.push(pattern(
                "monorepo",
                Confidence::High,
                "A workspace tool is configured",
                &[file],
                Origin::Fact,
            ));
            return;
        }
    }
    let workspaces = s
        .files
        .get("package.json")
        .and_then(|t| serde_json::from_str::<Value>(t).ok())
        .is_some_and(|json| !json["workspaces"].is_null());
    let cargo_workspace = s
        .files
        .get("Cargo.toml")
        .is_some_and(|t| t.lines().any(|l| l.trim() == "[workspace]"));
    if workspaces || cargo_workspace {
        let source = if workspaces {
            "package.json"
        } else {
            "Cargo.toml"
        };
        out.push(pattern(
            "monorepo",
            Confidence::High,
            "Workspaces are declared in the manifest",
            &[source],
            Origin::Fact,
        ));
        return;
    }
    // Several subfolders that each carry their own manifest: probably several projects.
    let mut owners: BTreeMap<&str, &str> = BTreeMap::new();
    for entry in s
        .entries
        .iter()
        .filter(|e| !e.is_dir && depth(&e.path) >= 1)
    {
        if MANIFESTS.contains(&file_name(&entry.path)) || entry.path.ends_with(".csproj") {
            owners.entry(parent_of(&entry.path)).or_insert(&entry.path);
        }
    }
    if owners.len() >= 2 {
        let manifests: Vec<&str> = owners.values().copied().take(4).collect();
        out.push(pattern(
            "monorepo",
            Confidence::Medium,
            "Several subfolders each have their own project manifest",
            &manifests,
            Origin::Inference,
        ));
    }
}

fn frontend_backend(s: &ScanSnapshot, out: &mut Vec<Finding>) {
    let top = s.subdirs("");
    let find = |names: &[&str]| {
        top.iter()
            .find(|d| names.contains(&d.to_lowercase().as_str()))
            .copied()
    };
    if let (Some(front), Some(back)) = (find(FRONTEND_DIRS), find(BACKEND_DIRS)) {
        out.push(pattern(
            "frontend_backend_split",
            Confidence::Medium,
            "Separate frontend and backend folders at the root",
            &[front, back],
            Origin::Inference,
        ));
    }
}

/// Folders that sit side by side under the same parent are what make a layering claim.
fn layers(s: &ScanSnapshot, out: &mut Vec<Finding>) {
    let mut parents: Vec<&str> = vec![""];
    parents.extend(s.all_dirs().filter(|d| depth(d) <= 2));
    for parent in parents {
        let names: Vec<String> = s.subdirs(parent).iter().map(|d| d.to_lowercase()).collect();
        let has = |n: &str| names.iter().any(|x| x == n);
        if has("ports") && has("adapters") && has("domain") {
            out.push(pattern(
                "hexagonal",
                Confidence::Medium,
                "domain, ports and adapters folders side by side",
                &join(parent, &["domain", "ports", "adapters"]),
                Origin::Inference,
            ));
            return;
        }
        if has("domain") && has("application") && has("infrastructure") {
            out.push(pattern(
                "clean_architecture",
                Confidence::Medium,
                "domain, application and infrastructure folders side by side",
                &join(parent, &["domain", "application", "infrastructure"]),
                Origin::Inference,
            ));
            return;
        }
        let present: Vec<&str> = LAYERED_DIRS.iter().copied().filter(|n| has(n)).collect();
        if present.len() >= 3 {
            out.push(pattern(
                "layered",
                Confidence::Medium,
                "Several layer folders (controllers, services, repositories…) side by side",
                &join(parent, &present),
                Origin::Inference,
            ));
            return;
        }
    }
}

fn feature_based(s: &ScanSnapshot, out: &mut Vec<Finding>) {
    for dir in s.all_dirs().filter(|d| depth(d) <= 2) {
        let name = file_name(dir).to_lowercase();
        if name != "features" && name != "modules" {
            continue;
        }
        let count = s.subdirs(dir).len();
        if count >= 2 {
            out.push(pattern(
                "feature_based",
                if count >= 3 {
                    Confidence::Medium
                } else {
                    Confidence::Low
                },
                &format!("{count} feature folders under {dir}/"),
                &[dir],
                Origin::Inference,
            ));
            return;
        }
    }
}

fn tests_directory(s: &ScanSnapshot, out: &mut Vec<Finding>) {
    let dir = s
        .all_dirs()
        .find(|d| depth(d) <= 1 && ["tests", "test", "__tests__", "spec"].contains(&file_name(d)));
    if let Some(dir) = dir {
        out.push(Finding::new(
            FindingCategory::Testing,
            "tests_directory",
            dir,
            Confidence::High,
            Origin::Fact,
            dir,
            "test folder present",
        ));
    }
}
