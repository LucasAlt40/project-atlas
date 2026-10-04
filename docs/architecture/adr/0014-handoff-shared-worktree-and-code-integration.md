# ADR 0014 — Handoff, the run's shared worktree and code integration

**Status:** accepted. Extends [ADR 0013](0013-workflow-engine-and-orchestrator.md) and
[ADR 0009](0009-git-worktree-isolation.md).

## Why

V0.9 ran steps, routed their results and said "completed". Three things were missing, and the third was worse than it looked:

1. **The communication between agents could not be seen.** What one step produced for the next was spread over state and
   prompts, with nowhere to inspect it.
2. **Steps did not share code.** Each isolated step got its own worktree from the base commit, so a Validator opened a
   tree without what the Developer had written.
3. **"Workflow completed" was read as "code delivered"** — and could not even be true: _no Atlas agent could write a file._
   By design since V0.3 the Claude runtime ran with `--tools Read,Grep,Glob`, OpenCode with `--agent plan` (edit denied) and
   the prompt told every agent it was in "read-only mode". A Developer step completed having changed nothing.

## Handoff

`AgentHandoff` (`domain/workflow/handoff.rs`) is what one step hands to the next **over one transition**: the edge (or
`failure:<node>`), the sending execution and pass, the step's `status` and `summary`, its `decisions`, the `artifacts` it
made, the files **Git** saw it change (`changed_files`) and any not yet committed, what it merely _claimed_ to have touched
(`reported_files`, kept apart), the `validation` it gave (status, findings) and, for a failure route, the failure. What a
step did not say stays empty and its status stays `unknown`; nothing is invented.

- Built by the orchestrator (`orchestration/handoff.rs::build_handoff`) from the step's `AgentResult` and from Git, right
  after the engine routes (`WorkflowEngine::taken_links`: the edges whose conditions the result met, or the failure route).
- Stored in `WorkflowExecution.handoffs` (same `config.json`, same atomic writes; they survive a restart with the run).
- **Given to the next step**, in the brief (`orchestration/brief.rs::handoff_block`), between `## WORKFLOW HANDOFF` and
  `END WORKFLOW HANDOFF`, with a sentence saying it is context and grants nothing. For each earlier step only the latest
  handoff along its transition is sent. Text inside is _quoted_: no line starts at the left edge and the block's own
  markers are defanged, so it cannot close the block or pose as the prompt's structure. A handoff changes nothing else: it
  has no path to permissions, policy, approvals, runtime or the workflow definition (tests try each).
- Not a chat: the orchestrator remains the only owner of the state.
- UI: nothing on the canvas. Click a **connection** to read what travelled over it; a node has _Input_ and _Output_ folds;
  the run has a _Handoffs_ tab.

## The run's shared worktree

When a workflow has agents that work in an isolated worktree, the run gets **one** worktree for all of them, made through
the existing `WorktreeService::prepare` and marked as the run's (`workflow_execution_id`). Each step gets a **lease**
(`WorktreeService::attach`): a record with the step's own execution id, the same folder and branch, `shared_with` pointing
at the primary. Leases exist because the security layer finds an isolated execution's folder by its execution id.

- A step ending commits what it left (`finish_step`), so the next step starts from a clean tree and the work is
  recoverable; the lease records the commit it ended at, and `step_delta` is the diff of that range, by Git.
- A lease is never merged or removed on its own. The primary is decided once, when the run ends (`close_shared`), and
  **never merged automatically**, whatever the policy: the user decides what enters the project.
- Steps whose agent does not isolate work in the checkout as always, and the run then has no code to integrate
  (`not_applicable`). If no worktree can be made (not a Git repository) steps meet that themselves, as before.
- A run cut short by a restart keeps its worktree (recorded as interrupted, never merged); resuming it **reopens** the same
  one (`reopen`), so no second worktree appears; cancelling it ends it like any cancel.

## ChangeSet

`ChangeSet` (`domain/worktree.rs`) is measured by Git and never by an agent: base and current revision, files with status
(added / modified / deleted / renamed, with the old path), lines added and removed, binary files, and uncommitted paths. The
diff text is not stored; it is read from Git when asked for (`diff_between`, bounded). Revisions must be full commit ids and
a file name must be a plain relative path (no `..`, no leading `-` or `:`, no globs): nothing a caller sends becomes an
option or pathspec magic. After the changes are applied and the worktree is gone, the diff is read from the project's own
history.

## Workflow completion is not code integration

`WorkflowExecution.integration` is separate from `status`:

| Status              | Meaning                                                                                                                                                              |
| ------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `not_applicable`    | no isolated worktree: nothing to integrate                                                                                                                           |
| `in_progress`       | the agents work in the worktree; nothing is in the project                                                                                                           |
| `no_changes`        | the run changed no code (the empty worktree is removed)                                                                                                              |
| `changes_available` | the code is in the worktree, **not in the project**; `can_apply` says whether it may be applied (a cancelled or failed run's work is kept for review, never applied) |
| `conflicts`         | applying was tried: Git found conflicts; the attempt was undone                                                                                                      |
| `blocked`           | applying is not possible now (`block_reason`: dirty checkout, branch changed, policy denies, uncommitted…)                                                           |
| `integrated`        | **the only state in which the screen says the code is in the project**                                                                                               |
| `kept_isolated`     | the user chose to leave it in the worktree                                                                                                                           |
| `discarded`         | the user discarded the worktree                                                                                                                                      |
| `failed`            | applying failed for a reason Git gave                                                                                                                                |

`WorkflowExecution.status` never changes because of any of this: a run that completed with a blocked or failed integration
is still a completed run.

**The user's decisions** (`IntegrationService`, reached only by the commands the user triggers; the orchestrator, the runner
and agents have no path to them): **apply** (the existing worktree merge, with all its rules: the agent policy for Git
writes, a clean checkout on the base branch, nothing forced, conflicts reported and undone; the policy is that of the
_strictest_ agent of the run), **keep isolated**, **discard** (confirmed in the UI; removes the worktree folder, unforced; the
_branch is kept_ because Atlas never deletes commits that were not merged, so the work stays recoverable with Git),
**review** (the real diff) and **open in an editor**.

## Opening an editor

`IdeLauncher` (port) / `SystemIdeLauncher`: a fixed list of editors (VS Code, Cursor, IntelliJ IDEA), found by their own
command-line launcher on `PATH` (Windows: `.cmd`/`.exe` names) or, on macOS, as an installed application (`open -a`). The
program is chosen from the list, the folder is one argument of a process started without a shell, and must be an absolute
path to an existing folder. The folder is never taken from the webview or an agent: it is the run's worktree while the code is
only there, the project once applied. Visual Studio is not detected yet.

## Letting agents write files

The root cause above is fixed narrowly, inside the existing security model:

- `RuntimeCapabilities.file_edit` says a runtime _can_ be launched with file-editing tools (Claude: yes; OpenCode: no, still
  `--agent plan`). `RuntimeRequest.allow_edits` says this execution _gets_ them.
- `ExecutionService::may_edit` grants them only when **all** hold: the agent works in an isolated worktree (an edit can never
  reach the project's checkout), the runtime can, and the agent's own policy (workspace ceiling ∩ permission profile) allows
  file writes. Unknown means no. Agents default to the most restrictive profile, so only an agent the user explicitly gave
  the _developer_ profile can write.
- Claude is then launched with `--tools Read,Grep,Glob,Edit,Write --permission-mode acceptEdits`. Still no shell, no web.
  The scope's `runtime_access` reflects what the tools can do _as launched_, which the guard checks against the policy again.
- The prompt says what is true: the editing rule for such an execution (isolated worktree, files only, nothing outside it,
  reviewed before it reaches the project), the read-only rule for every other one.

## Known limits

- Steps of one run that run in parallel share one tree: their commits can mix and the per-step delta can include the other
  step's files. Overlap warnings still apply.
- Only the user applies changes; there is no automatic integration, even for a policy that would allow a merge.
- OpenCode agents remain read-only. Agents cannot run tests or a shell (a different, larger decision).
- The step's changes are committed to the run's branch by Atlas when Git writes are not denied; with a policy that denies
  them the work stays uncommitted and cannot be applied from Atlas.
