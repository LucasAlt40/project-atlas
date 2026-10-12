# 0017 — Validação de workflow ciente do contrato e reparo assistido de rotas

## Contexto

Uma execução real terminou como `failed` embora todo passo tivesse feito o seu trabalho. O QA declarava os desfechos
`approved` / `changes_requested` (um contrato `review`), enquanto o workflow salvo roteava o QA com
`result.status = pass` / `fail`. Nenhuma aresta podia casar `approved`, então a execução parou com `no_route_matched`. O
engine se comportou corretamente: a definição estava errada, e nada avisou o usuário antes da execução. A única migração
que existe (ADR 0015) redireciona arestas `result.status` para `result.outcome` somente quando o agente declara
exatamente aquele valor, então com razão não tocou no QA.

## Decisão

**Quatro coisas ficam separadas.** O _contrato do agente_ diz quais desfechos um agente pode produzir. O _workflow_ diz o
que fazer com cada um. Um _nó End_ diz quando o workflow termina. O _status da execução_ é o estado técnico da
execução. Não existe "desfecho terminal": `approved`, `pass`, `fail` e `changes_requested` não são terminais em si; o
que termina um workflow é uma rota até um nó End. Nenhum fim de passo, e nenhuma rota ausente, jamais conclui uma
execução.

**`result.status` não é `result.outcome`.** `result.status` diz se o passo rodou (`success` / `failed`).
`result.outcome` é o veredito que o agente declarou. Apenas `pass` e `fail` são espelhados em `result.status`, para que
arestas salvas antes do roteamento por desfecho continuem funcionando para um agente que os declara. Todo o resto do
significado é lido de `result.outcome`.

**A validação compara contrato e rotas antes da execução** (`application/workflow/validation.rs`), com uma severidade
por problema; só erros impedem uma execução de iniciar:

| Problema                   | Severidade | Quando                                                                                              |
| -------------------------- | ---------- | --------------------------------------------------------------------------------------------------- |
| `outcome_without_route`    | erro       | O agente declara um desfecho que nenhuma aresta saindo do seu passo tomaria, havendo outras arestas |
| `outcome_without_route`    | aviso      | O passo não tem nenhuma aresta (também é um `dead_end`)                                             |
| `status_route_on_contract` | erro       | Uma aresta testa `result.status = X` para um agente com contrato, e `X` não é um desfecho espelhado |
| `undeclared_outcome`       | erro       | Uma aresta testa um desfecho que o agente não declara (ADR 0015)                                    |
| `cannot_reach_end`         | erro       | Uma aresta leva a uma parte do grafo da qual nenhum nó End é alcançável (só pelo grafo)             |

**Uma regra do engine de workflow, não uma consequência técnica (aguardando confirmação).** Recusar-se a executar um
agente que declara um desfecho sem rota, enquanto outros desfechos dele têm rota, é uma decisão sobre a _semântica_ dos
workflows: que todo veredito que um agente pode dar deve levar a algum lugar. Nada técnico a força. O engine já trata
uma rota ausente em tempo de execução (`no_route_matched`), e a verificação poderia muito bem ser um aviso, ou um erro
só para desfechos que o workflow marca como obrigatórios. É um erro por enquanto, e não é definitivo: a regra deve ser
confirmada antes de se depender dela. Mudá-la muda apenas a severidade escolhida em `check_outcome_routes`.

Uma aresta incondicional recebe todo desfecho. Uma aresta sobre outro fato conta como podendo recebê-lo: a validação
informa o que certamente não tem rota, não o que talvez não tenha. Nada nomeia um agente, uma personalidade ou um
desfecho.

**`no_route_matched` não muda.** Se uma definição rodasse mesmo assim (os testes iniciam uma sem validação de
propósito), um desfecho sem rota faz a execução falhar de forma determinística, nomeando o passo e o desfecho. Nunca
concluída, nunca uma aresta adivinhada, nunca um End presumido.

**O reparo é proposto, confirmado e registrado** (`application/workflow/repair.rs`). O Atlas não infere que `pass`
significa `approved`: `pass` pode ser uma verificação técnica e `approved` uma revisão. A validação consegue detectar a
incompatibilidade; só o usuário pode dizer o que se pretendia. Então:

1. `suggest_route_repairs` lista as arestas incompatíveis de cada passo com uma _sugestão_ construída apenas a partir do
   formato do workflow (as arestas na ordem desenhada, os desfechos sem rota na ordem do contrato). Não muda nada.
2. O usuário vê o atual e o sugerido lado a lado, pode escolher outro desfecho por aresta, e aplica ou cancela.
3. `repair_workflow_routes` reconfere cada escolha contra o workflow salvo (uma escolha obsoleta ou desconhecida não
   muda nada), reescreve as arestas confirmadas para `result.outcome`, cria uma nova versão do workflow e acrescenta um
   registro `RouteRepair` (aresta, passo, agente, alvo, condição anterior, condição nova, versão, hora) a
   `Workflow.route_repairs`. O histórico vive no workflow, junto das versões que ele explica; não há um novo sistema de
   auditoria, e a auditoria de permissões (`AuditLog`) continua sendo sobre permissões. O `update_workflow` nunca deixa
   um chamador reescrever esse histórico.

O reparo cobre toda rota que espera um resultado que o seu agente não consegue dar: um teste de `result.status`, ou um
desfecho que o contrato não declara.

**Os templates seguem o contrato do agente que recebem.** Um workflow padrão nunca pode nascer quebrado. Onde um
template tem um passo que julga (validator, QA), o `Builder::gate` o roteia pelo par declarado por inteiro pelo agente
que recebeu: `pass`/`fail` (o preset `validation`) ou `approved`/`changes_requested` (o preset `review`). Esses pares
são presets do próprio Atlas, não um palpite sobre redação. Um agente que não declara nenhum dos pares (um contrato
geral ou customizado) recebe `pass`/`fail`, e a validação diz o que lhe falta, já que nenhum par pode ser escolhido para
ele. Um teste constrói cada template contra cada combinação dos dois presets.

Abrir o app, validar e salvar nunca reparam nada, então a migração continua idempotente.

## Consequências

- Um workflow como o do bug é recusado com uma mensagem que explica status versus desfecho e nomeia os desfechos
  declarados do agente; o editor oferece "Revisar rotas".
- Os avisos são listados à parte e não impedem uma execução.
- Workflows que eram válidos por acidente (um agente com contrato e um conjunto parcial de rotas) agora são recusados
  até que o desfecho faltante seja roteado. Isso decorre da regra semântica acima, que ainda precisa ser confirmada.
- Retomar uma execução que parou por falta de rota, depois que as rotas forem reparadas, pode levar direto a um End; a
  execução então conclui de imediato e seu código é examinado como em qualquer execução que termina.
