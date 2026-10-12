# ADR 0033 — Workspaces with several projects

Status: accepted. Builds on [ADR 0009](0009-git-worktree-isolation.md) (worktrees), [ADR 0010](0010-project-harness.md) and
[ADR 0012](0012-task-aware-context.md) (Harness, task-aware context), [ADR 0013](0013-workflow-engine-and-orchestrator.md) and
[ADR 0014](0014-handoff-shared-worktree-and-code-integration.md) (workflows, shared worktree); changes none of their invariants.

## Context

A workspace was one folder (`projectPath`). A system of a frontend, a backend and several services needed one workspace per repository, and no
agent could understand the whole. Where the code assumed a single folder: `Workspace.project_path` (the only stored path), `HarnessService`
(`project_path(workspace_id)`), `ExecutionService::run_step` (working directory, Harness, skills, rules, worktree), the security resolver
(`project_root` = that path), workflow `open_workspace` (the run's shared worktree), MCP runtime files and the UI (`projectPath`).

Two facts about the existing code shaped the decision. The Harness already lives in each project's own `.atlas/` and every Harness function takes a
folder, so it is per-project already. And only an agent **isolated in a worktree** is given edit tools (`edit_access`), one worktree per execution,
created from whatever folder it is handed (it already copes with a project inside a larger repository).

## Decision: hybrid (A + global layer), not a unified Harness (B)

| Criterion                           | A: local Harness per project + workspace context                                           | B: one unified Harness                                     |
| ----------------------------------- | ------------------------------------------------------------------------------------------ | ---------------------------------------------------------- |
| Precision, isolation of local rules | each project's rules, findings and evidence stay as analysed                               | every item needs a project tag; local and global rules mix |
| Tokens / limits                     | a task gets the fragments of the projects in its scope, within the one existing allocation | one big store to filter, or all of it                      |
| Incremental update                  | adding or changing a project analyses only it                                              | any change invalidates one analysis                        |
| Cost to start                       | per project, partial failure is per project                                                | one long all-or-nothing run                                |
| Compatibility                       | today's code and `.atlas/` files unchanged                                                 | new format, migration of every Harness                     |
| Worktrees                           | a project is its own repository                                                            | no relation                                                |

Chosen: **A plus a workspace context** that relates the projects. Not a copy of the Harnesses: it holds names, roles, folders, stack and Harness state,
a few evidenced integration/data facts with their source, and the dependencies the **user declared** (Atlas does not guess them), plus the user's own
notes. Kept in Atlas's data folder (`workspace-context/<workspace id>/context.md` generated, `notes.md` the user's), never inside a repository: it
belongs to the workspace, a project can be in several workspaces, and nothing is committed. Regenerating never touches the notes. It carries a
fingerprint of what it was made from, so the screen can say it is out of date.

## Model and migration

`Workspace.projects: Vec<WorkspaceProject { id, name, path, role?, depends_on }>` (new `ProjectRole` label, nothing branches on it). `projectPath` stays and
is always the primary (first) project's folder, so everything that needs one folder keeps working. `Workspace::normalize` derives the single project
of an old workspace (id `<workspace id>-main`, name from the folder), keeps `project_path` in step and drops dependencies on missing projects; it is
idempotent, runs when the config loads (and is written then) and again whenever workspaces are listed. Nothing else is touched (ids, layout, security,
conversations, executions, `.atlas/`). Duplicates are compared by the folder as it really is (`ProjectInspector::canonical`; case-insensitive on
Windows and macOS). A workspace always keeps one project; removing a project forgets the registration only.

## Execution scope

`ScopeRequest { working_project_id?, read_project_ids }` → `ProjectScope { working, others }`, resolved by `WorkspaceService::resolve_scope`. No request is
the primary project alone, byte for byte the old prompt and launch. It travels on `SendMessageRequest`, `RunAgentRequest` and a workflow run
(`WorkflowExecution.scope`, every step alike, validated when the run starts).

- **Working project**: where the process runs, whose repository the worktree is made from, whose folder the security layer holds the process to
  (`ExecutionScope.project_id`, an id looked up in stored config, never a path).
- **Consulted projects** are read-only. The prompt lists them with the limits; a task-aware fragment of each one's Harness is told, labelled with the
  project and its folder, within the Harness section's existing allocation (workspace context ≤ 1,200 chars, working project ≥ 60% of the rest,
  others share the remainder; those that do not fit are named only). `HarnessContextBuilder::build_for_task_within` does the selection.
- **Folders given to the runtime** (`RuntimeRequest.extra_dirs`, `ModelRuntime::accepts_extra_dirs`, implemented for Claude as one `--add-dir` per
  folder): only to a run **without edit tools**. The Claude CLI cannot give a folder read-only to a run that edits, so an agent that edits knows the
  others through their Harness, not their files; the prompt and the log say so. Other runtimes are not given extra folders until an adapter is checked.

## Worktrees: one writable project per execution

Writing stays what ADR 0009 made it: one worktree per execution, of the working project. Independent repositories, a project inside a monorepo and
non-Git folders behave as before (a non-Git project cannot be isolated, so its agents only read). There is deliberately **no execution that writes
two projects**: that would need several worktrees per execution, with their own review, validation and merge, and a half-applied cross-repository
change is exactly what the review mechanisms exist to avoid. A task that changes the frontend and the backend is two executions (two steps, or two
runs), each reviewed and merged on its own. A workflow run has one shared worktree, hence one working project; the scope is the run's.

## Starting the Harness

`initialize_workspace`: the deterministic analysis and creation, per project, up to three at a time, each failure isolated; a project with a valid
Harness is skipped without being read again, so re-running after a failure or after adding a project only does what is missing. Progress per project
is the event `harness:workspace-progress`; the report is `complete` only if no requested project failed. The workspace context is rebuilt afterwards,
saying which projects have none. Model-assisted analysis and review stay per project (the existing modals, now addressed by `projectId`).

## Consequences and limits

- The "writes several projects in one task" scenario is not one execution (above).
- A project's staleness check scans the project; the overview scans every project each time it is read.
- Consulted projects are told as Harness fragments; their raw files are readable only by Claude runs without edit tools.
- Workflow scope is per run, not per step.
