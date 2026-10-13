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
- **Comparação antes/depois em execuções reais**: não existe ainda. As otimizações 3 a 5 só aparecem nos números das
  próximas execuções; a barra de consumo é como se mede.

## Riscos

- Uma edição pode ser aplicada limpa e ainda assim deixar o documento incoerente (um requisito citado em outra seção).
  A validação cruzada e a revisão por IA continuam sendo o que pega isso, como com qualquer revisão.
- O limite de tokens depende do que o runtime informa; uma execução sem uso informado não conta.
