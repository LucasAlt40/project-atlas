# 0019 — Revisão e Entrega: ler um workflow concluído e decidir sobre o seu código

## Contexto

Um workflow termina com seu código em um worktree isolado ([ADR 0014](0014-handoff-shared-worktree-and-code-integration.md),
[0015](0015-result-contracts-outcomes-and-worktree-lock.md)). O Live Workspace ([0018](0018-live-workspace.md)) mostra esse
código enquanto ele é escrito e diz REVIEW quando a execução termina, mas o usuário ainda não tinha um lugar único para
entender _o que os agentes fizeram, o que encontraram e decidiram, o que mudou e o que o Apply vai fazer_ antes de
decidir.

## Decisão

**Só frontend.** Nenhum backend, Apply, `merge_execution`, orquestrador, observador ou contrato mudou. O Review Workspace
compõe o que a execução já guarda: nós e tentativas (linha do tempo), `state.validationResults` (achados),
`state.decisions` / `state.artifacts`, `handoffs`, `changes` (change set), `integration` e o snapshot do Live Workspace.

**Uma superfície, LIVE → REVIEW.** Para uma execução que acabou (`completed`, `failed`, `cancelled`) a página renderiza o
`ReviewWorkspace` em vez do painel ao vivo simples; ele embute o mesmo painel (`LiveWorkspaceView`, alimentado por um
único `useLiveWorkspace`) como visualizador de código, então o worktree é observado e lido uma vez só e o diff nunca é
calculado duas vezes. Enquanto a execução trabalha, nada muda.

**Separados, no modelo e na tela.** Status de execução (`attempt.status`), desfecho (`attempt.outcome`), status do
workflow, change set e integração são campos distintos e palavras distintas. `reviewStateOf` deriva um status de
_revisão_ ao lado do status de _integração_; o status de revisão é derivado (nada sobre ele é persistido): `Pronto para
revisão` até o código ser aplicado, `Revisado` depois.

**"Integrado" nunca é mostrado.** `IntegrationStatus::Integrated` é apresentado como _Aplicado à árvore de trabalho —
sem commit_. "HEAD inalterado" só é dito enquanto o backend ainda diz que o apply pode ser desfeito (`canUndo`, isto é,
o `HEAD` está onde o Apply o deixou); caso contrário a tela diz que o Atlas não consegue mais saber.

**Os achados mantêm o histórico.** `state.validationResults` só recebe acréscimos, então um achado corrigido não é
apagado. Um achado é marcado como _validação posterior passou_ somente quando uma validação posterior do mesmo passo
passou (um fato derivado, redigido como tal). A história "achado → corretor → validador" é a linha do tempo desde a
primeira validação que não passou até o fim da execução.

**Declarado vs detectado.** Os handoffs já traziam as duas listas; a tela agora mostra os arquivos que o agente declarou
mas o Git não viu _e_ os arquivos que o Git viu e o agente não informou. O Git continua sendo a fonte da verdade.

**O Apply exige confirmação.** O botão Apply abre um diálogo que diz o que ele não fará (nenhum commit, nenhum push,
nenhum merge) e lista os conflitos da última tentativa; confirmar chama o `apply_changes`, inalterado. Um apply
bloqueado mostra os arquivos em conflito e orienta o usuário a resolvê-los no próprio projeto; o Atlas nunca
sobrescreve, faz merge ou stash.

**Worktree sumiu.** Quando o Live Workspace informa `missing`/`invalid`, a Revisão diz que o conteúdo ao vivo está
indisponível, continua mostrando o change set e o diff salvos, e oferece apenas o que ainda faz sentido (sem Apply, Keep
ou Open in IDE para um worktree que não existe; uma execução já aplicada mantém Open in IDE, que abre o projeto).

**Não adicionado.** Artefatos são mostrados, não abertos (sem novo acesso a arquivos). Nada de commit, push, PR, resumo
por IA ou nova persistência.

## Limites

- O status de revisão não é persistido: ao reabrir o app ele é derivado de novo a partir do status de integração.
- O runtime/modelo da linha do tempo vêm das execuções armazenadas; um passo cuja execução não está mais armazenada
  mostra apenas o seu agente.
- Nomes de severidade desconhecidos pela Revisão aparecem como _Info_ (a palavra do próprio agente é mantida na linha do
  achado).
