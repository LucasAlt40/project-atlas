# 0026 — Context & Tooling Platform (Phase A: architecture and spike)

## Context

Atlas builds one prompt per execution and hands it to a CLI runtime. Optimization Layer phases 0–5 (ADRs 0020–0025) gave it
measurement, a Context Engine, Skills, runtime isolation and Guardrails. What it lacks to manage context and tools as a platform:
a budget that belongs to a runtime + model, a record of what was really delivered, Rules, Memory, and MCP (which Claude is launched
without, on purpose, ADR 0024). The audit that preceded this ADR and the full design are in
[context-and-tooling-platform.md](../context-and-tooling-platform.md). This ADR records the decisions; it does not edit ADRs 0010–0025.

## Decisions

1. **One execution resolution.** Context, Rules, Memory and Tooling/MCP feed the same per-step resolution, reviewed by the existing
   guardrails and delivered by the runtime adapter. They are not independent features. No second Context Engine, no second permission
   system, no `McpSecurity`.
2. **`ExecutionBudget`** replaces the two `ContextBudget`s. It is a budget of one execution, with sections (system prompt, rules,
   Harness, skills, memory, history, handoffs, tool definitions, tool results) and a derived `available_for_task_context`. Every figure
   carries `source` (Reported / Configured / Default / Unknown) **and** `precision` (Exact / Estimated / Unknown) **and**, when
   estimated, a `method`. Reported ≠ Exact; Configured ≠ Exact. `chars/4` stays the only estimate, labelled `Estimated`.
3. **`ModelLimits` is keyed by (runtime, model)**, never by model alone or by provider. Resolution: reported → Atlas known table →
   workspace override → agent/workflow override; an override can narrow, never exceed a Reported or Exact ceiling, and the clamp is
   recorded. An unknown window stays unknown. A Required item that does not fit is never cut: DENY or ASK.
4. **Context Plan ≠ Context Manifest.** The Plan is what Atlas meant to assemble; the Manifest is what it prepared and delivered
   (seventeen questions in the design page). The Manifest holds references, fingerprints, sizes and reasons, not secrets or copies.
5. **Prompt hash** is computed immediately before `runtime.execute()` over the payload the adapter will deliver. The adapter owns one
   `delivery()` function used by both delivery and hashing; a spy-runner test asserts `manifest.prompt_hash == hash(captured payload)`.
   Where the payload cannot be represented, the manifest says so instead of hashing something else.
6. **`RuntimeSurface`.** Sources of what the agent sees are classified `AtlasControlled / RuntimeControlled / UserControlled / Unknown`,
   each `Reported / Declared / NotObserved`. For Claude, the plugins, `CLAUDE.md`, hooks, permission rules and auto-memory are
   `UserControlled` and stay so; Atlas does not claim to control them. Total isolation is not part of this work.
7. **Rules are a domain concept** (id, scope, priority, content, mandatory, provenance, fingerprint, enabled, applicability), resolved
   deterministically into `ContextItem`s. A Rule is neither a Permission nor the System Prompt; prompt-injection tests are required.
   Harness user constraints are not copied into Rules.
8. **Memory is a domain, not a JSON file**, and only for what neither Harness nor `SharedExecutionState` holds. Scopes kept: User,
   Workspace, Agent. Project (≈ Workspace, covered by Harness) and Workflow (covered by `SharedExecutionState`) are not Memory scopes.
   Retrieval by filters; no vectors, no RAG. Not in Phase B.
9. **MCP domain is runtime-agnostic** (`McpConnection`, `McpServer`, `McpTool`, `McpTransport`, `McpCapabilities`, `ToolPolicy`,
   `ToolAccess`); `ClaudeMcpAdapter` translates to `--mcp-config --strict-mcp-config`. Other runtimes: Unsupported / Not investigated.
   Strategy A (runtime-native, config generated per execution). No Tool Gateway or universal proxy.
10. **Workspace → Connection → Policy → Agent/Workflow/Step → Effective Tool Set.** A connection is not an authorization. Six
    separate states: server discovered, tool discovered, enabled, authorized, exposed to runtime, used.
11. **Guardrails extend `security/`**: an MCP axis in `SecurityPolicy` (default deny), an honest tri-state MCP capability in
    `RuntimeCapabilities`, and MCP servers, the effective tool set and a tool-configuration fingerprint in the evaluation fingerprint,
    so a change invalidates a previous approval. MCP results are untrusted data.
12. **STDIO is `executable + args[] + env`**, never a shell string; the user authorizes the local process explicitly. **HTTP is reserved**
    in `McpTransport` and not built; when built it needs its own destination/DNS/IP/redirect/TLS/auth layer.
13. **`CredentialStore` is a domain port**; the OS-native keyring is an infrastructure dependency. Secrets never enter prompt,
    manifest, metrics, logs or `UserConfig`.
14. **System prompt unchanged.** `--append-system-prompt` is not adopted; the architecture reserves a per-runtime
    `SystemPromptChannel` (default `Inline`).
15. **Figma is not a Phase E stdio integration**: the official server is HTTP + OAuth. It waits for HTTP transport and an OAuth
    decision. Chrome DevTools MCP (stdio) can come first, pinned to an explicit version, never started or authorized by default.

## Evidence (spike, no model call)

Claude Code 2.1.285, Atlas's launch arguments + `--mcp-config` + `--strict-mcp-config`, model that does not exist. Details and table:
design page, section 10; reproducible with `docs/architecture/spikes/mcp-claude/`.

- Only the server Atlas passes appears (`mcp_servers`: `connected`, `source: dynamic`); tools are named `mcp__<server>__<tool>`.
- **`--tools` does not limit MCP tools**; `--tools ""` still exposes them; `--allowedTools` does not hide; `--disallowedTools
mcp__<server>__<tool>` removes a tool from the list.
- A tool with an invalid schema makes the CLI drop **all** the server's tools while still reporting `connected`.
- A server that fails to start is `failed` and the run continues; invalid JSON config makes the CLI exit.
- No leftover MCP process after any run; `~/.claude/settings.json` and `CLAUDE.md` untouched.

## Consequences

- Per-tool authorization for Claude through `--disallowedTools` is a deny-list and fails open for tools a server adds later. Phase D
  must prove a mitigation (server-granular authorization, and checking `init.tools` and stopping before the first request) or document
  per-tool authorization as best effort.
- A text-only run must not receive an MCP config.
- `connected` is not evidence that a tool is usable; Atlas compares expected and exposed tools.
- Under strategy A the CLI starts and stops stdio servers outside Atlas's process guard, and tool results do not pass through Atlas.
  Atlas's authority is what it exposes at launch and what it hands on afterwards.
- Real input-token counts, mid-run server death, cancellation behaviour, auto-memory/`CLAUDE.md` reaching the model, and
  `--append-system-prompt` semantics need a real model run. None has been made; Atlas will ask before spending tokens.

## Phase B (implemented)

Decisions 2–6 are implemented as written; the details and the refinements are in the design page, section 15. In short: the
engine's and the Harness's `ContextBudget` are replaced (`ExecutionBudget`, `HarnessBudget`); `Precision` is added beside the phase-0
`TokenSource` rather than renaming it; `ModelRuntime::delivery()` is the single source of the payload and `manifest.promptHash` is
its SHA-256 (`sha2`, already in the tree), proved with a spy `ProcessRunner` through the real service and for all five adapters;
`RuntimeSurface` classifies by control and observation and confirms only the delivered prompt as received by the model. No model
limit is known to any runtime, so every limit is `Unknown` unless the user configures one. Not changed: Apply, worktrees, handoffs,
workflow semantics, the guardrails, the Claude isolation of ADR 0024.

## Not done

Rules, Memory, MCP, the system-prompt channel, the guardrail evaluation's fingerprint inside the manifest, and everything the design
page lists under "Still unknown or not done".
