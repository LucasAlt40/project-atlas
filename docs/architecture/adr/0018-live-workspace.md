# 0018 — Live Workspace: observing a workflow run's worktree while it works

## Context

A workflow run's code lives in one isolated worktree ([ADR 0014](0014-handoff-shared-worktree-and-code-integration.md)). Until the
run ended, the user saw status, timeline and handoffs, but not the code being written; the changes were only measured at the
end. To follow an agent without opening an IDE, Atlas must show what is _really_ in that worktree, as it changes. This ADR
covers the backend; the UI is a separate milestone.

## Decision

**Objective.** Keep, for each running workflow, a derived state of "the files of the run's worktree as they are now, compared with
where the run started", and tell the UI when it changes. It exists without any UI: a command can read it at any time.

**The target is the run's isolated worktree, never the project.** The folder is derived by Atlas from what it stored for the run
(`WorktreeService::live_target`): a workflow's _primary_ worktree (not a lease, not an ordinary execution), inside Atlas's worktree
folder, on the run's branch. Anything else is `NotFound` or `Invalid`. The project's own checkout is not observed and not written;
a test writes in it and checks the state does not move.

**Source of truth: Git against `base_commit`.** Every file is compared with the commit the workflow started from
(`baselineRevision`), never with the project's current `HEAD` and never with the agents' own commits (a step's saved work moves
`currentRevision`, not the changes). The state is read through `WorktreeManager::working_changes`: tracked changes (saved or
not) and new untracked files, with added/removed lines. Nothing comes from what an agent says it did.

**Observation: OS events first, polling as fallback.** `notify` (FSEvents, inotify, …) behind the `WorktreeWatcher` port reports
_where_ something happened; it reads nothing. If the platform cannot watch the folder, the state says `observation: polling` and the
worktree is looked at on a schedule that starts at 500 ms and doubles up to 5 s while nothing changes. With events working, a full
comparison still runs every 10 s (run `running`) or 30 s (otherwise) as a safety net.

**Debounce / coalescing.** An event never means "final state". Events collect into a batch that is read once no event has arrived for
150 ms, and at the latest 1 s after the batch's first event, so a continuous writer still shows progress and a file saved many
times is read once. Events for `.git` and for paths outside the worktree are dropped.

**Incremental and full reconciliation.**

- _Incremental_: only the paths of the batch (a folder stands for what is under it) are compared with the baseline and replace what
  the state had for them. A path that no longer differs leaves the state.
- _Full_: the whole worktree is compared and replaces the state. It happens when observation starts (so changes that already exist, as
  when a run is resumed, are read), when the run enters `waiting_for_input`, `idle` or `cancelled`, when it ends, on an explicit
  refresh, on the safety/polling schedule, and whenever the incremental path cannot be trusted: more than 200 paths in a batch, a
  lost or overflowed event queue, Git failing on a path, or a rename involved. Exact-content renames (a deletion plus an identical new
  file) are paired only on a full read.

**Lifecycle.** `idle` (worktree exists, no step working) → `running` (a step works in it) → `waiting_for_input` (a step stopped to ask
a person) / `cancelled` (the step was cancelled) → back to `running` with the next attempt → `ended` (the run completed) or `stopped`
(it failed or was cancelled). The phase only sets how often the worktree is looked at when nothing is reported, and what the UI says;
the watcher keeps running while a step waits, so an edit made during the wait is seen. `ended` / `stopped` are final: the run is read
one last time (before `close_shared`, while the worktree certainly exists), observation stops, and that state stays readable. The
final state equals the run's change set (tested). A run being resumed starts observation again on the same worktree and baseline, and
`revision` continues.

**Isolation from the agents' Git.** The observer only reads: Git runs with `--no-optional-locks`, it takes no lock the manager's write
operations hold, and it never commits, stages, merges or writes in the worktree. A test runs it against 40 writes and commits by an
"agent" without a failure.

**Events and state: snapshot + delta.** `get_live_workspace(executionId)` returns the snapshot (`LiveWorkspaceState`);
`refresh_live_workspace(executionId)` forces a full comparison. Changes arrive as `live_workspace:changed`
(`LiveWorkspaceUpdate`): `changed` files, `removed` paths, and `full: true` when `changed` is _every_ file. `revision` is monotonic
and counts state changes: a client holding revision N applies an update with N+1; **any other number (a gap, a repeat, an older one)
means "read the snapshot again"**. Updates are best effort, like every other event. The commands take the run's id (the same
`executionId` as `get_workflow_changes`); the webview never names a path.

**Security.** The only way a folder is chosen is the stored worktree record. Paths given to Git are relative, free of `..`, and
passed with `--literal-pathspecs` after `--`; refs and commits are ones Atlas derived. No shell. Both commands only read. The frontend
has no filesystem access; it receives only the models above.

**When the worktree disappears.** If the folder is gone (applied and removed, discarded, deleted by hand) the state becomes
`availability: missing` (`invalid` if it is no longer what Atlas made, e.g. its branch was switched), `observation: stopped`, and
the observer ends. The last known files are kept: they are the last thing known. For a run whose worktree cannot be read at all, the
snapshot is built from the record (its stored change set) with `missing`.

**No `activeFile`.** Atlas has no reliable signal for which file an agent is editing: the runtimes do not report it, and "the file
that changed last" would be a guess presented as a fact. The state has none; the UI should say the agent is working and show the
files as they really change.

**State is in memory.** It is derived, so nothing is persisted. A finished run's last state is kept in memory for the session.

## The viewer (UI)

Two read-only commands serve the viewer, both by `executionId` (the run) and a relative path, never a folder:
`get_live_file` (the file as it is now) and `get_live_diff` (the worktree as it is now against `baselineRevision`, of one file
or of everything, new files included). A path must be plain (relative, no `..`, no `.git`); a link is reported and never
followed, and a link in a folder of the path is refused. A deleted file is reported as `deleted`, not as an error; a text file is cut at 1 MiB (`truncated`).
After the worktree is gone (changes applied) the screen reads the diff from the saved change set instead and cannot read files.

The screen applies updates by `revision` (only the next one; older ones are dropped; a gap reads the snapshot again) and rereads
the selected file only when that file's entry was announced as changed. For this the observer remembers each file's size and
modification time and announces a file whose text changed even when its line counts did not.

## Limits (current)

- At most 500 files are listed (`MAX_CHANGESET_FILES`); beyond that the list is cut.
- Untracked-file line counts are read only for text files up to 1 MiB; larger files are listed without counts.
- Renames are recognized only on a full read and only for identical content.
- A writer that pauses longer than the quiet window can be read mid-way; the next event, or the safety look, corrects it.
- Contents and diffs are not part of this state (only paths, status and counts): they are read on demand from Git by later work.
- **Known limitation: after the app restarts, a run that already exists can be queried (a snapshot is a single look at its worktree),
  but observation is not reactivated automatically** until the run is resumed. Recovery and persistence of the Live Workspace are a
  possible future evolution.

## Consequences

- One new dependency, `notify`, behind a port; tests drive the observer with a scripted watcher or the real one.
- The runner tells the observer the run's phase; the orchestrator, the engine and the worktree service do not know it exists.
- It does not touch Apply ([ADR 0015](0015-result-contracts-outcomes-and-worktree-lock.md#workflow-apply-is-not-a-commit)) or
  `merge_execution`.
