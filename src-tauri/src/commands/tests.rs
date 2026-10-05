//! Calls every command through the real generated context, so a missing capability grant
//! (or a handler that is not registered) fails here.

use serde_json::{json, Value};
use tauri::test::{mock_builder, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{ipc::CallbackFn, Listener, WebviewWindow, WebviewWindowBuilder};

use crate::application::app_info::{AppInfoService, PlatformInfo};
use crate::state::AppState;

struct FakePlatform;

impl PlatformInfo for FakePlatform {
    fn platform_name(&self) -> String {
        "testos".to_owned()
    }
}

fn window() -> (tauri::App<MockRuntime>, WebviewWindow<MockRuntime>) {
    let app_info = AppInfoService::new("9.9.9".to_owned(), std::sync::Arc::new(FakePlatform));
    let app = mock_builder()
        .manage(AppState::for_tests(app_info))
        .invoke_handler(tauri::generate_handler![
            super::app::get_app_info,
            super::personalities::list_personalities,
            super::personalities::create_personality,
            super::personalities::update_personality,
            super::personalities::delete_personality,
            super::personalities::restore_default_personalities,
            super::runtimes::list_runtimes,
            super::agents::list_agents,
            super::agents::create_agent,
            super::agents::update_agent,
            super::agents::set_agent_result_contract,
            super::agents::delete_agent,
            super::workspace::list_workspaces,
            super::workspace::create_workspace,
            super::workspace::update_workspace,
            super::workspace::delete_workspace,
            super::workspace::add_agent_to_workspace,
            super::workspace::remove_agent_from_workspace,
            super::workspace::get_project_context,
            super::settings::get_settings,
            super::settings::set_language,
            super::settings::select_workspace,
            super::chat::send_message,
            super::chat::list_messages,
            super::chat::list_executions,
            super::usage::get_agent_usage,
            super::usage::get_workspace_usage,
            super::security::get_workspace_security,
            super::security::get_agent_permissions,
            super::security::set_agent_permission_profile,
            super::security::list_pending_approvals,
            super::security::resolve_approval,
            super::terminal::get_execution_terminal,
            super::terminal::execution_interrupt,
            super::terminal::execution_terminate,
            super::terminal::execution_terminal_input,
            super::terminal::execution_terminal_resize,
            super::worktree::list_execution_worktrees,
            super::worktree::merge_execution,
            super::harness::analyze_project,
            super::harness::initialize_project,
            super::harness::get_project_harness,
            super::harness::refresh_project_harness,
            super::harness::preview_task_context,
            super::live_workspace::get_live_workspace,
            super::live_workspace::refresh_live_workspace,
            super::live_workspace::get_live_file,
            super::live_workspace::get_live_diff,
            super::workflow::list_workflows,
            super::workflow::get_workflow,
            super::workflow::list_workflow_templates,
            super::workflow::select_workflow_template,
            super::workflow::create_workflow,
            super::workflow::create_workflow_from_template,
            super::workflow::update_workflow,
            super::workflow::delete_workflow,
            super::workflow::validate_workflow,
            super::workflow::start_workflow,
            super::workflow::pause_workflow,
            super::workflow::resume_workflow,
            super::workflow::cancel_workflow,
            super::workflow::answer_workflow_interaction,
            super::workflow::list_pending_interactions,
            super::workflow::get_workflow_execution,
            super::workflow::list_workflow_executions,
            super::workflow::get_workflow_changes,
            super::workflow::get_workflow_diff,
            super::workflow::apply_workflow_changes,
            super::workflow::keep_workflow_changes,
            super::workflow::discard_workflow_changes,
            super::workflow::list_ides,
            super::workflow::open_workflow_in_ide,
        ])
        .build(crate::context())
        .expect("failed to build mock app");
    let window = WebviewWindowBuilder::new(&app, "main", tauri::WebviewUrl::default())
        .build()
        .expect("failed to build mock window");
    (app, window)
}

fn invoke(window: &WebviewWindow<MockRuntime>, cmd: &str, body: Value) -> Result<Value, Value> {
    tauri::test::get_ipc_response(
        window,
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "tauri://localhost".parse().unwrap(),
            body: tauri::ipc::InvokeBody::Json(body),
            headers: tauri::http::HeaderMap::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .map(|response| response.deserialize().expect("invalid response"))
}

fn create_workspace(window: &WebviewWindow<MockRuntime>, name: &str, path: &str) -> Value {
    invoke(
        window,
        "create_workspace",
        json!({ "input": { "name": name, "projectPath": path, "description": "d" } }),
    )
    .unwrap()
}

fn create_agent(window: &WebviewWindow<MockRuntime>) -> Value {
    invoke(
        window,
        "create_agent",
        json!({ "request": {
            "name": "Architecture Expert", "personalityId": "architect",
            "runtimeId": "fake", "modelId": "m1", "instructions": "Be brief.",
            // The fake project folder is not a Git repository.
            "worktreeIsolation": false,
        }}),
    )
    .unwrap()
}

const WINDOWS: fn() -> Value = || json!({ "todayStart": 0, "weekStart": 0, "monthStart": 0 });

#[test]
fn get_app_info_is_allowed_by_capability_and_returns_app_info() {
    let (_app, window) = window();

    assert_eq!(
        invoke(&window, "get_app_info", json!({})).unwrap(),
        json!({ "name": "Project Atlas", "version": "9.9.9", "platform": "testos" })
    );
}

#[test]
fn personalities_can_be_created_edited_deleted_and_restored() {
    let (_app, window) = window();
    let body = |name: &str| json!({ "name": name, "description": "d", "systemInstructions": "text", "tags": [] });

    let all = invoke(&window, "list_personalities", json!({})).unwrap();
    assert_eq!(all[0]["id"], "architect");
    assert_eq!(all[0]["source"], "builtin");
    let created = invoke(
        &window,
        "create_personality",
        json!({ "request": body("Mine") }),
    )
    .unwrap();
    assert_eq!(created["source"], "custom");
    let edited = invoke(
        &window,
        "update_personality",
        json!({ "id": "qa", "request": body("My QA") }),
    )
    .unwrap();
    assert_eq!(edited["name"], "My QA");
    invoke(&window, "delete_personality", json!({ "id": "developer" })).unwrap();
    let restored = invoke(&window, "restore_default_personalities", json!({})).unwrap();
    assert_eq!(restored.as_array().unwrap().len(), 6);
    assert_eq!(restored[2]["name"], "QA");
}

#[test]
fn errors_are_structured_codes_not_translated_text() {
    let (_app, window) = window();

    let error = invoke(
        &window,
        "create_personality",
        json!({ "request": { "name": " ", "description": "", "systemInstructions": "x", "tags": [] } }),
    )
    .unwrap_err();
    assert_eq!(error["code"], "name_required");
    assert_eq!(error["params"], json!({}));

    let missing = invoke(
        &window,
        "create_workspace",
        json!({ "input": { "name": "X", "projectPath": "/nowhere" } }),
    )
    .unwrap_err();
    assert_eq!(missing["code"], "project_folder_not_found");
    assert_eq!(missing["params"]["path"], "/nowhere");

    let ghost = invoke(&window, "update_personality", json!({ "id": "ghost", "request": { "name": "x", "description": "", "systemInstructions": "x", "tags": [] } })).unwrap_err();
    assert_eq!(ghost["code"], "personality_not_found");
}

#[test]
fn lists_runtimes_with_their_usage_capabilities() {
    let (_app, window) = window();

    let runtimes = invoke(&window, "list_runtimes", json!({})).unwrap();

    assert_eq!(runtimes[0]["runtime"]["id"], "fake");
    assert_eq!(runtimes[0]["availability"], "ready");
    assert_eq!(runtimes[0]["modelDiscovery"], "discovered");
    let caps = &runtimes[0]["runtime"]["capabilities"];
    assert_eq!(caps["usageMetrics"], true);
    assert_eq!(caps["costMetrics"], true);
    assert_eq!(caps["quotaMetrics"], false);
    assert_eq!(runtimes[0]["notice"], Value::Null);
}

#[test]
fn agents_can_be_edited_and_deleted() {
    let (_app, window) = window();
    let ws = create_workspace(&window, "Atlas", "/atlas");
    let agent = create_agent(&window);
    invoke(
        &window,
        "add_agent_to_workspace",
        json!({ "workspaceId": ws["id"], "agentId": agent["id"] }),
    )
    .unwrap();

    let edited = invoke(
        &window,
        "update_agent",
        json!({ "id": agent["id"], "request": {
            "name": "Renamed", "personalityId": "qa", "runtimeId": "fake", "modelId": "m1", "instructions": "new",
        }}),
    )
    .unwrap();
    assert_eq!(
        (edited["id"].clone(), edited["name"].clone()),
        (agent["id"].clone(), json!("Renamed"))
    );
    assert_eq!(edited["resultContract"]["kind"], "general");
    let contracted = invoke(
        &window,
        "set_agent_result_contract",
        json!({ "agentId": agent["id"], "contract": {
            "kind": "validation",
            "outcomes": [
                { "id": "pass", "label": "Pass", "description": "" },
                { "id": "fail", "label": "Fail" },
            ],
        }}),
    )
    .unwrap();
    assert_eq!(contracted["resultContract"]["outcomes"][1]["id"], "fail");
    for bad in [
        json!({ "kind": "validation", "outcomes": [{ "id": "a", "label": "A" }, { "id": "a", "label": "B" }] }),
        json!({ "kind": "custom", "outcomes": [{ "id": "Not Valid", "label": "A" }] }),
        json!({ "kind": "general", "outcomes": [{ "id": "x", "label": "X" }] }),
    ] {
        let refused = invoke(
            &window,
            "set_agent_result_contract",
            json!({ "agentId": agent["id"], "contract": bad }),
        )
        .unwrap_err();
        assert_eq!(refused["code"], "result_contract_invalid");
    }
    let blocked = invoke(&window, "delete_personality", json!({ "id": "qa" })).unwrap_err();
    assert_eq!(blocked["code"], "personality_in_use");
    assert_eq!(blocked["params"]["agents"], "Renamed");

    let workspaces = invoke(&window, "delete_agent", json!({ "id": agent["id"] })).unwrap();
    assert_eq!(workspaces[0]["layout"]["agentPlacements"], json!([]));
    assert_eq!(
        invoke(&window, "list_agents", json!({})).unwrap(),
        json!([])
    );
}

#[test]
fn workspaces_are_created_renamed_inspected_and_deleted() {
    let (_app, window) = window();

    let created = create_workspace(&window, "Atlas", "/atlas");
    assert_eq!(created["projectPath"], "/atlas");
    assert_eq!(created["layout"]["rows"], 2);
    let context = invoke(
        &window,
        "get_project_context",
        json!({ "workspaceId": created["id"] }),
    )
    .unwrap();
    assert_eq!(
        context,
        json!({ "name": "atlas", "path": "/atlas", "technologies": ["Rust"] })
    );

    let renamed = invoke(
        &window,
        "update_workspace",
        json!({ "id": created["id"], "input": { "name": "Renamed", "projectPath": "/other" } }),
    )
    .unwrap();
    assert_eq!(renamed["name"], "Renamed");
    assert_eq!(renamed["projectPath"], "/other");
    assert_eq!(
        invoke(&window, "list_workspaces", json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let remaining = invoke(&window, "delete_workspace", json!({ "id": created["id"] })).unwrap();
    assert_eq!(remaining, json!([]));
}

#[test]
fn the_harness_commands_report_status_and_validate_the_workspace() {
    let (_app, window) = window();
    let created = create_workspace(&window, "Atlas", "/atlas");
    let id = created["id"].clone();

    let status = invoke(&window, "get_project_harness", json!({ "workspaceId": id })).unwrap();
    assert_eq!(status["status"], "not_initialized");

    let analysis = invoke(&window, "analyze_project", json!({ "workspaceId": id })).unwrap();
    assert_eq!(analysis["existing"]["status"], "not_initialized");
    assert_eq!(analysis["partial"], false);

    let initialized = invoke(
        &window,
        "initialize_project",
        json!({ "workspaceId": id, "input": { "mode": "create", "purpose": "ERP" } }),
    )
    .unwrap();
    assert_eq!(initialized["summary"]["status"], "initialized");
    let again = invoke(
        &window,
        "initialize_project",
        json!({ "workspaceId": id, "input": { "mode": "create" } }),
    )
    .unwrap_err();
    assert_eq!(again["code"], "project_already_initialized");

    let refreshed = invoke(
        &window,
        "refresh_project_harness",
        json!({ "workspaceId": id }),
    )
    .unwrap();
    // Without `confirm` a refresh only reports what would change.
    assert_eq!(refreshed["applied"], serde_json::Value::Null);
    let applied = invoke(
        &window,
        "refresh_project_harness",
        json!({ "workspaceId": id, "confirm": true }),
    )
    .unwrap();
    assert_eq!(applied["applied"]["summary"]["status"], "initialized");
    let missing = invoke(
        &window,
        "analyze_project",
        json!({ "workspaceId": "ghost" }),
    )
    .unwrap_err();
    assert_eq!(missing["code"], "workspace_not_found");
}

#[test]
fn agent_placement_is_per_workspace_and_leaves_the_agent_alone() {
    let (_app, window) = window();
    let (a, b) = (
        create_workspace(&window, "A", "/atlas"),
        create_workspace(&window, "B", "/other"),
    );
    let agent = create_agent(&window);

    let placed = invoke(
        &window,
        "add_agent_to_workspace",
        json!({ "workspaceId": a["id"], "agentId": agent["id"] }),
    )
    .unwrap();
    assert_eq!(
        placed["layout"]["agentPlacements"][0]["position"],
        json!({ "row": 0, "column": 0 })
    );
    let in_b = invoke(
        &window,
        "add_agent_to_workspace",
        json!({ "workspaceId": b["id"], "agentId": agent["id"] }),
    )
    .unwrap();
    assert_eq!(
        in_b["layout"]["agentPlacements"].as_array().unwrap().len(),
        1
    );
    let removed = invoke(
        &window,
        "remove_agent_from_workspace",
        json!({ "workspaceId": a["id"], "agentId": agent["id"] }),
    )
    .unwrap();
    assert_eq!(removed["layout"]["agentPlacements"], json!([]));
    assert_eq!(
        invoke(&window, "list_agents", json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn settings_default_to_portuguese_and_persist_language_and_selected_workspace() {
    let (_app, window) = window();
    let ws = create_workspace(&window, "Atlas", "/atlas");

    assert_eq!(
        invoke(&window, "get_settings", json!({})).unwrap(),
        json!({ "language": "pt-BR", "selectedWorkspaceId": null })
    );
    invoke(&window, "set_language", json!({ "language": "en-US" })).unwrap();
    invoke(
        &window,
        "select_workspace",
        json!({ "workspaceId": ws["id"] }),
    )
    .unwrap();
    let settings = invoke(&window, "get_settings", json!({})).unwrap();
    assert_eq!(settings["language"], "en-US");
    assert_eq!(settings["selectedWorkspaceId"], ws["id"]);
    assert_eq!(
        invoke(&window, "set_language", json!({ "language": "fr-FR" })).unwrap_err()["code"],
        "language_unsupported"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn send_message_returns_at_once_the_answer_arrives_as_events_and_usage_is_unavailable_not_zero() {
    let (app, window) = window();
    let ws = create_workspace(&window, "Atlas", "/atlas");
    let agent = create_agent(&window);
    let (tx, rx) = std::sync::mpsc::channel();
    let progress = tx.clone();
    app.listen(super::events::EXECUTION_PROGRESS_EVENT, move |event| {
        progress
            .send((
                "progress",
                serde_json::from_str::<Value>(event.payload()).unwrap(),
            ))
            .unwrap();
    });
    app.listen(super::events::CONVERSATION_MESSAGE_EVENT, move |event| {
        tx.send((
            "message",
            serde_json::from_str::<Value>(event.payload()).unwrap(),
        ))
        .unwrap();
    });

    let sent = invoke(
        &window,
        "send_message",
        json!({ "request": { "workspaceId": ws["id"], "agentId": agent["id"], "content": "Analyze auth" } }),
    )
    .unwrap();

    assert_eq!(sent["userMessage"]["role"], "user");
    assert_eq!(sent["userMessage"]["workspaceId"], ws["id"]);
    let execution_id = sent["executionId"].clone();
    let mut kinds = Vec::new();
    let answer = loop {
        let (channel, payload) = rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        if channel == "message" {
            break payload;
        }
        assert_eq!(payload["agentId"], agent["id"]);
        assert_eq!(payload["workspaceId"], ws["id"]);
        assert_eq!(payload["executionId"], execution_id);
        kinds.push(payload["kind"].as_str().unwrap().to_owned());
    };
    assert_eq!(
        kinds,
        [
            "started",
            "starting_runtime",
            "sending_prompt",
            "waiting_for_model",
            "completed"
        ]
    );
    assert_eq!(answer["content"], "fake answer");
    assert_eq!(answer["executionId"], execution_id);

    let messages = invoke(
        &window,
        "list_messages",
        json!({ "workspaceId": ws["id"], "agentId": agent["id"] }),
    )
    .unwrap();
    assert_eq!(messages.as_array().unwrap().len(), 2);
    let other = invoke(
        &window,
        "list_messages",
        json!({ "workspaceId": "elsewhere" }),
    )
    .unwrap();
    assert_eq!(other, json!([]));

    // The ended execution is kept with its timeline, and only for its own workspace.
    let ended = invoke(
        &window,
        "list_executions",
        json!({ "workspaceId": ws["id"], "agentId": agent["id"] }),
    )
    .unwrap();
    assert_eq!(ended.as_array().unwrap().len(), 1);
    assert_eq!(ended[0]["id"], execution_id);
    assert_eq!(ended[0]["status"], "completed");
    assert_eq!(
        ended[0]["events"].as_array().unwrap().last().unwrap()["kind"],
        "completed"
    );
    assert_eq!(
        invoke(
            &window,
            "list_executions",
            json!({ "workspaceId": "elsewhere" })
        )
        .unwrap(),
        json!([])
    );

    // The fake runtime reported no usage: one observed run, but no numbers (not zeros).
    let usage = invoke(
        &window,
        "get_agent_usage",
        json!({ "workspaceId": ws["id"], "agentId": agent["id"], "windows": WINDOWS() }),
    )
    .unwrap();
    assert_eq!(usage["today"]["runs"], 1);
    assert_eq!(usage["today"]["totalTokens"], Value::Null);
    assert_eq!(usage["today"]["cost"], Value::Null);
    assert_eq!(usage["conversation"]["runs"], 1);
    assert_eq!(usage["latestExecution"]["metrics"], Value::Null);
    assert_eq!(usage["quota"], Value::Null);
    let workspace_usage = invoke(
        &window,
        "get_workspace_usage",
        json!({ "workspaceId": ws["id"], "windows": WINDOWS() }),
    )
    .unwrap();
    assert_eq!(workspace_usage["week"]["runs"], 1);
    assert_eq!(
        invoke(
            &window,
            "get_workspace_usage",
            json!({ "workspaceId": "elsewhere", "windows": WINDOWS() })
        )
        .unwrap()["week"]["runs"],
        0
    );
}

#[test]
fn send_message_to_an_unknown_workspace_is_a_coded_error() {
    let (_app, window) = window();
    let agent = create_agent(&window);

    let error = invoke(
        &window,
        "send_message",
        json!({ "request": { "workspaceId": "ghost", "agentId": agent["id"], "content": "x" } }),
    )
    .unwrap_err();

    assert_eq!(error["code"], "workspace_not_found");
}

#[test]
fn security_is_visible_per_workspace_and_agent_and_approvals_are_explicit() {
    let (_app, window) = window();
    let ws = create_workspace(&window, "Atlas", "/atlas");
    let agent = create_agent(&window);

    // The workspace's policy: project folder only, developer-level ceiling, network off.
    let security = invoke(
        &window,
        "get_workspace_security",
        json!({ "workspaceId": ws["id"] }),
    )
    .unwrap();
    assert_eq!(security["projectPath"], "/atlas");
    assert_eq!(security["label"], "developer");
    assert_eq!(security["policy"]["filesystem"]["scope"], "project_only");
    assert_eq!(security["policy"]["network"]["mode"], "denied");
    assert_eq!(
        security["policy"]["git"]["destructive"],
        "approval_required"
    );
    assert_eq!(
        invoke(
            &window,
            "get_workspace_security",
            json!({ "workspaceId": "ghost" })
        )
        .unwrap_err()["code"],
        "workspace_not_found"
    );

    // A new agent may edit and run development commands (the workspace's policy still bounds
    // it); it can be switched to read-only.
    let ask = || {
        invoke(
            &window,
            "get_agent_permissions",
            json!({ "workspaceId": ws["id"], "agentId": agent["id"] }),
        )
        .unwrap()
    };
    let before = ask();
    assert_eq!(before["profile"], "developer");
    assert_eq!(
        before["availableProfiles"],
        json!(["read_only", "developer"])
    );
    assert_eq!(before["policy"]["processes"]["mode"], "allowed");
    assert_eq!(before["policy"]["filesystem"]["write"], "allowed");
    // The fake runtime's own tools can neither write nor run commands, which narrows the
    // profile: the runtime cannot be given more than it can do.
    assert_eq!(before["effective"]["processes"]["mode"], "denied");
    assert_eq!(before["effective"]["filesystem"]["write"], "denied");
    assert_eq!(before["unenforced"], json!([]));

    let updated = invoke(
        &window,
        "set_agent_permission_profile",
        json!({ "agentId": agent["id"], "profileId": "read_only" }),
    )
    .unwrap();
    assert_eq!(updated["permissionProfileId"], "read_only");
    let after = ask();
    assert_eq!(after["policy"]["processes"]["mode"], "denied");
    assert_eq!(after["policy"]["filesystem"]["write"], "denied");

    assert_eq!(
        invoke(
            &window,
            "set_agent_permission_profile",
            json!({ "agentId": agent["id"], "profileId": "root" })
        )
        .unwrap_err()["code"],
        "permission_profile_invalid"
    );

    // Nothing waits, and an answer to nothing is a coded error, not a silent success.
    assert_eq!(
        invoke(&window, "list_pending_approvals", json!({})).unwrap(),
        json!([])
    );
    assert_eq!(
        invoke(
            &window,
            "resolve_approval",
            json!({ "approvalId": "approval-1", "approve": true })
        )
        .unwrap_err()["code"],
        "approval_not_found"
    );
}

fn assert_sink<T: crate::application::security::PermissionSink>() {}

#[test]
fn permission_decisions_reach_the_webview_as_execution_progress() {
    use crate::domain::security::{
        DecisionSource, PermissionAction, PermissionEvent, PermissionOutcome, Reason,
    };

    let event = PermissionEvent {
        timestamp: 7,
        execution_id: "exec-1".to_owned(),
        workspace_id: "ws-1".to_owned(),
        task_id: "task-1".to_owned(),
        agent_id: "agent-1".to_owned(),
        action: PermissionAction::RunProcess,
        target: "npm test".to_owned(),
        cwd: Some("/atlas".to_owned()),
        decision: PermissionOutcome::ApprovalRequested,
        source: DecisionSource::Policy,
        reason: Some(Reason::NotInAllowedList),
        approval_id: Some("approval-3".to_owned()),
        notes: vec![],
    };

    let progress = super::events::permission_progress(&event);

    let json = serde_json::to_value(&progress).unwrap();
    assert_eq!(json["kind"], "permission");
    assert_eq!(
        (
            &json["executionId"],
            &json["workspaceId"],
            &json["agentId"],
            &json["taskId"]
        ),
        (
            &json!("exec-1"),
            &json!("ws-1"),
            &json!("agent-1"),
            &json!("task-1")
        )
    );
    assert_eq!(json["metadata"]["decision"], "approval_requested");
    assert_eq!(json["metadata"]["action"], "run_process");
    assert_eq!(json["metadata"]["source"], "policy");
    assert_eq!(json["metadata"]["reason"], "not_in_allowed_list");
    assert_eq!(json["metadata"]["target"], "npm test");
    assert_eq!(json["metadata"]["cwd"], "/atlas");
    assert_eq!(json["metadata"]["approvalId"], "approval-3");
    // The adapter the app registers is a sink that only forwards this.
    assert_sink::<super::events::TauriPermissionSink<MockRuntime>>();
}

fn assert_session_sink<T: crate::application::sessions::SessionSink>() {}

#[test]
fn terminal_commands_are_allowed_by_capability_and_answer_for_unknown_executions() {
    let (_app, window) = window();
    let ids = json!({ "workspaceId": "ws", "agentId": "a", "executionId": "exec-9" });

    // No terminal for these ids: an absent value, not an error.
    assert_eq!(
        invoke(&window, "get_execution_terminal", ids.clone()).unwrap(),
        Value::Null
    );
    // Controlling a process that is not there is a coded error, the same for "never existed" and
    // "belongs to someone else".
    for command in ["execution_interrupt", "execution_terminate"] {
        assert_eq!(
            invoke(&window, command, ids.clone()).unwrap_err()["code"],
            "execution_not_found",
            "{command}"
        );
    }
    let with = |extra: Value| {
        let mut body = ids.clone();
        body.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        body
    };
    assert_eq!(
        invoke(
            &window,
            "execution_terminal_input",
            with(json!({ "data": "x" }))
        )
        .unwrap_err()["code"],
        "execution_not_found"
    );
    assert_eq!(
        invoke(
            &window,
            "execution_terminal_resize",
            with(json!({ "cols": 80, "rows": 24 }))
        )
        .unwrap_err()["code"],
        "execution_not_found"
    );
    assert_session_sink::<super::events::TauriSessionSink<MockRuntime>>();
}

#[test]
fn the_webview_can_only_name_a_session_never_address_a_process() {
    // Nothing in the commands' parameters is an OS notion: no pid, signal, program or command.
    let (_app, window) = window();
    let ids =
        json!({ "workspaceId": "ws", "agentId": "a", "executionId": "e", "pid": 1, "signal": 9 });

    // Extra fields are not part of the contract and change nothing.
    assert_eq!(
        invoke(&window, "execution_terminate", ids).unwrap_err()["code"],
        "execution_not_found"
    );
}

#[test]
fn agents_isolate_executions_in_worktrees_by_default_and_can_say_otherwise() {
    let (_app, window) = window();
    let body = |extra: Value| {
        let mut request = json!({
            "name": "A", "personalityId": "architect", "runtimeId": "fake",
            "modelId": "m1", "instructions": "",
        });
        request
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        json!({ "request": request })
    };

    let default = invoke(&window, "create_agent", body(json!({}))).unwrap();
    let off = invoke(
        &window,
        "create_agent",
        body(json!({ "worktreeIsolation": false })),
    )
    .unwrap();

    assert_eq!(default["worktreeIsolation"], true);
    assert_eq!(off["worktreeIsolation"], false);
}

#[test]
fn an_isolated_agent_in_a_project_without_git_fails_instead_of_using_the_checkout() {
    let (_app, window) = window();
    let ws = create_workspace(&window, "Atlas", "/atlas");
    let agent = invoke(
        &window,
        "create_agent",
        json!({ "request": {
            "name": "A", "personalityId": "architect", "runtimeId": "fake",
            "modelId": "m1", "instructions": "",
        }}),
    )
    .unwrap();
    invoke(
        &window,
        "add_agent_to_workspace",
        json!({ "workspaceId": ws["id"], "agentId": agent["id"] }),
    )
    .unwrap();

    invoke(
        &window,
        "send_message",
        json!({ "request": { "workspaceId": ws["id"], "agentId": agent["id"], "content": "go" } }),
    )
    .unwrap();
    let messages = (0..100)
        .find_map(|_| {
            std::thread::sleep(std::time::Duration::from_millis(50));
            let messages = invoke(&window, "list_messages", json!({})).unwrap();
            (messages.as_array().unwrap().len() == 2).then_some(messages)
        })
        .expect("the failure is answered");

    assert_eq!(messages[1]["failed"], true);
    assert_eq!(messages[1]["failureKind"], "git_repository_required");
}

#[test]
fn worktrees_can_be_listed_and_merging_an_unknown_one_is_refused() {
    let (_app, window) = window();

    assert_eq!(
        invoke(&window, "list_execution_worktrees", json!({})).unwrap(),
        json!([])
    );
    assert_eq!(
        invoke(
            &window,
            "merge_execution",
            json!({ "workspaceId": "ws", "agentId": "a", "executionId": "exec-1" })
        )
        .unwrap_err()["code"],
        "worktree_not_found"
    );
}

#[test]
fn the_webview_has_no_command_that_runs_programs_or_addresses_processes() {
    // The surface is the allow-list in build.rs plus the capability file: each of these would
    // be a way around the guard, and none exists.
    let (_app, window) = window();

    for command in [
        "kill_process",
        "run_shell",
        "execute_command",
        "run_command",
        "spawn_process",
        "run_git",
        "remove_worktree",
        "delete_branch",
    ] {
        assert!(
            invoke(&window, command, json!({ "pid": 1, "command": "id" })).is_err(),
            "{command} must not exist"
        );
    }
}

// ---- workflows ---------------------------------------------------------------------------------

/// A workspace with one agent per role a template uses, on the fake runtime, without worktrees
/// (the fake project folder is not a Git repository).
fn workflow_workspace(window: &WebviewWindow<MockRuntime>, isolation: bool) -> Value {
    let ws = create_workspace(window, "Atlas", "/atlas");
    for personality in [
        "architect",
        "developer",
        "qa",
        "architecture-validator",
        "bug-fixer",
    ] {
        let agent = invoke(
            window,
            "create_agent",
            json!({ "request": {
                "name": personality, "personalityId": personality, "runtimeId": "fake",
                "modelId": "m1", "instructions": "", "worktreeIsolation": isolation,
            }}),
        )
        .unwrap();
        // Not placed on the grid (it holds four): templates fall back to the catalog.
        drop(agent);
    }
    ws
}

fn wait_for_execution(window: &WebviewWindow<MockRuntime>, id: &Value, status: &str) -> Value {
    (0..200)
        .find_map(|_| {
            std::thread::sleep(std::time::Duration::from_millis(25));
            let run = invoke(
                window,
                "get_workflow_execution",
                json!({ "executionId": id }),
            )
            .unwrap();
            (run["status"] == status).then_some(run)
        })
        .unwrap_or_else(|| panic!("the run never became {status}"))
}

#[test]
#[allow(clippy::too_many_lines)]
fn a_workflow_runs_through_the_execution_layer_and_is_followed_with_events() {
    let (app, window) = window();
    let ws = workflow_workspace(&window, false);
    let (tx, rx) = std::sync::mpsc::channel();
    for name in [
        "workflow:started",
        "workflow:node_started",
        "workflow:node_completed",
        "workflow:artifact_created",
        "workflow:completed",
    ] {
        let tx = tx.clone();
        app.listen(name, move |event| {
            tx.send((
                name,
                serde_json::from_str::<Value>(event.payload()).unwrap(),
            ))
            .unwrap();
        });
    }

    // Automatic: the task picks a template, which builds a workflow from the workspace's agents.
    let template = invoke(
        &window,
        "select_workflow_template",
        json!({ "task": "Implementar recuperação de senha" }),
    )
    .unwrap();
    assert_eq!(template, "software_feature");
    let templates = invoke(&window, "list_workflow_templates", json!({})).unwrap();
    assert_eq!(templates.as_array().unwrap().len(), 8);
    let built = invoke(
        &window,
        "create_workflow_from_template",
        json!({ "workspaceId": ws["id"], "templateId": "feature_basic", "mode": "automatic" }),
    )
    .unwrap();
    assert_eq!(built["missingRoles"], json!([]));
    let workflow = built["workflow"].clone();
    assert_eq!(workflow["mode"], "automatic");
    assert_eq!(workflow["version"], 1);
    assert_eq!(workflow["status"], "ready");
    let report = invoke(
        &window,
        "validate_workflow",
        json!({ "workflow": workflow }),
    )
    .unwrap();
    assert_eq!(report, json!({ "valid": true, "issues": [] }));

    let run = invoke(
        &window,
        "start_workflow",
        json!({ "workflowId": workflow["id"], "task": "Implementar recuperação de senha" }),
    )
    .unwrap();
    assert_eq!(run["status"], "running");
    assert_eq!(run["workflowVersion"], 1);
    let done = wait_for_execution(&window, &run["id"], "completed");

    // Architect, Developer, QA each ran as an ordinary execution.
    let nodes = done["nodes"].as_object().unwrap();
    for id in ["architect", "developer", "qa", "done"] {
        assert_eq!(nodes[id]["status"], "completed", "{id}");
    }
    let executions = invoke(
        &window,
        "list_executions",
        json!({ "workspaceId": ws["id"] }),
    )
    .unwrap();
    let executions = executions.as_array().unwrap();
    assert_eq!(executions.len(), 3);
    for execution in executions {
        assert_eq!(execution["workflow"]["workflowExecutionId"], run["id"]);
        assert_eq!(execution["status"], "completed");
        // The harness text comes from the execution layer: the context is recorded as usual.
        assert!(execution["workflow"]["nodeLabel"].is_string());
    }
    // The step's request and its answer are in the agent's conversation, like any execution.
    let messages = invoke(&window, "list_messages", json!({ "workspaceId": ws["id"] })).unwrap();
    assert_eq!(messages.as_array().unwrap().len(), 6);
    // Without a structured result the agent's text is kept as a summary artifact.
    assert_eq!(done["state"]["artifacts"].as_array().unwrap().len(), 3);

    let mut names = Vec::new();
    while let Ok((name, payload)) = rx.try_recv() {
        assert_eq!(payload["executionId"], run["id"]);
        assert_eq!(payload["workspaceId"], ws["id"]);
        names.push(name);
    }
    assert_eq!(names.first(), Some(&"workflow:started"));
    assert_eq!(names.last(), Some(&"workflow:completed"));
    assert_eq!(
        names
            .iter()
            .filter(|n| **n == "workflow:node_started")
            .count(),
        3
    );

    // Listing and reading what was saved.
    let listed = invoke(
        &window,
        "list_workflows",
        json!({ "workspaceId": ws["id"] }),
    )
    .unwrap();
    assert_eq!(listed.as_array().unwrap().len(), 1);
    let runs = invoke(
        &window,
        "list_workflow_executions",
        json!({ "workspaceId": ws["id"], "workflowId": workflow["id"] }),
    )
    .unwrap();
    assert_eq!(runs[0]["id"], run["id"]);
}

#[test]
fn a_workflow_step_of_an_isolated_agent_in_a_project_without_git_fails_and_never_runs_in_the_checkout(
) {
    let (_app, window) = window();
    let ws = workflow_workspace(&window, true);
    let built = invoke(
        &window,
        "create_workflow_from_template",
        json!({ "workspaceId": ws["id"], "templateId": "feature_basic", "mode": "custom" }),
    )
    .unwrap();

    let run = invoke(
        &window,
        "start_workflow",
        json!({ "workflowId": built["workflow"]["id"], "task": "go" }),
    )
    .unwrap();
    let failed = wait_for_execution(&window, &run["id"], "failed");

    assert_eq!(failed["failure"]["code"], "node_failed");
    assert_eq!(failed["nodes"]["architect"]["status"], "failed");
    assert_eq!(failed["nodes"]["developer"]["status"], "blocked");
    let executions = invoke(&window, "list_executions", json!({})).unwrap();
    assert_eq!(executions.as_array().unwrap().len(), 1);
    assert_eq!(executions[0]["failure"]["kind"], "git_repository_required");
    assert_eq!(executions[0]["workflow"]["nodeId"], "architect");
}

#[test]
fn workflow_commands_report_coded_errors_and_the_run_cannot_be_started_when_invalid() {
    let (_app, window) = window();
    let ws = create_workspace(&window, "Atlas", "/atlas");

    let unknown = invoke(
        &window,
        "start_workflow",
        json!({ "workflowId": "nope", "task": "x" }),
    )
    .unwrap_err();
    assert_eq!(unknown["code"], "workflow_not_found");
    let template = invoke(
        &window,
        "create_workflow_from_template",
        json!({ "workspaceId": ws["id"], "templateId": "ghost", "mode": "custom" }),
    )
    .unwrap_err();
    assert_eq!(template["code"], "workflow_template_not_found");

    // No agents exist: the template is built with empty nodes and says which roles are missing.
    let built = invoke(
        &window,
        "create_workflow_from_template",
        json!({ "workspaceId": ws["id"], "templateId": "feature_basic", "mode": "custom" }),
    )
    .unwrap();
    assert_eq!(
        built["missingRoles"],
        json!(["architect", "developer", "qa"])
    );
    assert_eq!(built["workflow"]["status"], "draft");
    let invalid = invoke(
        &window,
        "start_workflow",
        json!({ "workflowId": built["workflow"]["id"], "task": "x" }),
    )
    .unwrap_err();
    assert_eq!(invalid["code"], "workflow_invalid");

    let report = invoke(
        &window,
        "validate_workflow",
        json!({ "workflow": built["workflow"] }),
    )
    .unwrap();
    assert_eq!(report["valid"], false);
    assert_eq!(report["issues"][0]["code"], "missing_agent");
    assert_eq!(report["issues"][0]["nodeId"], "architect");

    invoke(
        &window,
        "delete_workflow",
        json!({ "workflowId": built["workflow"]["id"] }),
    )
    .unwrap();
    assert_eq!(
        invoke(
            &window,
            "list_workflows",
            json!({ "workspaceId": ws["id"] })
        )
        .unwrap(),
        json!([])
    );
    let gone = invoke(&window, "cancel_workflow", json!({ "executionId": "nope" })).unwrap_err();
    assert_eq!(gone["code"], "workflow_execution_not_found");
}

#[test]
fn workflows_can_be_composed_by_hand_and_versioned() {
    let (_app, window) = window();
    let ws = create_workspace(&window, "Atlas", "/atlas");
    let agent = create_agent(&window);
    let node = |id: &str| json!({ "id": id, "type": "agent", "agentId": agent["id"], "label": id });
    let created = invoke(
        &window,
        "create_workflow",
        json!({ "request": {
            "workspaceId": ws["id"], "name": "By hand", "mode": "custom",
            "nodes": [node("a"), { "id": "e", "type": "end", "label": "Done", "outcome": "done" }],
            "edges": [{ "id": "a-e", "sourceNodeId": "a", "targetNodeId": "e" }],
        }}),
    )
    .unwrap();
    assert_eq!(created["version"], 1);
    assert_eq!(created["status"], "ready");

    let mut moved = created.clone();
    moved["nodes"][0]["position"] = json!({ "x": 5.0, "y": 6.0 });
    let moved = invoke(&window, "update_workflow", json!({ "workflow": moved })).unwrap();
    assert_eq!(moved["version"], 1, "moving a node is not a new version");

    let mut changed = moved.clone();
    changed["nodes"][0]["retryPolicy"] = json!({ "maxRetries": 2 });
    let changed = invoke(&window, "update_workflow", json!({ "workflow": changed })).unwrap();
    assert_eq!(changed["version"], 2);
    assert_eq!(changed["nodes"][0]["retryPolicy"]["maxRetries"], 2);
}

#[test]
fn the_webview_cannot_reach_a_workflow_command_that_runs_or_edits_anything_outside_the_guard() {
    // Workflows add use cases, not authority: no command takes a program, a path or an approval.
    let (_app, window) = window();
    for command in [
        "run_workflow_node",
        "approve_workflow_node",
        "set_workflow_permissions",
        "run_workflow_shell",
        "write_workflow_file",
    ] {
        assert!(
            invoke(&window, command, json!({})).is_err(),
            "{command} must not exist"
        );
    }
}

#[test]
fn preview_task_context_is_reachable_from_the_webview() {
    let (_app, window) = window();
    let ws = create_workspace(&window, "Atlas", "/atlas");
    let result = invoke(
        &window,
        "preview_task_context",
        json!({ "workspaceId": ws["id"], "request": { "task": "x" } }),
    );
    // Whatever it answers, it must not be refused by the capability.
    if let Err(error) = result {
        assert!(!error.to_string().contains("not allowed"), "{error}");
    }
}

#[test]
fn the_commands_about_a_runs_code_are_reachable_and_take_only_a_run_never_a_path_or_a_program() {
    let (_app, window) = window();

    // Editors come from a fixed list; the run is the only thing the webview names.
    let ides = invoke(&window, "list_ides", json!({})).unwrap();
    assert_eq!(ides[0]["id"], "vscode");
    for (command, body) in [
        ("get_workflow_changes", json!({ "executionId": "nope" })),
        (
            "get_workflow_diff",
            json!({ "executionId": "nope", "file": "a.txt" }),
        ),
        ("apply_workflow_changes", json!({ "executionId": "nope" })),
        ("keep_workflow_changes", json!({ "executionId": "nope" })),
        ("discard_workflow_changes", json!({ "executionId": "nope" })),
        (
            "open_workflow_in_ide",
            json!({ "executionId": "nope", "ideId": "vscode" }),
        ),
    ] {
        let error = invoke(&window, command, body).unwrap_err();
        assert_eq!(error["code"], "workflow_execution_not_found", "{command}");
    }
    for forbidden in [
        "merge_workflow",
        "open_path_in_ide",
        "run_in_worktree",
        "force_apply_workflow_changes",
    ] {
        assert!(
            invoke(&window, forbidden, json!({ "path": "/" })).is_err(),
            "{forbidden}"
        );
    }
}

// ---- the live workspace ------------------------------------------------------------------------

#[test]
fn the_live_workspace_of_a_run_that_has_no_worktree_is_nothing_and_a_refresh_says_so() {
    let (_app, window) = window();

    let state = invoke(
        &window,
        "get_live_workspace",
        json!({ "executionId": "wfx-nope" }),
    )
    .unwrap();
    let refreshed = invoke(
        &window,
        "refresh_live_workspace",
        json!({ "executionId": "wfx-nope" }),
    );

    assert_eq!(state, Value::Null);
    assert!(refreshed.is_err());
}

#[test]
fn what_changes_in_a_worktree_reaches_the_webview_as_an_event_without_anyone_asking() {
    use crate::application::live_workspace::tests::service_over;
    use crate::application::worktree::tests::Env;
    use crate::domain::security::Permission;

    let (app, _window) = window();
    let (tx, rx) = std::sync::mpsc::channel();
    app.listen(super::events::LIVE_WORKSPACE_EVENT, move |event| {
        tx.send(serde_json::from_str::<Value>(event.payload()).unwrap())
            .unwrap();
    });
    let env = Env::new(Permission::Allowed);
    let prepared = env.prepare("exec-1");
    env.service.mark_workflow("exec-1", "wfx-1").unwrap();
    let service = service_over(
        &env,
        std::sync::Arc::new(super::events::TauriLiveSink {
            app: app.handle().clone(),
        }),
    );
    service.start("exec-1").unwrap();
    // The first look (nothing yet) is announced.
    let first = rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
    assert_eq!(first["full"], true);
    assert_eq!(first["worktreeExecutionId"], "exec-1");

    std::fs::write(prepared.working_dir.join("created.ts"), "export {};\n").unwrap();

    let update = loop {
        let update = rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        if update["changed"].as_array().is_some_and(|c| !c.is_empty()) {
            break update;
        }
    };
    assert_eq!(update["changed"][0]["path"], "created.ts");
    assert_eq!(update["changed"][0]["status"], "added");
    assert_eq!(update["availability"], "available");
    assert_eq!(update["observation"], "events");
    assert_eq!(update["filesChanged"], 1);
    assert_eq!(update["runId"], "wfx-1");
}

#[test]
fn the_viewer_commands_take_a_run_and_a_relative_path_and_refuse_anything_else() {
    let (_app, window) = window();

    // A path that leaves the worktree is refused before any run is looked at.
    for bad in ["../outside", "/etc/passwd", ".git/config", "a/../b", ""] {
        let file = invoke(
            &window,
            "get_live_file",
            json!({ "executionId": "wfx-1", "path": bad }),
        )
        .unwrap_err();
        let diff = invoke(
            &window,
            "get_live_diff",
            json!({ "executionId": "wfx-1", "path": bad }),
        )
        .unwrap_err();
        assert_eq!(file["code"], "worktree_invalid_state", "{bad:?}");
        assert_eq!(diff["code"], "worktree_invalid_state", "{bad:?}");
    }
    // A plain path of a run with no worktree: nothing to read.
    let unknown = invoke(
        &window,
        "get_live_file",
        json!({ "executionId": "wfx-nope", "path": "src/a.ts" }),
    )
    .unwrap_err();
    assert_eq!(unknown["code"], "worktree_not_found");
}
