//! Chooses which evidence a semantic analysis may see. Nothing is sent that this module did not
//! select from what the safe scanner already listed, and the selection is bounded.

use std::collections::BTreeSet;

use super::snapshot::{depth, file_name, parent_of, ScanSnapshot};

/// Explicit, internal limits. Reaching one makes the analysis *partial*; it is never hidden.
#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_field_names)]
pub struct SampleLimits {
    pub max_files: usize,
    pub max_total_bytes: usize,
    pub max_file_bytes: usize,
    pub max_tree_entries: usize,
}

impl Default for SampleLimits {
    fn default() -> Self {
        Self {
            max_files: 24,
            max_total_bytes: 100_000,
            max_file_bytes: 8_000,
            max_tree_entries: 400,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SampleFile {
    pub path: String,
    pub content: String,
    pub truncated: bool,
}

/// The controlled set of evidence for a semantic analysis: a bounded tree and a few files.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EvidenceBundle {
    /// Relative paths (folders end with `/`) the model is told about.
    pub tree: Vec<String>,
    pub files: Vec<SampleFile>,
    /// A limit cut the tree or the files short.
    pub partial: bool,
}

impl EvidenceBundle {
    /// Every path a statement may cite: the tree and the sampled files.
    pub fn citable_paths(&self) -> BTreeSet<String> {
        self.tree
            .iter()
            .map(|p| p.trim_end_matches('/').to_owned())
            .chain(self.files.iter().map(|f| f.path.clone()))
            .collect()
    }

    pub fn file_paths(&self) -> Vec<String> {
        self.files.iter().map(|f| f.path.clone()).collect()
    }
}

/// Names that may hold secrets: never sampled, whatever their priority.
pub fn looks_secret_path(path: &str) -> bool {
    let name = file_name(path).to_lowercase();
    name.starts_with(".env")
        || name.starts_with("id_rsa")
        || name.starts_with("id_ed25519")
        || name.contains("secret")
        || name.contains("credential")
        || name.contains("password")
        || ["pem", "key", "p12", "pfx", "jks", "keystore", "kdbx"]
            .iter()
            .any(|ext| name.ends_with(&format!(".{ext}")))
}

const SOURCE_EXTENSIONS: &[&str] = &[
    "ts", "tsx", "js", "jsx", "rs", "cs", "java", "kt", "py", "go", "vue", "svelte",
];
const LAYER_DIRS: &[&str] = &[
    "domain",
    "application",
    "infrastructure",
    "api",
    "controllers",
    "services",
    "repositories",
    "components",
    "features",
    "modules",
    "app",
    "commands",
];

/// Lower is chosen first. `None`: not worth sending.
fn priority(path: &str, snapshot: &ScanSnapshot) -> Option<u8> {
    if looks_secret_path(path) {
        return None;
    }
    let name = file_name(path);
    let lower = name.to_lowercase();
    let d = depth(path);
    if lower.starts_with("readme") && d <= 1 {
        return Some(0);
    }
    if snapshot.files.contains_key(path) {
        // Manifests the scanner already read: always useful and already bounded.
        return Some(1);
    }
    if lower.starts_with("architecture")
        || lower.starts_with("contributing")
        || (path.starts_with("docs/")
            && std::path::Path::new(&lower)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("md"))
            && d <= 2)
    {
        return Some(2);
    }
    if [
        "angular.json",
        "tsconfig.json",
        "dockerfile",
        "program.cs",
        "main.ts",
        "main.tsx",
        "main.rs",
        "main.go",
        "lib.rs",
        "mod.rs",
        "app.ts",
        "index.ts",
    ]
    .contains(&lower.as_str())
        || lower.starts_with("vite.config.")
        || lower.starts_with("docker-compose")
    {
        return Some(3);
    }
    if path.starts_with(".github/workflows/") && d == 2 {
        return Some(3);
    }
    let extension = lower.rsplit_once('.').map_or("", |(_, e)| e);
    if SOURCE_EXTENSIONS.contains(&extension) && !is_test_file(&lower) {
        let inside_layer = path
            .split('/')
            .rev()
            .skip(1)
            .any(|dir| LAYER_DIRS.contains(&dir.to_lowercase().as_str()));
        if inside_layer && d <= 4 {
            return Some(4);
        }
    }
    if is_test_file(&lower) && d <= 4 {
        return Some(5);
    }
    None
}

pub fn is_test_file(lower_name: &str) -> bool {
    lower_name.contains(".spec.")
        || lower_name.contains(".test.")
        || lower_name.ends_with("_test.go")
        || lower_name.ends_with("tests.cs")
        || lower_name.ends_with("test.java")
        || lower_name.starts_with("test_")
}

/// What the evidence is for. Documentation and manifests are read to check facts against each
/// other; code samples are only read when a model is going to see them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    Documents,
    Semantic,
}

/// What the sampler wants read and what it already has.
pub struct Selection {
    /// Files whose content must be read through the scanner port.
    pub to_read: Vec<String>,
    pub bundle: EvidenceBundle,
}

/// Picks the tree and the files. Manifests the scanner already read are used as they are; the
/// others are listed in `to_read`. At most one or two source files per layer folder, so the
/// sample is representative rather than large.
pub fn select(snapshot: &ScanSnapshot, limits: SampleLimits, purpose: Purpose) -> Selection {
    let mut partial = snapshot.truncated;

    let mut tree: Vec<String> = Vec::new();
    for entry in snapshot.entries.iter().filter(|e| e.is_dir) {
        tree.push(format!("{}/", entry.path));
    }
    for entry in snapshot
        .entries
        .iter()
        .filter(|e| !e.is_dir && depth(&e.path) <= 2 && !looks_secret_path(&e.path))
    {
        tree.push(entry.path.clone());
    }
    tree.sort();
    if tree.len() > limits.max_tree_entries {
        tree.truncate(limits.max_tree_entries);
        partial = true;
    }

    let mut candidates: Vec<(u8, &str)> = snapshot
        .entries
        .iter()
        .filter(|e| !e.is_dir)
        .filter_map(|e| priority(&e.path, snapshot).map(|p| (p, e.path.as_str())))
        .filter(|(p, _)| purpose == Purpose::Semantic || *p <= 2)
        .collect();
    candidates.sort_unstable();

    let mut per_parent: std::collections::BTreeMap<(u8, &str), usize> =
        std::collections::BTreeMap::default();
    let mut chosen: Vec<&str> = Vec::new();
    for (priority, path) in candidates {
        // Source and test files: a couple per folder is enough to see the style.
        if priority >= 4 {
            let seen = per_parent.entry((priority, parent_of(path))).or_default();
            if *seen >= 2 {
                continue;
            }
            *seen += 1;
        }
        if chosen.len() >= limits.max_files {
            partial = true;
            break;
        }
        chosen.push(path);
    }

    let mut files = Vec::new();
    let mut to_read = Vec::new();
    let mut total = 0;
    for path in chosen {
        if let Some(content) = snapshot.files.get(path) {
            let (content, truncated) = cut(content, limits.max_file_bytes);
            if total + content.len() > limits.max_total_bytes {
                partial = true;
                continue;
            }
            total += content.len();
            files.push(SampleFile {
                path: path.to_owned(),
                content,
                truncated,
            });
        } else {
            to_read.push(path.to_owned());
        }
    }
    Selection {
        to_read,
        bundle: EvidenceBundle {
            tree,
            files,
            partial,
        },
    }
}

/// Adds the files the scanner read for the sampler, within the byte budget.
pub fn complete(
    mut selection: Selection,
    read: Vec<(String, String)>,
    limits: SampleLimits,
) -> EvidenceBundle {
    let mut total: usize = selection.bundle.files.iter().map(|f| f.content.len()).sum();
    for (path, content) in read {
        let (content, truncated) = cut(&content, limits.max_file_bytes);
        if total + content.len() > limits.max_total_bytes {
            selection.bundle.partial = true;
            continue;
        }
        total += content.len();
        selection.bundle.files.push(SampleFile {
            path,
            content,
            truncated,
        });
    }
    selection.bundle.files.sort_by(|a, b| a.path.cmp(&b.path));
    selection.bundle
}

fn cut(text: &str, max: usize) -> (String, bool) {
    if text.len() <= max {
        return (text.to_owned(), false);
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_owned(), true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> ScanSnapshot {
        let mut s = ScanSnapshot::default();
        s.dir("src");
        s.dir("src/application");
        s.dir("docs");
        s.file("README.md", None);
        s.file("package.json", Some("{\"name\":\"x\"}"));
        s.file(".env", Some("TOKEN=abc"));
        s.file("server.key", None);
        s.file("docs/ARCHITECTURE.md", None);
        s.file("src/main.ts", None);
        s.file("src/application/a.ts", None);
        s.file("src/application/b.ts", None);
        s.file("src/application/c.ts", None);
        s.file("src/application/a.spec.ts", None);
        s.file("src/random/readme.txt", None);
        s
    }

    #[test]
    fn it_picks_documentation_manifests_entry_points_and_a_few_samples_never_secrets() {
        let selection = select(&project(), SampleLimits::default(), Purpose::Semantic);

        let mut chosen: Vec<&str> = selection.to_read.iter().map(String::as_str).collect();
        chosen.extend(selection.bundle.files.iter().map(|f| f.path.as_str()));
        for wanted in [
            "README.md",
            "package.json",
            "docs/ARCHITECTURE.md",
            "src/main.ts",
            "src/application/a.spec.ts",
        ] {
            assert!(chosen.contains(&wanted), "missing {wanted}");
        }
        assert!(!chosen.contains(&".env") && !chosen.contains(&"server.key"));
        // At most two source files per folder: a sample, not the folder.
        assert_eq!(
            chosen
                .iter()
                .filter(|p| p.starts_with("src/application/") && !p.contains("spec"))
                .count(),
            2
        );
        assert!(selection
            .bundle
            .tree
            .iter()
            .all(|p| !p.contains(".env") && !p.contains("server.key")));
    }

    #[test]
    fn without_a_model_only_documents_and_manifests_are_wanted() {
        let selection = select(&project(), SampleLimits::default(), Purpose::Documents);

        assert!(selection.to_read.iter().all(|p| !p.starts_with("src/")));
        assert!(selection.to_read.contains(&"README.md".to_owned()));
    }

    #[test]
    fn limits_are_explicit_and_reaching_one_makes_the_evidence_partial() {
        let few = SampleLimits {
            max_files: 2,
            ..SampleLimits::default()
        };
        assert!(select(&project(), few, Purpose::Semantic).bundle.partial);

        let small_tree = SampleLimits {
            max_tree_entries: 3,
            ..SampleLimits::default()
        };
        let selection = select(&project(), small_tree, Purpose::Semantic);
        assert_eq!(selection.bundle.tree.len(), 3);
        assert!(selection.bundle.partial);

        let selection = select(&project(), SampleLimits::default(), Purpose::Semantic);
        let tiny = SampleLimits {
            max_file_bytes: 4,
            max_total_bytes: 9,
            ..SampleLimits::default()
        };
        let read = vec![
            ("README.md".to_owned(), "abcdefgh".to_owned()),
            ("src/main.ts".to_owned(), "12345678".to_owned()),
            ("docs/ARCHITECTURE.md".to_owned(), "zzzzzzzz".to_owned()),
        ];
        let bundle = complete(selection, read, tiny);
        assert!(bundle.partial);
        let added: Vec<_> = bundle
            .files
            .iter()
            .filter(|f| f.path == "README.md" || f.path == "src/main.ts")
            .collect();
        assert!(added.iter().all(|f| f.content.len() <= 4 && f.truncated));
        assert!(bundle.files.iter().map(|f| f.content.len()).sum::<usize>() <= 12 + 9);
    }

    #[test]
    fn only_listed_paths_can_be_cited() {
        let selection = select(&project(), SampleLimits::default(), Purpose::Semantic);

        let citable = selection.bundle.citable_paths();

        assert!(citable.contains("src/application") && citable.contains("README.md"));
        assert!(!citable.contains("src/never"));
    }

    #[test]
    fn secret_looking_names_are_recognised() {
        for secret in [
            ".env",
            ".env.local",
            "id_rsa",
            "prod.pem",
            "app.key",
            "credentials.json",
            "secrets.yaml",
            "db_password.txt",
            "store.p12",
        ] {
            assert!(looks_secret_path(secret), "{secret}");
        }
        assert!(!looks_secret_path("src/keyboard.ts"));
    }
}
