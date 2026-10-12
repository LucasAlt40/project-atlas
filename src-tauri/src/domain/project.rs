use serde::Serialize;

/// What an agent is told about the project. This is the seam the future Harness
/// (architecture, conventions, constraints, memory…) will grow behind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectContext {
    /// The project folder's name.
    pub name: String,
    pub path: String,
    /// Technologies recognised from marker files in the folder (see `ProjectInspector`).
    pub technologies: Vec<String>,
}

/// The folder's own name (the last path component, either kind of separator), or the whole path
/// if it has none.
pub fn folder_name(path: &str) -> String {
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .map_or_else(|| path.to_owned(), str::to_owned)
}

/// A project of the execution's scope besides the working one, as the prompt tells it. Always
/// consulted only: an execution changes the one project it works in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectNote {
    pub name: String,
    pub path: String,
    pub role: Option<String>,
    pub technologies: Vec<String>,
    /// The runtime was given the folder: without it the agent can be told about the project but
    /// cannot read it.
    pub mounted: bool,
}
