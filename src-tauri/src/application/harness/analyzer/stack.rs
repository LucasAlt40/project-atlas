use serde_json::Value;

use super::SnapshotAnalyzer;
use crate::application::harness::snapshot::ScanSnapshot;
use crate::domain::harness::{Confidence, Finding, FindingCategory, Origin};

/// Manifests are looked for at the root and up to this many folders down (monorepo packages,
/// `src/App/App.csproj`).
const MANIFEST_DEPTH: usize = 2;
const PROJECT_FILE_DEPTH: usize = 3;

pub struct StackAnalyzer;

impl SnapshotAnalyzer for StackAnalyzer {
    fn analyze(&self, s: &ScanSnapshot) -> Vec<Finding> {
        let mut out = Vec::new();
        node(s, &mut out);
        rust(s, &mut out);
        dotnet(s, &mut out);
        java(s, &mut out);
        python(s, &mut out);
        go(s, &mut out);
        infrastructure(s, &mut out);
        environment(s, &mut out);
        out
    }
}

fn fact(
    category: FindingCategory,
    key: &str,
    value: &str,
    source: &str,
    evidence: &str,
) -> Finding {
    Finding::new(
        category,
        key,
        value,
        Confidence::High,
        Origin::Fact,
        source,
        evidence,
    )
}

fn inference(
    category: FindingCategory,
    key: &str,
    value: &str,
    confidence: Confidence,
    source: &str,
    evidence: &str,
) -> Finding {
    Finding::new(
        category,
        key,
        value,
        confidence,
        Origin::Inference,
        source,
        evidence,
    )
}

/// `^18.2.0` -> `18`; anything that is not a plain version (`workspace:*`, `latest`) -> `true`.
fn major_version(spec: &str) -> String {
    let trimmed = spec.trim_start_matches(|c: char| !c.is_ascii_digit());
    let major: String = trimmed.chars().take_while(char::is_ascii_digit).collect();
    if major.is_empty() {
        "true".to_owned()
    } else {
        major
    }
}

/// Any of `names` among the files at that depth, as the first path found.
fn any_file<'a>(s: &'a ScanSnapshot, names: &[&str], depth: usize) -> Option<&'a str> {
    names
        .iter()
        .find_map(|n| s.files_named(n, depth).into_iter().next())
}

/// The parsed `package.json` files of the project.
struct Packages<'a>(Vec<(&'a str, Value)>);

impl<'a> Packages<'a> {
    fn read(s: &'a ScanSnapshot) -> Self {
        Self(
            s.files_named("package.json", MANIFEST_DEPTH)
                .into_iter()
                .filter_map(|path| {
                    let json = serde_json::from_str::<Value>(s.files.get(path)?).ok()?;
                    Some((path, json))
                })
                .collect(),
        )
    }

    /// The first manifest that lists `name`, the dependency's major version and where it is listed.
    fn dependency(&self, name: &str) -> Option<(&'a str, String, String)> {
        self.0.iter().find_map(|(path, json)| {
            ["dependencies", "devDependencies"]
                .iter()
                .find_map(|section| {
                    json[section]
                        .get(name)
                        .and_then(Value::as_str)
                        .map(|spec| (*path, major_version(spec), format!("{section}[\"{name}\"]")))
                })
        })
    }
}

fn node(s: &ScanSnapshot, out: &mut Vec<Finding>) {
    let packages = Packages::read(s);
    let Some(first) = s
        .files_named("package.json", MANIFEST_DEPTH)
        .first()
        .copied()
    else {
        return;
    };
    let dependency = |name: &str| packages.dependency(name);
    let engine = packages
        .0
        .iter()
        .find_map(|(_, json)| json["engines"]["node"].as_str())
        .map_or_else(|| "true".to_owned(), major_version);
    out.push(fact(
        FindingCategory::Runtime,
        "nodejs",
        &engine,
        first,
        "package.json",
    ));

    for (lock, key) in [
        ("package-lock.json", "npm"),
        ("pnpm-lock.yaml", "pnpm"),
        ("yarn.lock", "yarn"),
        ("bun.lockb", "bun"),
    ] {
        if let Some(path) = any_file(s, &[lock], MANIFEST_DEPTH) {
            out.push(fact(
                FindingCategory::PackageManager,
                key,
                "true",
                path,
                lock,
            ));
        }
    }

    if let Some(path) = any_file(s, &["tsconfig.json"], MANIFEST_DEPTH) {
        out.push(fact(
            FindingCategory::Language,
            "typescript",
            "true",
            path,
            "tsconfig.json",
        ));
    } else if let Some((path, version, field)) = dependency("typescript") {
        out.push(fact(
            FindingCategory::Language,
            "typescript",
            &version,
            path,
            &field,
        ));
    } else {
        out.push(inference(
            FindingCategory::Language,
            "javascript",
            "true",
            Confidence::Medium,
            first,
            "package.json without TypeScript",
        ));
    }

    node_frameworks(s, &packages, out);
    node_tooling(s, &packages, out);
}

fn node_frameworks(s: &ScanSnapshot, packages: &Packages, out: &mut Vec<Finding>) {
    let dependency = |name: &str| packages.dependency(name);
    for (dep, key) in [
        ("@angular/core", "angular"),
        ("react", "react"),
        ("vue", "vue"),
        ("svelte", "svelte"),
        ("next", "nextjs"),
        ("nuxt", "nuxt"),
        ("@nestjs/core", "nestjs"),
        ("express", "express"),
        ("@tauri-apps/api", "tauri"),
    ] {
        if let Some((path, version, field)) = dependency(dep) {
            out.push(fact(
                FindingCategory::Framework,
                key,
                &version,
                path,
                &field,
            ));
        }
    }
    if let Some(path) = any_file(s, &["angular.json"], MANIFEST_DEPTH) {
        out.push(fact(
            FindingCategory::Framework,
            "angular",
            "true",
            path,
            "angular.json",
        ));
    }
    for (file, key) in [("next.config.js", "nextjs"), ("next.config.mjs", "nextjs")] {
        if let Some(path) = any_file(s, &[file], MANIFEST_DEPTH) {
            out.push(fact(FindingCategory::Framework, key, "true", path, file));
        }
    }

    for (dep, key) in [
        ("pg", "postgresql"),
        ("mysql2", "mysql"),
        ("mongodb", "mongodb"),
        ("mongoose", "mongodb"),
        ("redis", "redis"),
        ("ioredis", "redis"),
        ("better-sqlite3", "sqlite"),
    ] {
        if let Some((path, _, field)) = dependency(dep) {
            out.push(fact(FindingCategory::Database, key, "true", path, &field));
        }
    }
}

fn node_tooling(s: &ScanSnapshot, packages: &Packages, out: &mut Vec<Finding>) {
    let dependency = |name: &str| packages.dependency(name);
    for (dep, key) in [
        ("vitest", "vitest"),
        ("jest", "jest"),
        ("@playwright/test", "playwright"),
        ("cypress", "cypress"),
        ("mocha", "mocha"),
    ] {
        if let Some((path, version, field)) = dependency(dep) {
            out.push(fact(FindingCategory::Testing, key, &version, path, &field));
        }
    }
    for (dep, key) in [
        ("vite", "vite"),
        ("eslint", "eslint"),
        ("prettier", "prettier"),
    ] {
        if let Some((path, version, field)) = dependency(dep) {
            out.push(fact(FindingCategory::Tooling, key, &version, path, &field));
        }
    }
    for (prefix, key) in [("vite.config.", "vite"), ("eslint.config.", "eslint")] {
        if let Some(entry) = s
            .entries
            .iter()
            .find(|e| !e.is_dir && e.path.starts_with(prefix))
        {
            out.push(fact(
                FindingCategory::Tooling,
                key,
                "true",
                &entry.path,
                &entry.path,
            ));
        }
    }
    if let Some(entry) = s.entries.iter().find(|e| {
        !e.is_dir && (e.path.starts_with(".prettierrc") || e.path.starts_with("prettier.config."))
    }) {
        out.push(fact(
            FindingCategory::Tooling,
            "prettier",
            "true",
            &entry.path,
            &entry.path,
        ));
    }
    if s.has_path(".editorconfig") {
        out.push(fact(
            FindingCategory::Tooling,
            "editorconfig",
            "true",
            ".editorconfig",
            ".editorconfig present",
        ));
    }
}

fn rust(s: &ScanSnapshot, out: &mut Vec<Finding>) {
    let Some(manifest) = any_file(s, &["Cargo.toml"], MANIFEST_DEPTH) else {
        return;
    };
    out.push(fact(
        FindingCategory::Language,
        "rust",
        "true",
        manifest,
        "Cargo.toml",
    ));
    out.push(fact(
        FindingCategory::PackageManager,
        "cargo",
        "true",
        manifest,
        "Cargo.toml",
    ));
    out.push(inference(
        FindingCategory::Testing,
        "cargo_test",
        "true",
        Confidence::Medium,
        manifest,
        "Rust projects use the built-in test harness (cargo test)",
    ));
    let text = s.files.get(manifest).map_or("", String::as_str);
    if text.lines().any(|l| {
        let l = l.trim_start();
        l.starts_with("tauri =") || l.starts_with("tauri-build")
    }) {
        out.push(fact(
            FindingCategory::Framework,
            "tauri",
            "true",
            manifest,
            "tauri dependency",
        ));
    }
    for name in ["rustfmt.toml", ".rustfmt.toml"] {
        if s.has_path(name) {
            out.push(fact(
                FindingCategory::Tooling,
                "rustfmt",
                "true",
                name,
                name,
            ));
        }
    }
}

/// One project file per concern keeps this a single readable pass, so it is longer than usual.
#[allow(clippy::too_many_lines)]
fn dotnet(s: &ScanSnapshot, out: &mut Vec<Finding>) {
    let projects = s.files_with_extension("csproj", PROJECT_FILE_DEPTH);
    let fsharp = s.files_with_extension("fsproj", PROJECT_FILE_DEPTH);
    let solutions = s.files_with_extension("sln", MANIFEST_DEPTH);
    let Some(first) = projects
        .first()
        .or(fsharp.first())
        .or(solutions.first())
        .copied()
    else {
        return;
    };
    let language = if projects.is_empty() && !fsharp.is_empty() {
        "fsharp"
    } else {
        "csharp"
    };
    out.push(fact(
        FindingCategory::Language,
        language,
        "true",
        first,
        first,
    ));
    let texts: Vec<(&str, &str)> = projects
        .iter()
        .chain(fsharp.iter())
        .filter_map(|p| Some((*p, s.files.get(*p)?.as_str())))
        .collect();
    let target = texts.iter().find_map(|(path, text)| {
        let start = text.find("<TargetFramework>")? + "<TargetFramework>".len();
        let end = text[start..].find('<')? + start;
        Some((*path, text[start..end].to_owned()))
    });
    let sdk = any_file(s, &["global.json"], 0).and_then(|path| {
        let json: Value = serde_json::from_str(s.files.get(path)?).ok()?;
        Some((path, json["sdk"]["version"].as_str()?.to_owned()))
    });
    match (target, sdk) {
        (Some((path, framework)), _) => out.push(fact(
            FindingCategory::Runtime,
            "dotnet",
            framework.trim_start_matches("net"),
            path,
            &format!("TargetFramework {framework}"),
        )),
        (None, Some((path, version))) => out.push(fact(
            FindingCategory::Runtime,
            "dotnet",
            &major_version(&version),
            path,
            "global.json sdk",
        )),
        (None, None) => out.push(fact(
            FindingCategory::Runtime,
            "dotnet",
            "true",
            first,
            first,
        )),
    }
    if let Some((path, _)) = texts
        .iter()
        .find(|(_, t)| t.contains("Microsoft.NET.Sdk.Web"))
    {
        out.push(fact(
            FindingCategory::Framework,
            "aspnet",
            "true",
            path,
            "Microsoft.NET.Sdk.Web",
        ));
    }
    let references = |needle: &str| {
        texts
            .iter()
            .find(|(_, t)| t.contains(needle))
            .map(|(p, _)| *p)
    };
    for (needle, category, key) in [
        (
            "EntityFrameworkCore.SqlServer",
            FindingCategory::Database,
            "sqlserver",
        ),
        ("Npgsql", FindingCategory::Database, "postgresql"),
        (
            "EntityFrameworkCore.Sqlite",
            FindingCategory::Database,
            "sqlite",
        ),
        ("MySql", FindingCategory::Database, "mysql"),
        ("xunit", FindingCategory::Testing, "xunit"),
        ("NUnit", FindingCategory::Testing, "nunit"),
        ("MSTest", FindingCategory::Testing, "mstest"),
    ] {
        if let Some(path) = references(needle) {
            out.push(fact(category, key, "true", path, needle));
        }
    }
    if s.has_path("Directory.Build.props") {
        out.push(fact(
            FindingCategory::Tooling,
            "msbuild_props",
            "true",
            "Directory.Build.props",
            "Directory.Build.props present",
        ));
    }
}

fn java(s: &ScanSnapshot, out: &mut Vec<Finding>) {
    let pom = any_file(s, &["pom.xml"], MANIFEST_DEPTH);
    let gradle = any_file(
        s,
        &[
            "build.gradle",
            "build.gradle.kts",
            "settings.gradle",
            "settings.gradle.kts",
        ],
        MANIFEST_DEPTH,
    );
    let Some(first) = pom.or(gradle) else {
        return;
    };
    out.push(fact(
        FindingCategory::Language,
        "java",
        "true",
        first,
        first,
    ));
    out.push(fact(FindingCategory::Runtime, "jvm", "true", first, first));
    if let Some(path) = pom {
        out.push(fact(
            FindingCategory::PackageManager,
            "maven",
            "true",
            path,
            "pom.xml",
        ));
    }
    if let Some(path) = gradle {
        out.push(fact(
            FindingCategory::PackageManager,
            "gradle",
            "true",
            path,
            path,
        ));
    }
    for path in pom.into_iter().chain(gradle) {
        let text = s
            .files
            .get(path)
            .map_or(String::new(), |t| t.to_lowercase());
        if text.contains("spring-boot") {
            out.push(fact(
                FindingCategory::Framework,
                "spring_boot",
                "true",
                path,
                "spring-boot",
            ));
        }
        if text.contains("junit") {
            out.push(fact(
                FindingCategory::Testing,
                "junit",
                "true",
                path,
                "junit",
            ));
        }
    }
}

fn python(s: &ScanSnapshot, out: &mut Vec<Finding>) {
    let Some(first) = any_file(
        s,
        &["pyproject.toml", "requirements.txt", "setup.py"],
        MANIFEST_DEPTH,
    ) else {
        return;
    };
    out.push(fact(
        FindingCategory::Language,
        "python",
        "true",
        first,
        first,
    ));
    let pyproject = any_file(s, &["pyproject.toml"], MANIFEST_DEPTH);
    let requirements = any_file(s, &["requirements.txt"], MANIFEST_DEPTH);
    let text_of = |path: Option<&str>| {
        path.and_then(|p| s.files.get(p))
            .map_or(String::new(), |t| t.to_lowercase())
    };
    let (py_text, req_text) = (text_of(pyproject), text_of(requirements));
    let poetry = py_text.contains("[tool.poetry]");
    if let (true, Some(path)) = (poetry, pyproject) {
        out.push(fact(
            FindingCategory::PackageManager,
            "poetry",
            "true",
            path,
            "[tool.poetry]",
        ));
    } else if let Some(path) = requirements {
        out.push(fact(
            FindingCategory::PackageManager,
            "pip",
            "true",
            path,
            "requirements.txt",
        ));
    }
    for (name, key, category) in [
        ("django", "django", FindingCategory::Framework),
        ("flask", "flask", FindingCategory::Framework),
        ("fastapi", "fastapi", FindingCategory::Framework),
        ("pytest", "pytest", FindingCategory::Testing),
    ] {
        for (path, text) in [(pyproject, &py_text), (requirements, &req_text)] {
            if let (Some(path), true) = (path, text.contains(name)) {
                out.push(inference(
                    category,
                    key,
                    "true",
                    Confidence::Medium,
                    path,
                    &format!("{name} mentioned in {path}"),
                ));
                break;
            }
        }
    }
}

fn go(s: &ScanSnapshot, out: &mut Vec<Finding>) {
    let Some(path) = any_file(s, &["go.mod"], MANIFEST_DEPTH) else {
        return;
    };
    let version = s
        .files
        .get(path)
        .and_then(|t| {
            t.lines()
                .find_map(|l| l.trim().strip_prefix("go ").map(|v| v.trim().to_owned()))
        })
        .unwrap_or_else(|| "true".to_owned());
    out.push(fact(
        FindingCategory::Language,
        "go",
        &version,
        path,
        "go.mod",
    ));
    out.push(fact(
        FindingCategory::PackageManager,
        "go_modules",
        "true",
        path,
        "go.mod",
    ));
    if let Some(entry) = s
        .entries
        .iter()
        .find(|e| !e.is_dir && e.path.ends_with("_test.go"))
    {
        out.push(fact(
            FindingCategory::Testing,
            "go_test",
            "true",
            &entry.path,
            &entry.path,
        ));
    }
}

fn infrastructure(s: &ScanSnapshot, out: &mut Vec<Finding>) {
    if let Some(path) = any_file(s, &["Dockerfile"], MANIFEST_DEPTH) {
        out.push(fact(
            FindingCategory::Infrastructure,
            "docker",
            "true",
            path,
            "Dockerfile",
        ));
    }
    let compose: Vec<&str> = s
        .entries
        .iter()
        .filter(|e| {
            !e.is_dir && crate::application::harness::snapshot::depth(&e.path) <= MANIFEST_DEPTH
        })
        .map(|e| e.path.as_str())
        .filter(|p| {
            let name = crate::application::harness::snapshot::file_name(p).to_lowercase();
            (name.starts_with("docker-compose") || name.starts_with("compose."))
                && std::path::Path::new(&name).extension().is_some_and(|e| {
                    e.eq_ignore_ascii_case("yml") || e.eq_ignore_ascii_case("yaml")
                })
        })
        .collect();
    if let Some(first) = compose.first() {
        out.push(fact(
            FindingCategory::Infrastructure,
            "docker_compose",
            "true",
            first,
            first,
        ));
    }
    // Only the image names are read from compose files, never their environment.
    for path in &compose {
        let Some(text) = s.files.get(*path) else {
            continue;
        };
        for line in text.lines() {
            let Some(image) = line.trim().strip_prefix("image:") else {
                continue;
            };
            let image = image.trim().trim_matches(['"', '\'']).to_lowercase();
            for (needle, key) in [
                ("postgres", "postgresql"),
                ("mariadb", "mariadb"),
                ("mysql", "mysql"),
                ("mongo", "mongodb"),
                ("redis", "redis"),
                ("mssql", "sqlserver"),
            ] {
                if image.contains(needle) {
                    out.push(fact(
                        FindingCategory::Database,
                        key,
                        "true",
                        path,
                        &format!("image {image}"),
                    ));
                }
            }
        }
    }
    if let Some(path) = s.files_with_extension("tf", 1).first() {
        out.push(fact(
            FindingCategory::Infrastructure,
            "terraform",
            "true",
            path,
            path,
        ));
    }
    if let Some(path) = any_file(s, &["Chart.yaml"], MANIFEST_DEPTH) {
        out.push(fact(
            FindingCategory::Infrastructure,
            "helm",
            "true",
            path,
            "Chart.yaml",
        ));
    }
}

/// `.env` files are only noticed by name: their content is never read or kept.
fn environment(s: &ScanSnapshot, out: &mut Vec<Finding>) {
    if let Some(entry) = s
        .entries
        .iter()
        .find(|e| !e.is_dir && (e.path == ".env" || e.path.starts_with(".env.")))
    {
        out.push(fact(
            FindingCategory::Environment,
            "env_file",
            "true",
            &entry.path,
            ".env detected",
        ));
    }
}
