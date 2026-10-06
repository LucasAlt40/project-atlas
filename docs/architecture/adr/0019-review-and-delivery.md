# 0019 — Review & Delivery: reading a finished workflow and deciding about its code

## Context

A workflow ends with its code in an isolated worktree ([ADR 0014](0014-handoff-shared-worktree-and-code-integration.md),
[0015](0015-result-contracts-outcomes-and-worktree-lock.md)). The Live Workspace ([0018](0018-live-workspace.md)) shows that code
while it is written and says REVIEW once the run is over, but the user still had no single place to understand _what the agents
did, what they found and decided, what changed, and what Apply will do_ before deciding.

## Decision

**Frontend only.** No backend, Apply, `merge_execution`, orchestrator, observer or contract changed. The Review Workspace
composes what the run already keeps: nodes and attempts (timeline), `state.validationResults` (findings), `state.decisions` /
`state.artifacts`, `handoffs`, `changes` (change set), `integration`, and the Live Workspace snapshot.

**One surface, LIVE → REVIEW.** For a run that is over (`completed`, `failed`, `cancelled`) the page renders `ReviewWorkspace`
instead of the bare live panel; it embeds the same panel (`LiveWorkspaceView`, fed by a single `useLiveWorkspace`) as its code
viewer, so the worktree is watched and read once and the diff is never computed twice. While the run works, nothing changes.

**Kept apart, in the model and on screen.** Execution status (`attempt.status`), outcome (`attempt.outcome`), workflow status,
change set and integration are separate fields and separate words. `reviewStateOf` derives a _review_ status next to the
_integration_ status; the review status is derived (nothing is persisted about it): `Ready for review` until the code is applied,
`Reviewed` after.

**"Integrated" is never shown.** `IntegrationStatus::Integrated` is presented as _Applied to the working tree — uncommitted_.
"HEAD unchanged" is said only while the backend still says the apply can be taken back (`canUndo`, i.e. `HEAD` is where Apply
left it); otherwise the screen says Atlas can no longer tell.

**Findings keep their history.** `state.validationResults` is append-only, so a fixed finding is not erased. A finding is marked
_later validation passed_ only when a later validation of the same step passed (a derived fact, worded as such). The
"finding → fixer → validator" story is the timeline from the first non-passing validation to the end of the run.

**Claimed vs detected.** Handoffs already carried both lists; the screen now shows files the agent claimed but Git did not see
_and_ files Git saw that the agent did not report. Git stays the source of truth.

**Apply needs a confirmation.** The Apply button opens a dialog that says what it will not do (no commit, no push, no merge) and
lists the conflicts of the last attempt; confirming calls the unchanged `apply_changes`. A blocked apply shows the conflicting
files and tells the user to resolve them in their project; Atlas never overwrites, merges or stashes.

**Worktree gone.** When the Live Workspace reports `missing`/`invalid`, the Review says the live content is unavailable, keeps
showing the saved change set and diff, and offers only what still makes sense (no Apply, Keep or Open in IDE for a worktree that is
not there; an already applied run keeps Open in IDE, which opens the project).

**Not added.** Artifacts are shown, not opened (no new file access). No commit, push, PR, AI summary or new persistence.

## Limits

- The review status is not persisted: reopening the app derives it again from the integration status.
- The timeline's runtime/model come from the stored executions; a step whose execution is no longer stored shows only its agent.
- Severity names unknown to the Review are shown as _Info_ (the agent's own word is kept in the finding line).
