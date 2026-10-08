# Architecture

Project Atlas is a Tauri 2 desktop app with two layers:

```
Frontend (React + TypeScript)  — presentation and user interaction
        │  Tauri commands (typed, allow-listed)
Core (Rust)                    — everything privileged or local-machine related
```

## Frontend / backend boundary

- The webview never touches the filesystem, processes, shell, Git, credentials or the network directly.
  Those belong to Rust and are reached only through Tauri commands.
- All calls go through `src/lib/tauri/commands.ts`. `CommandMap` is the typed contract; ESLint forbids
  importing `@tauri-apps/api/core` anywhere else.
- Features wrap that in their own `services/` module (e.g. `features/home/services/appInfoService.ts`) and
  expose hooks to components. Components never know about `invoke`.
- Wire types are written by hand on both sides today (`AppInfoDto` ↔ `domain::app_info::AppInfo`). See
  "Future extension points" for when to generate them.

## Rust modules (`src-tauri/src`)

| Module            | Responsibility                                                                                        |
| ----------------- | ----------------------------------------------------------------------------------------------------- |
| `commands/`       | Tauri handlers. Thin adapters: pull services from state, call them, return DTOs. No business logic.   |
| `application/`    | Use cases (`AppInfoService`) and **ports** — traits that describe what the core needs from the world. |
| `domain/`         | Plain models and rules. No Tauri, no I/O.                                                             |
| `platform/`       | OS-specific adapters implementing ports (`OsPlatform`). Selected with `cfg`/runtime checks here only. |
| `infrastructure/` | Adapters for the outside world: child processes (`SystemProcessRunner`), JSON config file.            |
| `state/`          | `AppState`: services shared with commands via Tauri managed state.                                    |
| `lib.rs`          | Composition root: the only place that wires concrete adapters into services.                          |

Dependency direction: `commands → application → domain`. Adapters (`platform/`, and later
`infrastructure/`) depend on `application` to implement its ports, never the reverse (see ADR 0002).

## Agents, runtimes and executions (V0.3)

An agent is _personality + runtime + model + instructions_, run against a _project context_. The
concepts are kept distinct on purpose (see [ADR 0004](adr/0004-runtimes.md)):

| Concept         | Meaning                                            | Where                                             |
| --------------- | -------------------------------------------------- | ------------------------------------------------- |
| Personality     | How the agent behaves                              | `domain/personality`, `application/personalities` |
| Provider        | Who provides the AI (Anthropic, OpenCode…)         | `domain/runtime::ProviderRef`                     |
| Runtime         | How Atlas reaches it on this machine               | `application/runtimes` (port `ModelRuntime`)      |
| Model           | Which model reasons                                | a string id, listed by the runtime when it can    |
| Agent           | Saved personality + runtime + model + instructions | `domain/agent`, `application/agents`              |
| Project context | What the agent needs to know                       | `application/projects::context`                   |
| Prompt          | The three concerns assembled                       | `application/prompt` (`PromptBuilder`)            |
| Execution       | One attempt at a task                              | `domain/execution`, `application/executions`      |

```
WorkspacePage → WorkspaceProvider / useCatalog → feature services
  → lib/tauri → commands/*.rs → ChatService → ExecutionService → PromptBuilder
  → RuntimeRegistry → ModelRuntime (OpenCodeRuntime | ClaudeRuntime | …) → ProcessRunner → the CLI
```

- **Commands** (see the V0.5 sections for workspaces, settings, chat and usage): personalities, `list_runtimes`,
  agents. The webview only ever names a `runtimeId`, a `workspaceId` and an `agentId`.
- **`ModelRuntime`**: `info()` (id, name, provider, transport, `RuntimeCapabilities`, model hint),
  `detect()`, `list_models()`, `execute()`. `RuntimeRegistry::inspect_all` combines detection and model
  discovery into a structured `RuntimeStatus` (`availability`: not installed / unavailable / authentication
  required / ready; `authentication`; `modelDiscovery`: discovered / unsupported / unavailable / failed).
  Adding a runtime = one file in `application/runtimes/` plus one line in
  `RuntimeRegistry::with_default_runtimes`. Nothing in agents, prompts, executions, commands or the UI changes.
- **Runtimes today**: `OpenCodeRuntime`, `ClaudeRuntime`, `CodexRuntime`, `GeminiRuntime` and `AntigravityRuntime`
  execute tasks. `exec` never asks for approval, so Codex is bounded by its own sandbox (`read-only`, or
  `workspace-write` when the execution may edit); the sandbox-bypass flag is never used.
- **Antigravity CLI** (program `agy`, not `antigravity`): models from `agy models` (`id<TAB>name`, the reasoning
  effort is part of the id); runs `agy --output-format stream-json --model <id> --prompt=<prompt>` in a terminal
  (`-p` takes a value and ignores stdin, so the prompt is an option value, never stdin). A headless run denies
  every tool that would need a prompt (verified: file writes and shell), so the default is read-only;
  `allow_edits` adds `--mode accept-edits`. `--dangerously-skip-permissions` is never used. There is no flag to
  restrict tools (so `text_only` is off and `toolAccess` is reported open: the user's `settings.json` may allow
  commands) and no sign-in status command (reported `unknown`; a run that fails for it is classified).
- **Gemini CLI**: `gemini --output-format stream-json --model <model> --approval-mode default|auto_edit
--skip-trust --prompt=<prompt>`; headless runs cannot use tools that need approval, so `default` is read-only
  and `allow_edits` uses `auto_edit` (`--yolo` is never used). No model list or sign-in status. Google ended the
  CLI for individual accounts (`IneligibleTierError`), which is reported as unavailable and points to Antigravity.
- **OpenCode**: models from `opencode models`; `opencode run --agent plan -m <model> --format json`, prompt on
  stdin. `--agent plan` (OpenCode's built-in read-only agent) is what makes the run read-only: a plain
  `opencode run` was verified to create files and run shell commands without asking.
- **Claude CLI**: version from `claude --version`; sign-in from `claude auth status` (JSON `loggedIn`; exits 1
  when signed out, so stdout is parsed regardless of exit code); runs
  `claude -p --output-format json --model <model> --tools Read,Grep,Glob --no-session-persistence` with the
  prompt on stdin and reads the `result` field. The CLI has no model-list command, so discovery is reported as
  unsupported and the UI asks for a model id, with the runtime's own hint (aliases such as `sonnet`).
  Authentication is the user's existing Claude sign-in; no key is requested.
- **Read-only**: Atlas has no permission system yet. Claude runs with only the Read/Grep/Glob tools; OpenCode runs
  as its read-only `plan` agent and is never given `--auto`. Both also receive a read-only rule in the prompt.
  Atlas relies on each tool honouring this; there is no sandbox.
- **Authentication model**: `AuthKind` (`cli_session`, `api_key`, `environment_variable`, `credential_store`)
  and `Authentication { kind, state }` exist so API-key runtimes can be added. Only `cli_session` is used.
  Secrets must never go in `config.json`; they will use the OS credential store (Keychain / Credential Manager /
  Secret Service) behind a port.
- **Working directory**: the runner sets `PWD` together with the child's current directory. Without it OpenCode
  trusted the inherited `PWD` and rejected reads of the project as an external directory.
- **Process security and portability**: all child processes go through the `ProcessRunner` port
  (`infrastructure/process.rs`). Runtimes name a bare program and fixed arguments; model ids are validated so
  they cannot act as options. No shell is ever started. Executables are located on `PATH` plus the usual
  developer-tool directories (Windows: `.exe`, `.cmd`, `.bat`, and `%APPDATA%\npm`). Prompts go on stdin, never
  in arguments, because Windows `.cmd` shims reject multi-line arguments. Timeouts kill runaway processes.
- **Errors**: `RuntimeError` is normalized (not installed, unavailable, authentication required, model
  unavailable, timeout, execution failed, invalid request, unexpected response). A runtime failure becomes a
  _failed_ execution with a user-facing message and optional technical details.
- **Results**: an `Execution` records `runtimeId`, `modelId`, the exact prompt, the result or failure, and
  runtime-reported `metadata` (duration, cost…); the UI reads this normalized shape only.
- **Progress**: see "Workspace" below. No streaming yet.
- **Persistence**: custom personalities and agents live in `config.json` in the app data directory
  (atomic writes; an unreadable file is set aside, not overwritten). Agents saved before V0.3 with a
  `providerId` still load. Conversations and ended executions are persisted too (V0.6, [ADR 0008](adr/0008-conversation-and-execution-history.md));
  terminal output is not.
- **Navigation**: Workspace, Agents, Personalities, Settings (`src/app/navigation.ts`); `AppShell` shows the workspace
  switcher and language toggle in its header. No router yet.

## Live shell and process control (V0.6.x)

An execution that runs a CLI runtime runs in a real PTY and has a _process session_; the agent card has **Chat / Activity /
Terminal** views, and the user can interrupt (Ctrl+C) or terminate the process behind an execution. The core owns the PTY,
the PID and the signals (`infrastructure/pty.rs`, `application/sessions.rs`); the webview only names an execution (with its
workspace and agent) and listens to `execution:output` / `execution:status`. It controls a process the guard already
authorized and starts nothing. See [ADR 0007](adr/0007-live-shell-and-process-control.md). Ended executions and conversations
are kept (`application/history.rs`) and shown in the agent card's _Executions_ tab and the Execution Inspector; the header lists
running agents of every workspace. See [ADR 0008](adr/0008-conversation-and-execution-history.md).

## Git worktree isolation (V0.6.x)

Agents have `worktreeIsolation` (on by default). An isolated execution runs in its own Git worktree and branch under the app's
data folder, never in the project's checkout; `WorktreeService` (use case) drives `WorktreeManager` (port, Git adapter in
`infrastructure/git_worktree.rs`) and hands the runtime only a working directory. When the execution ends the work is committed
on its branch, measured by Git and, if the agent's `git.write` policy allows it and nothing is in the way, merged; otherwise it
is kept with a recommendation, and merging needs the user's explicit action (`merge_execution`). See
[ADR 0009](adr/0009-git-worktree-isolation.md).

## Project Harness (V0.7)

Atlas can turn a project into one it knows: `Initialize Project` analyses the repository deterministically (no code is run),
lets the user review the findings, and writes a versionable `.atlas/` Harness into the project. Executions then receive it
as prompt context through `HarnessContextBuilder`; runtimes and worktrees never know it exists. It is context, never
permission. See [ADR 0010](adr/0010-project-harness.md).

V0.7.1 makes that context trustworthy: every statement carries evidence and provenance (fact, inference, user), an optional
model-assisted semantic analysis proposes findings that must cite what it was shown, contradictions are recorded rather
than resolved silently, refresh shows a diff and never overwrites the user's own knowledge, and agents receive a compact
summary separating what is known, inferred, told by the user and unknown. See
[ADR 0011](adr/0011-harness-knowledge-model.md).

V0.8 makes the context **task-aware**: instead of the whole Harness, an agent is told the part its task needs, chosen by
deterministic rules (a small tag lexicon, areas, categories, paths and keywords; no model, no embeddings) and explainable
item by item. Constraints and decisions are never left out, inference stays inference, stale knowledge is told as outdated,
and what was left out is named. If a selection cannot be made the whole Harness context is used and the execution records
why. A preview shows the context before the run. See [ADR 0012](adr/0012-task-aware-context.md).

## Workflows: engine, orchestrator and visual graph (V0.9)

A task can now run as a **workflow** — a graph of agent steps, conditions and ends — instead of one agent. Four
separate concerns (see [ADR 0013](adr/0013-workflow-engine-and-orchestrator.md)):

```text
domain/workflow, domain/orchestration     the model: Workflow, Node, Edge, Condition, WorkflowExecution, shared state,
                                          Artifact, Decision, AgentResult — no behaviour that needs I/O
application/workflow/graph.rs             links, cycles (SCC), reachability
application/workflow/validation.rs        every rule that makes a graph runnable (bounded loops, agents exist, …)
application/workflow/engine.rs            the Workflow Engine: a pure state machine (ready / dispatch / complete / route /
                                          retry / block / skip / pause / cancel / interrupt)
application/workflow/orchestrator.rs      drives a run: StepRunner port, threads, approvals watch, persistence, events
application/workflow/chat_runner.rs       the real StepRunner: a step is an execution through ChatService
application/workflow/service.rs           definitions (versioned), runs (snapshots), recovery of interrupted runs
application/workflow/templates.rs         deterministic templates and the automatic selector
application/orchestration/                result parser, handoff into shared state, the per-step brief
commands/workflow.rs                      thin Tauri adapter (15 commands, `workflow:*` events)
features/workflow (frontend)              page, canvas (React Flow as presentation only), inspector, overview
```

A step goes through `ChatService::send_workflow_step`, so it is an ordinary execution: the agent's worktree policy, the
Harness (Task Context selected for the step's own work), the runtime registry and the permission guard all apply, and the
execution appears in history and in the Execution Inspector with a _Workflow › name › step › Execution #n_ breadcrumb.
Nothing in a workflow can run a process or approve anything.

Steps hand results to each other as explicit **handoffs**, share **one worktree per run** (what the Developer writes is what the
Validator reads), and the run's code is measured by Git and kept apart from the run's own status: a run can complete while its code is
still only in the isolated worktree, and only an explicit user decision (apply, keep, discard, open in an editor) changes that. See
[ADR 0014](adr/0014-handoff-shared-worktree-and-code-integration.md).

An agent can declare a **result contract** (outcomes such as `pass`/`fail` or `approved`/`changes_requested`); steps route on the
structured `result.outcome`, never on free text, and a step that must conclude and does not fails instead of being read as a pass.
Steps that write take the run's worktree exclusively; readers share it. See
[ADR 0015](adr/0015-result-contracts-outcomes-and-worktree-lock.md); routes are checked against contracts before a run and repaired only on the user's confirmation ([ADR 0017](adr/0017-contract-aware-workflow-validation-and-route-repair.md)).

## Human in the loop (V0.9.3)

An agent that stops to ask a person is **waiting**, not done ([ADR 0016](adr/0016-human-in-the-loop.md)).
`ExecutionStatus`, `NodeStatus` and the run each have a `WaitingForInput` state; the question is a
`PendingInteraction` persisted with the run. The detector (adapter signal → `atlas-interaction` block →
conservative text analysis) lives in `application/interaction`; the orchestrator pauses the run, takes the
answer through `answer_workflow_interaction`, and starts the step again from it in the same worktree. The UI
shows "Action required" in the header and on the run; answering never applies code to the project.

## Optimization Layer (V0.10)

A cross-cutting layer that measures what each execution sends and costs, and removes what is said twice, without touching required
or security text. It wraps the existing pieces instead of replacing them: `PromptBuilder` is still the only prompt assembly, the
Harness's `TaskContextService` still selects project context, `UsageMetrics` still holds what runtimes report, and `security/` still
decides what may run.

```text
ExecutionService.run_step
   ├─ Harness / Task Context ─────────── (existing selection)
   ├─ Skills (opt-in) ────────────────── discover → select → activate → references
   ├─ PromptBuilder.assemble ─────────── the only assembly
   ├─ Context Engine (opt-in) ────────── dedupe lines, compress whitespace, budget (reports, never cuts)
   ├─ PromptBuilder.assemble (again, only if the engine changed an input)
   ├─ Runtime (isolated from the user's MCP servers and skills for Claude)
   └─ OptimizationMetrics: sections, estimated vs reported tokens, latency, tools exposed/used, extensions loaded
```

Pictures of how the pieces fit (Atlas above the agent harness, layers, one step, authority, MCP): [diagrams.md](diagrams.md).

Status of every phase, what was measured, what was removed (RTK) and what is pending: [optimization-layer.md](optimization-layer.md).

## Workspaces, project context and agent chat (V0.5)

Atlas is organised around **workspaces**: a workspace is a project environment where agents work (see
[ADR 0005](adr/0005-workspaces-usage-i18n.md)). Agents never talk to each other.

```
Global (outlives every workspace)         Workspace-scoped
  settings: language, last workspace        Workspace { id, name, projectPath, description, layout }
  personalities, agents (config)              └ layout: rows, columns, agentPlacements[{ agentId, position }]
  installed runtimes and their status        conversations (agent × workspace), runs, usage records
```

- **Workspace vs agent.** `Workspace` (`domain/workspace`, `application/workspace`) owns its project folder, its
  layout (grid size is data; the default is 2 × 2) and where agents sit. An agent is global configuration and may be
  placed in several workspaces; placing, moving or removing it never creates or deletes the agent.
  Chain: Agent → workspace placement → conversation → execution. The 4-agents-per-workspace limit is a UI constant
  (`features/workspace/model/grid.ts`), not a domain rule.
- **Persistence.** Workspaces, settings (`language`, `selectedWorkspaceId`) and the usage ledger are stored in the same
  `config.json` as personalities and agents (`ConfigStore` port, atomic writes, no database). No secrets are ever
  stored there (API keys will use the OS credential store). Conversations are in memory.
- **Project folder.** Chosen with the native picker (`tauri-plugin-dialog`, granted only as `dialog:allow-open`),
  re-validated by the core (`ProjectInspector::is_directory`). Selecting a folder only records its path: nothing is read,
  indexed or sent. `ProjectInspector::technologies` looks at **top-level file names** (plus the dependency names in
  `package.json`) and recognises Git, Node.js, TypeScript, Angular, React, Vue, Svelte, Next.js, Rust, Tauri, .NET,
  Maven, Gradle, Go, Python, Docker, PHP, Ruby, Dart. No model is involved. `PromptBuilder` receives the project's name,
  path and detected technologies as `ProjectContext` metadata. There is no permission enforcement or sandbox yet.
- **Conversations and execution are keyed by workspace and agent** (`Message.workspaceId`, `ExecutionEvent.workspaceId`,
  busy-guard per workspace × agent). The same agent in two workspaces has separate conversations and can run in both at
  once. `ChatService::send` records the user message and returns at once; the run happens on a blocking thread
  (`ExecutionService::run_with_id`, which resolves the project from the workspace) and ends in an assistant message.
- **Switching workspaces** only changes what is shown. `WorkspaceProvider` (above the screens) keeps one reducer keyed
  by `workspaceId/agentId`, so a run keeps being tracked while another workspace or screen is visible and its state is
  correct on return. Deleting a workspace (not while an agent is working in it) also discards its conversations and
  usage; deleting an agent removes it from every workspace and its conversations but keeps the usage it already spent.
- **Events** (one mechanism, two channels): `execution:progress` carries `ExecutionEvent { executionId, workspaceId,
taskId, agentId, kind, message, timestamp, metadata }`; `conversation:message` announces assistant messages. Kinds:
  `started`, `starting_runtime`, `sending_prompt`, `waiting_for_model`, `output_chunk`, `tool_started`, `tool_completed`,
  `completed`, `failed`. A runtime emits only what its tool really streams (Claude: tokens and tools; OpenCode: text
  parts and finished tools). The UI words each step from `kind` + `metadata` (`runtime`, `model`, `tool`,
  `failureKind`); the core's English `message` is for logs, except `output_chunk` (the streamed answer text).
- **Real-time response and narration.** See the V0.3/V0.4 notes: `PromptBuilder` adds a narration rule to every
  personality; the stored answer is the model's concluding message.

## Usage, cost and quota (V0.5)

Usage is **capability-driven and never invented**.

- `RuntimeCapabilities` gained `usageMetrics`, `costMetrics`, `quotaMetrics`. OpenCode: usage and cost; Claude CLI:
  usage, cost and quota; detect-only runtimes: none.
- `UsageMetrics { inputTokens?, outputTokens?, totalTokens?, cost?, currency?, source }`: every field optional.
  `null` means the runtime did not report it and is different from `0`. `source` is `runtime_reported` (one execution),
  `atlas_calculated` (a sum Atlas made over executions it observed) or `provider_reported` (quota windows).
- **What each runtime really reports** (verified against the installed CLIs):
  - _Claude CLI_ (`stream-json` result): `usage.input_tokens`, `cache_creation_input_tokens`,
    `cache_read_input_tokens`, `output_tokens`, and `total_cost_usd` (the CLI's own figure, in USD, not necessarily
    billing); plus `rate_limit_event.unifiedWindows` (`five_hour`, `seven_day`: `utilization` 0 to 1 and `resetsAt`).
    Atlas reports input = all three input kinds, output, total = input + output.
  - _OpenCode_ (`--format json`): per-`step_finish` `tokens` (input, output, reasoning, cache read/write, total) and
    `cost`, summed over the steps. No currency is stated, so none is claimed; no quota.
- **Ledger.** After each execution `ChatService` records a `UsageRecord { executionId, workspaceId, agentId, runtimeId,
modelId, startedAt, completedAt, succeeded, metrics? }` in `config.json` (100-day retention) and the runtime's last
  reported quota (`quotas[runtimeId]`). `UsageReporter` sums records: `UsageTotals` carries `runs`, `runsWithTokens`,
  `runsWithCost` so a partial sum says how many runs it covers; amounts in different currencies are never added (a zero
  in an unstated currency is neutral). The UI passes the period boundaries (`today`, `week` from Monday, `month`) in its
  own time zone. "This conversation" is the executions that have messages in the conversation.
- **Not live.** Neither CLI reports usage while running, so Atlas shows the run's status and activity while it runs and
  the numbers when it finishes. Weekly figures are _Atlas-tracked_, not provider-reported; only quota windows are
  provider-reported, and only where the runtime exposes them.
- **UI.** Clicking an agent's header opens _Agent details_ (identity, status, running time, activity, last execution's
  metrics, conversation/today/week sums, quota windows, runtime/model/status). The workspace header shows a compact
  "this workspace" summary (today / week) that opens a broader view.

## Localization (V0.5)

- `src/i18n`: `en-US.ts` defines the keys, `pt-BR.ts` must define exactly the same ones (type-checked, and a test
  compares them), `createTranslator(language)` fills `{placeholders}`, `I18nProvider` / `useT()` serve it. Portuguese
  (Brazil) is the default; `LANGUAGES` is the single list to extend. Outside the provider (component tests) English is used.
- The language is a **global setting** persisted by the core (`set_language`); the interface switches without a restart.
  `LanguageSwitch` in the header and the Settings screen change it.
- **The core does not translate.** Command failures are `AppError { code, params, detail }` (`application/errors.rs`);
  the frontend maps `error.<code>` to text. Execution failures use `failureKind` (`failure.<kind>`), runtime states use
  `notice` / `modelHint` codes, activity steps are worded from event kinds. Not translated: model, provider, runtime and
  project names, user-created content, AI output, and the personalities' system instructions (they are the prompt). The
  descriptions and behavior lists of untouched built-in personalities are translated by the UI.

## Editing and removing (V0.5)

- **Personalities**: `update_personality`, `delete_personality`, `restore_default_personalities`. A custom
  personality is edited or removed in `config.json`. Built-ins ship with the app, so editing one stores the user's
  changed copy (`builtinOverrides`, same id; its behaviour summary is dropped because it described the original
  instructions) and removing one hides it (`hiddenBuiltins`); "Restore default personalities" clears both and leaves
  custom ones alone. A personality used by agents cannot be removed (the error names them), so no agent is left
  pointing at nothing.
- **Agents**: `update_agent` changes name, personality, runtime, model and instructions; id, creation time,
  conversation and workspace slot are kept, and the next run uses the new settings. `delete_agent` goes through
  `AgentLifecycle` (`application/lifecycle.rs`), the one place that knows an agent's configuration, workspace
  position and conversation go together: it locks the agent (a busy agent cannot be deleted), deletes it, frees
  its position and discards its messages. Past executions stay in the history.
- **UI**: the Agents screen lists, edits and deletes agents; personality cards have Edit / Delete; workspace cards have ✎
  (edit panel, with a confirmed "Delete agent"). Deleting asks for an inline confirmation
  (native dialogs are not dependable in the webview). The workspace re-reads its layout when an agent disappears
  from the catalog.

## Tauri responsibilities

Window lifecycle, IPC, the permission system (capabilities), CSP and bundling. Atlas-specific logic does
not live in Tauri configuration or command handlers.

## Security

Least privilege by default.

- Custom commands are listed in `src-tauri/build.rs`. Tauri generates an `allow-<command>` permission for
  each, and `capabilities/default.json` grants only those. A command not granted is unreachable from the webview.
- The only Tauri plugin is `dialog`, granted as `dialog:allow-open` (the native folder picker, which returns a path
  the core re-validates). No fs, shell, opener or http plugins are installed or granted.
- A strict CSP is set in `tauri.conf.json` (`default-src 'self'`; IPC only).
- A Rust test calls the command through the real generated context, so removing the grant fails CI.
- `unsafe` is forbidden in the crate.

Planned shape for agent execution (not implemented): `Agent → Tool → Permission Policy → User Approval →
Execution`, all inside Rust. The webview only renders approval prompts and sends the user's decision.

## Cross-platform strategy

- Nothing assumes a shell, path separator, or OS. Code that must differ lives in `platform/` behind a port
  declared in `application/` (e.g. a future `ProcessService` with Windows / Unix implementations).
- Use `std::path::Path`/`PathBuf` in Rust, never string-built paths; never hardcode `/bin/bash`.
- Frontend uses system font stacks and no OS-specific APIs.

## Future extension points

Project, Harness, Agents, Models, Tools, MCP, Workflows, Policies, Permissions, Memory, Executions and the
Orchestrator will each become a bounded area: a `domain/<concept>`, a use-case module in `application/`,
ports for their I/O, adapters in `infrastructure/` (to be created when the first adapter — filesystem,
process, Git, persistence — exists), and a `commands/<concept>.rs` plus a `features/<concept>/` folder.
Adding a command means touching `build.rs`, the capability file, `lib.rs` and `CommandMap`.
When the wire surface grows, generate TS types from Rust (e.g. `specta`/`ts-rs`) instead of hand-writing them.
A frontend router and global store are intentionally absent until a second screen / shared state needs them.

## ADRs

- [0001 — Tauri 2, React, Vite, TypeScript, CSS Modules](adr/0001-stack.md)
- [0002 — Ports in `application`, adapters outward](adr/0002-ports-and-adapters.md)
- [0003 — Deny-by-default command permissions](adr/0003-command-permissions.md)
- [0004 — Providers, runtimes and models](adr/0004-runtimes.md)
- [0005 — Workspaces, usage and localization](adr/0005-workspaces-usage-i18n.md)
- [0007 — Live shell and process control](adr/0007-live-shell-and-process-control.md)
- [0008 — Conversation and execution history](adr/0008-conversation-and-execution-history.md)
- [0009 — Git worktree isolation](adr/0009-git-worktree-isolation.md)
- [0010 — Project Harness](adr/0010-project-harness.md)
- [0011 — Harness knowledge model, evidence, semantic analysis, verification and staleness](adr/0011-harness-knowledge-model.md)
- [0012 — Task-aware context](adr/0012-task-aware-context.md)
- [0013 — Workflow Engine and Orchestrator](adr/0013-workflow-engine-and-orchestrator.md)
- [0014 — Handoff, the run's shared worktree and code integration](adr/0014-handoff-shared-worktree-and-code-integration.md)
- [0015 — Result contracts, outcomes and the shared worktree's lock](adr/0015-result-contracts-outcomes-and-worktree-lock.md)
- [0016 — Human in the loop: an agent waiting for a person](adr/0016-human-in-the-loop.md)
- [0017 — Contract-aware workflow validation and assisted route repair](adr/0017-contract-aware-workflow-validation-and-route-repair.md)
- [0018 — Live Workspace: observing a workflow run's worktree while it works](adr/0018-live-workspace.md)
- [0019 — Review & Delivery: reading a finished workflow and deciding about its code](adr/0019-review-and-delivery.md)
- [0020 — Optimization Layer, phase 0: measuring before optimizing](adr/0020-optimization-observability.md)
- [0021 — Optimization Layer, phase 1: the Context Engine](adr/0021-context-engine.md)
- [0022 — Optimization Layer, phase 2: Skills with progressive disclosure](adr/0022-skills.md)
- [0023 — RTK (Rust Token Killer): evaluated, built, and removed](adr/0023-rtk-evaluated-and-removed.md)
- [0024 — Optimization Layer, phase 4: what a runtime can reach (surface and isolation)](adr/0024-runtime-surface-and-isolation.md)
- [0025 — Optimization Layer, phase 5: Guardrails and Context Review](adr/0025-guardrails-and-context-review.md)
- [0026 — Context & Tooling Platform (Phase A: architecture and spike)](adr/0026-context-and-tooling-platform.md) — design: [context-and-tooling-platform.md](context-and-tooling-platform.md)
- [0027 — Rules, Context Authority and the system-prompt channel (Phase C)](adr/0027-rules-context-authority-and-system-prompt-channel.md)
- [0028 — MCP foundation (Phase D)](adr/0028-mcp-foundation.md)
- [0029 — MCP for every runtime that can take it](adr/0029-mcp-for-every-runtime.md) — what each CLI offers, measured
