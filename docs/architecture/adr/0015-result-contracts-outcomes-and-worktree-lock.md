# ADR 0015 — Contratos de resultado, desfechos e a trava do worktree compartilhado

**Status:** aceito. Estende a [ADR 0013](0013-workflow-engine-and-orchestrator.md) e a
[ADR 0014](0014-handoff-shared-worktree-and-code-integration.md).

## Por quê

O uso real da V0.9.1 mostrou quatro lacunas:

1. Um validador respondia em prosa ("a arquitetura está boa, mas…") e o workflow não conseguia distinguir uma aprovação
   de uma reprovação.
2. Aquilo com que um agente pode concluir estava implícito na personalidade, não algo que um agente (ou um editor de
   workflow) pudesse declarar.
3. Se um agente pode editar arquivos parecia uma propriedade da marca do runtime, em vez de uma capacidade mais uma
   permissão.
4. Dois passos que escrevem podiam compartilhar o worktree da execução ao mesmo tempo, misturando seus commits e
   change sets.

## Decisões

**Um contrato de resultado pertence ao agente.** `Agent.resultContract` é uma lista de desfechos (`id`, `label`,
`description`) com um `kind` que é só um preset para o editor (`general`, `validation`, `review`, `implementation`,
`custom`). O workflow nunca vê o kind nem a personalidade: apenas os ids dos desfechos. Uma personalidade pode carregar
um `suggestedContract`; ele é aplicado pelo editor quando um agente é criado e nunca se aplica sozinho. Agentes salvos
antes disto existir são `general` (nenhum desfecho exigido).

**Status da execução e desfecho são coisas diferentes.** `completed` + desfecho `fail` é um agente que fez o seu
trabalho e concluiu que o trabalho falhou; `failed` não tem desfecho. Cada tentativa guarda o seu próprio desfecho
(`NodeAttempt.outcome`), então uma nova tentativa nunca herda um. O desfecho chega às condições como o fato
`result.outcome` (`result.status` ainda funciona para arestas antigas, e um desfecho `pass`/`fail` o preenche quando o
bloco não deu nenhum).

**O desfecho é lido só de dados, e só um id declarado é mantido.** O prompt do passo carrega um OUTPUT CONTRACT listando
os desfechos permitidos; o parser de resultado lê o campo `outcome` do bloco `atlas-result` e o confere com o contrato.
Nunca se procura "pass" ou "fail" em prosa. Um contrato que exige um desfecho e não recebe nenhum válido termina o passo
como **falho** ("Nenhum desfecho válido…"): ele segue o caminho habitual de um passo falho (nova tentativa, rota de
falha, ou a execução falhando). Nada é jamais lido como aprovado, sucesso ou pass. Se um desfecho é válido mas nenhuma
aresta o aceita, a execução falha com `no_route_matched`, nomeando o desfecho. Não há fallback por IA.

**As rotas são conferidas antes da execução.** Uma aresta em `result.outcome` cujo valor o contrato do agente de origem
não declara (ou um agente que não declara nenhum) é o problema de validação `undeclared_outcome`, então o editor o
sinaliza ao vivo e a execução se recusa a iniciar. O editor de conexões só oferece os desfechos declarados. A ADR 0017
estende as verificações a desfechos sem rota, rotas `result.status` em um agente com contrato e rotas que não alcançam
um End.

**Os handoffs carregam a estrutura.** `AgentHandoff.outcome`, o status da execução e os achados (severidade, categoria,
título, arquivo, linha, descrição, evidência, recomendação) vão ao passo seguinte como contexto, claramente delimitado,
sem autoridade: não concedem nada, não mudam política nenhuma e não aprovam nada.

**Editar é uma capacidade, uma permissão e isolamento.** `RuntimeCapabilities.fileEdit` diz que um runtime pode ser
iniciado com ferramentas de edição de arquivos; `PermissionPolicy` diz se este agente pode escrever; a execução deve
estar isolada. Só os três juntos habilitam a edição (`ExecutionService::edit_access`). Os motivos são distintos
(`RuntimeCannotEdit` vs `PolicyDenied` vs `NotIsolated`) e a UI diz "Este runtime não suporta edição de arquivos" em
vez de "negado". Nenhum código nomeia um runtime: um runtime que ganhe a capacidade (e uma forma segura de iniciá-lo)
passa a editar sem mudança arquitetural. O `--permission-mode acceptEdits` do Claude continua sendo um detalhe de
implementação desse runtime; a autoridade é a política do Atlas.

**O worktree da execução tem uma trava.** O worktree é um recurso da execução. Um passo que escreveria (acesso efetivo
de edição, nunca um nome) precisa dele com exclusividade; passos somente leitura o compartilham. Um escritor que espera
não é ultrapassado por leitores posteriores. A trava vive no loop do orquestrador para aquela execução e é reconciliada
a cada rodada com os passos realmente em andamento, de modo que um passo que terminou de qualquer forma (concluído,
falho, cancelado, panic, interrompido por desligamento) não consegue mantê-la. Como um escritor fica sozinho, o
`revision_before → revision_after` do Git para um passo contém só as mudanças desse passo.

## Workflow Apply não é um commit

A ADR 0014 primeiro descreveu "apply" como o merge do worktree. Isso estava errado para o histórico do projeto, e é
substituído aqui:

```text
Workflow Apply
    -> materializa o ChangeSet da execução na árvore de trabalho principal do projeto
    -> SEM COMMIT (modificado / novo / excluído em `git status`; HEAD, refs e reflog intocados)
```

e **não**:

```text
Apply -> git merge -> commit (-> push)
```

- Os commits da execução (`Atlas: work of …`) vivem no branch da própria execução: histórico da execução, para
  rastreabilidade e recuperação. O projeto recebe o _conteúdo_ (`git diff <fork>..<branch>` aplicado com `git apply`),
  nunca esses commits.
- O Apply nunca faz commit, merge, rebase, cherry-pick, stage, push, nem muda um remoto ou a configuração Git do
  projeto. Commitar é do desenvolvedor.
- Preparar, validar, detectar conflitos, aplicar, verificar: o patch inteiro é conferido contra a árvore de trabalho
  antes de qualquer arquivo ser escrito (`git apply` escreve tudo ou nada); um snapshot do HEAD e dos caminhos alterados
  é tirado antes e comparado de novo logo antes de escrever e depois (o HEAD não pode ter se movido).
- O trabalho não commitado do próprio usuário é preservado. Um arquivo que tanto a execução quanto a árvore de trabalho
  alteraram (um arquivo não rastreado conta) é um **conflito**: nada é escrito, o worktree é mantido, e o usuário
  decide. Um checkout sujo sozinho não bloqueia mais o Apply. Um checkout que saiu do branch base ainda bloqueia.
- Depois de um apply bem-sucedido, a pasta do worktree é removida e seu **branch é mantido** como registro do que foi
  aplicado (`MergeStatus::Applied`, mostrado como `integrated`).
- Provado por testes com Git real: HEAD, reflog, refs, index, `MERGE_HEAD` e um remoto bare permanecem inalterados
  depois do Apply.

**"Manter isolado" depois do Apply retira as mudanças aplicadas.** O branch é mantido, então o mesmo patch (`git diff
<fork>..<branch>`) é aplicado ao contrário (`git apply -R`), e somente ele. Só é permitido enquanto o projeto está no
branch base no `HEAD` registrado quando as mudanças foram aplicadas (`appliedHead`): com um novo commit, as mudanças
podem fazer parte do histórico, e o Atlas nunca o reescreve. O patch invertido é conferido inteiro primeiro; um arquivo
aplicado que o usuário alterou depois é um conflito e nada é escrito. As outras mudanças do usuário permanecem. Depois,
a pasta do worktree é recriada a partir do branch (`git worktree add`, sem novo branch) e a execução fica
`kept_isolated`, podendo ser revisada ou aplicada de novo. Quando não é possível, as mudanças continuam aplicadas e a
execução diz o motivo (`canUndo`, `blockReason: conflict`, ou `message: project_moved`).

Depois de um Apply bem-sucedido, a tela abre o editor da pessoa (o último escolhido, senão o primeiro encontrado) no
projeto. Nenhum dos lançadores dos editores suportados consegue abrir a sua visão do Git, então as mudanças aparecem
nos indicadores de Git do próprio editor.

**`merge_execution` é uma operação diferente.** O merge da execução isolada de um _único agente_ (o comando
`merge_execution` e o merge automático guiado por política) é um `git merge --no-ff` de verdade e cria um commit de
merge no branch base. É o "merge" explícito do usuário, com regras próprias. **Não é alterado por esta decisão**; se ele
também deve virar um apply na árvore de trabalho é uma decisão arquitetural separada, ainda em aberto. Os dois caminhos
são escolhidos pelo tipo de worktree (`WorktreeService::delivers_to_working_tree`): o worktree de um workflow nunca pode
ser mesclado ao histórico, qualquer que seja o comando que o alcance.

## Fora de escopo

Os agentes continuam sem executar shell, testes ou comandos, e sem rede. Isso é um marco separado.
