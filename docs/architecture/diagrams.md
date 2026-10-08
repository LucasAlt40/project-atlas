# Atlas in diagrams

Pictures of what the code does today. They are [Mermaid](https://mermaid.js.org) (GitHub, VS Code and most viewers draw them; the source stays in git
and is reviewed like code). Every box exists in the code; what does not exist yet is drawn dashed and says so. The words behind each picture are in
[context-and-tooling-platform.md](context-and-tooling-platform.md).

## 1. Atlas and the agent harness

A coding-agent **harness** (Claude Code, Codex, Gemini CLI…) holds the context window, runs the agentic loop and executes the tools. Atlas does not
replace it: **Atlas sits one level above, decides what goes into the harness, and watches what comes back.** That is the main difference from the
usual "harness" picture.

```mermaid
flowchart LR
    User(["Person"]) -->|"task, answers, Apply"| Atlas

    subgraph Atlas["ATLAS — the layer above (what this repo builds)"]
        direction TB
        Rules["Rules<br/>global, project, agent, task"]
        Ctx["Context engine<br/>sections, priorities, budget"]
        Plan["ContextPlan to guardrails to ContextManifest<br/>+ prompt hash"]
        Gate["Guardrails<br/>ALLOW, ASK, DENY, TRANSFORM"]
        MCPcfg["MCP connections + grants<br/>no secrets in config"]
        Rules --> Ctx --> Plan --> Gate
        MCPcfg --> Plan
    end

    Gate -->|"ONE delivery: the prompt (and its hash)"| Harness

    subgraph Harness["HARNESS — a runtime CLI (Claude Code today)"]
        direction TB
        Window["Context window<br/>its own system prompt, skills,<br/>CLAUDE.md, memory, history"]
        Loop(("Agentic<br/>loop"))
        Tools["Tools run here<br/>built-in + MCP servers"]
        Window <--> Loop
        Loop -->|"tool_use"| Tools
        Tools -->|"tool_result"| Window
    end

    Loop <-->|"requests"| LLM["LLM<br/>provider"]
    Harness -->|"answer, usage, init report,<br/>tool events"| Watch
    Watch["Atlas watches:<br/>manifest, metrics, unauthorized tool stop"] --> User
    MCPcfg -.->|"--mcp-config file, secrets in env"| Tools

    classDef atlas fill:#134e4a,stroke:#2dd4bf,color:#ecfeff;
    classDef harness fill:#3b0764,stroke:#a78bfa,color:#f5f3ff;
    class Rules,Ctx,Plan,Gate,MCPcfg,Watch atlas;
    class Window,Loop,Tools harness;
```

How each box of the usual picture maps:

| In the usual picture      | In Atlas                                                                                                                                                                                  |
| ------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| System prompt             | The runtime's own is **not Atlas's** (surface entry "runtime controlled"). Atlas's instructions travel inside the prompt body (`SystemPromptChannel::Unsupported` for all five runtimes). |
| Rules (`CLAUDE.md`)       | Atlas has its own Rules (`.atlas/context/rules.md`, `UserConfig.rules`, task rules). The user's `CLAUDE.md` is loaded by the CLI and Atlas lists it as "not observed".                    |
| Skills                    | Turned off for Claude runs (`--disable-slash-commands`).                                                                                                                                  |
| Tool schemas / MCPs       | Atlas decides **which servers and tools** (grants, plan). It does not see schemas or descriptions (unknown, so `tool_definitions` is Unknown in the budget).                              |
| `Memory.md`               | Not built. Memory is designed (doc section 6), explicitly postponed.                                                                                                                      |
| History                   | Part of the prompt Atlas assembles (conversation and task context), under the same budget and priorities.                                                                                 |
| Context window / payload  | The **Manifest**: what was delivered, with a SHA-256 of the exact payload. What the CLI adds around it is shown as unobserved, never claimed.                                             |
| Agentic loop              | Owned by the harness. Atlas starts it, can stop it, and sees only what the CLI reports.                                                                                                   |
| Tools executed in harness | Same. Atlas never runs a tool itself and has no MCP gateway or proxy.                                                                                                                     |

## 2. Layers of the application

```mermaid
flowchart TB
    subgraph FE["Frontend: React + TypeScript — presentation only"]
        UI["features/*, Inspector, panels"] --> Cmd["lib/tauri/commands.ts<br/>typed CommandMap"]
    end
    Cmd -->|"Tauri commands, allow-listed"| Commands

    subgraph Core["Core: Rust (src-tauri/src)"]
        Commands["commands/<br/>thin adapters"] --> App
        App["application/<br/>use cases + ports"] --> Dom["domain/<br/>plain models and rules"]
        Infra["infrastructure/ + platform/<br/>adapters: processes, config file, Keychain"] -.->|"implement ports"| App
        Lib["lib.rs<br/>composition root"] -.-> Infra
    end

    Infra --> OS[("OS: processes, files,<br/>credential store")]

    classDef note stroke-dasharray: 4 3;
    class Lib note;
```

The webview never touches files, processes, Git, credentials or the network. Dependencies point inward (ADR 0002).

## 3. One step, from request to evidence

What `ExecutionService::run_step` does, in order.

```mermaid
flowchart TD
    A["Task + agent + runtime + model"] --> B["Resolve rules<br/>by scope and strength"]
    B --> C["Build context sections<br/>and apply the budget"]
    C --> D["Plan MCP<br/>connections, grants, policy, runtime support"]
    D --> E["ContextPlan<br/>+ Context Review claims"]
    E --> F{"Guardrails"}
    F -->|"DENY"| X["Step fails, with the reason"]
    F -->|"ASK"| P["Paused for a person<br/>approval bound to a fingerprint"]
    P -->|"answered"| G
    F -->|"ALLOW / TRANSFORM"| G["Read secrets, only now<br/>and only for servers exposed"]
    G --> H["runtime.delivery()<br/>the single payload"]
    H --> I["Runtime CLI runs<br/>stream watched"]
    I --> J{"Runtime lists a tool<br/>nobody authorized?"}
    J -->|"yes"| K["Stop: kill the process group<br/>fail, discard the answer"]
    J -->|"no"| L["Answer, usage, tools reported and used"]
    L --> M["ContextManifest<br/>+ hash + metrics"]
    K --> M
    M --> N["Review in the Inspector<br/>Apply stays human"]

    classDef gate fill:#7c2d12,stroke:#fb923c,color:#fff7ed;
    class F,J gate;
```

## 4. Who decides what: authority and trust

Two axes that are never merged: **where content came from** (`SourceTrust`) and **what it is allowed to be for the model** (`ContextAuthority`).
Only the user's own rules can bind.

```mermaid
flowchart LR
    subgraph Origins["Origin of a rule"]
        U["User"]
        PF["Project file"]
        GN["Generated"]
        EX["External"]
    end
    U --> Bind["May be Mandatory"]
    PF --> Bind
    GN --> NoBind["Never binding"]
    EX --> NoBind

    subgraph Authority["ContextAuthority of an item"]
        Au["Authoritative"] --> In["Instructional"] --> If["Informational"] --> Un["Untrusted"]
    end
    Bind --> Au
    Tool["MCP tool result<br/>other agent's output, files"] --> Un
    Un -.->|"can NEVER change"| Never["rules, permissions, approvals,<br/>policy, guardrail decisions"]

    classDef bad fill:#7f1d1d,stroke:#f87171,color:#fef2f2;
    class Never,NoBind bad;
```

## 5. MCP: from a connection to a tool in a step

```mermaid
flowchart TD
    Conn["McpConnection<br/>off by default, config without secrets"] --> En{"Enabled by<br/>the user?"}
    En -->|"no"| Out1["not exposed"]
    En -->|"yes"| Gr{"A grant covers this<br/>agent, workflow, step?"}
    Gr -->|"no"| Out1
    Gr -->|"yes"| Pol{"Agent policy allows MCP?<br/>read-only: never"}
    Pol -->|"no"| Out1
    Pol -->|"yes"| Sup{"Runtime has an adapter?<br/>only Claude today"}
    Sup -->|"no"| Out1
    Sup -->|"yes"| Val{"Configuration valid?<br/>no shell, no variable tricks"}
    Val -->|"no"| Out1
    Val -->|"yes"| Tl["Tools: whole server or named only<br/>discovery refreshed if old"]
    Tl --> Sec["Secrets from the OS store<br/>after guardrails ALLOW"]
    Sec --> Ad["ClaudeMcpAdapter"]
    Ad --> Cfg["--mcp-config temp file 0600<br/>references only"]
    Ad --> Env["secrets in the CLI environment"]
    Ad --> Dis["--disallowedTools for named-only grants"]
    Cfg --> CLI["Claude CLI starts the servers"]
    Env --> CLI
    Dis --> CLI
    CLI --> Rep["init report lists tools"]
    Rep -->|"a stranger"| Stop["Stop the run"]
    Rep -->|"all authorized"| Go["Model runs; manifest records<br/>reported and used tools"]

    Http["HTTP / OAuth (Figma)<br/>DevTools integration, connection UI"]:::later
    classDef later stroke-dasharray: 5 4,fill:none,color:#94a3b8;
```

The dashed box is Phase E: not built.

## Keeping these true

A diagram that drifts is worse than none. When a step in section 3 or 5 changes in code, change it here in the same commit. The Mermaid blocks are
plain text, so a reviewer sees the diff.
