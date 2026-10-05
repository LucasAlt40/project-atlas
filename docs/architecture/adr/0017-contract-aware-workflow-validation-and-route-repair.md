# 0017 — Contract-aware workflow validation and assisted route repair

## Context

A real run ended as `failed` although every step did its job. QA declared the outcomes
`approved` / `changes_requested` (a `review` contract), while the saved workflow routed QA with
`result.status = pass` / `fail`. No edge could match `approved`, so the run stopped with
`no_route_matched`. The engine behaved correctly: the definition was wrong, and nothing told the
user before the run. The one migration that exists (ADR 0015) retargets `result.status` edges to
`result.outcome` only when the agent declares that very value, so it rightly did not touch QA.

## Decision

**Four things stay apart.** The _agent contract_ says which outcomes an agent may produce. The
_workflow_ says what to do with each. An _End node_ says when the workflow is done. The
_execution status_ is the technical state of the run. There is no "terminal outcome": `approved`,
`pass`, `fail` and `changes_requested` are not terminal in themselves; a route to an End node is
what finishes a workflow. No step ending, and no missing route, ever completes a run.

**`result.status` is not `result.outcome`.** `result.status` says whether the step ran
(`success` / `failed`). `result.outcome` is the verdict the agent declared. Only `pass` and `fail`
are mirrored into `result.status`, so that edges saved before outcome routing keep working for an
agent that declares them. Everything else about meaning is read from `result.outcome`.

**Validation compares contract and routes before the run** (`application/workflow/validation.rs`),
with a severity per issue; only errors keep a run from starting:

| Issue                      | Severity | When                                                                                               |
| -------------------------- | -------- | -------------------------------------------------------------------------------------------------- |
| `outcome_without_route`    | error    | The agent declares an outcome no edge out of its step would take, while other edges exist          |
| `outcome_without_route`    | warning  | The step has no edge at all (it is also a `dead_end`)                                              |
| `status_route_on_contract` | error    | An edge tests `result.status = X` for an agent with a contract, and `X` is not a mirrored outcome  |
| `undeclared_outcome`       | error    | An edge tests an outcome the agent does not declare (ADR 0015)                                     |
| `cannot_reach_end`         | error    | An edge leads into a part of the graph from which no End node can be reached (from the graph only) |

**A rule of the workflow engine, not a technical consequence (pending confirmation).** Refusing to
run an agent that declares an outcome which has no route, while other outcomes of it are routed, is
a decision about the _semantics_ of workflows: that every verdict an agent may give must lead
somewhere. Nothing technical forces it. The engine already handles a missing route at run time
(`no_route_matched`), and the check could as well be a warning, or an error only for outcomes the
workflow marks as required. It is an error for now, and is not final: the rule is to be confirmed
before it is relied on. Changing it only changes the severity chosen in `check_outcome_routes`.

An unconditional edge takes every outcome. An edge on another fact counts as possibly taking it:
validation reports what surely has no route, not what might. Nothing names an agent, a
personality or an outcome.

**`no_route_matched` is unchanged.** Were a definition to run anyway (tests start one without
validation on purpose), an outcome with no route fails the run deterministically, naming the step
and the outcome. Never completed, never a guessed edge, never an assumed End.

**Repair is proposed, confirmed and recorded** (`application/workflow/repair.rs`). Atlas does not
infer that `pass` means `approved`: `pass` may be a technical check and `approved` a review.
Validation can detect the mismatch; only the user can say what it was meant to be. So:

1. `suggest_route_repairs` lists the incompatible edges of each step with a _suggestion_ built only
   from the shape of the workflow (the edges in the order drawn, the unrouted outcomes in the order
   of the contract). It changes nothing.
2. The user sees current and suggested side by side, may choose another outcome per edge, and
   applies or cancels.
3. `repair_workflow_routes` re-checks every choice against the saved workflow (a stale or unknown
   choice changes nothing), rewrites the confirmed edges to `result.outcome`, makes a new workflow
   version, and appends a `RouteRepair` record (edge, step, agent, target, previous condition, new
   condition, version, time) to `Workflow.route_repairs`. The history lives on the workflow, next
   to the versions it explains; there is no new audit system, and the permission audit
   (`AuditLog`) stays about permissions. `update_workflow` never lets a caller rewrite that
   history.

Repair covers every route that waits for a result its agent cannot give: a `result.status` test, or
an outcome the contract does not declare.

**Templates follow the contract of the agent they are given.** A default workflow may never be born
broken. Where a template has a step that judges (validator, QA), `Builder::gate` routes it on the
pair declared in full by the agent it got: `pass`/`fail` (the `validation` preset) or
`approved`/`changes_requested` (the `review` preset). These pairs are Atlas's own presets, not a
guess about wording. An agent declaring neither pair (a general or custom contract) gets
`pass`/`fail`, and validation says what it lacks, since no pair can be chosen for it. A test builds
every template against every combination of the two presets.

Opening the app, validating and saving never repair anything, so the migration stays idempotent.

## Consequences

- A workflow like the one in the bug is refused with a message that explains status against
  outcome and names the agent's declared outcomes; the editor offers "Review routes".
- Warnings are listed apart and do not block a run.
- Workflows that were valid by accident (an agent with a contract and a partial set of routes) are
  now refused until the missing outcome is routed. This follows from the semantic rule above, which
  is still to be confirmed.
- Resuming a run that stopped for want of a route, once the routes are repaired, can lead straight
  to an End; the run then completes at once and its code is looked at as for any run that ends.
