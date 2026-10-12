# O Atlas em diagramas

Figuras do que o código faz hoje. São [Mermaid](https://mermaid.js.org) (GitHub, VS Code e a maioria dos visualizadores os
desenham; o código-fonte fica no git e é revisado como código). Toda caixa existe no código; o que ainda não existe é
desenhado tracejado e diz isso. As palavras por trás de cada figura estão em
[context-and-tooling-platform.md](context-and-tooling-platform.md).

## 1. O Atlas e o harness de agentes

Um **harness** de agente de código (Claude Code, Codex, Gemini CLI…) guarda a janela de contexto, executa o loop agêntico
e executa as ferramentas. O Atlas não o substitui: **o Atlas fica um nível acima, decide o que entra no harness e observa
o que volta.** Essa é a principal diferença em relação à figura usual de "harness".

```mermaid
flowchart LR
    User(["Pessoa"]) -->|"tarefa, respostas, Apply"| Atlas

    subgraph Atlas["ATLAS — a camada acima (o que este repositório constrói)"]
        direction TB
        Rules["Rules<br/>global, projeto, agente, tarefa"]
        Ctx["Context engine<br/>seções, prioridades, orçamento"]
        Plan["ContextPlan para guardrails para ContextManifest<br/>+ hash do prompt"]
        Gate["Guardrails<br/>ALLOW, ASK, DENY, TRANSFORM"]
        MCPcfg["Conexões MCP + concessões<br/>sem segredos na configuração"]
        Rules --> Ctx --> Plan --> Gate
        MCPcfg --> Plan
    end

    Gate -->|"UMA entrega: o prompt (e o seu hash)"| Harness

    subgraph Harness["HARNESS — uma CLI de runtime (Claude Code hoje)"]
        direction TB
        Window["Janela de contexto<br/>seu próprio system prompt, skills,<br/>CLAUDE.md, memória, histórico"]
        Loop(("Loop<br/>agêntico"))
        Tools["As ferramentas rodam aqui<br/>embutidas + servidores MCP"]
        Window <--> Loop
        Loop -->|"tool_use"| Tools
        Tools -->|"tool_result"| Window
    end

    Loop <-->|"requisições"| LLM["Provedor<br/>do LLM"]
    Harness -->|"resposta, uso, relatório init,<br/>eventos de ferramenta"| Watch
    Watch["O Atlas observa:<br/>manifest, métricas, parada de ferramenta não autorizada"] --> User
    MCPcfg -.->|"arquivo --mcp-config, segredos no env"| Tools

    classDef atlas fill:#134e4a,stroke:#2dd4bf,color:#ecfeff;
    classDef harness fill:#3b0764,stroke:#a78bfa,color:#f5f3ff;
    class Rules,Ctx,Plan,Gate,MCPcfg,Watch atlas;
    class Window,Loop,Tools harness;
```

Como cada caixa da figura usual se mapeia:

| Na figura usual                   | No Atlas                                                                                                                                                                                                    |
| --------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| System prompt                     | O do próprio runtime **não é do Atlas** (entrada de superfície "controlado pelo runtime"). As instruções do Atlas viajam dentro do corpo do prompt (`SystemPromptChannel::Unsupported` nos cinco runtimes). |
| Rules (`CLAUDE.md`)               | O Atlas tem suas próprias Rules (`.atlas/context/rules.md`, `UserConfig.rules`, rules de tarefa). O `CLAUDE.md` do usuário é carregado pela CLI e o Atlas o lista como "não observado".                     |
| Skills                            | Desligadas nas execuções do Claude (`--disable-slash-commands`).                                                                                                                                            |
| Esquemas de ferramentas / MCPs    | O Atlas decide **quais servidores e ferramentas** (concessões, plano). Ele não vê esquemas nem descrições (desconhecidos, então `tool_definitions` é Unknown no orçamento).                                 |
| `Memory.md`                       | Não construída. A Memory está desenhada (doc seção 6), explicitamente adiada.                                                                                                                               |
| Histórico                         | Parte do prompt que o Atlas monta (conversa e contexto da tarefa), sob o mesmo orçamento e prioridades.                                                                                                     |
| Janela de contexto / payload      | O **Manifest**: o que foi entregue, com um SHA-256 do payload exato. O que a CLI acrescenta em volta é mostrado como não observado, nunca afirmado.                                                         |
| Loop agêntico                     | Pertence ao harness. O Atlas o inicia, pode pará-lo, e só vê o que a CLI informa.                                                                                                                           |
| Ferramentas executadas no harness | Igual. O Atlas nunca executa uma ferramenta por conta própria e não tem gateway nem proxy de MCP.                                                                                                           |

## 2. Camadas da aplicação

```mermaid
flowchart TB
    subgraph FE["Frontend: React + TypeScript — só apresentação"]
        UI["features/*, Inspetor, painéis"] --> Cmd["lib/tauri/commands.ts<br/>CommandMap tipado"]
    end
    Cmd -->|"comandos Tauri, com lista de permissão"| Commands

    subgraph Core["Núcleo: Rust (src-tauri/src)"]
        Commands["commands/<br/>adaptadores finos"] --> App
        App["application/<br/>casos de uso + portas"] --> Dom["domain/<br/>modelos e regras simples"]
        Infra["infrastructure/ + platform/<br/>adaptadores: processos, arquivo de config, Keychain"] -.->|"implementam portas"| App
        Lib["lib.rs<br/>raiz de composição"] -.-> Infra
    end

    Infra --> OS[("SO: processos, arquivos,<br/>armazenamento de credenciais")]

    classDef note stroke-dasharray: 4 3;
    class Lib note;
```

A webview nunca toca em arquivos, processos, Git, credenciais nem na rede. As dependências apontam para dentro (ADR 0002).

## 3. Um passo, da requisição à evidência

O que o `ExecutionService::run_step` faz, em ordem.

```mermaid
flowchart TD
    A["Tarefa + agente + runtime + modelo"] --> B["Resolver rules<br/>por escopo e força"]
    B --> C["Montar seções de contexto<br/>e aplicar o orçamento"]
    C --> D["Planejar MCP<br/>conexões, concessões, política, suporte do runtime"]
    D --> E["ContextPlan<br/>+ afirmações da Revisão de Contexto"]
    E --> F{"Guardrails"}
    F -->|"DENY"| X["O passo falha, com o motivo"]
    F -->|"ASK"| P["Pausado para uma pessoa<br/>aprovação vinculada a uma impressão digital"]
    P -->|"respondido"| G
    F -->|"ALLOW / TRANSFORM"| G["Ler segredos, só agora<br/>e só para servidores expostos"]
    G --> H["runtime.delivery()<br/>o único payload"]
    H --> I["A CLI do runtime executa<br/>fluxo observado"]
    I --> J{"O runtime lista uma ferramenta<br/>que ninguém autorizou?"}
    J -->|"sim"| K["Parar: matar o grupo de processos<br/>falhar, descartar a resposta"]
    J -->|"não"| L["Resposta, uso, ferramentas informadas e usadas"]
    L --> M["ContextManifest<br/>+ hash + métricas"]
    K --> M
    M --> N["Revisão no Inspetor<br/>o Apply continua humano"]

    classDef gate fill:#7c2d12,stroke:#fb923c,color:#fff7ed;
    class F,J gate;
```

## 4. Quem decide o quê: autoridade e confiança

Dois eixos que nunca se fundem: **de onde o conteúdo veio** (`SourceTrust`) e **o que ele pode ser para o modelo**
(`ContextAuthority`). Só as rules do próprio usuário podem obrigar.

```mermaid
flowchart LR
    subgraph Origins["Origem de uma rule"]
        U["Usuário"]
        PF["Arquivo do projeto"]
        GN["Gerada"]
        EX["Externa"]
    end
    U --> Bind["Pode ser Mandatory"]
    PF --> Bind
    GN --> NoBind["Nunca obrigatória"]
    EX --> NoBind

    subgraph Authority["ContextAuthority de um item"]
        Au["Authoritative"] --> In["Instructional"] --> If["Informational"] --> Un["Untrusted"]
    end
    Bind --> Au
    Tool["Resultado de ferramenta MCP<br/>saída de outro agente, arquivos"] --> Un
    Un -.->|"NUNCA pode mudar"| Never["rules, permissões, aprovações,<br/>política, decisões de guardrail"]

    classDef bad fill:#7f1d1d,stroke:#f87171,color:#fef2f2;
    class Never,NoBind bad;
```

## 5. MCP: de uma conexão a uma ferramenta em um passo

```mermaid
flowchart TD
    Conn["McpConnection<br/>desligada por padrão, configuração sem segredos"] --> En{"Habilitada pelo<br/>usuário?"}
    En -->|"não"| Out1["não exposta"]
    En -->|"sim"| Gr{"Uma concessão cobre este<br/>agente, workflow, passo?"}
    Gr -->|"não"| Out1
    Gr -->|"sim"| Pol{"A política do agente permite MCP?<br/>somente leitura: nunca"}
    Pol -->|"não"| Out1
    Pol -->|"sim"| Sup{"O runtime tem um adaptador?<br/>só o Claude hoje"}
    Sup -->|"não"| Out1
    Sup -->|"sim"| Val{"Configuração válida?<br/>sem shell, sem truques de variável"}
    Val -->|"não"| Out1
    Val -->|"sim"| Tl["Ferramentas: servidor inteiro ou só as nomeadas<br/>descoberta renovada se antiga"]
    Tl --> Sec["Segredos do armazenamento do SO<br/>depois do ALLOW dos guardrails"]
    Sec --> Ad["ClaudeMcpAdapter"]
    Ad --> Cfg["--mcp-config arquivo temporário 0600<br/>só referências"]
    Ad --> Env["segredos no ambiente da CLI"]
    Ad --> Dis["--disallowedTools para concessões só de nomeadas"]
    Cfg --> CLI["A CLI do Claude inicia os servidores"]
    Env --> CLI
    Dis --> CLI
    CLI --> Rep["o relatório init lista as ferramentas"]
    Rep -->|"uma desconhecida"| Stop["Parar a execução"]
    Rep -->|"todas autorizadas"| Go["O modelo executa; o manifest registra<br/>as ferramentas informadas e usadas"]

    Http["HTTP / OAuth (Figma)<br/>integração DevTools, UI de conexões"]:::later
    classDef later stroke-dasharray: 5 4,fill:none,color:#94a3b8;
```

A caixa tracejada é a Fase E: não construída.

## 6. MCP entre runtimes: Strategy e Factory

Cada CLI recebe servidores MCP no seu próprio dialeto. O Atlas fala com todas por uma só interface (**Strategy**:
`McpAdapter`), e um só lugar escolhe o dialeto (**Factory**: `McpAdapterFactory`). Um runtime guarda um adaptador e não
sabe de qual CLI é o dialeto que fala; o catálogo de integrações conhecidas é construído da mesma forma (`McpPreset`,
`McpPresetFactory`).

```mermaid
classDiagram
    class ModelRuntime {
        <<trait>>
        +execute(request)
        +probe_mcp(server)
        +mcp_tools_in(names, launched)
    }
    class McpAdapter {
        <<trait, a strategy>>
        +program()
        +features() McpFeatures
        +prepare(launch) McpPreparation
        +probe_plan(server) ProbePlan
        +classify(names, launched)
        +probe(runner, server)
    }
    class McpAdapterFactory {
        +create(dialect) McpAdapter
        +declared(adapter) support, features
    }
    class ClaudeMcpAdapter {
        config em arquivo, lista de negação, strict
    }
    class GeminiMcpAdapter {
        arquivo de settings, lista de permissão
    }
    class OpenCodeMcpAdapter {
        JSON no env, lista de permissão
    }
    class CodexMcpAdapter {
        overrides -c, lista de permissão
    }
    ModelRuntime --> McpAdapter : guarda
    McpAdapterFactory ..> McpAdapter : cria
    McpAdapter <|.. ClaudeMcpAdapter
    McpAdapter <|.. GeminiMcpAdapter
    McpAdapter <|.. OpenCodeMcpAdapter
    McpAdapter <|.. CodexMcpAdapter
```

`McpFeatures` é o que o plano lê (não um `match` no runtime): `tool_filter` (lista de negação, lista de permissão, não
suportado), `probe` (nenhuma, status, ferramentas) e `strict`. O Antigravity não pode receber servidores, então declara
`cli_configured`: o plano casa uma conexão com os servidores que `agy mcp list` mostra e diz ao agente para usar aquele.

## Mantê-los verdadeiros

Um diagrama que se desatualiza é pior que nenhum. Quando um passo das seções 3 ou 5 mudar no código, mude-o aqui no
mesmo commit. Os blocos Mermaid são texto simples, então um revisor vê o diff.
