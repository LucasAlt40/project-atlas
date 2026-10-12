//! The HTTP side of mobile access (ADR 0034): a small hyper server that answers a phone's browser.
//!
//! It is an adapter: every decision about who may do what is in `application::remote`; this file
//! only turns requests into calls to it and back. Every request passes, in order, the checks of
//! ADR 0034 §6 — Host, then (for writes) Origin and content type, then the session — and nothing
//! reaches the gateway before that.
//!
//! It runs on the async runtime, apart from the threads that drive workflow runs: stopping it, or
//! it failing, affects no run.

use std::collections::HashMap;
use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full, Limited};
use hyper::body::{Frame, Incoming};
use hyper::header::{self, HeaderValue};
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::{TokioIo, TokioTimer};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Semaphore};
use tokio::task::JoinSet;

use crate::application::remote::gateway::{AnswerRequest, ControlRequest, RemoteError};
use crate::application::remote::hub::{RemoteEvent, Replay};
use crate::application::remote::sessions::{AuthError, Device, PairError};
use crate::application::remote::{AssetSource, RemoteAccess, RemoteFailure, PORTS};
use crate::application::support::{now_ms, LockExt};
use crate::application::workflow::runner::WorkflowObserver;

const COOKIE: &str = "atlas_session";
/// A request body is a few words; anything larger is not from the page.
const MAX_BODY: usize = 16 * 1024;
const MAX_CONNECTIONS: usize = 48;
const MAX_CONNECTIONS_PER_ADDRESS: u32 = 12;
/// How long a client has to send its request line and headers (and how long an idle connection is kept).
#[cfg(not(test))]
const HEADER_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(test)]
const HEADER_TIMEOUT: Duration = Duration::from_secs(1);
/// The stream says it is alive this often, which is also when it notices a revoked session.
const HEARTBEAT: Duration = Duration::from_secs(15);
const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; \
                   img-src 'self' data:; font-src 'self'; connect-src 'self'; \
                   frame-ancestors 'none'; base-uri 'none'; form-action 'none'";

type Body = BoxBody<Bytes, Infallible>;

/// What the server needs from the app, supplied when it starts.
pub struct ServerDeps {
    pub access: Arc<RemoteAccess>,
    /// The desktop's own observer: a remote answer is announced exactly as a local one.
    pub observer: Arc<dyn WorkflowObserver>,
    pub assets: Arc<dyn AssetSource>,
}

pub struct Started {
    pub port: u16,
    /// Ends the listener and every connection.
    pub stop: Box<dyn FnOnce() + Send>,
}

struct Shared {
    deps: ServerDeps,
    /// `ip:port`, `localhost:port`… the only values of `Host` that are accepted.
    hosts: Vec<String>,
    per_address: Mutex<HashMap<IpAddr, u32>>,
}

/// Starts listening on `ip` at the first free port of [`PORTS`]. Must be called from inside the
/// async runtime.
///
/// # Errors
///
/// [`RemoteFailure::PortUnavailable`] if every port is taken, [`RemoteFailure::BindFailed`] if the
/// system refuses for another reason.
pub fn start(ip: IpAddr, deps: ServerDeps) -> Result<Started, RemoteFailure> {
    start_in(ip, PORTS, deps)
}

/// As [`start`], over the ports given (tests use their own).
///
/// # Errors
///
/// As [`start`].
pub fn start_in(
    ip: IpAddr,
    ports: impl IntoIterator<Item = u16>,
    deps: ServerDeps,
) -> Result<Started, RemoteFailure> {
    let mut last = RemoteFailure::PortUnavailable;
    for port in ports {
        match std::net::TcpListener::bind(SocketAddr::new(ip, port)) {
            Ok(listener) => return serve(listener, ip, port, deps),
            Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {}
            Err(error) if error.kind() == std::io::ErrorKind::AddrNotAvailable => {
                return Err(RemoteFailure::AddressUnavailable)
            }
            Err(_) => last = RemoteFailure::BindFailed,
        }
    }
    Err(last)
}

fn serve(
    listener: std::net::TcpListener,
    ip: IpAddr,
    port: u16,
    deps: ServerDeps,
) -> Result<Started, RemoteFailure> {
    listener
        .set_nonblocking(true)
        .map_err(|_| RemoteFailure::BindFailed)?;
    let listener = TcpListener::from_std(listener).map_err(|_| RemoteFailure::BindFailed)?;
    let mut hosts = vec![format!("{ip}:{port}")];
    for name in ["localhost", "127.0.0.1"] {
        hosts.push(format!("{name}:{port}"));
    }
    let context = Arc::new(Shared {
        deps,
        hosts,
        per_address: Mutex::new(HashMap::new()),
    });
    let task = tokio::spawn(accept_loop(listener, context));
    Ok(Started {
        port,
        stop: Box::new(move || task.abort()),
    })
}

/// A connection counted against its address, so one host cannot take every slot.
struct Slot {
    context: Arc<Shared>,
    address: IpAddr,
}

impl Slot {
    fn take(context: &Arc<Shared>, address: IpAddr) -> Option<Self> {
        let mut counts = context.per_address.lock_or_recover();
        let count = counts.entry(address).or_insert(0);
        if *count >= MAX_CONNECTIONS_PER_ADDRESS {
            return None;
        }
        *count += 1;
        Some(Self {
            context: context.clone(),
            address,
        })
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        let mut counts = self.context.per_address.lock_or_recover();
        if let Some(count) = counts.get_mut(&self.address) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                counts.remove(&self.address);
            }
        }
    }
}

async fn accept_loop(listener: TcpListener, context: Arc<Shared>) {
    let permits = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    // Dropping the set (when this task is aborted) aborts every connection with it.
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let Ok((stream, peer)) = accepted else {
                    // A failed accept (too many open files…) is not a reason to stop serving.
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                };
                let Ok(permit) = permits.clone().try_acquire_owned() else { continue };
                let Some(slot) = Slot::take(&context, peer.ip()) else { continue };
                let context = context.clone();
                connections.spawn(async move {
                    let _held = (permit, slot);
                    let service = service_fn(move |request| {
                        let context = context.clone();
                        async move { Ok::<_, Infallible>(route(request, peer, &context).await) }
                    });
                    let _ = hyper::server::conn::http1::Builder::new()
                        .timer(TokioTimer::new())
                        .header_read_timeout(HEADER_TIMEOUT)
                        .serve_connection(TokioIo::new(stream), service)
                        .await;
                });
            }
            Some(_) = connections.join_next(), if !connections.is_empty() => {}
        }
    }
}

// ---- responses ---------------------------------------------------------------------------------

fn full(bytes: impl Into<Bytes>) -> Body {
    Full::new(bytes.into())
        .map_err(|never| match never {})
        .boxed()
}

fn json<T: Serialize>(status: u16, value: &T) -> Response<Body> {
    let body = serde_json::to_vec(value).unwrap_or_else(|_| b"{}".to_vec());
    let mut response = Response::new(full(body));
    *response.status_mut() = StatusCode::from_u16(status).unwrap_or(StatusCode::OK);
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    response
}

fn error(status: u16, code: &str) -> Response<Body> {
    json(status, &serde_json::json!({ "error": { "code": code } }))
}

fn remote_error(error: RemoteError) -> Response<Body> {
    self::error(error.status(), error.code())
}

/// Headers every response carries, whatever it is.
fn hardened(mut response: Response<Body>) -> Response<Body> {
    let headers = response.headers_mut();
    for (name, value) in [
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
        ("referrer-policy", "no-referrer"),
        ("cross-origin-resource-policy", "same-origin"),
        ("content-security-policy", CSP),
    ] {
        headers.insert(
            header::HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    // There is no CORS: nothing from another origin is ever allowed to read an answer.
    headers
        .entry(header::CACHE_CONTROL)
        .or_insert(HeaderValue::from_static("no-store"));
    response
}

// ---- the checks ----------------------------------------------------------------------------------

/// Whether `Host` names this server. A browser led to another name (DNS rebinding) sends the
/// attacker's name here, so nothing the attacker's page asks is answered.
fn host_ok(request: &Request<Incoming>, context: &Shared) -> bool {
    request
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|host| {
            context
                .hosts
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(host))
        })
}

/// A write must come from this server's own page: `Origin` is one of its hosts, the browser says
/// the request is same-origin when it says anything, and the body is JSON (which a cross-site
/// form cannot send).
fn write_ok(request: &Request<Incoming>, context: &Shared) -> bool {
    let headers = request.headers();
    let origin_ok = headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|origin| {
            context.hosts.iter().any(|host| {
                origin.len() == host.len() + "http://".len()
                    && origin[..7].eq_ignore_ascii_case("http://")
                    && origin[7..].eq_ignore_ascii_case(host)
            })
        });
    let site_ok = headers
        .get("sec-fetch-site")
        .and_then(|v| v.to_str().ok())
        .is_none_or(|site| site == "same-origin");
    let json_ok = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.starts_with("application/json"));
    origin_ok && site_ok && json_ok
}

fn token_of(request: &Request<Incoming>) -> Option<String> {
    request
        .headers()
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|line| line.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE)
        .map(|(_, value)| value.to_owned())
}

fn session_cookie(token: &str, max_age: u64) -> HeaderValue {
    // No `Secure`: the page is served over plain HTTP on purpose (ADR 0034 §7), and a browser
    // refuses a Secure cookie on it. HttpOnly keeps scripts away from it; Strict keeps other
    // sites from sending it.
    HeaderValue::from_str(&format!(
        "{COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={max_age}"
    ))
    .unwrap_or_else(|_| HeaderValue::from_static(""))
}

#[allow(clippy::result_large_err)] // an error here is a whole response, sent once
async fn read_json<T: for<'de> Deserialize<'de> + Default>(
    request: Request<Incoming>,
) -> Result<T, Response<Body>> {
    let bytes = Limited::new(request.into_body(), MAX_BODY)
        .collect()
        .await
        .map_err(|_| error(413, "too_large"))?
        .to_bytes();
    if bytes.is_empty() {
        return Ok(T::default());
    }
    serde_json::from_slice(&bytes).map_err(|_| error(400, "bad_request"))
}

// ---- routing -------------------------------------------------------------------------------------

async fn route(
    request: Request<Incoming>,
    peer: SocketAddr,
    context: &Arc<Shared>,
) -> Response<Body> {
    hardened(dispatch(request, peer, context).await)
}

async fn dispatch(
    request: Request<Incoming>,
    peer: SocketAddr,
    context: &Arc<Shared>,
) -> Response<Body> {
    if !host_ok(&request, context) {
        return error(400, "bad_host");
    }
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();

    // The page itself: public, and holds nothing that is not in the app anyway.
    if method == Method::GET && !path.starts_with("/api/") {
        return static_file(&path, context);
    }
    if !path.starts_with("/api/") {
        return error(405, "method_not_allowed");
    }

    if path == "/api/pair" {
        return if method == Method::POST {
            pair(request, peer, context).await
        } else {
            error(405, "method_not_allowed")
        };
    }

    let is_write = method != Method::GET;
    if is_write && !write_ok(&request, context) {
        return error(403, "bad_origin");
    }
    let now = now_ms();
    let device = match token_of(&request)
        .map(|token| context.deps.access.sessions.authenticate(&token, now))
    {
        Some(Ok(device)) => device,
        Some(Err(AuthError::Expired)) => return error(401, "session_expired"),
        _ => return error(401, "unauthenticated"),
    };
    if is_write && !context.deps.access.writes.allow(&device.id, now) {
        return error(429, "rate_limited");
    }

    match (method, segments.as_slice()) {
        (Method::GET, ["api", "session"]) => session(&device, context),
        (Method::DELETE, ["api", "session"]) => logout(&device, context),
        (Method::GET, ["api", "runs"]) => {
            let gateway = context.deps.access.gateway.clone();
            blocking(move || json(200, &gateway.overview())).await
        }
        (Method::GET, ["api", "runs", id]) => {
            let (gateway, id) = (context.deps.access.gateway.clone(), (*id).to_owned());
            blocking(move || match gateway.run(&id) {
                Ok(run) => json(200, &run),
                Err(e) => remote_error(e),
            })
            .await
        }
        (Method::POST, ["api", "runs", id, "interactions", interaction, "answer"]) => {
            let (id, interaction) = ((*id).to_owned(), (*interaction).to_owned());
            let body: AnswerRequest = match read_json(request).await {
                Ok(body) => body,
                Err(response) => return response,
            };
            let (gateway, observer) = (
                context.deps.access.gateway.clone(),
                context.deps.observer.clone(),
            );
            blocking(
                move || match gateway.answer(&id, &interaction, &body, observer.as_ref()) {
                    Ok(answered) => json(200, &answered),
                    Err(e) => remote_error(e),
                },
            )
            .await
        }
        (Method::POST, ["api", "runs", id, "control"]) => {
            let id = (*id).to_owned();
            let body: ControlRequest = match read_json(request).await {
                Ok(body) => body,
                Err(response) => return response,
            };
            let (gateway, observer) = (
                context.deps.access.gateway.clone(),
                context.deps.observer.clone(),
            );
            blocking(
                move || match gateway.control(&id, &body, observer.as_ref()) {
                    Ok(run) => json(200, &run),
                    Err(e) => remote_error(e),
                },
            )
            .await
        }
        (Method::GET, ["api", "events"]) => events(&request, &device, context),
        _ => error(404, "not_found"),
    }
}

/// Gateway calls take locks and may wait for a run's loop: they do not belong on the reactor.
async fn blocking(work: impl FnOnce() -> Response<Body> + Send + 'static) -> Response<Body> {
    tokio::task::spawn_blocking(work)
        .await
        .unwrap_or_else(|_| error(500, "internal"))
}

fn static_file(path: &str, context: &Shared) -> Response<Body> {
    let wanted = match path {
        "/" | "/index.html" | "/mobile.html" => "mobile.html",
        other => other.trim_start_matches('/'),
    };
    // Only the page and what it loads: no walking out of the assets, no desktop-only files.
    let allowed = wanted == "mobile.html"
        || (wanted.starts_with("assets/") && !wanted.contains("..") && !wanted.contains('\\'));
    let Some(asset) = allowed.then(|| context.deps.assets.get(wanted)).flatten() else {
        return error(404, "not_found");
    };
    let mut response = Response::new(full(asset.bytes));
    if let Ok(value) = HeaderValue::from_str(&asset.content_type) {
        response.headers_mut().insert(header::CONTENT_TYPE, value);
    }
    let cache = if wanted == "mobile.html" {
        "no-cache"
    } else {
        "public, max-age=3600"
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    response
}

// ---- pairing and session -------------------------------------------------------------------------

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairRequest {
    code: String,
    device_name: String,
}

async fn pair(
    request: Request<Incoming>,
    peer: SocketAddr,
    context: &Arc<Shared>,
) -> Response<Body> {
    if !write_ok(&request, context) {
        return error(403, "bad_origin");
    }
    let access = &context.deps.access;
    let now = now_ms();
    if !access.pair_attempts.allow(&peer.ip().to_string(), now) {
        return error(429, "rate_limited");
    }
    let body: PairRequest = match read_json(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };
    if access.sessions.devices(now).len() >= crate::application::remote::sessions::MAX_DEVICES {
        return error(409, "too_many_devices");
    }
    match access.pairing.redeem(&body.code, now) {
        Ok(()) => {}
        Err(PairError::Invalid) => return error(403, "invalid_code"),
        Err(PairError::Locked { retry_after_ms }) => {
            return json(
                429,
                &serde_json::json!({ "error": { "code": "pairing_locked", "retryAfterMs": retry_after_ms } }),
            );
        }
    }
    let Some((token, device)) =
        access
            .sessions
            .open(&body.device_name, &peer.ip().to_string(), now)
    else {
        return error(409, "too_many_devices");
    };
    let mut response = json(200, &session_body(&device, context));
    response.headers_mut().insert(
        header::SET_COOKIE,
        session_cookie(
            &token,
            crate::application::remote::sessions::SESSION_MAX_MS / 1000,
        ),
    );
    response
}

fn session_body(device: &Device, context: &Shared) -> serde_json::Value {
    let access = &context.deps.access;
    serde_json::json!({
        "device": { "id": device.id, "name": device.name },
        "serverTime": now_ms(),
        "epoch": access.hub.epoch(),
    })
}

fn session(device: &Device, context: &Shared) -> Response<Body> {
    let mut body = session_body(device, context);
    body["language"] = serde_json::Value::String(context.deps.access.gateway.language());
    json(200, &body)
}

fn logout(device: &Device, context: &Shared) -> Response<Body> {
    context.deps.access.revoke_device(&device.id);
    let mut response = json(200, &serde_json::json!({ "ok": true }));
    response
        .headers_mut()
        .insert(header::SET_COOKIE, session_cookie("", 0));
    response
}

// ---- the event stream ----------------------------------------------------------------------------

struct ChannelBody(mpsc::Receiver<Bytes>);

impl hyper::body::Body for ChannelBody {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Infallible>>> {
        self.0
            .poll_recv(cx)
            .map(|item| item.map(|b| Ok(Frame::data(b))))
    }
}

fn frame(event: &str, id: Option<&str>, data: &str) -> Bytes {
    let id = id.map_or_else(String::new, |id| format!("id: {id}\n"));
    Bytes::from(format!("{id}event: {event}\ndata: {data}\n\n"))
}

fn cursor_id(epoch: &str, seq: u64) -> String {
    format!("{epoch}:{seq}")
}

/// `epoch:seq` from `Last-Event-ID` (which the browser sends on its own when it reconnects) or
/// `?after=` (for a page that opens the stream afresh).
fn cursor_of(request: &Request<Incoming>) -> Option<(String, u64)> {
    let from_header = request
        .headers()
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let from_query = request.uri().query().and_then(|q| {
        q.split('&')
            .find_map(|pair| pair.strip_prefix("after=").map(str::to_owned))
    });
    let raw = from_header.or(from_query)?;
    let (epoch, seq) = raw.rsplit_once(':')?;
    Some((epoch.to_owned(), seq.parse().ok()?))
}

fn events(request: &Request<Incoming>, device: &Device, context: &Arc<Shared>) -> Response<Body> {
    let access = context.deps.access.clone();
    let cursor = cursor_of(request);
    let device_id = device.id.clone();
    let (tx, rx) = mpsc::channel::<Bytes>(64);
    tokio::spawn(async move {
        let hub = access.hub.clone();
        // How long the browser waits before it reconnects by itself.
        if tx
            .send(Bytes::from_static(b"retry: 3000\n\n"))
            .await
            .is_err()
        {
            return;
        }
        let mut wake = hub.subscribe();
        let epoch = hub.epoch();
        // The start of a stream: the page either resumes from a cursor or reads everything.
        let mut last = match &cursor {
            Some((e, seq)) => match hub.replay(Some(e), *seq) {
                Replay::Events(missed) => {
                    let mut last = *seq;
                    for event in missed {
                        last = event.seq;
                        if tx.send(event_frame(&epoch, &event)).await.is_err() {
                            return;
                        }
                    }
                    last
                }
                Replay::Resync => hub.seq(),
            },
            None => hub.seq(),
        };
        let resync = cursor
            .as_ref()
            .is_none_or(|(e, seq)| matches!(hub.replay(Some(e), *seq), Replay::Resync));
        if resync {
            let data = serde_json::json!({ "epoch": epoch, "seq": last }).to_string();
            if tx
                .send(frame("resync", Some(&cursor_id(&epoch, last)), &data))
                .await
                .is_err()
            {
                return;
            }
        }
        let mut beat = tokio::time::interval(HEARTBEAT);
        beat.tick().await;
        loop {
            let ticked = tokio::select! {
                changed = wake.changed() => { if changed.is_err() { return; } false }
                _ = beat.tick() => true,
                () = tx.closed() => return,
            };
            if !access.sessions.is_active(&device_id, now_ms()) {
                let _ = tx.send(frame("revoked", None, "{}")).await;
                return;
            }
            match hub.replay(Some(&epoch), last) {
                Replay::Events(events) => {
                    for event in events {
                        last = event.seq;
                        if tx.send(event_frame(&epoch, &event)).await.is_err() {
                            return;
                        }
                    }
                    if ticked && tx.send(Bytes::from_static(b": ping\n\n")).await.is_err() {
                        return;
                    }
                }
                Replay::Resync => {
                    last = hub.seq();
                    let data = serde_json::json!({ "epoch": epoch, "seq": last }).to_string();
                    if tx
                        .send(frame("resync", Some(&cursor_id(&epoch, last)), &data))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
            }
        }
    });
    let mut response = Response::new(ChannelBody(rx).boxed());
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert("x-accel-buffering", HeaderValue::from_static("no"));
    // When the stream ends (revoked session, server stopping) the connection ends with it.
    headers.insert(header::CONNECTION, HeaderValue::from_static("close"));
    response
}

fn event_frame(epoch: &str, event: &RemoteEvent) -> Bytes {
    let data = serde_json::to_string(event).unwrap_or_else(|_| "{}".to_owned());
    frame("change", Some(&cursor_id(epoch, event.seq)), &data)
}
