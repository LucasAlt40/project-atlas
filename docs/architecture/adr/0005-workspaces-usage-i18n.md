# ADR 0005 — Workspaces, usage and localization

**Status:** accepted

## Workspace and project context

A **workspace** is the unit of work: a project folder plus the agents placed in it and their conversations. Before this
decision the app had one in-memory layout and a hard-coded project. Now `Workspace { id, name, projectPath,
description, createdAt, updatedAt, layout }` is persisted, and several can exist.

Agents, personalities and installed runtimes stay **global** (an agent can be placed in several workspaces, so its
configuration is reused instead of duplicated). Placement, conversations, runs and usage are **workspace-scoped** and keyed
by workspace + agent. The 2 × 2 layout and the four-agent display limit are UI/layout data, not domain rules.

The project folder is the context boundary. Choosing it records a path and nothing more; the only thing read from it is the
list of top-level file names (and `package.json` dependency names) to name the technologies, locally and
deterministically. Permissions and sandboxing are a later milestone.

Persistence reuses `config.json` behind the existing `ConfigStore` port (workspaces, settings, usage ledger). A database is
deliberately not introduced yet; the port keeps the option open. Secrets are never stored there.

## Usage is authoritative or unavailable

Every figure is either what a runtime reported for one execution, a sum of those over executions Atlas observed
(`atlas_calculated`), or a provider-reported quota. A figure a runtime did not report is `null`, never `0`, and
sums say how many runs they cover. Currencies are not mixed. Runtimes advertise `usageMetrics`, `costMetrics` and
`quotaMetrics` as capabilities, so the UI says "not available for this runtime" instead of guessing. Atlas-tracked
usage is not the provider's billing, and the UI says so.

## Localization

The interface is translated in the frontend (pt-BR default, en-US), with the language persisted as a global setting. The
core never produces user-facing text: it returns stable codes (`AppError.code`, `failureKind`, runtime `notice`,
`modelHint`, event kinds) and the UI words them. Adding a language is a new dictionary plus an entry in `LANGUAGES`.
