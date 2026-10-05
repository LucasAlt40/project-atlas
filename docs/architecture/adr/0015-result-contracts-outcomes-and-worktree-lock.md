# ADR 0015 — Result contracts, outcomes and the shared worktree's lock

**Status:** accepted. Extends [ADR 0013](0013-workflow-engine-and-orchestrator.md) and
[ADR 0014](0014-handoff-shared-worktree-and-code-integration.md).

## Why

Real use of V0.9.1 showed four gaps:

1. A validator answered in prose ("the architecture is good, but…") and the workflow could not tell a pass from a fail.
2. What an agent may conclude with was implicit in the personality, not something an agent (or a workflow editor) could state.
3. Whether an agent can edit files looked like a property of the runtime's brand instead of a capability plus a permission.
4. Two steps that write could share the run's worktree at once, mixing their commits and change sets.

## Decisions

**A result contract belongs to the agent.** `Agent.resultContract` is a list of outcomes (`id`, `label`, `description`) with a
`kind` that is only a preset for the editor (`general`, `validation`, `review`, `implementation`, `custom`). The workflow never
sees the kind or the personality: only outcome ids. A personality may carry a `suggestedContract`; it is applied by the editor
when an agent is created and never applies by itself. Agents saved before this exist are `general` (no outcome required).

**Execution status and outcome are different things.** `completed` + outcome `fail` is an agent that did its work and concluded
the work failed; `failed` has no outcome. Each attempt keeps its own outcome (`NodeAttempt.outcome`), so a retry never inherits
one. The outcome reaches conditions as the fact `result.outcome` (`result.status` still works for older edges, and a `pass`/`fail`
outcome fills it when the block gave none).

**The outcome is read only from data, and only a declared id is kept.** The step's prompt carries an OUTPUT CONTRACT listing the
allowed outcomes; the result parser reads the `outcome` field of the `atlas-result` block and checks it against the contract.
Prose is never searched for "pass" or "fail". A contract that requires an outcome and gets none valid ends the step as
**failed** ("No valid outcome…"): it takes the usual road of a failed step (retry, failure route, or the run failing). Nothing is
ever read as pass, success or approved. If an outcome is valid but no edge accepts it, the run fails with `no_route_matched`,
naming the outcome. There is no AI fallback.

**Routes are checked before the run.** An edge on `result.outcome` whose value the source agent's contract does not declare
(or an agent that declares none) is the validation issue `undeclared_outcome`, so the editor flags it live and the run refuses to
start. The connection editor only offers the declared outcomes. ADR 0017 extends the checks to outcomes with
no route, `result.status` routes on an agent with a contract, and routes that cannot reach an End.

**Handoffs carry the structure.** `AgentHandoff.outcome`, the execution status and findings (severity, category, title, file,
line, description, evidence, recommendation) go to the next step as context, clearly delimited, with no authority: they grant
nothing, change no policy and approve nothing.

**Editing is a capability, a permission and isolation.** `RuntimeCapabilities.fileEdit` says a runtime can be launched with
file-editing tools; `PermissionPolicy` says whether this agent may write; the execution must be isolated. Only all three together
enable editing (`ExecutionService::edit_access`). The reasons are distinct (`RuntimeCannotEdit` vs `PolicyDenied` vs
`NotIsolated`) and the UI says "This runtime does not support file editing" rather than "denied". No code names a runtime:
a runtime that gains the capability (and a safe launch for it) gets editing with no architectural change. Claude's
`--permission-mode acceptEdits` stays an implementation detail of that runtime; the authority is the Atlas policy.

**The run's worktree has a lock.** The worktree is one resource of the run. A step that would write (effective edit access, never
a name) needs it exclusively; read-only steps share it. A waiting writer is not overtaken by later readers. The lock lives in the
orchestrator's loop for that run and is reconciled each round against the steps really in flight, so a step that ended in any
way (completed, failed, cancelled, panicked, interrupted by shutdown) cannot keep it. Because a writer is alone, Git's
`revision_before → revision_after` for a step contains only that step's changes.

## Workflow Apply is not a commit

ADR 0014 first described "apply" as the worktree merge. That was wrong for the project's history, and is superseded here:

```text
Workflow Apply
    -> materialises the run's ChangeSet in the project's main working tree
    -> UNCOMMITTED (modified / new / deleted in `git status`; HEAD, refs and reflog untouched)
```

and **not**:

```text
Apply -> git merge -> commit (-> push)
```

- The run's commits (`Atlas: work of …`) live on the run's own branch: history of the run, for traceability and recovery. The
  project receives the _content_ (`git diff <fork>..<branch>` applied with `git apply`), never those commits.
- Apply never commits, merges, rebases, cherry-picks, stages, pushes, changes a remote or changes the project's Git
  configuration. Committing is the developer's.
- Prepare, validate, detect conflicts, apply, verify: the whole patch is checked against the working tree before a file is
  written (`git apply` writes all or nothing); a snapshot of HEAD and of the changed paths is taken first and compared again
  right before writing and after (HEAD must not have moved).
- The user's own uncommitted work is preserved. A file both the run and the working tree have changed (an untracked file
  counts) is a **conflict**: nothing is written, the worktree is kept, and the user decides. A dirty checkout alone no longer
  blocks Apply. A checkout that left the base branch still does.
- After a successful apply the worktree folder is removed and its **branch is kept** as the record of what was applied
  (`MergeStatus::Applied`, shown as `integrated`).
- Proved by real-Git tests: HEAD, reflog, refs, index, `MERGE_HEAD` and a bare remote are unchanged after Apply.

**"Keep isolated" after Apply takes the applied changes back out.** The branch is kept, so the same patch (`git diff
<fork>..<branch>`) is applied in reverse (`git apply -R`), and only that. It is allowed only while the project is on the base
branch at the `HEAD` recorded when the changes were applied (`appliedHead`): with a new commit the changes may be part of
the history, and Atlas never rewrites that. The reversed patch is checked as a whole first; an applied file the user changed
since is a conflict and nothing is written. The user's other changes stay. Afterwards the worktree folder is made again
from the branch (`git worktree add`, no new branch) and the run is `kept_isolated`, so it can be reviewed or applied again.
When it cannot be done the changes stay applied and the run says why (`canUndo`, `blockReason: conflict`, or
`message: project_moved`).

After a successful Apply the screen opens the person's editor (the one last chosen, else the first found) on the project.
None of the supported editors' launchers can open its Git view, so the changes show up in the editor's own Git indicators.

**`merge_execution` is a different operation.** The merge of a _single agent's_ isolated execution (the `merge_execution`
command and the policy-driven automatic merge) is a real `git merge --no-ff` and creates a merge commit on the base branch. It
is the user's explicit "merge", with its own rules. It is **unchanged by this decision**; whether it should also become a
working-tree apply is a separate architectural decision, still open. Both paths are chosen by the kind of worktree
(`WorktreeService::delivers_to_working_tree`): a workflow's worktree can never be merged into history, whichever command
reaches it.

## Out of scope

Agents still run no shell, tests or commands, and have no network. That is a separate milestone.
