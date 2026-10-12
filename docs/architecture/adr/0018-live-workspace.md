# 0018 — Live Workspace: observar o worktree de uma execução de workflow enquanto ela trabalha

## Contexto

O código de uma execução de workflow vive em um único worktree isolado ([ADR 0014](0014-handoff-shared-worktree-and-code-integration.md)).
Até a execução terminar, o usuário via status, linha do tempo e handoffs, mas não o código sendo escrito; as mudanças só
eram medidas no final. Para acompanhar um agente sem abrir uma IDE, o Atlas precisa mostrar o que _realmente_ está nesse
worktree, conforme muda. Esta ADR cobre o backend; a UI é um marco separado.

## Decisão

**Objetivo.** Manter, para cada workflow em execução, um estado derivado de "os arquivos do worktree da execução como
estão agora, comparados com onde a execução começou", e avisar a UI quando ele muda. Ele existe sem nenhuma UI: um
comando pode lê-lo a qualquer momento.

**O alvo é o worktree isolado da execução, nunca o projeto.** A pasta é derivada pelo Atlas a partir do que ele guardou
para a execução (`WorktreeService::live_target`): o worktree _primário_ de um workflow (não um lease, não uma execução
comum), dentro da pasta de worktrees do Atlas, no branch da execução. Qualquer outra coisa é `NotFound` ou `Invalid`. O
checkout do próprio projeto não é observado nem escrito; um teste escreve nele e confere que o estado não se move.

**Fonte da verdade: o Git contra o `base_commit`.** Todo arquivo é comparado com o commit de que o workflow partiu
(`baselineRevision`), nunca com o `HEAD` atual do projeto e nunca com os commits dos próprios agentes (o trabalho salvo
de um passo move `currentRevision`, não as mudanças). O estado é lido pelo `WorktreeManager::working_changes`: mudanças
rastreadas (salvas ou não) e novos arquivos não rastreados, com linhas adicionadas/removidas. Nada vem do que um agente
diz ter feito.

**Observação: eventos do SO primeiro, polling como alternativa.** O `notify` (FSEvents, inotify, …) atrás da porta
`WorktreeWatcher` informa _onde_ algo aconteceu; ele não lê nada. Se a plataforma não consegue observar a pasta, o estado
diz `observation: polling` e o worktree é olhado em um cronograma que começa em 500 ms e dobra até 5 s enquanto nada
muda. Com os eventos funcionando, uma comparação completa ainda roda a cada 10 s (execução `running`) ou 30 s (nos outros
casos) como rede de segurança.

**Debounce / coalescência.** Um evento nunca significa "estado final". Os eventos se juntam em um lote que é lido quando
nenhum evento chegou por 150 ms, e no máximo 1 s depois do primeiro evento do lote, de modo que um escritor contínuo
ainda mostre progresso e um arquivo salvo várias vezes seja lido uma só vez. Eventos para `.git` e para caminhos fora do
worktree são descartados.

**Reconciliação incremental e completa.**

- _Incremental_: apenas os caminhos do lote (uma pasta representa o que há dentro dela) são comparados com a base e
  substituem o que o estado tinha para eles. Um caminho que deixa de diferir sai do estado.
- _Completa_: o worktree inteiro é comparado e substitui o estado. Acontece quando a observação começa (para que mudanças
  que já existem, como quando uma execução é retomada, sejam lidas), quando a execução entra em `waiting_for_input`,
  `idle` ou `cancelled`, quando termina, em um refresh explícito, no cronograma de segurança/polling, e sempre que o
  caminho incremental não pode ser confiável: mais de 200 caminhos em um lote, uma fila de eventos perdida ou estourada,
  o Git falhando em um caminho, ou um rename envolvido. Renames de conteúdo exato (uma exclusão mais um arquivo novo
  idêntico) só são pareados em uma leitura completa.

**Ciclo de vida.** `idle` (o worktree existe, nenhum passo trabalhando) → `running` (um passo trabalha nele) →
`waiting_for_input` (um passo parou para perguntar a uma pessoa) / `cancelled` (o passo foi cancelado) → de volta a
`running` com a próxima tentativa → `ended` (a execução concluiu) ou `stopped` (falhou ou foi cancelada). A fase só
define com que frequência o worktree é olhado quando nada é informado, e o que a UI diz; o watcher continua rodando
enquanto um passo espera, então uma edição feita durante a espera é vista. `ended` / `stopped` são finais: a execução é
lida uma última vez (antes de `close_shared`, enquanto o worktree certamente existe), a observação para, e esse estado
continua legível. O estado final é igual ao change set da execução (testado). Uma execução sendo retomada reinicia a
observação sobre o mesmo worktree e base, e `revision` continua.

**Isolamento do Git dos agentes.** O observador só lê: o Git roda com `--no-optional-locks`, não pega nenhum lock que as
operações de escrita do manager seguram, e nunca faz commit, stage, merge ou escrita no worktree. Um teste o executa
contra 40 escritas e commits de um "agente" sem uma falha.

**Eventos e estado: snapshot + delta.** `get_live_workspace(executionId)` devolve o snapshot (`LiveWorkspaceState`);
`refresh_live_workspace(executionId)` força uma comparação completa. As mudanças chegam como `live_workspace:changed`
(`LiveWorkspaceUpdate`): arquivos `changed`, caminhos `removed` e `full: true` quando `changed` é _todo_ arquivo.
`revision` é monotônica e conta mudanças de estado: um cliente com a revisão N aplica uma atualização com N+1; **qualquer
outro número (uma lacuna, uma repetição, uma mais antiga) significa "leia o snapshot de novo"**. As atualizações são de
melhor esforço, como todo outro evento. Os comandos recebem o id da execução (o mesmo `executionId` do
`get_workflow_changes`); a webview nunca nomeia um caminho.

**Segurança.** A única forma de uma pasta ser escolhida é o registro do worktree guardado. Os caminhos passados ao Git
são relativos, sem `..`, e passados com `--literal-pathspecs` depois de `--`; refs e commits são os que o Atlas
derivou. Sem shell. Os dois comandos só leem. O frontend não tem acesso ao sistema de arquivos; ele recebe apenas os
modelos acima.

**Quando o worktree desaparece.** Se a pasta sumiu (aplicada e removida, descartada, apagada à mão) o estado passa a
`availability: missing` (`invalid` se não for mais o que o Atlas criou, p. ex. seu branch foi trocado),
`observation: stopped`, e o observador termina. Os últimos arquivos conhecidos são mantidos: são a última coisa sabida.
Para uma execução cujo worktree não pode ser lido de jeito nenhum, o snapshot é montado a partir do registro (seu change
set guardado) com `missing`.

**Sem `activeFile`.** O Atlas não tem um sinal confiável de qual arquivo um agente está editando: os runtimes não o
informam, e "o arquivo que mudou por último" seria um palpite apresentado como fato. O estado não tem nenhum; a UI deve
dizer que o agente está trabalhando e mostrar os arquivos como realmente mudam.

**O estado fica em memória.** É derivado, então nada é persistido. O último estado de uma execução terminada é mantido
em memória durante a sessão.

## O visualizador (UI)

Dois comandos somente leitura servem o visualizador, ambos por `executionId` (a execução) e um caminho relativo, nunca
uma pasta: `get_live_file` (o arquivo como está agora) e `get_live_diff` (o worktree como está agora contra o
`baselineRevision`, de um arquivo ou de tudo, arquivos novos incluídos). Um caminho deve ser simples (relativo, sem `..`,
sem `.git`); um link é informado e nunca seguido, e um link em uma pasta do caminho é recusado. Um arquivo excluído é
informado como `deleted`, não como erro; um arquivo de texto é cortado em 1 MiB (`truncated`). Depois que o worktree
some (mudanças aplicadas) a tela lê o diff do change set salvo e não consegue ler arquivos.

A tela aplica as atualizações por `revision` (só a próxima; as mais antigas são descartadas; uma lacuna lê o snapshot de
novo) e relê o arquivo selecionado somente quando a entrada daquele arquivo foi anunciada como alterada. Para isso o
observador lembra o tamanho e a data de modificação de cada arquivo e anuncia um arquivo cujo texto mudou mesmo quando as
contagens de linhas não mudaram.

## Limites (atuais)

- No máximo 500 arquivos são listados (`MAX_CHANGESET_FILES`); além disso a lista é cortada.
- As contagens de linhas de arquivos não rastreados só são lidas para arquivos de texto de até 1 MiB; arquivos maiores
  são listados sem contagens.
- Renames só são reconhecidos em uma leitura completa e só para conteúdo idêntico.
- Um escritor que pausa por mais que a janela de silêncio pode ser lido pela metade; o próximo evento, ou a olhada de
  segurança, corrige.
- Conteúdos e diffs não fazem parte deste estado (só caminhos, status e contagens): são lidos sob demanda do Git por
  trabalho posterior.
- **Limitação conhecida: depois que o app reinicia, uma execução que já existe pode ser consultada (um snapshot é uma
  única olhada no seu worktree), mas a observação não é reativada automaticamente** até a execução ser retomada.
  Recuperação e persistência do Live Workspace são uma possível evolução futura.

## Consequências

- Uma nova dependência, `notify`, atrás de uma porta; os testes conduzem o observador com um watcher roteirizado ou o
  real.
- O runner informa ao observador a fase da execução; o orquestrador, o engine e o serviço de worktree não sabem que ele
  existe.
- Não toca no Apply ([ADR 0015](0015-result-contracts-outcomes-and-worktree-lock.md#workflow-apply-não-é-um-commit)) nem
  no `merge_execution`.
