//! What can shape an execution besides the prompt Atlas wrote, and who controls it.
//!
//! Atlas's context is not the runtime's effective context: a CLI adds its own system prompt, loads
//! the user's instruction files, plugins and hooks. The goal is not to control what Atlas cannot
//! control, only not to pretend it does. Each entry says **who controls** the source
//! ([`SurfaceControl`]) and **how Atlas knows** ([`Observation`]); neither says the model
//! received it (see [`SurfaceEntry::reaches_model`]).

use serde::{Deserialize, Serialize};

use crate::domain::runtime::SystemPromptChannel;

/// Who decides whether this source applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceControl {
    /// Atlas chose it and wrote it (the prompt, the flags and tool list it passes).
    AtlasControlled,
    /// The runtime adds it on its own (its default system prompt, its built-in plugins).
    RuntimeControlled,
    /// It comes from the user's own setup (instruction files, installed plugins, hooks, settings).
    UserControlled,
    /// Atlas cannot say.
    Unknown,
}

/// How Atlas knows an entry exists. Every value falls short of "the model received it".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Observation {
    /// The runtime said so itself (for example the CLI's start-up report). That it exists or was
    /// loaded is reported; that it reached the model is not.
    Reported,
    /// Atlas knows by its own design (a flag it passes, the prompt it built).
    Declared,
    /// Nothing observed: it may exist.
    NotObserved,
}

/// What kind of source an entry is. A closed set the UI translates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceKind {
    /// The prompt Atlas delivers (the one the Manifest hashes).
    Prompt,
    /// The arguments Atlas launches the runtime with.
    LaunchFlags,
    /// The runtime's own tools.
    Tools,
    McpServers,
    /// Skills and slash commands.
    Skills,
    Plugins,
    /// The runtime's own system prompt and built-in tool descriptions.
    SystemPrompt,
    /// How Atlas's own system instructions reach the runtime: `prompt_body` (no system channel is
    /// used, they are the first section of the one text), `native` or `appended`.
    SystemChannel,
    /// Instruction files the user keeps (`CLAUDE.md`).
    UserInstructions,
    Hooks,
    /// The user's permission rules and other settings of the runtime.
    UserSettings,
    /// A memory the runtime keeps by itself.
    AutoMemory,
    /// Whatever else the runtime or the user's setup may add.
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceEntry {
    pub kind: SurfaceKind,
    pub control: SurfaceControl,
    pub observation: Observation,
    /// The model's input is confirmed to contain this: true only for what Atlas delivered and
    /// hashed. `Reported` and `Declared` never set it.
    pub reaches_model: bool,
    /// A fact, not prose: a list of names, flags or a path. Never a secret.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl SurfaceEntry {
    pub fn new(kind: SurfaceKind, control: SurfaceControl, observation: Observation) -> Self {
        Self {
            kind,
            control,
            observation,
            reaches_model: false,
            detail: None,
        }
    }

    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// How Atlas's system instructions travel: declared by Atlas (it is the one that sends them),
    /// whatever the runtime then does with them. Unsupported means they are in the prompt body.
    pub fn system_channel(channel: SystemPromptChannel) -> Self {
        let how = match channel {
            SystemPromptChannel::Unsupported => "prompt_body",
            SystemPromptChannel::Native => "native",
            SystemPromptChannel::Appended => "appended",
        };
        Self::new(
            SurfaceKind::SystemChannel,
            SurfaceControl::AtlasControlled,
            Observation::Declared,
        )
        .with_detail(how)
    }

    /// The prompt Atlas delivered: the only entry whose arrival at the model is established.
    pub fn delivered_prompt() -> Self {
        Self {
            reaches_model: true,
            ..Self::new(
                SurfaceKind::Prompt,
                SurfaceControl::AtlasControlled,
                Observation::Declared,
            )
        }
    }
}

/// The whole surface of one execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSurface {
    pub runtime_id: String,
    pub entries: Vec<SurfaceEntry>,
}

impl RuntimeSurface {
    /// The surface of any runtime: what Atlas delivered, how its instructions travelled, and an
    /// honest entry for the rest.
    pub fn baseline(runtime_id: &str, channel: SystemPromptChannel) -> Self {
        Self {
            runtime_id: runtime_id.to_owned(),
            entries: vec![
                SurfaceEntry::delivered_prompt(),
                SurfaceEntry::system_channel(channel),
                SurfaceEntry::new(
                    SurfaceKind::Other,
                    SurfaceControl::Unknown,
                    Observation::NotObserved,
                ),
            ],
        }
    }

    pub fn of(runtime_id: &str, entries: Vec<SurfaceEntry>) -> Self {
        Self {
            runtime_id: runtime_id.to_owned(),
            entries,
        }
    }

    #[cfg(test)]
    pub fn count(&self, control: SurfaceControl) -> usize {
        self.entries.iter().filter(|e| e.control == control).count()
    }

    /// Whether part of the surface is not known to Atlas: something unobserved or of unknown
    /// control. Always true for a CLI runtime today.
    pub fn partly_unobserved(&self) -> bool {
        self.entries.iter().any(|e| {
            e.observation == Observation::NotObserved || e.control == SurfaceControl::Unknown
        })
    }

    /// Records what the runtime reported. An entry of the same kind and control that Atlas
    /// declared becomes `Reported` and takes the detail; otherwise a new `Reported` entry is added
    /// under the control the caller gives.
    pub fn report(&mut self, kind: SurfaceKind, control: SurfaceControl, detail: Option<String>) {
        let existing = self
            .entries
            .iter_mut()
            .find(|e| e.kind == kind && e.control == control);
        if let Some(entry) = existing {
            entry.observation = Observation::Reported;
            if detail.is_some() {
                entry.detail = detail;
            }
        } else {
            let mut entry = SurfaceEntry::new(kind, control, Observation::Reported);
            entry.detail = detail;
            // Before the closing "everything else" entry.
            let at = self
                .entries
                .iter()
                .position(|e| e.kind == SurfaceKind::Other)
                .unwrap_or(self.entries.len());
            self.entries.insert(at, entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_system_channel_is_declared_by_atlas_and_never_invented() {
        let body = SurfaceEntry::system_channel(SystemPromptChannel::Unsupported);
        assert_eq!(body.detail.as_deref(), Some("prompt_body"));
        assert_eq!(body.control, SurfaceControl::AtlasControlled);
        assert_eq!(body.observation, Observation::Declared);
        assert!(!body.reaches_model);
        assert_eq!(
            SurfaceEntry::system_channel(SystemPromptChannel::Native)
                .detail
                .as_deref(),
            Some("native")
        );
    }

    #[test]
    fn the_baseline_confirms_only_the_prompt_and_admits_the_rest() {
        let surface = RuntimeSurface::baseline("rt", SystemPromptChannel::Unsupported);

        let prompt = &surface.entries[0];
        assert_eq!(prompt.kind, SurfaceKind::Prompt);
        assert_eq!(prompt.control, SurfaceControl::AtlasControlled);
        assert_eq!(prompt.observation, Observation::Declared);
        assert!(prompt.reaches_model);

        let rest = surface.entries.last().unwrap();
        assert_eq!(rest.control, SurfaceControl::Unknown);
        assert_eq!(rest.observation, Observation::NotObserved);
        assert!(!rest.reaches_model);
        assert!(surface.partly_unobserved());
        // The prompt and how Atlas's instructions travel.
        assert_eq!(surface.count(SurfaceControl::AtlasControlled), 2);
        assert_eq!(surface.count(SurfaceControl::UserControlled), 0);
    }

    #[test]
    fn what_a_runtime_reports_exists_but_is_never_confirmed_as_received() {
        let mut surface = RuntimeSurface::baseline("rt", SystemPromptChannel::Unsupported);

        surface.report(
            SurfaceKind::Plugins,
            SurfaceControl::UserControlled,
            Some("atlassian@official".to_owned()),
        );

        let plugins = surface
            .entries
            .iter()
            .find(|e| e.kind == SurfaceKind::Plugins)
            .unwrap();
        assert_eq!(plugins.observation, Observation::Reported);
        assert_eq!(plugins.control, SurfaceControl::UserControlled);
        assert!(!plugins.reaches_model);
        // The closing entry stays last.
        assert_eq!(surface.entries.last().unwrap().kind, SurfaceKind::Other);
    }

    #[test]
    fn reporting_a_declared_entry_upgrades_it_instead_of_duplicating_it() {
        let mut surface = RuntimeSurface::of(
            "rt",
            vec![SurfaceEntry::new(
                SurfaceKind::Tools,
                SurfaceControl::AtlasControlled,
                Observation::Declared,
            )
            .with_detail("Read")],
        );

        surface.report(
            SurfaceKind::Tools,
            SurfaceControl::AtlasControlled,
            Some("Read,Grep".to_owned()),
        );

        assert_eq!(surface.entries.len(), 1);
        assert_eq!(surface.entries[0].observation, Observation::Reported);
        assert_eq!(surface.entries[0].detail.as_deref(), Some("Read,Grep"));
        assert!(!surface.partly_unobserved());
    }

    #[test]
    fn the_four_controls_and_three_observations_are_distinct_values() {
        use SurfaceControl::{AtlasControlled, RuntimeControlled, Unknown, UserControlled};
        let all = [AtlasControlled, RuntimeControlled, UserControlled, Unknown];
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a, b);
            }
        }
        assert_eq!(
            serde_json::to_string(&Observation::NotObserved).unwrap(),
            "\"not_observed\""
        );
        assert_eq!(
            serde_json::to_string(&SurfaceControl::UserControlled).unwrap(),
            "\"user_controlled\""
        );
    }
}
