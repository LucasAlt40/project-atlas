# 6. Persistência, eventos, uso/custo e observabilidade

> Parte do [guia do Atlas](README.md). ADRs: [0005](../architecture/adr/0005-workspaces-usage-i18n.md),
> [0008](../architecture/adr/0008-conversation-and-execution-history.md),
> [0020](../architecture/adr/0020-optimization-observability.md).

## Onde cada coisa é gravada

| Dado                                                      | Onde                                             | Observações                                                       |
| --------------------------------------------------------- | ------------------------------------------------ | ----------------------------------------------------------------- |
| Estado do Atlas (config, agentes, workspaces, histórico…) | `config.json` no **diretório de dados do app**   | Um arquivo só, escrita atômica (temp + rename), sem banco         |
| Conhecimento do projeto (Harness)                         | `<projeto>/.atlas/`                              | Versionável no Git do projeto; ver [03](03-harness-e-contexto.md) |
| Worktrees de execução / de run                            | `<app data>/worktrees/<workspaceId>/exec-NNNNNN` | Fora do projeto; ver [05](05-seguranca-e-isolamento.md)           |
| Saída bruta de terminal                                   | **Memória** (últimos 512 KiB/sessão, 8 sessões)  | Nunca vai para o disco                                            |
| Segredos / chaves                                         | **Em lugar nenhum**                              | Auth = sessão da CLI do usuário                                   |

### `config.json` (`UserConfig`)

Todos os campos são `#[serde(default)]` — arquivos antigos continuam carregando. Arquivo ilegível é **posto de lado**,
não sobrescrito. Uma falha ao salvar **nunca** derruba o chat na tela (é logada).

| Campo                                                 | Conteúdo                                                                                                            |
| ----------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `personalities`, `builtinOverrides`, `hiddenBuiltins` | personalidades próprias, cópias editadas das embutidas, embutidas escondidas                                        |
| `agents`                                              | agentes (inclui `resultContract`, perfil de permissão, `worktreeIsolation`)                                         |
| `workspaces`                                          | projeto, layout (grade e `agentPlacements`), política de segurança                                                  |
| `settings`                                            | idioma, workspace selecionado, flag de métricas de otimização                                                       |
| `usage`, `quotas`                                     | ledger de uso por execução (retenção **100 dias**) e última cota reportada por runtime                              |
| `messages`, `executions`                              | conversas e execuções encerradas (limites abaixo)                                                                   |
| `worktrees`                                           | metadados de worktrees (gravados na **criação**)                                                                    |
| `workflows`                                           | definições de workflow (versionadas, com `route_repairs`)                                                           |
| `workflow_executions`                                 | runs: snapshot, estado compartilhado, tentativas, handoffs, interações, integração, log de eventos (400 mais novos) |

## Como o contexto de conversa/execução é gravado (ADR 0008)

Conversas e execuções encerradas **sobrevivem a reinício**. Tudo é chaveado por **(workspaceId, agentId)**:

```text
Workspace → Agent → Conversation (messages) + Executions (encerradas)
```

- `ChatService` escreve via `ConversationHistory`: a mensagem do usuário **quando é enviada**; a execução (com timeline)
  e a mensagem do assistente **quando o run termina**.
- **Guardado**: mensagens (ambos os papéis, `executionId`, hora, tipo de falha); por execução: status, tarefa,
  runtime, modelo, horários, falha, uso/custo como reportado, `metadata`, **timeline** (eventos), `context` (modo
  task-aware/fallback, tamanhos) e `optimization`.
- **Não guardado**: saída bruta do terminal; o texto transmitido (vira a mensagem); o **prompt montado** (contexto de
  projeto repetido) — a resposta mora só na mensagem do assistente.
- **Limites por conversa**: **200 mensagens**, **100 execuções**, **300 eventos de timeline** por execução (os mais novos
  ficam). Excluir workspace ou agente apaga suas conversas/execuções; excluir agente **mantém** o uso já gasto.
- **Ids**: `exec-N`; o contador continua após o maior id guardado **ou** qualquer worktree ainda em disco — nunca reusa.
- **Run cortado** (app fechado/crash): no próximo início a mensagem do usuário sem resposta recebe **uma** mensagem de
  assistente _failed_ e uma execução `app_closed` ("Atlas was closed while this was running"); **não** é mostrada como
  cancelada.
- Quando migrar para SQLite (mesma interface `ConversationHistory`): arquivo regularmente acima de alguns MB, busca no
  histórico, ou retenção por tempo.
- Limitações: a timeline de um run em andamento está em memória (gravada ao terminar); o uso de um run só aparece depois
  que ele termina; histórico é por máquina, sem exportação/sync.

## Uso, custo e cota — "nunca inventado" (ADR 0005)

`RuntimeCapabilities` declara `usageMetrics`, `costMetrics`, `quotaMetrics`. `UsageMetrics { inputTokens?, outputTokens?,
totalTokens?, cachedInputTokens?, cost?, currency?, source }` — todo campo opcional; **`null` ≠ `0`** (null = o runtime
não reportou).

| `source`            | Significado                                       |
| ------------------- | ------------------------------------------------- |
| `runtime_reported`  | Uma execução, como a CLI reportou                 |
| `atlas_calculated`  | Soma que o Atlas fez sobre execuções que observou |
| `provider_reported` | Janelas de cota                                   |

- **Claude CLI**: `usage.input_tokens` + `cache_creation_input_tokens` + `cache_read_input_tokens` (entrada = as três),
  `output_tokens`, `total_cost_usd` (número da própria CLI, não necessariamente cobrança) e `rate_limit_event`
  (`five_hour`, `seven_day`: `utilization` 0–1 e `resetsAt`). **OpenCode**: soma por `step_finish` de `tokens` e `cost`
  (sem moeda declarada → nenhuma é afirmada; sem cota).
- Após cada execução, o `ChatService` grava um `UsageRecord` (execução, workspace, agente, runtime, modelo, tempos,
  sucesso, métricas). `UsageReporter` soma; `UsageTotals` traz `runs`, `runsWithTokens`, `runsWithCost` para que uma
  soma parcial diga quantas execuções cobre; **moedas diferentes nunca são somadas**.
- Nenhuma CLI reporta uso **durante** a execução: o Atlas mostra status/atividade ao vivo e os números ao fim. Os
  totais semanais são "rastreados pelo Atlas"; só as janelas de cota são do provedor.

## Eventos (core → UI)

| Evento                          | Para quê                                                                                                                                     |
| ------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| `execution:progress`            | `ExecutionEvent { executionId, workspaceId, taskId, agentId, kind, message, timestamp, metadata }`                                           |
| `conversation:message`          | anuncia mensagens do assistente                                                                                                              |
| `execution:output`              | chunks do terminal (`seq` por sessão)                                                                                                        |
| `execution:status`              | estado do processo (`Running/Interrupting/Terminating/Exited`)                                                                               |
| `workflow:*`                    | ciclo do run e dos nós (carregam ids; a UI relê o run) — ver [04](04-workflow.md)                                                            |
| `live_workspace:changed`        | delta do Live Workspace com `revision` monotônica                                                                                            |
| `optimization_*` (via progress) | `optimization_context_built`, `optimization_prompt_built`, `optimization_metrics_recorded` — só para ouvintes, não entram no log da execução |

Eventos são _best effort_: o estado de verdade é sempre relido (snapshot) quando há dúvida.

## Camada de otimização — fase 0: medir antes de otimizar (ADR 0020)

O Atlas é dono do prompt que envia e da resposta que lê, **não** do laço interno de ferramentas da CLI. A fase 0 é
**somente observabilidade**: ligar/desligar métricas **não muda** o prompt, a seleção do Task Context, o pedido ao
runtime, as permissões nem os eventos (um teste roda a mesma tarefa com a flag ligada/desligada e compara o prompt
byte a byte).

- `PromptBuilder` continua sendo a única montagem: `assemble` devolve o prompt **e** um `PromptLayout` tirado das
  próprias strings juntadas (nada é re-parseado). O `StepBrief` reporta um `BriefLayout` (workflow context, handoff,
  protocols) no ponto em que os escreve.
- **`PromptBreakdown`**: seções (`personality`, `atlas_rules`, `live_narration`, `plan_rule`, `harness` ou
  `task_context`, `project_context`, `agent_instructions`, `brief_*`, `task`) + `framing` (cabeçalhos/separadores),
  somando exatamente os bytes enviados.
- **Tokens**: o Atlas não tem tokenizador — todo número próprio é `ceil(chars / 4)` com `token_source: estimated`;
  o que o runtime reporta fica em `UsageMetrics` (`runtime_reported`). `PromptVersusRuntime` os coloca lado a lado.
- **`OptimizationMetrics`** (em `Execution`/`StoredExecution.optimization`, opcional): breakdown, números do Harness,
  **latência** (ms: build de contexto, build de prompt, startup do runtime, execução, total do runtime, `run_step`
  inteiro e `instrumentation_ms` — o custo de medir), chamadas de ferramenta, bytes de handoff, contadores.
- **Não observável (e dito)**: bytes de saída de ferramentas e seleção de ferramentas (`tools.totalOutputBytes` = `None`;
  `tools.calls` é o nº de eventos de ferramenta que o runtime transmitiu); caches (não existem ainda);
  tokens quando o runtime não reporta.
- **Flag**: `AppSettings.optimization.metricsEnabled`, **ligada por padrão**, lida uma vez por execução
  (port `OptimizationFlags`). Sem fonte de flag, nada é medido.
- **Benchmark** (`optimization/benchmark.rs`, só testes): `BenchmarkRow`/`Comparison` e cinco cargas de referência
  (tarefa simples, com Harness, passo de workflow com handoff, 2º passo com Harness, contexto grande). Na fase 0
  baseline e "otimizado" são o mesmo código → prompts idênticos são **critério de aceitação**.
- **Próximos passos previstos**: Context Engine (fase 1) julgado contra o `PromptBreakdown`; otimização de saída de
  ferramentas (ex.: RTK) exigirá _hooks_ nativos de cada runtime, pois esse dado não existe nesta camada. Nenhuma
  configuração persistente do usuário (hooks do RTK, `~/.claude`) é tocada.

## Dívidas de documentação já identificadas

- `docs/architecture/README.md` ainda diz "Conversations are in memory" e "No streaming yet" — o histórico persiste
  desde a V0.6 (ADR 0008) e há streaming de progresso/saída.
- ADR 0006 (segurança/sandbox) é citado por vários ADRs mas não está no repositório.
