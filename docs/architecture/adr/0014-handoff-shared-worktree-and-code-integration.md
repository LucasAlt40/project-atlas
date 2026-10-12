# ADR 0014 — Handoff, o worktree compartilhado da execução e a integração do código

**Status:** aceito. Estende a [ADR 0013](0013-workflow-engine-and-orchestrator.md) e a
[ADR 0009](0009-git-worktree-isolation.md).

## Por quê

A V0.9 executava passos, roteava seus resultados e dizia "concluído". Faltavam três coisas, e a terceira era pior do que
parecia:

1. **A comunicação entre agentes não podia ser vista.** O que um passo produzia para o próximo estava espalhado entre
   estado e prompts, sem lugar onde inspecionar.
2. **Os passos não compartilhavam código.** Cada passo isolado recebia seu próprio worktree a partir do commit base,
   então um Validator abria uma árvore sem o que o Developer tinha escrito.
3. **"Workflow concluído" era lido como "código entregue"** — e nem podia ser verdade: _nenhum agente do Atlas conseguia
   escrever um arquivo._ Por projeto, desde a V0.3 o runtime do Claude rodava com `--tools Read,Grep,Glob`, o OpenCode com
   `--agent plan` (edição negada) e o prompt dizia a todo agente que ele estava em "modo somente leitura". Um passo de
   Developer concluía sem ter mudado nada.

## Handoff

`AgentHandoff` (`domain/workflow/handoff.rs`) é o que um passo entrega ao próximo **por uma transição**: a aresta (ou
`failure:<nó>`), a execução e a passada que enviam, o `status` e o `summary` do passo, suas `decisions`, os `artifacts`
que produziu, os arquivos que o **Git** viu ele mudar (`changed_files`) e os ainda não commitados, o que ele apenas
_alegou_ ter tocado (`reported_files`, mantido à parte), a `validation` que deu (status, achados) e, para uma rota de
falha, a falha. O que um passo não disse fica vazio e seu status fica `unknown`; nada é inventado.

- Construído pelo orquestrador (`orchestration/handoff.rs::build_handoff`) a partir do `AgentResult` do passo e do Git,
  logo depois que o engine roteia (`WorkflowEngine::taken_links`: as arestas cujas condições o resultado atendeu, ou a
  rota de falha).
- Guardado em `WorkflowExecution.handoffs` (mesmo `config.json`, mesmas gravações atômicas; sobrevive a uma
  reinicialização junto com a execução).
- **Entregue ao próximo passo**, no brief (`orchestration/brief.rs::handoff_block`), entre `## WORKFLOW HANDOFF` e
  `END WORKFLOW HANDOFF`, com uma frase dizendo que é contexto e não concede nada. Para cada passo anterior só o handoff
  mais recente ao longo da sua transição é enviado. O texto dentro é _citado_: nenhuma linha começa na margem esquerda e
  os marcadores do próprio bloco são neutralizados, então ele não pode fechar o bloco nem se passar pela estrutura do
  prompt. Um handoff não muda mais nada: não tem caminho até permissões, política, aprovações, runtime ou a definição do
  workflow (os testes tentam cada um).
- Não é um chat: o orquestrador continua sendo o único dono do estado.
- UI: nada no canvas. Clique em uma **conexão** para ler o que trafegou por ela; um nó tem as dobras _Entrada_ e
  _Saída_; a execução tem uma aba _Handoffs_.

## O worktree compartilhado da execução

Quando um workflow tem agentes que trabalham em um worktree isolado, a execução recebe **um** worktree para todos eles,
criado pelo `WorktreeService::prepare` existente e marcado como da execução (`workflow_execution_id`). Cada passo recebe
um **lease** (`WorktreeService::attach`): um registro com o id de execução do próprio passo, a mesma pasta e o mesmo
branch, com `shared_with` apontando para o primário. Os leases existem porque a camada de segurança encontra a pasta de
uma execução isolada pelo seu id de execução.

- Um passo, ao terminar, commita o que deixou (`finish_step`), de modo que o próximo passo comece de uma árvore limpa e o
  trabalho seja recuperável; o lease registra o commit em que terminou, e `step_delta` é o diff desse intervalo, pelo
  Git.
- Um lease nunca é mesclado nem removido sozinho. O primário é decidido uma vez, quando a execução termina
  (`close_shared`), e **nunca é mesclado automaticamente**, seja qual for a política: o usuário decide o que entra no
  projeto.
- Passos cujo agente não isola trabalham no checkout como sempre, e a execução então não tem código a integrar
  (`not_applicable`). Se nenhum worktree puder ser criado (não é um repositório Git), os passos encontram isso por conta
  própria, como antes.
- Uma execução interrompida por uma reinicialização mantém seu worktree (registrada como interrompida, nunca mesclada);
  retomá-la **reabre** o mesmo (`reopen`), então nenhum segundo worktree aparece; cancelá-la a encerra como todo
  cancelamento.

## ChangeSet

`ChangeSet` (`domain/worktree.rs`) é medido pelo Git e nunca por um agente: revisão base e atual, arquivos com status
(adicionado / modificado / excluído / renomeado, com o caminho antigo), linhas adicionadas e removidas, arquivos
binários e caminhos não commitados. O texto do diff não é guardado; é lido do Git quando pedido (`diff_between`,
limitado). As revisões devem ser ids de commit completos e o nome de um arquivo deve ser um caminho relativo simples
(sem `..`, sem `-` ou `:` no início, sem globs): nada que um chamador envia vira uma opção ou mágica de pathspec. Depois
que as mudanças são aplicadas e o worktree some, o diff é lido do próprio histórico do projeto.

## Workflow concluído não é código integrado

`WorkflowExecution.integration` é separado de `status`:

| Status              | Significado                                                                                                                                                                    |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `not_applicable`    | sem worktree isolado: nada a integrar                                                                                                                                          |
| `in_progress`       | os agentes trabalham no worktree; nada está no projeto                                                                                                                         |
| `no_changes`        | a execução não mudou código (o worktree vazio é removido)                                                                                                                      |
| `changes_available` | o código está no worktree, **não no projeto**; `can_apply` diz se pode ser aplicado (o trabalho de uma execução cancelada ou com falha é mantido para revisão, nunca aplicado) |
| `conflicts`         | tentou-se aplicar: um arquivo também foi alterado no projeto (ou não se aplica); nada foi escrito                                                                              |
| `blocked`           | aplicar não é possível agora (`block_reason`: branch mudou, política nega, não commitado…)                                                                                     |
| `integrated`        | **o único estado em que a tela diz que o código está no projeto** (sem commit, na árvore de trabalho)                                                                          |
| `kept_isolated`     | o usuário escolheu deixá-lo no worktree                                                                                                                                        |
| `discarded`         | o usuário descartou o worktree                                                                                                                                                 |
| `failed`            | aplicar falhou por um motivo informado pelo Git                                                                                                                                |

`WorkflowExecution.status` nunca muda por causa de nada disso: uma execução que concluiu com uma integração bloqueada ou
com falha continua sendo uma execução concluída.

**As decisões do usuário** (`IntegrationService`, alcançado somente pelos comandos que o usuário aciona; o
orquestrador, o runner e os agentes não têm caminho até ele): **aplicar** (coloca as mudanças na _árvore de trabalho_ do
projeto, sem commit; nunca um merge nem um commit: veja a
[ADR 0015](0015-result-contracts-outcomes-and-worktree-lock.md#workflow-apply-não-é-um-commit); a política de escritas no
Git do agente ainda se aplica, e é a do agente _mais restrito_ da execução), **manter isolado**, **descartar**
(confirmado na UI; remove a pasta do worktree, sem forçar; o _branch é mantido_ porque o Atlas nunca apaga commits que
não foram mesclados, então o trabalho continua recuperável com o Git), **revisar** (o diff real) e **abrir em um
editor**.

## Abrir um editor

`IdeLauncher` (porta) / `SystemIdeLauncher`: uma lista fixa de editores (VS Code, Cursor, IntelliJ IDEA), encontrados
pelo seu próprio lançador de linha de comando no `PATH` (Windows: nomes `.cmd`/`.exe`) ou, no macOS, como um aplicativo
instalado (`open -a`). O programa é escolhido da lista, a pasta é um argumento de um processo iniciado sem shell, e deve
ser um caminho absoluto para uma pasta existente. A pasta nunca vem da webview nem de um agente: é o worktree da
execução enquanto o código está só lá, o projeto depois de aplicado. O Visual Studio ainda não é detectado.

## Deixar os agentes escreverem arquivos

A causa raiz acima é corrigida de forma estreita, dentro do modelo de segurança existente:

- `RuntimeCapabilities.file_edit` diz que um runtime _pode_ ser iniciado com ferramentas de edição de arquivos (Claude:
  sim; OpenCode: não, ainda `--agent plan`). `RuntimeRequest.allow_edits` diz que esta execução _recebe_ essas
  ferramentas.
- `ExecutionService::may_edit` só as concede quando **todas** as condições valem: o agente trabalha em um worktree
  isolado (uma edição nunca pode alcançar o checkout do projeto), o runtime pode, e a política do próprio agente (teto
  do workspace ∩ perfil de permissão) permite escrita de arquivos. Desconhecido significa não. Os agentes partem do
  perfil mais restritivo, então só um agente a quem o usuário deu explicitamente o perfil _developer_ pode escrever.
- O Claude é então iniciado com `--tools Read,Grep,Glob,Edit,Write --permission-mode acceptEdits`. Ainda sem shell, sem
  web. O `runtime_access` do escopo reflete o que as ferramentas podem fazer _como foram iniciadas_, o que o guard
  confere de novo contra a política.
- O prompt diz o que é verdade: a regra de edição para uma execução assim (worktree isolado, só arquivos, nada fora
  dele, revisado antes de chegar ao projeto), a regra de somente leitura para todas as outras.

## Limites conhecidos

- Passos de uma mesma execução que rodam em paralelo compartilham uma árvore: seus commits podem se misturar e o delta
  por passo pode incluir arquivos do outro passo. Os avisos de sobreposição continuam valendo.
- Só o usuário aplica mudanças; não há integração automática, mesmo para uma política que permitiria um merge.
- Os agentes do OpenCode continuam somente leitura. Agentes não podem executar testes nem um shell (uma decisão
  diferente e maior).
- As mudanças do passo são commitadas no branch da execução pelo Atlas quando as escritas no Git não são negadas; com uma
  política que as nega o trabalho fica sem commit e não pode ser aplicado a partir do Atlas.
