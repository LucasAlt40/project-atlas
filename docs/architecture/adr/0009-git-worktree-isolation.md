# ADR 0009 — Git worktree isolation

**Status:** accepted

## What it is

An agent has `worktreeIsolation` (on by default, also for agents saved before it existed). When it is on, **each execution
works in its own Git worktree and branch** and never in the project's checkout:

```
Agent (worktreeIsolation)
 └─ Execution A ── worktree A (atlas/exec-000101)
 └─ Execution B ── worktree B (atlas/exec-000102)

ExecutionService → WorktreeService → WorktreeManager (port) → git
                 → Runtime(workingDirectory) → ProcessSession → PTY
```

The runtime is handed a working directory and nothing else: there is no worktree code in `ClaudeRuntime` or
`OpenCodeRuntime`. With isolation off the working directory is the project, as before.

## Pieces

| Piece                                                 | Responsibility                                                                                                                                  |
| ----------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `application/worktree` — `WorktreeManager` (port)     | `is_git_repository`, `inspect_base`, `create`, `get_status`, `get_branch`, `get_diff`, `commit_all`, `assess`, `base_ready`, `merge`, `remove`. |
| `infrastructure/git_worktree.rs`                      | The only code that runs `git` for worktrees. Argument lists, never a shell; hooks off; time-limited; one writer at a time.                      |
| `application/worktree/layout.rs` — `WorktreeLayout`   | The only place a branch name or a path is derived from an id.                                                                                   |
| `application/worktree/service.rs` — `WorktreeService` | Lifecycle: `prepare`, `finalize`, `merge`, `cleanup_pending`, `recover_interrupted`; the decision rules (`decide`); persistence.                |
| `domain/worktree.rs` — `ExecutionWorktree`            | What is stored: ids, `baseBranch`, `branchName`, `worktreePath`, `status`, `mergeStatus`, `blockReason`, `recommendation`, measured `changes`.  |
| `ExecutionScope.isolated` + `SecurityService`         | The guard holds an isolated execution to **its worktree**, found in the stored record, only while the execution is active.                      |
| `commands/worktree.rs`                                | `list_execution_worktrees`, `merge_execution(workspaceId, agentId, executionId)`.                                                               |

## Where and how

- **Place:** `<app data>/worktrees/<workspace id>/exec-NNNNNN`, branch `atlas/exec-NNNNNN`. Outside the project on purpose:
  nothing is added to the user's repository or its ignore rules, the agent cannot reach the main checkout through a parent
  folder, and cleaning up is removing one folder. Both names come from the execution number (`exec-42` → `exec-000042`);
  ids that are not `exec-<digits>` are refused, so no caller-supplied string becomes a branch or a path.
- **Base:** whatever branch is checked out in the project at that moment (`main`, `master`, `release/1.0`…), recorded with
  its commit. No fetch, pull, rebase or any remote operation. Detached HEAD or an empty repository fails with a clear error.
- **Not a Git repository:** the execution **fails** (`git_repository_required`); there is no fallback to the checkout and Atlas
  never runs `git init`. The user adds Git to the project, or turns isolation off for that agent.
- **Uncommitted changes in the checkout:** never touched (no reset, clean or stash). They are not in the worktree (the agent
  starts from what is committed), which is recorded (`baseDirtyAtStart`) and shown. They also block a merge into that checkout
  until the user deals with them.
- **The prompt** names the worktree as the project's path, not the checkout.

## Finishing

`finalize` runs when the runtime returns. It saves the work with a commit on the execution's own branch (hooks off, unless the
policy denies Git writes), measures it with Git (files, commits ahead/behind, a trial `merge-tree` that touches no checkout) and
decides, in this order (`decide`, one function):

1. worktree not what Atlas made (other folder, branch switched) → blocked, left alone;
2. nothing changed → nothing to merge; the empty worktree and branch are removed;
3. execution failed / cancelled / cut short → blocked; **kept**, never merged;
4. changes that could not be committed → blocked;
5. validation failed → blocked (Atlas runs no tests yet: validation is `not_run`, which does not block);
6. Git says conflicts → `conflict`; branch and worktree kept; the checkout is untouched;
7. policy denies Git writes → blocked;
8. checkout dirty / left the base branch → blocked (the user can ask again later);
9. policy `git.write = approval_required` → `pending` (recommendation: merge, or review if the base moved);
10. policy `git.write = allowed` → merge now (`--no-ff`), then remove the worktree and delete the **merged** branch.

The policy is the agent's own resolved `git.write` (global maximum ∩ workspace ∩ profile). "The model said merge" is not
authorization: a model's text never reaches this code. For `approval_required` the authorization is the user pressing the
button in the Execution Inspector, which calls `merge_execution`; it re-checks everything and does not override a policy that
denies. A conflict found at merge time is undone with `git merge --abort` (only a merge Atlas started has a `MERGE_HEAD`).

## What Atlas never does

reset, clean, stash, `--force` on anything, delete a branch that is not fully merged, remove a worktree Git considers unclean,
remove a path that is not exactly the one the layout derives for that execution (and not a symlink), reuse a branch or folder,
resolve conflicts, fetch/pull/push, run a repository's hooks, or run git through `sh -c` / `cmd /c` / `powershell`.

## Persistence

`UserConfig.worktrees` holds metadata only (no diff, no output), written when the worktree is **created** so a crash cannot
lose it. At start-up executions that never ended are marked failed/blocked and kept; merged-but-not-removed worktrees are
retried; new execution ids continue after the highest stored one **or** any worktree still on disk.

## Known limits

- Git runs through `std::process`, not through the guard: the guard is for what an agent asks. The manager is the single
  place that does it and is constrained as above. The guard still decides what an isolated execution may do, with the
  worktree as its project.
- A runtime's own tools are not sandboxed (ADR 0006): an agent that writes to an absolute path outside its worktree is not
  stopped by Atlas. The prompt no longer tells it the checkout's path.
- Submodules, sparse checkouts and `git worktree` repositories with unusual layouts are not specifically handled.
- Windows and Linux paths follow the same code but are not validated yet.
- A worktree per execution is a full checkout; a default-isolated read-only chat pays for it (an unchanged worktree is
  removed right after).
