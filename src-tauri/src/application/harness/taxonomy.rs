//! The vocabulary shared by the task analyzer and by the knowledge it is matched against: a small,
//! closed set of tags with the words (English and Portuguese) that mean them.
//!
//! Tags are derived by rules, never by a model, and only from a fixed lexicon: a word that is not
//! in it is not a tag. The same lexicon reads a task (`"Adicionar endpoint de recuperação de
//! senha"`) and a finding (`src/auth/password-reset.ts`), so a match means the same words were
//! meant on both sides.
//!
//! A term ending in `*` matches any word that starts with it; other terms match the word itself,
//! with a plural `s` ignored.

use std::collections::BTreeSet;

use crate::domain::harness::FindingCategory;
use crate::domain::task_context::Area;

struct Entry {
    tag: &'static str,
    /// Areas whose knowledge a task with this tag needs.
    areas: &'static [Area],
    terms: &'static [&'static str],
}

const LEXICON: &[Entry] = &[
    Entry {
        tag: "authentication",
        areas: &[Area::Modules, Area::Business],
        terms: &[
            "auth",
            "authn",
            "authenticat*",
            "login",
            "logout",
            "signin",
            "signup",
            "session",
            "jwt",
            "oauth",
            "oidc",
            "sso",
            "credential*",
            "autentic*",
            "entrar",
            "sessao",
        ],
    },
    Entry {
        tag: "authorization",
        areas: &[Area::Modules, Area::Business],
        terms: &[
            "authz",
            "authoriz*",
            "permission*",
            "role",
            "rbac",
            "acl",
            "autoriz*",
            "permiss*",
        ],
    },
    Entry {
        tag: "password",
        areas: &[Area::Business, Area::Modules],
        terms: &[
            "password*",
            "passwd",
            "senha*",
            "recover*",
            "recuper*",
            "forgot",
            "reset",
            "esquec*",
        ],
    },
    Entry {
        tag: "api",
        areas: &[Area::Conventions, Area::EntryPoints],
        terms: &[
            "api",
            "rest*",
            "endpoint*",
            "route*",
            "router",
            "controller*",
            "handler*",
            "graphql",
            "grpc",
            "http",
            "openapi",
            "swagger",
            "rota*",
        ],
    },
    Entry {
        tag: "backend",
        areas: &[],
        terms: &[
            "backend", "server", "servidor", "nestjs", "express", "fastify", "django", "flask",
            "fastapi", "spring*", "axum", "actix", "rails", "laravel", "koa", "hono",
        ],
    },
    Entry {
        tag: "frontend",
        areas: &[Area::Conventions],
        terms: &[
            "frontend",
            "angular",
            "react",
            "vue",
            "svelte",
            "next*",
            "nuxt",
            "tailwind",
            "css",
            "scss",
            "html",
            "dom",
            "component*",
            "tela",
            "vite",
            "webpack",
        ],
    },
    Entry {
        tag: "ui",
        areas: &[Area::Conventions],
        terms: &[
            "ui",
            "ux",
            "layout",
            "page",
            "pagina*",
            "screen",
            "style*",
            "estilo*",
            "visual",
            "modal",
            "button",
            "botao",
            "form*",
            "theme",
            "tema",
            "design",
            "interface",
        ],
    },
    Entry {
        tag: "database",
        areas: &[Area::Infrastructure],
        terms: &[
            "database",
            "db",
            "banco",
            "sql",
            "postgres*",
            "mysql",
            "sqlite",
            "mongo*",
            "prisma",
            "typeorm",
            "sequelize",
            "knex",
            "drizzle",
            "diesel",
            "sqlx",
            "orm",
            "schema",
            "query",
            "queries",
            "table",
            "tabela",
        ],
    },
    Entry {
        tag: "migrations",
        areas: &[Area::Infrastructure, Area::Conventions],
        terms: &["migration*", "migrat*", "migracao", "migracoes"],
    },
    Entry {
        tag: "users",
        areas: &[Area::Modules, Area::Business],
        terms: &[
            "user*", "usuario*", "account*", "conta", "profile", "perfil", "customer", "cliente",
        ],
    },
    Entry {
        tag: "testing",
        areas: &[Area::Testing],
        terms: &[
            "test*",
            "spec",
            "jest",
            "vitest",
            "mocha",
            "pytest",
            "cypress",
            "playwright",
            "coverage",
            "tdd",
            "teste*",
            "unit",
            "e2e",
            "junit",
        ],
    },
    Entry {
        tag: "build",
        areas: &[Area::Ci],
        terms: &[
            "build", "compile*", "compil*", "bundle", "bundler", "webpack", "vite", "cargo",
            "gradle", "maven", "makefile",
        ],
    },
    Entry {
        tag: "ci",
        areas: &[Area::Ci],
        terms: &[
            "ci",
            "cicd",
            "pipeline*",
            "workflow*",
            "github",
            "actions",
            "gitlab",
            "jenkins",
            "circleci",
            "travis",
        ],
    },
    Entry {
        tag: "deployment",
        areas: &[Area::Ci, Area::Infrastructure],
        terms: &[
            "deploy*",
            "docker*",
            "container*",
            "kubernetes",
            "k8s",
            "helm",
            "terraform",
            "release",
            "compose",
            "nginx",
            "implant*",
        ],
    },
    Entry {
        tag: "configuration",
        areas: &[Area::Infrastructure],
        terms: &[
            "config*",
            "env",
            "environment*",
            "settings",
            "setting",
            "dotenv",
            "yaml",
            "toml",
            "variavel*",
            "ambiente",
        ],
    },
    Entry {
        tag: "caching",
        areas: &[Area::Infrastructure, Area::Stack],
        terms: &["cach*", "redis", "memcached", "ttl"],
    },
    Entry {
        tag: "billing",
        areas: &[Area::Business, Area::Modules],
        terms: &[
            "billing",
            "invoice*",
            "payment*",
            "pagamento*",
            "fatura*",
            "cobranca",
            "stripe",
            "checkout",
            "subscription*",
        ],
    },
    Entry {
        tag: "reporting",
        areas: &[Area::Business, Area::Modules],
        terms: &["report*", "relatorio*", "dashboard*", "analytics", "chart*"],
    },
    Entry {
        tag: "errors",
        areas: &[Area::Conventions],
        terms: &[
            "error*",
            "exception*",
            "erro",
            "erros",
            "falha*",
            "failure*",
        ],
    },
    Entry {
        tag: "logging",
        areas: &[Area::Conventions],
        terms: &["log", "logs", "logger", "logging", "tracing", "telemetry"],
    },
    Entry {
        tag: "security",
        areas: &[Area::Constraints, Area::Business],
        terms: &[
            "security",
            "secure",
            "secret*",
            "encrypt*",
            "crypto*",
            "csrf",
            "xss",
            "cors",
            "sanitiz*",
            "seguranca",
            "vulnerab*",
            "owasp",
        ],
    },
    Entry {
        tag: "documentation",
        areas: &[Area::Business],
        terms: &[
            "doc",
            "docs",
            "documentation",
            "readme",
            "documentacao",
            "changelog",
            "adr",
        ],
    },
];

/// A tag that brings others with it: a password is about authentication, an endpoint is backend.
const IMPLIES: &[(&str, &[&str])] = &[
    ("password", &["authentication"]),
    ("authorization", &["authentication"]),
    ("api", &["backend"]),
    ("ui", &["frontend"]),
    ("migrations", &["database"]),
    ("database", &["backend"]),
    ("ci", &["build", "testing", "deployment"]),
];

/// The tags that say which layer of a system something belongs to. An item from one layer is
/// not what a task in another layer needs (an Angular note for a database migration).
const LAYERS: &[&str] = &["frontend", "backend", "database"];

/// Words that name a technology: a task that asks for one the Harness never mentions is told
/// that it was not identified. (Display name, words).
const TECHNOLOGIES: &[(&str, &[&str])] = &[
    ("Redis", &["redis"]),
    ("Memcached", &["memcached"]),
    ("Kafka", &["kafka"]),
    ("RabbitMQ", &["rabbitmq"]),
    ("MongoDB", &["mongodb", "mongo"]),
    ("PostgreSQL", &["postgres", "postgresql"]),
    ("MySQL", &["mysql"]),
    ("SQLite", &["sqlite"]),
    ("Elasticsearch", &["elasticsearch"]),
    ("GraphQL", &["graphql"]),
    ("gRPC", &["grpc"]),
    ("Docker", &["docker"]),
    ("Kubernetes", &["kubernetes", "k8s"]),
    ("Terraform", &["terraform"]),
    ("Stripe", &["stripe"]),
    ("Firebase", &["firebase"]),
    ("Supabase", &["supabase"]),
];

/// Lower-cased words of a text: accents removed, `camelCase` and separators split (`/`, `-`,
/// `_`, `.`, spaces).
pub fn words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut prev_lower = false;
    for c in text.chars() {
        let c = fold(c);
        if c.is_alphanumeric() {
            if c.is_uppercase() && prev_lower && !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            prev_lower = c.is_lowercase() || c.is_numeric();
            current.extend(c.to_lowercase());
        } else {
            prev_lower = false;
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn fold(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'A',
        'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'O',
        'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        'Ç' => 'C',
        other => other,
    }
}

/// A word without its plural `s` (`routes` -> `route`, `class` stays).
fn singular(word: &str) -> &str {
    if word.len() > 3 && word.ends_with('s') && !word.ends_with("ss") {
        &word[..word.len() - 1]
    } else {
        word
    }
}

fn term_matches(word: &str, term: &str) -> bool {
    match term.strip_suffix('*') {
        Some(prefix) => word.starts_with(prefix),
        None => word == term || singular(word) == term,
    }
}

/// Whether two words are the same word, or one starts the other (at least four letters long):
/// `password` and `passwords`, `auth` and `authentication`; but not `art` and `article`.
pub fn same_word(a: &str, b: &str) -> bool {
    if a == b || singular(a) == singular(b) {
        return true;
    }
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    short.len() >= 4 && long.starts_with(short)
}

/// The tags the given words mean, with what each implies.
pub fn tags_of_words(words: &[String]) -> BTreeSet<&'static str> {
    let mut tags = BTreeSet::new();
    for entry in LEXICON {
        if words
            .iter()
            .any(|w| entry.terms.iter().any(|t| term_matches(w, t)))
        {
            tags.insert(entry.tag);
        }
    }
    let direct: Vec<&str> = tags.iter().copied().collect();
    for tag in direct {
        for (from, implied) in IMPLIES {
            if *from == tag {
                tags.extend(implied.iter().copied());
            }
        }
    }
    tags
}

pub fn tags_of_text(text: &str) -> BTreeSet<&'static str> {
    tags_of_words(&words(text))
}

/// Tags a category carries by itself, whatever its text says.
pub fn category_tags(category: FindingCategory) -> &'static [&'static str] {
    match category {
        FindingCategory::Database | FindingCategory::Data => &["database", "backend"],
        FindingCategory::Testing => &["testing"],
        FindingCategory::Build => &["build"],
        FindingCategory::Ci => &["ci", "build"],
        FindingCategory::Infrastructure => &["deployment"],
        FindingCategory::Environment => &["configuration"],
        _ => &[],
    }
}

/// The categories a tag points at: what a task about it wants from the Harness.
pub fn tag_categories(tag: &str) -> &'static [FindingCategory] {
    match tag {
        "database" | "migrations" => &[FindingCategory::Database, FindingCategory::Data],
        "testing" => &[FindingCategory::Testing],
        "build" => &[FindingCategory::Build],
        "ci" => &[FindingCategory::Ci],
        "deployment" => &[FindingCategory::Infrastructure, FindingCategory::Ci],
        "configuration" => &[FindingCategory::Environment],
        "api" => &[FindingCategory::EntryPoint, FindingCategory::Integration],
        _ => &[],
    }
}

/// The areas whose knowledge a tag asks for.
pub fn tag_areas(tag: &str) -> &'static [Area] {
    LEXICON
        .iter()
        .find(|e| e.tag == tag)
        .map_or(&[], |e| e.areas)
}

pub fn is_layer(tag: &str) -> bool {
    LAYERS.contains(&tag)
}

/// The technologies the words name, by display name.
pub fn technologies_of_words(words: &[String]) -> Vec<&'static str> {
    TECHNOLOGIES
        .iter()
        .filter(|(_, names)| words.iter().any(|w| names.iter().any(|n| w == n)))
        .map(|(display, _)| *display)
        .collect()
}

/// The tags a technology means (`Redis` is caching).
pub fn tags_of_technology(display: &str) -> Vec<String> {
    let names: Vec<String> = technology_words(display)
        .iter()
        .map(|w| (*w).to_owned())
        .collect();
    tags_of_words(&names)
        .into_iter()
        .map(str::to_owned)
        .collect()
}

/// The words a technology goes by.
pub fn technology_words(display: &str) -> &'static [&'static str] {
    TECHNOLOGIES
        .iter()
        .find(|(d, _)| *d == display)
        .map_or(&[], |(_, names)| names)
}

/// Whether the words of an evidence path say what the finding is about. A manifest or
/// configuration file at the root (`package.json`, `docker-compose.yml`) or in a dot folder
/// (`.github/workflows`) is evidence for many unrelated findings, so its name says where the
/// finding was read, not what it is; a path inside the source tree (`src/auth/login.ts`) does.
fn names_a_place_in_the_source(path: &str) -> bool {
    path.contains('/') && !path.starts_with('.')
}

/// Tags of a finding: its category's, its words' and those of the paths of its evidence.
pub fn tags_of_finding(
    category: Option<FindingCategory>,
    texts: &[&str],
    paths: &[&str],
) -> Vec<String> {
    let mut all: Vec<String> = Vec::new();
    for text in texts {
        all.extend(words(text));
    }
    for path in paths.iter().filter(|p| names_a_place_in_the_source(p)) {
        all.extend(words(path));
    }
    let mut tags = tags_of_words(&all);
    if let Some(category) = category {
        tags.extend(category_tags(category).iter().copied());
    }
    tags.into_iter().map(str::to_owned).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_fold_accents_and_split_paths_and_camel_case() {
        assert_eq!(
            words("src/auth/passwordReset.ts"),
            ["src", "auth", "password", "reset", "ts"]
        );
        assert_eq!(
            words("Recuperação de SENHA"),
            ["recuperacao", "de", "senha"]
        );
    }

    #[test]
    fn a_path_means_what_its_words_mean_and_implications_follow() {
        let tags = tags_of_text("src/auth/password-reset.ts");
        assert!(tags.contains("authentication"));
        assert!(tags.contains("password"));
        let tags = tags_of_text("add an endpoint");
        assert!(tags.contains("api") && tags.contains("backend"));
    }

    #[test]
    fn a_manifest_at_the_root_does_not_tag_what_it_is_evidence_for() {
        let tags = tags_of_finding(None, &["PostgreSQL"], &["docker-compose.yml"]);
        assert!(tags.contains(&"database".to_owned()));
        assert!(!tags.contains(&"deployment".to_owned()));
        let tags = tags_of_finding(None, &["Tests"], &[".github/workflows/ci.yml"]);
        assert!(!tags.contains(&"ci".to_owned()));
        let tags = tags_of_finding(None, &["Something"], &["src/auth/login.ts"]);
        assert!(tags.contains(&"authentication".to_owned()));
        // "infrastructure" is a folder of layered code, not deployment.
        assert!(!tags_of_text("src/infrastructure").contains("deployment"));
    }

    #[test]
    fn a_word_outside_the_lexicon_is_not_a_tag() {
        assert!(tags_of_text("banana kiwi zebra").is_empty());
    }

    #[test]
    fn plurals_and_stems_match() {
        assert!(tags_of_text("passwords").contains("password"));
        assert!(tags_of_text("migrations").contains("database"));
        assert!(same_word("password", "passwords"));
        assert!(same_word("auth", "authentication"));
        assert!(!same_word("art", "article"));
    }
}
