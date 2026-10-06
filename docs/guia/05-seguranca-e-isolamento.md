# 5. Segurança, isolamento e controle de processos

> Parte do [guia do Atlas](README.md). ADRs: [0003](../architecture/adr/0003-command-permissions.md),
> [0007](../architecture/adr/0007-live-shell-and-process-control.md),
> [0009](../architecture/adr/0009-git-worktree-isolation.md).
> Nota: vários ADRs citam um "ADR 0006" (modelo de segurança/sandbox) que **não existe** em `docs/architecture/adr/`;
> o comportamento descrito abaixo vem do código (`application/security/`) e dos demais ADRs.

Princípio: **privilégio mínimo por padrão; texto de modelo nunca é autorização.** O Atlas é uma _fronteira de política_,
**não um sandbox de SO**: ele controla o que **inicia** e o que a política permite, mas não consegue impedir o que as
ferramentas internas de uma CLI fazem dentro do próprio processo (ex.: um agente que escreve em caminho absoluto fora do
worktree não é parado pelo Atlas — o prompt só deixa de revelar o caminho do checkout).

## 1. Fronteira webview ↔ core

- O webview não toca disco, processo, shell, Git, credenciais nem rede. Só chama comandos Tauri com allow-list
  (`build.rs` + `capabilities/default.json`); um teste Rust chama o comando pelo contexto gerado real, então remover a
  permissão quebra a CI.
- **Nenhum comando aceita programa, linha de comando, caminho de pasta de trabalho ou aprovação vindos do webview.**
  Controle de processo nomeia `workspaceId` + `agentId` + `executionId`; o core revalida os três.
- Segredos nunca vão em `config.json` (futuro: credential store do SO atrás de um port).

## 2. Política de permissões

`SecurityPolicy` = `filesystem` (escopo **somente o projeto**; escrita) · `processes` (modo + lista de comandos
permitidos sem perguntar) · `network` · `git` (`read`, `write`, `destructive`). Cada valor é um `Permission`:
`Denied < ApprovalRequired < Allowed`.

**Camadas — cada uma só pode _restringir_ a anterior** (`restrict` toma o menor valor de cada item; um comando só roda
sem perguntar se **todas** as listas o nomeiam):

```text
máximo global do Atlas  ∩  política do workspace  ∩  perfil de permissão do agente  ∩  capacidades das ferramentas do runtime
```

| Perfil      | Escrita de arquivos | Processos                                                             | Rede   | Git                                              |
| ----------- | ------------------- | --------------------------------------------------------------------- | ------ | ------------------------------------------------ |
| `read_only` | negada              | nenhum                                                                | negada | só leitura                                       |
| `developer` | permitida (projeto) | lista de dev: `git npm pnpm yarn cargo dotnet java mvn gradle python` | negada | leitura e escrita; destrutivo **pede aprovação** |

- **Máximo global**: rede e Git destrutivo **nunca** são automáticos (`ApprovalRequired` no máximo).
- Agentes sem perfil válido recebem o **mais restritivo** (`read_only`). Workspaces novos começam com teto `developer`.
- A política efetiva é resolvida pelo core a partir de ids; nunca do que o webview envia.

## 3. O guard de processos (`GuardedProcessRunner`)

**Único lugar onde um processo pode iniciar.** Para cada `ProcessSpec`:

1. resolve workspace, agente e política pelos ids do contexto da execução;
2. avalia o pedido (`evaluator`);
3. grava a decisão no **log de auditoria** (por execução);
4. se "exige aprovação", **estaciona** a execução até o **usuário** responder na UI (ou 5 minutos → rejeitado);
5. só então repassa programa+argumentos **inalterados** ao runner interno, via hook de sandbox (`NoSandbox` hoje).

Negado/não aprovado = o runner interno **nunca é chamado**. Regras que valem para qualquer processo:

- **Shells nunca rodam** (`sh bash zsh fish cmd powershell pwsh wsl osascript …`) e **launchers** (`env xargs sudo
nohup exec eval time timeout find …`) — rodar um shell transforma "executável + argumentos" em "qualquer string que o
  agente monte", anulando as outras regras.
- Variáveis de ambiente que reconfiguram o programa (`PATH`, `LD_*`, `DYLD_*`, `GIT_*`, `NODE_OPTIONS`, `PYTHON*`,
  `RUSTC_WRAPPER`, `JAVA_TOOL_OPTIONS`…) são negadas.
- Programa com caminho (`./npm`, `/tmp/npm`) não é confundido com a ferramenta do PATH.
- Interpretador com código na linha de comando (`python -c`, `node -e`…) **pergunta**, mesmo permitido.
- Ferramentas de rede (`curl wget ssh scp nc …`) seguem a política de rede. Git é classificado em
  leitura / escrita / destrutivo; **o que não é reconhecido é tratado como o mais perigoso**.
- O diretório de trabalho precisa estar dentro do projeto (checagem física, com symlinks resolvidos). Para execução
  **isolada**, "o projeto" é o **worktree** dela, achado no registro guardado, apenas enquanto a execução está ativa.
- Os próprios runtimes são lançados pelo guard (`evaluate_runtime_launch`); o guard anota quando as ferramentas do
  runtime excedem a política (`runtime_exceeds_policy`) — algo que o Atlas não consegue restringir.

**Aprovações** (`ApprovalBroker`): o único caminho para deixar um comando "que exige aprovação" rodar é o usuário
clicando no painel de segurança. Um workflow **não** oferece botão de aprovar e não lê nada que um agente diga como
aprovação.

## 4. Isolamento por Git worktree (ADR 0009)

Agente com `worktreeIsolation` (padrão **ligado**) executa em **worktree + branch próprios**, nunca no checkout.

- **Onde**: `<app data>/worktrees/<workspaceId>/exec-NNNNNN`, branch `atlas/exec-NNNNNN`. Fora do projeto de propósito
  (nada é adicionado ao repositório do usuário; o agente não alcança o checkout por pasta-pai; limpar = remover uma
  pasta). Nomes vêm **só** do número da execução (`exec-42` → `exec-000042`); ids que não são `exec-<dígitos>` são
  recusados — nenhuma string de quem chama vira branch ou caminho (`WorktreeLayout`).
- **Base**: a branch que estiver ativa no projeto no momento, com seu commit. Sem fetch/pull/rebase/remoto. HEAD
  destacado ou repositório vazio → erro claro.
- **Não é repositório Git → a execução falha** (`git_repository_required`). Não há fallback para o checkout e o Atlas
  **nunca roda `git init`**.
- **Mudanças não commitadas no checkout** nunca são tocadas (sem reset/clean/stash); não estão no worktree (o agente
  parte do que está commitado) — registrado em `baseDirtyAtStart`.
- O Harness **não** é copiado para o worktree (chega como texto do prompt).

Ao terminar, `finalize` commita o trabalho na branch da execução (hooks desligados, salvo política que nega escrita Git),
mede com o Git (arquivos, ahead/behind, `merge-tree` de teste que não toca checkout) e decide, **nesta ordem**
(`decide`, uma função): worktree que não é o que o Atlas fez → bloqueado; nada mudou → remove vazio; execução
falhou/cancelada → bloqueado e **mantido**; não commitável → bloqueado; validação falhou → bloqueado; conflito →
`conflict` (checkout intocado); política nega escrita Git → bloqueado; checkout sujo/saiu da base → bloqueado;
`git.write = approval_required` → `pending` (o usuário aperta o botão → `merge_execution`); `git.write = allowed` →
merge `--no-ff` e remove worktree + branch **mergeada**.

**O que o Atlas nunca faz**: reset, clean, stash, `--force`, apagar branch não totalmente mergeada, remover worktree que o
Git considera sujo, remover caminho diferente do derivado pelo layout (nem symlink), reutilizar branch/pasta, resolver
conflitos, fetch/pull/push, rodar hooks do repositório, rodar git via `sh -c`/`cmd /c`/`powershell`.

Git para worktrees roda por `std::process` (não pelo guard — o guard é para o que o _agente pede_): `infrastructure/
git_worktree.rs` é o único lugar, com listas de argumentos, hooks off, limite de tempo e **um escritor por vez**.

`UserConfig.worktrees` guarda só metadados, gravados na **criação** (um crash não perde o registro). Na inicialização,
execuções que não terminaram são marcadas failed/blocked e mantidas; worktrees mergeados e não removidos são tentados
de novo.

## 5. Shell ao vivo e controle de processo (ADR 0007)

Uma execução de CLI roda em um **PTY real** (`portable-pty`; ConPTY no Windows) com uma **sessão de processo**. O chat
mostra o que o agente _diz_; o terminal mostra o que o processo _faz_ — nunca se misturam. É **observabilidade e
controle, não sandbox**: controla um processo que o guard já autorizou e **não inicia nada**.

- **Por que PTY**: CLIs mudam de comportamento quando stdout não é terminal; o PTY dá ANSI, tamanho redimensionável e
  Ctrl+C como interrupção do grupo de processos em primeiro plano.
- **Custos**: stdout e stderr se fundem; `claude -p`/`opencode run` ignoram stdin de terminal (prompt vai como último
  argumento após `--`); sequências de controle são removidas por um _stripper_ com estado antes do parse; em Windows
  `.cmd` com prompt multilinha volta aos pipes.
- **Identidade**: `executionId` + `processSessionId`; toda requisição de controle nomeia workspace + agente + execução
  e só é atendida se os três baterem com o escopo aprovado pelo guard; qualquer outra coisa é o mesmo
  `ExecutionNotFound`. O frontend nunca vê PID, sinal ou grupo de processos.
- **Interromper** = byte `0x03` no PTY (SIGINT no grupo em primeiro plano; evento de controle no ConPTY); **Terminar** =
  SIGTERM ao grupo e, após 2 s, SIGKILL (Windows: `taskkill /T /F`). Sinais são enviados sob um lock também mantido ao
  colher o filho (nunca atinge PID reutilizado).
- **Estados**: `Running → Interrupting | Terminating → Exited`; a execução persistida ganhou `Cancelled` (só quando o
  usuário agiu e ela não completou).
- **Entrada manual**: CLIs atuais reportam `terminalInput = false` (terminal somente leitura); o caminho existe e é
  imposto **no core**: recusado se o runtime não habilitou, bytes passam sem interpretação, > 64 KiB recusado, `0x03`
  isolado vira _interrupt_.
- **Saída**: chunks `execution:output` (≤ 8 KiB, `seq` por sessão); o core guarda os últimos **512 KiB** por sessão e as
  últimas **8** sessões encerradas; **nada é persistido** em `config.json`.
- **Ao fechar o Atlas**: pergunta se há agentes rodando (cancelar e sair / manter aberto); ao encerrar, os processos
  dos agentes terminam junto (até 4 s). Se o Atlas for morto, o kernel faz hangup nos filhos.
- Validado em macOS com as CLIs reais (testes opt-in: `cargo test real_ -- --ignored --nocapture --test-threads=1`).

## 6. Limites conhecidos (resumo honesto)

- Não há sandbox de SO (`sandbox.rs` é só o ponto de encaixe). As ferramentas internas de uma CLI não são contidas.
- Agentes ainda **não rodam shell, testes ou rede**; só leem e (se permitido) editam arquivos no worktree.
- Windows não foi compilado/executado; Linux não foi executado.
- Submódulos e layouts incomuns de worktree não têm tratamento específico.
- Um worktree por execução é um checkout completo (chat isolado somente leitura paga esse custo; worktree sem mudanças
  é removido logo em seguida).
