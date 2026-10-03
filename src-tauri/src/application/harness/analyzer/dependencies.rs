use super::{npm, SnapshotAnalyzer};
use crate::application::harness::snapshot::{depth, file_name, ScanSnapshot};
use crate::domain::harness::{Confidence, Evidence, Finding, FindingCategory, Origin};

/// The few dependencies that say something about the project (framework libraries, ORM,
/// messaging, auth, observability…), not the hundreds it lists. Also what they suggest about
/// data and integrations, always as inferences: a dependency is not proof of use.
pub struct DependencyAnalyzer;

#[derive(Clone, Copy)]
enum Kind {
    Ui,
    State,
    Http,
    Auth,
    Observability,
    Resilience,
    Orm,
    Migration,
    Api,
    Messaging,
    WebServer,
}

impl Kind {
    fn describe(self) -> &'static str {
        match self {
            Self::Ui => "UI",
            Self::State => "state management",
            Self::Http => "HTTP client",
            Self::Auth => "authentication",
            Self::Observability => "observability",
            Self::Resilience => "resilience",
            Self::Orm => "ORM / data access",
            Self::Migration => "database migrations",
            Self::Api => "API",
            Self::Messaging => "messaging",
            Self::WebServer => "web server framework",
        }
    }
}

/// `(needle, key, kind)`. For JSON manifests the needle is the exact package name; for text
/// manifests it is looked for case-insensitively.
type Table = &'static [(&'static str, &'static str, Kind)];

const NPM: &[(&str, &str, Kind)] = &[
    ("primeng", "primeng", Kind::Ui),
    ("@angular/material", "angular_material", Kind::Ui),
    ("@mui/material", "mui", Kind::Ui),
    ("tailwindcss", "tailwindcss", Kind::Ui),
    ("bootstrap", "bootstrap", Kind::Ui),
    ("rxjs", "rxjs", Kind::State),
    ("@ngrx/store", "ngrx", Kind::State),
    ("redux", "redux", Kind::State),
    ("@reduxjs/toolkit", "redux_toolkit", Kind::State),
    ("zustand", "zustand", Kind::State),
    ("pinia", "pinia", Kind::State),
    ("mobx", "mobx", Kind::State),
    ("axios", "axios", Kind::Http),
    ("@tanstack/react-query", "react_query", Kind::Http),
    ("passport", "passport", Kind::Auth),
    ("jsonwebtoken", "jsonwebtoken", Kind::Auth),
    ("next-auth", "next_auth", Kind::Auth),
    ("@opentelemetry/api", "opentelemetry", Kind::Observability),
    ("winston", "winston", Kind::Observability),
    ("@sentry/browser", "sentry", Kind::Observability),
    ("@sentry/node", "sentry", Kind::Observability),
    ("prisma", "prisma", Kind::Orm),
    ("@prisma/client", "prisma", Kind::Orm),
    ("typeorm", "typeorm", Kind::Orm),
    ("sequelize", "sequelize", Kind::Orm),
    ("drizzle-orm", "drizzle", Kind::Orm),
    ("mongoose", "mongoose", Kind::Orm),
    ("graphql", "graphql", Kind::Api),
    ("@apollo/client", "apollo_client", Kind::Api),
    ("@apollo/server", "apollo_server", Kind::Api),
    ("@grpc/grpc-js", "grpc", Kind::Api),
    ("kafkajs", "kafka", Kind::Messaging),
    ("amqplib", "rabbitmq", Kind::Messaging),
    ("bullmq", "bullmq", Kind::Messaging),
    ("express", "express", Kind::WebServer),
    ("@nestjs/core", "nestjs", Kind::WebServer),
    ("fastify", "fastify", Kind::WebServer),
];

const CARGO: &[(&str, &str, Kind)] = &[
    ("sqlx", "sqlx", Kind::Orm),
    ("diesel", "diesel", Kind::Orm),
    ("sea-orm", "sea_orm", Kind::Orm),
    ("reqwest", "reqwest", Kind::Http),
    ("tracing", "tracing", Kind::Observability),
    ("opentelemetry", "opentelemetry", Kind::Observability),
    ("axum", "axum", Kind::WebServer),
    ("actix-web", "actix_web", Kind::WebServer),
    ("tonic", "tonic", Kind::Api),
    ("async-graphql", "async_graphql", Kind::Api),
    ("lapin", "rabbitmq", Kind::Messaging),
    ("rdkafka", "kafka", Kind::Messaging),
];

const DOTNET: &[(&str, &str, Kind)] = &[
    (
        "Microsoft.EntityFrameworkCore",
        "entity_framework",
        Kind::Orm,
    ),
    ("Dapper", "dapper", Kind::Orm),
    ("Serilog", "serilog", Kind::Observability),
    ("OpenTelemetry", "opentelemetry", Kind::Observability),
    ("Polly", "polly", Kind::Resilience),
    ("MassTransit", "masstransit", Kind::Messaging),
    ("RabbitMQ.Client", "rabbitmq", Kind::Messaging),
    ("Confluent.Kafka", "kafka", Kind::Messaging),
    ("Grpc.", "grpc", Kind::Api),
    ("HotChocolate", "hotchocolate", Kind::Api),
    (
        "Microsoft.AspNetCore.Authentication",
        "aspnet_authentication",
        Kind::Auth,
    ),
    ("IdentityServer", "identityserver", Kind::Auth),
    ("Microsoft.NET.Sdk.Web", "aspnet", Kind::WebServer),
];

const JAVA: &[(&str, &str, Kind)] = &[
    ("hibernate", "hibernate", Kind::Orm),
    ("spring-data-jpa", "spring_data_jpa", Kind::Orm),
    ("flyway", "flyway", Kind::Migration),
    ("liquibase", "liquibase", Kind::Migration),
    ("spring-security", "spring_security", Kind::Auth),
    ("spring-kafka", "kafka", Kind::Messaging),
    ("spring-amqp", "rabbitmq", Kind::Messaging),
    ("micrometer", "micrometer", Kind::Observability),
    ("opentelemetry", "opentelemetry", Kind::Observability),
    ("spring-boot-starter-web", "spring_web", Kind::WebServer),
    ("grpc", "grpc", Kind::Api),
];

const PYTHON: &[(&str, &str, Kind)] = &[
    ("sqlalchemy", "sqlalchemy", Kind::Orm),
    ("alembic", "alembic", Kind::Migration),
    ("celery", "celery", Kind::Messaging),
    ("httpx", "httpx", Kind::Http),
    ("requests", "requests", Kind::Http),
    ("fastapi", "fastapi", Kind::WebServer),
    ("flask", "flask", Kind::WebServer),
    ("django", "django", Kind::WebServer),
];

impl SnapshotAnalyzer for DependencyAnalyzer {
    fn analyze(&self, s: &ScanSnapshot) -> Vec<Finding> {
        let mut found: Vec<Hit> = Vec::new();

        for (path, json) in npm::manifests(s) {
            for (name, key, kind) in NPM {
                if let Some((field, spec)) = npm::dependency(&json, name) {
                    found.push(Hit::new(key, npm::major(&spec), *kind, path, &field));
                }
            }
        }
        let text_manifests: [(Table, Vec<&str>, bool); 4] = [
            (CARGO, s.files_named("Cargo.toml", 2), false),
            (
                DOTNET,
                s.files_with_extension("csproj", 3)
                    .into_iter()
                    .chain(s.files_with_extension("fsproj", 3))
                    .collect(),
                true,
            ),
            (
                JAVA,
                ["pom.xml", "build.gradle", "build.gradle.kts"]
                    .iter()
                    .flat_map(|n| s.files_named(n, 2))
                    .collect(),
                false,
            ),
            (
                PYTHON,
                ["pyproject.toml", "requirements.txt"]
                    .iter()
                    .flat_map(|n| s.files_named(n, 2))
                    .collect(),
                false,
            ),
        ];
        for (table, paths, case_sensitive) in text_manifests {
            for path in paths {
                let Some(text) = s.files.get(path) else {
                    continue;
                };
                let lower = text.to_lowercase();
                for (needle, key, kind) in table {
                    let present = if case_sensitive {
                        text.contains(needle)
                    } else {
                        lower.contains(&needle.to_lowercase())
                    };
                    if present {
                        found.push(Hit::new(key, "true".to_owned(), *kind, path, needle));
                    }
                }
            }
        }

        let mut out = Vec::new();
        findings_from(&found, &mut out);
        data_and_integrations(s, &found, &mut out);
        out
    }
}

struct Hit {
    key: &'static str,
    version: String,
    kind: Kind,
    evidence: Evidence,
}

impl Hit {
    fn new(key: &'static str, version: String, kind: Kind, source: &str, field: &str) -> Self {
        Self {
            key,
            version,
            kind,
            evidence: Evidence::new(source, Some(field)),
        }
    }
}

/// Dependencies are facts: the manifest lists them. Their category follows what they are for.
fn findings_from(found: &[Hit], out: &mut Vec<Finding>) {
    for hit in found {
        let category = match hit.kind {
            Kind::Orm | Kind::Migration => FindingCategory::Data,
            Kind::Api | Kind::Messaging => FindingCategory::Integration,
            _ => FindingCategory::Dependency,
        };
        // Web servers and brokers are reported through the integration inferences below.
        if matches!(hit.kind, Kind::WebServer) {
            continue;
        }
        let (confidence, origin) = if matches!(hit.kind, Kind::Api | Kind::Messaging) {
            (Confidence::Medium, Origin::Inference)
        } else {
            (Confidence::High, Origin::Fact)
        };
        let mut finding = Finding::new(
            category,
            hit.key,
            &hit.version,
            confidence,
            origin,
            &hit.evidence.source,
            hit.evidence.field.as_deref().unwrap_or(""),
        );
        finding.reason = Some(match origin {
            Origin::Fact => format!("Listed as a dependency ({})", hit.kind.describe()),
            _ => format!(
                "Listed as a dependency ({}); that suggests, but does not prove, it is used",
                hit.kind.describe()
            ),
        });
        out.push(finding);
    }
}

fn data_and_integrations(s: &ScanSnapshot, found: &[Hit], out: &mut Vec<Finding>) {
    // Migration folders are facts about the repository.
    if let Some(dir) = s.all_dirs().find(|d| {
        depth(d) <= 3
            && ["migrations", "migrate", "migration"]
                .contains(&file_name(d).to_lowercase().as_str())
    }) {
        out.push(
            Finding::new(
                FindingCategory::Data,
                "migrations",
                dir,
                Confidence::High,
                Origin::Fact,
                dir,
                "",
            )
            .with_label("Database migrations folder"),
        );
    }
    if let Some(dir) = s
        .all_dirs()
        .find(|d| depth(d) <= 4 && file_name(d).eq_ignore_ascii_case("repositories"))
    {
        out.push(
            Finding::new(
                FindingCategory::Data,
                "repository_pattern",
                "possible",
                Confidence::Medium,
                Origin::Inference,
                dir,
                "",
            )
            .with_reason("A repositories folder suggests the repository pattern")
            .with_label("Repository pattern"),
        );
    }

    // REST: a web server framework, plus the folders that usually hold its endpoints.
    let servers: Vec<&Hit> = found
        .iter()
        .filter(|h| matches!(h.kind, Kind::WebServer))
        .collect();
    let has_proto = !s.files_with_extension("proto", 3).is_empty();
    if !servers.is_empty() {
        let mut evidence: Vec<Evidence> = servers.iter().map(|h| h.evidence.clone()).collect();
        if let Some(dir) = s
            .all_dirs()
            .find(|d| depth(d) <= 4 && file_name(d).eq_ignore_ascii_case("controllers"))
        {
            evidence.push(Evidence::new(dir, None));
        }
        out.push(
            Finding::new(
                FindingCategory::Integration,
                "rest_api",
                "possible",
                Confidence::Medium,
                Origin::Inference,
                "",
                "",
            )
            .with_evidence(evidence)
            .with_reason("A web server framework is a dependency; this suggests an HTTP API")
            .with_label("Possible REST API"),
        );
    }
    if has_proto {
        let proto = s.files_with_extension("proto", 3)[0];
        out.push(
            Finding::new(
                FindingCategory::Integration,
                "grpc",
                "possible",
                Confidence::Medium,
                Origin::Inference,
                proto,
                "",
            )
            .with_reason(".proto files usually define gRPC services")
            .with_label("Possible gRPC"),
        );
    }
}
