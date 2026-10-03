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
