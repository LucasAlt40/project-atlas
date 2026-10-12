# ADR 0009 — Isolamento por Git worktree

**Status:** aceito

## O que é

Um agente tem `worktreeIsolation` (ligado por padrão, inclusive para agentes salvos antes de ele existir). Quando está
ligado, **cada execução trabalha no seu próprio Git worktree e branch** e nunca no checkout do projeto:

```
Agente (worktreeIsolation)
 └─ Execução A ── worktree A (atlas/exec-000101)
 └─ Execução B ── worktree B (atlas/exec-000102)

ExecutionService → WorktreeService → WorktreeManager (porta) → git
                 → Runtime(workingDirectory) → ProcessSession → PTY
```

O runtime recebe um diretório de trabalho e mais nada: não há código de worktree no `ClaudeRuntime` nem no
`OpenCodeRuntime`. Com o isolamento desligado, o diretório de trabalho é o projeto, como antes.

## Peças

| Peça                                                  | Responsabilidade                                                                                                                                |
| ----------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `application/worktree` — `WorktreeManager` (porta)    | `is_git_repository`, `inspect_base`, `create`, `get_status`, `get_branch`, `get_diff`, `commit_all`, `assess`, `base_ready`, `merge`, `remove`. |
| `infrastructure/git_worktree.rs`                      | O único código que executa `git` para worktrees. Listas de argumentos, nunca shell; hooks desligados; com limite de tempo; um escritor por vez. |
| `application/worktree/layout.rs` — `WorktreeLayout`   | O único lugar em que um nome de branch ou um caminho é derivado de um id.                                                                       |
| `application/worktree/service.rs` — `WorktreeService` | Ciclo de vida: `prepare`, `finalize`, `merge`, `cleanup_pending`, `recover_interrupted`; as regras de decisão (`decide`); persistência.         |
| `domain/worktree.rs` — `ExecutionWorktree`            | O que é guardado: ids, `baseBranch`, `branchName`, `worktreePath`, `status`, `mergeStatus`, `blockReason`, `recommendation`, `changes` medidas. |
| `ExecutionScope.isolated` + `SecurityService`         | O guard prende uma execução isolada ao **seu worktree**, encontrado no registro guardado, somente enquanto a execução está ativa.               |
| `commands/worktree.rs`                                | `list_execution_worktrees`, `merge_execution(workspaceId, agentId, executionId)`.                                                               |

## Onde e como

- **Local:** `<app data>/worktrees/<id do workspace>/exec-NNNNNN`, branch `atlas/exec-NNNNNN`. Fora do projeto de
  propósito: nada é adicionado ao repositório do usuário nem às suas regras de ignore, o agente não alcança o checkout
  principal por uma pasta pai, e limpar é remover uma pasta. Os dois nomes vêm do número da execução (`exec-42` →
  `exec-000042`); ids que não sejam `exec-<dígitos>` são recusados, então nenhuma string fornecida por quem chama vira
  um branch ou um caminho.
- **Base:** o branch que estiver com checkout no projeto naquele momento (`main`, `master`, `release/1.0`…), registrado
  com o seu commit. Sem fetch, pull, rebase ou qualquer operação remota. HEAD destacado ou repositório vazio falham com
  um erro claro.
- **Não é um repositório Git:** a execução **falha** (`git_repository_required`); não há fallback para o checkout e o
  Atlas nunca executa `git init`. O usuário adiciona Git ao projeto ou desliga o isolamento para aquele agente.
- **Mudanças não commitadas no checkout:** nunca tocadas (sem reset, clean ou stash). Elas não estão no worktree (o
  agente parte do que está commitado), o que é registrado (`baseDirtyAtStart`) e mostrado. Elas também bloqueiam um merge
  naquele checkout até que o usuário cuide delas.
- **O prompt** nomeia o worktree como o caminho do projeto, não o checkout.

## Finalização

`finalize` roda quando o runtime retorna. Ele salva o trabalho com um commit no branch da própria execução (hooks
desligados, a menos que a política negue escritas no Git), mede com o Git (arquivos, commits à frente/atrás, um
`merge-tree` de teste que não toca nenhum checkout) e decide, nesta ordem (`decide`, uma só função):

1. worktree diferente do que o Atlas criou (outra pasta, branch trocado) → bloqueado, deixado em paz;
2. nada mudou → nada para fazer merge; o worktree vazio e o branch são removidos;
3. a execução falhou / foi cancelada / foi interrompida → bloqueado; **mantido**, nunca mesclado;
4. mudanças que não puderam ser commitadas → bloqueado;
5. validação falhou → bloqueado (o Atlas ainda não roda testes: a validação é `not_run`, o que não bloqueia);
6. o Git diz que há conflitos → `conflict`; branch e worktree mantidos; o checkout fica intocado;
7. a política nega escritas no Git → bloqueado;
8. checkout sujo / saiu do branch base → bloqueado (o usuário pode pedir de novo depois);
9. política `git.write = approval_required` → `pending` (recomendação: merge, ou revisar se a base mudou);
10. política `git.write = allowed` → merge agora (`--no-ff`), depois remove o worktree e apaga o branch **mesclado**.

A política é o `git.write` resolvido do próprio agente (máximo global ∩ workspace ∩ perfil). "O modelo disse para fazer
merge" não é autorização: o texto de um modelo nunca chega a este código. Para `approval_required` a autorização é o
usuário apertar o botão no Inspetor de Execução, que chama `merge_execution`; ele reconfere tudo e não sobrepõe uma
política que nega. Um conflito encontrado na hora do merge é desfeito com `git merge --abort` (só um merge iniciado
pelo Atlas tem um `MERGE_HEAD`).

## O que o Atlas nunca faz

reset, clean, stash, `--force` em qualquer coisa, apagar um branch que não esteja totalmente mesclado, remover um
worktree que o Git considera sujo, remover um caminho que não seja exatamente o que o layout deriva para aquela execução
(e que não seja um symlink), reutilizar um branch ou pasta, resolver conflitos, fetch/pull/push, executar os hooks de um
repositório, ou rodar o git por `sh -c` / `cmd /c` / `powershell`.

## Persistência

`UserConfig.worktrees` guarda só metadados (sem diff, sem saída), gravados quando o worktree é **criado** para que uma
queda não os perca. Na inicialização, execuções que nunca terminaram são marcadas como falha/bloqueadas e mantidas;
worktrees mesclados mas não removidos são tentados de novo; novos ids de execução continuam depois do maior armazenado
**ou** de qualquer worktree ainda no disco.

## Limites conhecidos

- O Git roda por `std::process`, não pelo guard: o guard é para o que um agente pede. O manager é o único lugar que faz
  isso e é restringido como acima. O guard ainda decide o que uma execução isolada pode fazer, com o worktree como seu
  projeto.
- As ferramentas próprias de um runtime não têm sandbox (ADR 0006): um agente que escreve em um caminho absoluto fora do
  seu worktree não é parado pelo Atlas. O prompt não informa mais o caminho do checkout.
- Submódulos, sparse checkouts e repositórios `git worktree` com layouts incomuns não são tratados especificamente.
- Os caminhos de Windows e Linux seguem o mesmo código, mas ainda não foram validados.
- Um worktree por execução é um checkout completo; um chat somente leitura isolado por padrão paga esse custo (um
  worktree sem mudanças é removido logo em seguida).
