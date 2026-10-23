use serde::Serialize;

/// Static identity of the running application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub platform: String,
    /// What happened to the stored config at start, when it could not be read as it was.
    #[serde(rename = "configNotice", skip_serializing_if = "Option::is_none")]
    pub config_notice: Option<ConfigNotice>,
}

/// The stored config was not read as it was: what Atlas did about it, for the person to know.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConfigNotice {
    /// The file was JSON but held records this version cannot read (another version wrote it):
    /// those records were left out, the rest was kept, and the original file is at `backup`.
    Recovered {
        backup: String,
        dropped: Vec<DroppedRecords>,
    },
    /// The file was not JSON at all: Atlas started empty, and the original file is at `backup`.
    Reset { backup: String },
    /// The file could not be read (permissions, disk): nothing is saved until Atlas restarts, so
    /// the file is never overwritten with an empty config.
    Unreadable { path: String },
}

/// How many records of one part of the config (`agents`, `workflowExecutions`…) were left out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DroppedRecords {
    pub section: String,
    pub count: usize,
}
