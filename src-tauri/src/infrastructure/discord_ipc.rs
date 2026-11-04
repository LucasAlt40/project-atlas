//! Discord's local IPC, the way Rich Presence talks to the desktop client: a Unix socket
//! (`discord-ipc-N`) on macOS and Linux, a named pipe on Windows. Frames are
//! `op: u32 LE | length: u32 LE | JSON`. No OAuth: `SET_ACTIVITY` only needs the application id.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Mutex;
#[cfg(unix)]
use std::time::Duration;

use serde_json::{json, Value};

use crate::application::presence::{PresenceTransport, Published};
use crate::application::support::LockExt;
use crate::domain::presence::Activity;

const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;
const OP_CLOSE: u32 = 2;
/// Discord rejects lines shorter than 2 or longer than 128 characters.
const MAX_LINE: usize = 128;
#[cfg(unix)]
const IO_TIMEOUT: Duration = Duration::from_secs(2);

trait Stream: Read + Write + Send {}
impl<T: Read + Write + Send> Stream for T {}

struct Connection {
    client_id: String,
    stream: Box<dyn Stream>,
    nonce: u64,
}

pub struct DiscordIpc {
    /// Where the socket may be, in order. Tests point it at their own.
    candidates: Vec<PathBuf>,
    connection: Mutex<Option<Connection>>,
}

impl DiscordIpc {
    pub fn system() -> Self {
        Self::with_candidates(system_candidates())
    }

    pub fn with_candidates(candidates: Vec<PathBuf>) -> Self {
        Self {
            candidates,
            connection: Mutex::new(None),
        }
    }

    fn connect(&self, client_id: &str) -> Result<Connection, String> {
        let mut last_error = "Discord is not running".to_owned();
        for path in &self.candidates {
            match open(path) {
                Ok(stream) => {
                    let mut connection = Connection {
                        client_id: client_id.to_owned(),
                        stream,
                        nonce: 0,
                    };
                    match handshake(&mut connection) {
                        Ok(()) => return Ok(connection),
                        Err(error) => last_error = error,
                    }
                }
                Err(error) => {
                    // Only the failures that say something are kept: "not there" is the usual one.
                    if error.kind() != std::io::ErrorKind::NotFound {
                        last_error = format!("{}: {error}", path.display());
                    }
                }
            }
        }
        Err(last_error)
    }
}

impl PresenceTransport for DiscordIpc {
    fn publish(&self, published: Option<&Published>) -> Result<(), String> {
        let mut guard = self.connection.lock_or_recover();
        // A connection belongs to one application id.
        if let (Some(connection), Some(published)) = (guard.as_ref(), published) {
            if connection.client_id != published.client_id {
                *guard = None;
            }
        }
        if guard.is_none() {
            let Some(published) = published else {
                // Nothing to take down: Discord was never reached.
                return Ok(());
            };
            *guard = Some(self.connect(&published.client_id)?);
        }
        let connection = guard.as_mut().ok_or("no connection")?;
        let result = send_activity(connection, published.map(|p| &p.activity));
        if result.is_err() {
            // Discord was closed or restarted: the next call connects again.
            *guard = None;
        }
        result
    }
}

fn handshake(connection: &mut Connection) -> Result<(), String> {
    let hello = json!({ "v": 1, "client_id": connection.client_id });
    write_frame(connection.stream.as_mut(), OP_HANDSHAKE, &hello)?;
    let (op, reply) = read_frame(connection.stream.as_mut())?;
    if op == OP_CLOSE || reply.get("evt").and_then(Value::as_str) == Some("ERROR") {
        let message = reply
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Discord refused the application id");
        return Err(message.to_owned());
    }
    Ok(())
}

fn send_activity(connection: &mut Connection, activity: Option<&Activity>) -> Result<(), String> {
    connection.nonce += 1;
    let mut args = json!({ "pid": std::process::id() });
    if let Some(activity) = activity {
        args["activity"] = activity_json(activity);
    }
    let frame = json!({
        "cmd": "SET_ACTIVITY",
        "args": args,
        "nonce": connection.nonce.to_string(),
    });
    write_frame(connection.stream.as_mut(), OP_FRAME, &frame)?;
    let (_, reply) = read_frame(connection.stream.as_mut())?;
    if reply.get("evt").and_then(Value::as_str) == Some("ERROR") {
        let message = reply
            .pointer("/data/message")
            .and_then(Value::as_str)
            .unwrap_or("Discord refused the activity");
        return Err(message.to_owned());
    }
    Ok(())
}

fn activity_json(activity: &Activity) -> Value {
    let mut value = json!({
        "details": clip(&activity.details),
        "state": clip(&activity.state),
    });
    if let Some(start) = activity.started_at {
        value["timestamps"] = json!({ "start": start });
    }
    let mut assets = json!({ "large_image": "atlas" });
    if let Some(text) = &activity.large_text {
        assets["large_text"] = json!(clip(text));
    }
    if let Some(image) = activity.small_image {
        assets["small_image"] = json!(image);
    }
    if let Some(text) = &activity.small_text {
        assets["small_text"] = json!(clip(text));
    }
    value["assets"] = assets;
    value
}

/// At most 128 characters, and at least 2 (Discord drops shorter lines).
fn clip(text: &str) -> String {
    let mut clipped: String = text.chars().take(MAX_LINE).collect();
    while clipped.chars().count() < 2 {
        clipped.push(' ');
    }
    clipped
}

fn write_frame<W: Write + ?Sized>(stream: &mut W, op: u32, payload: &Value) -> Result<(), String> {
    let body = serde_json::to_vec(payload).map_err(|e| e.to_string())?;
    let length = u32::try_from(body.len()).map_err(|e| e.to_string())?;
    let mut frame = Vec::with_capacity(8 + body.len());
    frame.extend_from_slice(&op.to_le_bytes());
    frame.extend_from_slice(&length.to_le_bytes());
    frame.extend_from_slice(&body);
    stream.write_all(&frame).map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())
}

fn read_frame<R: Read + ?Sized>(stream: &mut R) -> Result<(u32, Value), String> {
    let mut header = [0_u8; 8];
    stream.read_exact(&mut header).map_err(|e| e.to_string())?;
    let op = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
    let length = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
    // Discord's frames are tiny; a huge length means this is not Discord talking.
    if length > 64 * 1024 {
        return Err("unexpected frame from Discord".to_owned());
    }
    let mut body = vec![0_u8; length];
    stream.read_exact(&mut body).map_err(|e| e.to_string())?;
    let value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    Ok((op, value))
}

#[cfg(unix)]
fn open(path: &std::path::Path) -> std::io::Result<Box<dyn Stream>> {
    let stream = std::os::unix::net::UnixStream::connect(path)?;
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    Ok(Box::new(stream))
}

/// Named pipes open like files. Untested on Windows: there is no read timeout here.
#[cfg(windows)]
fn open(path: &std::path::Path) -> std::io::Result<Box<dyn Stream>> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?;
    Ok(Box::new(file))
}

fn system_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for slot in 0..10 {
        let name = format!("discord-ipc-{slot}");
        if cfg!(windows) {
            candidates.push(PathBuf::from(format!(r"\\.\pipe\{name}")));
            continue;
        }
        let mut dirs: Vec<PathBuf> = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]
            .iter()
            .filter_map(std::env::var_os)
            .map(PathBuf::from)
            .collect();
        dirs.push(PathBuf::from("/tmp"));
        for dir in dirs {
            candidates.push(dir.join(&name));
            // Flatpak and Snap keep it in their own folders.
            candidates.push(dir.join("app/com.discordapp.Discord").join(&name));
            candidates.push(dir.join("snap.discord").join(&name));
        }
    }
    candidates.dedup();
    candidates
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::net::UnixListener;
    use std::sync::mpsc;
    use std::thread;

    use super::*;

    fn read_op(stream: &mut std::os::unix::net::UnixStream) -> (u32, Value) {
        read_frame(stream).unwrap()
    }

    fn activity() -> Activity {
        Activity {
            details: "Workflow: Ship it".to_owned(),
            state: "x".to_owned(),
            started_at: Some(1_700_000_000),
            large_text: Some("Workspace: Billing".to_owned()),
            small_image: Some("running"),
            small_text: Some("Running".to_owned()),
        }
    }

    #[test]
    fn speaks_the_discord_protocol_and_reconnects() {
        let dir = std::env::temp_dir().join(format!("atlas-ipc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let socket = dir.join("discord-ipc-0");
        let _ = std::fs::remove_file(&socket);
        let listener = UnixListener::bind(&socket).unwrap();
        let (seen, inbox) = mpsc::channel();
        // A fake Discord: answers READY to the handshake and OK to each frame, twice over (the
        // client drops the first connection to prove it comes back).
        let server = thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let (op, hello) = read_op(&mut stream);
                seen.send((op, hello)).unwrap();
                let ready = json!({ "cmd": "DISPATCH", "evt": "READY" });
                write_frame(&mut stream, OP_FRAME, &ready).unwrap();
                let (op, frame) = read_op(&mut stream);
                seen.send((op, frame)).unwrap();
                let ok = json!({ "cmd": "SET_ACTIVITY", "evt": null });
                write_frame(&mut stream, OP_FRAME, &ok).unwrap();
                // Dropping the stream closes the socket.
            }
        });
        let ipc = DiscordIpc::with_candidates(vec![dir.join("discord-ipc-9"), socket]);
        let published = Published {
            client_id: "42".to_owned(),
            activity: activity(),
        };

        ipc.publish(Some(&published)).unwrap();
        let (op, hello) = inbox.recv().unwrap();
        assert_eq!(
            (op, hello["client_id"].as_str(), hello["v"].as_i64()),
            (0, Some("42"), Some(1))
        );
        let (op, frame) = inbox.recv().unwrap();
        assert_eq!(op, 1);
        assert_eq!(frame["cmd"], "SET_ACTIVITY");
        assert_eq!(frame["args"]["pid"], std::process::id());
        let shown = &frame["args"]["activity"];
        assert_eq!(shown["details"], "Workflow: Ship it");
        assert_eq!(
            shown["state"], "x ",
            "lines shorter than two characters are padded"
        );
        assert_eq!(shown["timestamps"]["start"], 1_700_000_000_u64);
        assert_eq!(shown["assets"]["large_image"], "atlas");
        assert_eq!(shown["assets"]["small_image"], "running");

        // The fake closed the socket: the next send fails, and the one after connects again.
        thread::sleep(Duration::from_millis(50));
        assert!(ipc.publish(Some(&published)).is_err());
        ipc.publish(Some(&published)).unwrap();
        let _ = inbox.recv().unwrap();
        let (_, frame) = inbox.recv().unwrap();
        assert_eq!(frame["args"]["activity"]["details"], "Workflow: Ship it");
        server.join().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn clearing_without_a_connection_is_a_no_op_and_a_missing_discord_is_an_error() {
        let ipc = DiscordIpc::with_candidates(vec![PathBuf::from("/nonexistent/discord-ipc-0")]);
        assert!(ipc.publish(None).is_ok());
        let published = Published {
            client_id: "42".to_owned(),
            activity: activity(),
        };

        assert_eq!(
            ipc.publish(Some(&published)).unwrap_err(),
            "Discord is not running"
        );
    }

    #[test]
    fn long_lines_are_cut() {
        assert_eq!(clip(&"a".repeat(300)).chars().count(), 128);
    }
}
