# ADR 0032 — Sharing configurations: export and import as a file

Status: accepted. Builds on [ADR 0002](0002-ports-and-adapters.md) (ports and adapters), [ADR 0004](0004-runtimes.md) (runtimes and models),
[ADR 0030](0030-workflow-templates-and-tools.md) (saved templates); changes none of their invariants.

## Context

What a person builds in Atlas (a personality, an agent, a workflow, a saved template) lived only in that installation's `config.json`. Handing
an agent to a colleague, moving to another machine or keeping a backup meant recreating everything by hand. The stored records are not a fit for
a file: they carry local ids, workspace ids, timestamps, versions and history that mean nothing elsewhere, and their shape changes with the
database. A file exchanged between people has to outlive both.

## Decisions

1. **JSON, in its own versioned format.** `serde_json` is what the core and the webview already speak. JSON has no implicit typing, anchors or
   aliases to be surprised by (YAML's), reads and edits by hand, and a size-limited parse of untrusted text is simple to reason about. TOML does
   not express a graph well. The file is `{ format: "atlas-config", schemaVersion, exportedAt, source, roots, items, external }`, written as
   `<name>.atlas.json`.
2. **The file is its own contract, not a dump of the store.** Each kind has its own `*Spec` types (`application/portability/spec.rs`) and an
   explicit mapping to and from the domain, with exhaustive `match`es, so a renamed or split field in the stored model is absorbed in one place
   and old files keep reading. No local id is exported: items name each other with the file's own `ref`s (`agent-2`); workflow step and edge ids
   are the only ids kept, because they are the graph's own. Workspace ids, project paths, timestamps, versions, route-repair history, canvas
   viewport, runs, conversations and MCP grants are left out.
3. **Versioning.** `schemaVersion` is checked first: a newer one is refused saying so, an older one is brought forward by the `UPGRADES` chain
   (empty while the schema is 1; the mechanism is tested with a stand-in step). Adding an optional field does not change the version and unknown
   fields are ignored; renaming or re-meaning one does, with an upgrade step.
4. **One `Category` per kind** (`personality`, `agent`, `workflow`, `workflow_template`), behind `category(kind)`. A category owns checking its spec,
   exporting, inspecting and writing; the flow (`export.rs`, `inspect.rs`, `apply.rs`) does not know what an agent is. A new kind is a new
   `Kind` variant and a new category. There is no plugin mechanism: four implementations of one trait were enough.
5. **Importing is never "deserialize and save"**: parse (size, JSON, identity, version, shape, references, specs) → _preview_ (`preview_config_import`, changes
   nothing) → decisions → _import_ (`import_configs`) in a single `ConfigRepository::modify`, so the installation has all of it or none of it,
   including when the save fails. The file is read again against what is here _at import time_; the preview is advice, not a ticket.
6. **Conflicts are decided by the person, per item**: `create` (new local id; a taken name gets ` (2)`), `use_existing`, `replace`, `skip`. An item
   is matched by name (a built-in personality also by its id, which means the same everywhere). Content equal to what is here defaults to
   `use_existing` (so importing a file twice adds nothing); a name match with other content defaults to `create`; nothing is overwritten
   without `replace`, which the UI makes the person confirm item by item. A match with no decision is refused (`decision_required`).
7. **Dependencies.** A file carries what its items need (an agent's personality, a workflow's agents) unless exported without it, in which case
   `external` names them and the importer binds each to one of their own (`bindings`). The same-name candidate is suggested and pre-selected in
   the preview but is only applied when the person imports. Skipping something that other items need, or leaving a binding empty, refuses the
   import (`dependency_unresolved`). Workflows are imported into a chosen workspace; steps keep their order, routes, retries, failure routes,
   loops, conditions, read-only flags and per-step instructions. One that does not validate yet is kept as a draft and said so.
8. **Models are checked, never assumed.** The file keeps the agent's runtime id, runtime and provider names, and model id. The preview asks this
   installation's own runtimes (`RuntimeRegistry::inspect_all`, once, and only when the file has agents) and reports one of: `available`,
   `unverified` (the runtime cannot list models, as when typing a model id into the form), `model_missing`, `runtime_not_ready`,
   `runtime_unsupported` (cannot run tasks), `runtime_missing`, plus the ready runtimes with their models as alternatives (the same runtime first).
   The original is kept unless the person picks an alternative; the first four can be imported with the original and set up later (the agent
   shows as unavailable like any other whose runtime is not ready); the last two need another runtime or the item is skipped. The report keeps
   the original runtime and model next to what was applied.
9. **Importing grants nothing and runs nothing.** No run, grant, connection or secret is created. Agents are imported with the `read_only`
   permission profile unless the person ticks "keep the permission to edit files" in the preview (the preview says which agents ask for it and
   warns when their runtime cannot edit). Saved templates keep their tool _suggestions_ (a capability or a connection name), which are only
   ever turned into grants by a person starting a workflow with that option (ADR 0030). Instructions and prompts are text: they are shown in the
   preview (expandable) and stored verbatim, and nothing in a file is executed, expanded or interpreted.
10. **Hard limits on untrusted input**: 2 MiB per file (checked before parsing), 200 items, 200 steps and 400 routes per graph, refs of
    `[a-z0-9_-]{1,64}`, names of at most 80 printable characters, graphs whose steps and routes name each other consistently (anything else is
    refused whole), domain rules for names, model ids (no flag-like or whitespace-bearing ids), result contracts and permission profiles.
    Error messages carry codes and positions, never the file's content.
11. **Files are chosen in native dialogs opened by the core** (`pick_config_file`, `save_config_file`, with the `tauri-plugin-dialog` already in
    use). The webview never names a path: it holds the file's text. `save_config_file` only writes text that parses as an Atlas file, and writes
    through a temporary file and a rename. No file-system plugin or capability was added.
12. **Credentials never travel.** Agents have none; MCP connections (executables, arguments, secrets) are not exported at all: the MCP Hub has its
    own import/export for CLI configuration files. A text that _looks_ like a credential (the Harness's secret detector) raises a warning when
    exporting and again when previewing; it is not redacted, because changing someone's instruction silently is worse than telling them.

## What is portable, and what is not

| Exported / imported                               | Why                                                        |
| ------------------------------------------------- | ---------------------------------------------------------- |
| Personalities (custom, and built-ins as edited)   | Reusable behaviour; no local data                          |
| Agents (with their personality)                   | Reusable setup; runtime and model checked on arrival       |
| Workflows (with their agents and personalities)   | Reusable process; imported into a workspace of the choice  |
| Saved workflow templates (with tool suggestions)  | Reusable process; suggestions grant nothing                |
| _Not_ exported: workspaces                        | Tied to a project folder of one machine                    |
| _Not_ exported: MCP connections, grants, secrets  | Executables and credentials; the MCP Hub has its own flow  |
| _Not_ exported: conversations, executions, usage  | Operational history, possibly private                      |
| _Not_ exported: runs, worktrees, quotas, settings | Local state of one installation                            |
| _Not_ exported: rules                             | No user-facing editor yet; project rules live in `.atlas/` |
| _Not_ exported: harness (`.atlas/`)               | Already lives in the project and travels with its Git      |

## Limits

- An agent imported with a model its runtime does not list keeps that id; nothing records that it is "pending" beyond the agent's runtime
  availability and the import report. After choosing an alternative the original is only in the report.
- A file cannot be imported into a workflow that has a run in progress (`replace` is refused); a different decision is needed.
- `use_existing` and `replace` match by name only; two things with one name in the same scope are matched to the first.
- Built-in workflow templates are code and are not exported.
- Drag and drop of a file onto the window, and an import from a URL, were not built.
