use std::path::Path;

use crate::domain::project::ProjectContext;

/// Port: what Atlas may learn about a project folder. Implemented in `infrastructure/`.
///
/// Deliberately small: is it a folder, and which well-known technologies do the marker files in
/// its top level suggest. Nothing is read beyond that and nothing is sent anywhere.
pub trait ProjectInspector: Send + Sync {
    fn is_directory(&self, path: &str) -> bool;
    fn technologies(&self, path: &str) -> Vec<String>;
}

/// The folder's own name (the last path component), or the whole path if it has none.
pub fn folder_name(path: &str) -> String {
    Path::new(path).file_name().map_or_else(
        || path.to_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// What an agent is told about the project at `path`.
pub fn project_context(inspector: &dyn ProjectInspector, path: &str) -> ProjectContext {
    ProjectContext {
        name: folder_name(path),
        path: path.to_owned(),
        technologies: inspector.technologies(path),
    }
}

#[cfg(test)]
pub mod fake {
    use std::collections::BTreeMap;

    use super::ProjectInspector;

    /// Folders that "exist", each with the technologies it "contains".
    #[derive(Default)]
    pub struct FakeInspector {
        pub folders: BTreeMap<String, Vec<String>>,
    }

    impl FakeInspector {
        pub fn with(folders: &[(&str, &[&str])]) -> Self {
            Self {
                folders: folders
                    .iter()
                    .map(|(path, tech)| {
                        (
                            (*path).to_owned(),
                            tech.iter().map(|t| (*t).to_owned()).collect(),
                        )
                    })
                    .collect(),
            }
        }
    }

    impl ProjectInspector for FakeInspector {
        fn is_directory(&self, path: &str) -> bool {
            self.folders.contains_key(path)
        }

        fn technologies(&self, path: &str) -> Vec<String> {
            self.folders.get(path).cloned().unwrap_or_default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::FakeInspector;
    use super::*;

    #[test]
    fn names_the_project_after_its_folder() {
        assert_eq!(folder_name("/home/lucas/dev/acme"), "acme");
        assert_eq!(folder_name("/home/lucas/dev/acme/"), "acme");
    }

    #[test]
    fn the_context_carries_the_path_and_detected_technologies() {
        let inspector = FakeInspector::with(&[("/dev/erp", &["Angular", "Git"])]);

        let context = project_context(&inspector, "/dev/erp");

        assert_eq!(context.name, "erp");
        assert_eq!(context.path, "/dev/erp");
        assert_eq!(context.technologies, ["Angular", "Git"]);
    }
}
