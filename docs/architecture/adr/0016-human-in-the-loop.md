# 0016 — Humano no circuito: um agente esperando por uma pessoa

## Contexto

Um runtime como `claude -p` ou `opencode run` responde uma vez e encerra. Quando o agente precisava de uma decisão, ele
escrevia uma pergunta como resposta, o processo terminava e o Atlas chamava a execução de `completed`: o workflow
seguia para o próximo passo sem que nada tivesse sido feito.

## Decisão

**Esperar é um estado do domínio, não uma linha de log.**

| Nível            | Estado                                                                                              |
| ---------------- | --------------------------------------------------------------------------------------------------- |
| Execução         | `ExecutionStatus::WaitingForInput` — não é `completed` (sem resultado), nem `failed`                |
| Nó do workflow   | `NodeStatus::WaitingForInput`, tentativa `AttemptStatus::WaitingForInput`; o passo não tem desfecho |
| Execução do flow | `WorkflowExecutionStatus::WaitingForInput`: nada novo começa até a pergunta ser respondida          |
| A pergunta       | `PendingInteraction` (`domain/interaction.rs`), guardada em `WorkflowExecution.interactions`        |

O `outcome` continua sendo o que o contrato de resultado do agente diz (`implemented`, `pass`…); "esperando" nunca é um
desfecho. Estado do processo e estado lógico são coisas diferentes: o processo em geral já saiu, e o passo continua
esperando.

### Detecção

O `ExecutionService` olha a resposta de um passo de workflow (e só de um passo de workflow: numa conversa a pessoa
simplesmente responde). Em ordem, vale a primeira que se manifestar:

1. o adaptador do runtime, `ModelRuntime::detect_interaction` (o único lugar que pode conhecer o protocolo próprio de uma
   ferramenta; nenhum adaptador atual tem um);
2. o bloco `atlas-interaction` com que todo agente é instruído a terminar quando não consegue continuar
   (`application/interaction/structured.rs`) — funciona com todo runtime;
3. uma análise conservadora de texto de como a resposta _termina_ (`heuristic.rs`): a última frase é uma pergunta,
   dirigida à pessoa, e a resposta não traz nenhum `atlas-result`. Ofertas de mais trabalho ("Quer que eu também…") e `?`
   dentro de código ou no meio da resposta não contam.

Nenhum modelo é consultado para classificar a saída. Uma detecção abaixo de `MIN_CONFIDENCE` (70) nunca pausa. Um
`atlas-result` depois de um bloco de interação significa que o agente concluiu.

O bloco é dado: um conjunto fechado de campos, truncados, um conjunto fechado de tipos. Para aprovação, permissão e
confirmação de runtime os botões são do Atlas (`approve/reject`, `allow/deny`, `yes/no`); um agente não pode inventá-los.

### Pausar, responder, retomar

O `Orchestrator::handle` transforma um `StepStatus::WaitingForInput` em `engine.wait_for_input`: a pergunta é persistida
com a execução, nenhum resultado é interpretado, nenhum handoff é construído, nenhuma rota é seguida.
`answer_interaction` valida a resposta contra a pergunta (ainda pendente, uma das suas opções, texto limitado), registra
e devolve o nó para `Ready` como uma _continuação da mesma passada_ (`continuing`, mesma iteração). O próximo início é
uma nova tentativa/execução cuja instrução carrega um bloco `HUMAN INPUT` (a pergunta e a resposta, citadas), porque os
runtimes de execução única não guardam conversa; os arquivos já escritos continuam no worktree da execução.

Uma recusa também não é uma conclusão: o agente é informado de que foi recusado e termina sob o próprio contrato
(`blocked`, `fail`…).

Uma resposta a uma execução que ninguém conduz (o app foi fechado depois da pergunta) é registrada e a execução é
retomada com `resume_interrupted`. Uma execução interrompida enquanto esperava mantém a sua pergunta.

Cancelar a execução faz a pergunta expirar (`cancelled`) e cancela o nó.

### Trava do worktree

Um passo que está esperando mantém seu lugar no `WorktreeLock` (ainda está "em andamento"), e um passo prestes a
continuar o mantém durante a rodada em que é reenviado, de modo que nenhum outro escritor possa entrar entre a pergunta e
a resposta. Isso troca a latência dos escritores paralelos por nunca mudar o estado debaixo de um agente que vai
continuar nele. Leitores não são afetados por outros leitores.

### Não é uma integração

Responder deixa o agente continuar **dentro do worktree isolado da execução**. Isso nunca aplica nada ao projeto e nunca
muda uma política, uma permissão ou a política de integração. O ChangeSet é revisado e aplicado pela pessoa depois,
como antes; um validador somente leitura ou o QA continua sem bloqueá-lo.

### Auditoria

Cada transição é um evento de workflow na execução: `interaction_detected`, `node_waiting_for_input` (execução
pausada), `interaction_answered`, `node_input_resolved` (execução retomada), `interaction_rejected` (uma resposta
recusada), `interaction_cancelled`. As perguntas com suas respostas e horários ficam em `interactions`.

### Roteamento

Os templates de workflow embutidos roteiam por `result.outcome`, que os contratos dos agentes validador/QA declaram
(`pass`/`fail`). Workflows que roteiam por `result.status` continuam funcionando, mas os templates não fazem mais isso;
um agente sem o outcome é apontado pela validação antes da execução.

## Não feito

- Timeout opcional, global (`AppSettings.questionTimeoutMinutes`, sem limite por padrão): a pergunta vira `expired`
  e o passo falha sem nova tentativa (ver [guia 04](../../guia/04-workflow.md#humano-no-loop)).
- No máximo uma pergunta pendente por passo.
- Nenhum runtime informa ainda um evento estruturado de pergunta; o gancho existe.
- Respostas de `permission`/`approval` são informação para o agente. O que uma ferramenta pode de fato fazer continua
  sendo decidido pela política e pelo broker de aprovação, que mantêm suas próprias aprovações.
