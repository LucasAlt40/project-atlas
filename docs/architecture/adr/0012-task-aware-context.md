# ADR 0012 — Task-aware context

**Status:** accepted. Extends [ADR 0011](0011-harness-knowledge-model.md).

## Why

Until V0.7.2 every execution received the same project context: the whole Harness, cut to a budget by a fixed
priority. A password-recovery task and a CI fix were told the same things, so the budget was spent on what the task did
not need and, on a big Harness, could drop what it did.

V0.8 chooses the part of the Harness a task needs:

```text
Repository ─▶ Harness ─▶ Task Context Selector ─▶ Task Context ─▶ PromptBuilder ─▶ Runtime ─▶ Agent
```

The Harness stays the only source of knowledge. The selector **creates no knowledge**: it selects, ranks and orders what
exists. (The one sentence it can add is the honest one that a technology the task names was not identified in what the
Harness holds.)

## Harness context vs. Task Context

|                | Harness context (V0.7)                | Task Context (V0.8)                                   |
| -------------- | ------------------------------------- | ----------------------------------------------------- |
| Chosen by      | fixed priority per category           | the task's text, then the same budget                 |
| Same for every | task                                  | never: it changes with the task                       |
| Explainable    | "dropped for size"                    | every candidate has a score and the matches behind it |
| Role now       | fallback, and the baseline to compare | the default                                           |

Both are produced by `HarnessContextBuilder` (`build` and `build_for_task`) from the same loaded items, so a finding is
worded identically in both. Prompt layers are unchanged: Atlas rules → (Task Context | Project Harness) → project
context → task. The prompt carries one or the other, never both; runtimes never see `.atlas/`.

## Model

- `ContextItem` (`domain/task_context.rs`) is a **projection** of a finding (or of the user's constraints, decisions and
  business files, or of a gap): `id`, `area`, `category`, `content`, `provenance`, `verification`, `confidence`,
  `evidence`, `source_finding_id`, `tags`. It copies nothing the finding does not hold. It replaced the builder's private
  `Item`, so there is one representation, not two.
- **Areas** (closed set, 13): `architecture`, `stack`, `modules`, `conventions`, `business`, `testing`, `dependencies`,
  `entry_points`, `ci`, `constraints`, `decisions`, `infrastructure` and `general` (statements about the Harness itself).
  `Area::of(FindingCategory)` maps the existing categories; nothing was added to the stored findings.
- **Tags** are derived, never stored and never produced by a model. `taxonomy.rs` holds a fixed lexicon (about 20 tags:
  `authentication`, `password`, `api`, `backend`, `frontend`, `ui`, `database`, `migrations`, `testing`, `build`, `ci`,
  `deployment`, `caching`, …) with English and Portuguese terms and a few implications (a password is about
  authentication; an endpoint is backend). A finding's tags come from its category, its words and the words of the paths
  of its evidence (`src/auth/password-reset.ts` → `authentication`, `password`). A word outside the lexicon is not a tag.
  The same lexicon reads the task, so a match means the same words were meant on both sides.
- `TaskContextRequest { task, workspace_id, agent_id }`: the task is only its text. This is not a task manager.
- `TaskAnalyzer` (`task_context.rs`) → `TaskSignals { keywords, areas, baseline_areas, tags, intent, technologies }`:
  words are lower-cased, accent-folded and split on camelCase and separators; stop words and intent verbs are not
  keywords; any language works (words it does not know carry no tag); an empty task has no signals.

## Ranking

An item's score is the sum of weights centralised in `Weights`:

| Signal                                                             | Weight                |
| ------------------------------------------------------------------ | --------------------- |
| exact tag of the task on the item (at most two count)              | +50 each              |
| area the task's tags ask for, **only together with another match** | +30                   |
| area that orients any task of the intent (architecture for _add_)  | +25                   |
| stack item with no layer tag (language, runtime, tooling)          | +22                   |
| keyword of the task in the item's text                             | +25                   |
| category the task's tags ask for                                   | +20                   |
| a path of the evidence shares a word with the task                 | +20                   |
| the user confirmed, corrected or wrote it (once anything matches)  | +40                   |
| verified (once anything matches)                                   | +5                    |
| item's layer (frontend, backend, database) is not the task's layer | −40                   |
| constraint or decision                                             | +100, always included |

Relevant means `score ≥ 20`. The "area only with another match" rule exists because tags point at areas: without it every
module is "about authentication" once the task mentions a password.

`SelectionReason { score, matched_areas, matched_tags, matched_categories, matched_paths, matched_keywords,
always_included, penalties }` is kept for **every** candidate, included or not, and is what the preview shows. Selection
is a pure function: the same Harness and task always give the same context.

## Constraints, decisions, the user's word, inference, staleness, gaps

- **Constraints and decisions are never judged.** They are always included (they are already capped), security
  constraints first. This is deliberately stricter than "keep the relevant ones": splitting the user's text to decide what
  a task needs is where a critical constraint would be lost.
- **The user's word** outranks analysis (+40) and is told under `WHAT THE USER TOLD US`. A conflict in which a claim is the
  user's is told as _conflict detected; the user's decision is authoritative for project guidance; the repository
  evidence may indicate drift_, whether or not the user already chose a side. Atlas does not resolve it.
- **Inference stays inference.** `Possible architecture: … (medium confidence; not verified; evidence: …)`; a verified fact
  and an inference are never in the same block.
- **Stale** findings are never `WHAT WE KNOW`: they appear under `WHAT MAY BE OUTDATED` when relevant, with the date they
  were last verified, and the status line says the project changed.
- **Not found / unknown** stay distinct. A technology the task names (`Redis`) that no finding, label or evidence path
  mentions becomes `Not found: Redis was not identified in the analyzed evidence. This does not prove that Redis is not
used.` — never "the project does not use Redis".

## Budget and what was left out

The budget is the same ~6,000 characters but is now spent **after** ranking. Order: security constraints, decisions,
other constraints, the user's other words, verified facts, architecture, modules, conventions, testing, dependencies,
other inferences, unknown/not found, outdated; within a class, by score. Constraints and decisions are kept even beyond
the budget.

Nothing is hidden: the text ends with `NOT SELECTED (judged unrelated to this task; …): 3 modules, 2 infrastructure` and,
when relevant items did not fit, `Context was truncated. Omitted for size: 4 testing finding(s), …`.

## Fallback and metadata

If a selection cannot be made (a Harness from before knowledge existed; a task with no readable signals; nothing relevant)
the **whole Harness context** is used and the reason is recorded (`harness_without_knowledge`, `task_without_signals`,
`no_relevant_knowledge`). An execution never runs without context silently: `Execution.context` /
`StoredExecution.context` records `mode` (`task_aware` | `fallback`), `totalHarnessCharacters`,
`selectedContextCharacters`, `selectedItems`, `omittedItems` and `fallbackReason`. The text is the prompt the execution
already stores; it is not duplicated.

## Preview

`preview_task_context(request)` returns the same `TaskContext` a run would use (same builder instance), with the signals,
every entry with its reason, the included and excluded areas, sizes against the whole Harness and the exact text. It
reads and writes nothing. The UI shows it, debounced, above the composer; viewing it is optional and it cannot be edited
in this version. A project's fingerprint (for staleness) is reused for five seconds so typing does not rescan the project.

## Security

`TaskContextService` and `taxonomy` are pure: no disk, no commands, no network, no permissions, no worktrees. A test reads
their source and fails if `std::fs`, `std::process`, `std::net`, `Command::new`, a store or a scanner appear. The Task
Context keeps the same authority notice as the Harness context: it is background, grants no filesystem, process, network,
Git or merge permission, and loses to Atlas's rules and to the task. It reads only what V0.7.2 already read (the persisted
Harness and the existing fingerprint); there is no second scanner.

## Why not RAG, embeddings or a model

The first selector must be **testable and explainable**: for every item we can say which words and which weights put it
in or out, and the same input gives the same output. A model or vector store would add cost, latency, nondeterminism and a
second, unauditable opinion about what the agent may not see. A future version can add _semantic re-ranking_ on top of this
deterministic candidate selection; the `SelectionReason` is the contract such a layer would have to extend.

## Known limits

- The lexicon is small and hand-made; a project whose vocabulary is not in it gets baseline context only (architecture,
  conventions, testing, stack), plus anything its keywords match in labels and paths.
- Constraints, decisions and business text are items of the user's files, not split by sentence; a business file that
  mentions a task keyword is included whole (each field is capped at 1,500 characters).
- Relevance looks at the Harness, not the code: a task in a module the analysis did not find gets no module-specific context.
