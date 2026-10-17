//! Discord Rich Presence (proof of concept): what the person chose to share about what Atlas is doing.

use serde::{Deserialize, Serialize};

/// The Discord application of Atlas itself. Not a secret: it only names the app ("Atlas") on the
/// profile and where the images come from, so every install can share it.
pub const DEFAULT_CLIENT_ID: &str = "1560879410364686366";

/// Off until the person switches it on: it tells someone else what they are working on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresenceSettings {
    #[serde(default)]
    pub enabled: bool,
    /// Whether the names of the workspace, the workflow and the step go to Discord. Off: only
    /// generic texts ("Running a workflow").
    #[serde(default = "show_names_default")]
    pub show_names: bool,
    /// Another Discord application id (for a fork). `None`: `ATLAS_DISCORD_CLIENT_ID`, then Atlas's.
    #[serde(default)]
    pub client_id: Option<String>,
}

const fn show_names_default() -> bool {
    true
}

impl Default for PresenceSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            show_names: true,
            client_id: None,
        }
    }
}

impl PresenceSettings {
    /// The application id to use: the setting, then `ATLAS_DISCORD_CLIENT_ID`, then Atlas's own.
    pub fn resolved_client_id(&self) -> String {
        self.client_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_owned)
            .or_else(|| {
                std::env::var("ATLAS_DISCORD_CLIENT_ID")
                    .ok()
                    .map(|id| id.trim().to_owned())
                    .filter(|id| !id.is_empty())
            })
            .unwrap_or_else(|| DEFAULT_CLIENT_ID.to_owned())
    }
}

/// What Discord shows: two lines, a timer and two images.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activity {
    pub details: String,
    pub state: String,
    /// Seconds since the Unix epoch the timer counts from.
    pub started_at: Option<u64>,
    /// Hover text of the large image.
    pub large_text: Option<String>,
    /// Asset key (uploaded in the Developer Portal) of the small image.
    pub small_image: Option<&'static str>,
    pub small_text: Option<String>,
}
