# ADR 0031 — Plans wait for the person (or for their standing approval)

Status: accepted. Builds on [ADR 0016](0016-human-in-the-loop.md); changes none of its invariants.

## Context

Every agent is already told that a plan is a document to be shown in full (the plans rule), and a workflow step can already stop and ask a person
(ADR 0016, `PendingInteraction` with a `document`). Nothing made a plan a _gate_: an agent could write one and go on to carry it out, and the
person had only yes/no buttons for an approval, with no way to say what to change.

## Decisions

1. **A plan is an interaction of its own kind** (`InteractionKind::Plan`). The agent ends its message, after writing the plan in full, with
   `{"type":"plan"}` in the `atlas-interaction` block (no question needed). The plan is the text before the block and is kept as the interaction's
   document. The instruction is part of every workflow step's brief, so it holds for any agent, built-in or created by the user, on any runtime.
   Conversations are not affected: there the person just replies.
2. **Three answers, fixed by Atlas**: `approve`, `revise` (the person's words are required: "change it" without saying how is refused) and `redo`
   (a note is optional). An agent cannot add a button. Neither `revise` nor `redo` is a refusal; the next pass is told which one it got and
   to present the plan again in full instead of carrying anything out (`HUMAN INPUT` block). Approval tells it to carry the plan out, in the run's
   own worktree, and grants nothing.
3. **The run waits** exactly as ADR 0016 describes (`WaitingForInput`, nothing new starts, the question is kept with the run and survives a
   restart) until one of the three answers arrives.
4. **Automatic approval is the person's standing instruction**, never an agent's. `WorkflowExecution.auto_approve_plans` is set when the run is
   started (`start_workflow(autoApprovePlans)`) or later (`set_auto_approve_plans`, which also approves plans already waiting). With it on,
   the orchestrator answers a plan `approve` itself and marks the interaction `auto`, so the trail and the plans panel say "approved
   automatically". It answers only plans: a clarification, permission, runtime confirmation or guardrail question still waits for a person. The
   screen remembers the last choice on this machine and offers it for each new run.
5. **Plans stay readable**: the run's dock lists every plan with what was decided about it (waiting, approved, approved automatically, changes
   requested, redone).

## Limits

- Detection is the agent's cooperation, like every structured interaction: an agent that writes a plan and does not end with the block is not
  stopped. The text heuristic of ADR 0016 is not extended to guess plans.
- A run started before this change has no setting: it reads as off.
- No real model was asked to follow the new instruction; the behaviour is covered with scripted runners.
