# ADR 0010 — Project Harness

**Status:** accepted. Extended by [ADR 0011](0011-harness-knowledge-model.md) (evidence, provenance, semantic analysis, refresh diff).

## What it is

The Harness is what Atlas knows about a project, kept **in the project** under `.atlas/` (not in Atlas's own config, not
inside a worktree). It is versionable with the project's Git, human-readable, and independent of any provider, model or
runtime.

```
User → Initialize Project
        ProjectScanner → ScanSnapshot → ProjectAnalyzer → findings
                                              ↓ (human review: untick, correct, add context)
                                        HarnessGenerator → HarnessStore → <project>/.atlas/

Execution → HarnessContextBuilder → PromptBuilder → Runtime (in the worktree)
```

```
.atlas/
├── project.yaml          structural metadata only (version, project, repository, stack, corrections)
├── context/              architecture.md, stack.md, conventions.md, business.md, constraints.md
├── agents/ workflows/ policies/ memory/    (placeholders; Atlas only makes sure they exist)
└── backups/<timestamp>/  whatever an update replaced
```

## Pieces

| Piece                                              | Responsibility                                                                                                                            |
| -------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| `infrastructure/project_scanner.rs`                | The only code that reads a project folder for analysis. Names of entries plus the text of a short whitelist of manifests.                 |
| `application/harness/analyzer` — `ProjectAnalyzer` | Snapshot in, findings out. `DeterministicAnalyzer` = Git + stack + structure + CI analyzers. A semantic analyzer can implement the trait. |
| `domain/harness.rs` — `Finding`                    | `category`, `key`, `label`, `value`, `confidence` (high/medium/low), `origin` (fact/inference/user), `source`, `evidence`.                |
| `application/harness/generator.rs`                 | Findings + user context → files. Says where each claim comes from; absence stays absence ("has not been documented").                     |
| `infrastructure/harness_store.rs` — `HarnessStore` | The only code that writes `.atlas/`. Backup-before-replace, atomic rename, never through a symlink, paths cannot leave `.atlas/`.         |
| `application/harness/context.rs`                   | `HarnessContextBuilder`: reads the Harness at the **project root**, renders capped, secret-redacted text for the prompt.                  |
| `application/harness/mod.rs` — `HarnessService`    | `analyze`, `initialize` (create / use existing / update existing), `refresh`, `get`. Commands are domain-shaped; no generic file access.  |

## Rules

- **Analysis is reading.** Nothing is executed, built, installed, started or fetched. Symlinks are not followed, so the
  scan cannot leave the root. Generated folders (`node_modules`, `target`, `dist`, `.git`, …) are noted, not entered.
  Limits: depth 4, 5 000 entries (hitting the entry limit is reported as _partial_), 256 KiB per manifest.
- **Secrets never enter the Harness.** `.env` files are noted by name (`.env detected`) and never read. Compose files
  contribute image names only. Text the user types, and every context file when it is loaded, has secret-looking lines
  redacted.
- **No invented context.** Architecture is only ever an inference ("appears to be…", medium confidence at most) and needs
  several folders side by side; otherwise `unknown`. User corrections win and are recorded in `project.yaml`, so a refresh
  does not undo them.
- **Nothing is destroyed.** Creating over an existing manifest is refused. Updating backs up what it replaces. Files
  that hold the user's words (`business.md`, `constraints.md`) are only rewritten when the user supplies new text.
- **Context is not permission.** The prompt places the Harness after the Atlas rules and labels it as context without
  authority. What an agent may do is decided by the process guard, which never reads prompts. The Harness cannot grant
  filesystem, network, Git, runtime or merge rights.
- **Worktrees.** The Harness is read once at the project root and reaches the agent as prompt text. It is not copied into
  worktrees, and the runtime and `WorktreeManager` never see it. Changes to `.atlas/` follow the normal Git lifecycle.
- `.atlas/` is never added to `.gitignore`.

## Known limitations

- Deterministic only; no semantic (LLM) analysis, no business or architecture understanding beyond folder shape.
- No diff between a regenerated and an edited Harness: an update replaces generated files after backing them up.
- `harnessStatus` is computed from `.atlas/` on demand (`get_project_harness`), not stored on the workspace.
- `agents/`, `workflows/`, `policies/` and `memory/` are placeholders: nothing reads or executes them yet.
