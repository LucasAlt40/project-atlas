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
            super::usage::get_agent_usage,
            super::usage::get_workspace_usage,
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
    assert_eq!(restored.as_array().unwrap().len(), 4);
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
