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
start. The connection editor only offers the declared outcomes.

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

## Out of scope

Agents still run no shell, tests or commands, and have no network. That is a separate milestone.
