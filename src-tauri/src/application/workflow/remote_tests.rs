//! Mobile access end to end (ADR 0034): a real server on a loopback socket, a real orchestrator
//! driving a scripted run, and a raw HTTP client standing in for a phone's browser.
//!
//! They live here because they need the scripted runner and environment of the orchestrator tests.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::orchestrator::Orchestrator;
use super::orchestrator_tests::{
    asks, asks_with, dev_flow, env, ok, plan, wait_until, Env, Scripted, Step,
};
use super::runner::WorkflowObserver;
use crate::application::chat::ChatObserver;
use crate::application::executions::ExecutionObserver;
use crate::application::projects::fake::FakeInspector;
use crate::application::remote::gateway::RemoteGateway;
use crate::application::remote::hub::EventHub;
use crate::application::remote::network::fake::Fixed;
use crate::application::remote::{Asset, AssetSource, RemoteAccess};
use crate::application::settings::SettingsService;
use crate::application::support::now_ms;
use crate::application::workspace::WorkspaceService;
use crate::domain::conversation::Message;
use crate::domain::execution::ExecutionEvent;
use crate::domain::interaction::{DetectionSource, InteractionKind, InteractionStatus};
use crate::domain::workflow::{WorkflowEvent, WorkflowExecution, WorkflowExecutionStatus};
use crate::infrastructure::{start_remote_server_in, ServerDeps};

// ---- the fixture -------------------------------------------------------------------------------

/// What the desktop's observer does: tells the phone's hub, and keeps what it heard.
struct Tee {
    hub: Arc<EventHub>,
    heard: Mutex<Vec<String>>,
}

impl ExecutionObserver for Tee {
    fn on_event(&self, _: &ExecutionEvent) {}
}
impl ChatObserver for Tee {
    fn on_message(&self, _: &Message) {}
}
impl WorkflowObserver for Tee {
    fn on_workflow_event(&self, event: &WorkflowEvent) {
        self.heard.lock().unwrap().push(event.kind.wire_name());
        self.hub.publish_workflow(event);
    }
}

struct Files(HashMap<&'static str, (&'static str, &'static str)>);

impl AssetSource for Files {
    fn get(&self, path: &str) -> Option<Asset> {
        self.0.get(path).map(|(body, kind)| Asset {
            bytes: body.as_bytes().to_vec(),
            content_type: (*kind).to_owned(),
        })
    }
}

struct Fx {
    env: Env,
    orchestrator: Arc<Orchestrator>,
    access: Arc<RemoteAccess>,
    tee: Arc<Tee>,
    workflow_id: String,
    port: u16,
}

static NEXT_PORT: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(47_700);

impl Fx {
    /// A server on a free port, and a workflow whose developer step goes through `script`.
    fn new(script: Vec<Step>) -> Self {
        Self::serving(
            script,
            Arc::new(Files(HashMap::from([
                ("mobile.html", ("<html>mobile</html>", "text/html")),
                ("assets/app.js", ("console.log(1)", "text/javascript")),
                ("index.html", ("<html>DESKTOP</html>", "text/html")),
            ]))),
        )
    }

    fn serving(script: Vec<Step>, assets: Arc<dyn AssetSource>) -> Self {
        let env = env();
        let workflow = dev_flow(&env);
        let runner = Scripted::new();
        runner.script("developer", script);
        let orchestrator = Arc::new(env.orchestrator(&runner));
        let workspaces = Arc::new(WorkspaceService::new(
            env.config.clone(),
            env.agent_service.clone(),
            Arc::new(FakeInspector::with(&[("/atlas", &["Rust"])])),
        ));
        let gateway = Arc::new(RemoteGateway::new(
            env.service.clone(),
            workspaces,
            env.agent_service.clone(),
            Arc::new(SettingsService::new(env.config.clone())),
            orchestrator.clone(),
        ));
        let access = Arc::new(RemoteAccess::new(
            gateway,
            Arc::new(Fixed(vec![("lo0", "127.0.0.1")])),
        ));
        let tee = Arc::new(Tee {
            hub: access.hub.clone(),
            heard: Mutex::default(),
        });
        let mut fx = Self {
            env,
            orchestrator,
            access,
            tee,
            workflow_id: workflow.id,
            port: 0,
        };
        fx.listen(assets);
        fx
    }

    fn listen(&mut self, assets: Arc<dyn AssetSource>) {
        let ip = self.access.begin_start(Some("127.0.0.1")).unwrap();
        // Each test takes its own pair of ports, so tests can run side by side.
        let first = NEXT_PORT.fetch_add(2, std::sync::atomic::Ordering::SeqCst);
        let started = start_remote_server_in(
            ip,
            first..=first + 1,
            ServerDeps {
                access: self.access.clone(),
                observer: self.tee.clone(),
                assets,
            },
        )
        .unwrap();
        self.port = started.port;
        self.access.mark_active(ip, started.port, started.stop);
    }

    /// Starts a run and drives it on its own thread.
    fn begin(&self) -> (String, std::thread::JoinHandle<()>) {
        let exec = self
            .env
            .service
            .start_with_plans(&self.workflow_id, "Implement the users API", false)
            .unwrap();
        let (orchestrator, tee, id) =
            (self.orchestrator.clone(), self.tee.clone(), exec.id.clone());
        let thread = std::thread::spawn(move || orchestrator.run(&id, tee).unwrap());
        (exec.id, thread)
    }

    fn run_of(&self, id: &str) -> WorkflowExecution {
        self.env.service.execution(id).unwrap()
    }

    fn pending_id(&self, run: &str) -> String {
        wait_until("a question to wait", || {
            self.run_of(run).pending_interaction("developer").is_some()
        });
        self.run_of(run)
            .pending_interaction("developer")
            .unwrap()
            .id
            .clone()
    }

    fn origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Pairs a "phone" with the code on the desktop's screen and returns its cookie.
    async fn paired(&self) -> String {
        let code = self.access.pairing.current(now_ms()).unwrap().code;
        self.pair_with(&code)
            .await
            .cookie
            .expect("a session cookie")
    }

    async fn pair_with(&self, code: &str) -> Reply {
        self.send(
            "POST",
            "/api/pair",
            &[
                ("Origin", &self.origin()),
                ("Content-Type", "application/json"),
            ],
            Some(&format!(r#"{{"code":"{code}","deviceName":"Pixel"}}"#)),
        )
        .await
    }

    async fn get(&self, path: &str, cookie: &str) -> Reply {
        self.send("GET", path, &[("Cookie", cookie)], None).await
    }

    async fn post(&self, path: &str, cookie: &str, body: &str) -> Reply {
        let origin = self.origin();
        self.send(
            "POST",
            path,
            &[
                ("Cookie", cookie),
                ("Origin", &origin),
                ("Content-Type", "application/json"),
            ],
            Some(body),
        )
        .await
    }

    async fn send(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: Option<&str>,
    ) -> Reply {
        let mut text = format!("{method} {path} HTTP/1.1\r\n");
        if !headers.iter().any(|(n, _)| n.eq_ignore_ascii_case("host")) {
            let _ = write!(text, "Host: 127.0.0.1:{}\r\n", self.port);
        }
        for (name, value) in headers {
            let _ = write!(text, "{name}: {value}\r\n");
        }
        if let Some(body) = body {
            let _ = write!(text, "Content-Length: {}\r\n", body.len());
        }
        text.push_str("Connection: close\r\n\r\n");
        text.push_str(body.unwrap_or(""));
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).await.unwrap();
        stream.write_all(text.as_bytes()).await.unwrap();
        let mut raw = Vec::new();
        tokio::time::timeout(Duration::from_secs(15), stream.read_to_end(&mut raw))
            .await
            .expect("the server answered")
            .unwrap();
        Reply::parse(&String::from_utf8_lossy(&raw))
    }

    /// Opens the event stream and returns the socket to read frames from.
    async fn stream(&self, cookie: &str, last_event_id: Option<&str>) -> Stream {
        let mut text = format!(
            "GET /api/events HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nCookie: {cookie}\r\n",
            self.port
        );
        if let Some(id) = last_event_id {
            let _ = write!(text, "Last-Event-ID: {id}\r\n");
        }
        text.push_str("\r\n");
        let mut socket = TcpStream::connect(("127.0.0.1", self.port)).await.unwrap();
        socket.write_all(text.as_bytes()).await.unwrap();
        Stream {
            socket,
            seen: String::new(),
        }
    }
}

struct Stream {
    socket: TcpStream,
    seen: String,
}

impl Stream {
    /// Reads until what arrived contains `needle`; panics if it does not in time.
    async fn until(&mut self, needle: &str) -> String {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while !self.seen.contains(needle) {
            let mut chunk = [0_u8; 4096];
            let read = tokio::time::timeout_at(deadline, self.socket.read(&mut chunk))
                .await
                .unwrap_or_else(|_| panic!("timed out waiting for {needle:?}; saw {:?}", self.seen))
                .unwrap();
            assert!(
                read > 0,
                "the stream closed before {needle:?}; saw {:?}",
                self.seen
            );
            self.seen.push_str(&String::from_utf8_lossy(&chunk[..read]));
        }
        self.seen.clone()
    }

    /// Whether the server closes the stream soon.
    async fn closes(&mut self) -> bool {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            let mut chunk = [0_u8; 4096];
            match tokio::time::timeout_at(deadline, self.socket.read(&mut chunk)).await {
                Ok(Ok(0) | Err(_)) => return true,
                Ok(Ok(read)) => self.seen.push_str(&String::from_utf8_lossy(&chunk[..read])),
                Err(_) => return false,
            }
        }
    }
}

struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
    cookie: Option<String>,
}

impl Reply {
    fn parse(raw: &str) -> Self {
        let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((raw, ""));
        let mut lines = head.lines();
        let status = lines
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let headers: Vec<(String, String)> = lines
            .filter_map(|l| l.split_once(':'))
            .map(|(n, v)| (n.trim().to_ascii_lowercase(), v.trim().to_owned()))
            .collect();
        let cookie = headers
            .iter()
            .find(|(n, _)| n == "set-cookie")
            .and_then(|(_, v)| v.split(';').next())
            .map(str::to_owned);
        Self {
            status,
            headers,
            body: body.to_owned(),
            cookie,
        }
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).unwrap_or_else(|e| panic!("not json ({e}): {}", self.body))
    }

    fn code(&self) -> String {
        self.json()["error"]["code"]
            .as_str()
            .unwrap_or("")
            .to_owned()
    }
}

fn answer_path(run: &str, interaction: &str) -> String {
    format!("/api/runs/{run}/interactions/{interaction}/answer")
}

// ---- the page and the checks every request passes -------------------------------------------------

#[tokio::test]
async fn the_page_is_served_with_a_strict_policy_and_never_the_desktop_ui() {
    let fx = Fx::new(vec![plan()]);

    let page = fx.send("GET", "/", &[], None).await;
    assert_eq!(page.status, 200);
    assert!(page.body.contains("mobile"));
    let csp = page.header("content-security-policy").unwrap();
    assert!(csp.contains("default-src 'self'") && csp.contains("frame-ancestors 'none'"));
    assert_eq!(page.header("x-frame-options"), Some("DENY"));
    assert_eq!(page.header("referrer-policy"), Some("no-referrer"));
    assert!(
        page.header("access-control-allow-origin").is_none(),
        "no CORS, ever"
    );

    assert_eq!(
        fx.send("GET", "/assets/app.js", &[], None).await.status,
        200
    );
    // Only the page and its assets: not the desktop's entry, not a way out of the folder.
    for path in [
        "/index.html/../x",
        "/assets/../index.html",
        "/assets/missing.js",
        "/secret",
    ] {
        assert_eq!(fx.send("GET", path, &[], None).await.status, 404, "{path}");
    }
}

#[tokio::test]
async fn a_request_for_another_host_name_is_refused_so_dns_rebinding_gets_nothing() {
    let fx = Fx::new(vec![plan()]);
    let cookie = fx.paired().await;
    for host in [
        "evil.example",
        "evil.example:80",
        "127.0.0.1",
        "127.0.0.1:1",
    ] {
        let page = fx.send("GET", "/", &[("Host", host)], None).await;
        assert_eq!(page.status, 400, "{host}");
        let api = fx
            .send(
                "GET",
                "/api/runs",
                &[("Host", host), ("Cookie", &cookie)],
                None,
            )
            .await;
        assert_eq!(
            (api.status, api.code().as_str()),
            (400, "bad_host"),
            "{host}"
        );
    }
    // The names the server answers to still work.
    let local = format!("localhost:{}", fx.port);
    assert_eq!(
        fx.send(
            "GET",
            "/api/runs",
            &[("Host", &local), ("Cookie", &cookie)],
            None
        )
        .await
        .status,
        200
    );
}

#[tokio::test]
async fn nothing_can_be_read_or_done_without_a_session() {
    let fx = Fx::new(vec![plan()]);
    let (run, thread) = fx.begin();
    let interaction = fx.pending_id(&run);
    let origin = fx.origin();
    let json = [
        ("Origin", origin.as_str()),
        ("Content-Type", "application/json"),
    ];

    for path in [
        "/api/runs",
        &format!("/api/runs/{run}"),
        "/api/session",
        "/api/events",
    ] {
        let reply = fx.send("GET", path, &[], None).await;
        assert_eq!(
            (reply.status, reply.code().as_str()),
            (401, "unauthenticated"),
            "{path}"
        );
        let forged = fx
            .send("GET", path, &[("Cookie", "atlas_session=nothing")], None)
            .await;
        assert_eq!(forged.status, 401, "{path}");
    }
    let answer = fx
        .send(
            "POST",
            &answer_path(&run, &interaction),
            &json,
            Some(r#"{"choice":"approve"}"#),
        )
        .await;
    assert_eq!(answer.status, 401);
    let control = fx
        .send(
            "POST",
            &format!("/api/runs/{run}/control"),
            &json,
            Some(r#"{"action":"cancel","confirm":true}"#),
        )
        .await;
    assert_eq!(control.status, 401);

    // Nothing happened to the run.
    let still = fx.run_of(&run);
    assert_eq!(still.status, WorkflowExecutionStatus::WaitingForInput);
    assert!(still.pending_interaction("developer").is_some());
    fx.access.stop();
    fx.orchestrator
        .control(&run, super::orchestrator::Control::Cancel, fx.tee.as_ref())
        .unwrap();
    thread.join().unwrap();
}

#[tokio::test]
async fn a_write_must_come_from_the_servers_own_page() {
    let fx = Fx::new(vec![plan()]);
    let cookie = fx.paired().await;
    let (run, thread) = fx.begin();
    let interaction = fx.pending_id(&run);
    let path = answer_path(&run, &interaction);
    let body = r#"{"choice":"approve"}"#;
    let own = fx.origin();

    let attempts: Vec<(&str, Vec<(&str, &str)>)> = vec![
        (
            "another site's origin",
            vec![
                ("Origin", "http://evil.example"),
                ("Content-Type", "application/json"),
            ],
        ),
        (
            "a look-alike origin",
            vec![
                ("Origin", "http://127.0.0.1:1"),
                ("Content-Type", "application/json"),
            ],
        ),
        ("no origin", vec![("Content-Type", "application/json")]),
        (
            "a form post",
            vec![
                ("Origin", own.as_str()),
                ("Content-Type", "application/x-www-form-urlencoded"),
            ],
        ),
        (
            "plain text",
            vec![("Origin", own.as_str()), ("Content-Type", "text/plain")],
        ),
        (
            "a cross-site fetch",
            vec![
                ("Origin", own.as_str()),
                ("Content-Type", "application/json"),
                ("Sec-Fetch-Site", "cross-site"),
            ],
        ),
    ];
    for (what, mut headers) in attempts {
        headers.push(("Cookie", cookie.as_str()));
        let reply = fx.send("POST", &path, &headers, Some(body)).await;
        assert_eq!(
            (reply.status, reply.code().as_str()),
            (403, "bad_origin"),
            "{what}"
        );
    }
    assert!(
        fx.run_of(&run).pending_interaction("developer").is_some(),
        "none of them reached the run"
    );

    let good = fx.post(&path, &cookie, body).await;
    assert_eq!(good.status, 200, "{}", good.body);
    thread.join().unwrap();
}

#[tokio::test]
async fn writes_are_limited_per_device() {
    let fx = Fx::new(vec![plan()]);
    let cookie = fx.paired().await;
    let mut limited = false;
    for _ in 0..40 {
        let reply = fx
            .post("/api/runs/none/control", &cookie, r#"{"action":"pause"}"#)
            .await;
        if reply.status == 429 {
            assert_eq!(reply.code(), "rate_limited");
            limited = true;
            break;
        }
        assert_eq!(reply.status, 404);
    }
    assert!(limited, "a device cannot hammer what changes things");
}

// ---- pairing, sessions, revocation -------------------------------------------------------------------

#[tokio::test]
async fn pairing_turns_a_single_use_code_into_a_session_that_never_shows_its_token() {
    let fx = Fx::new(vec![plan()]);
    let issued = fx.access.pairing.current(now_ms()).unwrap();

    let paired = fx.pair_with(&issued.code).await;
    assert_eq!(paired.status, 200, "{}", paired.body);
    let set_cookie = paired.header("set-cookie").unwrap();
    assert!(set_cookie.contains("HttpOnly") && set_cookie.contains("SameSite=Strict"));
    assert!(set_cookie.contains("Path=/"));
    let token = set_cookie
        .split(';')
        .next()
        .and_then(|c| c.split_once('='))
        .map(|(_, v)| v.to_owned())
        .unwrap();
    assert_eq!(token.len(), 64);
    assert!(
        !paired.body.contains(&token),
        "the token is only in the cookie"
    );
    assert_eq!(paired.json()["device"]["name"], "Pixel");

    // The code was spent: a second phone with the same photo of the QR gets nothing.
    let again = fx.pair_with(&issued.code).await;
    assert_eq!((again.status, again.code().as_str()), (403, "invalid_code"));
    assert!(again.cookie.is_none());

    let cookie = paired.cookie.unwrap();
    let session = fx.get("/api/session", &cookie).await;
    assert_eq!(session.status, 200);
    assert_eq!(session.json()["language"], "pt-BR");
    assert_eq!(fx.access.status().devices.len(), 1);
}

#[tokio::test]
async fn guessing_codes_locks_pairing_and_burns_the_real_code() {
    let fx = Fx::new(vec![plan()]);
    let real = fx.access.pairing.current(now_ms()).unwrap().code;
    let mut last = fx.pair_with("AAAAAAAAAAAAAAAA").await;
    for _ in 0..4 {
        assert!(matches!(last.status, 403 | 429));
        last = fx.pair_with("BBBBBBBBBBBBBBBB").await;
    }
    assert_eq!(last.status, 429);
    assert_eq!(last.code(), "pairing_locked");
    assert!(last.json()["error"]["retryAfterMs"].as_u64().unwrap() > 0);

    let with_real = fx.pair_with(&real).await;
    assert_eq!(
        with_real.status, 429,
        "a right code does not get through a lock"
    );
    assert!(with_real.cookie.is_none());
    assert!(
        fx.access.status().pairing.is_none(),
        "the desktop shows no code to scan"
    );
}

#[tokio::test]
async fn a_revoked_session_stops_working_at_once() {
    let fx = Fx::new(vec![plan()]);
    let cookie = fx.paired().await;
    assert_eq!(fx.get("/api/runs", &cookie).await.status, 200);

    let device = fx.access.status().devices[0].id.clone();
    assert!(fx.access.revoke_device(&device));

    let after = fx.get("/api/runs", &cookie).await;
    assert_eq!(
        (after.status, after.code().as_str()),
        (401, "unauthenticated")
    );
    assert_eq!(fx.access.status().devices.len(), 0);
}

#[tokio::test]
async fn a_new_code_replaces_the_old_one_and_revoking_everything_clears_unused_codes() {
    let fx = Fx::new(vec![plan()]);
    let old = fx.access.pairing.current(now_ms()).unwrap().code;
    fx.access.new_pairing_code().unwrap();
    assert_eq!(fx.pair_with(&old).await.code(), "invalid_code");

    let cookie = fx.paired().await;
    fx.access.new_pairing_code().unwrap();
    assert!(fx.access.status().pairing.is_some());
    assert_eq!(fx.access.revoke_all_devices(), 1);
    assert!(
        fx.access.status().pairing.is_none(),
        "an unused code is a way in too"
    );
    assert_eq!(fx.get("/api/runs", &cookie).await.status, 401);
}

#[tokio::test]
async fn the_number_of_paired_devices_is_bounded_and_the_code_is_kept_when_full() {
    let fx = Fx::new(vec![plan()]);
    for _ in 0..crate::application::remote::sessions::MAX_DEVICES {
        fx.paired().await;
        fx.access.new_pairing_code().unwrap();
    }
    let code = fx.access.pairing.current(now_ms()).unwrap().code;
    let full = fx.pair_with(&code).await;
    assert_eq!(
        (full.status, full.code().as_str()),
        (409, "too_many_devices")
    );
    assert!(
        fx.access.pairing.current(now_ms()).is_some(),
        "not spent on a refusal"
    );
}

// ---- reading -------------------------------------------------------------------------------------

#[tokio::test]
async fn the_dashboard_puts_what_waits_for_a_person_first_and_shows_only_what_is_known() {
    let fx = Fx::new(vec![plan()]);
    let cookie = fx.paired().await;
    let (run, thread) = fx.begin();
    fx.pending_id(&run);

    let list = fx.get("/api/runs", &cookie).await;
    assert_eq!(list.status, 200);
    let body = list.json();
    let first = &body["runs"][0];
    assert_eq!(first["id"], run.as_str());
    assert_eq!(first["status"], "waiting_for_input");
    assert_eq!(first["attention"], "needs_answer");
    assert_eq!(first["pending"], 1);
    assert_eq!(first["workspaceName"], "Atlas");
    assert_eq!(first["workflowName"], "Password recovery");
    // A count of steps, never an invented percentage.
    assert_eq!(first["stepsTotal"], 2);
    assert_eq!(first["stepsDone"], 0);
    assert!(first.get("progress").is_none() && first.get("percent").is_none());
    assert_eq!(first["current"][0]["status"], "waiting_for_input");

    fx.access.stop();
    fx.orchestrator
        .control(&run, super::orchestrator::Control::Cancel, fx.tee.as_ref())
        .unwrap();
    thread.join().unwrap();
}

#[tokio::test]
async fn a_run_shows_its_steps_the_plan_and_what_the_phone_may_do_without_folders_or_secrets() {
    let document =
        "# Plan\n\n- add the route\n- API_KEY=sk-abcdefghijklmnopqrstuvwxyz0123\n- test it";
    let fx = Fx::new(vec![asks_with(
        InteractionKind::Plan,
        "Approve this plan?",
        document,
        DetectionSource::Structured,
    )]);
    let cookie = fx.paired().await;
    let (run, thread) = fx.begin();
    let interaction = fx.pending_id(&run);

    let detail = fx.get(&format!("/api/runs/{run}"), &cookie).await;
    assert_eq!(detail.status, 200);
    let json = detail.json();
    assert_eq!(json["steps"].as_array().unwrap().len(), 3);
    assert_eq!(json["steps"][0]["nodeId"], "developer");
    assert_eq!(json["steps"][0]["agent"], "developer agent");
    assert!(json["edges"].as_array().unwrap().len() >= 2);
    let question = &json["interactions"][0];
    assert_eq!(question["id"], interaction.as_str());
    assert_eq!(question["kind"], "plan");
    assert_eq!(question["remote"]["canAnswer"], true);
    assert_eq!(
        question["options"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["approve", "revise", "redo"]
    );
    let shown = question["document"].as_str().unwrap();
    assert!(shown.contains("add the route") && shown.contains("test it"));
    assert!(
        !shown.contains("sk-abcdefghij"),
        "a secret in a plan is not sent"
    );
    assert!(
        !detail.body.contains("/atlas"),
        "no folder of the computer leaves it"
    );
    assert_eq!(json["controls"]["cancel"], true);

    assert_eq!(
        fx.get("/api/runs/does-not-exist", &cookie).await.status,
        404
    );

    fx.access.stop();
    fx.orchestrator
        .control(&run, super::orchestrator::Control::Cancel, fx.tee.as_ref())
        .unwrap();
    thread.join().unwrap();
}

#[tokio::test]
async fn runs_that_ended_long_ago_are_not_listed_but_what_is_going_always_is() {
    let fx = Fx::new(vec![ok("success", ""), plan()]);
    let cookie = fx.paired().await;
    let (old, thread) = fx.begin();
    thread.join().unwrap();
    let mut finished = fx.run_of(&old);
    assert!(finished.status.is_final());
    finished.updated_at = 1;
    fx.env.service.save_execution(&finished).unwrap();
    let (waiting, thread) = fx.begin();
    fx.pending_id(&waiting);

    let listed = fx.get("/api/runs", &cookie).await.json();
    let ids: Vec<&str> = listed["runs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [waiting.as_str()]);
    // Still readable by its id, for a link that was kept.
    assert_eq!(
        fx.get(&format!("/api/runs/{old}"), &cookie).await.status,
        200
    );

    fx.access.stop();
    fx.orchestrator
        .control(
            &waiting,
            super::orchestrator::Control::Cancel,
            fx.tee.as_ref(),
        )
        .unwrap();
    thread.join().unwrap();
}

// ---- answering ------------------------------------------------------------------------------------

#[tokio::test]
async fn approving_a_plan_from_the_phone_goes_through_the_orchestrator_and_the_desktop_hears_it() {
    let fx = Fx::new(vec![plan(), ok("success", "")]);
    let cookie = fx.paired().await;
    let (run, thread) = fx.begin();
    let interaction = fx.pending_id(&run);

    let reply = fx
        .post(
            &answer_path(&run, &interaction),
            &cookie,
            r#"{"choice":"approve"}"#,
        )
        .await;
    assert_eq!(reply.status, 200, "{}", reply.body);
    let json = reply.json();
    assert_eq!(json["applied"], "now");
    // What is shown is what the backend recorded.
    let recorded = &json["run"]["interactions"][0];
    assert_eq!(recorded["status"], "answered");
    assert_eq!(recorded["choice"], "approve");
    assert_eq!(recorded["auto"], false);

    thread.join().unwrap();
    let done = fx.run_of(&run);
    assert_eq!(done.status, WorkflowExecutionStatus::Completed);
    let heard = fx.tee.heard.lock().unwrap().clone();
    assert!(
        heard.contains(&"workflow:interaction_answered".to_owned()),
        "{heard:?}"
    );
}

#[tokio::test]
async fn asking_for_changes_needs_words_and_is_refused_by_the_engine_not_by_the_page() {
    let fx = Fx::new(vec![plan(), plan(), ok("success", "")]);
    let cookie = fx.paired().await;
    let (run, thread) = fx.begin();
    let first = fx.pending_id(&run);
    let path = answer_path(&run, &first);

    let empty = fx.post(&path, &cookie, r#"{"choice":"revise"}"#).await;
    assert_eq!(
        (empty.status, empty.code().as_str()),
        (422, "invalid_answer")
    );
    let not_an_option = fx
        .post(&path, &cookie, r#"{"choice":"delete-everything"}"#)
        .await;
    assert_eq!(not_an_option.code(), "invalid_answer");
    assert!(fx.run_of(&run).pending_interaction("developer").is_some());

    let revised = fx
        .post(
            &path,
            &cookie,
            r#"{"choice":"revise","text":"Split it in two steps"}"#,
        )
        .await;
    assert_eq!(revised.status, 200, "{}", revised.body);
    // The agent presents the plan again; this one gets approved.
    wait_until("the revised plan", || {
        fx.run_of(&run)
            .pending_interaction("developer")
            .is_some_and(|i| i.id != first)
    });
    let second = fx.pending_id(&run);
    assert_eq!(
        fx.post(
            &answer_path(&run, &second),
            &cookie,
            r#"{"choice":"approve"}"#
        )
        .await
        .status,
        200
    );
    thread.join().unwrap();
}

#[tokio::test]
async fn answering_twice_is_idempotent_and_answering_differently_is_a_conflict() {
    let fx = Fx::new(vec![plan(), ok("success", "")]);
    let cookie = fx.paired().await;
    let (run, thread) = fx.begin();
    let interaction = fx.pending_id(&run);
    let path = answer_path(&run, &interaction);

    let first = fx.post(&path, &cookie, r#"{"choice":"approve"}"#).await;
    assert_eq!(first.json()["applied"], "now");
    // A retry after a reply that never arrived: the same answer is not an error.
    let retry = fx.post(&path, &cookie, r#"{"choice":"approve"}"#).await;
    assert_eq!(retry.status, 200);
    assert_eq!(retry.json()["applied"], "already");
    // A different answer to a question that is already resolved is refused.
    let other = fx.post(&path, &cookie, r#"{"choice":"redo"}"#).await;
    assert_eq!(
        (other.status, other.code().as_str()),
        (409, "already_resolved")
    );
    thread.join().unwrap();

    // And a question of a run that has ended cannot be answered any more.
    let late = fx
        .post(&path, &cookie, r#"{"choice":"revise","text":"too late"}"#)
        .await;
    assert_eq!(late.code(), "already_resolved");
    assert_eq!(fx.run_of(&run).status, WorkflowExecutionStatus::Completed);
    assert_eq!(
        fx.post(
            &answer_path(&run, "no-such-question"),
            &cookie,
            r#"{"choice":"approve"}"#
        )
        .await
        .status,
        404
    );
}

#[tokio::test]
async fn when_the_desktop_answers_first_the_phone_is_told_what_was_decided() {
    let fx = Fx::new(vec![plan(), ok("success", "")]);
    let cookie = fx.paired().await;
    let (run, thread) = fx.begin();
    let interaction = fx.pending_id(&run);

    // The person approves at the computer while the phone is still showing the plan.
    fx.orchestrator
        .answer_interaction(
            &run,
            &interaction,
            crate::domain::interaction::InteractionAnswer {
                choice: Some("approve".into()),
                text: None,
            },
            fx.tee.as_ref(),
        )
        .unwrap();
    wait_until("the answer", || {
        fx.run_of(&run)
            .interactions
            .iter()
            .any(|i| i.status == InteractionStatus::Answered)
    });

    let path = answer_path(&run, &interaction);
    let rejects = fx.post(&path, &cookie, r#"{"choice":"redo"}"#).await;
    assert_eq!(rejects.code(), "already_resolved");
    let agrees = fx.post(&path, &cookie, r#"{"choice":"approve"}"#).await;
    assert_eq!(agrees.json()["applied"], "already");
    let state = fx.get(&format!("/api/runs/{run}"), &cookie).await.json();
    assert_eq!(state["interactions"][0]["choice"], "approve");
    thread.join().unwrap();
}

#[tokio::test]
async fn a_phone_and_the_desktop_answering_at_once_produce_one_decision() {
    let fx = Fx::new(vec![plan(), ok("success", "")]);
    let cookie = fx.paired().await;
    let (run, thread) = fx.begin();
    let interaction = fx.pending_id(&run);
    let path = answer_path(&run, &interaction);

    let desktop = {
        let (orchestrator, tee, run, interaction) = (
            fx.orchestrator.clone(),
            fx.tee.clone(),
            run.clone(),
            interaction.clone(),
        );
        std::thread::spawn(move || {
            orchestrator
                .answer_interaction(
                    &run,
                    &interaction,
                    crate::domain::interaction::InteractionAnswer {
                        choice: Some("redo".into()),
                        text: None,
                    },
                    tee.as_ref(),
                )
                .is_ok()
        })
    };
    let phone = fx.post(&path, &cookie, r#"{"choice":"approve"}"#).await;
    let desktop_sent = desktop.join().unwrap();

    // Whatever the order, the run holds exactly one answer, and the phone says which.
    let recorded: Vec<_> = fx
        .run_of(&run)
        .interactions
        .iter()
        .filter(|i| i.id == interaction)
        .map(|i| (i.status, i.choice.clone()))
        .collect();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].0, InteractionStatus::Answered);
    let decided = recorded[0].1.clone().unwrap();
    match phone.status {
        200 => assert_eq!(decided, "approve"),
        409 => {
            assert_eq!(phone.code(), "already_resolved");
            assert_eq!(decided, "redo");
        }
        other => panic!("unexpected {other}: {}", phone.body),
    }
    assert!(desktop_sent || phone.status == 200);
    // Either way the run is not left waiting: finish it.
    wait_until("the run to move on", || {
        fx.run_of(&run).pending_interaction("developer").is_none()
            || fx.run_of(&run).status.is_final()
    });
    if let Some(next) = fx
        .run_of(&run)
        .pending_interaction("developer")
        .map(|i| i.id.clone())
    {
        let _ = fx
            .post(
                &answer_path(&run, &next),
                &cookie,
                r#"{"choice":"approve"}"#,
            )
            .await;
    }
    let _ = thread;
}

#[tokio::test]
async fn a_permission_can_be_refused_from_the_phone_but_never_granted() {
    let fx = Fx::new(vec![
        asks(InteractionKind::Permission, "May I run the migrations?"),
        ok("success", ""),
    ]);
    let cookie = fx.paired().await;
    let (run, thread) = fx.begin();
    let interaction = fx.pending_id(&run);
    let path = answer_path(&run, &interaction);

    let detail = fx.get(&format!("/api/runs/{run}"), &cookie).await.json();
    assert_eq!(
        detail["interactions"][0]["remote"]["desktopOnly"],
        "grants_authority"
    );
    assert_eq!(detail["interactions"][0]["remote"]["choices"][0], "deny");
    assert_eq!(detail["attention"], "needs_answer");

    let allow = fx.post(&path, &cookie, r#"{"choice":"allow"}"#).await;
    assert_eq!((allow.status, allow.code().as_str()), (403, "desktop_only"));
    assert!(fx.run_of(&run).pending_interaction("developer").is_some());

    let deny = fx.post(&path, &cookie, r#"{"choice":"deny"}"#).await;
    assert_eq!(deny.status, 200, "{}", deny.body);
    thread.join().unwrap();
}

#[tokio::test]
async fn a_guardrail_question_is_for_the_computer_only() {
    let fx = Fx::new(vec![asks_with(
        InteractionKind::Approval,
        "Send this context to the model?",
        "",
        DetectionSource::Guardrail,
    )]);
    let cookie = fx.paired().await;
    let (run, thread) = fx.begin();
    let interaction = fx.pending_id(&run);

    let list = fx.get("/api/runs", &cookie).await.json();
    assert_eq!(list["runs"][0]["attention"], "needs_computer");
    for body in [r#"{"choice":"approve"}"#, r#"{"choice":"reject"}"#] {
        let reply = fx
            .post(&answer_path(&run, &interaction), &cookie, body)
            .await;
        assert_eq!(
            (reply.status, reply.code().as_str()),
            (403, "desktop_only"),
            "{body}"
        );
    }
    assert!(fx.run_of(&run).pending_interaction("developer").is_some());

    fx.access.stop();
    fx.orchestrator
        .control(&run, super::orchestrator::Control::Cancel, fx.tee.as_ref())
        .unwrap();
    thread.join().unwrap();
}

// ---- controlling a run -------------------------------------------------------------------------------

#[tokio::test]
async fn cancelling_needs_confirmation_and_keeps_the_run_consistent() {
    let fx = Fx::new(vec![plan()]);
    let cookie = fx.paired().await;
    let (run, thread) = fx.begin();
    fx.pending_id(&run);
    let path = format!("/api/runs/{run}/control");

    let unconfirmed = fx.post(&path, &cookie, r#"{"action":"cancel"}"#).await;
    assert_eq!(
        (unconfirmed.status, unconfirmed.code().as_str()),
        (422, "confirmation_required")
    );
    assert_eq!(fx.post(&path, &cookie, "{}").await.status, 422);
    assert_eq!(
        fx.post(&path, &cookie, r#"{"action":"explode","confirm":true}"#)
            .await
            .status,
        400
    );
    assert!(!fx.run_of(&run).status.is_final());

    let cancelled = fx
        .post(&path, &cookie, r#"{"action":"cancel","confirm":true}"#)
        .await;
    assert_eq!(cancelled.status, 200, "{}", cancelled.body);
    thread.join().unwrap();
    assert_eq!(fx.run_of(&run).status, WorkflowExecutionStatus::Cancelled);

    // A run that ended offers no more controls.
    let again = fx
        .post(&path, &cookie, r#"{"action":"cancel","confirm":true}"#)
        .await;
    assert_eq!(again.status, 409);
}

#[tokio::test]
async fn a_run_nobody_is_driving_must_be_resumed_at_the_computer() {
    let fx = Fx::new(vec![plan(), ok("success", "")]);
    let cookie = fx.paired().await;
    let (run, thread) = fx.begin();
    let interaction = fx.pending_id(&run);
    // The app is closed and opened again: the run is interrupted.
    fx.orchestrator
        .control(&run, super::orchestrator::Control::Cancel, fx.tee.as_ref())
        .unwrap();
    thread.join().unwrap();
    let mut interrupted = fx.run_of(&run);
    interrupted.status = WorkflowExecutionStatus::Interrupted;
    fx.env.service.save_execution(&interrupted).unwrap();

    let list = fx.get("/api/runs", &cookie).await.json();
    assert_eq!(list["runs"][0]["attention"], "needs_computer");
    let control = fx
        .post(
            &format!("/api/runs/{run}/control"),
            &cookie,
            r#"{"action":"cancel","confirm":true}"#,
        )
        .await;
    assert_eq!(control.code(), "run_not_live");
    let _ = interaction;
}

// ---- the stream ----------------------------------------------------------------------------------------

#[tokio::test]
async fn the_stream_starts_with_a_resync_then_announces_changes() {
    let fx = Fx::new(vec![plan(), ok("success", "")]);
    let cookie = fx.paired().await;
    let mut stream = fx.stream(&cookie, None).await;
    let head = stream.until("event: resync").await;
    assert!(head.starts_with("HTTP/1.1 200"));
    assert!(head.contains("text/event-stream"));
    assert!(head.contains("retry: 3000"));

    let (run, thread) = fx.begin();
    let interaction = fx.pending_id(&run);
    let seen = stream.until("interaction_detected").await;
    assert!(seen.contains("event: change"));
    assert!(seen.contains(&format!("\"runId\":\"{run}\"")));
    // Events say that something changed, never what.
    assert!(!seen.contains("Approve") && !seen.contains("step one"));

    fx.post(
        &answer_path(&run, &interaction),
        &cookie,
        r#"{"choice":"approve"}"#,
    )
    .await;
    stream.until("interaction_answered").await;
    thread.join().unwrap();
}

#[tokio::test]
async fn a_stream_that_reconnects_gets_what_it_missed_once_and_no_more() {
    let fx = Fx::new(vec![plan(), ok("success", "")]);
    let cookie = fx.paired().await;
    let epoch = fx.access.hub.epoch();

    let mut first = fx.stream(&cookie, None).await;
    first.until("event: resync").await;
    let (run, thread) = fx.begin();
    let interaction = fx.pending_id(&run);
    first.until("interaction_detected").await;
    let before = fx.access.hub.seq();
    drop(first); // the phone loses its Wi-Fi

    // While it is away the person answers at the computer.
    fx.orchestrator
        .answer_interaction(
            &run,
            &interaction,
            crate::domain::interaction::InteractionAnswer {
                choice: Some("approve".into()),
                text: None,
            },
            fx.tee.as_ref(),
        )
        .unwrap();
    thread.join().unwrap();

    let cursor = format!("{epoch}:{before}");
    let mut again = fx.stream(&cookie, Some(&cursor)).await;
    let seen = again.until("interaction_answered").await;
    assert!(
        !seen.contains("event: resync"),
        "it only missed a few events"
    );
    assert!(
        !seen.contains("interaction_detected"),
        "nothing is repeated"
    );

    // From a cursor of another start, or one that makes no sense, it reads everything again.
    for stale in ["deadbeef:3", &format!("{epoch}:99999"), "garbage"] {
        let mut fresh = fx.stream(&cookie, Some(stale)).await;
        fresh.until("event: resync").await;
    }
}

#[tokio::test]
async fn revoking_a_device_closes_its_stream() {
    let fx = Fx::new(vec![plan()]);
    let cookie = fx.paired().await;
    let mut stream = fx.stream(&cookie, None).await;
    stream.until("event: resync").await;

    let device = fx.access.status().devices[0].id.clone();
    fx.access.revoke_device(&device);
    stream.until("event: revoked").await;
    assert!(stream.closes().await);
}

// ---- abuse of the connection itself ---------------------------------------------------------------------

#[tokio::test]
async fn a_body_larger_than_the_page_would_ever_send_is_refused() {
    let fx = Fx::new(vec![plan()]);
    let cookie = fx.paired().await;
    let big = format!(r#"{{"action":"pause","pad":"{}"}}"#, "x".repeat(20_000));

    let reply = fx.post("/api/runs/none/control", &cookie, &big).await;

    assert_eq!((reply.status, reply.code().as_str()), (413, "too_large"));
}

#[tokio::test]
async fn a_connection_that_sends_nothing_is_closed_and_does_not_hold_a_slot() {
    let fx = Fx::new(vec![plan()]);
    let mut idle = TcpStream::connect(("127.0.0.1", fx.port)).await.unwrap();
    let mut byte = [0_u8; 1];

    let closed = tokio::time::timeout(Duration::from_secs(10), idle.read(&mut byte)).await;

    assert!(
        matches!(closed, Ok(Ok(0) | Err(_))),
        "the server hung up: {closed:?}"
    );
    assert_eq!(fx.send("GET", "/", &[], None).await.status, 200);
}

#[tokio::test]
async fn one_address_cannot_take_every_connection() {
    let fx = Fx::new(vec![plan()]);
    let mut held = Vec::new();
    for _ in 0..12 {
        held.push(TcpStream::connect(("127.0.0.1", fx.port)).await.unwrap());
    }
    // The next one from the same address is dropped as soon as it is accepted.
    let mut over = TcpStream::connect(("127.0.0.1", fx.port)).await.unwrap();
    let mut byte = [0_u8; 1];
    let dropped = tokio::time::timeout(Duration::from_secs(5), over.read(&mut byte)).await;
    assert!(matches!(dropped, Ok(Ok(0) | Err(_))), "{dropped:?}");

    // Once the others go, the address is served again.
    drop(held);
    let mut served = false;
    for _ in 0..50 {
        let page =
            tokio::time::timeout(Duration::from_secs(2), fx.send("GET", "/", &[], None)).await;
        if page.is_ok_and(|p| p.status == 200) {
            served = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(served);
}

// ---- isolation and lifecycle --------------------------------------------------------------------------------

#[tokio::test]
async fn switching_the_server_off_ends_access_and_leaves_the_runs_alone() {
    let fx = Fx::new(vec![plan(), ok("success", "")]);
    let cookie = fx.paired().await;
    let (run, thread) = fx.begin();
    let interaction = fx.pending_id(&run);
    let mut stream = fx.stream(&cookie, None).await;
    stream.until("event: resync").await;
    let port = fx.port;

    fx.access.stop();

    // The phone cannot reach anything any more...
    assert!(stream.closes().await);
    let mut refused = false;
    for _ in 0..50 {
        if TcpStream::connect(("127.0.0.1", port)).await.is_err() {
            refused = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(refused, "nothing listens once the server is off");
    assert_eq!(
        fx.access.status().state,
        crate::application::remote::RemoteState::Disabled
    );
    assert_eq!(fx.access.status().devices.len(), 0);
    assert!(fx.access.status().pairing.is_none());

    // ...and the run was not touched: it still waits, and the desktop can still answer it.
    let waiting = fx.run_of(&run);
    assert_eq!(waiting.status, WorkflowExecutionStatus::WaitingForInput);
    fx.orchestrator
        .answer_interaction(
            &run,
            &interaction,
            crate::domain::interaction::InteractionAnswer {
                choice: Some("approve".into()),
                text: None,
            },
            fx.tee.as_ref(),
        )
        .unwrap();
    thread.join().unwrap();
    assert_eq!(fx.run_of(&run).status, WorkflowExecutionStatus::Completed);
}

#[tokio::test]
async fn a_phone_that_comes_and_goes_changes_nothing_in_the_run() {
    let fx = Fx::new(vec![plan(), ok("success", "")]);
    let cookie = fx.paired().await;
    let (run, thread) = fx.begin();
    let interaction = fx.pending_id(&run);
    let before = fx.run_of(&run);

    for _ in 0..3 {
        let mut stream = fx.stream(&cookie, None).await;
        stream.until("event: resync").await;
        drop(stream);
        assert_eq!(fx.get("/api/runs", &cookie).await.status, 200);
    }
    assert_eq!(fx.run_of(&run), before, "watching changes nothing");

    fx.post(
        &answer_path(&run, &interaction),
        &cookie,
        r#"{"choice":"approve"}"#,
    )
    .await;
    thread.join().unwrap();
}

#[tokio::test]
async fn a_port_in_use_is_skipped_and_when_all_are_taken_the_failure_is_named() {
    let fx = Fx::new(vec![plan()]);
    let ip: IpAddr = "127.0.0.1".parse().unwrap();
    let deps = || ServerDeps {
        access: fx.access.clone(),
        observer: fx.tee.clone(),
        assets: Arc::new(Files(HashMap::new())),
    };
    let base = NEXT_PORT.fetch_add(3, std::sync::atomic::Ordering::SeqCst);
    let taken = std::net::TcpListener::bind((ip, base)).unwrap();

    let started = start_remote_server_in(ip, base..=base + 1, deps()).unwrap();
    assert_eq!(started.port, base + 1, "the busy port is skipped");

    let all_taken = start_remote_server_in(ip, base..=base + 1, deps());
    assert_eq!(
        all_taken.err(),
        Some(crate::application::remote::RemoteFailure::PortUnavailable)
    );
    drop(taken);
    (started.stop)();
}

#[tokio::test]
async fn the_server_binds_only_the_address_it_was_given() {
    let fx = Fx::new(vec![plan()]);
    // It answers on that address and refuses an address that is not offered.
    assert!(fx
        .access
        .status()
        .url
        .unwrap()
        .starts_with("http://127.0.0.1:"));
    fx.access.stop();
    assert_eq!(
        fx.access.begin_start(Some("0.0.0.0")),
        Err(crate::application::remote::RemoteFailure::AddressUnavailable)
    );
    assert_eq!(
        fx.access.begin_start(Some("8.8.8.8")),
        Err(crate::application::remote::RemoteFailure::AddressUnavailable)
    );
    assert!(
        fx.access.begin_start(None).is_err(),
        "loopback alone is not recommended"
    );
}

// ---- for looking at the page in a real browser ------------------------------------------------------

/// The built page, read from `ATLAS_MOBILE_DIST` (the folder `npm run build` makes).
struct DistFolder(std::path::PathBuf);

impl AssetSource for DistFolder {
    fn get(&self, path: &str) -> Option<Asset> {
        if path.contains("..") {
            return None;
        }
        let bytes = std::fs::read(self.0.join(path)).ok()?;
        let content_type = match path.rsplit('.').next() {
            Some("html") => "text/html; charset=utf-8",
            Some("js") => "text/javascript; charset=utf-8",
            Some("css") => "text/css; charset=utf-8",
            Some("woff2") => "font/woff2",
            _ => "application/octet-stream",
        };
        Some(Asset {
            bytes,
            content_type: content_type.to_owned(),
        })
    }
}

/// Not a test: serves the built phone page with a few runs (one waiting for a plan, one finished,
/// one failed) so it can be opened in a browser. Run it by name, never with a broad `--ignored`:
/// `ATLAS_MOBILE_DIST=dist ATLAS_PREVIEW_OUT=/path/to/url.txt cargo test --manifest-path
/// src-tauri/Cargo.toml serve_the_phone_page_for_a_look_in_a_browser -- --ignored --exact --nocapture`
#[tokio::test(flavor = "multi_thread")]
#[ignore = "serves a page until stopped; for a person to look at"]
async fn serve_the_phone_page_for_a_look_in_a_browser() {
    let dist = std::env::var("ATLAS_MOBILE_DIST").expect("ATLAS_MOBILE_DIST");
    let out = std::env::var("ATLAS_PREVIEW_OUT").expect("ATLAS_PREVIEW_OUT");
    let seconds: u64 = std::env::var("ATLAS_PREVIEW_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(900);
    let document = "# Plan: password recovery\n\n## Steps\n\n1. Add `POST /auth/recover` that takes an email\n2. Send a one-time link (valid 15 minutes)\n3. Add `POST /auth/reset` that checks the token and sets the password\n\n```ts\nexport async function recover(email: string) {\n  const token = await tokens.issue(email, { ttl: '15m' });\n  await mail.send(email, resetLink(token));\n}\n```\n\n| Risk | Mitigation |\n| --- | --- |\n| Account enumeration | Always answer 202 |\n| Token reuse | Single use, hashed at rest |\n\nTests: unit for the token, integration for both endpoints.";
    let fx = Fx::serving(
        vec![
            ok("success", ""),
            super::orchestrator_tests::fail("The tests did not pass"),
            asks_with(
                InteractionKind::Plan,
                "Do you approve this plan?",
                document,
                DetectionSource::Structured,
            ),
        ],
        Arc::new(DistFolder(dist.into())),
    );
    // One run per workflow at a time: one that finishes, one that fails, then the one that waits.
    let mut threads = Vec::new();
    for _ in 0..2 {
        let (run, thread) = fx.begin();
        threads.push(thread);
        wait_until("the run to end", || fx.run_of(&run).status.is_final());
    }
    let (waiting, thread) = fx.begin();
    threads.push(thread);
    fx.pending_id(&waiting);
    let code = fx.access.pairing.current(now_ms()).unwrap().code;
    let url = format!("{}/#{code}", fx.origin());
    std::fs::write(&out, &url).unwrap();
    println!("OPEN {url}");
    let _ = tokio::time::timeout(Duration::from_secs(seconds), std::future::pending::<()>()).await;
    fx.access.stop();
}
