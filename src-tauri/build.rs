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
    "list_executions",
    "get_agent_usage",
    "get_workspace_usage",
    "get_workspace_security",
    "get_agent_permissions",
    "set_agent_permission_profile",
    "list_pending_approvals",
    "resolve_approval",
    "get_execution_terminal",
    "execution_interrupt",
    "execution_terminate",
    "execution_terminal_input",
    "execution_terminal_resize",
    "list_execution_worktrees",
    "merge_execution",
    "analyze_project",
    "initialize_project",
    "get_project_harness",
    "refresh_project_harness",
];

fn main() {
    tauri_build::try_build(Attributes::new().app_manifest(AppManifest::new().commands(COMMANDS)))
        .expect("failed to run tauri-build");
}
