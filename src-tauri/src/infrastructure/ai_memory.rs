//! The `ai-memory` server on this machine (ADR 0040), read over plain HTTP on a loopback address.
//!
//! Only search is used. The access token is read from the server's own data directory on every
//! call and goes nowhere but the `Authorization` header of a request to `127.0.0.1`, `localhost`
//! or `[::1]`: the settings are checked again here, so a hand-edited config cannot point it at
//! another machine. No new dependency: a search is one small request.

use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::application::memory::external::{
    is_atlas_page, ExternalError, ExternalHit, ExternalMemory, ExternalScope, Page,
};
use crate::domain::memory::ExternalMemorySettings;

const CONNECT_TIMEOUT: Duration = Duration::from_millis(800);
const IO_TIMEOUT: Duration = Duration::from_millis(2000);
/// One search or write, from the first connection attempt to the last byte.
const TOTAL_DEADLINE: Duration = Duration::from_millis(3000);
/// A search answer is a few notes; anything bigger is not what was asked for.
const MAX_RESPONSE_BYTES: u64 = 256 * 1024;

#[derive(Default)]
pub struct AiMemoryHttp;

#[derive(Deserialize)]
struct RawHit {
    id: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    snippet: String,
}

impl ExternalMemory for AiMemoryHttp {
    fn search(
        &self,
        settings: &ExternalMemorySettings,
        scope: &ExternalScope,
        query: &str,
        limit: u8,
    ) -> Result<Vec<ExternalHit>, ExternalError> {
        let authority = settings
            .loopback_authority()
            .ok_or(ExternalError::Address)?;
        let token = read_token(&settings.data_dir)?;
        let path = format!(
            "/admin/search?q={}&workspace={}&project={}&limit={limit}",
            encode(query),
            encode(&scope.workspace),
            encode(&scope.project),
        );
        // A project the server has never heard of has no notes: that is an answer, not a failure.
        // A project the server has never heard of has no notes: that is an answer, not a failure.
        let body = request("GET", &authority, &path, &token, None, Some("[]"))?;
        let raw: Vec<RawHit> =
            serde_json::from_str(&body).map_err(|_| ExternalError::Unreadable)?;
        Ok(raw
            .into_iter()
            .filter(|h| {
                // An id is a short plain token; anything else is not a page this feature made.
                h.id.len() <= 64
                    && !h.id.is_empty()
                    && h.id
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            })
            .map(|h| ExternalHit {
                id: h.id,
                path: h.path,
                title: h.title,
                snippet: h.snippet,
            })
            .collect())
    }

    fn publish(
        &self,
        settings: &ExternalMemorySettings,
        scope: &ExternalScope,
        page: &Page,
    ) -> Result<(), ExternalError> {
        let authority = settings
            .loopback_authority()
            .ok_or(ExternalError::Address)?;
        let token = read_token(&settings.data_dir)?;
        // Only Atlas's own pages are ever written, so a person's page cannot be overwritten.
        if !is_atlas_page(&page.path) {
            return Err(ExternalError::OnlyAtlasPages);
        }
        let body = serde_json::json!({
            "workspace": scope.workspace,
            "project": scope.project,
            "path": page.path,
            "body": page.body,
            "title": page.title,
            "kind": page.kind,
            "tier": "semantic",
            "tags": page.tags,
            "pinned": false,
        });
        request(
            "POST",
            &authority,
            "/admin/write-page",
            &token,
            Some(&body.to_string()),
            None,
        )
        .map(|_| ())
    }

    fn retire(
        &self,
        settings: &ExternalMemorySettings,
        scope: &ExternalScope,
        path: &str,
    ) -> Result<(), ExternalError> {
        let authority = settings
            .loopback_authority()
            .ok_or(ExternalError::Address)?;
        let token = read_token(&settings.data_dir)?;
        // Only a page Atlas published is ever removed: the path is checked here too.
        if !is_atlas_page(path) {
            return Err(ExternalError::OnlyAtlasPages);
        }
        let body = serde_json::json!({
            "workspace": scope.workspace,
            "project": scope.project,
            "path": path,
        });
        request(
            "POST",
            &authority,
            "/admin/delete-page",
            &token,
            Some(&body.to_string()),
            // Nothing to retire in a project the server does not know.
            Some("{}"),
        )
        .map(|_| ())
    }
}

fn read_token(data_dir: &str) -> Result<String, ExternalError> {
    let dir = Path::new(data_dir);
    if data_dir.is_empty() || !dir.is_absolute() {
        return Err(ExternalError::DataDir);
    }
    let token =
        std::fs::read_to_string(dir.join("auth-token")).map_err(|_| ExternalError::Token)?;
    let token = token.trim();
    // A token is a plain word; anything else would let a file smuggle a second header.
    if token.is_empty()
        || !token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
    {
        return Err(ExternalError::Token);
    }
    Ok(token.to_owned())
}

/// Percent-encoding of everything but the unreserved characters.
fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

fn request(
    method: &str,
    authority: &str,
    path: &str,
    token: &str,
    body: Option<&str>,
    when_missing: Option<&str>,
) -> Result<String, ExternalError> {
    let started = Instant::now();
    // Every address the name resolves to is tried (`localhost` may give `::1` first), and only
    // loopback ones are ever used.
    let addrs: Vec<_> = authority
        .to_socket_addrs()
        .map_err(|_| ExternalError::Address)?
        .filter(|a| a.ip().is_loopback())
        .collect();
    if addrs.is_empty() {
        return Err(ExternalError::Address);
    }
    let mut stream = addrs
        .iter()
        .find_map(|addr| TcpStream::connect_timeout(addr, CONNECT_TIMEOUT).ok())
        .ok_or(ExternalError::NotRunning)?;
    // HTTP/1.0: the answer is not chunked, and the connection ends with it.
    let mut head = format!(
        "{method} {path} HTTP/1.0\r\nHost: {authority}\r\nAuthorization: Bearer {token}\r\nAccept: application/json\r\n"
    );
    let payload = body.unwrap_or_default();
    if body.is_some() {
        let _ = write!(
            head,
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            payload.len()
        );
    }
    head.push_str("\r\n");
    // One write: the head and the body leave together.
    head.push_str(payload);
    let slow = || ExternalError::Timeout;
    stream
        .set_write_timeout(Some(IO_TIMEOUT))
        .map_err(|_| slow())?;
    stream
        .write_all(head.as_bytes())
        .map_err(|_| ExternalError::NotRunning)?;
    // The whole exchange has one deadline: a server that answers a byte at a time cannot hold the
    // caller for longer than it.
    let mut raw = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let left = TOTAL_DEADLINE
            .checked_sub(started.elapsed())
            .filter(|d| !d.is_zero())
            .ok_or_else(slow)?;
        stream.set_read_timeout(Some(left)).map_err(|_| slow())?;
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                raw.extend_from_slice(&chunk[..n]);
                if raw.len() as u64 > MAX_RESPONSE_BYTES {
                    return Err(ExternalError::TooLarge);
                }
            }
            Err(_) => return Err(slow()),
        }
    }
    let text = String::from_utf8_lossy(&raw);
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or(ExternalError::Unreadable)?;
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .unwrap_or_default();
    match status {
        "200" => Ok(body.to_owned()),
        // A 404 is "this project has no notes" only when the server says so in its own words; a
        // 404 from anything else (a wrong port, an older server without the route) is an error, so
        // `check` does not call it reachable and a page is not forgotten while it is still there.
        "404" if when_missing.is_some() && body.to_lowercase().contains("not found") => {
            Ok(when_missing.unwrap_or_default().to_owned())
        }
        "401" | "403" => Err(ExternalError::Refused),
        other => Err(ExternalError::Server(other.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    /// A one-shot server that records the request and answers `status` with `body`.
    fn serve(status: &str, body: &str) -> (u16, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let reply = format!("HTTP/1.0 {status}\r\nContent-Type: application/json\r\n\r\n{body}");
        let handle = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            // Read the whole request: the head, then as many body bytes as it announces.
            let mut received = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                let n = socket.read(&mut buf).unwrap();
                received.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&received).into_owned();
                if let Some((head, body)) = text.split_once("\r\n\r\n") {
                    let wanted = head
                        .lines()
                        .find_map(|l| l.strip_prefix("Content-Length: "))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if body.len() >= wanted {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            socket.write_all(reply.as_bytes()).unwrap();
            String::from_utf8_lossy(&received).into_owned()
        });
        (port, handle)
    }

    fn settings(port: u16, dir: &Path) -> ExternalMemorySettings {
        ExternalMemorySettings {
            enabled: true,
            server_url: format!("http://127.0.0.1:{port}"),
            data_dir: dir.to_string_lossy().into_owned(),
            ..ExternalMemorySettings::default()
        }
    }

    fn scope() -> ExternalScope {
        ExternalScope {
            workspace: "ws".to_owned(),
            project: "app".to_owned(),
        }
    }

    struct DataDir(std::path::PathBuf);

    impl DataDir {
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for DataDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn data_dir(token: &str) -> DataDir {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("atlas-aimem-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("auth-token"), format!("{token}\n")).unwrap();
        DataDir(dir)
    }

    #[test]
    fn it_searches_with_the_token_in_the_header_only() {
        let dir = data_dir("tok-123");
        let (port, request) = serve(
            "200 OK",
            r#"[{"id":"p1","path":"n.md","title":"T","snippet":"S <mark>x</mark>","rank":-1.0}]"#,
        );

        let hits = AiMemoryHttp
            .search(&settings(port, dir.path()), &scope(), "retry OR limit", 3)
            .unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "p1");
        let sent = request.join().unwrap();
        assert!(sent.starts_with(
            "GET /admin/search?q=retry%20OR%20limit&workspace=ws&project=app&limit=3 HTTP/1.0"
        ));
        assert!(sent.contains("Authorization: Bearer tok-123"));
        assert!(!sent.lines().next().unwrap().contains("tok-123"));
    }

    #[test]
    fn a_server_that_is_not_on_this_machine_is_never_contacted() {
        let dir = data_dir("tok");
        let mut s = settings(49374, dir.path());
        s.server_url = "http://example.com:49374".to_owned();

        let err = AiMemoryHttp.search(&s, &scope(), "q", 3).unwrap_err();

        assert_eq!(err, ExternalError::Address);
    }

    #[test]
    fn a_refused_token_and_a_missing_server_are_reported_not_raised() {
        let dir = data_dir("tok");
        let (port, _request) = serve("401 Unauthorized", "{}");
        let err = AiMemoryHttp
            .search(&settings(port, dir.path()), &scope(), "q", 3)
            .unwrap_err();
        assert_eq!(err, ExternalError::Refused);

        // nothing listens on this port any more
        let free = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = free.local_addr().unwrap().port();
        drop(free);
        let err = AiMemoryHttp
            .search(&settings(port, dir.path()), &scope(), "q", 3)
            .unwrap_err();
        assert_eq!(err, ExternalError::NotRunning);
    }

    #[test]
    fn a_token_that_could_add_a_header_is_not_used() {
        let dir = data_dir("tok\r\nX-Evil: 1");
        assert!(read_token(&dir.path().to_string_lossy()).is_err());
        assert!(read_token("relative/dir").is_err());
        assert!(read_token("").is_err());
    }
    fn page() -> Page {
        Page {
            path: "atlas/m1.md".to_owned(),
            title: "Retry limit".to_owned(),
            body: "It is \"MAXTRY\" in deploy/limits.cfg".to_owned(),
            kind: "gotcha",
            tags: vec!["atlas".to_owned(), "pitfall".to_owned()],
        }
    }

    #[test]
    fn publishing_posts_the_page_as_json_with_the_token_in_the_header_only() {
        let dir = data_dir("tok-123");
        let (port, request) = serve("200 OK", r#"{"page_id":"p","path":"atlas/m1.md"}"#);

        AiMemoryHttp
            .publish(&settings(port, dir.path()), &scope(), &page())
            .unwrap();

        let sent = request.join().unwrap();
        assert!(sent.starts_with("POST /admin/write-page HTTP/1.0"));
        assert!(sent.contains("Authorization: Bearer tok-123"));
        assert!(sent.contains("Content-Type: application/json"));
        let (head, body) = sent.split_once("\r\n\r\n").unwrap();
        assert!(head.contains(&format!("Content-Length: {}", body.len())));
        let json: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!(json["workspace"], "ws");
        assert_eq!(json["project"], "app");
        assert_eq!(json["path"], "atlas/m1.md");
        assert_eq!(json["kind"], "gotcha");
        assert_eq!(json["body"], "It is \"MAXTRY\" in deploy/limits.cfg");
        assert!(!body.contains("tok-123"));
    }

    #[test]
    fn retiring_posts_a_delete_and_never_touches_a_page_that_is_not_atlass() {
        let dir = data_dir("tok");
        let (port, request) = serve("200 OK", r#"{"path":"atlas/m1.md","deleted":true}"#);
        let s = settings(port, dir.path());

        AiMemoryHttp.retire(&s, &scope(), "atlas/m1.md").unwrap();
        let sent = request.join().unwrap();
        assert!(sent.starts_with("POST /admin/delete-page HTTP/1.0"));

        // Refused before any connection is made.
        for path in ["notes/other.md", "atlas/../notes/other.md", "../atlas/x.md"] {
            assert!(AiMemoryHttp.retire(&s, &scope(), path).is_err(), "{path}");
        }
    }

    /// The whole write path against a real `ai-memory` server (no model, no cost): publish a page,
    /// find it again, retire it, and see it gone. Needs `ATLAS_AIMEM_DATA` (its data folder) and
    /// the server running at `ATLAS_AIMEM_URL` (default `http://127.0.0.1:49391`).
    #[test]
    #[ignore = "needs a running ai-memory server (ATLAS_AIMEM_DATA); calls no model"]
    fn real_server_publish_find_and_retire() {
        let data = std::env::var("ATLAS_AIMEM_DATA").expect("ATLAS_AIMEM_DATA");
        let url = std::env::var("ATLAS_AIMEM_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:49391".to_owned());
        let s = ExternalMemorySettings {
            enabled: true,
            server_url: url,
            data_dir: data,
            ..ExternalMemorySettings::default()
        };
        let scope = ExternalScope {
            workspace: "default".to_owned(),
            project: "atlas-roundtrip".to_owned(),
        };
        let page = Page {
            path: "atlas/roundtrip-1.md".to_owned(),
            title: "Roundtrip pitfall".to_owned(),
            body: "Zanzibar quokka limit lives in deploy/limits.cfg".to_owned(),
            kind: "gotcha",
            tags: vec!["atlas".to_owned()],
        };

        AiMemoryHttp.publish(&s, &scope, &page).unwrap();
        let found = AiMemoryHttp
            .search(&s, &scope, "zanzibar OR quokka", 3)
            .unwrap();
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].path, "atlas/roundtrip-1.md");
        assert!(found[0].snippet.contains("deploy/limits.cfg"));

        AiMemoryHttp.retire(&s, &scope, &page.path).unwrap();
        let after = AiMemoryHttp
            .search(&s, &scope, "zanzibar OR quokka", 3)
            .unwrap();
        assert!(after.is_empty(), "the page is still found: {after:?}");
        // Retiring what is not there is not an error.
        AiMemoryHttp.retire(&s, &scope, &page.path).unwrap();
    }

    #[test]
    fn a_project_the_server_does_not_know_has_no_notes_and_nothing_to_retire() {
        let dir = data_dir("tok");
        let (port, _) = serve("404 Not Found", r#"{"error":"project 'app' not found"}"#);
        let s = settings(port, dir.path());
        assert_eq!(AiMemoryHttp.search(&s, &scope(), "q", 3).unwrap(), []);

        let (port, _) = serve(
            "404 Not Found",
            r#"{"error":"project 'app' not found in workspace 'ws'"}"#,
        );
        let s = settings(port, dir.path());
        AiMemoryHttp.retire(&s, &scope(), "atlas/m1.md").unwrap();
    }

    #[test]
    fn a_404_from_something_that_is_not_the_servers_own_answer_is_an_error() {
        let dir = data_dir("tok");
        for body in ["<html>404 page</html>", "{}", ""] {
            let (port, _) = serve("404 Not Found", body);
            let s = settings(port, dir.path());
            assert_eq!(
                AiMemoryHttp.search(&s, &scope(), "q", 3).unwrap_err(),
                ExternalError::Server("404".to_owned()),
                "{body:?}"
            );
            let (port, _) = serve("404 Not Found", body);
            let s = settings(port, dir.path());
            assert_eq!(
                AiMemoryHttp
                    .retire(&s, &scope(), "atlas/m1.md")
                    .unwrap_err(),
                ExternalError::Server("404".to_owned()),
                "{body:?}"
            );
        }
    }

    #[test]
    fn a_server_that_drips_bytes_cannot_hold_the_caller_past_the_deadline() {
        let dir = data_dir("tok");
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let drip = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut buf = [0u8; 1024];
            let _ = socket.read(&mut buf);
            // A byte every half second for much longer than the deadline.
            for byte in b"HTTP/1.0 200 OK\r\n\r\n[]          " {
                if socket.write_all(&[*byte]).is_err() {
                    return;
                }
                thread::sleep(Duration::from_millis(500));
            }
        });
        let started = Instant::now();

        let err = AiMemoryHttp
            .search(&settings(port, dir.path()), &scope(), "q", 3)
            .unwrap_err();

        assert_eq!(err, ExternalError::Timeout);
        assert!(
            started.elapsed() < Duration::from_millis(4500),
            "{:?}",
            started.elapsed()
        );
        drop(drip);
    }
}
