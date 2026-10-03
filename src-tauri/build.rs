use tauri_build::{AppManifest, Attributes};

/// Every command the frontend may call must be listed here. Tauri then generates an
/// `allow-<command>` permission for it, and the command is unreachable from the webview
/// unless a capability in `capabilities/` grants that permission.
const COMMANDS: &[&str] = &[
    "get_app_info",
    "list_personalities",
    "create_personality",
    "update_personality",
    "delete_personality",
    "restore_default_personalities",
    "list_runtimes",
    "list_agents",
    "create_agent",
    "update_agent",
    "delete_agent",
    "list_workspaces",
    "create_workspace",
    "update_workspace",
    "delete_workspace",
    "add_agent_to_workspace",
    "remove_agent_from_workspace",
    "get_project_context",
    "get_settings",
    "set_language",
    "select_workspace",
    "send_message",
    "list_messages",
    "get_agent_usage",
    "get_workspace_usage",
];

fn main() {
    tauri_build::try_build(Attributes::new().app_manifest(AppManifest::new().commands(COMMANDS)))
        .expect("failed to run tauri-build");
}
