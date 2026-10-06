# 1. Visão geral e arquitetura

> Parte do [guia do Atlas](README.md). Resumo de uma página: [RESUMO.md](../RESUMO.md).

## O que é o Atlas

Aplicativo desktop (Tauri 2) que **orquestra agentes de IA em projetos de desenvolvimento**. O Atlas não é um modelo
nem um agente: é o **controlador** que decide _o que roda, onde roda, com que contexto e com que permissão_. Os
modelos (Claude, OpenCode, Gemini, Antigravity) são acessados por **CLIs já instaladas na máquina do usuário**; o Atlas
nunca pede chave de API (autenticação = a sessão que o usuário já tem na CLI).

Princípio que atravessa tudo:

> **A IA sugere e faz o trabalho; o Atlas controla o processo.**
> Nada que um agente escreve altera permissões, rotas de workflow, aprovações ou política. Texto de agente é _dado_,
> nunca _comando_.

## Camadas

```text
Frontend (React + TypeScript + Vite, CSS Modules)      apresentação e interação — nunca toca disco/processo/Git/rede
        │  comandos Tauri tipados e com allow-list (src/lib/tauri/commands.ts)
Core (Rust)                                            tudo que é privilegiado ou da máquina local
```

- O webview **só** fala com o core por comandos Tauri. Todos os comandos estão listados em `src-tauri/build.rs`
  (Tauri gera uma permissão `allow-<comando>` para cada um) e concedidos em `capabilities/default.json`. Comando não
  concedido é inalcançável. Há mais de 70 comandos.
- O único plugin Tauri é `dialog` (`dialog:allow-open`, seletor de pasta). Não há plugins de fs, shell, http ou opener.
- CSP estrita (`default-src 'self'`). `unsafe` é proibido no crate Rust.
- ESLint proíbe importar `@tauri-apps/api/core` fora de `src/lib/tauri/commands.ts`; cada feature tem um `services/`
  que embrulha isso e expõe hooks aos componentes.

## Módulos Rust (`src-tauri/src`)

| Módulo            | Responsabilidade                                                                                          |
| ----------------- | --------------------------------------------------------------------------------------------------------- |
| `commands/`       | Handlers Tauri. Adaptadores finos: pegam serviços do estado, chamam, devolvem DTOs. Sem regra de negócio. |
| `application/`    | Casos de uso e **ports** (traits que descrevem o que o core precisa do mundo).                            |
| `domain/`         | Modelos e regras puras. Sem Tauri, sem I/O.                                                               |
| `infrastructure/` | Adaptadores do mundo externo: processos, PTY, Git, scanner de projeto, store do Harness, `config.json`.   |
| `platform/`       | Adaptadores específicos de SO.                                                                            |
| `state/`          | `AppState`: serviços compartilhados com os comandos.                                                      |
| `lib.rs`          | **Composition root**: único lugar que liga adaptadores concretos aos serviços.                            |

Direção de dependência: `commands → application → domain`. Adaptadores dependem de `application` para implementar
seus ports, nunca o contrário ([ADR 0002](../architecture/adr/0002-ports-and-adapters.md)).

### Mapa de `application/`

| Pasta / arquivo                                                        | O que faz                                                                              |
| ---------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| `agents/`, `personalities/`, `projects/`, `workspace/`, `lifecycle.rs` | Cadastro de agentes, personalidades, workspaces; remoção coerente de agente            |
| `runtimes/`                                                            | Um arquivo por CLI (`claude`, `opencode`, `gemini`, `antigravity`, `detect_only`)      |
| `prompt.rs`                                                            | `PromptBuilder`: **único** lugar onde um prompt é montado                              |
| `executions/`                                                          | `ExecutionService`: uma execução de um agente (o coração do pipeline)                  |
| `chat/`                                                                | `ChatService`: conversa, uso/custo, ponte para workflows (`send_workflow_step`)        |
| `history.rs`, `config.rs`, `settings.rs`, `usage.rs`                   | Persistência de conversas/execuções, config, ajustes e ledger de uso                   |
| `harness/`                                                             | Análise do projeto, geração do `.atlas/`, contexto e Task Context                      |
| `workflow/`                                                            | Grafo, validação, **engine**, **orquestrador**, templates, reparo de rotas, integração |
| `orchestration/`                                                       | Parser de resultado, handoff, brief de cada passo                                      |
| `interaction/`                                                         | Detecção de "agente esperando uma pessoa"                                              |
| `worktree/`, `live_workspace/`                                         | Git worktree por execução/run e observação ao vivo do código                           |
| `security/`                                                            | Política de permissões, guard de processos, aprovações, auditoria                      |
| `sessions.rs`                                                          | Sessões de processo (PTY): controle de Ctrl+C / terminate                              |
| `optimization/`                                                        | Observabilidade de prompt/latência (fase 0)                                            |

## Conceitos do domínio

| Conceito        | Significado                                                                                                  | Onde vive                                    |
| --------------- | ------------------------------------------------------------------------------------------------------------ | -------------------------------------------- |
| **Workspace**   | Um projeto (pasta local) + layout dos agentes (grade, padrão 2×2)                                            | `domain/workspace`                           |
| **Personality** | _Como_ o agente se comporta (instruções de sistema)                                                          | `domain/personality`                         |
| **Runtime**     | _Como_ o Atlas alcança um modelo nesta máquina (uma CLI)                                                     | `application/runtimes` (port `ModelRuntime`) |
| **Model**       | Qual modelo raciocina (string; listado pela CLI quando ela sabe)                                             | string no agente                             |
| **Agent**       | personalidade + runtime + modelo + instruções + perfil de permissão + isolamento + **contrato de resultado** | `domain/agent`                               |
| **Execution**   | Uma tentativa de uma tarefa (id `exec-N`)                                                                    | `domain/execution`                           |
| **Harness**     | O que o Atlas _sabe_ sobre o projeto, em `.atlas/` dentro do projeto                                         | `domain/harness`                             |
| **Workflow**    | Grafo declarativo de passos (agentes, condições, fins)                                                       | `domain/workflow`                            |
| **Run**         | Uma instância de um workflow, com _snapshot_ da definição                                                    | `WorkflowExecution`                          |

Agentes **nunca conversam entre si**. Passos de workflow trocam _artefatos, decisões e resultados estruturados_
(handoff) mediados pelo orquestrador — ver [04](04-workflow.md).

## Workspaces e escopo

- Agentes e personalidades são **globais**; conversas, execuções e uso são **por (workspace, agente)**.
- O mesmo agente em dois workspaces tem conversas separadas e pode rodar nos dois ao mesmo tempo. Um agente roda uma
  coisa por vez por workspace (lock de ocupado).
- Trocar de workspace só muda o que é mostrado; execuções em andamento continuam (`WorkspaceProvider` mantém o estado
  por `workspaceId/agentId`).
- Selecionar uma pasta só grava o caminho: nada é lido ou enviado até o usuário pedir.

## Internacionalização

`src/i18n`: `en-US.ts` define as chaves, `pt-BR.ts` (padrão) precisa ter exatamente as mesmas (checado por tipo e por
teste). **O core não traduz**: erros são `AppError { code, params, detail }` e a UI mapeia `error.<code>`; falhas de
execução usam `failureKind`. Não se traduzem: nomes de modelo/provedor/projeto, conteúdo do usuário, saída da IA e as
instruções de sistema das personalidades (elas _são_ o prompt).

## Frontend (`src/`)

```text
src/app/          shell e navegação (telas: Workspace, Workflow, Agents, Personalities, Settings)
src/features/     agents · harness · home · settings · usage · workflow · workspace
                  cada feature: components/ hooks/ model/ pages/ services/ types/
src/lib/tauri/    commands.ts — o contrato tipado com o core (CommandMap)
src/i18n/         en-US.ts, pt-BR.ts
```

- `features/workflow`: página, canvas (React Flow **só como apresentação**; `model/graph.ts` é a única tradução entre o
  domínio e o que a lib desenha), inspetor, visão geral, handoffs, Live Workspace, Review Workspace, painéis de
  ação-requerida, recuperação e reparo de rotas.
- Tipos de fio são escritos à mão nos dois lados (a geração a partir do Rust — `specta`/`ts-rs` — está prevista).
- Sem router nem store global de propósito, até haver necessidade.

## Estratégia multiplataforma

Nada assume shell, separador de caminho ou SO. Código que difere fica em `platform/` atrás de um port. Caminhos usam
`Path`/`PathBuf`; nunca se inicia `sh -c`/`cmd /c`/`powershell`. Validado em **macOS**; Linux usa os mesmos caminhos
POSIX (não executado); Windows tem ramos `cfg(windows)` mas **não foi compilado nem executado**
([ADR 0007](../architecture/adr/0007-live-shell-and-process-control.md)).
