use super::*;
use crate::domain::harness::{Confidence, Evidence, FindingCategory, Origin};

fn snapshot(entries: &[&str]) -> ScanSnapshot {
    let mut s = ScanSnapshot {
        root_name: "demo".to_owned(),
        ..ScanSnapshot::default()
    };
    for entry in entries {
        match entry.strip_suffix('/') {
            Some(dir) => s.dir(dir),
            None => s.file(entry, None),
        }
    }
    s
}

fn with_file(mut s: ScanSnapshot, path: &str, content: &str) -> ScanSnapshot {
    s.files.insert(path.to_owned(), content.to_owned());
    s
}

fn find<'a>(findings: &'a [Finding], id: &str) -> Option<&'a Finding> {
    findings.iter().find(|f| f.id == id)
}

fn analyze(s: &ScanSnapshot) -> Vec<Finding> {
    DeterministicAnalyzer::default().analyze(s)
}

#[test]
fn angular_is_a_high_confidence_fact_with_its_version_and_evidence() {
    let s = with_file(
        snapshot(&[
            "package.json",
            "angular.json",
            "tsconfig.json",
            "package-lock.json",
        ]),
        "package.json",
        r#"{"dependencies":{"@angular/core":"^18.2.0"},"devDependencies":{"vitest":"^2.0.0","eslint":"9"}}"#,
    );

    let findings = analyze(&s);

    let angular = find(&findings, "framework:angular").unwrap();
    assert_eq!(angular.value, "18");
    assert_eq!(angular.label, "Angular 18");
    assert_eq!(angular.confidence, Confidence::High);
    assert_eq!(angular.origin, Origin::Fact);
    assert_eq!(
        angular.evidence,
        [Evidence::new(
            "package.json",
            Some("dependencies[\"@angular/core\"]")
        )]
    );
    assert!(find(&findings, "language:typescript").is_some());
    assert!(find(&findings, "runtime:nodejs").is_some());
    assert!(find(&findings, "package_manager:npm").is_some());
    assert!(find(&findings, "testing:vitest").is_some());
    assert!(find(&findings, "tooling:eslint").is_some());
}

#[test]
fn react_and_vue_come_from_dependencies() {
    let react = with_file(
        snapshot(&["package.json", "vite.config.ts"]),
        "package.json",
        r#"{"dependencies":{"react":"19.1.0"}}"#,
    );
    let vue = with_file(
        snapshot(&["package.json"]),
        "package.json",
        r#"{"dependencies":{"vue":"~3.4.0"}}"#,
    );

    let react = analyze(&react);
    assert_eq!(find(&react, "framework:react").unwrap().value, "19");
    assert!(find(&react, "tooling:vite").is_some());
    assert!(find(&react, "language:javascript").is_some());
    assert_eq!(
        find(&analyze(&vue), "framework:vue").unwrap().label,
        "Vue 3"
    );
}

#[test]
fn dotnet_is_read_from_project_files() {
    let s = with_file(
        snapshot(&[
            "App.sln",
            "src/",
            "src/Web/",
            "src/Web/Web.csproj",
            "global.json",
        ]),
        "src/Web/Web.csproj",
        r#"<Project Sdk="Microsoft.NET.Sdk.Web"><PropertyGroup><TargetFramework>net10.0</TargetFramework></PropertyGroup>
<ItemGroup><PackageReference Include="Microsoft.EntityFrameworkCore.SqlServer" /><PackageReference Include="xunit" /></ItemGroup></Project>"#,
    );

    let findings = analyze(&s);

    assert_eq!(find(&findings, "runtime:dotnet").unwrap().value, "10.0");
    assert!(find(&findings, "language:csharp").is_some());
    assert!(find(&findings, "framework:aspnet").is_some());
    assert!(find(&findings, "database:sqlserver").is_some());
    assert!(find(&findings, "testing:xunit").is_some());
}

#[test]
fn java_rust_python_and_go_are_recognised() {
    let java = with_file(
        snapshot(&["pom.xml"]),
        "pom.xml",
        "<artifactId>spring-boot-starter</artifactId><artifactId>junit-jupiter</artifactId>",
    );
    let java = analyze(&java);
    assert!(find(&java, "language:java").is_some());
    assert!(find(&java, "package_manager:maven").is_some());
    assert!(find(&java, "framework:spring_boot").is_some());
    assert!(find(&java, "testing:junit").is_some());

    let rust = with_file(
        snapshot(&["Cargo.toml", "src-tauri/"]),
        "Cargo.toml",
        "[dependencies]\ntauri = \"2\"\n",
    );
    let rust = analyze(&rust);
    assert!(find(&rust, "language:rust").is_some());
    assert!(find(&rust, "framework:tauri").is_some());
    // `cargo test` is how Rust projects test, but it is a conclusion, not something read.
    let cargo_test = find(&rust, "testing:cargo_test").unwrap();
    assert_eq!(cargo_test.origin, Origin::Inference);
    assert_eq!(cargo_test.confidence, Confidence::Medium);

    let python = with_file(
        snapshot(&["pyproject.toml"]),
        "pyproject.toml",
        "[tool.poetry]\ndependencies = { fastapi = \"*\" }\n[tool.pytest]\n",
    );
    let python = analyze(&python);
    assert!(find(&python, "language:python").is_some());
    assert!(find(&python, "package_manager:poetry").is_some());
    assert_eq!(
        find(&python, "framework:fastapi").unwrap().confidence,
        Confidence::Medium
    );

    let go = with_file(
        snapshot(&["go.mod", "main_test.go"]),
        "go.mod",
        "module x\n\ngo 1.22\n",
    );
    let go = analyze(&go);
    assert_eq!(find(&go, "language:go").unwrap().value, "1.22");
    assert!(find(&go, "testing:go_test").is_some());
}

#[test]
fn git_docker_and_ci_are_detected() {
    let mut s = snapshot(&[
        ".git/",
        ".gitignore",
        "Dockerfile",
        "docker-compose.yml",
        ".github/",
        ".github/workflows/",
        ".github/workflows/ci.yml",
        ".gitlab-ci.yml",
        "Jenkinsfile",
    ]);
    s.git_head = Some("ref: refs/heads/main".to_owned());
    let s = with_file(
        s,
        "docker-compose.yml",
        "services:\n  db:\n    image: postgres:16\n    environment:\n      POSTGRES_PASSWORD: hunter2\n",
    );

    let findings = analyze(&s);

    assert_eq!(
        find(&findings, "repository:git").unwrap().confidence,
        Confidence::High
    );
    assert_eq!(
        find(&findings, "repository:current_branch").unwrap().value,
        "main"
    );
    assert!(find(&findings, "infrastructure:docker").is_some());
    assert!(find(&findings, "infrastructure:docker_compose").is_some());
    assert!(find(&findings, "database:postgresql").is_some());
    let actions = find(&findings, "ci:github_actions").unwrap();
    assert_eq!(
        actions.evidence,
        [Evidence::new(".github/workflows/", Some("ci.yml"))]
    );
    assert!(find(&findings, "ci:gitlab_ci").is_some());
    assert!(find(&findings, "ci:jenkins").is_some());
    // The compose file's environment is never carried into a finding.
    assert!(findings
        .iter()
        .all(|f| !format!("{:?}", f.evidence).contains("hunter2")));
}

#[test]
fn a_detached_head_is_not_called_a_branch() {
    let mut s = snapshot(&[".git/"]);
    s.git_head = Some("0123456789abcdef0123456789abcdef01234567".to_owned());

    assert_eq!(
        find(&analyze(&s), "repository:current_branch")
            .unwrap()
            .value,
        "(detached)"
    );
}

#[test]
fn env_files_are_noted_by_name_and_nothing_more() {
    let findings = analyze(&snapshot(&[".env", "package.json"]));

    let env = find(&findings, "environment:env_file").unwrap();
    assert_eq!(env.evidence[0].field.as_deref(), Some(".env detected"));
    assert_eq!(env.value, "true");
}

#[test]
fn clean_layering_needs_the_folders_side_by_side_and_is_only_possible() {
    let s = snapshot(&[
        "src/",
        "src/domain/",
        "src/application/",
        "src/infrastructure/",
    ]);

    let findings = analyze(&s);

    let clean = find(&findings, "architecture:clean_architecture").unwrap();
    assert_eq!(clean.value, "possible");
    assert_eq!(clean.confidence, Confidence::Medium);
    assert_eq!(clean.origin, Origin::Inference);
    assert!(find(&findings, "architecture:unknown").is_none());
}

#[test]
fn similar_folder_names_alone_do_not_make_an_architecture() {
    // `domain` and `application` without `infrastructure` is not enough to claim anything.
    let findings = analyze(&snapshot(&["domain/", "application/", "src/"]));

    assert!(find(&findings, "architecture:clean_architecture").is_none());
    let unknown = find(&findings, "architecture:unknown").unwrap();
    assert_eq!(unknown.value, "unknown");
    assert_eq!(unknown.confidence, Confidence::Low);
}

#[test]
fn feature_folders_hexagonal_layered_and_frontend_backend_are_recognised() {
    let feature = analyze(&snapshot(&[
        "src/",
        "src/features/",
        "src/features/a/",
        "src/features/b/",
        "src/features/c/",
    ]));
    assert_eq!(
        find(&feature, "architecture:feature_based")
            .unwrap()
            .confidence,
        Confidence::Medium
    );

    let hex = analyze(&snapshot(&["domain/", "ports/", "adapters/"]));
    assert!(find(&hex, "architecture:hexagonal").is_some());

    let layered = analyze(&snapshot(&["controllers/", "services/", "repositories/"]));
    assert!(find(&layered, "architecture:layered").is_some());

    let split = analyze(&snapshot(&["frontend/", "backend/"]));
    assert!(find(&split, "architecture:frontend_backend_split").is_some());
}

#[test]
fn monorepos_are_a_fact_when_declared_and_an_inference_when_only_guessed() {
    let declared = with_file(
        snapshot(&["package.json", "pnpm-workspace.yaml", "apps/", "apps/a/"]),
        "package.json",
        "{}",
    );
    let monorepo = find(&analyze(&declared), "architecture:monorepo")
        .cloned()
        .unwrap();
    assert_eq!(monorepo.confidence, Confidence::High);
    assert_eq!(monorepo.origin, Origin::Fact);

    let guessed = analyze(&snapshot(&[
        "web/",
        "web/package.json",
        "api/",
        "api/package.json",
    ]));
    let monorepo = find(&guessed, "architecture:monorepo").unwrap();
    assert_eq!(monorepo.confidence, Confidence::Medium);
    assert_eq!(monorepo.origin, Origin::Inference);

    // One nested manifest next to a root manifest is an app with a part, not a monorepo.
    let single = analyze(&snapshot(&[
        "package.json",
        "src-tauri/",
        "src-tauri/Cargo.toml",
    ]));
    assert!(find(&single, "architecture:monorepo").is_none());
}

#[test]
fn an_unknown_project_reports_unknown_architecture_and_nothing_invented() {
    let findings = analyze(&snapshot(&["notes.txt", "docs/"]));

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].id, "architecture:unknown");
    assert_eq!(findings[0].category, FindingCategory::Architecture);
}

#[test]
fn findings_are_unique_and_ordered_by_category() {
    let s = with_file(
        snapshot(&["package.json", "angular.json"]),
        "package.json",
        r#"{"dependencies":{"@angular/core":"18"}}"#,
    );

    let findings = analyze(&s);

    let mut ids: Vec<&str> = findings.iter().map(|f| f.id.as_str()).collect();
    let count = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), count);
    assert!(findings.windows(2).all(|w| w[0].category <= w[1].category));
}

// ---- V0.7.1: entry points, commands, dependencies, data, integrations, testing ----

#[test]
fn entry_points_say_what_they_are_the_entry_of_only_when_a_neighbour_confirms_it() {
    let angular = snapshot(&["angular.json", "src/", "src/main.ts", "package.json"]);
    let stray = snapshot(&["src/", "src/main.ts"]);
    let dotnet = snapshot(&["Api/", "Api/Program.cs", "Api/Api.csproj"]);
    let rust = snapshot(&["Cargo.toml", "src/", "src/main.rs"]);
    let react = snapshot(&["src/", "src/main.tsx"]);

    let a = analyze(&angular);
    let entry = find(&a, "entry_point:src/main.ts").unwrap();
    assert_eq!(
        (entry.value.as_str(), entry.confidence, entry.origin),
        ("Frontend (Angular)", Confidence::High, Origin::Fact)
    );
    assert_eq!(entry.evidence.len(), 2);
    assert!(find(&analyze(&stray), "entry_point:src/main.ts").is_none());
    assert_eq!(
        find(&analyze(&dotnet), "entry_point:Api/Program.cs")
            .unwrap()
            .value,
        "Backend (.NET)"
    );
    assert_eq!(
        find(&analyze(&rust), "entry_point:src/main.rs")
            .unwrap()
            .value,
        "Binary (Rust)"
    );
    let react = analyze(&react);
    let main = find(&react, "entry_point:src/main.tsx").unwrap();
    assert_eq!(
        (main.confidence, main.origin),
        (Confidence::Medium, Origin::Inference)
    );
}

#[test]
fn declared_commands_are_facts_and_toolchain_commands_are_inferences_and_nothing_is_run() {
    let node = with_file(
        snapshot(&["package.json", "pnpm-lock.yaml"]),
        "package.json",
        r#"{"scripts":{"test":"vitest run","build":"vite build","lint":"eslint .","weird":"x","dev":42}}"#,
    );
    let n = analyze(&node);
    assert_eq!(find(&n, "build:test_node").unwrap().value, "pnpm test");
    assert_eq!(
        find(&n, "build:build_node").unwrap().value,
        "pnpm run build"
    );
    assert_eq!(
        find(&n, "build:lint_node").unwrap().evidence[0]
            .field
            .as_deref(),
        Some("scripts.lint")
    );
    assert!(find(&n, "build:dev_node").is_none());
    // The script's body is not copied into the Harness.
    assert!(n.iter().all(|f| !format!("{f:?}").contains("vite build")));

    let rust = with_file(snapshot(&["Cargo.toml"]), "Cargo.toml", "[package]");
    let r = analyze(&rust);
    let cargo = find(&r, "build:test_cargo").unwrap();
    assert_eq!(
        (cargo.value.as_str(), cargo.origin, cargo.confidence),
        ("cargo test", Origin::Inference, Confidence::Medium)
    );
    assert!(cargo.reason.is_some());
    let gradle = analyze(&snapshot(&["build.gradle", "gradlew"]));
    assert_eq!(
        find(&gradle, "build:test_gradle").unwrap().value,
        "./gradlew test"
    );
}

#[test]
fn key_dependencies_are_facts_with_the_field_and_integrations_are_only_possible() {
    let s = with_file(
        snapshot(&[
            "package.json",
            "src/",
            "src/controllers/",
            "prisma/",
            "prisma/migrations/",
        ]),
        "package.json",
        r#"{"dependencies":{"express":"4","primeng":"^17.2.0","rxjs":"7","prisma":"5","kafkajs":"2","left-pad":"1"}}"#,
    );

    let f = analyze(&s);

    let primeng = find(&f, "dependency:primeng").unwrap();
    assert_eq!(
        (primeng.value.as_str(), primeng.origin),
        ("17", Origin::Fact)
    );
    assert_eq!(
        primeng.evidence[0].field.as_deref(),
        Some("dependencies[\"primeng\"]")
    );
    assert!(find(&f, "dependency:left_pad").is_none());
    assert!(find(&f, "data:prisma").is_some());
    assert_eq!(find(&f, "data:migrations").unwrap().origin, Origin::Fact);
    let kafka = find(&f, "integration:kafka").unwrap();
    assert_eq!(
        (kafka.origin, kafka.confidence),
        (Origin::Inference, Confidence::Medium)
    );
    let rest = find(&f, "integration:rest_api").unwrap();
    assert_eq!(rest.label, "Possible REST API");
    assert_eq!(rest.origin, Origin::Inference);
    let sources: Vec<&str> = rest.evidence.iter().map(|e| e.source.as_str()).collect();
    assert_eq!(sources, ["package.json", "src/controllers"]);
}

#[test]
fn dotnet_rust_java_dependencies_and_a_dependency_free_project() {
    let csproj = with_file(
        snapshot(&["Api/", "Api/Api.csproj"]),
        "Api/Api.csproj",
        r#"<Project Sdk="Microsoft.NET.Sdk.Web"><PackageReference Include="Microsoft.EntityFrameworkCore.SqlServer"/><PackageReference Include="Serilog"/><PackageReference Include="MassTransit"/></Project>"#,
    );
    let d = analyze(&csproj);
    assert!(find(&d, "data:entity_framework").is_some());
    assert!(find(&d, "dependency:serilog").is_some());
    assert_eq!(
        find(&d, "integration:masstransit").unwrap().origin,
        Origin::Inference
    );
    assert!(find(&d, "integration:rest_api").is_some());

    let cargo = with_file(
        snapshot(&["Cargo.toml"]),
        "Cargo.toml",
        "[dependencies]\nsqlx = \"0.8\"\naxum = \"0.7\"\n",
    );
    let c = analyze(&cargo);
    assert!(find(&c, "data:sqlx").is_some() && find(&c, "integration:rest_api").is_some());

    let bare = analyze(&snapshot(&["notes.txt"]));
    assert!(bare.iter().all(|f| !matches!(
        f.category,
        FindingCategory::Dependency | FindingCategory::Data | FindingCategory::Integration
    )));
}

#[test]
fn testing_is_described_in_separate_facts_and_a_framework_alone_proves_nothing() {
    let installed_only = with_file(
        snapshot(&["package.json"]),
        "package.json",
        r#"{"devDependencies":{"vitest":"^3.0.0"}}"#,
    );
    let f = analyze(&installed_only);
    assert!(find(&f, "testing:vitest").is_some());
    assert!(find(&f, "testing:test_files").is_none());
    assert!(find(&f, "testing:coverage_config").is_none());
    assert!(find(&f, "convention:test_file_pattern").is_none());

    let full = with_file(
        snapshot(&[
            "package.json",
            "src/",
            "src/a.spec.ts",
            "src/b.spec.ts",
            "src/c.test.ts",
            "e2e/",
            "tests/integration/",
        ]),
        "package.json",
        r#"{"devDependencies":{"vitest":"3","@vitest/coverage-v8":"3"},"scripts":{"test":"vitest"}}"#,
    );
    let g = analyze(&full);
    let files = find(&g, "testing:test_files").unwrap();
    assert_eq!(files.value, "3");
    assert_eq!(files.evidence.len(), 3);
    let pattern = find(&g, "convention:test_file_pattern").unwrap();
    assert_eq!(pattern.value, "*.spec.ts");
    assert_eq!(pattern.origin, Origin::Fact);
    assert_eq!(pattern.evidence.len(), 2);
    assert!(find(&g, "testing:e2e_tests").is_some());
    assert!(find(&g, "testing:integration_tests").is_some());
    assert_eq!(
        find(&g, "testing:coverage_config").unwrap().evidence[0]
            .field
            .as_deref(),
        Some("devDependencies[\"@vitest/coverage-v8\"]")
    );
    assert!(find(&g, "build:test_node").is_some());
}
