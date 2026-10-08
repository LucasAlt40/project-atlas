# 0028 — MCP foundation (Context & Tooling Platform, Phase D)

## Context

ADR 0026 chose the shape (a runtime-agnostic domain, a Claude adapter, strategy A: the runtime's own MCP support, configured by Atlas per
execution) and measured the Claude CLI without a model call. ADR 0027 fixed the invariant for MCP results: `ContextAuthority::Untrusted`,
never able to change a rule, permission, approval, policy or guardrail decision. Phase D builds the foundation, STDIO only, with Claude as
the one runtime that has an adapter. The design is in [context-and-tooling-platform.md](../context-and-tooling-platform.md), section 17.

## Decisions

1. **The domain names no runtime** (`domain/mcp.rs`): `McpConnection`, `McpTransport`, `McpGrant`, `ToolSelection`, `McpSupport`, `McpProblem`,
   `McpToolState`, `McpRecord`, `Secret`. The Claude adapter (`runtimes/claude_mcp.rs`) translates; nothing in the domain is a CLI flag.
2. **A connection is not an authorization.** A connection says "this workspace has this integration" and is stored in the configuration
   (`UserConfig.mcp_connections`) without any secret, switched off and granted to nobody. A grant (`mcp_grants`) says which agent,
   optionally which workflow and which step, may use it and which tools. The effective tool set of a step is a pure function
   (`application/mcp/plan.rs`): enabled → granted → policy allows → runtime has an adapter → configuration valid → tools can be held back
   → secrets present. The first failure is the reason it is not exposed.
3. **Six facts per tool, never one flag:** discovered, enabled, authorized, exposed (what Atlas arranged), reported (what the runtime listed:
   `None` is "did not say"), used. They are in the manifest (`McpRecord`) and the Inspector.
4. **The policy axis is in `security/`**, not beside it: `SecurityPolicy.mcp` (`Allowed` / `ApprovalRequired` / `Denied`), narrowed like every
   other setting; the read-only profile denies it, the developer profile allows it, and an older stored policy does not restrict it. Nothing
   is exposed without a grant either way, so the default is deny. `ApprovalRequired` is a finding (`mcp_approval_required`, Error): an ASK
   in a workflow step, a DENY without one.
5. **The guardrails see MCP through the existing path:** `McpFacts` in the Context Review (`mcp_unavailable`: Blocking for a required
   connection that cannot be given, Warning for an optional one), and `McpPlan::canonical()` in the evaluation fingerprint: connection
   configuration and credential versions, discovery, what each grant covers, what is held back, the policy. Any of them changing voids an
   earlier approval. No `McpSecurity`.
6. **STDIO is `executable + args[] + env`, never a shell.** Validation refuses shells (`sh`, `bash`, `cmd`, `powershell`, `env`…, by name in
   either path style), relative or spaced executables, dangerous variables (`PATH`, `NODE_OPTIONS`, `LD_*`…, `ATLAS_*`), and **any `${`**:
   the CLI expands `${VAR}` from Atlas's environment in a server's command, arguments and values, so a configuration could otherwise name a
   variable holding another connection's secret. Atlas writes those references itself, for its own secrets, and nothing else may.
7. **Secrets go to the OS credential store** through the `CredentialStore` port (`KeyringCredentialStore`, the `keyring` crate, used only in
   `infrastructure/`). The configuration holds the name and when each was stored. A secret reaches a server as `${ATLAS_MCP_<SERVER>_<NAME>}`
   in the config and as a value in the CLI process's **environment**: never in a file, an argument, the prompt, the manifest, the metrics,
   an event or a log (`Secret` has no `Display`/`Serialize` and a `Debug` that shows nothing). An unset variable is **not** an error for the
   CLI (the text stays literal), so Atlas checks presence first and reads the secret only after the guardrails allowed the step.
8. **Discovery is the runtime's own report**, not an MCP client of Atlas: `ModelRuntime::probe_mcp` starts the server through the CLI with a
   model that does not exist, which reports the server's status and the names of its tools and fails at the first request. It needs the
   connection to be switched on (that is the user's decision to let Atlas start this process) and its secrets stored. Names only: the report
   has no descriptions or schemas, and none are invented (the budget's `tool_definitions` stays `Unknown`).
9. **Holding tools back, and what that cannot do.** `--tools` does not limit MCP tools; `--disallowedTools mcp__<server>__<tool>` hides a
   discovered one; a tool the server adds later is listed anyway. A grant to the whole server is therefore the default granularity (the user
   trusts the server), and named tools need a discovery so the rest can be held back. When the runtime lists, before it asks any model
   anything, an MCP tool nobody authorized (a held-back tool that came back, one a server added, a server Atlas did not launch such as a
   connector of the user's), Atlas stops the step: it asks the session to terminate (possible only when the run is attached to a terminal),
   fails the execution (`mcp_tool_not_authorized`) and does not hand its answer on. This is detection plus a best-effort stop, not
   prevention; a request may already have been sent.
10. **Failure never becomes permission.** A connection that cannot be started as planned (secret vanished, store locked) is left out of the
    step, and a **required** one stops it (`mcp_connection_unavailable`). A text-only run is never given a server (`--tools ""` does not turn
    MCP off, so the only way is not to give it). A runtime nobody investigated gets nothing (`McpSupport::NotInvestigated`).
11. **The invariant for results stands as in ADR 0027:** an MCP tool's result is `Untrusted`; for a CLI runtime it never passes through Atlas
    at all. A test shows text claiming a person approved cannot answer the question the policy asks.

## Evidence (no model call)

- Claude Code 2.1.285, real CLI, the Phase A harmless server: the probe sees `connected` and `echo_static`; a secret travels environment →
  `${ATLAS_MCP_…}` → the server, which reports it back as a tool name; a server that cannot start is `failed` with no tools; a held-back
  tool is not listed and a tool the server adds later is (and the real `init` event reaches Atlas's stream).
- 20 execution-level tests with the real service, the real security service as policy resolver, the real MCP service over a store in memory
  (grants by agent, workflow and step; workspace isolation; read-only denial; revoke and switch-off; named tools; required and optional
  failures; secrets never in anything kept; approvals voided by command, credential version, discovery, grant or policy; the stop on an
  unauthorized tool, checked by mutation).

## Consequences

- Only Claude can be given MCP servers. The other four runtimes are `NotInvestigated`: a connection granted to their agents is simply not given,
  with a finding.
- The macOS Keychain round trip was exercised by the ignored real test (exact name). The Windows and Linux stores were not: `keyring` compiles
  for `x86_64-pc-windows-msvc`, the whole crate could not be built for it here (no `llvm-rc`), the Linux target is not installed.
- The connection UI arrived in Phase E (doc section 18): the Integrations panel and a catalogue with DevTools (pinned) and Figma (listed, not addable).
- The stop on an unauthorized tool no longer needs a terminal session: `SessionRegistry::stop` ends a terminal session or makes the pipe runner
  kill the process group (see doc section 17.7). A request may already have reached the model when the report arrives.
- Descriptions and schemas of tools are unknown to Atlas, so it cannot show or budget them.
- HTTP transport and OAuth (Figma's official server) are not built; `McpTransport::Http` exists and is refused.

## Not done

Chrome DevTools and Figma integrations (Phase E; Figma needs HTTP and OAuth), any UI to add or grant a connection, an MCP client in Atlas,
a Tool Gateway or proxy, HTTP and its SSRF layer, per-call interception, MCP for the other four runtimes, a real model run.
