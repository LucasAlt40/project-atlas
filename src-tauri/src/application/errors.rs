use std::collections::BTreeMap;
use std::fmt;

use serde::Serialize;

/// Every user-facing failure the core can report. The UI turns a code into a message in the
/// user's language; the core never translates. Adding a code means adding its text on the
/// frontend (`src/i18n`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    NameRequired,
    NameTooLong,
    InstructionsRequired,
    PersonalityRequired,
    RuntimeRequired,
    RuntimeNotSupported,
    ModelRequired,
    ModelInvalid,
    PersonalityNotFound,
    PersonalityInUse,
    AgentNotFound,
    AgentBusy,
    AgentAlreadyPlaced,
    WorkspaceNotFound,
    WorkspaceFull,
    WorkspaceBusy,
    ProjectFolderRequired,
    ProjectFolderNotFound,
    MessageEmpty,
    LanguageUnsupported,
    StorageFailed,
}

/// A failure with a stable code, values for the message (`params`), and an optional technical
/// `detail` that is for logs and "technical details", never the primary message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppError {
    pub code: ErrorCode,
    pub params: BTreeMap<String, String>,
    pub detail: Option<String>,
}

impl AppError {
    pub fn new(code: ErrorCode) -> Self {
        Self {
            code,
            params: BTreeMap::new(),
            detail: None,
        }
    }

    #[must_use]
    pub fn with(mut self, key: &str, value: impl Into<String>) -> Self {
        self.params.insert(key.to_owned(), value.into());
        self
    }

    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn storage(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::StorageFailed).with_detail(detail)
    }

    #[cfg(test)]
    pub fn is(&self, code: ErrorCode) -> bool {
        self.code == code
    }
}

/// English text, for logs and as a last-resort fallback; the UI uses the code.
impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.code)?;
        if let Some(detail) = &self.detail {
            write!(f, ": {detail}")?;
        }
        Ok(())
    }
}

impl std::error::Error for AppError {}
