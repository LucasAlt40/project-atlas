# ADR 0034 — Mobile access: watching and answering runs from a phone on the local network

Status: accepted. Builds on [ADR 0013](0013-workflow-engine-and-orchestrator.md) (runs, events),
[ADR 0016](0016-human-in-the-loop.md) and [ADR 0031](0031-plan-approval.md) (what waits for a person).

## Context

A workflow run stops when an agent needs a person (a plan to approve, a question) and the person
may be away from the computer. The run is owned by Atlas Desktop; nothing outside the app can see
or answer it. We want a **phone browser** to follow runs and answer what waits, with nothing in the
cloud, no native app, and no second workflow engine.

## Findings in the code

| Question                             | Answer                                                                                                                                        |
| ------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------- |
| Source of truth of a run             | `WorkflowExecution` in `config.json`, written by the `Orchestrator` at every transition (`WorkflowService::execution/executions`).            |
| How a person answers                 | `Orchestrator::answer_interaction(run, interaction, answer, observer)`; the engine re-validates inside the run loop (`NotPending` on a race). |
| How controls work                    | `Orchestrator::control(run, Pause/Resume/Cancel, observer)`.                                                                                  |
| What the UI hears                    | `WorkflowObserver` (`workflow:*`), `ExecutionObserver` (`execution:progress`), adapters in `commands/events.rs`. Events carry ids, not state. |
| Where services are wired             | `build_state` (pure Rust, no Tauri); Tauri-specific adapters are created in commands/`setup`.                                                 |
| Command approvals (`ApprovalBroker`) | UI-only by design (ADR 0003/0007): authorise a process command.                                                                               |
| Frontend                             | React 19 + Vite, design tokens in `styles/global.css`, `Markdown`, typed i18n (pt-BR/en-US). One entry (`index.html`).                        |

## Decisions

### 1. The server is an adapter over existing services

`application/remote/` holds the rules (pairing, sessions, authorisation tiers, the read model, the
event hub); `infrastructure/remote_server/` is an HTTP adapter; `commands/remote.rs` is the desktop
UI's way to switch it on. The gateway **calls** `WorkflowService` / `Orchestrator`; it contains no
orchestration. A remote answer takes exactly the path of a desktop answer, so the desktop reflects
it through the same observer and `workflow:*` events, and a desktop/phone race is settled by the
engine (`NotPending`), not by the transport.

### 2. HTTP stack: hyper 1 on Tokio, both already in the dependency tree

Tauri already links Tokio and hyper. We enable `hyper`'s `server`/`http1` and write a small router;
no web framework is added. New crates are small and single-purpose: `if-addrs` (list network
interfaces on all three OSes), `qrcode` (QR as SVG, no image stack) and `getrandom` (OS CSPRNG).

### 3. Real time: Server-Sent Events with a cursor, not WebSocket

The traffic is one-directional (server → phone); writes are ordinary authenticated `POST`s, which
also gives them retries, status codes and idempotency for free. SSE reconnects by itself and sends
`Last-Event-ID`. Events are **hints, not state**: `{seq, kind, runId}`. The client reacts by
re-reading the REST resource, so the source of truth is always the store, never the stream:

- The hub keeps a ring of the last 512 events with monotonic `seq`. On connect the client's
  `Last-Event-ID` is compared with it; if the cursor is older than the ring (or from a previous
  server start) the stream starts with a `resync` event, and the client reloads everything.
- Duplicates and reordering cannot corrupt anything: handling an event means "read again".
- A 15 s heartbeat keeps the connection alive through proxies and lets the browser notice a dead network; revoking a device also wakes
  its stream at once.
- The page also reads again on `visibilitychange`, when the network comes back and every 30 s as a safety net, and when the stream
  drops it asks the computer directly after 2 s: no answer is shown as "can't reach the computer", not as an endless "reconnecting".

### 4. Mobile UI: a second entry of the same Vite project, served from the app's own assets

`mobile.html` + `src/mobile/` (small, no router, no graph library) reuse the design tokens, the
`Markdown` renderer and the i18n catalogue. The built files are part of Tauri's embedded frontend,
so the Rust server serves them with `asset_resolver` (no files to ship, nothing to unpack, same
version as the app). It is not the desktop UI shrunk to a small screen: it has its own screens
(Dashboard, Run, Plan/Question) built for touch.

### 5. Authentication: one-time pairing code → session cookie

- Turning the server on creates a **pairing code**: 80 random bits (16 base32 characters), valid for
  5 minutes, **single use**. The QR encodes `http://<ip>:<port>/#<code>`. The code is in the URL
  **fragment**: it is never sent to the server in the request line, never in `Referer`, logs or
  history of the server; the page removes it with `history.replaceState` straight away.
- The page `POST /api/pair {code, deviceName}`. The server consumes the code and answers with a
  **session**: 256 random bits in an `HttpOnly; SameSite=Strict; Path=/` cookie. Only the SHA-256 of
  the token is kept (in memory). Idle expiry 24 h, absolute 7 days.
- Sessions live **in memory only**: stopping the server (or closing Atlas) ends them all. Each
  activation is a new explicit authorisation; no secret is written to disk.
- Brute force: 5 failed pairings invalidate every pending code and lock pairing for 60 s (doubling
  while it goes on); the code space (2^80) makes guessing hopeless even before that. Every code
  check is a hash lookup, not a string comparison against the secret.
- Revocation: one device, all devices, "new code" (invalidates earlier unused ones), disable server.
  Revoking sends the device's open streams a `revoked` event and closes them at once.
- Tokens never appear in URLs, logs, errors or events.

### 6. Network protections

A private LAN is not trusted, so every request passes, in order:

1. **Host check** against an allowlist built from the bound address (`<ip>:<port>`, `localhost`):
   defeats DNS rebinding.
2. **No CORS headers at all** (same origin only). For writes (`POST`) the `Origin` header, when
   present, must equal the allowed origin; a missing `Origin` on a write is refused; and the body
   must be `application/json` (not a form post).
3. **Authentication** for everything except the static shell and `POST /api/pair`.
4. **Rate limit** per session on writes and per client IP on pairing; body ≤ 16 KB.
5. Responses carry `Cache-Control: no-store`, `X-Content-Type-Options`, `Referrer-Policy: no-referrer`,
   `X-Frame-Options: DENY` and a strict CSP (`default-src 'self'; connect-src 'self'; frame-ancestors 'none'`).

The server binds **one** interface address chosen by the user (never `0.0.0.0` and never a public
address by default), only while enabled, on a port from a small range (fallback if busy). Atlas does
no port forwarding, UPnP or tunnelling.

### 7. HTTP, not HTTPS

A self-signed certificate on a LAN IP is not trusted by phones; the browser shows a full-page warning
that trains the user to click through, and installing a private CA on a phone is not an acceptable
setup. Real certificates for an IP are not obtainable without a public name and a cloud. We
therefore serve plain HTTP and say so, and compensate: single-use short-lived pairing, HttpOnly +
SameSite cookie, no secrets in URLs, strict CSP, read model without secrets, and tiered actions
(§8). The limitation is shown in the desktop dialog: **anyone who can capture traffic on the same
network (open Wi-Fi) can read what the phone shows and steal its session**; use the feature on a
network you trust, or reach the computer through a VPN (WireGuard/Tailscale), which encrypts it and
needs nothing from Atlas. A secure context is also unavailable over HTTP, so the Notification API and
service workers are not offered (see §10).

### 8. What the phone may do (authorisation tiers)

| Tier | Action                                                                                                                                                                                           | Remote                                                                   |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------ |
| 0    | Read runs, steps, events, plans, questions, results (redacted)                                                                                                                                   | yes                                                                      |
| 1    | Answer a **plan**, a **clarification**, an **approval** interaction                                                                                                                              | yes                                                                      |
| 2    | **Pause / resume** a run                                                                                                                                                                         | yes                                                                      |
| 2    | **Cancel** a run (keeps what completed)                                                                                                                                                          | yes, with explicit confirmation in the request (`confirm: true`)         |
| 3    | A **permission** or **runtime confirmation** question                                                                                                                                            | only the _refusing_ answer (`deny`/`no`): remote may narrow, never widen |
| 4    | Guardrail questions (what context goes to a model), command approvals (`ApprovalBroker`), apply/keep/discard code, resume a failed/interrupted run, start a run, terminal input, files, settings | desktop only — shown on the phone as "needs the computer"                |

Anything not in this table does not exist as an endpoint: there is no generic proxy of Tauri
commands, no path, program or file parameter anywhere in the remote API.

### 9. Read model: a projection, not the domain objects

`application/remote/view.rs` builds DTOs from `WorkflowExecution`: names instead of folder paths,
steps with agent name, status, attempts and times, edges (to show parallel/conditional structure),
interactions (full document for the pending ones), events (messages as the engine wrote them), and
the result summary. Never sent: project paths, worktree paths, environment, MCP secrets, raw
terminal output, diffs. Every free text coming from agents (plan documents, summaries, event
messages) goes through the existing secret redaction (`harness::secrets::redact_secrets`) — a plan
may quote a `.env` it read. Progress is shown only where the engine knows it (steps completed out of
steps in the graph is **not** presented as a percentage).

### 10. Notifications

v1 delivers alerts **while the page is open**: an in-page attention banner, the browser tab title
(`(2) Atlas`), and a pending-first dashboard. Browser notifications and push need a secure context
and a push service; they are explicitly not promised. The hub already emits typed events, so another
channel is a new subscriber, not a redesign.

### 11. Lifecycle

- Disabled at every start; never persisted as "on".
- `RemoteAccess` holds the state `disabled → starting → active | error`. A bind problem is reported with a code (`no_network`,
  `address_unavailable`, `port_unavailable`, `bind_failed`). A network change is noticed by the desktop dialog, which reads the status every
  second: the status says `addressLost` when the bound address is no longer one of the computer's, and the dialog asks to turn it off and on
  (sessions end then; there is no silent rebind).
- The server runs on the async runtime, apart from the orchestrator's threads: a crash or stop of it
  affects no run. Stopping aborts every connection and drops all sessions. Atlas exiting does the
  same.
- Sleep/off: the phone's stream breaks, the page shows "computer unreachable" with the time of the
  last update and retries; it makes no promise of monitoring.

## Not done in v1

HTTPS/push, remote start of runs, applying code from the phone, answering guardrail/permission grants
remotely, persistent devices across restarts, multi-user accounts, remote terminal.

## Verification

- Rust: `application/remote/*` unit tests (tokens, pairing, sessions, hub, policy, interface ranking) and
  `application/workflow/remote_tests.rs`, which run the real server on a loopback socket against a real `Orchestrator` driving a scripted run.
- Frontend: `src/mobile/*.test.*` (pairing, dashboard, run, answers, conflicts, offline, revocation, alerts) and
  `src/features/remote/*.test.tsx` (the dialog).
- The phone page was also opened in the app's embedded browser at a phone viewport, served by the real server over real data (pairing from the
  link, dashboard, plan, approval, live update, server killed → "reconnecting"). The packaged asset path (`asset_resolver`) is covered by a test
  that reads the page out of the embedded frontend.
- **Not verified here:** a physical phone on a Wi-Fi network; Windows and Linux builds (every crate added is cross-platform, but only macOS was
  compiled and run); a packaged app (`tauri build`) with the server switched on from the dialog.

## Consequences

- One new HTTP surface in the app. Everything it can do is in `application/remote/policy.rs` and the routes of `remote_server.rs`; a new
  route or action is a new line there and a new test, not a new proxy.
- `orchestrator_tests` exposes a few helpers to the sibling test module (`pub(super)`); production code is unchanged except for the observer
  taps in `commands/events.rs`, the `RemoteAccess` entry of `AppState` and the new commands.
