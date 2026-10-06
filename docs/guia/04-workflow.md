# 4. Workflow: engine, orquestrador, handoff, contratos e entrega

> Parte do [guia do Atlas](README.md). ADRs: [0013](../architecture/adr/0013-workflow-engine-and-orchestrator.md),
> [0014](../architecture/adr/0014-handoff-shared-worktree-and-code-integration.md),
> [0015](../architecture/adr/0015-result-contracts-outcomes-and-worktree-lock.md),
> [0016](../architecture/adr/0016-human-in-the-loop.md),
> [0017](../architecture/adr/0017-contract-aware-workflow-validation-and-route-repair.md),
> [0018](../architecture/adr/0018-live-workspace.md), [0019](../architecture/adr/0019-review-and-delivery.md).

Até a V0.8 a unidade de trabalho era _um agente, uma tarefa_. Um **workflow** transforma o processo (projetar →
implementar → validar → corrigir → validar de novo) em algo **visível, editável e determinístico**, **sem** dar a nada
novo o poder de rodar processos:

```text
Tarefa ─▶ Workflow ─▶ Workflow Engine ─▶ Orquestrador ─▶ ExecutionService ─▶ RuntimeRegistry ─▶ ModelRuntime
                                                                   └─▶ PermissionPolicy ─▶ ProcessRunner  (nunca: Workflow ─▶ Processo)
```

## Quem faz o quê

| Parte                | É                                                        | Não sabe nada sobre                         |
| -------------------- | -------------------------------------------------------- | ------------------------------------------- |
| **Workflow**         | O grafo declarativo (nós, arestas, condições, políticas) | como é desenhado nem como roda              |
| **Workflow Engine**  | Máquina de estados **pura** sobre um run                 | relógio, threads, storage, runtimes         |
| **Orquestrador**     | Conduz um run: inicia passos, ouve o fim, persiste       | o que o grafo significa (pergunta à engine) |
| **ExecutionService** | Uma execução de agente (inalterada)                      | workflows                                   |
| **Runtime**          | Fala com Claude/OpenCode/… (inalterado)                  | workflows                                   |
| **Graph UI**         | Uma forma de desenhar/editar (`@xyflow/react`)           | a engine; o core nunca lê o que ela guarda  |

- **Workflow ≠ Execution.** Definição (editada, versionada) × execução (um run de agente). Um _run_ é uma instância do
  workflow e possui muitas execuções. Listas separadas (`workflows`, `workflow_executions`) no mesmo `config.json`.
- **O orquestrador não é um agente.** É código determinístico. Nada que um agente escreve chega ao grafo.
- **Não há comunicação agente↔agente.** Passos deixam artefatos, decisões e resultados num estado compartilhado
  estruturado e o orquestrador entrega a cada passo o que ele precisa (uma conversa entre agentes seria ilimitada, cara,
  irreprodutível e tornaria "quem decidiu isto?" irrespondível).

Código: `domain/workflow`, `domain/orchestration` (modelo) · `application/workflow/{graph,validation,engine,orchestrator,
chat_runner,runner,service,templates,repair,integration,worktree_lock}.rs` · `application/orchestration/{result_parser,
handoff,brief}.rs` · `commands/workflow.rs` (`workflow:*`) · `src/features/workflow`.

## Semântica do grafo

- **Nó**: `agent` (guarda só `agent_id`, suas instruções, política de retry/falha/loop — nunca personalidade, runtime ou
  modelo), `condition` (testa o resultado anterior) ou `end` (terminal: done / failed / cancelled).
- **Aresta**: transição tomada quando a origem conclui **e** a condição opcional vale. Uma **rota de falha**
  (`FailurePolicy::RouteToNode`) também é transição.
- **Condição**: `{ field, operator, value }` sobre um conjunto **fechado** de campos — `result.status`, `result.outcome`,
  `result.summary`, `result.next_action`, `result.findings`, `result.matched`, `validation.status` — e quatro
  operadores (`equals`, `not_equals`, `exists`, `not_exists`). Sem expressões, sem código.
- **Artefato**: saída compartilhável de um passo (tipo, nome, produtor, **caminho relativo**, resumo); referencia o
  trabalho, não o copia. **Decisão**: conhecimento que os passos seguintes devem respeitar (título, decisão, justificativa).
- **Run**: guarda **snapshot** da versão do workflow de que partiu.

### Ativação, joins e loops

Um nó fica pronto quando é **ativado** pelos links disparados até ele. Links de fora de um ciclo são **requisitos**
(arestas incondicionais: todas devem ter disparado) ou **alternativas** (condicionais e rotas de falha: qualquer uma).
Links **dentro** de um ciclo são sempre alternativas — por isso um join espera dois ramos paralelos e
`QA → Bug Fixer → QA` não trava.

Loops são permitidos, mas **todo ciclo é limitado**: nós com `loop_policy { loop_id, max_iterations }`; a validação
remove-os e rejeita o grafo se sobrar ciclo. Estourar o limite falha o run com `max_iterations_reached`.

### Estados

- Nó: `pending ▸ ready ▸ running ▸ (waiting_approval | waiting_for_input) ▸ completed | failed | cancelled`; mais
  `blocked` (dependência não pode mais ser satisfeita) e `skipped` (condições o contornaram).
- Run: `running`, `paused`, `waiting_for_input`, `completed`, `failed`, `cancelled`, `interrupted`.
- Um run termina quando nada está em voo nem pronto; só é `completed` se um End de sucesso foi alcançado e nada falhou
  sem tratamento. **Resultado que não casa nenhuma rota falha o run com `no_route_matched` — nunca é adivinhado nem
  lido como "pass".**

### Falha, retry, pausa, cancelamento, aprovação, concorrência

- **Retry** até `retry_policy.max_retries` (≤ 5); cada tentativa tem seu `exec-id`. Depois, `failure_policy`:
  `stop_workflow` (dependentes `blocked`) ou `route_to_node`.
- **Pausa** só impede _iniciar_ passos; o que já roda termina.
- **Cancelar** cancela o não iniciado, pede aos processos dos passos em execução que terminem (mesmo controle do
  terminal, via `SessionRegistry`), mantém o que concluiu, e termina o run como `cancelled`.
- **Aprovação**: passo `waiting_approval` só quando o mecanismo **real** (`ApprovalBroker::pending`) diz que a execução
  tem aprovação pendente. O workflow não oferece botão de aprovar nem lê nada que um agente diga como aprovação.
- **Concorrência**: no máximo `max_parallel_steps` (4) simultâneos; um agente nunca roda dois passos ao mesmo tempo
  (lock de ocupado por agente/workspace). Ordem dos prontos: prioridade, criação, id → mesmo histórico = mesma ordem.

## Templates (modo Automático × Custom)

Ambos usam o **mesmo** modelo `Workflow`; só muda o autor. **Automático** escolhe um template por regras fixas
(`select_for_task`: palavras de refactor / review / bug, senão feature), monta o grafo com os agentes do workspace
**por personalidade** e o mostra. **Customize** muda `mode` para `custom`. Um template **nunca cria um agente**: papéis
sem agente são reportados e ficam vazios para o usuário preencher. **Não existe planejador por LLM**, nem roteamento
adaptativo, nem chat entre agentes.

| Template (id)                    | Papéis                                                | Categoria |
| -------------------------------- | ----------------------------------------------------- | --------- |
| `software_feature` (recomendado) | Architect, Developer, Validator, Bug Fixer, QA        | feature   |
| `feature_basic`                  | Architect, Developer, QA                              | feature   |
| `feature_with_validation`        | Architect, Developer, Validator, QA                   | feature   |
| `feature_with_bug_fix_loop`      | Architect, Developer, QA, Bug Fixer                   | feature   |
| `full_development`               | Architect, Backend, Frontend, Validator, QA + 2 loops | feature   |
| `bug_fix` (recomendado)          | Bug Fixer, QA                                         | bug       |
| `refactoring` (recomendado)      | Architect, Developer, Validator, Bug Fixer, QA        | refactor  |
| `code_review` (recomendado)      | Architecture & Code Validator (somente leitura)       | review    |

Papéis → personalidades: `architect`, `developer`, `architecture-validator`, `qa`, `bug-fixer`. Onde um template tem
um passo que **julga** (validator/QA), `Builder::gate` roteia no par declarado **pelo agente escolhido**:
`pass`/`fail` (preset `validation`) ou `approved`/`changes_requested` (preset `review`). Um agente sem nenhum dos dois
pares recebe `pass`/`fail` e a validação aponta o que falta.

## O que cada passo recebe (o brief)

A instrução entregue ao `ExecutionService` é montada por `orchestration/brief.rs`. **Passos não veem a transcrição uns
dos outros** — veem artefatos, decisões e os vereditos que precisam tratar. Estrutura, em ordem:

```text
Instructions for this step:        ← instruções do nó (se houver)
Overall task:                      ← a tarefa do run

WORKFLOW CONTEXT                   ← (≤ 4 500 chars)
  Workflow / Current step (pass N of at most M)
  Previous completed steps
  Artifacts from earlier steps     ← até 8: nome (caminho) do produtor: resumo
  Decisions to respect             ← até 10
  Reports to act on                ← até 3 vereditos não-pass, até 6 findings cada
  Your previous report on this task (se for repetição de um loop)
  Next steps (they are not yours to do)
  "Stay within this step's responsibility."

## WORKFLOW HANDOFF … END WORKFLOW HANDOFF     ← o que os passos anteriores entregaram (ver abaixo)

HUMAN INPUT                        ← só ao continuar após uma pergunta respondida
RESULT PROTOCOL                    ← como encerrar (bloco atlas-result)
OUTPUT CONTRACT                    ← se o agente declara outcomes: lista dos permitidos
ASKING THE PERSON                  ← como parar e perguntar (bloco atlas-interaction)
```

O Harness **não** é construído aqui: o brief entrega um `context_query` ao serviço de Task Context (via execução).
O workflow nunca acrescenta sua própria cópia do Harness.

### Handoff (como os passos "conversam")

`AgentHandoff` é o que um passo entrega ao próximo **sobre uma transição**: a aresta (ou `failure:<nó>`), a execução e
o _pass_ de origem, `status`, `outcome`, `summary`, `decisions`, `artifacts`, os arquivos que o **Git** viu mudar
(`changed_files`) e os não commitados, o que o agente apenas **alegou** ter tocado (`reported_files`, mantido à parte),
a `validation` (status + findings: severidade, categoria, título, arquivo, linha, descrição, evidência, recomendação) e,
numa rota de falha, a falha. O que um passo não disse fica vazio (status `unknown`); nada é inventado.

- Construído pelo orquestrador (`build_handoff`) logo depois que a engine roteia (`taken_links`), guardado em
  `WorkflowExecution.handoffs` (sobrevive a reinício).
- Entregue ao próximo passo entre `## WORKFLOW HANDOFF` e `END WORKFLOW HANDOFF`, com uma frase dizendo que é contexto,
  **não concede nada** e perde para as regras do Atlas e para a tarefa. Para cada passo anterior só o último handoff
  daquela transição é enviado.
- O texto interno é **citado** (`quote`): nenhuma linha começa na margem esquerda e os marcadores do bloco são
  neutralizados, então não dá para fechar o bloco nem imitar a estrutura do prompt.
- Na UI: clique numa **conexão** para ver o que trafegou; cada nó tem _Input_/_Output_; o run tem aba _Handoffs_.

## Formato das respostas dos agentes

### `atlas-result` (fim de todo passo de workflow)

O agente encerra a resposta final com **um bloco cercado** (o parser lê o **último** `atlas-result`):

````text
```atlas-result
{"status":"success","outcome":"<um dos permitidos>","summary":"…",
 "artifacts":[{"type":"implementation_summary","name":"…","path":"relative/path","summary":"…"}],
 "decisions":[{"title":"…","decision":"…","rationale":"…"}],
 "findings":[{"severity":"high","category":"…","title":"…","description":"…","file":"relative/path","line":1,"evidence":"…","recommendation":"…"}],
 "touchedFiles":["relative/path"],"touchedAreas":["area"],"nextAction":"…"}
```
````

- `status` ∈ `success | pass | fail | warning` (aceita sinônimos `passed/failed/ok/done`); fora disso, `unknown`.
- Caminhos **relativos**, que não saem do projeto; o resto é descartado.
- **Sem bloco**: o Atlas guarda um resumo textual e o status é `unknown` — que nenhuma aresta `pass`/`fail` casa.
- `outcome` só é lido do **campo `outcome`** do bloco e só é mantido se **declarado** no contrato do agente.
  **Prosa nunca é procurada** por "pass"/"fail". Não há fallback por IA.
- `nextAction` é **informação**; roteamento só ocorre pelas condições que o workflow define.

### Contrato de resultado (`Agent.resultContract`)

Lista de **outcomes** (`id`, `label`, `description`); `kind` é só um preset do editor:

| Preset           | Outcomes                                              |
| ---------------- | ----------------------------------------------------- |
| `general`        | nenhum (resultado é só resumo)                        |
| `validation`     | `pass`, `fail`                                        |
| `review`         | `approved`, `changes_requested`                       |
| `implementation` | `implemented`, `partial`, `blocked`                   |
| `custom`         | definido pelo usuário (máx. 12; ids `a-z0-9_-`, ≤ 40) |

- **Status de execução ≠ outcome.** `completed` + outcome `fail` = o agente fez o trabalho e concluiu que ele falhou;
  `failed` não tem outcome. Cada tentativa guarda o seu (`NodeAttempt.outcome`); retry não herda.
- `result.status` diz se o passo rodou; `result.outcome` é o veredito declarado. Só `pass`/`fail` são espelhados em
  `result.status` (compatibilidade com arestas antigas).
- Contrato que exige outcome e não recebe um válido → o passo termina **`failed`** ("No valid outcome…"), seguindo o
  caminho de qualquer falha (retry, rota de falha ou falha do run). **Nada é lido como pass/aprovado.**
- Outcome válido sem aresta que o aceite → run falha com `no_route_matched`, nomeando o outcome.

### Validação antes do run (`validation.rs`)

Só **erros** impedem o run de começar; avisos são listados à parte.

| Problema                   | Severidade | Quando                                                                                   |
| -------------------------- | ---------- | ---------------------------------------------------------------------------------------- |
| `undeclared_outcome`       | erro       | Aresta testa um outcome que o agente não declara                                         |
| `outcome_without_route`    | erro/aviso | O agente declara outcome sem aresta (erro se há outras arestas; aviso se não há nenhuma) |
| `status_route_on_contract` | erro       | Aresta `result.status = X` para agente com contrato, e `X` não é `pass`/`fail` espelhado |
| `cannot_reach_end`         | erro       | Aresta leva a parte do grafo de onde nenhum End é alcançável                             |
| (e as regras estruturais)  | erro       | loops limitados, agentes existentes, campos de condição conhecidos…                      |

> A regra "todo veredito declarado precisa levar a algum lugar" é uma decisão **semântica** do Atlas, hoje como erro,
> e está **pendente de confirmação** (ADR 0017); mudar é trocar a severidade em `check_outcome_routes`.

**Reparo assistido** (`repair.rs`): o Atlas **não infere** que `pass` significa `approved`. `suggest_route_repairs`
propõe (só pela forma do grafo), o usuário vê lado a lado, escolhe e confirma; `repair_workflow_routes` reconfere,
reescreve as arestas confirmadas para `result.outcome`, cria nova versão e acrescenta um `RouteRepair` em
`Workflow.route_repairs`. Abrir o app, validar e salvar **nunca** reparam sozinhos.

## Humano no loop

Um agente que para para perguntar a uma pessoa está **esperando**, não terminado.

| Nível    | Estado                                                                    |
| -------- | ------------------------------------------------------------------------- |
| Execução | `ExecutionStatus::WaitingForInput` (nem `completed`, nem `failed`)        |
| Nó       | `NodeStatus::WaitingForInput`; tentativa `AttemptStatus::WaitingForInput` |
| Run      | `WorkflowExecutionStatus::WaitingForInput`                                |
| Pergunta | `PendingInteraction`, guardada em `WorkflowExecution.interactions`        |

**Detecção** (só em passos de workflow; em conversa a pessoa simplesmente responde). A primeira que falar vence:

1. o adaptador do runtime (`ModelRuntime::detect_interaction`; nenhum adaptador atual implementa);
2. o bloco **`atlas-interaction`** que todo agente é instruído a usar quando não pode continuar:

````text
```atlas-interaction
{"type":"clarification","question":"…","context":"…","options":["…"]}
```
````

`type` ∈ `clarification | approval | permission`. É dado: campos fechados, recortados; os botões de
aprovar/permitir/sim-não são **do Atlas**, o agente não os inventa; 3. análise conservadora de texto de como a resposta **termina** (última frase é pergunta dirigida à pessoa, sem
`atlas-result`; ofertas de mais trabalho e `?` em código não contam). Confiança mínima **70**.

Nenhum modelo é consultado para classificar a saída. Um `atlas-result` depois de um bloco de interação = o agente concluiu.

**Pausa → resposta → retomada.** O orquestrador pausa o run (sem parse de resultado, sem handoff, sem rota);
`answer_workflow_interaction` valida a resposta (pendente, uma das opções, texto limitado), registra e devolve o nó a
`Ready` como **continuação do mesmo pass**. A próxima partida é nova tentativa cuja instrução traz um bloco
`HUMAN INPUT` (pergunta e resposta, citadas), porque runtimes one-shot não guardam conversa; os arquivos já escritos
continuam no worktree do run. Negar não é concluir: o agente é avisado e encerra sob o próprio contrato (`blocked`,
`fail`…). Responder **nunca** aplica código ao projeto nem muda política.

Auditoria: `interaction_detected`, `node_waiting_for_input`, `interaction_answered`, `node_input_resolved`,
`interaction_rejected`, `interaction_cancelled`. Ainda não há timeout (o modelo tem `expired`, nada o define).

## Código compartilhado: um worktree por run

Quando há agentes isolados, o run recebe **um único worktree** (feito por `WorktreeService::prepare` e marcado como do
run). Cada passo recebe um **lease** (`attach`): registro com o `exec-id` do passo, mesma pasta/branch, `shared_with`
apontando ao primário (o guard encontra a pasta de uma execução isolada pelo id da execução).

- Ao fim de um passo, o Atlas faz **commit** do que ficou (`finish_step`) — o próximo parte de árvore limpa e o trabalho
  é recuperável; `step_delta` é o diff daquele intervalo, **medido pelo Git**.
- Leases nunca são mergeados nem removidos sozinhos. O primário é decidido **uma vez**, quando o run termina
  (`close_shared`), e **nunca é mergeado automaticamente**, seja qual for a política.
- Passos sem isolamento trabalham no checkout como sempre e o run fica `not_applicable`. Sem repositório Git → o passo falha.
- Run cortado por reinício mantém o worktree (nunca mergeado); retomar **reabre o mesmo** (`reopen`).

**Lock do worktree** (`worktree_lock.rs`): passo que **escreveria** (acesso de edição efetivo, nunca pelo nome) exige o
worktree **exclusivamente**; leitores compartilham. Um escritor esperando não é ultrapassado por leitores novos. O lock é
reconciliado a cada rodada contra o que realmente está em voo — nenhum fim (concluído, falho, cancelado, pânico,
interrompido) o segura. Passo `waiting_for_input` **mantém** o lugar no lock. Como o escritor está sozinho,
`revision_before → revision_after` contém só as mudanças dele.

## Entrega do código (integração ≠ conclusão do workflow)

`WorkflowExecution.integration` é **separado** de `status`: um run que completou com integração bloqueada continua
`completed`.

| Status              | Significado                                                                                                                             |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| `not_applicable`    | sem worktree isolado: nada a integrar                                                                                                   |
| `in_progress`       | os agentes trabalham no worktree; nada está no projeto                                                                                  |
| `no_changes`        | o run não mudou código (worktree vazio removido)                                                                                        |
| `changes_available` | o código está no worktree, **não no projeto**; `can_apply` diz se pode aplicar (run cancelado/falho: guarda para revisão, nunca aplica) |
| `conflicts`         | tentou aplicar: arquivo também mudou no projeto; nada foi escrito                                                                       |
| `blocked`           | não dá para aplicar agora (`block_reason`: branch mudou, política nega, …)                                                              |
| `integrated`        | **único** estado em que a tela diz que o código está no projeto (_não commitado_, na árvore de trabalho)                                |
| `kept_isolated`     | usuário optou por deixar no worktree                                                                                                    |
| `discarded`         | usuário descartou o worktree (a **branch é mantida**)                                                                                   |
| `failed`            | aplicar falhou por motivo do Git                                                                                                        |

Decisões **só do usuário** (`IntegrationService`; orquestrador, runner e agentes não têm caminho até elas):

- **Apply** = materializa o `ChangeSet` do run na **árvore de trabalho** do projeto, **sem commitar**
  (`git diff <fork>..<branch>` aplicado com `git apply`; HEAD, refs e reflog intocados). **Nunca** faz commit, merge,
  rebase, cherry-pick, stage, push nem muda config do Git. O patch inteiro é validado antes de escrever (tudo ou nada);
  HEAD é comparado antes/depois. Trabalho não commitado do usuário é preservado; arquivo alterado pelos dois
  (inclusive untracked) = **conflito** e nada é escrito. Depois do apply, o worktree some e a branch fica como registro.
  Há testes com Git real provando que HEAD, reflog, refs, index, `MERGE_HEAD` e um remoto bare não mudam.
- **Keep isolated** (depois de apply): desfaz **só** o patch aplicado (`git apply -R`), apenas enquanto o projeto está na
  branch base no `HEAD` registrado (`appliedHead`); depois disso o Atlas não reescreve história.
- **Discard** (confirmado na UI): remove a pasta do worktree sem `--force`; a branch fica.
- **Review** (o diff real) e **abrir no editor** (`IdeLauncher`: VS Code, Cursor, IntelliJ IDEA; lista fixa, sem shell,
  pasta = o worktree enquanto o código só está lá, o projeto depois do apply — nunca vinda do webview ou de agente).

> `merge_execution` (merge de **uma** execução avulsa isolada) é outra operação: `git merge --no-ff` com commit de merge,
> sujeito à política `git.write`. Um worktree **de workflow** nunca pode ser mergeado na história, por qualquer comando
> (`WorktreeService::delivers_to_working_tree`).

## Live Workspace (ver o código sendo escrito)

Para cada run, o Atlas mantém um **estado derivado** de "os arquivos do worktree agora × onde o run começou".

- **Alvo**: o worktree **primário** do run, derivado do registro guardado — nunca o checkout do projeto, nunca uma
  pasta vinda do webview.
- **Fonte da verdade**: Git contra o `base_commit` (`baselineRevision`), via `WorktreeManager::working_changes`
  (rastreados salvos ou não + novos untracked, com linhas). Nada vem do que o agente _diz_ ter feito.
- **Observação**: eventos do SO (`notify`, atrás do port `WorktreeWatcher`) dizem _onde_; fallback para _polling_
  (500 ms dobrando até 5 s). Comparação completa de segurança a cada 10 s (run `running`) ou 30 s.
- **Debounce**: lote lido após 150 ms de silêncio, no máximo 1 s após o primeiro evento; eventos de `.git` e de fora do
  worktree são descartados.
- **Incremental × completo**: incremental só nos caminhos do lote; completo ao iniciar, em `waiting_for_input`/`idle`/
  `cancelled`, ao terminar, em refresh explícito, e sempre que o incremental não é confiável (> 200 caminhos, fila
  perdida, falha do Git, rename).
- **Ciclo de vida**: `idle → running → waiting_for_input | cancelled → running → ended | stopped`. Final = lido uma
  última vez antes de `close_shared`; o estado final é igual ao change set do run.
- **Somente leitura**: Git com `--no-optional-locks`, sem commit/stage/escrita. Caminhos relativos, sem `..`, com
  `--literal-pathspecs`.
- **Sem `activeFile`**: nenhum runtime informa qual arquivo o agente edita; adivinhar seria apresentar palpite como fato.
- **Snapshot + delta**: `get_live_workspace` (snapshot), `refresh_live_workspace`, evento `live_workspace:changed` com
  `revision` monotônica (N+1 aplica; qualquer outro número → ler o snapshot de novo). Visualizador:
  `get_live_file` (corte em 1 MiB) e `get_live_diff`; link simbólico é reportado, nunca seguido.
- Estado em memória (derivado). Limite: até 500 arquivos listados. Após reinício a observação não é reativada
  automaticamente até o run ser retomado.

## Review & Delivery (UI)

Só frontend (ADR 0019): para um run **terminado** a página mostra o `ReviewWorkspace` em vez do painel ao vivo,
embutindo o mesmo painel como visualizador de código (um só `useLiveWorkspace`). Mantém separados, no modelo e na tela:
status da execução, outcome, status do workflow, change set e integração; `reviewStateOf` deriva "Ready for review"
→ "Reviewed" (nada persistido). "Integrated" nunca é exibido: aparece como **"Applied to the working tree —
uncommitted"**; "HEAD unchanged" só enquanto `canUndo`. Findings têm histórico (append-only); só são marcados "later
validation passed" quando uma validação posterior do mesmo passo passou. Mostra _alegado × detectado_ (Git é a verdade).
**Apply exige confirmação** num diálogo que diz o que **não** fará (sem commit/push/merge) e lista conflitos.

## Recuperação e versões

- Definição tem `version`, incrementada por mudança **estrutural**; o run guarda um **snapshot** completo. Com run em
  andamento (running/paused/interrupted) a definição não pode ser editada/apagada; um run por workflow.
- Fechar o app durante um run: nada é retomado nem dado por terminado — no próximo início o run fica `interrupted`,
  tentativas em voo são `interrupted`, e o usuário escolhe **resume** (passos cortados recomeçam como novas tentativas do
  mesmo pass), **restart**, **cancel** ou inspeção.

## Eventos

Um evento Tauri por tipo, em `workflow:*` (`started`, `paused`, `resumed`, `completed`, `failed`, `cancelled`,
`interrupted`, `node_ready`, `node_started`, `node_waiting_approval`, `node_approval_resolved`, `node_completed`,
`node_failed`, `node_blocked`, `node_skipped`, `node_retrying`, `artifact_created`, `decision_created`,
`overlap_detected`, e os de interação). Carregam **ids, não estado**: a UI relê o run. O log de eventos de um run é
guardado dentro dele (os 400 mais novos). O progresso dos passos usa os eventos existentes (`execution:progress`,
`conversation:message`). **Sobreposição** (dois passos paralelos tocando o mesmo lugar) é detectada e mostrada, nunca resolvida.

## O que um workflow **não** pode fazer

Ler/escrever arquivos, rodar shell, abrir rede, aprovar algo ou mudar política. Cada passo é uma execução comum via
`ChatService::send_workflow_step` (isolamento do agente, Harness, registro de runtimes e guard valem). Nenhum comando
Tauri aceita programa, caminho ou aprovação vindos do webview. Há um teste com um resultado de agente que tenta tudo isso.
