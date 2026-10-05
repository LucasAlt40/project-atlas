# 0020 — Optimization Layer, phase 0: measuring before optimizing

## Context

Atlas builds a prompt for every execution (personality, rules, Harness/Task Context, project, agent instructions, and for a
workflow step a brief with state and handoff) and hands it to a CLI runtime. Nobody could say how big each part is, how long each
stage takes, or how much of what Atlas assembled became tokens at the model. Optimizing without that would be guessing, and a
smaller prompt that drops something useful is a regression.

The runtimes are CLIs (`claude -p`, `opencode run`, …). Atlas owns the prompt it sends and the answer it reads; it does **not** own
the CLI's internal tool loop. What a tool printed never passes through Atlas, so tool output and tool selection are not observable
here (see "Not observable").

## Decision

**Phase 0 is observability only.** Metrics on or off, the prompt, the Task Context selection, the runtime request, the permissions
and the events other than the three below are the same. There is no `if metrics { change the prompt }` anywhere; a test runs the
same task with the flag on and off and compares the prompt byte for byte and everything else the runtime is handed.

- **`PromptBuilder` stays the only assembly.** `assemble` returns the prompt and a `PromptLayout` taken from the very strings it
  joined; `build_with_access` is the same call without the layout. Nothing is parsed back out of the prompt. A workflow step's
  `StepBrief` likewise reports a `BriefLayout` (workflow context, handoff, protocols) where it writes them, passed to the execution
  through `StepRequest` → `WorkflowStepRequest` → `StepOptions`.
- **`PromptBreakdown`** lists sections (`personality`, `atlas_rules`, `live_narration`, `plan_rule`, `harness` or `task_context`,
  `project_context`, `agent_instructions`, the three `brief_*`, `task`) and adds the rest as `framing` (headings and separators), so
  the sections always add up to the bytes sent.
- **Tokens.** Atlas has no tokenizer. Every figure it computes is `ceil(chars / 4)` and carries `token_source: estimated`. What a
  runtime reports stays in the existing `UsageMetrics` (`source: runtime_reported`), now with `cached_input_tokens`; there is no
  second counter. `PromptVersusRuntime` puts the two side by side and marks the runtime side `exact` or `unavailable`.
- **`OptimizationMetrics`** (domain, no behaviour) is kept on `Execution` and `StoredExecution` as `optimization`, `#[serde(default)]`
  and skipped when absent, so old history loads and old readers see the old shape. It holds the breakdown, the Harness numbers
  (from the existing `ContextRecord`), latency, tool calls, handoff bytes and counters. A figure Atlas cannot observe is `None`.
- **Latency** (milliseconds, f64): context build (only with a Harness service), prompt build, runtime startup (launch → the
  runtime's own `Waiting` report), runtime execution, runtime total, whole `run_step`, and `instrumentation_ms`, what measuring
  itself cost. A `RuntimeProbe` watches the progress callback the runtime already calls; it reports nothing of its own.
- **Events** reuse `ExecutionEvent`/`ExecutionObserver`: `optimization_context_built`, `optimization_prompt_built`,
  `optimization_metrics_recorded`. They are announced to listeners only (not added to the execution's log).
- **Flag.** `AppSettings.optimization.metricsEnabled` (`optimization.metrics.enabled`), on by default, read once per execution
  through the `OptimizationFlags` port. `ExecutionService` built without a flag source measures nothing.
- **Benchmark base** (`application/optimization/benchmark.rs`, tests only): `BenchmarkRow` and `Comparison` (two runs of a scenario
  and whether their prompts are identical) and five golden workloads in `executions` tests: simple task, task with Harness,
  workflow step with handoff, second step of a workflow with Harness, large context. In phase 0 baseline and "optimized" are the
  same code, so identical prompts are an acceptance criterion, not a result.

## Not observable (and said so)

- **Tool output bytes and tool selection**: the CLI's tools talk to the model directly. `tools.totalOutputBytes` is always
  `None`. `tools.calls` is the number of tool events the runtime streamed, and `None` when it streamed none (Atlas cannot tell "no
  tool used" from "this runtime does not stream tool events").
- **Caches**: none exist yet; `cacheHits`/`cacheMisses` are `None`. `deduplicatedItems`/`compressedItems` are 0 because phase 0
  does neither.
- **Runtime tokens** when the runtime reports none (Gemini's cache figure is not read; the stand-in CLIs report only a cost).

## Consequences

- Phase 1 (Context Engine) can wrap `TaskContextService` and be judged against `PromptBreakdown` and the golden workloads.
- Tool-output optimization is out of reach of this layer: the data it would measure does not exist here (see ADR 0023).
- Persistent user configuration is never touched (no agent hooks, no `~/.claude` changes).
