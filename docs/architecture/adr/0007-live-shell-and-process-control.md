# ADR 0007 — Live shell and process control

**Status:** accepted

## What it is

An execution that runs a CLI runtime (Claude, OpenCode) now runs attached to a **real pseudo-terminal**. While its process
lives it has a **process session** (`processSessionId`); the UI can show the process's real output (xterm.js), interrupt it
(Ctrl+C), terminate it, and — only if the runtime reads input — type into it. Chat is what the agent _says_; the terminal is
what its process _does_; the two are never mixed.

```
Workspace → Agent → Execution → ProcessSession
ExecutionService → RuntimeRegistry → ModelRuntime → ProcessRunner (guard → system) → PTY → claude | opencode
```

It is **observability and process control, not a sandbox.** Security still comes from the `PermissionPolicy`, the runtime's
own restrictions and, later, an OS sandbox (ADR 0006). The live shell controls a process Atlas already authorized; it cannot
start one.

## Pieces

| Piece                                                           | Responsibility                                                                                                                   |
| --------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `application/process.rs` — `ProcessSpec.terminal`               | A runtime asks to run in a terminal. Nothing from the webview reaches it.                                                        |
| `infrastructure/pty.rs`                                         | Starts the process in a PTY (`portable-pty`), reads its output, ends it, implements `ProcessSession`.                            |
| `application/sessions.rs` — `ProcessSession`, `SessionRegistry` | Identity, bounded ephemeral output, state, who stopped it, and the **only** control path.                                        |
| `ExecutionService`                                              | Asks the registry whether the user stopped a run that ended badly → `Cancelled` instead of `Failed`.                             |
| `commands/terminal.rs`                                          | `get_execution_terminal`, `execution_interrupt`, `execution_terminate`, `execution_terminal_input`, `execution_terminal_resize`. |
| `commands/events.rs`                                            | `execution:output` (chunks), `execution:status` (state), and activity on the existing `execution:progress`.                      |
| `commands/exit.rs`                                              | What happens to running agents when Atlas closes.                                                                                |

`ProcessRunner` stays small (`locate`, `run`). A running process's controls are a separate object, `ProcessSession`
(`write`, `interrupt`, `terminate`, `resize`), reached through the registry — a runner starts, a session controls.
Runtime-specific behaviour stays in the runtime: `cli::deliver_prompt` is the one shared helper, and no code branches on
which runtime it is.

## Why a PTY, and not pipes

With pipes the child sees that stdout is not a terminal; CLIs change behaviour (buffering, colours, prompts, progress
output) and some refuse to be interactive. A PTY gives the child a terminal: ANSI output, a size it can query and that can
change (`resize`), and a terminal driver that turns **Ctrl+C into an interrupt for the foreground process group**.

Library: **`portable-pty`** (the PTY layer of WezTerm). It supports Unix PTYs and Windows ConPTY behind one API, has no
async runtime requirement, and is maintained by a project that depends on it for its own terminal. Alternatives considered:
`pty-process` and `nix::pty` (Unix only, so Windows would need a second implementation), `conpty` (Windows only), and
hand-rolled `openpty` (needs `unsafe`, which this crate forbids). New dependencies: `portable-pty` and, on Unix only, `nix`
(already a transitive dependency of `portable-pty`, same version; used for the safe `killpg`). The crate keeps
`unsafe_code = "forbid"`.

Cost of a PTY, verified against the real CLIs:

- **One output stream.** stdout and stderr merge. The runtimes read their diagnostics from the non-JSON lines of the merged
  output (`cli::diagnostics`); the `stream` field of `execution:output` is part of the contract but is always `stdout`.
- **No input channel for a prompt.** `claude -p` and `opencode run` **ignore a terminal's stdin** (they report "input must be
  provided…"). In a terminal the prompt is therefore the last argument, after `--` so that text starting with `-` is never an
  option. The pipe path (prompt on stdin) remains for the one case a terminal cannot serve (below).
- **Control sequences in the output.** The structured result (`stream-json` / `--format json`) still comes through the
  terminal, but may be wrapped in cursor/colour sequences and CRLF. The runner strips them (a stateful stripper that survives
  sequences split across reads) before the runtimes parse; the raw bytes go to the terminal view. A test runs the real Claude
  runtime against a stand-in CLI that prints sequences around its JSON.
- **Windows `.cmd` shims** cannot take multi-line arguments, so a prompt with newlines on a `.cmd`/`.bat` shim falls back to
  pipes (that execution has no live terminal). Native `.exe` installs use the terminal.

`--output-format stream-json` is compatible with a terminal: the JSON lines are the process's real output, the runtime's
`ExecutionResult` is unchanged, and nothing was removed to make the terminal work.

## Identity

`executionId` (Atlas's own, existed already) and `processSessionId` (new, per live process). The registry is keyed by
execution. Every control request names **all three of workspace, agent and execution**; a session answers only when all
three equal the ones it was opened with (taken from the guard-approved `ExecutionScope`, never from the webview). Anything
else — unknown, other workspace, other agent — is the same `ExecutionNotFound`, so a probe learns nothing. There is no
"current process" and no singleton: Agent A, B, C and D each have their own execution and session, and interrupting A
cannot reach B. The frontend knows execution ids, events and capabilities; it never sees a PID, a signal or a process group.

## Ctrl+C, terminate, and process trees

**Interrupt** is the byte `0x03` written to the PTY — one mechanism on every OS:

- **macOS / Linux:** the terminal driver sends `SIGINT` to the PTY's foreground process group. The CLI gets it as it would
  from a person, and so does everything it started in the foreground (`node → npm → test`). The child is a session leader
  (`setsid`, done by `portable-pty`), so its process group is its own.
- **Windows:** ConPTY translates the input into a console control event for the processes attached to the pseudo-console.

It is deliberately not `kill(pid)`: that would hit only the root, skip the CLI's graceful shutdown, and could not
distinguish "stop" from "die". The CLI may handle the interrupt or ignore it; Atlas reports the state (`interrupting`) and
leaves the next step to the user. A second interrupt is just a second Ctrl+C and cannot panic.

**Terminate** is the second step, always available and visually secondary:

- **Unix:** `SIGTERM` to the whole process group (`killpg`), and if the group is still there after 2 s, `SIGKILL` to it.
- **Windows:** `taskkill /PID <pid> /T /F` (kills the tree; there is no graceful group signal).

Signals are sent while holding a lock that is also held when the child is reaped, so a signal can never reach a PID the OS has
already reused. When a process ends, its terminal is closed: the kernel hangs up on anything still attached (`SIGHUP`).
A descendant that deliberately detaches (new session, ignores `SIGHUP`) is outside Atlas's reach.

State: `Running → Interrupting | Terminating → Exited` (live, per process, `execution:status`). The persisted execution
status gained **`Cancelled`**; `Interrupting`/`Terminating` are live session states, not stored. `Timeout` stays a failure kind
of a `Failed` execution (compatible with what exists); `Queued` and `Starting` were not added: nothing queues, and starting
is already `starting_runtime`. A run ends as `Cancelled` when it did not complete **and** the user acted on its process;
a run that finished before the interrupt landed stays `Completed`.

## Manual input

Investigated, not assumed: both CLIs run non-interactively (`claude -p`, `opencode run`) and **ignore a terminal's stdin**.
Both therefore report `terminalInput = false` and their terminal is read-only. Ctrl+C works regardless. The path exists for
a future interactive runtime (`RuntimeCapabilities.terminal_input`, then `ProcessSpec.terminal.input`), and is enforced in
the core, not the UI: input is refused unless the runtime enabled it; bytes are passed on **untouched** (Ctrl+D, Ctrl+Z,
escape sequences and pasted multi-line text are just input to that process's own terminal and are never interpreted as Atlas
commands); a payload over 64 KiB is refused; and `0x03` is the one exception — alone it is routed through _interrupt_ (so
the state and the cancellation are tracked), mixed with other bytes it is refused.

## Capabilities

`RuntimeCapabilities` gained `interactiveTerminal`, `interrupt`, `terminalInput`, `terminalResize`.

| Runtime      | interactiveTerminal | interrupt | terminalInput | terminalResize |
| ------------ | :-----------------: | :-------: | :-----------: | :------------: |
| Claude CLI   |         yes         |    yes    |      no       |      yes       |
| OpenCode CLI |         yes         |    yes    |      no       |      yes       |
| Codex CLI    |         yes         |    yes    |      no       |      yes       |

The UI shows only what the runtime can do ("does not provide a terminal", no Interrupt button, read-only badge).

## Security: relation to `PermissionPolicy`

- A process session exists only for a process the **guard let start** (`GuardedProcessRunner` evaluates every spec before the
  system runner is reached; a denied or unknown program never gets a session). The runner never accepts a program from the
  commands, and **no command starts a process** — there is no shell, no "open terminal", nothing that takes a program or a
  command line from the webview.
- The webview is not trusted with authorization: the core re-checks workspace/agent/execution on every request.
- Terminal output is data for display. It is never parsed as instructions or turned into Atlas actions.
- The guard still records `LaunchRuntime` and `runtime_exceeds_policy` notes; the live shell adds activity (user vs. agent):
  `terminal_connected`, `user_interrupted`, `user_terminated`, `process_exited`, `cancelled`.
- Limits: the live shell is not a sandbox. It cannot stop what the runtime's own tools do inside the process (OpenCode's
  shell tool runs inside the terminal's process group, for example); Ctrl+C reaches them, nothing here restricts them.

## Output, persistence and performance

Output reaches the UI as incremental `execution:output` chunks (`seq` per session), reassembled in a `TerminalHub` outside
React state. The core keeps the last **512 KiB** per session and the last **8** finished sessions so a terminal can be
re-drawn after a reload or workspace switch (`get_execution_terminal` returns a snapshot with `nextSeq`, so the snapshot and
the live chunks meet without a gap or repeat). Nothing is persisted: `config.json` never receives terminal output; the
execution record still holds prompt, answer, status, usage, timestamps and failure. Chunks are as read from the PTY (≤ 8 KiB);
no batching or backpressure was added — the CLIs produce lines at token rate and this has not shown a need. Revisit if
a runtime streams megabytes per second.

## When Atlas closes

- Window closed / quit with executions running: Atlas asks ("Agents are still running": **Cancel executions and exit** /
  **Keep Atlas open**). The dialog is native and worded by the core in the user's language (the webview is going away).
- Whichever way the application ends, its agents' processes are ended with it (`RunEvent::Exit` →
  terminate all, waiting up to 4 s). If Atlas is killed outright (`SIGKILL`, crash, power loss) the PTY master closes and the
  kernel hangs up on the children, which makes the usual CLIs exit; one that ignores `SIGHUP` could survive. Windows
  closes the pseudo-console with the same effect, not verified.
- Switching workspace never touches processes. Sleep/wake: processes simply continue; nothing is done.

## Validation and known limitations

- **macOS:** validated, including with the real Claude and OpenCode CLIs (opt-in tests: `cargo test real_ -- --ignored
--nocapture --test-threads=1` — a real long task, output observed, Ctrl+C, process gone, execution `Cancelled`).
- **Linux:** the Unix code paths are the same and use only POSIX facilities; not run on a Linux machine in this milestone.
- **Windows:** the code has `cfg(windows)` branches but was **neither run nor compiled** here (the Tauri build script
  needs the Windows resource compiler, absent on this machine). ConPTY's handling of `0x03` as a console control event,
  `taskkill` behaviour and `.cmd` shims need a Windows machine; until then treat it as unvalidated.
- Merged stdout/stderr; prompt visible in the process's argument list (`ps`) while a terminal run is in progress (it was in
  stdin before); both are consequences of the PTY.
- Interrupt/terminate are best effort: the process may ignore Ctrl+C (that is what Terminate is for) and a deliberately
  detached grandchild is out of reach.
- Terminal output is not searchable after Atlas restarts (ephemeral by design); a log with an explicit limit would be a later,
  separate decision.
