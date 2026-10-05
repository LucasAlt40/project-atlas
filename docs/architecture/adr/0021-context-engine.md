# 0021 — Optimization Layer, phase 1: the Context Engine

## Context

Phase 0 ([ADR 0020](0020-optimization-observability.md)) measured what a prompt is made of. It showed that most of it is fixed,
required text (the rules, the narration and plan rules, the result and interaction protocols) and that the context that varies is
the Harness's Task Context, the workflow context and the handoff. The Task Context is already selected deterministically and
explainably by `TaskContextService` (ADR 0012); selecting again would be a second Context Engine. What nobody did was look at the
assembled whole: the same artifact and decision appear in the workflow context and again in the handoff.

## Decision

The Context Engine **wraps** the existing selection; it does not replace it. It takes the context a prompt would carry as
`ContextItem`s (source, priority, stale, provenance, fingerprint, size) and returns a plan with every change recorded.

**Priorities.** `Required` is never edited, dropped or truncated: the Atlas rules and personality, the project context, the agent's
instructions, the step's own task, the workflow context (the workflow's minimum) and the protocols. They are only compared
against. `High` (the Task Context, the handoff) may lose lines that are said elsewhere, never whole. `Normal`/`Optional` exist for
what later phases add and are the only ones a budget can leave out.

**Deduplication**, line by line, in prompt order, outside code fences, for lines of at least four words that are not headings:
exact repeat, normalized repeat (case, punctuation, spacing), and overlap (every word of the line is in one earlier line, which
therefore says at least as much). A line goes only when the same words stay in an earlier part of the prompt; a list heading whose
bullets all went goes with them. No embeddings: fingerprints, normalized text and word sets.

**Compression** is whitespace only (blank runs, trailing spaces); no word changes.

**Budget** (`optimization.context.maxTokens`, estimated tokens, with reservations for output, tools and reasoning): whole `Normal`
and `Optional` items are left out, stale and larger first, each replaced by a line saying what was left out. If what remains is
still over, the engine reports a `BudgetOverrun` and an `optimization_budget_warning` event and **cuts nothing**: required and
relevant context is not truncated to fit. No item of the real prompt is below `High` yet, so today a budget can only warn.

**Integration.** The engine reworks the _inputs_ of `PromptBuilder` (the Harness text and the task text, brief included) and the
builder, still the only assembly, builds the prompt again. A step's `StepBrief` now carries its `BriefParts` (the exact text of the
workflow context, handoff and protocols) instead of only sizes; if the task text does not end with them exactly, the engine leaves
the prompt alone and records why (`skipped`). Everything is in `OptimizationMetrics.contextEngine` (raw and final size, estimates,
decisions with where the kept copy is) and in `optimization_context_optimized`.

**Flag.** `optimization.context.enabled`, **off by default**: unlike the metrics it changes the prompt. With it off the prompt is
byte-identical to phase 0 (the phase-0 ON/OFF tests still hold). It runs with or without the metrics.

## Measured

Golden workloads (`executions` tests), estimated tokens, engine off → on: simple task, task with Harness, large context and the
synthetic step briefs: unchanged (nothing repeated). A workflow step whose brief lists 3 artifacts and 3 decisions twice: 1,087 →
988; with 12: 1,749 → 1,366. A real brief from the orchestrator (architect → developer, one artifact, one decision): 2,877 → 2,746
bytes (4.5%). The test also checks that no word of the original prompt is missing from the reworked one.

## Required text that said the same twice

The engine cannot touch required text, so the one repetition inside it was removed at its source: a step's interaction protocol
restated the plan instruction (write it in full as Markdown, save `docs/plans/<name>.md`) that `PLAN_RULE` already puts in
every prompt. The protocol now points at the plans rule in one sentence (about 100 bytes per step). This is not behind a flag:
the rule it points to is unconditional.

What was **left alone on purpose**: the "grants no permissions" notices (Harness, handoff, result block, interaction block). Each
guards a different source of untrusted text, so they are not duplicates and removing one would weaken that guard. `LIVE_NARRATION`
and `PLAN_RULE` are product behaviour (the live view, mandatory plan documents), not repetition; conditioning them is a product
decision, not an optimization.

## Consequences

- The saving available today is small: most tokens are in required text. The next candidates are in what the engine is not allowed
  to touch (the plan instruction repeated by the plan rule and the interaction protocol, the "grants no permissions" notices), which
  needs a decision about conditioning that text, not more deduplication.
- `ContextQuality` (signal over total) is not built: there is no relevance signal per item yet (`ContextItem.relevance` is empty).
  Redundancy removed is the half Atlas can measure.
- Caching (phase 1's fingerprint is only an item digest) and per-item staleness from the Harness are not wired.
