# 0024 — Optimization Layer, phase 4: what a runtime can reach (surface and isolation)

## Context

Phase 4 of the Optimization Layer was written around tool and MCP selection: a catalog, a selector, deferred loading. Before
building any of it, the question was what an Atlas agent is actually given. Atlas does not choose tools through a catalog: each
runtime adapter passes fixed flags (`claude -p --tools Read,Grep,Glob`), and what the CLI does around those flags is the CLI's.

That was measured, not assumed. Claude Code reports what it loaded in the `init` message of its `stream-json` output. Starting the
real CLI (2.1.285) with **exactly the arguments Atlas used** and a model that does not exist (it fails at the first request, so no
model is called and nothing is spent) gave, on a developer machine with the usual connectors and plugins:

| Reported by the CLI at start | Atlas's launch before                                                                                                                                | With `--strict-mcp-config --disable-slash-commands` |
| ---------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------- |
| Tools the model can call     | **11**: `Read`, `Grep`, `Glob` and eight `mcp__claude_ai_Claude_Docs__*` (`batch`, `create`, `delete`, `export`, `guide`, `query`, `read`, `update`) | **3**: `Glob`, `Grep`, `Read`                       |
| MCP servers loaded           | 8 (Docs, Atlassian, Figma, Google Drive, Calendar, Gmail, Notion…; one connected, the rest waiting for sign-in)                                      | 0                                                   |
| Skills / slash commands      | 48 / 84                                                                                                                                              | 0 / 0                                               |

So an agent Atlas described as read-only (`tool_access: none`: "no edit, no shell, no web tool") could, in principle, call the
user's write-capable connector tools. Whether a headless run lets such a call through depends on the user's permission settings,
which Atlas neither reads nor controls; the tool was in the model's hands either way. The same listing also costs context on every
execution: skill and tool descriptions the agent never needed.

## Decision

**Isolate the Claude runtime from the user's own extensions, unconditionally** (`launch_args` in `runtimes/claude.rs`):

- `--strict-mcp-config`, with no `--mcp-config`: no MCP server is loaded.
- `--disable-slash-commands`: no skills. Atlas has its own skills layer (ADR 0022) and decides what an agent gets.

This is a hardening more than an optimization (the stated tool surface becomes true), so it has no switch. It applies to every way
Atlas launches Claude (read-only, with edits, text-only). It does not touch authentication: the CLI keeps using the user's own
sign-in (`--bare` would not, which is why it is not used).

Not done, on purpose: `--bare` or `--restricted` (they change authentication or settings handling, a larger step), and
`--setting-sources` (the user's hooks, permission rules and `CLAUDE.md` files still load; they are not in the measured surface above
and need their own measurement).

**Record what the runtime says it loaded** (observability, per execution). The Claude adapter reads the `init` message into
the facts it already reports (`toolsExposed`, `mcpServers`, `skillsLoaded`, `slashCommands`, `pluginsLoaded`); the metrics keep
them as `tools.exposed` and `extensions` (MCP servers, skills, slash commands, plugins), and `tools.used` counts the tools the model
called by name from the runtime's tool events. A runtime that does not say leaves them `None`: nothing is inferred.

An opt-in test (`cargo test real_claude_surface -- --ignored`) repeats the measurement against the real CLI with no model call.

## Not built (and why)

- **ToolCatalog / ToolSelector / MCP registry / deferred discovery.** Atlas gives an agent a fixed tool list per mode (read-only,
  edit, none) and, now, no MCP at all. There is no choice for a selector to make and no MCP server Atlas itself offers. The
  moment an agent needs an MCP server, the way is to generate an explicit `--mcp-config` per execution with only that server; the
  runtime's own tool search and deferred loading are then the native capability to use.
- **OpenCode, Gemini and Antigravity.** Their surface was not measured: OpenCode reads its own configuration and may load the user's
  MCP servers; Gemini and Antigravity deny shell and file writes in headless mode, and their MCP and extension loading is not known.
  Until measured, nothing is claimed for them and their `init`-style facts are absent.
- **Dropping the user's `CLAUDE.md`, hooks and permission rules** from Atlas runs. They still apply. Doing it right needs the
  authentication-safe flags above to be understood first, and a measurement of how much context they add.

## Consequences

- A Claude agent in Atlas can call exactly `Read`, `Grep` and `Glob` (plus `Edit` and `Write` when edits were granted). Anything
  more is a decision to take explicitly, not an inheritance from whoever's machine it runs on.
- The prompt the CLI builds around Atlas's is smaller by the listing of 48 skills, 84 slash commands and the MCP tool definitions
  (not yet measured in tokens: a real run is needed for the runtime-reported input, which Phase 0 records).
- Atlas runs are more reproducible: they no longer depend on which connectors the user happens to have signed in.
