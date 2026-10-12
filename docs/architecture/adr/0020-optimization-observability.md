# 0020 — Camada de Otimização, fase 0: medir antes de otimizar

## Contexto

O Atlas monta um prompt para cada execução (personalidade, regras, Harness/Task Context, projeto, instruções do agente e,
para um passo de workflow, um brief com estado e handoff) e o entrega a um runtime de CLI. Ninguém sabia dizer o tamanho
de cada parte, quanto tempo leva cada etapa, nem quanto do que o Atlas montou virou token no modelo. Otimizar sem isso
seria adivinhar, e um prompt menor que perde algo útil é uma regressão.

Os runtimes são CLIs (`claude -p`, `opencode run`, …). O Atlas é dono do prompt que envia e da resposta que lê; ele **não**
é dono do loop interno de ferramentas da CLI. O que uma ferramenta imprimiu nunca passa pelo Atlas, então a saída das
ferramentas e a seleção de ferramentas não são observáveis aqui (veja "Não observável").

## Decisão

**A fase 0 é só observabilidade.** Com as métricas ligadas ou desligadas, o prompt, a seleção do Task Context, a
requisição ao runtime, as permissões e os eventos além dos três abaixo são os mesmos. Não existe em lugar nenhum um
`if metrics { mudar o prompt }`; um teste roda a mesma tarefa com a flag ligada e desligada e compara o prompt byte a
byte e tudo o mais que é entregue ao runtime.

- **O `PromptBuilder` continua sendo a única montagem.** `assemble` devolve o prompt e um `PromptLayout` tirado das
  próprias strings que juntou; `build_with_access` é a mesma chamada sem o layout. Nada é lido de volta a partir do
  prompt. O `StepBrief` de um passo de workflow igualmente informa um `BriefLayout` (contexto do workflow, handoff,
  protocolos) onde os escreve, passado à execução por `StepRequest` → `WorkflowStepRequest` → `StepOptions`.
- **`PromptBreakdown`** lista as seções (`personality`, `atlas_rules`, `live_narration`, `plan_rule`, `harness` ou
  `task_context`, `project_context`, `agent_instructions`, os três `brief_*`, `task`) e soma o resto como `framing`
  (títulos e separadores), de modo que as seções sempre somem os bytes enviados.
- **Tokens.** O Atlas não tem tokenizador. Todo número que ele calcula é `ceil(chars / 4)` e carrega
  `token_source: estimated`. O que um runtime informa fica no `UsageMetrics` existente (`source: runtime_reported`), agora
  com `cached_input_tokens`; não há um segundo contador. `PromptVersusRuntime` coloca os dois lado a lado e marca o lado
  do runtime como `exact` ou `unavailable`.
- **`OptimizationMetrics`** (domínio, sem comportamento) é guardado em `Execution` e `StoredExecution` como
  `optimization`, com `#[serde(default)]` e omitido quando ausente, então históricos antigos carregam e leitores antigos
  veem o formato antigo. Ele guarda o breakdown, os números do Harness (do `ContextRecord` existente), latência,
  chamadas de ferramenta, bytes de handoff e contadores. Um número que o Atlas não consegue observar é `None`.
- **Latência** (milissegundos, f64): montagem do contexto (só com um serviço de Harness), montagem do prompt, partida do
  runtime (início → o próprio relatório `Waiting` do runtime), execução do runtime, total do runtime, o `run_step`
  inteiro e `instrumentation_ms`, o que a própria medição custou. Um `RuntimeProbe` observa o callback de progresso que o
  runtime já chama; ele não reporta nada por conta própria.
- **Eventos** reaproveitam `ExecutionEvent`/`ExecutionObserver`: `optimization_context_built`,
  `optimization_prompt_built`, `optimization_metrics_recorded`. São anunciados só aos ouvintes (não entram no log da
  execução).
- **Flag.** `AppSettings.optimization.metricsEnabled` (`optimization.metrics.enabled`), ligada por padrão, lida uma vez
  por execução pela porta `OptimizationFlags`. Um `ExecutionService` construído sem fonte de flag não mede nada.
- **Base de benchmark** (`application/optimization/benchmark.rs`, só em testes): `BenchmarkRow` e `Comparison` (duas
  execuções de um cenário e se seus prompts são idênticos) e cinco cargas de referência nos testes de `executions`:
  tarefa simples, tarefa com Harness, passo de workflow com handoff, segundo passo de um workflow com Harness, contexto
  grande. Na fase 0 o baseline e o "otimizado" são o mesmo código, então prompts idênticos são um critério de
  aceitação, não um resultado.

## Não observável (e dito como tal)

- **Bytes de saída das ferramentas e a seleção de ferramentas**: as ferramentas da CLI falam direto com o modelo.
  `tools.totalOutputBytes` é sempre `None`. `tools.calls` é o número de eventos de ferramenta que o runtime transmitiu,
  e `None` quando não transmitiu nenhum (o Atlas não distingue "nenhuma ferramenta usada" de "este runtime não transmite
  eventos de ferramenta").
- **Caches**: ainda não existe nenhum; `cacheHits`/`cacheMisses` são `None`. `deduplicatedItems`/`compressedItems` são 0
  porque a fase 0 não faz nenhuma das duas coisas.
- **Tokens do runtime** quando o runtime não informa nenhum (o número de cache do Gemini não é lido; as CLIs de
  substituição informam só um custo).

## Consequências

- A fase 1 (Context Engine) pode envolver o `TaskContextService` e ser julgada contra o `PromptBreakdown` e as cargas de
  referência.
- A otimização da saída de ferramentas está fora do alcance desta camada: os dados que ela mediria não existem aqui
  (veja a ADR 0023).
- A configuração persistente do usuário nunca é tocada (sem hooks de agente, sem mudanças em `~/.claude`).
