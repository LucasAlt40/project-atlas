# Arquitetura

O Project Atlas é um app desktop Tauri 2 com duas camadas:

```
Frontend (React + TypeScript)  — apresentação e interação com o usuário
        │  comandos Tauri (tipados, com lista de permissão)
Núcleo (Rust)                  — tudo o que é privilegiado ou ligado à máquina local
```

## Fronteira frontend / backend

- A webview nunca toca diretamente no sistema de arquivos, em processos, no shell, no Git, em credenciais nem na rede.
  Isso pertence ao Rust e só é alcançado por comandos Tauri.
- Todas as chamadas passam por `src/lib/tauri/commands.ts`. `CommandMap` é o contrato tipado; o ESLint proíbe importar
  `@tauri-apps/api/core` em qualquer outro lugar.
- As funcionalidades envolvem isso no seu próprio módulo `services/` (p. ex. `features/home/services/appInfoService.ts`)
  e expõem hooks aos componentes. Os componentes nunca conhecem o `invoke`.
- Os tipos de transporte são escritos à mão dos dois lados hoje (`AppInfoDto` ↔ `domain::app_info::AppInfo`). Veja
  "Pontos de extensão futuros" para saber quando gerá-los.

## Módulos Rust (`src-tauri/src`)

| Módulo            | Responsabilidade                                                                                                            |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `commands/`       | Handlers do Tauri. Adaptadores finos: pegam serviços do estado, os chamam, devolvem DTOs. Sem lógica de negócio.            |
| `application/`    | Casos de uso (`AppInfoService`) e **portas** — traits que descrevem o que o núcleo precisa do mundo.                        |
| `domain/`         | Modelos e regras simples. Sem Tauri, sem I/O.                                                                               |
| `platform/`       | Adaptadores específicos de SO que implementam portas (`OsPlatform`). Escolhidos com `cfg`/verificações de execução só aqui. |
| `infrastructure/` | Adaptadores para o mundo externo: processos filhos (`SystemProcessRunner`), arquivo de configuração JSON.                   |
| `state/`          | `AppState`: serviços compartilhados com os comandos pelo estado gerenciado do Tauri.                                        |
| `lib.rs`          | Raiz de composição: o único lugar que liga adaptadores concretos aos serviços.                                              |

Direção das dependências: `commands → application → domain`. Os adaptadores (`platform/` e, depois,
`infrastructure/`) dependem de `application` para implementar suas portas, nunca o contrário (veja a ADR 0002).

## Agentes, runtimes e execuções (V0.3)

Um agente é _personalidade + runtime + modelo + instruções_, executado sobre um _contexto de projeto_. Os conceitos são
mantidos distintos de propósito (veja a [ADR 0004](adr/0004-runtimes.md)):

| Conceito            | Significado                                          | Onde                                              |
| ------------------- | ---------------------------------------------------- | ------------------------------------------------- |
| Personalidade       | Como o agente se comporta                            | `domain/personality`, `application/personalities` |
| Provedor            | Quem fornece a IA (Anthropic, OpenCode…)             | `domain/runtime::ProviderRef`                     |
| Runtime             | Como o Atlas a alcança nesta máquina                 | `application/runtimes` (porta `ModelRuntime`)     |
| Modelo              | Qual modelo raciocina                                | um id em texto, listado pelo runtime quando pode  |
| Agente              | Personalidade + runtime + modelo + instruções salvos | `domain/agent`, `application/agents`              |
| Contexto do projeto | O que o agente precisa saber                         | `application/projects::context`                   |
| Prompt              | As três preocupações montadas                        | `application/prompt` (`PromptBuilder`)            |
| Execução            | Uma tentativa de uma tarefa                          | `domain/execution`, `application/executions`      |

```
WorkspacePage → WorkspaceProvider / useCatalog → serviços da funcionalidade
  → lib/tauri → commands/*.rs → ChatService → ExecutionService → PromptBuilder
  → RuntimeRegistry → ModelRuntime (OpenCodeRuntime | ClaudeRuntime | …) → ProcessRunner → a CLI
```

- **Comandos** (veja as seções da V0.5 para workspaces, configurações, chat e uso): personalidades, `list_runtimes`,
  agentes. A webview só nomeia um `runtimeId`, um `workspaceId` e um `agentId`.
- **`ModelRuntime`**: `info()` (id, nome, provedor, transporte, `RuntimeCapabilities`, dica de modelo), `detect()`,
  `list_models()`, `execute()`. `RuntimeRegistry::inspect_all` combina detecção e descoberta de modelos em um
  `RuntimeStatus` estruturado (`availability`: não instalado / indisponível / autenticação necessária / pronto;
  `authentication`; `modelDiscovery`: descoberto / não suportado / indisponível / falhou). Adicionar um runtime = um
  arquivo em `application/runtimes/` mais uma linha em `RuntimeRegistry::with_default_runtimes`. Nada em agentes,
  prompts, execuções, comandos ou na UI muda.
- **Runtimes hoje**: `OpenCodeRuntime`, `ClaudeRuntime`, `CodexRuntime`, `GeminiRuntime` e `AntigravityRuntime`
  executam tarefas. O `exec` nunca pede aprovação, então o Codex fica limitado pelo seu próprio sandbox (`read-only`,
  ou `workspace-write` quando a execução pode editar); a flag que desativa o sandbox nunca é usada.
- **Antigravity CLI** (programa `agy`, não `antigravity`): modelos de `agy models` (`id<TAB>nome`, o esforço de
  raciocínio faz parte do id); executa `agy --output-format stream-json --model <id> --prompt=<prompt>` em um terminal
  (`-p` recebe um valor e ignora a stdin, então o prompt é o valor de uma opção, nunca a stdin). Uma execução headless
  nega toda ferramenta que precisaria de um prompt (verificado: escrita de arquivos e shell), então o padrão é somente
  leitura; `allow_edits` acrescenta `--mode accept-edits`. `--dangerously-skip-permissions` nunca é usada. Não há flag
  para restringir ferramentas (então `text_only` fica desligado e `toolAccess` é informado como aberto: o
  `settings.json` do usuário pode permitir comandos) nem comando de status de login (informado como `unknown`; uma
  execução que falha por isso é classificada).
- **Gemini CLI**: `gemini --output-format stream-json --model <modelo> --approval-mode default|auto_edit
--skip-trust --prompt=<prompt>`; execuções headless não podem usar ferramentas que precisam de aprovação, então
  `default` é somente leitura e `allow_edits` usa `auto_edit` (`--yolo` nunca é usada). Sem lista de modelos nem status
  de login. O Google encerrou a CLI para contas individuais (`IneligibleTierError`), o que é informado como
  indisponível e aponta para o Antigravity.
- **OpenCode**: modelos de `opencode models`; `opencode run --agent plan -m <modelo> --format json`, prompt na stdin.
  `--agent plan` (o agente somente leitura embutido do OpenCode) é o que torna a execução somente leitura: verificou-se
  que um `opencode run` simples cria arquivos e executa comandos de shell sem perguntar.
- **Claude CLI**: versão de `claude --version`; login de `claude auth status` (JSON `loggedIn`; sai com 1 quando não
  está logado, então a stdout é lida independentemente do código de saída); executa
  `claude -p --output-format json --model <modelo> --tools Read,Grep,Glob --no-session-persistence` com o prompt na
  stdin e lê o campo `result`. A CLI não tem comando de lista de modelos, então a descoberta é informada como não
  suportada e a UI pede um id de modelo, com a dica do próprio runtime (aliases como `sonnet`). A autenticação é o login
  Claude já existente do usuário; nenhuma chave é pedida.
- **Somente leitura**: o Atlas ainda não tem sistema de permissões. O Claude roda só com as ferramentas Read/Grep/Glob;
  o OpenCode roda como seu agente `plan` somente leitura e nunca recebe `--auto`. Os dois também recebem uma regra de
  somente leitura no prompt. O Atlas confia em cada ferramenta honrar isso; não há sandbox.
- **Modelo de autenticação**: `AuthKind` (`cli_session`, `api_key`, `environment_variable`, `credential_store`) e
  `Authentication { kind, state }` existem para que runtimes com chave de API possam ser adicionados. Só `cli_session` é
  usado. Segredos nunca devem ir no `config.json`; usarão o armazenamento de credenciais do SO (Keychain / Credential
  Manager / Secret Service) atrás de uma porta.
- **Diretório de trabalho**: o runner define `PWD` junto com o diretório atual do filho. Sem isso o OpenCode confiava no
  `PWD` herdado e recusava leituras do projeto como diretório externo.
- **Segurança e portabilidade de processos**: todos os processos filhos passam pela porta `ProcessRunner`
  (`infrastructure/process.rs`). Os runtimes nomeiam um programa simples e argumentos fixos; os ids de modelo são
  validados para não poderem agir como opções. Nenhum shell é iniciado. Os executáveis são localizados no `PATH` mais os
  diretórios usuais de ferramentas de desenvolvimento (Windows: `.exe`, `.cmd`, `.bat` e `%APPDATA%\npm`). Os prompts
  vão na stdin, nunca em argumentos, porque os shims `.cmd` do Windows rejeitam argumentos de várias linhas. Timeouts
  matam processos descontrolados.
- **Erros**: `RuntimeError` é normalizado (não instalado, indisponível, autenticação necessária, modelo indisponível,
  timeout, execução falhou, requisição inválida, resposta inesperada). Uma falha de runtime vira uma execução _falha_
  com uma mensagem para o usuário e detalhes técnicos opcionais.
- **Resultados**: uma `Execution` registra `runtimeId`, `modelId`, o prompt exato, o resultado ou a falha, e `metadata`
  informada pelo runtime (duração, custo…); a UI lê apenas esse formato normalizado.
- **Progresso**: veja "Workspace" abaixo (progresso e texto da resposta chegam em streaming).
- **Persistência**: personalidades personalizadas e agentes vivem em `config.json` no diretório de dados do app
  (gravações atômicas; um arquivo ilegível é posto de lado, não sobrescrito). Agentes salvos antes da V0.3 com um
  `providerId` ainda carregam. Conversas e execuções encerradas também são persistidas (V0.6,
  [ADR 0008](adr/0008-conversation-and-execution-history.md)); a saída do terminal não.
- **Navegação**: Workspace, Agentes, Personalidades, Configurações (`src/app/navigation.ts`); o `AppShell` mostra o
  seletor de workspace e a troca de idioma no cabeçalho. Ainda sem roteador.

## Shell ao vivo e controle de processos (V0.6.x)

Uma execução que roda um runtime de CLI roda em um PTY real e tem uma _sessão de processo_; o card do agente tem as
visões **Chat / Atividade / Terminal**, e o usuário pode interromper (Ctrl+C) ou terminar o processo por trás de uma
execução. O núcleo é dono do PTY, do PID e dos sinais (`infrastructure/pty.rs`, `application/sessions.rs`); a webview só
nomeia uma execução (com seu workspace e agente) e escuta `execution:output` / `execution:status`. Ele controla um
processo que o guard já autorizou e não inicia nada. Veja a [ADR 0007](adr/0007-live-shell-and-process-control.md).
Execuções encerradas e conversas são mantidas (`application/history.rs`) e mostradas na aba _Execuções_ do card do
agente e no Inspetor de Execução; o cabeçalho lista os agentes em execução de todos os workspaces. Veja a
[ADR 0008](adr/0008-conversation-and-execution-history.md).

## Isolamento por Git worktree (V0.6.x)

Os agentes têm `worktreeIsolation` (ligado por padrão). Uma execução isolada roda no seu próprio Git worktree e branch
sob a pasta de dados do app, nunca no checkout do projeto; o `WorktreeService` (caso de uso) conduz o `WorktreeManager`
(porta, adaptador Git em `infrastructure/git_worktree.rs`) e entrega ao runtime só um diretório de trabalho. Quando a
execução termina, o trabalho é commitado no seu branch, medido pelo Git e, se a política `git.write` do agente permitir e
nada atrapalhar, mesclado; do contrário é mantido com uma recomendação, e o merge precisa da ação explícita do usuário
(`merge_execution`). Veja a [ADR 0009](adr/0009-git-worktree-isolation.md).

## Project Harness (V0.7)

O Atlas pode transformar um projeto em um que ele conhece: `Initialize Project` analisa o repositório de forma
determinística (nenhum código é executado), deixa o usuário revisar os achados e escreve um Harness `.atlas/`
versionável no projeto. As execuções então o recebem como contexto de prompt pelo `HarnessContextBuilder`; runtimes e
worktrees nunca sabem que ele existe. É contexto, nunca permissão. Veja a [ADR 0010](adr/0010-project-harness.md).

A V0.7.1 torna esse contexto confiável: toda afirmação carrega evidência e proveniência (fato, inferência, usuário), uma
análise semântica opcional assistida por modelo propõe achados que devem citar o que lhe foi mostrado, contradições são
registradas em vez de resolvidas em silêncio, o refresh mostra um diff e nunca sobrescreve o conhecimento do próprio
usuário, e os agentes recebem um resumo compacto que separa o que se sabe, o que se infere, o que o usuário disse e o
que é desconhecido. Veja a [ADR 0011](adr/0011-harness-knowledge-model.md).

A V0.8 torna o contexto **ciente da tarefa**: em vez do Harness inteiro, um agente recebe a parte de que a sua tarefa
precisa, escolhida por regras determinísticas (um pequeno léxico de tags, áreas, categorias, caminhos e palavras-chave;
sem modelo, sem embeddings) e explicável item por item. Restrições e decisões nunca ficam de fora, inferência continua
inferência, conhecimento obsoleto é informado como desatualizado, e o que ficou de fora é nomeado. Se uma seleção não
puder ser feita, usa-se o contexto inteiro do Harness e a execução registra o motivo. Uma prévia mostra o contexto
antes da execução. Veja a [ADR 0012](adr/0012-task-aware-context.md).

## Workflows: engine, orquestrador e grafo visual (V0.9)

Uma tarefa agora pode rodar como um **workflow** — um grafo de passos de agente, condições e finais — em vez de um só
agente. Quatro preocupações separadas (veja a [ADR 0013](adr/0013-workflow-engine-and-orchestrator.md)):

```text
domain/workflow, domain/orchestration     o modelo: Workflow, Node, Edge, Condition, WorkflowExecution, estado compartilhado,
                                          Artifact, Decision, AgentResult — sem comportamento que precise de I/O
application/workflow/graph.rs             vínculos, ciclos (SCC), alcançabilidade
application/workflow/validation.rs        toda regra que torna um grafo executável (loops limitados, agentes existem, …)
application/workflow/engine.rs            o Workflow Engine: uma máquina de estados pura (ready / dispatch / complete / route /
                                          retry / block / skip / pause / cancel / interrupt)
application/workflow/orchestrator.rs      conduz uma execução: porta StepRunner, threads, vigia de aprovações, persistência, eventos
application/workflow/chat_runner.rs       o StepRunner real: um passo é uma execução pelo ChatService
application/workflow/service.rs           definições (versionadas), execuções (snapshots), recuperação de execuções interrompidas
application/workflow/templates.rs         templates determinísticos e o seletor automático
application/orchestration/                parser de resultado, handoff para o estado compartilhado, o brief por passo
commands/workflow.rs                      adaptador Tauri fino (15 comandos, eventos `workflow:*`)
features/workflow (frontend)              página, canvas (React Flow só como apresentação), inspetor, visão geral
```

Um passo passa pelo `ChatService::send_workflow_step`, então é uma execução comum: a política de worktree do agente, o
Harness (Task Context selecionado para o trabalho do próprio passo), o registro de runtimes e o guard de permissões se
aplicam, e a execução aparece no histórico e no Inspetor de Execução com um caminho _Workflow › nome › passo ›
Execução #n_. Nada em um workflow pode executar um processo nem aprovar algo.

Os passos entregam resultados uns aos outros como **handoffs** explícitos, compartilham **um worktree por execução** (o
que o Developer escreve é o que o Validator lê), e o código da execução é medido pelo Git e mantido à parte do status da
própria execução: uma execução pode concluir enquanto seu código ainda está só no worktree isolado, e apenas uma decisão
explícita do usuário (aplicar, manter, descartar, abrir em um editor) muda isso. Veja a
[ADR 0014](adr/0014-handoff-shared-worktree-and-code-integration.md).

Um agente pode declarar um **contrato de resultado** (desfechos como `pass`/`fail` ou `approved`/`changes_requested`); os
passos roteiam pelo `result.outcome` estruturado, nunca por texto livre, e um passo que deve concluir e não o faz falha em
vez de ser lido como aprovação. Passos que escrevem tomam o worktree da execução com exclusividade; leitores o
compartilham. Veja a [ADR 0015](adr/0015-result-contracts-outcomes-and-worktree-lock.md); as rotas são conferidas
contra os contratos antes de uma execução e reparadas somente com a confirmação do usuário
([ADR 0017](adr/0017-contract-aware-workflow-validation-and-route-repair.md)).

## Humano no circuito (V0.9.3)

Um agente que para para perguntar a uma pessoa está **esperando**, não terminou ([ADR 0016](adr/0016-human-in-the-loop.md)).
`ExecutionStatus`, `NodeStatus` e a execução têm cada um um estado `WaitingForInput`; a pergunta é uma
`PendingInteraction` persistida com a execução. O detector (sinal do adaptador → bloco `atlas-interaction` → análise de
texto conservadora) vive em `application/interaction`; o orquestrador pausa a execução, recebe a resposta por
`answer_workflow_interaction` e inicia o passo de novo a partir dela no mesmo worktree. A UI mostra "Ação necessária" no
cabeçalho e na execução; responder nunca aplica código ao projeto.

## Acesso pelo celular

O navegador de um celular na mesma rede pode acompanhar execuções e responder o que espera por uma pessoa
([ADR 0034](adr/0034-mobile-remote-access.md)). Desligado até a pessoa ligar no app. `application/remote` guarda as
regras (pareamento de uso único, sessões em memória, uma política em níveis do que o celular pode fazer, um modelo de
leitura redigido, um hub de eventos numerado); `infrastructure/remote_server.rs` é um pequeno adaptador hyper; a página é
uma segunda entrada do Vite (`mobile.html`) servida pelos arquivos embutidos do próprio app. O gateway **chama** o
`WorkflowService` e o `Orchestrator`: não há um segundo engine, e uma resposta remota segue exatamente o caminho de uma
do desktop.

## Camada de Otimização (V0.10)

Uma camada transversal que mede o que cada execução envia e custa, e remove o que é dito duas vezes, sem tocar em texto
obrigatório ou de segurança. Ela envolve as peças existentes em vez de substituí-las: o `PromptBuilder` continua sendo a
única montagem de prompt, o `TaskContextService` do Harness continua selecionando o contexto do projeto, o
`UsageMetrics` continua guardando o que os runtimes informam, e o `security/` continua decidindo o que pode executar.

```text
ExecutionService.run_step
   ├─ Harness / Task Context ─────────── (seleção existente)
   ├─ Skills (opt-in) ────────────────── descobrir → selecionar → ativar → referências
   ├─ PromptBuilder.assemble ─────────── a única montagem
   ├─ Context Engine (opt-in) ────────── deduplica linhas, comprime espaços, orçamento (informa, nunca corta)
   ├─ PromptBuilder.assemble (de novo, só se o engine mudou uma entrada)
   ├─ Runtime (isolado dos servidores MCP e skills do usuário para o Claude)
   └─ OptimizationMetrics: seções, tokens estimados vs informados, latência, ferramentas expostas/usadas, extensões carregadas
```

Figuras de como as peças se encaixam (Atlas acima do harness de agentes, camadas, um passo, autoridade, MCP):
[diagrams.md](diagrams.md).

Status de cada fase, o que foi medido, o que foi removido (RTK) e o que está pendente:
[optimization-layer.md](optimization-layer.md).

## Workspaces, contexto do projeto e chat de agentes (V0.5)

O Atlas é organizado em torno de **workspaces**: um workspace é um ambiente de projeto onde agentes trabalham (veja a
[ADR 0005](adr/0005-workspaces-usage-i18n.md)). Agentes nunca conversam entre si.

```
Global (sobrevive a todo workspace)       Por workspace
  configurações: idioma, último workspace   Workspace { id, name, projectPath, description, layout }
  personalidades, agentes (config)            └ layout: rows, columns, agentPlacements[{ agentId, position }]
  runtimes instalados e seu status          conversas (agente × workspace), execuções, registros de uso
```

- **Workspace vs agente.** O `Workspace` (`domain/workspace`, `application/workspace`) é dono da sua pasta de projeto, do
  seu layout (o tamanho da grade é dado; o padrão é 2 × 2) e de onde os agentes ficam. Um agente é configuração global e
  pode ser colocado em vários workspaces; colocar, mover ou remover nunca cria nem apaga o agente. Cadeia: Agente →
  posição no workspace → conversa → execução. O limite de 4 agentes por workspace é uma constante de UI
  (`features/workspace/model/grid.ts`), não uma regra de domínio.
- **Persistência.** Workspaces, configurações (`language`, `selectedWorkspaceId`) e o registro de uso ficam no mesmo
  `config.json` das personalidades e agentes (porta `ConfigStore`, gravações atômicas, sem banco de dados). Nenhum
  segredo é guardado ali (os segredos de conexões MCP ficam no cofre de credenciais do SO). As conversas são persistidas
  desde a V0.6 (ADR 0008).
- **Pasta do projeto.** Escolhida com o seletor nativo (`tauri-plugin-dialog`, concedido apenas como
  `dialog:allow-open`), revalidada pelo núcleo (`ProjectInspector::is_directory`). Selecionar uma pasta apenas registra
  o seu caminho: nada é lido, indexado ou enviado. `ProjectInspector::technologies` olha os **nomes de arquivo do nível
  superior** (mais os nomes das dependências no `package.json`) e reconhece Git, Node.js, TypeScript, Angular, React,
  Vue, Svelte, Next.js, Rust, Tauri, .NET, Maven, Gradle, Go, Python, Docker, PHP, Ruby, Dart. Nenhum modelo é
  envolvido. O `PromptBuilder` recebe o nome, o caminho e as tecnologias detectadas do projeto como metadados de
  `ProjectContext`. Permissões e sandbox vieram depois (ADR 0003 e ADR 0038).
- **Conversas e execução são indexadas por workspace e agente** (`Message.workspaceId`, `ExecutionEvent.workspaceId`,
  trava de ocupado por workspace × agente). O mesmo agente em dois workspaces tem conversas separadas e pode rodar nos
  dois ao mesmo tempo. `ChatService::send` registra a mensagem do usuário e retorna de imediato; a execução acontece em
  uma thread bloqueante (`ExecutionService::run_with_id`, que resolve o projeto a partir do workspace) e termina em uma
  mensagem do assistente.
- **Trocar de workspace** só muda o que é mostrado. O `WorkspaceProvider` (acima das telas) mantém um reducer indexado
  por `workspaceId/agentId`, então uma execução continua sendo acompanhada enquanto outro workspace ou tela está
  visível e seu estado está correto na volta. Excluir um workspace (não enquanto um agente trabalha nele) também
  descarta suas conversas e uso; excluir um agente o remove de todo workspace e suas conversas, mas mantém o uso que ele
  já gastou.
- **Eventos** (um mecanismo, dois canais): `execution:progress` carrega `ExecutionEvent { executionId, workspaceId,
taskId, agentId, kind, message, timestamp, metadata }`; `conversation:message` anuncia mensagens do assistente. Tipos:
  `started`, `starting_runtime`, `sending_prompt`, `waiting_for_model`, `output_chunk`, `tool_started`, `tool_completed`,
  `completed`, `failed`. Um runtime só emite o que sua ferramenta realmente transmite (Claude: tokens e ferramentas;
  OpenCode: partes de texto e ferramentas concluídas). A UI redige cada passo a partir de `kind` + `metadata`
  (`runtime`, `model`, `tool`, `failureKind`); a `message` em inglês do núcleo é para logs, exceto `output_chunk` (o
  texto da resposta em streaming).
- **Resposta em tempo real e narração.** Veja as notas da V0.3/V0.4: o `PromptBuilder` acrescenta uma regra de narração a
  toda personalidade; a resposta armazenada é a mensagem de conclusão do modelo.

## Uso, custo e cota (V0.5)

O uso é **guiado por capacidades e nunca inventado**.

- `RuntimeCapabilities` ganhou `usageMetrics`, `costMetrics`, `quotaMetrics`. OpenCode: uso e custo; Claude CLI: uso,
  custo e cota; runtimes apenas detectados: nenhum.
- `UsageMetrics { inputTokens?, outputTokens?, totalTokens?, cost?, currency?, source }`: todo campo é opcional. `null`
  significa que o runtime não informou e é diferente de `0`. `source` é `runtime_reported` (uma execução),
  `atlas_calculated` (uma soma que o Atlas fez sobre execuções que observou) ou `provider_reported` (janelas de cota).
- **O que cada runtime realmente informa** (verificado contra as CLIs instaladas):
  - _Claude CLI_ (resultado `stream-json`): `usage.input_tokens`, `cache_creation_input_tokens`,
    `cache_read_input_tokens`, `output_tokens` e `total_cost_usd` (o número da própria CLI, em USD, não necessariamente
    a cobrança); mais `rate_limit_event.unifiedWindows` (`five_hour`, `seven_day`: `utilization` de 0 a 1 e
    `resetsAt`). O Atlas informa entrada = os três tipos de entrada, saída, total = entrada + saída.
  - _OpenCode_ (`--format json`): `tokens` por `step_finish` (entrada, saída, raciocínio, leitura/escrita de cache,
    total) e `cost`, somados sobre os passos. Nenhuma moeda é declarada, então nenhuma é afirmada; sem cota.
- **Registro.** Depois de cada execução o `ChatService` grava um `UsageRecord { executionId, workspaceId, agentId,
runtimeId, modelId, startedAt, completedAt, succeeded, metrics? }` em `config.json` (retenção de 100 dias) e a última
  cota informada pelo runtime (`quotas[runtimeId]`). O `UsageReporter` soma registros: `UsageTotals` carrega `runs`,
  `runsWithTokens`, `runsWithCost`, de modo que uma soma parcial diga quantas execuções cobre; valores em moedas
  diferentes nunca são somados (um zero em uma moeda não declarada é neutro). A UI passa os limites do período
  (`today`, `week` a partir de segunda, `month`) no seu próprio fuso horário. "Esta conversa" são as execuções que têm
  mensagens na conversa.
- **Não é ao vivo.** Nenhuma das CLIs informa uso durante a execução, então o Atlas mostra o status e a atividade da
  execução enquanto ela roda e os números quando termina. Os números semanais são _rastreados pelo Atlas_, não
  informados pelo provedor; só as janelas de cota são informadas pelo provedor, e só onde o runtime as expõe.
- **UI.** Clicar no cabeçalho de um agente abre os _Detalhes do agente_ (identidade, status, tempo de execução,
  atividade, métricas da última execução, somas da conversa/hoje/semana, janelas de cota, runtime/modelo/status). O
  cabeçalho do workspace mostra um resumo compacto "este workspace" (hoje / semana) que abre uma visão mais ampla.

## Localização (V0.5)

- `src/i18n`: `en-US.ts` define as chaves, `pt-BR.ts` deve definir exatamente as mesmas (conferido pelo sistema de
  tipos, e um teste as compara), `createTranslator(language)` preenche `{placeholders}`, `I18nProvider` / `useT()` o
  servem. O português (Brasil) é o padrão; `LANGUAGES` é a lista única para estender. Fora do provider (testes de
  componente) usa-se o inglês.
- O idioma é uma **configuração global** persistida pelo núcleo (`set_language`); a interface troca sem reiniciar. O
  `LanguageSwitch` no cabeçalho e a tela de Configurações o alteram.
- **O núcleo não traduz.** As falhas de comando são `AppError { code, params, detail }` (`application/errors.rs`); o
  frontend mapeia `error.<code>` para o texto. As falhas de execução usam `failureKind` (`failure.<kind>`), os estados
  de runtime usam códigos `notice` / `modelHint`, os passos de atividade são redigidos a partir dos tipos de evento. Não
  traduzidos: nomes de modelo, provedor, runtime e projeto, conteúdo criado pelo usuário, saída da IA e as instruções de
  sistema das personalidades (elas são o prompt). As descrições e listas de comportamento das personalidades embutidas
  não alteradas são traduzidas pela UI.

## Editar e remover (V0.5)

- **Personalidades**: `update_personality`, `delete_personality`, `restore_default_personalities`. Uma personalidade
  personalizada é editada ou removida em `config.json`. As embutidas vêm com o app, então editar uma guarda a cópia
  alterada do usuário (`builtinOverrides`, mesmo id; seu resumo de comportamento é descartado porque descrevia as
  instruções originais) e remover uma a esconde (`hiddenBuiltins`); "Restaurar personalidades padrão" limpa os dois e
  deixa as personalizadas em paz. Uma personalidade usada por agentes não pode ser removida (o erro os nomeia), então
  nenhum agente fica apontando para o nada.
- **Agentes**: `update_agent` muda nome, personalidade, runtime, modelo e instruções; id, hora de criação, conversa e
  posição no workspace são mantidos, e a próxima execução usa as novas configurações. `delete_agent` passa pelo
  `AgentLifecycle` (`application/lifecycle.rs`), o único lugar que sabe que a configuração, a posição no workspace e a
  conversa de um agente andam juntas: ele trava o agente (um agente ocupado não pode ser excluído), o exclui, libera sua
  posição e descarta suas mensagens. As execuções passadas continuam no histórico.
- **UI**: a tela de Agentes lista, edita e exclui agentes; os cards de personalidade têm Editar / Excluir; os cards do
  workspace têm ✎ (painel de edição, com um "Excluir agente" confirmado). Excluir pede uma confirmação em linha
  (diálogos nativos não são confiáveis na webview). O workspace relê seu layout quando um agente some do catálogo.

## Responsabilidades do Tauri

Ciclo de vida da janela, IPC, o sistema de permissões (capabilities), CSP e empacotamento. A lógica específica do Atlas
não vive na configuração do Tauri nem nos handlers de comando.

## Segurança

Privilégio mínimo por padrão.

- Os comandos personalizados são listados em `src-tauri/build.rs`. O Tauri gera uma permissão `allow-<comando>` para
  cada um, e `capabilities/default.json` concede somente essas. Um comando não concedido é inalcançável pela webview.
- O único plugin do Tauri é o `dialog`, concedido como `dialog:allow-open` (o seletor nativo de pasta, que devolve um
  caminho que o núcleo revalida). Nenhum plugin de fs, shell, opener ou http é instalado ou concedido.
- Uma CSP estrita é definida em `tauri.conf.json` (`default-src 'self'`; somente IPC).
- Um teste em Rust chama o comando pelo contexto real gerado, então remover a concessão faz o CI falhar.
- `unsafe` é proibido no crate.

Formato planejado para a execução de agentes (não implementado): `Agente → Ferramenta → Política de Permissão →
Aprovação do Usuário → Execução`, tudo dentro do Rust. A webview só renderiza os pedidos de aprovação e envia a decisão
do usuário.

## Estratégia multiplataforma

- Nada presume um shell, um separador de caminho ou um SO. O código que precisa diferir vive em `platform/` atrás de uma
  porta declarada em `application/` (p. ex. um futuro `ProcessService` com implementações para Windows / Unix).
- Use `std::path::Path`/`PathBuf` no Rust, nunca caminhos montados como string; nunca fixe `/bin/bash`.
- O frontend usa pilhas de fontes do sistema e nenhuma API específica de SO.

## Pontos de extensão futuros

Projeto, Harness, Agentes, Modelos, Ferramentas, MCP, Workflows, Políticas, Permissões, Memória, Execuções e o
Orquestrador se tornarão cada um uma área delimitada: um `domain/<conceito>`, um módulo de caso de uso em
`application/`, portas para o seu I/O, adaptadores em `infrastructure/` (a ser criado quando o primeiro adaptador —
sistema de arquivos, processo, Git, persistência — existir), e um `commands/<conceito>.rs` mais uma pasta
`features/<conceito>/`. Adicionar um comando significa tocar em `build.rs`, no arquivo de capabilities, em `lib.rs` e em
`CommandMap`. Quando a superfície de transporte crescer, gere os tipos TS a partir do Rust (p. ex. `specta`/`ts-rs`) em
vez de escrevê-los à mão. Um roteador de frontend e um store global estão ausentes de propósito até que uma segunda tela
ou um estado compartilhado precise deles.

## ADRs

- [0001 — Tauri 2, React, Vite, TypeScript, CSS Modules](adr/0001-stack.md)
- [0002 — Portas em `application`, adaptadores para fora](adr/0002-ports-and-adapters.md)
- [0003 — Permissões de comando negadas por padrão](adr/0003-command-permissions.md)
- [0004 — Provedores, runtimes e modelos](adr/0004-runtimes.md)
- [0005 — Workspaces, uso e localização](adr/0005-workspaces-usage-i18n.md)
- [0007 — Shell ao vivo e controle de processos](adr/0007-live-shell-and-process-control.md)
- [0008 — Histórico de conversas e execuções](adr/0008-conversation-and-execution-history.md)
- [0009 — Isolamento por Git worktree](adr/0009-git-worktree-isolation.md)
- [0010 — Project Harness](adr/0010-project-harness.md)
- [0011 — Modelo de conhecimento do Harness, evidências, análise semântica, verificação e obsolescência](adr/0011-harness-knowledge-model.md)
- [0012 — Contexto ciente da tarefa](adr/0012-task-aware-context.md)
- [0013 — Workflow Engine e Orchestrator](adr/0013-workflow-engine-and-orchestrator.md)
- [0014 — Handoff, o worktree compartilhado da execução e a integração do código](adr/0014-handoff-shared-worktree-and-code-integration.md)
- [0015 — Contratos de resultado, desfechos e a trava do worktree compartilhado](adr/0015-result-contracts-outcomes-and-worktree-lock.md)
- [0016 — Humano no circuito: um agente esperando por uma pessoa](adr/0016-human-in-the-loop.md)
- [0017 — Validação de workflow ciente do contrato e reparo assistido de rotas](adr/0017-contract-aware-workflow-validation-and-route-repair.md)
- [0018 — Live Workspace: observar o worktree de uma execução de workflow enquanto ela trabalha](adr/0018-live-workspace.md)
- [0019 — Revisão e Entrega: ler um workflow concluído e decidir sobre o seu código](adr/0019-review-and-delivery.md)
- [0020 — Camada de Otimização, fase 0: medir antes de otimizar](adr/0020-optimization-observability.md)
- [0021 — Camada de Otimização, fase 1: o Context Engine](adr/0021-context-engine.md)
- [0022 — Camada de Otimização, fase 2: Skills com divulgação progressiva](adr/0022-skills.md)
- [0023 — RTK (Rust Token Killer): avaliado, construído e removido](adr/0023-rtk-evaluated-and-removed.md)
- [0024 — Camada de Otimização, fase 4: o que um runtime consegue alcançar (superfície e isolamento)](adr/0024-runtime-surface-and-isolation.md)
- [0025 — Camada de Otimização, fase 5: Guardrails e Revisão de Contexto](adr/0025-guardrails-and-context-review.md)
- [0026 — Context & Tooling Platform (Fase A: arquitetura e spike)](adr/0026-context-and-tooling-platform.md) — desenho: [context-and-tooling-platform.md](context-and-tooling-platform.md)
- [0027 — Rules, Autoridade de Contexto e o canal de system prompt (Fase C)](adr/0027-rules-context-authority-and-system-prompt-channel.md)
- [0028 — Fundação MCP (Fase D)](adr/0028-mcp-foundation.md)
- [0029 — MCP para todo runtime que consegue suportá-lo](adr/0029-mcp-for-every-runtime.md) — o que cada CLI oferece, medido
- [0030 — Templates de workflow: propósito, ferramentas e cópias salvas](adr/0030-workflow-templates-and-tools.md) — Implementer, Debugger, Reviewer; MCP por passo
- [0031 — Planos esperam pela pessoa](adr/0031-plan-approval.md) — aprovar, mudar ou refazer; a aprovação automática é uma configuração da pessoa
- [0032 — Compartilhar configurações como arquivo](adr/0032-configuration-import-export.md) — JSON versionado; prévia, decisões e uma importação de tudo ou nada
- [0033 — Workspaces com vários projetos](adr/0033-multi-project-workspaces.md) — Harness local por projeto mais um contexto do workspace; escopo, worktrees, migração
- [0034 — Acesso pelo celular](adr/0034-mobile-remote-access.md) — um celular na rede local acompanha execuções e responde o que espera; pareamento, níveis de ação, SSE com cursor, limites do HTTP
- [0035 — Specs (SDD)](adr/0035-spec-driven-development.md) — especificação escrita por agentes e aprovada pela pessoa antes do workflow; OpenSpec como primeiro formato, motor interno, validação em duas camadas, publicação segura, CLI opcional
- [0036 — Custo do SDD](adr/0036-custo-do-sdd.md) — consumo por spec medido, limite de tokens, alteração por seções, revisão que sabe o que mudou; o que ainda não foi feito
- [0037 — Memória](adr/0037-memory.md) — lições curtas entre sessões, propostas pelo agente e decididas pela pessoa; escopos usuário/workspace/agente, desligada por padrão; M1–M3 feitas, M4 pendente
- [0038 — Sandbox do macOS para os runtimes](adr/0038-macos-sandbox.md) — o processo inteiro da CLI sob um perfil do sistema; `off`/`auto`/`required`, desligado por padrão; o que não confina
- [0039 — GitHub Copilot CLI como runtime](adr/0039-github-copilot-cli.md) — sexto runtime: execução, permissões, MCP e modelos medidos com o CLI real; limites conhecidos
