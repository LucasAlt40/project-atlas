use std::fs;
use std::path::Path;

use crate::application::projects::ProjectInspector;

/// `package.json` is read only up to this size, and only for its dependency names.
const MAX_PACKAGE_JSON_BYTES: u64 = 1024 * 1024;

/// Marker files (exact names, compared case-insensitively) and the technology they suggest.
const FILE_MARKERS: &[(&str, &str)] = &[
    ("package.json", "Node.js"),
    ("angular.json", "Angular"),
    ("tsconfig.json", "TypeScript"),
    ("cargo.toml", "Rust"),
    ("tauri.conf.json", "Tauri"),
    ("pom.xml", "Maven"),
    ("build.gradle", "Gradle"),
    ("build.gradle.kts", "Gradle"),
    ("settings.gradle", "Gradle"),
    ("go.mod", "Go"),
    ("pyproject.toml", "Python"),
    ("requirements.txt", "Python"),
    ("setup.py", "Python"),
    ("dockerfile", "Docker"),
    ("docker-compose.yml", "Docker"),
    ("docker-compose.yaml", "Docker"),
    ("compose.yaml", "Docker"),
    ("composer.json", "PHP"),
    ("gemfile", "Ruby"),
    ("pubspec.yaml", "Dart"),
];

/// File extensions (without the dot) and the technology they suggest.
const EXTENSION_MARKERS: &[(&str, &str)] =
    &[("sln", ".NET"), ("csproj", ".NET"), ("fsproj", ".NET")];

/// `package.json` dependency names and the framework they indicate.
const DEPENDENCY_MARKERS: &[(&str, &str)] = &[
    ("@angular/core", "Angular"),
    ("react", "React"),
    ("vue", "Vue"),
    ("svelte", "Svelte"),
    ("next", "Next.js"),
];

/// Looks only at the names of entries in the project folder's top level (and, for `package.json`,
/// its dependency names). No LLM, no recursion, no project contents leave the machine.
pub struct FsProjectInspector;

impl ProjectInspector for FsProjectInspector {
    fn is_directory(&self, path: &str) -> bool {
        Path::new(path).is_dir()
    }

    fn technologies(&self, path: &str) -> Vec<String> {
        let root = Path::new(path);
        let Ok(entries) = fs::read_dir(root) else {
            return Vec::new();
        };
        let names: Vec<String> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().to_lowercase())
            .collect();

        let mut found: Vec<&str> = Vec::new();
        let mut push = |tech: &'static str| {
            if !found.contains(&tech) {
                found.push(tech);
            }
        };
        if names.iter().any(|n| n == ".git") {
            push("Git");
        }
        for (marker, tech) in FILE_MARKERS {
            if names.iter().any(|n| n == marker) {
                push(tech);
            }
        }
        if names.iter().any(|n| n == "src-tauri") {
            push("Tauri");
        }
        for (extension, tech) in EXTENSION_MARKERS {
            if names
                .iter()
                .any(|n| n.rsplit_once('.').is_some_and(|(_, e)| e == *extension))
            {
                push(tech);
            }
        }
        let mut result: Vec<String> = found.into_iter().map(str::to_owned).collect();
        for framework in package_json_frameworks(&root.join("package.json")) {
            if !result.iter().any(|t| t == framework) {
                result.push(framework.to_owned());
            }
        }
        result
    }
}

fn package_json_frameworks(path: &Path) -> Vec<&'static str> {
    let Ok(metadata) = fs::metadata(path) else {
        return Vec::new();
    };
    if !metadata.is_file() || metadata.len() > MAX_PACKAGE_JSON_BYTES {
        return Vec::new();
    }
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    DEPENDENCY_MARKERS
        .iter()
        .filter(|(dependency, _)| {
            ["dependencies", "devDependencies"]
                .iter()
                .any(|section| json[section].get(dependency).is_some())
        })
        .map(|(_, framework)| *framework)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(name: &str, files: &[&str]) -> String {
        let dir = std::env::temp_dir().join(format!("atlas-inspect-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for file in files {
            let path = dir.join(file);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            if file.ends_with('/') {
                fs::create_dir_all(&path).unwrap();
            } else {
                fs::write(&path, "").unwrap();
            }
        }
        dir.display().to_string()
    }

    #[test]
    fn recognises_technologies_from_marker_files_only() {
        let path = project(
            "erp",
            &[
                ".git/",
                "angular.json",
                "App.sln",
                "Web/Web.csproj",
                "Dockerfile",
                "notes.txt",
            ],
        );

        let found = FsProjectInspector.technologies(&path);

        // Only the top level is looked at: the csproj inside "Web/" is not seen, the sln is.
        assert_eq!(found, ["Git", "Angular", "Docker", ".NET"]);
    }

    #[test]
    fn reads_framework_names_from_package_json_dependencies() {
        let path = project("web", &["package.json"]);
        fs::write(
            Path::new(&path).join("package.json"),
            r#"{"dependencies":{"react":"^19"},"devDependencies":{"next":"1"}}"#,
        )
        .unwrap();

        let found = FsProjectInspector.technologies(&path);

        assert_eq!(found, ["Node.js", "React", "Next.js"]);
    }

    #[test]
    fn an_empty_or_missing_folder_has_nothing_to_report() {
        let empty = project("empty", &[]);

        assert!(FsProjectInspector.is_directory(&empty));
        assert_eq!(
            FsProjectInspector.technologies(&empty),
            Vec::<String>::new()
        );
        assert!(!FsProjectInspector.is_directory("/definitely/not/a/folder"));
        assert!(!FsProjectInspector.is_directory(&format!("{empty}/package.json")));
        assert_eq!(
            FsProjectInspector.technologies("/definitely/not/a/folder"),
            Vec::<String>::new()
        );
    }
}
