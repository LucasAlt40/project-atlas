# ADR 0011 — Harness knowledge model, evidence and semantic analysis

**Status:** accepted. Extends [ADR 0010](0010-project-harness.md).

## Principle

`Evidence > Inference > Guess`, and `Unknown > Invented`. Atlas never teaches an agent something about the project that
it cannot point to evidence for, and it says what it does not know. An agent that receives only the Harness context and
its task should be able to tell four things apart:

```text
WHAT WE KNOW (read from the repository)   facts
WHAT WE INFER (candidates, unconfirmed)   medium-confidence inferences
WHAT THE USER TOLD US                     business, constraints, decisions, corrections, confirmations
WHAT WE DON'T KNOW                        gaps, weak signals, conflicts, partial analysis
```

## Pipeline

```text
ProjectScanner ─▶ ScanSnapshot ─▶ DeterministicAnalyzer ──────────────┐
      │                                                                 ├▶ ConsistencyChecker ─▶ conflicts
      └▶ RepositorySampler ─▶ EvidenceBundle ─▶ SemanticAnalyzer (opt-in)┘
                                                          │
                       findings + conflicts + diff ──▶ HUMAN REVIEW ──▶ HarnessGenerator ─▶ HarnessStore ─▶ .atlas/
.atlas/ ─▶ HarnessContextBuilder (compact, budgeted summary) ─▶ PromptBuilder ─▶ Runtime ─▶ Agent
```

`ProjectAnalyzer` is the one trait (`analyze_project(&AnalysisInput) -> AnalysisOutput`). `DeterministicAnalyzer` composes
rule-based `SnapshotAnalyzer`s (Git, stack, structure, CI, build commands, entry points, dependencies, testing).
`SemanticAnalyzer` is the second implementation. Review is mandatory: a model never writes the Harness.

## Knowledge model

A `Finding` is `category`, `key`, `value`, `confidence`, `origin`, optional `reason`, and **`evidence`: a list of
`{source, field}`**. Every statement that came from analysis has at least one piece of evidence; the only exception is
Atlas's own `architecture: unknown`, whose origin is `generated`.

| Origin           | Meaning                                         | Trust                       |
| ---------------- | ----------------------------------------------- | --------------------------- |
| `user_corrected` | The user replaced a value                       | Highest; survives refresh   |
| `user_confirmed` | The user confirmed an inference                 | High; survives refresh      |
| `fact`           | Read from the repository (`package.json` field) | High                        |
| `inference`      | Concluded from shape or by a model              | Medium at most              |
| `generated`      | Atlas's own statement, not from evidence        | Not knowledge, only a label |

Confidence: **high** = direct, unambiguous evidence; **medium** = several signals plus interpretation (`domain/` +
`application/` + `infrastructure/` side by side → _possible_ clean architecture); **low** = weak signals. A model can
never produce `high` (it is capped to `medium`). **Low never reaches an agent as knowledge**: the context builder lists it
under "what we don't know" as a weak signal.

Categories now include dependencies (curated, not hundreds), data (ORM, migrations), integrations (always "possible":
a dependency suggests but does not prove use), build/run commands (observed in manifests, **never executed**), entry
points, testing (framework, test files, kinds of tests, commands and coverage are _separate_ facts: an installed
framework proves nothing else), conventions (e.g. `*.spec.ts` with sample files as evidence), modules and architecture.

## Semantic analysis

Opt-in per request, by choosing an agent. A model gets **only** the prompt: a bounded folder tree, the deterministic facts,
and a bounded, selected set of files (`RepositorySampler`: README, manifests, entry points, docs, a couple of source and
test files per layer folder; never `.env`/keys/credentials; secret-looking lines redacted). Limits are explicit
(`SampleLimits`, `ScanLimits`); reaching one makes the analysis **partial**, recorded in the Harness.

- The runtime is run **without tools** (`RuntimeRequest.text_only`; Claude `--tools ""`). A runtime that cannot
  (OpenCode) is refused (`SemanticUnsupported`). The process guard still starts it, under the agent's own policy.
- The answer must be JSON in a fixed schema (unknown fields rejected). Prose is never interpreted:
  `SemanticAnalysisFailed`. Each statement must cite paths that were actually sent, with allowed category/key/length;
  otherwise it is dropped and counted. The result lists exactly which files were sent.
- A failed semantic step does not fail the analysis; the deterministic findings stand.
- Not recorded in conversations or usage (limitation).

## Consistency

`ConsistencyChecker` records contradictions without choosing: model vs. fact, README/docs vs. facts (versions), and a
user correction the repository has since moved past. A `Conflict` keeps both claims with their evidence; the user's
decision (a correction for that finding) resolves it and wins. `HarnessHealth`: `Conflicted` > `Partial` > `NeedsReview`
(unconfirmed medium inferences, or a V0.7 Harness without knowledge) > `Healthy`; `Stale` is modelled, not detected.

## Files and what Atlas may overwrite

```text
.atlas/
├── project.yaml                 metadata + the user's decisions (corrections, exclusions, confirmations)
├── knowledge/findings.yaml      GENERATED: raw findings with evidence, conflicts, analysis info
├── context/architecture.md stack.md conventions.md testing.md      GENERATED views (banner says so)
├── context/business.md constraints.md decisions.md                 USER-OWNED
├── agents/ workflows/ policies/ memory/                            placeholders
└── backups/<timestamp>/         what an update replaced
```

- Generated files are replaced on update, only after the user saw the diff, and with a backup.
- User-owned files are written only if absent or if the user changed that field in the review. An empty field keeps what
  the file says. `business.md` is parsed by its four headings; a hand-written shape Atlas cannot reproduce is
  **left untouched** and reported (it is still sent to agents as the user's word). Typed text has secret-looking lines
  removed.
- The raw findings are stored, not the user's choices applied to them, so a refresh can diff raw against raw and reapply
  corrections. Model findings are carried over by a refresh while their evidence still exists.

## Refresh

`refresh_project_harness(confirm=false)` re-analyses (deterministic) and returns diff + conflicts, writing nothing.
`confirm=true` applies it: generated files updated (backup), corrections/confirmations/exclusions kept, user files
unchanged. Atlas can therefore go from Angular 18 to 21 without losing business context or decisions.

## Context building

`HarnessContextBuilder` produces a compact _Project Context Summary_ under a budget (default 6 000 characters). Items are
kept by priority — constraints/decisions, architecture, stack, modules, conventions, business, testing, the rest — and
what is omitted is named in the text and logged on the execution. Each item keeps its area so a later task-aware filter
can select items without changing storage (not implemented). The text begins by stating it is context, not authority.
Layers, outermost first: global user context → project Harness → workspace context → personality → execution context →
task. The Security Policy is not a layer: it is enforced by the process guard and cannot be changed by any context.

## Known limitations

- Semantic analysis only works through runtimes that can run without tools (Claude); it is not tracked in usage.
- Conventions, modules and data flow beyond the listed rules come only from the model, always as medium/low inferences.
- Conflict detection covers fact-vs-model, docs-vs-fact versions and user-vs-new-fact; not every possible contradiction.
- `Stale` is not detected; refresh is user-initiated. Business context is never inferred.
