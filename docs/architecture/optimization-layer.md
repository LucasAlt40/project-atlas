# Optimization Layer — status and decisions

The goal: fewer tokens, less latency and less noise per execution **without** losing quality or safety. The rule: measure before
optimizing, never call something optimized without numbers, and never let an optimization remove required or security context.
This page is the map; each phase has its ADR.

## Where each phase stands

| Phase | What                                                                                                                                                                                                                               | State                                                                                    | Default                                                | ADR                                               |
| ----- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- | ------------------------------------------------------ | ------------------------------------------------- |
| 0     | Observability: prompt sections, estimated tokens, runtime-reported tokens, latency, events, A/B and golden workloads                                                                                                               | done                                                                                     | on (`optimization.metrics.enabled`)                    | [0020](adr/0020-optimization-observability.md)    |
| 1     | Context Engine: priorities, line-level deduplication, whitespace compression, token budget that reports instead of cutting                                                                                                         | done                                                                                     | **off** (`optimization.context.enabled`, `.maxTokens`) | [0021](adr/0021-context-engine.md)                |
| 2     | Skills (Agent Skills format): discovery, activation, resources, quality checks, cache, deterministic selection                                                                                                                     | done                                                                                     | **off** (`optimization.skills.enabled`)                | [0022](adr/0022-skills.md)                        |
| 3     | RTK (output filtering)                                                                                                                                                                                                             | built, then **removed**: no Atlas agent runs commands through Atlas, so it saved nothing | —                                                      | [0023](adr/0023-rtk-evaluated-and-removed.md)     |
| 4     | Runtime surface: what a runtime can reach; Claude isolated from the user's MCP servers and skills; per-run record of tools exposed, used and extensions loaded                                                                     | done for Claude                                                                          | always on (hardening)                                  | [0024](adr/0024-runtime-surface-and-isolation.md) |
| 5     | Guardrails (one authority, `security/` extended) and Context Review: ALLOW / ASK / DENY / TRANSFORM before an agent starts and before a result is handed on; deterministic review of the context; ASK through `PendingInteraction` | done, hardened in 5.1 (approvals bound to the evaluation; ChangeSet review before Apply) | on (`optimization.guardrails.enabled`)                 | [0025](adr/0025-guardrails-and-context-review.md) |
| 6     | Hooks                                                                                                                                                                                                                              | not started                                                                              | —                                                      | —                                                 |
| 7     | Dashboard                                                                                                                                                                                                                          | not started                                                                              | —                                                      | —                                                 |

Phases 1 and 2 change the prompt, so they are off until someone turns them on deliberately. With them off, the prompt is
byte-identical to what `PromptBuilder` builds (a test proves it, and the golden workloads compare both).

## What was measured

All sizes are estimates (`chars / 4`, always labelled `estimated`) unless the runtime reported them (`exact`).

- **Prompt composition** (golden workloads): a simple task is about 1.7 KB (418 tokens), mostly fixed required text (rules, narration,
  plan rule, personality); a workflow step with a brief is 6 to 8 KB, of which the protocols are about 1.8 KB per step.
- **Deduplication (phase 1)**: 4.5% of a real workflow brief (the same artifact and decision appear in the workflow context and in
  the handoff); 9% to 22% in synthetic briefs with many repeated items; nothing on tasks without repetition. No word of the
  original prompt is lost.
- **Skills (phase 2)**: with ten skills of about 800 tokens and one relevant, progressive disclosure sends 808 tokens instead of
  8,160, and none of the catalogue metadata.
- **Guardrails (phase 5)**: a step whose instructions and skill disagree on the database pauses before any process starts, asks, and
  runs only after a person's yes (or fails safely after a no); checked with the real stack and with the real Claude CLI on a model
  that does not exist (no model call). A secret in another agent's result or in untrusted context is taken out before it travels.
- **Runtime surface (phase 4)**: Atlas's Claude launch exposed 11 tools, 8 MCP servers, 48 skills and 84 slash commands (measured
  with the real CLI, no model call); it now exposes 3 tools, 0, 0 and 0.

## Decisions that are easy to forget

- Atlas does not own the CLI runtimes' tool loop. Tool output never passes through Atlas, so output optimization has nothing to act
  on until agents run commands through Atlas (ADR 0023 says what that would take).
- Required text (the Atlas rules, protocols, the "grants no permissions" notices) is not shortened as an optimization: each notice
  guards a different source of untrusted text. `LIVE_NARRATION` and `PLAN_RULE` are product behaviour.
- A budget never truncates: it leaves out whole items below `High` priority, says so in the prompt, and otherwise reports the overrun.
- Autonomy modes (Manual / Guided / Auto) do not exist in the code; phase 5 did not invent them. Atlas cannot interpose on the
  tools a CLI runtime runs inside its process, so guardrails act before the agent starts and before a result is handed on.
- Nothing here installs software or edits the user's configuration (`~/.claude`, agent settings).
- Apply is always a person's act and never commits, whatever the guardrails say: the ChangeSet review can only hold an Apply back
  (and a person can apply again after seeing a `NeedsReview`), never perform one.
- A real model run has not been made for any of this; every number above comes from deterministic tests or from the CLI reporting
  its own start-up state with no model call.

## What is pending

1. **A real run with measurement**: runtime-reported input tokens against Atlas's estimate (calibrates `chars / 4`), cached tokens,
   and how much of the real input the user's `CLAUDE.md`, hooks and plugins add. It costs tokens on the user's account, so it waits
   for their go-ahead.
2. **OpenCode, Gemini, Antigravity surfaces** (what MCP and extensions they load in headless runs).
3. Phases 6 and 7, and the decisions listed in ADR 0025 (autonomy; `--restricted` for Claude: investigated, not adopted, a small real run would settle it).
4. A decision on agents running commands through Atlas (security and product), which is what would make output filtering matter.
