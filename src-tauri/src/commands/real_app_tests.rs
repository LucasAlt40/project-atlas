//! The real app, end to end through the IPC: the composition root of `lib.rs` (`build_state`) over
//! a temporary data folder, real files, real Git, the real guard, a real PTY and a real CLI. Only
//! the webview is missing: every call goes through Tauri's real invoke handler with the same JSON
//! the UI sends, and the events the core emits are collected as the UI would receive them.
//!
//! The ones that call a model use `OpenCode`'s free models and are opt-in: run them by exact name.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::test::MockRuntime;
use tauri::{Listener, Manager, WebviewWindow};

use super::events::{TauriHarnessProgress, TauriLiveSink, TauriPermissionSink, TauriSessionSink};
use super::specs::TauriSpecEvents;
use super::tests::{app_window, invoke};
use crate::application::security::testutil::TempDir;

const MODEL: &str = "opencode/big-pickle";

/// Events as the webview receives them: `(name, payload)`.
type Heard = Arc<Mutex<Vec<(String, Value)>>>;

const EVENTS: [&str; 7] = [
    "conversation:message",
    "execution:progress",
    "execution:status",
    "workflow:started",
    "workflow:node_started",
    "workflow:completed",
    "workflow:failed",
];

/// The app as `lib.rs` builds it, its data in `data`.
fn real_app(data: &Path) -> (tauri::App<MockRuntime>, WebviewWindow<MockRuntime>, Heard) {
    let (app, window) = app_window(None);
    let handle = app.handle().clone();
    app.manage(crate::build_state(
        data,
        Arc::new(TauriPermissionSink {
            app: handle.clone(),
        }),
        Arc::new(TauriSessionSink {
            app: handle.clone(),
        }),
        Some(Arc::new(TauriHarnessProgress {
            app: handle.clone(),
        })),
        Arc::new(TauriLiveSink {
            app: handle.clone(),
        }),
        Arc::new(TauriSpecEvents { app: handle }),
    ));
    let heard = Heard::default();
    for name in EVENTS {
        let sink = heard.clone();
        app.listen_any(name, move |event| {
            let payload = serde_json::from_str(event.payload()).unwrap_or(Value::Null);
            sink.lock().unwrap().push((name.to_owned(), payload));
        });
    }
    (app, window, heard)
}

fn call(window: &WebviewWindow<MockRuntime>, cmd: &str, body: Value) -> Value {
    invoke(window, cmd, body).unwrap_or_else(|error| panic!("{cmd}: {error}"))
}

fn git(dir: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .args(["-c", "user.name=Test", "-c", "user.email=t@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {out:?}");
}

/// Waits (bounded) until `done` returns a value, approving on the way any command the guard holds
/// for the person, as the person would in the app. Returns the value and the approvals given.
fn wait_for<T>(
    window: &WebviewWindow<MockRuntime>,
    limit: Duration,
    what: &str,
    mut done: impl FnMut() -> Option<T>,
) -> (T, usize) {
    let began = Instant::now();
    let mut approved = 0;
    loop {
        if let Some(value) = done() {
            return (value, approved);
        }
        let pending = call(window, "list_pending_approvals", json!({}));
        for request in pending.as_array().into_iter().flatten() {
            println!("approving: {request}");
            call(
                window,
                "resolve_approval",
                json!({ "requestId": request["id"], "decision": "allow_once" }),
            );
            approved += 1;
        }
        assert!(
            began.elapsed() < limit,
            "{what}: still not done after {limit:?}"
        );
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// An agent as the agent form creates it: the personality's suggested contract and profile.
fn create_agent(
    window: &WebviewWindow<MockRuntime>,
    workspace: &str,
    personality: &str,
    isolate: bool,
) -> String {
    let personalities = call(window, "list_personalities", json!({}));
    let profile = personalities
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == personality)
        .unwrap_or_else(|| panic!("no personality {personality}"));
    let agent = call(
        window,
        "create_agent",
        json!({ "request": {
            "name": format!("{personality} (test)"),
            "personalityId": personality,
            "runtimeId": "opencode",
            "modelId": MODEL,
            "instructions": "",
            "worktreeIsolation": isolate,
            "permissionProfileId": profile["suggestedPermissionProfile"],
            "resultContract": profile["suggestedContract"],
        }}),
    );
    let id = agent["id"].as_str().unwrap().to_owned();
    call(
        window,
        "add_agent_to_workspace",
        json!({ "workspaceId": workspace, "agentId": id }),
    );
    id
}

fn names(heard: &Heard, event: &str) -> Vec<Value> {
    heard
        .lock()
        .unwrap()
        .iter()
        .filter(|(name, _)| name == event)
        .map(|(_, payload)| payload.clone())
        .collect()
}

/// A message to a real agent: the answer comes back through the events and the stores, and all of
/// it (workspace, agent, conversation, execution) is still there after the app restarts.
#[test]
#[ignore = "needs the OpenCode CLI; makes a real (free) model call"]
#[allow(clippy::too_many_lines)] // one story: a message, its answer, then a restart
fn real_app_chat_with_a_real_agent_survives_a_restart() {
    let root = TempDir::new("real-app-chat");
    let data = root.path().join("data");
    let project = root.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(project.join("README.md"), "# Demo\n").unwrap();
    let nonce = format!(
        "ATLAS-{}",
        crate::application::support::now_ms() % 1_000_000
    );

    let (workspace, agent, execution) = {
        let (_app, window, heard) = real_app(&data);
        let runtimes = call(&window, "list_runtimes", json!({}));
        for status in runtimes.as_array().unwrap() {
            println!(
                "runtime {}: {} / auth {}",
                status["runtime"]["id"], status["availability"], status["authentication"]["state"]
            );
        }
        let opencode = runtimes
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["runtime"]["id"] == "opencode")
            .unwrap();
        assert_eq!(opencode["availability"], "ready");
        assert!(opencode["availableModels"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["id"] == MODEL));

        let workspace = call(
            &window,
            "create_workspace",
            json!({ "input": { "name": "Demo", "projectPath": project, "description": null } }),
        )["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let agent = create_agent(&window, &workspace, "developer", false);
        let sent = call(
            &window,
            "send_message",
            json!({ "request": {
                "workspaceId": workspace, "agentId": agent,
                "content": format!("Reply with exactly this text and nothing else: {nonce}. Do not use any tools."),
            }}),
        );
        let execution = sent["executionId"].as_str().unwrap().to_owned();
        assert_eq!(sent["userMessage"]["role"], "user");

        let (stored, approvals) = wait_for(&window, Duration::from_secs(240), "the answer", || {
            call(
                &window,
                "list_executions",
                json!({ "workspaceId": workspace, "agentId": agent }),
            )
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["id"] == execution.as_str())
            .cloned()
        });
        println!("stored execution: {stored}");
        assert_eq!(stored["status"], "completed", "{stored}");
        assert_eq!(approvals, 0, "a text answer needs no approval");
        let messages = call(
            &window,
            "list_messages",
            json!({ "workspaceId": workspace, "agentId": agent }),
        );
        let answer = messages
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["role"] == "assistant")
            .expect("an assistant message");
        assert!(
            answer["content"].as_str().unwrap().contains(&nonce),
            "{answer}"
        );
        assert_eq!(answer["executionId"], execution.as_str());
        // What the webview heard: the message, and the progress of the same execution only.
        assert!(names(&heard, "conversation:message")
            .iter()
            .any(|m| m["content"].as_str().is_some_and(|c| c.contains(&nonce))));
        let progress = names(&heard, "execution:progress");
        assert!(
            !progress.is_empty(),
            "no progress event reached the webview"
        );
        assert!(progress
            .iter()
            .all(|p| p["executionId"] == execution.as_str()));
        (workspace, agent, execution)
    };

    // A restart: a new app over the same data folder.
    assert!(data.join("config.json").is_file());
    let (_app, window, _) = real_app(&data);
    let workspaces = call(&window, "list_workspaces", json!({}));
    assert!(workspaces
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w["id"] == workspace.as_str()));
    assert!(call(&window, "list_agents", json!({}))
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["id"] == agent.as_str() && a["modelId"] == MODEL));
    let messages = call(
        &window,
        "list_messages",
        json!({ "workspaceId": workspace, "agentId": agent }),
    );
    assert!(messages
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["content"].as_str().is_some_and(|c| c.contains(&nonce))));
    assert!(call(
        &window,
        "list_executions",
        json!({ "workspaceId": workspace, "agentId": agent }),
    )
    .as_array()
    .unwrap()
    .iter()
    .any(|e| e["id"] == execution.as_str() && e["status"] == "completed"));
}

/// The "Quick change" template, created and started through the commands, run by real agents in an
/// isolated worktree, and applied to the project only when the person applies it.
#[test]
#[ignore = "needs the OpenCode CLI; makes real (free) model calls"]
#[allow(clippy::too_many_lines)] // one story, from the template to the applied file
fn real_app_template_workflow_is_applied_to_the_project_only_on_request() {
    let root = TempDir::new("real-app-workflow");
    let data = root.path().join("data");
    let project = root.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    git(&project, &["init", "--quiet", "-b", "main"]);
    std::fs::write(
        project.join("README.md"),
        "# Calc\nA tiny CommonJS project.\n",
    )
    .unwrap();
    git(&project, &["add", "--all"]);
    git(&project, &["commit", "--quiet", "-m", "initial"]);

    let (_app, window, heard) = real_app(&data);
    let workspace = call(
        &window,
        "create_workspace",
        json!({ "input": { "name": "Calc", "projectPath": project, "description": null } }),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    for personality in ["architect", "developer", "qa"] {
        create_agent(&window, &workspace, personality, true);
    }
    let built = call(
        &window,
        "create_workflow_from_template",
        json!({ "workspaceId": workspace, "templateId": "feature_basic", "name": null, "mode": "custom" }),
    );
    assert_eq!(built["missingRoles"], json!([]), "{built}");
    let workflow = built["workflow"]["id"].as_str().unwrap().to_owned();
    let run = call(
        &window,
        "start_workflow",
        json!({
            "workflowId": workflow,
            "task": "Create the file src/sum.js, a CommonJS module that exports a function sum(a, b) \
                     returning a + b, exactly as: module.exports = { sum }. Acceptance: sum(2, 3) === 5. \
                     Touch no other file.",
            "autoApprovePlans": true,
        }),
    );
    let run_id = run["id"].as_str().unwrap().to_owned();

    let (done, approvals) = wait_for(&window, Duration::from_secs(900), "the run", || {
        let run = call(
            &window,
            "get_workflow_execution",
            json!({ "executionId": run_id }),
        );
        matches!(
            run["status"].as_str(),
            Some("completed" | "failed" | "cancelled" | "interrupted")
        )
        .then_some(run)
    });
    println!("status: {} failure: {}", done["status"], done["failure"]);
    println!("approvals given: {approvals}");
    println!("integration: {}", done["integration"]);
    if done["status"] != "completed" {
        // What each agent answered, to tell a model's slip from Atlas's.
        let messages = call(
            &window,
            "list_messages",
            json!({ "workspaceId": workspace }),
        );
        for message in messages.as_array().into_iter().flatten() {
            if message["role"] == "assistant" {
                let text = message["content"].as_str().unwrap_or_default();
                println!(
                    "--- answer ({} chars), last 1500:\n{}",
                    text.len(),
                    &text[text.len().saturating_sub(1500)..]
                );
            }
        }
    }
    assert_eq!(done["status"], "completed", "{}", done["failure"]);
    let started: Vec<String> = names(&heard, "workflow:node_started")
        .iter()
        .filter(|e| e["executionId"] == run_id.as_str())
        .filter_map(|e| e["nodeId"].as_str().map(str::to_owned))
        .collect();
    println!("steps started: {started:?}");
    assert_eq!(started, ["architect", "developer", "qa"]);
    assert_eq!(names(&heard, "workflow:completed").len(), 1);

    // Nothing reached the project before the person applies it.
    assert!(!project.join("src/sum.js").exists());
    let changes = call(
        &window,
        "get_workflow_changes",
        json!({ "executionId": run_id }),
    );
    println!("changes: {changes}");
    assert!(changes.to_string().contains("src/sum.js"));

    let applied = call(
        &window,
        "apply_workflow_changes",
        json!({ "executionId": run_id }),
    );
    println!("after apply: {}", applied["integration"]);
    let sum = std::fs::read_to_string(project.join("src/sum.js")).expect("applied to the project");
    let out = std::process::Command::new("node")
        .args([
            "-e",
            "const {sum}=require('./src/sum.js');console.log(sum(2,3)===5?'OK':'WRONG')",
        ])
        .current_dir(&project)
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "OK", "{sum}");
}

/// Initialize Project on a real folder, then the person writes in the business context, then a
/// refresh: what the person wrote stays. No model.
#[test]
fn real_app_harness_initializes_a_real_project_and_a_refresh_keeps_what_the_person_wrote() {
    let root = TempDir::new("real-app-harness");
    let data = root.path().join("data");
    let project = root.path().join("project");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join("package.json"),
        r#"{ "name": "shop", "scripts": { "test": "vitest run", "build": "tsc" },
             "dependencies": { "react": "^19.0.0" }, "devDependencies": { "typescript": "^5.0.0", "vitest": "^3.0.0" } }"#,
    )
    .unwrap();
    std::fs::write(project.join("tsconfig.json"), "{}").unwrap();
    std::fs::write(project.join("src/index.ts"), "export const x = 1;\n").unwrap();

    let (_app, window, _) = real_app(&data);
    let workspace = call(
        &window,
        "create_workspace",
        json!({ "input": { "name": "Shop", "projectPath": project, "description": null } }),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let analysis = call(
        &window,
        "analyze_project",
        json!({ "workspaceId": workspace }),
    );
    let text = analysis.to_string().to_lowercase();
    assert!(text.contains("typescript"), "{analysis}");
    assert!(text.contains("react"), "{analysis}");
    assert!(text.contains("vitest"), "{analysis}");
    assert!(
        !project.join(".atlas").exists(),
        "an analysis writes nothing"
    );

    let initialized = call(
        &window,
        "initialize_project",
        json!({ "workspaceId": workspace, "input": { "mode": "create", "purpose": "An online shop." } }),
    );
    assert_eq!(
        initialized["summary"]["status"], "initialized",
        "{initialized}"
    );
    let atlas = project.join(".atlas");
    let written: Vec<String> = walk(&atlas);
    println!("written: {written:?}");
    assert!(atlas.join("project.yaml").is_file());
    // The purpose the person gave is kept as theirs.
    let all: String = written
        .iter()
        .map(|p| std::fs::read_to_string(atlas.join(p)).unwrap_or_default())
        .collect();
    assert!(all.contains("An online shop."));

    // The person writes in a file of their own, then the project changes and is refreshed.
    let mine = written
        .iter()
        .find(|p| p.contains("business") || p.contains("domain"))
        .cloned()
        .unwrap_or_else(|| panic!("no business context file in {written:?}"));
    let path = atlas.join(&mine);
    let edited = format!(
        "{}\n\nWritten by the person: orders over 500 need a manager.\n",
        std::fs::read_to_string(&path).unwrap()
    );
    std::fs::write(&path, &edited).unwrap();
    std::fs::write(project.join("src/extra.ts"), "export const y = 2;\n").unwrap();
    let preview = call(
        &window,
        "refresh_project_harness",
        json!({ "workspaceId": workspace }),
    );
    assert_eq!(preview["applied"], Value::Null);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        edited,
        "a preview writes nothing"
    );
    call(
        &window,
        "refresh_project_harness",
        json!({ "workspaceId": workspace, "confirm": true }),
    );
    assert!(std::fs::read_to_string(&path)
        .unwrap()
        .contains("orders over 500 need a manager"));
    let agent = create_agent(&window, &workspace, "developer", false);
    let context = call(
        &window,
        "preview_task_context",
        json!({ "request": { "workspaceId": workspace, "agentId": agent, "task": "add a discount rule" } }),
    );
    assert!(
        context
            .to_string()
            .contains("orders over 500 need a manager"),
        "the agents get what the person wrote: {context}"
    );
}

fn walk(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                found.push(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    found.sort();
    found
}

/// Spec-driven development end to end: a spec written by a real agent document by document, each
/// approved as the person would, answered when the agent asks, checked for consistency, reviewed
/// by another real agent, approved, published into the project, and then checked by the real
/// `OpenSpec` CLI. Findings the review raises are recorded and dismissed with a note (the test
/// cannot judge them), which the report shows.
#[test]
#[ignore = "needs the OpenCode and OpenSpec CLIs; makes real (free) model calls"]
#[allow(clippy::too_many_lines)] // the driver plays the person through every step
fn real_app_spec_is_written_reviewed_published_and_accepted_by_the_openspec_cli() {
    let root = TempDir::new("real-app-spec");
    let data = root.path().join("data");
    let project = root.path().join("project");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join("README.md"),
        "# Notes API\nA tiny HTTP API for notes.\n",
    )
    .unwrap();
    std::fs::write(
        project.join("src/notes.js"),
        "const notes = [];\nfunction addNote(text) { notes.push({ text }); return notes.length; }\n\
         module.exports = { addNote, notes };\n",
    )
    .unwrap();

    let (_app, window, _) = real_app(&data);
    let workspace = call(
        &window,
        "create_workspace",
        json!({ "input": { "name": "Notes", "projectPath": project, "description": null } }),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let author = create_agent(&window, &workspace, "spec-architect", false);
    let reviewer = create_agent(&window, &workspace, "architecture-validator", false);
    let created = call(
        &window,
        "create_spec",
        json!({ "request": {
            "workspaceId": workspace, "title": "Delete a note",
            "request": "Add a function deleteNote(index) in src/notes.js that removes the note at that \
                        position and returns true, or returns false when there is no such note. Keep it small.",
            "context": "", "authorAgentId": author, "reviewerAgentId": reviewer,
        }}),
    );
    let spec = created["spec"]["id"].as_str().unwrap().to_owned();
    call(&window, "start_spec", json!({ "specId": spec }));

    let began = Instant::now();
    let mut log: Vec<String> = Vec::new();
    let mut dismissed = Vec::new();
    let mut retries = 0;
    let mut steps = 0;
    let view = loop {
        assert!(
            began.elapsed() < Duration::from_secs(2400),
            "the spec did not finish: {log:#?}"
        );
        steps += 1;
        assert!(steps < 2000, "{log:#?}");
        let view = call(&window, "get_spec", json!({ "specId": spec }));
        if view["busy"] == true {
            std::thread::sleep(Duration::from_secs(2));
            continue;
        }
        let mut acted = false;
        for question in view["spec"]["questions"].as_array().into_iter().flatten() {
            if question["status"] == "pending" {
                log.push(format!("question: {}", question["question"]));
                let choice = question["options"][0]["id"].as_str().map(str::to_owned);
                call(
                    &window,
                    "answer_spec_question",
                    json!({ "specId": spec, "questionId": question["id"], "choice": choice,
                            "answer": "Keep it minimal; use your own judgement." }),
                );
                acted = true;
            }
        }
        if acted {
            continue;
        }
        let phases = view["phases"].as_object().unwrap().clone();
        for artifact in ["proposal", "specs", "design", "tasks"] {
            let phase = phases.get(artifact).and_then(Value::as_str).unwrap_or("");
            match phase {
                "awaiting_review" => {
                    let revision = view["currentRevisions"][artifact].as_u64().unwrap();
                    log.push(format!("approve {artifact} r{revision}"));
                    call(
                        &window,
                        "decide_spec_artifact",
                        json!({ "specId": spec, "artifactId": artifact, "expectedRevision": revision,
                                "decision": "approve", "note": null }),
                    );
                    acted = true;
                }
                "ready" => {
                    log.push(format!("generate {artifact}"));
                    call(
                        &window,
                        "generate_spec_artifact",
                        json!({ "specId": spec, "artifactId": artifact }),
                    );
                    acted = true;
                }
                "failed" => {
                    let detail = view["spec"]["artifacts"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|a| a["id"] == artifact)
                        .map(|a| a["generation"].to_string())
                        .unwrap_or_default();
                    // What the agent answered, to tell a model's slip from Atlas's.
                    let messages = call(
                        &window,
                        "list_messages",
                        json!({ "workspaceId": workspace }),
                    );
                    if let Some(last) = messages
                        .as_array()
                        .and_then(|m| m.iter().rev().find(|m| m["role"] == "assistant"))
                    {
                        let text = last["content"].as_str().unwrap_or_default();
                        println!(
                            "--- {artifact} answer ({} chars), first 2500:\n{}",
                            text.len(),
                            &text[..text.len().min(2500)]
                        );
                    }
                    // As the person would: "Generate again", twice at most.
                    retries += 1;
                    assert!(retries <= 2, "{artifact} failed: {detail}\n{log:#?}");
                    log.push(format!("{artifact} failed ({detail}); generate again"));
                    call(
                        &window,
                        "generate_spec_artifact",
                        json!({ "specId": spec, "artifactId": artifact }),
                    );
                    acted = true;
                }
                _ => {}
            }
            if acted {
                break;
            }
        }
        if acted {
            continue;
        }
        if phases.values().any(|p| p != "approved") {
            std::thread::sleep(Duration::from_secs(2));
            continue;
        }
        for finding in view["spec"]["findings"].as_array().into_iter().flatten() {
            if finding["status"] == "open" {
                dismissed.push(format!(
                    "[{}] {}: {}",
                    finding["severity"], finding["title"], finding["description"]
                ));
                call(
                    &window,
                    "resolve_spec_finding",
                    json!({ "specId": spec, "findingId": finding["id"], "status": "dismissed",
                            "note": "Recorded by the end-to-end test; not judged." }),
                );
                acted = true;
            }
        }
        if acted {
            continue;
        }
        if view["consistencyCurrent"] != true {
            log.push("consistency".to_owned());
            call(&window, "validate_spec", json!({ "specId": spec }));
        } else if view["reviewCurrent"] != true {
            log.push("review".to_owned());
            call(&window, "run_spec_review", json!({ "specId": spec }));
        } else if view["finalApprovalCurrent"] != true {
            log.push("final approval".to_owned());
            call(&window, "approve_spec", json!({ "specId": spec }));
        } else if view["publicationCurrent"] != true {
            log.push("publish".to_owned());
            call(
                &window,
                "publish_spec",
                json!({ "specId": spec, "changeId": null }),
            );
        } else {
            break view;
        }
        std::thread::sleep(Duration::from_millis(500));
    };
    println!("steps: {log:#?}");
    println!("review findings dismissed by the test: {dismissed:#?}");
    println!("documents generated again after a failure: {retries}");
    println!("usage: {}", view["usage"]);
    let change = view["spec"]["changeId"].as_str().unwrap();
    let folder = project.join("openspec/changes").join(change);
    let files = walk(&folder);
    println!("published: {files:?}");
    assert!(files.iter().any(|f| f == "proposal.md"), "{files:?}");
    assert!(files.iter().any(|f| f == "tasks.md"), "{files:?}");
    assert!(files.iter().any(|f| f.starts_with("specs/")), "{files:?}");
    let checked = call(&window, "check_spec_with_cli", json!({ "specId": spec }));
    println!("openspec CLI: {}", checked["spec"]["cliCheck"]);
    assert_eq!(checked["spec"]["cliCheck"]["valid"], true);
}
