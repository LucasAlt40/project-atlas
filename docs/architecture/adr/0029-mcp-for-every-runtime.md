# ADR 0029 — MCP for every runtime that can take it

Status: accepted. Builds on [ADR 0028](0028-mcp-foundation.md); changes none of its invariants.

## Context

Phase D gave MCP servers to Claude only, because only Claude had been measured. A person who granted an integration to an agent on another
runtime saw nothing happen: the manifest said `runtime_unsupported`. The ask: every runtime Atlas supports should be able to use MCP.

## What was measured (real CLIs, no model call)

| Runtime     | Per-run configuration (nothing of the user's is written)         | Secret                                       | Tool filter                                      | Look without a model                                 | Only Atlas's servers? |
| ----------- | ---------------------------------------------------------------- | -------------------------------------------- | ------------------------------------------------ | ---------------------------------------------------- | --------------------- |
| Claude      | `--mcp-config <file>` + `--strict-mcp-config`                    | `${VAR}` from the CLI's environment          | deny-list (`--disallowedTools`), needs discovery | `init` report: status **and tool names**             | yes                   |
| Gemini      | `GEMINI_CLI_SYSTEM_SETTINGS_PATH=<file>` with `mcpServers`       | `$VAR` in `env` values                       | allow-list (`includeTools`), no discovery        | `gemini mcp list`: status only (stderr, no terminal) | no                    |
| OpenCode    | `OPENCODE_CONFIG_CONTENT=<json>` with `mcp`                      | `{env:VAR}` in `environment` values          | none verified: named-tool grants refused         | `opencode mcp list`: status only                     | no                    |
| Codex       | `-c mcp_servers.<name>.<key>=<toml>`                             | `env_vars=["NAME"]` forwards `NAME` from env | allow-list (`enabled_tools`), no discovery       | none (`codex exec` reports nothing about servers)    | no                    |
| Antigravity | none found: `agy mcp add` writes the user's global configuration | —                                            | —                                                | —                                                    | —                     |

Each "secret" row was shown by a server that starts only if the secret reached it (spike `MODE=requireenv`): `connected` with the right value,
`failed`/`Disconnected` with a wrong one. Codex's `-c` arguments were checked with `codex mcp get --json`.

## Decisions

1. **Capabilities, not a boolean.** `RuntimeCapabilities.mcp_features` says how a runtime filters tools (`unsupported`, `deny_list`,
   `allow_list`), what a probe shows (`none`, `status_only`, `tools`) and whether it loads only Atlas's servers (`strict`). The plan uses them:
   a grant that names tools needs discovery on a deny-list runtime, none on an allow-list one, and is **not honoured** (the server is not given,
   `tool_filter_unsupported`) where nothing holds the rest back (no runtime is in that state today).
2. **Antigravity stays `Unsupported`**, with the evidence. Giving it a server would mean changing the user's global configuration, which Atlas
   does not do.
3. **Not strict where not measured.** Gemini, OpenCode and Codex merge Atlas's servers with the user's own. Atlas says so (surface entry
   "user controlled, not observed"; a note where a grant is made) instead of claiming to have excluded them. Gemini's
   `--allowed-mcp-server-names` was not passed: its effect was not verified.
4. **Calls must be allowed.** A non-interactive run cannot ask a person, so Claude gets `--allowedTools` for what was granted and Gemini
   `trust` on the servers it gives: the person's grant (and the policy and the guardrails before the step) is the decision. Read in the CLIs'
   references, **not run** (a run costs tokens).
5. **Names that cannot be told apart cannot coexist.** `web-2` and `web_2` map to one environment variable; they are refused together, a name may
   not hold `__`, a second server with an already used key is left out of a launch. Configurations may not contain any of the CLIs' expansion
   syntaxes (`${…}`, `$VAR`, `{env:…}`, `{file:…}`).
6. **Codex secrets travel under the server's own variable name** (what `env_vars` forwards); two servers using one name cannot be launched together.

## Structure: Strategy and Factory

`runtimes/mcp_adapter.rs` defines `McpAdapter` (program, features, `prepare`, `probe_plan`, `classify`; `probe` is provided from the plan) with one
implementation per CLI, and `McpAdapterFactory::create(McpDialect)`. A runtime holds an `Arc<dyn McpAdapter>`: its `execute` calls
`prepare_for(adapter, request)` and adds the arguments and environment it gets; `probe_mcp` and `mcp_tools_in` delegate; its declared
capabilities come from `McpAdapterFactory::declared`. The same shape builds the catalogue (`McpPreset`, `McpPresetFactory`). What this removed:
four copies of the "write a file, build the environment, run the probe, read the answer" sequence, and the capability literals that said the
same thing in each runtime. The measured behaviour is unchanged (the real-model tests pass before and after).

## Verified with real models (the user authorized the spend; a few tokens each)

Through the whole of Atlas (real guard, real CLI, real MCP service, a step through `ExecutionService`), against a harmless server that returns a
nonce no model can guess (`executions/real_runtime_tests.rs`, run by exact name):

| Runtime  | Named tools (`echo_static` only, `env_zzz` left out)     | Whole server | Chrome DevTools entry of the catalogue (pinned, headless) |
| -------- | -------------------------------------------------------- | ------------ | --------------------------------------------------------- |
| Claude   | called; the other tool: "NOT AVAILABLE"; recorded `used` | both called  | opened `https://example.com`, listed the pages            |
| Codex    | called; the other tool: "NOT AVAILABLE"; recorded `used` | both called  | opened `https://example.com`, listed the pages            |
| OpenCode | called; the other tool: "NOT AVAILABLE"; recorded `used` | both called  | opened `https://example.com`, listed the pages            |

What the real runs showed, and fixed:

- **Claude refused every call without `--allowedTools`** (a permission denial in a non-interactive run): shown directly, then fixed.
- **Codex refused with "MCP tool call requires approval, but approval policy is never"** until `default_tools_approval_mode="approve"` was set.
- **OpenCode** needed nothing for approval; its allow-list is the `tools` map (`<server>_*: false`, `<server>_<tool>: true`). A `permission` rule did
  not give a clear result and is not used.
- **A call OpenCode reports only once finished** was not counted: the probe now counts completed calls too (per tool, the larger of started and
  completed), so `tool_calls` and `used` are right for it.
- **Antigravity** runs headless with the MCP servers of the user's own configuration (`call_mcp_tool`, with the server and tool as parameters) and
  reached the user's `chrome-devtools-mcp` when the task named it; Atlas shows the call as `mcp__<server>__<tool>`. It does not use a server unless
  the task asks: **the instruction of an agent on Antigravity has to say so**.

## Not verified

- **Gemini CLI**: the account is refused by the CLI ("no longer supported for Gemini Code Assist for individuals"), so no model call was possible.
  Its settings file, the secret and the probe were verified without a model; `trust` letting a call through was not.
- Tool names a Gemini run reports (so its `used` stays unknown); Codex reports `mcp_tool_call` items, shown as `mcp__server__tool`, and OpenCode
  `<server>_<tool>`, both recorded as `used`.
- A managed Gemini system settings file would be replaced for the run by Atlas's.
