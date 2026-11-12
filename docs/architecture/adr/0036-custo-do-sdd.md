# ADR 0036 — Custo do SDD: medir, editar por seções, não repetir trabalho

**Status:** aceito (parcialmente implementado: ver "O que não foi feito")

## Contexto

Escrever e revisar uma especificação (ADR 0035) é uma sequência de execuções de agente. Antes de mexer em qualquer coisa
foi preciso saber onde o custo estava, com os dados que o Atlas já guarda (`config.json`: execuções e o registro de uso).

### Linha de base (medida, de uma única spec, na máquina de quem a escreveu)

Uma spec de exclusão de conta, com `optimization.metrics.enabled` ligado e o runtime Claude. Das 16 execuções de spec que
ainda estavam no histórico, 13 tinham uso informado pelo runtime:

|                                       | Entrada         | Saída          | Do cache | Custo informado |
| ------------------------------------- | --------------- | -------------- | -------- | --------------- |
| Soma das 13                           | 711 745         | 83 438         | 443 196  | US$ 2,00        |
| Maior execução (design, primeira vez) | 289 695         | 14 104         | 245 690  | US$ 0,37        |
| Revisão final (2×)                    | 93 939 / 93 333 | 8 486 / 5 759  | ~64 000  | US$ 0,22 / 0,18 |
| Reescrita do design após um pedido    | 25 111 / 26 405 | 8 871 / 11 972 | 4 544    | US$ 0,17 / 0,21 |
| Reescrita de tarefas                  | 21 256–28 000   | 5 758–6 250    | ~4 500   | US$ 0,13–0,16   |

Não é um benchmark: é uma spec, uma máquina, e três das 16 execuções não informaram uso. Serve para dizer onde olhar:

1. **Saída.** Cada pedido de alteração fazia o agente escrever o documento inteiro de novo (6 a 12 mil tokens de saída
   por vez), ainda que a mudança fosse uma seção.
2. **Exploração do projeto.** O prompt que o Atlas monta tem 3 a 13 mil tokens estimados; o que o runtime cobra de
   entrada é muito mais (14 a 290 mil), porque os agentes de spec leem o projeto com `Glob`, `Grep` e `Read`. A
   execução mais cara fez 15 chamadas dessas.
3. **Revisão final.** Lê os quatro documentos e o projeto de novo, a cada vez.
4. **Nada mostrava isso na tela.**

## Decisões

### 1. Consumo por spec, na tela

`SpecView.usage` soma, a partir do registro de uso que o Atlas já guarda para cada execução (`UsageLedger`), as execuções
da spec: entrada, saída, entrada servida do cache, custo e moeda, por execução e no total, e **quantas das execuções
informaram números**. Um campo que o runtime não informou é `None`, nunca zero; custos em moedas diferentes não se
somam. A tela mostra uma barra "Consumo" com o total e, aberta, a lista por execução. Os números são os que o runtime
informou (`runtime_reported`): o Atlas não os estima.

### 2. Orçamento de tokens por spec

`optimization.specs.maxTokens` (`AppSettings.optimization.specs_max_tokens`, padrão: sem limite; Configurações → "Limite
de tokens por especificação"). Entrada mais saída informadas pelo runtime. Ao chegar nele, `prepare_generation` e
`prepare_review` recusam (`SpecBudgetReached`, com o gasto e o limite) e a tela mostra um aviso com "Continuar (+limite)":
cada vez que a pessoa continua, a spec ganha o limite de novo (`Spec.budget_steps`). Nada é apagado nem aprovado por isso,
e uma decisão da pessoa (pedido de alterações, resposta) fica gravada mesmo quando a execução não começa. Sem uso
informado, nada é considerado gasto: o limite não é adivinhado.

### 3. Alteração por seções

Quando a pessoa pede uma mudança num documento longo (a partir de 3 000 caracteres), nada do que ele é escrito a partir de
mudou e o pedido tem texto, o agente responde só com as seções que mudam:

```
<<<ATLAS-EDIT path="design.md" op="replace" heading="## Decisões">>>
## Decisões
…texto novo da seção…
<<<END-ATLAS-EDIT>>>
```

`op` é `replace`, `insert_after` ou `delete`. O Atlas aplica as edições ao documento que já tem (`specs/edits.rs`):
a seção vai do título até o próximo título do mesmo nível ou superior, ignorando o que está dentro de blocos de código.
O resto do documento tem os mesmos bytes de antes. Uma edição é recusada, e a execução falha com o documento intacto,
quando: o título não existe ou aparece mais de uma vez, o arquivo não existe, o texto novo não começa com um título do
mesmo nível, contém um título do mesmo nível ou superior (seria de outra seção), tenta apagar o título do documento, ou a
resposta foi cortada. Depois de uma falha por edição, a próxima tentativa pede o documento inteiro (`Generation::Failed`
com `output_edit_*`). Uma resposta com documentos inteiros continua valendo como sempre (e pode vir junto com edições,
para um arquivo novo). A revisão resultante passa pela mesma validação e pela mesma aprovação de qualquer outra.

Documentos curtos, reescritas ("escrever de novo") e revisões porque um documento de origem mudou **não** usam edições:
o documento inteiro pode precisar acompanhar a mudança.

### 4. Uma revisão que diz o mesmo não deixa os dependentes desatualizados

Se o texto novo é igual ao anterior (fim de linha e espaços no fim da linha não contam), os documentos escritos a partir
dele não são marcados como desatualizados. Qualquer palavra diferente conta como mudança. A nova revisão continua sendo
uma revisão nova: a aprovação, a revisão por IA e a validação cruzada que leram a anterior continuam sendo refeitas.

### 5. A segunda revisão sabe o que mudou

O prompt do revisor, quando já houve uma revisão concluída, traz o resumo do que ele disse, a lista de documentos cujas
revisões mudaram desde então, e pede que olhe com mais cuidado o que mudou e como se encaixa no resto. Ele continua
recebendo **todos** os documentos: a revisão final é sobre o conjunto. Quando nada mudou, a tela diz que rodar de novo
gasta tokens sem dizer nada de novo.

## O que já existia e continua valendo

- Os turnos de spec são execuções normais: passam pelo Context Engine, pelas Skills e pelos guardrails **quando as flags
  ligam**. Na configuração em que isto foi medido, `context.enabled` e `skills.enabled` estão desligados (padrão do
  Atlas); `guardrails.enabled` e `metrics.enabled` estão ligados. Os guardrails rodam antes de cada turno de spec (o
  bloqueio indevido do protocolo de resultado, que só vale para passos de workflow com brief, foi corrigido).
- A validação de estrutura e de coerência é código determinístico, sem modelo, e não precisa de cache: roda em
  milissegundos e é refeita só quando a revisão muda.
- O Atlas não cacheia respostas de modelos: uma aprovação ou uma revisão por IA só vale para as revisões exatas que leu.

## Medição com modelo real (2026-11-11)

As otimizações 3 e 5 não se desligam, então o antes e o depois foram medidos **dentro da mesma spec**: a mesma espécie
de mudança no `design.md` pedida uma vez como edição de uma seção (`Revise`) e outra como reescrita inteira (`Redo`), e
a primeira revisão contra a segunda, depois de uma mudança. Pelo Atlas inteiro (serviço de specs, runner, chat,
execuções, guard, CLI real), num projeto pequeno de exemplo; números informados pelo runtime, lidos do registro de
uso. Teste: `application::specs::real_cost_tests::sdd_real_cost_before_and_after` (`#[ignore]`, nome exato). Claude
Sonnet, uma execução por turno.

| Turno                                   | Desfecho             | Entrada | Do cache | Saída  | Custo (US$) |
| --------------------------------------- | -------------------- | ------- | -------- | ------ | ----------- |
| escrever a proposta                     | concluído            | 25 205  | 20 148   | 1 441  | 0,037       |
| escrever as specs                       | concluído            | 9 122   | 3 882    | 1 457  | 0,036       |
| escrever o design                       | concluído            | 31 371  | 24 379   | 3 526  | 0,066       |
| escrever as tarefas                     | concluído            | 12 281  | 3 882    | 3 067  | 0,065       |
| **design: mudar uma seção (edição)**    | concluído            | 13 554  | 3 882    | 2 600  | **0,065**   |
| **design: escrever de novo (inteiro)**  | parou para perguntar | 13 518  | 3 882    | 1 080  | 0,050       |
| design: escrever de novo, após resposta | concluído            | 13 670  | 3 882    | 5 759  | **0,097**   |
| **revisão 1**                           | concluída            | 34 119  | 20 862   | 3 926  | **0,094**   |
| tarefas: mudar uma seção (edição)       | concluído            | 16 579  | 3 882    | 1 479  | 0,066       |
| **revisão 2 (sabe o que mudou)**        | concluída            | 17 407  | 3 882    | 3 197  | **0,087**   |
| **Total (10 turnos)**                   |                      | 186 826 | 92 563   | 27 532 | **0,66**    |

O que os números dizem, com uma execução por turno (indício, não estatística):

- **Otimização 3 (edição por seção):** a mudança respondida como edição saiu com **55% menos saída** (2 600 contra 5 759
  tokens) e **33% mais barata** (US$ 0,065 contra 0,097) que a reescrita inteira do mesmo documento, com a mesma
  entrada. A reescrita ainda custou um turno a mais: o agente parou para perguntar (US$ 0,050). O teste não registrou
  se a resposta veio como blocos `ATLAS-EDIT` ou como documento inteiro; a saída menor que a do design original
  (3 526) indica edição.
- **Otimização 5 (a segunda revisão sabe o que mudou):** metade da entrada (17 mil contra 34 mil) e 19% menos saída,
  mas só **8% mais barata** (US$ 0,087 contra 0,094), porque a primeira teve 21 mil tokens servidos do cache e a
  segunda quase nenhum. O ganho em tokens é real; em custo, depende do cache.
- **Uma spec inteira neste projeto pequeno custou US$ 0,66**, contra US$ 2,00 da linha de base. Não é comparável: a
  linha de base é outra spec, num projeto maior, e a exploração do projeto é a maior parte do custo.

Outro ponto, do histórico local da pessoa (24/10, Claude Opus, outra spec, depois destas otimizações): 9 execuções,
US$ 4,19, e duas tentativas seguidas de alterar o design falharam antes da terceira passar. Os tamanhos de saída
(612 e 1 178 tokens) sugerem edições recusadas e depois o documento inteiro; o motivo não foi conferido.

## O que não foi feito

- **Modelo por etapa.** O modelo é o do agente escolhido para escrever e para revisar; não há como trocar por etapa sem
  criar um segundo sistema de configuração. Dá para ter um agente "revisor" mais barato ou mais forte que o autor.
- **Prompt caching.** O runtime Claude já informa tokens vindos do cache (aparecem na barra de consumo), mas o Atlas
  entrega o prompt como um bloco só, então só o prefixo do sistema do próprio runtime é aproveitado (cerca de 4 500
  tokens, nas medições). Reordenar o prompt não muda isso sem partir o prompt em blocos, o que depende do runtime.
  Os demais runtimes não informam tokens de cache; nenhuma economia é declarada para eles.
- **Estimativa antes de executar.** Há o consumo real depois e o limite; uma estimativa antes dependeria de dados que a
  maioria das execuções não tem.
- **Exploração do projeto pelos agentes.** É a maior parte do custo medido e também o que ancora os documentos no
  código real. Não foi limitada. O limite de tokens por spec é o freio.
- **Invalidação por seção.** Documentos dependentes continuam sendo marcados como desatualizados por inteiro quando o
  de origem muda de verdade; decidir "esta mudança não afeta o design" exigiria interpretar o texto.
- **Skills especializadas** para elicitação, cenários ou decomposição: sem evidência de que ajudariam, não foram criadas.
- **Diff entre revisões** na tela.

## Riscos

- Uma edição pode ser aplicada limpa e ainda assim deixar o documento incoerente (um requisito citado em outra seção).
  A validação cruzada e a revisão por IA continuam sendo o que pega isso, como com qualquer revisão.
- O limite de tokens depende do que o runtime informa; uma execução sem uso informado não conta.
