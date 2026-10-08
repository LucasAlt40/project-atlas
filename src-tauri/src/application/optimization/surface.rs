//! Fills a runtime's `RuntimeSurface` with what the runtime reported after it ran.
//!
//! The `Declared` half (what Atlas passes) comes from `ModelRuntime::surface` before the run. This
//! is the `Reported` half: the facts the adapter put in the output's metadata (`toolsExposed`,
//! `mcpServers`, `skillsLoaded`, `slashCommands`, `pluginsLoaded`, `pluginSources`,
//! `autoMemoryPath`). A key that is absent says nothing; nothing is inferred from it.

use std::collections::BTreeMap;

use crate::domain::context::{RuntimeSurface, SurfaceControl, SurfaceKind};

/// The suffix the CLI gives its own built-in plugins (`name@builtin`).
const BUILTIN_SOURCE: &str = "@builtin";

pub fn with_reported(
    mut surface: RuntimeSurface,
    metadata: &BTreeMap<String, String>,
) -> RuntimeSurface {
    let get = |key: &str| metadata.get(key).map(String::as_str);
    let mut known = |kind: SurfaceKind, fallback: SurfaceControl, detail: String| {
        // The control is whoever Atlas declared the entry under, else the runtime's.
        let control = surface
            .entries
            .iter()
            .find(|e| e.kind == kind)
            .map_or(fallback, |e| e.control);
        surface.report(kind, control, Some(detail));
    };
    if let Some(tools) = get("toolsExposed") {
        known(
            SurfaceKind::Tools,
            SurfaceControl::RuntimeControlled,
            tools.to_owned(),
        );
    }
    if let Some(servers) = get("mcpServers") {
        let detail = if servers.is_empty() {
            "none".to_owned()
        } else {
            servers.to_owned()
        };
        known(
            SurfaceKind::McpServers,
            SurfaceControl::RuntimeControlled,
            detail,
        );
    }
    let skills: Vec<String> = [
        ("skillsLoaded", "skills"),
        ("slashCommands", "slash commands"),
    ]
    .iter()
    .filter_map(|(key, label)| get(key).map(|count| format!("{count} {label}")))
    .collect();
    if !skills.is_empty() {
        known(
            SurfaceKind::Skills,
            SurfaceControl::RuntimeControlled,
            skills.join(", "),
        );
    }
    match (get("pluginSources"), get("pluginsLoaded")) {
        (Some(sources), _) => {
            let (builtin, user): (Vec<&str>, Vec<&str>) = sources
                .split(',')
                .filter(|s| !s.is_empty())
                .partition(|s| s.ends_with(BUILTIN_SOURCE));
            for (control, names) in [
                (SurfaceControl::RuntimeControlled, builtin),
                (SurfaceControl::UserControlled, user),
            ] {
                if !names.is_empty() {
                    surface.report(SurfaceKind::Plugins, control, Some(names.join(",")));
                }
            }
        }
        // A count without sources: Atlas cannot say whose they are.
        (None, Some(count)) => {
            surface.report(
                SurfaceKind::Plugins,
                SurfaceControl::Unknown,
                Some(format!("{count} plugins")),
            );
        }
        (None, None) => {}
    }
    if let Some(path) = get("autoMemoryPath") {
        surface.report(
            SurfaceKind::AutoMemory,
            SurfaceControl::RuntimeControlled,
            Some(path.to_owned()),
        );
    }
    surface
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::context::{Observation, SurfaceEntry};
    use crate::domain::runtime::SystemPromptChannel;

    fn metadata(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    fn entry(
        surface: &RuntimeSurface,
        kind: SurfaceKind,
        control: SurfaceControl,
    ) -> Option<&SurfaceEntry> {
        surface
            .entries
            .iter()
            .find(|e| e.kind == kind && e.control == control)
    }

    #[test]
    fn plugins_are_split_between_the_runtimes_own_and_the_users() {
        let surface = with_reported(
            RuntimeSurface::baseline("claude", SystemPromptChannel::Unsupported),
            &metadata(&[
                (
                    "pluginSources",
                    "figma@claude-plugins-official,cc-plugin-telemetry@builtin",
                ),
                ("pluginsLoaded", "2"),
            ]),
        );

        let user = entry(
            &surface,
            SurfaceKind::Plugins,
            SurfaceControl::UserControlled,
        )
        .unwrap();
        assert_eq!(
            user.detail.as_deref(),
            Some("figma@claude-plugins-official")
        );
        assert_eq!(user.observation, Observation::Reported);
        let own = entry(
            &surface,
            SurfaceKind::Plugins,
            SurfaceControl::RuntimeControlled,
        )
        .unwrap();
        assert_eq!(own.detail.as_deref(), Some("cc-plugin-telemetry@builtin"));
        // Reported is not received.
        assert!(!user.reaches_model && !own.reaches_model);
    }

    #[test]
    fn a_plugin_count_without_sources_is_attributed_to_nobody() {
        let surface = with_reported(
            RuntimeSurface::baseline("rt", SystemPromptChannel::Unsupported),
            &metadata(&[("pluginsLoaded", "3")]),
        );

        let plugins = entry(&surface, SurfaceKind::Plugins, SurfaceControl::Unknown).unwrap();
        assert_eq!(plugins.detail.as_deref(), Some("3 plugins"));
    }

    #[test]
    fn what_atlas_declared_is_upgraded_to_reported_under_the_same_control() {
        let declared = RuntimeSurface::of(
            "claude",
            vec![SurfaceEntry::new(
                SurfaceKind::Tools,
                SurfaceControl::AtlasControlled,
                Observation::Declared,
            )
            .with_detail("Read,Grep,Glob")],
        );

        let surface = with_reported(
            declared,
            &metadata(&[("toolsExposed", "Glob,Grep,Read,mcp__x__y")]),
        );

        assert_eq!(surface.entries.len(), 1);
        assert_eq!(surface.entries[0].control, SurfaceControl::AtlasControlled);
        assert_eq!(surface.entries[0].observation, Observation::Reported);
        // The report is what the runtime said, including what Atlas did not grant.
        assert_eq!(
            surface.entries[0].detail.as_deref(),
            Some("Glob,Grep,Read,mcp__x__y")
        );
    }

    #[test]
    fn auto_memory_is_runtime_controlled_and_not_claimed_to_reach_the_model() {
        let surface = with_reported(
            RuntimeSurface::baseline("claude", SystemPromptChannel::Unsupported),
            &metadata(&[("autoMemoryPath", "/home/u/.claude/projects/p/memory/")]),
        );

        let memory = entry(
            &surface,
            SurfaceKind::AutoMemory,
            SurfaceControl::RuntimeControlled,
        )
        .unwrap();
        assert_eq!(memory.observation, Observation::Reported);
        assert!(!memory.reaches_model);
    }

    #[test]
    fn a_runtime_that_reported_nothing_adds_nothing() {
        let before = RuntimeSurface::baseline("rt", SystemPromptChannel::Unsupported);

        let after = with_reported(before.clone(), &BTreeMap::new());

        assert_eq!(after, before);
    }
}
