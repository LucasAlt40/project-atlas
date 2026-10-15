# Spike: sandbox do macOS para os runtimes de CLI

Data: 2026-10-15. Custo: zero (nenhuma resposta de modelo; cada CLI foi chamada com um modelo que não existe e falha no
primeiro pedido, como já se fazia no spike do MCP). Nada foi instalado e nada em `~/.claude` ou em outros diretórios das
ferramentas foi editado por mim: o que as CLIs escreveram, escreveram sozinhas, ao iniciar.

## Pergunta

Hoje o `SandboxProvider` (`application/security/sandbox.rs`) só tem `NoSandbox`: um comando permitido roda com todos os
direitos da pessoa, e as ferramentas internas de um runtime (`Edit`, `Write`, `Bash` do Claude…) não passam pelo guard do
Atlas (ADR 0025). Se o **processo da CLI** inteiro rodasse num perfil `sandbox-exec`, o sistema operacional confinaria
também as ferramentas internas e tudo o que elas iniciam. O que cada CLI precisa tocar para isso funcionar?

## Método

1. **Escritas**: marcar o tempo, rodar a CLI sem sandbox e listar o que foi criado ou alterado em `$HOME`, caches e
   temporários (`find -newer`). Não precisa de root nem de log do sistema.
2. **Confinamento**: rodar a mesma chamada sob um perfil que nega toda escrita fora do worktree, do temporário e da lista
   da CLI, e nega a leitura de cofres de credenciais. Comparar com a saída sem sandbox.
3. **Controle negativo**: repetir sem a lista da CLI, para provar que o perfil estava ativo.
4. **Git**: um commit dentro de um worktree vinculado, com e sem acesso ao `.git` do repositório principal.

Descartado: `(trace …)` do `sandbox-exec` (aceito e ignorado no macOS 27, nenhum arquivo é gerado) e o log do sistema
(`log show` não devolve negações sem privilégio). Por isso o método 1.

## Resultados

`sandbox-exec` existe em `/usr/bin` e funciona no macOS 27.0.1 (obsoleto desde 2016, mas ativo). Versões: claude 2.1.292,
codex 0.162.1, opencode 1.19.0, gemini 0.46.0, agy (Antigravity).

### O que cada CLI escreve ao iniciar (ruído de outros programas removido)

| CLI      | Escreve em                                                                                                                                       |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| claude   | `~/.claude/**` (sessões, plugins), `~/.claude.json` (e temporários irmãos), `~/Library/Caches/claude-cli-nodejs`                                 |
| codex    | `~/.codex/**` (bancos sqlite com `-wal`/`-shm`, `sessions`, `skills`, `plugins`, `tmp/arg0`), e `~/.npm` quando um MCP do usuário sobe por `npx` |
| opencode | `~/.local/share/opencode` (log e `opencode.db`), `~/.local/state/opencode`                                                                       |
| gemini   | `~/.gemini/**` (`tmp`, `history`, `oauth_creds.json`, `projects.json`), `~/.npm`                                                                 |
| agy      | `~/.gemini/antigravity-cli/**` (logs, cache, `.db`, mcp), `~/.local/bin/.new.tar.gz` (**autoatualização do binário**), `login.keychain-db`       |

### Sob o perfil restritivo (só a lista da própria CLI liberada)

| CLI      | Resultado                                                                         | Sem a lista (controle)                                                                      |
| -------- | --------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| claude   | idêntico ao sem sandbox (chega à API e recebe o erro de modelo)                   | **continua funcionando**: engole as falhas de escrita (sessão e cache não são gravados)     |
| codex    | idêntico                                                                          | falha na hora: "failed to initialize in-process app-server client: Operation not permitted" |
| opencode | idêntico                                                                          | falha na hora: não abre `~/.local/share/opencode/log/opencode.log`                          |
| gemini   | idêntico (falha adiante por conta de elegibilidade da conta, igual a sem sandbox) | não medido                                                                                  |
| agy      | idêntico                                                                          | falha ao migrar o estado                                                                    |

Rede e Keychain: com `(allow default)` e escrita negada, **autenticação (Keychain) e rede funcionam** nas cinco CLIs. Os
processos filhos herdam o perfil: um `sh` iniciado dentro dele não consegue criar arquivo em `$HOME`, consegue no
worktree, não lê `~/.ssh` e alcança a rede.

### Git em worktree (o achado que mais pesa)

O `.git` de um worktree vinculado é um arquivo que aponta para `<repo>/.git/worktrees/<id>`. **Um `git commit` escreve no
`.git` do repositório principal** (objetos, ref da branch, lock do índice). Só com o worktree liberado o commit falha
("não foi possível criar `.git/worktrees/<id>/index.lock`"). Liberar o `.git` inteiro resolve, mas deixa o agente editar
`hooks/` (execução de código fora do perfil), `config` e a branch principal.

Um conjunto estreito basta e foi testado (commit e `git status/log` funcionam):

```
<repo>/.git/worktrees/<id>      <repo>/.git/objects
<repo>/.git/refs/heads/atlas    <repo>/.git/logs/refs/heads/atlas
```

Com ele, **negados**: atualizar `refs/heads/main`, mudar `config` (`core.hooksPath`) e criar `hooks/post-commit`. Também
ficam negados `git stash`, `git branch -d`, `git gc`, `fetch`/`pull`/`push` (escrevem fora desse conjunto).

### O que o perfil não faz

- **Rede por domínio**: o SBPL só filtra por endereço e porta. Dá para fechar tudo menos TCP 443, DNS e `localhost`, não
  para listar `api.anthropic.com`. A exfiltração por HTTPS continua possível; o perfil protege a leitura de segredos e a
  escrita, não o canal de saída.
- **Aninhar sandboxes**: se uma ferramenta já aplica seatbelt a si mesma (o sandbox opcional do Claude Code), o
  `sandbox_apply` interno falha dentro do nosso perfil.
- **Windows e Linux**: nada daqui vale; ficam em `NoSandbox`, e a interface precisa dizer isso.

## Decisões de desenho que o spike sugere

1. **Permissivo por padrão, negando o que importa**: `(allow default)` + `(deny file-write*)` + lista de escrita + negação
   de leitura dos cofres (`~/.ssh`, `~/.aws`, `~/.gnupg`, `~/.kube`, `~/.docker`, `~/.netrc`, `~/.config/gh`,
   `~/.config/gcloud`, perfis de navegador, Mail, Messages). Um perfil negar-tudo exigiria o log de negações, que não está
   disponível, e quebra com cada atualização das CLIs.
2. **Escrita liberada por execução**: o worktree; o temporário **da execução** (o Atlas define `TMPDIR` para uma pasta
   própria; não se libera `/private/tmp` inteiro); o `.git` no conjunto estreito acima; e a lista da CLI da tabela
   (`~/.claude`, `~/.claude.json*`, `~/Library/Caches/claude-cli-nodejs`; `~/.codex`; `~/.local/share/opencode`,
   `~/.local/state/opencode`; `~/.gemini`).
3. **Caches de ferramentas de desenvolvimento são uma decisão da pessoa**: `npm ci`, `cargo test`, `pip install` escrevem
   em `~/.npm`, `~/.cargo`, `~/.cache`, `~/Library/Caches`, `~/.pnpm-store`, `~/.gradle`, `~/.m2`. Negar quebra builds e
   testes; liberar permite envenenar um cache compartilhado. Proposta: liberar uma lista conhecida, ligada por padrão e
   mostrada nas configurações, e `~/.npm` só quando houver MCP por `npx`.
4. **Falha fechada e visível**: se o `sandbox-exec` não existir ou o perfil não puder ser montado, a execução não começa
   (a política diz "sandbox exigido") ou começa sem sandbox **dizendo isso** no manifest e na interface. Nunca em silêncio.
5. **Perfil gerado, não escrito à mão**: o Atlas monta o perfil por execução (escapando caminhos) em um arquivo `0600`
   no diretório da execução e passa os caminhos por `-D`, nunca por interpolação de texto do agente.

## Não verificado

- Uma tarefa real com ferramentas (um `Edit`, um `Bash`, `npm`/`cargo`) dentro do perfil: exige uma resposta de modelo, ou
  seja, tokens. As escritas durante uma tarefa (transcrições, `shell-snapshots`, `todos`) são previstas pelas listas, mas
  só uma execução real confirma.
- Modo terminal (PTY) do Atlas sob o perfil.
- Windows e Linux (fora do escopo desta fase).
- Remoção do `.new.tar.gz` do agy: com o perfil, a autoatualização do binário fica bloqueada; é o desejado, mas muda o
  comportamento dessa CLI e deve estar explícito na interface.
