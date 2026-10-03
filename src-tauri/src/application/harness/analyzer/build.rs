use serde_json::Value;

use super::{npm, SnapshotAnalyzer};
use crate::application::harness::snapshot::ScanSnapshot;
use crate::domain::harness::{Confidence, Evidence, Finding, FindingCategory, Origin};

/// How the project is built and tested, as *observed*: commands are read from manifests and
/// never run. A script that is declared is a fact; a command that merely follows from the
/// toolchain (`cargo test`) is an inference.
pub struct BuildAnalyzer;

impl SnapshotAnalyzer for BuildAnalyzer {
    fn analyze(&self, s: &ScanSnapshot) -> Vec<Finding> {
        let mut out = Vec::new();
        node_scripts(s, &mut out);
        toolchains(s, &mut out);
        out
    }
}

fn command(kind: &str, tool: &str, text: &str) -> Finding {
    let pretty = match kind {
        "test" => "Test command",
        "build" => "Build command",
        "dev" => "Dev command",
        "start" => "Start command",
        _ => "Lint command",
    };
    Finding::new(
        FindingCategory::Build,
        &format!("{kind}_{tool}"),
        text,
        Confidence::High,
        Origin::Fact,
        "",
        "",
    )
    .with_label(&format!("{pretty}: {text}"))
}

fn package_manager(s: &ScanSnapshot, manifest: &str) -> &'static str {
    let dir = manifest.rsplit_once('/').map_or("", |(d, _)| d);
    let has = |lock: &str| {
        let path = if dir.is_empty() {
            lock.to_owned()
        } else {
            format!("{dir}/{lock}")
        };
        s.has_path(&path) || s.has_path(lock)
    };
    if has("pnpm-lock.yaml") {
        "pnpm"
    } else if has("yarn.lock") {
        "yarn"
    } else {
        "npm"
    }
}

fn node_scripts(s: &ScanSnapshot, out: &mut Vec<Finding>) {
    for (path, json) in npm::manifests(s) {
        let Some(scripts) = json["scripts"].as_object() else {
            continue;
        };
        let pm = package_manager(s, path);
        for (kind, script) in [
            ("test", "test"),
            ("build", "build"),
            ("dev", "dev"),
            ("start", "start"),
            ("lint", "lint"),
        ] {
            if !scripts.get(script).is_some_and(Value::is_string) {
                continue;
            }
            let text = match (pm, script) {
                ("yarn", _) => format!("yarn {script}"),
                (_, "test" | "start") => format!("{pm} {script}"),
                _ => format!("{pm} run {script}"),
            };
            let mut finding = command(kind, "node", &text);
            finding.evidence = vec![Evidence::new(path, Some(&format!("scripts.{script}")))];
            out.push(finding);
        }
    }
}

fn toolchains(s: &ScanSnapshot, out: &mut Vec<Finding>) {
    let mut inferred = |kind: &str, tool: &str, text: &str, source: &str, why: &str| {
        let mut finding = command(kind, tool, text).with_reason(why);
        finding.confidence = Confidence::Medium;
        finding.origin = Origin::Inference;
        finding.evidence = vec![Evidence::new(source, None)];
        out.push(finding);
    };
    if let Some(manifest) = s.files_named("Cargo.toml", 2).first() {
        let why = "Standard Cargo command for a Cargo project";
        inferred("build", "cargo", "cargo build", manifest, why);
        inferred("test", "cargo", "cargo test", manifest, why);
    }
    if !s.files_with_extension("sln", 2).is_empty()
        || !s.files_with_extension("csproj", 3).is_empty()
    {
        let source = s
            .files_with_extension("sln", 2)
            .first()
            .or(s.files_with_extension("csproj", 3).first())
            .copied()
            .unwrap_or("");
        let why = "Standard .NET command for a .NET project";
        inferred("build", "dotnet", "dotnet build", source, why);
        inferred("test", "dotnet", "dotnet test", source, why);
    }
    if let Some(pom) = s.files_named("pom.xml", 2).first() {
        let why = "Standard Maven lifecycle command";
        inferred("build", "maven", "mvn package", pom, why);
        inferred("test", "maven", "mvn test", pom, why);
    }
    if let Some(gradle) = ["build.gradle", "build.gradle.kts"]
        .iter()
        .find_map(|n| s.files_named(n, 2).first().copied())
    {
        let tool = if s.has_path("gradlew") {
            "./gradlew"
        } else {
            "gradle"
        };
        let why = "Standard Gradle task";
        inferred("build", "gradle", &format!("{tool} build"), gradle, why);
        inferred("test", "gradle", &format!("{tool} test"), gradle, why);
    }
    if let Some(go) = s.files_named("go.mod", 2).first() {
        let why = "Standard Go command for a Go module";
        inferred("build", "go", "go build ./...", go, why);
        inferred("test", "go", "go test ./...", go, why);
    }
}
