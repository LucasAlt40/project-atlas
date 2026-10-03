# ADR 0004 — Providers, runtimes and models

**Status:** accepted

V0.2 modelled "provider + model". That conflated two things: _who_ provides an AI (Anthropic) and _how_
Atlas reaches it on this machine (the Claude CLI today, the Anthropic API later). An agent now references a
**runtime** (`runtimeId`) and a model; the provider is a property of the runtime, used only for grouping in
the UI.

A runtime implements `ModelRuntime` (`info`, `detect`, `list_models`, `execute`) and declares
`RuntimeCapabilities`. The execution layer sees only that trait, so supporting a new tool means adding an
adapter, never editing agents, prompts or executions. Differences between tools (no model discovery, no
system-prompt flag, different auth) are expressed as capabilities and structured status, not as branches in
shared code.

Runtimes own their executable, arguments, environment and working directory. The webview only names a
`runtimeId`. All process execution goes through the `ProcessRunner` port.

Authentication is modelled (`AuthKind`, `AuthState`) but only the tool's own signed-in session is
implemented. API keys must be stored in the operating system's credential store, never in `config.json`.
