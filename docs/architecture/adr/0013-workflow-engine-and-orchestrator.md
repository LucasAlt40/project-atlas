# ADR 0013 — Workflow Engine and Orchestrator

**Status:** accepted. Builds on [ADR 0009](0009-git-worktree-isolation.md) (isolation),
[ADR 0010](0010-project-harness.md) / [0012](0012-task-aware-context.md) (context) and
[ADR 0003](0003-command-permissions.md) / [0007](0007-live-shell-and-process-control.md) (permissions and processes).

## Why

Until V0.8 the unit of work was one agent doing one task:

```text
Task ─▶ Agent ─▶ Execution
```

Real work is a process: someone designs, someone builds, someone checks, someone fixes what the checks found, and the
checking runs again. V0.9 makes that process a first-class, visible, editable thing, **without** giving anything new the
power to run processes:

```text
Task ─▶ Workflow ─▶ Workflow Engine ─▶ Orchestrator ─▶ ExecutionService ─▶ RuntimeRegistry ─▶ ModelRuntime
```

Philosophy: **an AI can suggest and do work; the Workflow Engine controls the process.**

## Who does what

| Part                 | Is                                                         | Knows nothing about                      |
| -------------------- | ---------------------------------------------------------- | ---------------------------------------- |
| **Workflow**         | the declarative graph (nodes, edges, conditions, policies) | how it is drawn, how it runs             |
| **Workflow Engine**  | a pure state machine over a run: what is ready, what next  | clocks, threads, storage, runtimes       |
| **Orchestrator**     | drives a run: starts steps, hears how they ended, persists | what the graph means (asks the engine)   |
| **ExecutionService** | one agent execution (unchanged)                            | workflows                                |
| **Runtime**          | talks to Claude / OpenCode (unchanged)                     | workflows                                |
| **Graph UI**         | one way to draw and edit a Workflow (`@xyflow/react`)      | the engine; it is never read by the core |

- **Workflow is not Execution.** A workflow is a definition that is edited and versioned; an execution is one agent run.
  A _workflow execution_ (a run) is an instance of the workflow and owns many executions. Definitions and runs are kept
  in separate lists (`workflows`, `workflow_executions`) of the same `config.json` (atomic writes; no new database).
- **The Orchestrator is not an Agent.** An agent is a model with a personality; it can be wrong, be prompted into
  anything and has no authority. The orchestrator is deterministic code. Nothing an agent writes reaches the graph:
  see _Authority_.
- **The graph library does not define the domain.** `@xyflow/react` appears in `features/workflow/components` only.
  `features/workflow/model/graph.ts` is the single translation between the domain `Workflow` and what the library draws;
  the persisted model has plain coordinates and its own ids. Rust has never heard of it.
- **There is no agent-to-agent communication.** Steps do not talk to each other. They leave artifacts, decisions and
  results in a shared, structured state, and the orchestrator hands each step what it needs. (A conversation between
  agents would be unbounded, expensive, unreproducible and would make "who decided this" unanswerable.)

## Graph semantics

- **Node** — a unit of work: `agent` (a run of an existing Agent: the node stores only `agent_id`, its own
  instructions, retry, failure and loop policies — never the personality, runtime or model), `condition` (tests the
  previous result and records `result.matched`), `end` (a terminal: done / failed / cancelled).
- **Edge** — a transition, taken when its source completes and its optional **condition** holds for the source's result.
  A **failure route** (`FailurePolicy::RouteToNode`) is also a transition; the engine treats it as a link like an edge.
- **Condition** — `{ field, operator, value }` over a closed set of fields (`result.status`, `result.summary`,
  `result.next_action`, `result.findings`, `result.matched`, `validation.status`) and four operators (`equals`,
  `not_equals`, `exists`, `not_exists`). No expressions, no code. Validation rejects an unknown field.
- **Artifact** — a shareable output of a step: type, name, producer node, execution, a _relative path_ and a summary. It
  references the work; it does not copy it. **Decision** — knowledge later steps must respect (title, decision,
  rationale, source node).
- **Workflow Execution** — one instance, holding a **snapshot** of the workflow version it started from.

### Activation, joins and loops

A node becomes ready when it is _activated_ by the links that fired towards it. Links from outside a cycle are
**requirements** (unconditional edges: all must have fired) or **alternatives** (conditional edges and failure routes:
any one will do); either group is enough. Links _inside_ a cycle (both ends in the same strongly connected component)
are always alternatives: any one re-activates the node. That is what lets a join wait for two parallel branches while
`QA → Bug Fixer → QA` does not deadlock. A link out of a step that failed and was handed to its failure route is ignored
by joins (it will never fire).

Loops are allowed but **every cycle must be bounded**: validation removes the nodes that carry a loop limit
(`loop_policy { loop_id, max_iterations }`) and rejects the graph if a cycle remains. A node may _start_ at most
`max_iterations` times; the next start fails the run with `max_iterations_reached` (loop id and node recorded).
`QA → Bug Fixer → QA` and `Validator → Bug Fixer → Validator` are two distinct loops with distinct limits.

### States

Node: `pending ▸ ready ▸ running ▸ (waiting_approval) ▸ completed | failed | cancelled`, plus `blocked` (a dependency
can no longer be satisfied) and `skipped` (the conditions routed around it). Run: `running`, `paused`, `completed`,
`failed`, `cancelled`, `interrupted`. A run ends when nothing is in flight and nothing is ready; it is `completed` only
if an End node that means success was reached and nothing failed unhandled. A step whose result matches none of its
routes fails the run with `no_route_matched`: **it is never guessed at, and never read as a pass.**

### Failure, retry, pause, cancel, approval

- **Failure.** A failed step is retried up to `retry_policy.max_retries` (≤ 5), each attempt with its own execution id
  and the failed ones kept. Then its `failure_policy`: `stop_workflow` (dependents are `blocked`; the run fails once
  nothing else can run) or `route_to_node`. The run is not failed while a failure route exists.
- **Pause** stops _starting_ steps. A step already running is left to finish (and its result is kept); nothing new
  starts until resume.
- **Cancel** cancels what has not started, asks the running steps' processes to end (the same control the user has in
  the terminal, through `SessionRegistry`), keeps everything that completed, and ends the run as `cancelled`.
- **Approval.** A step is `waiting_approval` only when the _real_ approval mechanism says its execution has a pending
  approval (`ApprovalBroker::pending`). The workflow offers no approve button and reads nothing an agent says as one:
  the user answers in the existing security panel. Dependents stay `pending`; the step goes back to `running` when the
  approval is answered.
- **Concurrency.** At most `max_parallel_steps` (4) run at once; an agent never runs two steps at once (the existing
  per-agent, per-workspace busy lock is the one used: a second step of a busy agent simply waits). Ready nodes are
  ordered by priority, then creation order, then id, so the same history gives the same schedule.

## Context, handoff and results

- **Task context is reused, not duplicated.** For a step, the instruction handed to `ExecutionService` is: the node's
  instructions, the overall task, a **WORKFLOW CONTEXT** block (workflow, current step and pass, previous steps,
  artifacts, decisions, the failing reports the step must act on, next steps) and a **RESULT PROTOCOL**. What the Task
  Context is _selected for_ is a separate `context_query` — the task plus this step's label and instructions — so a
  Backend step and a Frontend step of one task get different parts of the Harness. The Harness text itself is added by
  the execution layer as always; the workflow never adds its own copy.
- **Handoff is by artifact, not transcript.** A later step does not receive earlier agents' text. It receives artifacts
  (name, path, summary), decisions and the latest failing validation entries (severity, category, evidence,
  recommendation). Bounded in size.
- **Structured result.** A step may end its answer with an `atlas-result` JSON block (status, summary, artifacts,
  decisions, findings, touched files/areas, next action). Without it Atlas keeps a textual summary and the status is
  `unknown`, which no `pass`/`fail` edge matches. Paths in a result must be relative and cannot leave the project;
  anything else is dropped.
- **Overlap.** Touched files and areas are recorded per node. Two steps that could run side by side (neither leads to
  the other) touching the same place raise `workflow:overlap_detected` and a warning in the shared state. Detected and
  shown; never resolved.

## Authority — what a workflow cannot do

A workflow gains **no** new authority. It cannot read or write files, run a shell, open the network, approve anything or
change a policy. The chain is unchanged:

```text
Workflow ─▶ Orchestrator ─▶ ExecutionService ─▶ Runtime ─▶ PermissionPolicy ─▶ ProcessRunner      (never: Workflow ─▶ Process)
```

- Each step is an execution like any other through `ChatService` (`send_workflow_step`): the agent's worktree policy
  (isolation stays the agent's, on by default; a node can only inherit it, never weaken it), the Harness, the
  runtime registry and the guard all apply. If the agent needs a worktree and the project is not a Git repository the
  step **fails**; it never falls back to the checkout.
- The engine does not know Claude, OpenCode, PTYs or `ProcessRunner`; it sees the `StepRunner` port.
- **What an agent says never changes the workflow.** `next_action` is information. Routing is decided only by the
  conditions the workflow defines, evaluated on the closed set of facts above; a result cannot add a node, edit an
  edge, grant or deny an approval, or touch a policy. (There is a test with a result block that tries all of it.)
- No Tauri command takes a program, a path or an approval; `build.rs` and the capability file list each workflow command.

## Versions, snapshots, editing, recovery

- A definition has a `version`, incremented by any **structural** change (nodes, edges, policies); moving nodes, the
  viewport and the name are not. A run records the version and holds a full **snapshot**: editing the definition never
  changes a run in progress or a finished one.
- While a workflow has a run in progress (running, paused or interrupted) its definition cannot be edited or deleted; the
  UI says so and offers to wait or to save a copy. One run at a time per workflow.
- If the app closes during a run, nothing is resumed on its own and nothing is called finished: on the next start the
  run is `interrupted` (recoverable), its in-flight attempts are recorded as `interrupted`, and the user can **resume**
  (the cut-short steps start again as new attempts of the same pass), **restart** (cancel and run again), **cancel** or
  inspect it.

## Automatic and Custom

Both use the same `Workflow` model; only the author differs. **Automatic** picks a template with fixed rules
(`select_for_task`: refactor / review / bug words, else feature), builds the workflow from the workspace's agents by
personality, and shows it. **Customize** changes `mode` to `custom`; the graph is then the user's. A template never
creates an agent: roles with no agent are reported and left empty for the user to fill. Templates live in
`application/workflow/templates.rs`; a future planner would produce a `Workflow` the same way and hand it to the same
engine (after human approval). **Not implemented here:** no LLM planner, no adaptive routing, no agent-to-agent chat.

## Roles, as the personalities say them

- **Architect** — what must be built, and how?
- **Developer** — how do I implement it?
- **Architecture & Code Validator** (read-only) — was it built _correctly_, within the architecture, decisions, Harness
  and conventions?
- **QA** — does it _work_?
- **Bug Fixer** — why did it fail, and how do I correct it (root cause, minimal change, evidence for re-validation)?

## Events

One Tauri event per kind, under `workflow:*`: `started`, `paused`, `resumed`, `completed`, `failed`, `cancelled`,
`interrupted`, `node_ready`, `node_started`, `node_waiting_approval`, `node_approval_resolved`, `node_completed`,
`node_failed`, `node_blocked`, `node_skipped`, `node_retrying`, `artifact_created`, `decision_created`,
`overlap_detected`. They carry ids, not state: the UI re-reads the run. Steps' own progress and messages use the
existing `execution:progress` and `conversation:message` events, so the agent cards stay live. The event log of a run is
kept inside the run (the newest 400).

## Known limits

- ~~Each isolated step had its own worktree~~ and ~~touched files were what the agent reported~~: both are changed by
  [ADR 0014](0014-handoff-shared-worktree-and-code-integration.md) (one shared worktree per run; files measured by Git).
- A step with a process can be stopped on cancel; a runtime without a terminal session finishes on its own.
- `max_parallel_steps` is fixed at 4 (stored per run; not yet configurable).
