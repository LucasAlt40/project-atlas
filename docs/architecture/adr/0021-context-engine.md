# 0021 — Camada de Otimização, fase 1: o Context Engine

Status: aceito e implementado.

## Contexto

A fase 0 ([ADR 0020](0020-optimization-observability.md)) mediu do que um prompt é feito. Mostrou que a maior parte é
texto fixo e obrigatório (as regras, as regras de narração e de plano, os protocolos de resultado e de interação) e que
o contexto que varia é o Task Context do Harness, o contexto do workflow e o handoff. O Task Context já é selecionado de
forma determinística e explicável pelo `TaskContextService` (ADR 0012); selecionar de novo seria um segundo Context
Engine. O que ninguém fazia era olhar o conjunto montado: o mesmo artefato e a mesma decisão aparecem no contexto do
workflow e de novo no handoff.

## Decisão

O Context Engine **envolve** a seleção existente; não a substitui. Ele recebe o contexto que um prompt carregaria como
`ContextItem`s (origem, prioridade, obsolescência, proveniência, fingerprint, tamanho) e devolve um plano com cada
mudança registrada.

**Prioridades.** `Required` nunca é editado, descartado ou truncado: as regras e a personalidade do Atlas, o contexto do
projeto, as instruções do agente, a própria tarefa do passo, o contexto do workflow (o mínimo do workflow) e os
protocolos. Eles servem apenas de comparação. `High` (o Task Context, o handoff) pode perder linhas que já são ditas em
outro lugar, nunca inteiro. `Normal`/`Optional` existem para o que fases futuras adicionarem e são os únicos que um
orçamento pode deixar de fora.

**Deduplicação**, linha a linha, na ordem do prompt, fora de blocos de código, para linhas de pelo menos quatro
palavras que não sejam títulos: repetição exata, repetição normalizada (caixa, pontuação, espaçamento) e sobreposição
(toda palavra da linha está em uma linha anterior, que portanto diz pelo menos o mesmo). Uma linha só sai quando as
mesmas palavras permanecem em uma parte anterior do prompt; um título de lista cujos itens saíram todos sai junto. Sem
embeddings: fingerprints, texto normalizado e conjuntos de palavras.

**Compressão** é só de espaços em branco (sequências de linhas vazias, espaços no fim); nenhuma palavra muda.

**Orçamento** (`optimization.context.maxTokens`, tokens estimados, com reservas para saída, ferramentas e raciocínio):
itens `Normal` e `Optional` inteiros são deixados de fora, os obsoletos e maiores primeiro, cada um substituído por uma
linha dizendo o que foi deixado de fora. Se o que sobra ainda estiver acima, o engine informa um `BudgetOverrun` e um
evento `optimization_budget_warning` e **não corta nada**: contexto obrigatório e relevante não é truncado para caber.
Nenhum item do prompt real está abaixo de `High` ainda, então hoje um orçamento só consegue avisar.

**Integração.** O engine retrabalha as _entradas_ do `PromptBuilder` (o texto do Harness e o texto da tarefa, brief
incluso) e o builder, que continua sendo a única montagem, monta o prompt de novo. O `StepBrief` de um passo agora carrega
suas `BriefParts` (o texto exato do contexto do workflow, handoff e protocolos) em vez de só tamanhos; se o texto da
tarefa não terminar exatamente com elas, o engine deixa o prompt em paz e registra o motivo (`skipped`). Tudo fica em
`OptimizationMetrics.contextEngine` (tamanho bruto e final, estimativas, decisões com o local da cópia mantida) e em
`optimization_context_optimized`.

**Flag.** `optimization.context.enabled`, **desligada por padrão**: diferente das métricas, ela muda o prompt. Desligada,
o prompt é idêntico byte a byte ao da fase 0 (os testes ON/OFF da fase 0 continuam valendo). Ela roda com ou sem as
métricas.

## Medido

Cargas de referência (testes de `executions`), tokens estimados, engine desligado → ligado: tarefa simples, tarefa com
Harness, contexto grande e os briefs sintéticos de passo: inalterados (nada repetido). Um passo de workflow cujo brief
lista 3 artefatos e 3 decisões duas vezes: 1.087 → 988; com 12: 1.749 → 1.366. Um brief real do orquestrador (architect →
developer, um artefato, uma decisão): 2.877 → 2.746 bytes (4,5%). O teste também confere que nenhuma palavra do prompt
original falta no retrabalhado.

## Texto obrigatório que dizia a mesma coisa duas vezes

O engine não pode tocar texto obrigatório, então a única repetição dentro dele foi removida na origem: o protocolo de
interação de um passo repetia a instrução de plano (escrevê-lo por inteiro em Markdown, salvar `docs/plans/<nome>.md`)
que o `PLAN_RULE` já coloca em todo prompt. O protocolo agora aponta para a regra de planos em uma frase (cerca de 100
bytes por passo). Isto não está atrás de uma flag: a regra para a qual aponta é incondicional.

O que foi **deixado de lado de propósito**: os avisos de "não concede permissões" (Harness, handoff, bloco de
resultado, bloco de interação). Cada um protege uma fonte diferente de texto não confiável, então não são duplicatas, e
remover um enfraqueceria essa proteção. `PLAN_RULE` é comportamento de produto (os planos que a pessoa lê) e fica em
todo prompt.

**A narração ao vivo, condicionada** (decisão do usuário em 2026-11-11). `LIVE_NARRATION` só molda a resposta ao vivo,
e a tela de uma spec mostra o documento, não o turno ao vivo. Com `optimization.narrationOnlyWhenWatched` ligada, um
turno de spec (`StepOptions::watched = false`, marcado em `ChatService::send_spec_step`) não recebe a instrução de
narrar; fica só a frase que manda pôr caminhos entre aspas nos comandos (`QUOTE_PATHS`), que protege um comando de
quebrar num caminho com espaços. Conversas e passos de workflow continuam com a narração. **Desligada por padrão**: aí o
prompt é idêntico byte a byte ao de antes. Testes: `a_run_nobody_watches_is_not_asked_to_narrate_and_keeps_the_advice_on_paths`
e `only_a_run_nobody_watches_drops_the_narration_and_only_when_the_person_chose_so`. Não há interruptor na tela; a
configuração fica no arquivo, como as outras flags desta camada. A economia é a do texto da narração (cerca de 90 tokens
estimados por turno de spec); não foi medida com modelo.

## Consequências

- A economia disponível hoje é pequena: a maioria dos tokens está em texto obrigatório. A repetição da instrução de
  plano saiu na origem, os avisos de "não concede permissões" ficam por segurança, e a narração é condicionada atrás
  de uma flag (acima).
- **Relevância por item e o sinal sobre o total (C4, 2026-11-11, decisão do usuário).** Um item carrega relevância só
  quando algo a mediu: o Task Context, a fatia do que ele incluiu por casar com a tarefa (`ContextRecord.matched_items`
  ÷ `selected_items`; restrições, decisões e a palavra do usuário entram sempre e não contam como casamento); uma skill
  que a tarefa pediu pelo nome, 1. Memórias, rules e o resto ficam sem relevância (desconhecida, não irrelevante).
  Quando um orçamento obriga a cortar, entre itens de uma prioridade sai antes o desatualizado, depois o menos
  relevante, e um item sem nota conta como meio caminho. `ContextEngineMetrics.quality` (`ContextQuality`) registra
  quantos itens há, quantos têm relevância, a média dos que têm (em milésimos) e quantos estão desatualizados; nenhuma
  média é inventada quando nada foi medido. Testes: `a_task_contexts_relevance_is_the_share_it_chose_for_the_task`,
  `a_budget_leaves_out_the_less_relevant_first_and_an_unscored_item_counts_as_halfway`,
  `the_context_quality_counts_only_what_is_measured`.
- **Obsolescência e cache (C5, 2026-11-11).** Um Task Context que inclui conhecimento desatualizado
  (`ContextRecord.stale_items > 0`) marca o seu item como desatualizado. O cache do fingerprint do projeto
  (`with_fingerprint_cache`, 5 s no app, para as prévias enquanto a pessoa digita) agora conta o que serve do cache e o
  que escaneia (`fingerprint_cache_stats`); medido por `the_fingerprint_cache_serves_a_second_build_and_counts_both`.
  Um cache do plano do engine pelo fingerprint dos itens não foi feito: o engine roda em milissegundos, então não haveria
  o que ganhar.
- **Limite da granularidade.** O Harness e o Task Context entram no engine como **um item cada**, com prioridade `High`,
  que um orçamento nunca corta. A relevância e a obsolescência deles aparecem no sinal de qualidade, mas não mudam um
  corte; mudam os cortes entre itens abaixo de `High` (skills casadas, memórias, preferências, partes do brief).
  Quebrar o Harness em itens por entrada seria a próxima etapa, se um uso real mostrar contexto errado por isso.
