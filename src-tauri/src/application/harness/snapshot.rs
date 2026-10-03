use std::collections::BTreeMap;

/// What a scanner saw of a project: names of entries and the text of a few well-known manifest
/// files. The analyzer works only from this, so it can be tested without a disk and never runs
/// anything.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanSnapshot {
    pub root_name: String,
    /// Relative paths with `/` separators, directories flagged. Symlinks are listed as entries
    /// of neither kind: they are never followed.
    pub entries: Vec<ScanEntry>,
    /// Text of known manifests, by relative path. Never `.env` files or anything secret.
    pub files: BTreeMap<String, String>,
    /// The first line of `.git/HEAD` when `.git` is a folder.
    pub git_head: Option<String>,
    /// A limit (depth or entry count) stopped the scan early.
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanEntry {
    pub path: String,
    pub is_dir: bool,
}

impl ScanSnapshot {
    #[cfg(test)]
    pub fn dir(&mut self, path: &str) {
        self.entries.push(ScanEntry {
            path: path.to_owned(),
            is_dir: true,
        });
    }

    #[cfg(test)]
    pub fn file(&mut self, path: &str, content: Option<&str>) {
        self.entries.push(ScanEntry {
            path: path.to_owned(),
            is_dir: false,
        });
        if let Some(content) = content {
            self.files.insert(path.to_owned(), content.to_owned());
        }
    }

    pub fn has_dir(&self, path: &str) -> bool {
        self.entries.iter().any(|e| e.is_dir && e.path == path)
    }

    pub fn has_path(&self, path: &str) -> bool {
        self.entries.iter().any(|e| e.path == path)
    }

    /// Files (not folders) at most `max_depth` folders below the root whose name matches,
    /// case-insensitively. Depth 0 is the root itself.
    pub fn files_named<'a>(&'a self, name: &str, max_depth: usize) -> Vec<&'a str> {
        self.entries
            .iter()
            .filter(|e| !e.is_dir && depth(&e.path) <= max_depth)
            .filter(|e| file_name(&e.path).eq_ignore_ascii_case(name))
            .map(|e| e.path.as_str())
            .collect()
    }

    /// Files whose name ends with `.extension`, at most `max_depth` folders deep.
    pub fn files_with_extension<'a>(&'a self, extension: &str, max_depth: usize) -> Vec<&'a str> {
        let suffix = format!(".{extension}");
        self.entries
            .iter()
            .filter(|e| !e.is_dir && depth(&e.path) <= max_depth)
            .filter(|e| file_name(&e.path).to_ascii_lowercase().ends_with(&suffix))
            .map(|e| e.path.as_str())
            .collect()
    }

    /// Direct subfolders of `parent` (`""` is the root).
    pub fn subdirs(&self, parent: &str) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|e| e.is_dir && parent_of(&e.path) == parent)
            .map(|e| file_name(&e.path))
            .collect()
    }

    /// Every scanned folder's relative path.
    pub fn all_dirs(&self) -> impl Iterator<Item = &str> {
        self.entries
            .iter()
            .filter(|e| e.is_dir)
            .map(|e| e.path.as_str())
    }
}

/// How many folders deep a path is: `a.json` is 0, `x/a.json` is 1.
pub fn depth(path: &str) -> usize {
    path.matches('/').count()
}

pub fn file_name(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

pub fn parent_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}
