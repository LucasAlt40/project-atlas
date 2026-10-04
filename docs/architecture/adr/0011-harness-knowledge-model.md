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
kept by priority — constraints/decisions (never dropped: each is capped on its own), verified facts and stack,
what may be outdated and conflicts, architecture, modules, conventions, testing, inferences, business, unknowns — and
what is omitted is named in the text and logged on the execution. Each item keeps its area so a later task-aware filter
can select items without changing storage (not implemented). The text begins by stating it is context, not authority.
Layers, outermost first: global user context → project Harness → workspace context → personality → execution context →
task. The Security Policy is not a layer: it is enforced by the process guard and cannot be changed by any context.

## Known limitations

- Semantic analysis only works through runtimes that can run without tools (Claude); it is not tracked in usage.
- Conventions, modules and data flow beyond the listed rules come only from the model, always as medium/low inferences.
- Conflict detection covers fact-vs-model, docs-vs-fact versions and user-vs-new-fact; not every possible contradiction.
- Staleness is detected by comparing relevant files and folders, not by understanding the change: an edit to an ordinary
  source file is invisible to it, and a relevant change marks only the findings whose evidence lives there (the rest of
  the Harness still says the project changed). Refresh is user-initiated. Business context is never inferred.
- Nothing is verified by running a command in V0.7.2; `command_execution` exists in the model for a later version.

## V0.7.2 — Verification, staleness and negative knowledge

The Harness must be able to say what it knows, how it knows, when, and whether that may still hold.

**Provenance and verification are different axes.** `origin` says who made a statement (`fact` read from a repository
file, `inference`, `user_confirmed`, `user_corrected`); `verification` says whether it was _checked_
(`unverified` | `verified` + `verifiedAt` + `method` | `stale`). A fact is verified by the very repository file it is read
from (`repository_file`); an inference is `unverified` until something checks it. `command_execution` is reserved for a
future version that runs a command on request; V0.7.2 never runs anything to obtain a verification. The user vouching for
an inference makes it `user_confirmed` with `originalOrigin: inference` — it never becomes a fact. A correction replaces
the value, so the earlier verification no longer applies. Knowledge written before V0.7.2 is read as it was: its facts
are stamped verified at the analysis time.

**Evidence is not optional for analysis.** Any non-generated finding without at least one evidence source is rejected
(counted in `semantic.rejected`), never downgraded to low confidence.

**Unknown vs not found** (`negative.rs`). _Not found_ is a bounded search that came back empty (database, test framework,
CI, infrastructure) and says where it looked and that it is not proof of absence. _Unknown_ is what nothing available can
establish (production deployment, whether commands work, whether an inferred architecture is still enforced). The
context says "Not found: …" / "Unknown: …" under WHAT WE DON'T KNOW and never states absence or turns either into a
fact.

**Project fingerprint** (`fingerprint.rs`). Stored in `knowledge/findings.yaml` (`analysis.fingerprint`) as one digest
plus per-part digests, so a later check can name what changed. Parts: manifests and lockfiles, configuration
(`tsconfig*`, `vite.config.*`, `angular.json`, `tauri.conf.json`, linters, toolchains), CI and containers
(`.github/workflows`, `Dockerfile`, compose, `docker/`), `README*`, and the _existence_ of folders up to two levels deep.
Ordinary source files are not parts: editing `Button.tsx` is not relevant, adding `src/infrastructure/` is. Digests are
FNV-1a over line-ending-normalised text (stable across platforms and Rust versions; not a security boundary), paths use
`/`, links are never followed, secrets (`.env`, keys) and generated folders (`node_modules`, `target`, `dist`, `.git`…)
are excluded, and nothing is executed. Files that are not analysed (lockfiles, config) are only digested by the scanner
(first 4 MiB).

**Staleness.** `summarize` and the context builder compare the stored fingerprint with the project as it is now. A
change marks as `stale` only the findings whose evidence path equals, contains or is contained by a changed path; user
statements are never marked (they are not claims about the repository). Stale knowledge is moved to WHAT MAY BE OUTDATED
in the context, with its last verified date, and HARNESS STATUS says the project changed. Nothing is deleted or
rewritten: staleness is computed on read. A Harness without a fingerprint is `NeedsReview` (`no_fingerprint`), not stale,
because nothing says whether it changed. If the project cannot be scanned, staleness is not claimed.

**Health precedence**, strongest first: `Stale > Conflicted > Partial > NeedsReview > Healthy`. Stale outranks the rest
because its other judgements describe a project that no longer exists as analysed, and refreshing is the first step
(it may dissolve them); every reason is still listed. `Healthy` means not stale, no unresolved conflict, a complete
analysis and no relevant inference waiting for confirmation.

**Refresh and human knowledge** are unchanged: refresh previews first (now with the relevant changes), writes only after
confirmation, with backups, and never overwrites `business.md`, `constraints.md` or `decisions.md` or the user's
corrections, confirmations and exclusions. After an applied refresh the new fingerprint is stored and the Harness is no
longer stale. Contradictions between repository evidence and the user's knowledge stay conflicts for the user to decide.

**Context shape.** `HARNESS STATUS` (last analysed, stale/conflict notes) → WHAT WE KNOW (verified facts only; a fact
without verification is told as a candidate) → WHAT WE INFER (always "not verified") → WHAT THE USER TOLD US
(confirmations say "inferred by Atlas, confirmed by the user") → WHAT WE DON'T KNOW → WHAT MAY BE OUTDATED. The builder
takes an optional `ProjectScanner` to fingerprint the project at run time; without it no staleness is claimed.
