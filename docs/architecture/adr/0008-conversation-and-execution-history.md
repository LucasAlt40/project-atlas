# ADR 0008 — Conversation and execution history

**Status:** accepted

## What it is

Conversations and ended executions survive a restart. Together with ADR 0007 (live process, terminal, interrupt) this makes an
execution **observable, controllable, persistent, isolated**:

```
Workspace → Agent → Conversation (messages) + Executions (ended)
```

- `domain/conversation::Message` and the new `domain/execution::StoredExecution` are what is kept.
- `application/history::ConversationHistory` is the use case over them. `ChatService` writes through it: the user message when it
  is sent, the execution (with its timeline) and the assistant message when the run ends.
- The command `list_executions` returns ended executions, newest first; `list_messages` now returns what was stored.
- The UI: an agent card has **Chat / Activity / Terminal / Details / Executions**; _Executions_ lists the agent's ended runs
  (today / earlier) and opens an **Execution Inspector** (Details, Activity, Chat, Terminal note); _Details_ shows the current or
  latest run. The header shows `● N agents running` while anything runs in _any_ workspace, and picking one opens that agent.

## What is stored, and what is not

| Kept                                                                                                                 | Not kept                                         |
| -------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------ |
| messages (both roles, `executionId`, time, failure kind)                                                             | raw terminal output (live only, ADR 0007)        |
| per execution: status, task, runtime, model, times, failure, usage/cost as reported, metadata, **timeline** (events) | streamed answer text (it becomes the message)    |
|                                                                                                                      | the assembled prompt (project context, repeated) |

The answer lives only in the assistant message; the execution does not repeat it.

## Storage: `config.json` is still enough

`messages` and `executions` are two more fields of `UserConfig`, `#[serde(default)]`, so existing files load. No database was
introduced: the volume is small and **bounded per conversation** (an agent in a workspace): the newest **200 messages**,
**100 executions** and **300 timeline events** per execution. Writes stay atomic (temp file + rename) and a failed save never
breaks the chat on screen (it is logged).

**Cost and when to change it.** Every change rewrites the whole file, so the cost grows with the amount kept: worst case a few
MB per heavy agent. That is acceptable now. Move to SQLite (one table each for messages and executions, same
`ConversationHistory` interface) when the file regularly exceeds a few MB, when search across history is wanted, or when
retention should be time-based rather than count-based. Nothing outside `ConversationHistory` would change.

## Isolation

Everything is keyed by `(workspaceId, agentId)`: queries filter by both, retention is applied per pair, and deleting a workspace
or an agent deletes its conversations and executions. The frontend filters again by workspace and agent, so a card never shows
another workspace's run. Switching workspace keeps running executions alive (ADR 0007); the global list reaches them.

## Ids and interrupted runs

- Execution ids are `exec-N`. On start the counter continues after the highest stored id, so an id is never reused across runs
  of the app.
- If Atlas was closed (or crashed) while a run was in progress, its user message has no answer. On the next start the history
  answers it once with a **failed** assistant message and a stored execution of kind `app_closed` ("Atlas was closed while this
  was running"). It is not shown as cancelled: the user did not cancel it.

## Limitations

- The timeline of a run in progress is in memory (UI); it is stored when the run ends. A crash loses the timeline of the run
  that was active, but not the question.
- A run's usage appears in the inspector only after it ends (runtimes report it at the end).
- History is per machine and per `config.json`; there is no export or sync.
