# ADR 0013 — Workflow Engine e Orchestrator

**Status:** aceito. Baseia-se na [ADR 0009](0009-git-worktree-isolation.md) (isolamento), nas
[ADR 0010](0010-project-harness.md) / [0012](0012-task-aware-context.md) (contexto) e nas
[ADR 0003](0003-command-permissions.md) / [0007](0007-live-shell-and-process-control.md) (permissões e processos).

## Por quê

Até a V0.8 a unidade de trabalho era um agente fazendo uma tarefa:

```text
Tarefa ─▶ Agente ─▶ Execução
```

O trabalho real é um processo: alguém projeta, alguém constrói, alguém confere, alguém corrige o que a conferência
achou, e a conferência roda de novo. A V0.9 torna esse processo algo de primeira classe, visível e editável, **sem**
dar a nada de novo o poder de executar processos:

```text
Tarefa ─▶ Workflow ─▶ Workflow Engine ─▶ Orchestrator ─▶ ExecutionService ─▶ RuntimeRegistry ─▶ ModelRuntime
```

Filosofia: **uma IA pode sugerir e fazer o trabalho; o Workflow Engine controla o processo.**

## Quem faz o quê

| Parte                | É                                                                                     | Não sabe nada sobre                          |
| -------------------- | ------------------------------------------------------------------------------------- | -------------------------------------------- |
| **Workflow**         | o grafo declarativo (nós, arestas, condições, políticas)                              | como é desenhado, como executa               |
| **Workflow Engine**  | uma máquina de estados pura sobre uma execução: o que está pronto, o que vem a seguir | relógios, threads, armazenamento, runtimes   |
| **Orchestrator**     | conduz uma execução: inicia passos, ouve como terminaram, persiste                    | o que o grafo significa (pergunta ao engine) |
| **ExecutionService** | uma execução de agente (inalterado)                                                   | workflows                                    |
| **Runtime**          | conversa com Claude / OpenCode (inalterado)                                           | workflows                                    |
| **Graph UI**         | uma forma de desenhar e editar um Workflow (`@xyflow/react`)                          | o engine; nunca é lido pelo núcleo           |

- **Workflow não é Execution.** Um workflow é uma definição que é editada e versionada; uma execution é uma execução de
  agente. Uma _execução de workflow_ (um run) é uma instância do workflow e é dona de várias execuções. Definições e
  runs ficam em listas separadas (`workflows`, `workflow_executions`) do mesmo `config.json` (gravações atômicas; sem
  novo banco de dados).
- **O Orchestrator não é um Agente.** Um agente é um modelo com uma personalidade; pode errar, ser induzido a qualquer
  coisa e não tem autoridade. O orquestrador é código determinístico. Nada que um agente escreve chega ao grafo: veja
  _Autoridade_.
- **A biblioteca de grafo não define o domínio.** O `@xyflow/react` aparece apenas em `features/workflow/components`.
  `features/workflow/model/graph.ts` é a única tradução entre o `Workflow` do domínio e o que a biblioteca desenha; o
  modelo persistido tem coordenadas simples e ids próprios. O Rust nunca ouviu falar dela.
- **Não há comunicação agente a agente.** Os passos não conversam entre si. Eles deixam artefatos, decisões e resultados
  em um estado compartilhado e estruturado, e o orquestrador entrega a cada passo o que ele precisa. (Uma conversa entre
  agentes seria ilimitada, cara, irreproduzível e tornaria "quem decidiu isto" impossível de responder.)

## Semântica do grafo

- **Nó** — uma unidade de trabalho: `agent` (uma execução de um Agente existente: o nó guarda só o `agent_id`, suas
  próprias instruções e as políticas de retry, falha e loop — nunca a personalidade, o runtime ou o modelo),
  `condition` (testa o resultado anterior e registra `result.matched`), `end` (um terminal: concluído / falhou /
  cancelado).
- **Aresta** — uma transição, tomada quando sua origem conclui e sua **condição** opcional vale para o resultado da
  origem. Uma **rota de falha** (`FailurePolicy::RouteToNode`) também é uma transição; o engine a trata como um vínculo
  igual a uma aresta.
- **Condição** — `{ field, operator, value }` sobre um conjunto fechado de campos (`result.status`, `result.summary`,
  `result.next_action`, `result.findings`, `result.matched`, `validation.status`) e quatro operadores (`equals`,
  `not_equals`, `exists`, `not_exists`). Sem expressões, sem código. A validação rejeita um campo desconhecido.
- **Artefato** — uma saída compartilhável de um passo: tipo, nome, nó produtor, execução, um _caminho relativo_ e um
  resumo. Ele referencia o trabalho; não o copia. **Decisão** — conhecimento que os passos seguintes devem respeitar
  (título, decisão, justificativa, nó de origem).
- **Execução de Workflow** — uma instância, guardando um **snapshot** da versão do workflow de que partiu.

### Ativação, junções e loops

Um nó fica pronto quando é _ativado_ pelos vínculos que dispararam em sua direção. Vínculos de fora de um ciclo são
**requisitos** (arestas incondicionais: todos devem ter disparado) ou **alternativas** (arestas condicionais e rotas de
falha: qualquer uma serve); qualquer dos dois grupos basta. Vínculos _dentro_ de um ciclo (as duas pontas no mesmo
componente fortemente conexo) são sempre alternativas: qualquer um reativa o nó. É isso que permite a uma junção esperar
dois ramos paralelos enquanto `QA → Bug Fixer → QA` não trava. Um vínculo saindo de um passo que falhou e foi entregue à
sua rota de falha é ignorado pelas junções (nunca vai disparar).

Loops são permitidos, mas **todo ciclo precisa ser limitado**: a validação remove os nós que carregam um limite de loop
(`loop_policy { loop_id, max_iterations }`) e rejeita o grafo se restar um ciclo. Um nó pode _iniciar_ no máximo
`max_iterations` vezes; a próxima partida faz a execução falhar com `max_iterations_reached` (id do loop e nó
registrados). `QA → Bug Fixer → QA` e `Validator → Bug Fixer → Validator` são dois loops distintos com limites
distintos.

### Estados

Nó: `pending ▸ ready ▸ running ▸ (waiting_approval) ▸ completed | failed | cancelled`, mais `blocked` (uma dependência
não pode mais ser satisfeita) e `skipped` (as condições o contornaram). Execução: `running`, `paused`, `completed`,
`failed`, `cancelled`, `interrupted`. Uma execução termina quando nada está em andamento e nada está pronto; é
`completed` somente se um nó End que significa sucesso foi alcançado e nada falhou sem tratamento. Um passo cujo
resultado não casa com nenhuma de suas rotas faz a execução falhar com `no_route_matched`: **nunca se adivinha, e nunca
é lido como aprovado.**

### Falha, retry, pausa, cancelamento, aprovação

- **Falha.** Um passo que falha é repetido até `retry_policy.max_retries` (≤ 5), cada tentativa com seu próprio id de
  execução e as que falharam mantidas. Depois vale a sua `failure_policy`: `stop_workflow` (os dependentes ficam
  `blocked`; a execução falha quando nada mais pode rodar) ou `route_to_node`. A execução não falha enquanto existir uma
  rota de falha.
- **Pausa** impede _iniciar_ passos. Um passo já em execução é deixado terminar (e seu resultado é mantido); nada novo
  começa até retomar.
- **Cancelamento** cancela o que não começou, pede aos processos dos passos em execução que terminem (o mesmo controle
  que o usuário tem no terminal, pelo `SessionRegistry`), mantém tudo o que concluiu e encerra a execução como
  `cancelled`.
- **Aprovação.** Um passo fica `waiting_approval` somente quando o mecanismo _real_ de aprovação diz que sua execução
  tem uma aprovação pendente (`ApprovalBroker::pending`). O workflow não oferece botão de aprovar e não lê nada que um
  agente diga como uma aprovação: o usuário responde no painel de segurança existente. Os dependentes ficam `pending`; o
  passo volta a `running` quando a aprovação é respondida.
- **Concorrência.** No máximo `max_parallel_steps` (4) rodam ao mesmo tempo; um agente nunca roda dois passos ao mesmo
  tempo (usa-se o lock de ocupado por agente e por workspace já existente: um segundo passo de um agente ocupado
  simplesmente espera). Os nós prontos são ordenados por prioridade, depois ordem de criação, depois id, de modo que o
  mesmo histórico dá o mesmo agendamento.

## Contexto, handoff e resultados

- **O contexto da tarefa é reaproveitado, não duplicado.** Para um passo, a instrução entregue ao `ExecutionService` é:
  as instruções do nó, a tarefa geral, um bloco **WORKFLOW CONTEXT** (workflow, passo e passada atuais, passos
  anteriores, artefatos, decisões, os relatórios com falha sobre os quais o passo deve agir, próximos passos) e um
  **RESULT PROTOCOL**. Aquilo para o qual o Task Context é _selecionado_ é um `context_query` separado — a tarefa mais o
  rótulo e as instruções deste passo — de modo que um passo de Backend e um de Frontend da mesma tarefa recebem partes
  diferentes do Harness. O texto do Harness em si é acrescentado pela camada de execução como sempre; o workflow nunca
  acrescenta uma cópia própria.
- **O handoff é por artefato, não por transcrição.** Um passo posterior não recebe o texto dos agentes anteriores. Ele
  recebe artefatos (nome, caminho, resumo), decisões e as últimas entradas de validação com falha (severidade,
  categoria, evidência, recomendação). Com tamanho limitado.
- **Resultado estruturado.** Um passo pode terminar sua resposta com um bloco JSON `atlas-result` (status, resumo,
  artefatos, decisões, achados, arquivos/áreas tocados, próxima ação). Sem ele o Atlas guarda um resumo textual e o
  status é `unknown`, que nenhuma aresta `pass`/`fail` casa. Os caminhos em um resultado devem ser relativos e não podem
  sair do projeto; qualquer outro é descartado.
- **Sobreposição.** Arquivos e áreas tocados são registrados por nó. Dois passos que poderiam rodar lado a lado (nenhum
  leva ao outro) tocando o mesmo lugar geram `workflow:overlap_detected` e um aviso no estado compartilhado. Detectada e
  mostrada; nunca resolvida.

## Autoridade — o que um workflow não pode fazer

Um workflow **não** ganha nenhuma autoridade nova. Ele não pode ler nem escrever arquivos, executar um shell, abrir a
rede, aprovar nada nem mudar uma política. A cadeia continua a mesma:

```text
Workflow ─▶ Orchestrator ─▶ ExecutionService ─▶ Runtime ─▶ PermissionPolicy ─▶ ProcessRunner      (nunca: Workflow ─▶ Process)
```

- Cada passo é uma execução como qualquer outra por meio do `ChatService` (`send_workflow_step`): a política de worktree
  do agente (o isolamento continua sendo do agente, ligado por padrão; um nó só pode herdá-lo, nunca enfraquecê-lo), o
  Harness, o registro de runtimes e o guard se aplicam. Se o agente precisa de um worktree e o projeto não é um
  repositório Git, o passo **falha**; nunca recorre ao checkout.
- O engine não conhece Claude, OpenCode, PTYs nem `ProcessRunner`; ele enxerga a porta `StepRunner`.
- **O que um agente diz nunca muda o workflow.** `next_action` é informação. O roteamento é decidido apenas pelas
  condições que o workflow define, avaliadas sobre o conjunto fechado de fatos acima; um resultado não pode adicionar um
  nó, editar uma aresta, conceder ou negar uma aprovação, nem tocar numa política. (Há um teste com um bloco de resultado
  que tenta tudo isso.)
- Nenhum comando Tauri recebe um programa, um caminho ou uma aprovação; o `build.rs` e o arquivo de capabilities listam
  cada comando de workflow.

## Versões, snapshots, edição, recuperação

- Uma definição tem uma `version`, incrementada por qualquer mudança **estrutural** (nós, arestas, políticas); mover
  nós, a viewport e o nome não contam. Uma execução registra a versão e guarda um **snapshot** completo: editar a
  definição nunca muda uma execução em andamento nem uma terminada.
- Enquanto um workflow tem uma execução em andamento (running, paused ou interrupted), sua definição não pode ser
  editada nem excluída; a UI diz isso e oferece esperar ou salvar uma cópia. Uma execução por vez por workflow.
- Se o app fecha durante uma execução, nada é retomado sozinho e nada é chamado de terminado: na próxima inicialização a
  execução fica `interrupted` (recuperável), suas tentativas em andamento são registradas como `interrupted`, e o
  usuário pode **retomar** (os passos interrompidos começam de novo como novas tentativas da mesma passada),
  **reiniciar** (cancelar e executar de novo), **cancelar** ou inspecioná-la.

## Automático e Personalizado

Os dois usam o mesmo modelo `Workflow`; só muda o autor. O **Automático** escolhe um template com regras fixas
(`select_for_task`: palavras de refactor / review / bug, senão feature), monta o workflow a partir dos agentes do
workspace por personalidade e o mostra. **Personalizar** muda `mode` para `custom`; o grafo passa então a ser do
usuário. Um template nunca cria um agente: papéis sem agente são informados e deixados vazios para o usuário preencher.
Os templates ficam em `application/workflow/templates.rs`; um futuro planejador produziria um `Workflow` do mesmo jeito
e o entregaria ao mesmo engine (depois de aprovação humana). **Não implementado aqui:** nenhum planejador LLM, nenhum
roteamento adaptativo, nenhum chat entre agentes.

## Papéis, como as personalidades os dizem

- **Architect** — o que deve ser construído, e como?
- **Developer** — como eu implemento isto?
- **Architecture & Code Validator** (somente leitura) — foi construído _corretamente_, dentro da arquitetura, das
  decisões, do Harness e das convenções?
- **QA** — _funciona_?
- **Bug Fixer** — por que falhou, e como corrijo (causa raiz, mudança mínima, evidência para revalidação)?

## Eventos

Um evento Tauri por tipo, sob `workflow:*`: `started`, `paused`, `resumed`, `completed`, `failed`, `cancelled`,
`interrupted`, `node_ready`, `node_started`, `node_waiting_approval`, `node_approval_resolved`, `node_completed`,
`node_failed`, `node_blocked`, `node_skipped`, `node_retrying`, `artifact_created`, `decision_created`,
`overlap_detected`. Eles carregam ids, não estado: a UI relê a execução. O progresso e as mensagens dos próprios passos
usam os eventos `execution:progress` e `conversation:message` existentes, então os cards dos agentes continuam vivos. O
log de eventos de uma execução é guardado dentro dela (os 400 mais novos).

## Limites conhecidos

- ~~Cada passo isolado tinha seu próprio worktree~~ e ~~os arquivos tocados eram o que o agente informava~~: os dois
  foram alterados pela [ADR 0014](0014-handoff-shared-worktree-and-code-integration.md) (um worktree compartilhado por
  execução; arquivos medidos pelo Git).
- Um passo com um processo pode ser parado no cancelamento; um runtime sem sessão de terminal termina por conta própria.
- `max_parallel_steps` é fixo em 4 (guardado por execução; ainda não configurável).
