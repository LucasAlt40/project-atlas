pub use crate::domain::project::folder_name;
use crate::domain::project::ProjectContext;

/// Port: what Atlas may learn about a project folder. Implemented in `infrastructure/`.
///
/// Deliberately small: is it a folder, and which well-known technologies do the marker files in
/// its top level suggest. Nothing is read beyond that and nothing is sent anywhere.
pub trait ProjectInspector: Send + Sync {
    fn is_directory(&self, path: &str) -> bool;
    fn technologies(&self, path: &str) -> Vec<String>;
    /// The folder as it really is: symlinks resolved and, where the file system ignores case, the
    /// real spelling, so two spellings of one folder compare equal. A folder that cannot be
    /// resolved comes back as given, without a trailing separator.
    fn canonical(&self, path: &str) -> String {
        path.trim_end_matches(['/', '\\']).to_owned()
    }
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
        /// Spellings that resolve to another folder (a symlink, a different case).
        pub aliases: BTreeMap<String, String>,
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
                aliases: BTreeMap::new(),
            }
        }

        pub fn alias(mut self, spelling: &str, folder: &str) -> Self {
            self.aliases.insert(spelling.to_owned(), folder.to_owned());
            self
        }
    }

    impl ProjectInspector for FakeInspector {
        fn is_directory(&self, path: &str) -> bool {
            let path = path.trim_end_matches('/');
            self.folders.contains_key(path) || self.aliases.contains_key(path)
        }

        fn technologies(&self, path: &str) -> Vec<String> {
            self.folders.get(path).cloned().unwrap_or_default()
        }

        fn canonical(&self, path: &str) -> String {
            let path = path.trim_end_matches('/');
            self.aliases
                .get(path)
                .cloned()
                .unwrap_or_else(|| path.to_owned())
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
