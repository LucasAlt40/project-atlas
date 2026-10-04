# 0016 — Human in the loop: an agent waiting for a person

## Context

A runtime like `claude -p` or `opencode run` answers once and exits. When the agent needed a
decision it wrote a question as its answer, the process ended, and Atlas called the execution
`completed`: the workflow went on to the next step with nothing done.

## Decision

**Waiting is a state of the domain, not a log line.**

| Level         | State                                                                                            |
| ------------- | ------------------------------------------------------------------------------------------------ |
| Execution     | `ExecutionStatus::WaitingForInput` — not `completed` (no result), not `failed`                   |
| Workflow node | `NodeStatus::WaitingForInput`, attempt `AttemptStatus::WaitingForInput`; the step has no outcome |
| Workflow run  | `WorkflowExecutionStatus::WaitingForInput`: nothing new starts until the question is answered    |
| The question  | `PendingInteraction` (`domain/interaction.rs`), kept in `WorkflowExecution.interactions`         |

`outcome` stays what the agent's result contract says (`implemented`, `pass`…); "waiting" is never
an outcome. Process state and logical state are different: the process has usually exited, and the
step is still waiting.

### Detection

`ExecutionService` looks at a workflow step's answer (and only a workflow step's: in a conversation
the person just replies). In order, the first that speaks wins:

1. the runtime adapter, `ModelRuntime::detect_interaction` (the only place that may know a tool's own
   protocol; no shipped adapter has one yet);
2. the `atlas-interaction` block every agent is told to end with when it cannot go on
   (`application/interaction/structured.rs`) — works with every runtime;
3. a conservative text analysis of how the answer _ends_ (`heuristic.rs`): the last sentence is a
   question, addressed to the person, and the answer carries no `atlas-result`. Offers of more work
   ("Would you like me to also…") and `?` inside code or mid-answer do not count.

No model is asked to classify output. A detection under `MIN_CONFIDENCE` (70) never pauses. An
`atlas-result` after an interaction block means the agent concluded.

The block is data: a closed set of fields, clipped, a closed set of types. For approval, permission
and runtime confirmation the buttons are Atlas's (`approve/reject`, `allow/deny`, `yes/no`); an agent
cannot invent them.

### Pause, answer, resume

`Orchestrator::handle` turns a `StepStatus::WaitingForInput` into `engine.wait_for_input`: the
question is persisted with the run, no result is parsed, no handoff is built, no route is taken.
`answer_interaction` validates the answer against the question (still pending, one of its options,
bounded text), records it, and sends the node back to `Ready` as a _continuation of the same pass_
(`continuing`, same iteration). The next start is a new attempt/execution whose instruction carries a
`HUMAN INPUT` block (the question and the answer, quoted), because one-shot runtimes keep no
conversation; the files already written are still in the run's worktree.

A denial is not a completion either: the agent is told it was declined and ends under its own
contract (`blocked`, `fail`…).

An answer to a run nobody drives (the app was closed since it asked) is recorded and the run is
picked up with `resume_interrupted`. A run interrupted while waiting keeps its question.

Cancelling the run lapses the question (`cancelled`) and cancels the node.

### Worktree lock

A step that is waiting keeps its place in `WorktreeLock` (it is still "in flight"), and a step about
to continue keeps it through the round it is re-dispatched in, so no other writer can slip in between
the question and the answer. This trades parallel writers' latency for never changing the state under
an agent that will continue on it. Readers are unaffected by other readers.

### Not an integration

Answering lets the agent continue **inside the run's isolated worktree**. It never applies anything
to the project and never changes a policy, permission or the integration policy. The ChangeSet is
reviewed and applied by the person afterwards, as before; a read-only validator or QA still does not
block it.

### Audit

Each transition is a workflow event on the run: `interaction_detected`, `node_waiting_for_input`
(execution paused), `interaction_answered`, `node_input_resolved` (execution resumed),
`interaction_rejected` (an answer that was refused), `interaction_cancelled`. The questions with their
answers and times stay in `interactions`.

### Routing

Built-in workflow templates route on `result.outcome`, which the validator/QA agents' contracts
declare (`pass`/`fail`). Workflows that route on `result.status` keep working, but templates no
longer do; an agent without the outcome is reported by validation before the run.

## Not done

- No timeout (the model has `expired`; nothing sets it).
- At most one pending question per step.
- No runtime reports a structured question event yet; the hook exists.
- `permission`/`approval` answers are information for the agent. What a tool may really do is still
  decided by the policy and the approval broker, which keep their own approvals.
