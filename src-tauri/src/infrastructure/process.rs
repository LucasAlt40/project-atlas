use std::env;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::application::process::{
    ProcessError, ProcessEvent, ProcessOutput, ProcessRunner, ProcessSpec,
};

/// Output beyond this is drained and dropped, so a chatty tool cannot exhaust memory.
const MAX_OUTPUT_BYTES: u64 = 8 * 1024 * 1024;
const POLL_INTERVAL: Duration = Duration::from_millis(20);
/// How long to wait for the input writer when output arrives before it reports being done.
const INPUT_GRACE: Duration = Duration::from_millis(50);

/// Runs real child processes. Executables are looked up on `PATH` plus the usual install
/// directories of developer tools, because an app launched from the desktop does not
/// inherit the PATH of the user's shell.
pub struct SystemProcessRunner {
    search_dirs: Vec<PathBuf>,
}

impl SystemProcessRunner {
    pub fn new() -> Self {
        let mut dirs: Vec<PathBuf> = env::var_os("PATH")
            .map(|p| env::split_paths(&p).collect())
            .unwrap_or_default();
        if let Some(home) = env::var_os("HOME").or_else(|| env::var_os("USERPROFILE")) {
            let home = PathBuf::from(home);
            for relative in [
                ".opencode/bin",
                ".local/bin",
                ".npm-global/bin",
                ".cargo/bin",
                ".bun/bin",
            ] {
                dirs.push(home.join(relative));
            }
        }
        if let Some(appdata) = env::var_os("APPDATA") {
            // npm's global bin directory on Windows, where `.cmd` shims live.
            dirs.push(PathBuf::from(appdata).join("npm"));
        }
        if cfg!(unix) {
            // Homebrew and the traditional prefix: desktop-launched apps do not inherit them.
            for fixed in ["/opt/homebrew/bin", "/usr/local/bin"] {
                dirs.push(PathBuf::from(fixed));
            }
        }
        Self { search_dirs: dirs }
    }

    fn path_for_children(&self) -> OsString {
        env::join_paths(&self.search_dirs).unwrap_or_default()
    }
}

impl Default for SystemProcessRunner {
    fn default() -> Self {
        Self::new()
    }
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Reads a stream line by line, sending each line (without its ending) to `lines` as soon as
/// it arrives. Returns the whole output, capped like [`read_capped`].
fn read_lines(stream: impl Read, lines: &mpsc::Sender<String>) -> Vec<u8> {
    let mut reader = BufReader::new(stream);
    let mut all = Vec::new();
    let mut line = Vec::new();
    loop {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                if (all.len() as u64) < MAX_OUTPUT_BYTES {
                    all.extend_from_slice(&line);
                }
                let text = String::from_utf8_lossy(&line);
                // A receiver that is gone just means nobody is listening any more.
                let _ = lines.send(text.trim_end_matches(['\r', '\n']).to_owned());
            }
        }
    }
    all
}

fn read_capped(mut stream: impl Read) -> Vec<u8> {
    let mut buffer = Vec::new();
    let _ = (&mut stream)
        .take(MAX_OUTPUT_BYTES)
        .read_to_end(&mut buffer);
    let _ = std::io::copy(&mut stream, &mut std::io::sink());
    buffer
}

impl ProcessRunner for SystemProcessRunner {
    fn locate(&self, program: &str) -> Option<PathBuf> {
        let names: &[&str] = if cfg!(windows) {
            &[".exe", ".cmd", ".bat", ""]
        } else {
            &[""]
        };
        self.search_dirs
            .iter()
            .flat_map(|dir| {
                names
                    .iter()
                    .map(move |ext| dir.join(format!("{program}{ext}")))
            })
            .find(|candidate| is_executable(candidate))
    }

    fn run(
        &self,
        spec: &ProcessSpec,
        on_event: &dyn Fn(ProcessEvent),
    ) -> Result<ProcessOutput, ProcessError> {
        let executable = self.locate(&spec.program).ok_or(ProcessError::NotFound)?;
        let mut command = Command::new(executable);
        command
            .args(&spec.args)
            .env("PATH", self.path_for_children())
            .env("NO_COLOR", "1")
            .stdin(if spec.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(cwd) = &spec.cwd {
            // `current_dir` does not update the inherited `PWD`, and tools that trust `PWD`
            // (OpenCode does) would see the project as outside their working directory.
            command.current_dir(cwd).env("PWD", cwd);
        }
        let mut child = command
            .spawn()
            .map_err(|error| ProcessError::Spawn(error.to_string()))?;
        on_event(ProcessEvent::Spawned);

        let (line_tx, line_rx) = mpsc::channel();
        let stdout = child
            .stdout
            .take()
            .map(|s| thread::spawn(move || read_lines(s, &line_tx)));
        let stderr = child
            .stderr
            .take()
            .map(|s| thread::spawn(move || read_capped(s)));

        // Written on a thread: a prompt larger than the pipe buffer must not block the
        // timeout loop below.
        let (written_tx, written_rx) = mpsc::channel();
        match (spec.stdin.clone(), child.stdin.take()) {
            (Some(input), Some(mut pipe)) => {
                thread::spawn(move || {
                    // A tool that exits early closes the pipe; its exit status tells the story.
                    let _ = pipe.write_all(input.as_bytes());
                    drop(pipe);
                    let _ = written_tx.send(());
                });
            }
            _ => {
                let _ = written_tx.send(());
            }
        }

        let deadline = Instant::now() + spec.timeout;
        let mut input_reported = false;
        let status = loop {
            if !input_reported && written_rx.try_recv().is_ok() {
                input_reported = true;
                on_event(ProcessEvent::InputSent);
            }
            // Waiting on the line channel wakes us the moment output arrives; when the
            // channel is closed (stdout ended) fall back to plain polling.
            match line_rx.recv_timeout(POLL_INTERVAL) {
                Ok(line) => {
                    // Output means the tool has its input (or does not need it): report that
                    // first, so listeners see the events in order.
                    if !input_reported {
                        let _ = written_rx.recv_timeout(INPUT_GRACE);
                        input_reported = true;
                        on_event(ProcessEvent::InputSent);
                    }
                    on_event(ProcessEvent::StdoutLine(line));
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => thread::sleep(POLL_INTERVAL),
            }
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() >= deadline => {
                    kill(&mut child);
                    return Err(ProcessError::Timeout);
                }
                Ok(None) => {}
                Err(error) => {
                    kill(&mut child);
                    return Err(ProcessError::Io(error.to_string()));
                }
            }
        };

        let collect = |handle: Option<thread::JoinHandle<Vec<u8>>>| {
            handle.and_then(|h| h.join().ok()).unwrap_or_default()
        };
        let stdout = collect(stdout);
        if !input_reported {
            on_event(ProcessEvent::InputSent);
        }
        // The reader has finished: deliver any lines still queued.
        for line in line_rx.try_iter() {
            on_event(ProcessEvent::StdoutLine(line));
        }
        Ok(ProcessOutput {
            exit_code: status.code(),
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&collect(stderr)).into_owned(),
        })
    }
}

fn kill(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(all(test, unix))]
mod tests {
    use std::cell::RefCell;

    use super::*;

    fn spec(program: &str, args: &[&str], stdin: Option<&str>, timeout: Duration) -> ProcessSpec {
        ProcessSpec {
            program: program.to_owned(),
            args: args.iter().map(|a| (*a).to_owned()).collect(),
            stdin: stdin.map(str::to_owned),
            cwd: None,
            timeout,
        }
    }

    #[test]
    fn locates_programs_and_reports_missing_ones() {
        let runner = SystemProcessRunner::new();

        assert!(runner.locate("sh").is_some());
        assert!(runner.locate("definitely-not-a-real-program").is_none());
        assert_eq!(
            runner.run(
                &spec(
                    "definitely-not-a-real-program",
                    &[],
                    None,
                    Duration::from_secs(1)
                ),
                &|_| {}
            ),
            Err(ProcessError::NotFound)
        );
    }

    #[test]
    fn feeds_stdin_captures_output_and_reports_phases() {
        let events = RefCell::new(Vec::new());

        let output = SystemProcessRunner::new()
            .run(
                &spec(
                    "sh",
                    &["-c", "cat; echo err >&2; exit 3"],
                    Some("hello"),
                    Duration::from_secs(5),
                ),
                &|e| events.borrow_mut().push(e),
            )
            .unwrap();

        assert_eq!(output.exit_code, Some(3));
        assert_eq!(output.stdout, "hello");
        assert_eq!(output.stderr.trim(), "err");
        assert_eq!(
            *events.borrow(),
            [
                ProcessEvent::Spawned,
                ProcessEvent::InputSent,
                ProcessEvent::StdoutLine("hello".to_owned()),
            ]
        );
    }

    #[test]
    fn delivers_stdout_lines_while_the_process_is_still_running() {
        let seen = RefCell::new(Vec::new());

        let output = SystemProcessRunner::new()
            .run(
                &spec(
                    "sh",
                    &["-c", "echo one; sleep 0.4; echo two"],
                    None,
                    Duration::from_secs(5),
                ),
                &|event| {
                    if let ProcessEvent::StdoutLine(line) = event {
                        seen.borrow_mut().push((line, Instant::now()));
                    }
                },
            )
            .unwrap();

        let finished = Instant::now();
        let lines: Vec<_> = seen.borrow().iter().map(|(l, _)| l.clone()).collect();
        assert_eq!(lines, ["one", "two"]);
        assert_eq!(output.stdout, "one\ntwo\n");
        // "one" was delivered long before the process ended, not after it.
        let first = seen.borrow()[0].1;
        assert!(finished.duration_since(first) >= Duration::from_millis(300));
    }

    #[test]
    fn the_working_directory_is_also_what_the_tool_sees_in_pwd() {
        let dir = std::fs::canonicalize(std::env::temp_dir()).unwrap();
        let mut spec = spec(
            "sh",
            &["-c", "pwd -P; echo \"$PWD\""],
            None,
            Duration::from_secs(5),
        );
        spec.cwd = Some(dir.clone());

        let output = SystemProcessRunner::new().run(&spec, &|_| {}).unwrap();

        let lines: Vec<_> = output.stdout.lines().collect();
        assert_eq!(lines, [dir.to_str().unwrap(), dir.to_str().unwrap()]);
    }

    #[test]
    fn kills_processes_that_exceed_the_timeout() {
        let started = Instant::now();

        let result = SystemProcessRunner::new().run(
            &spec("sh", &["-c", "sleep 30"], None, Duration::from_millis(100)),
            &|_| {},
        );

        assert_eq!(result, Err(ProcessError::Timeout));
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
