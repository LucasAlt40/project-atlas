# ADR 0038 — Sandbox do macOS para o processo dos runtimes

Status: aceito e implementado, **desligado por padrão**. Verificado com modelo real em quatro CLIs (o trabalho normal
funciona dentro do perfil); a tentativa de fuga pela CLI real não pôde ser exercitada: veja "Verificação com modelo real". Baseia-se no spike
[macos-sandbox.md](../spikes/macos-sandbox.md), na [ADR 0003](0003-command-permissions.md) (permissões de comandos), na
[ADR 0009](0009-git-worktree-isolation.md) (worktrees) e na [ADR 0025](0025-guardrails-and-context-review.md), que registra
o limite que isto fecha em parte: o Atlas não consegue se interpor às ferramentas internas de um runtime (`Edit`, `Write`,
`Bash`).

## Contexto

O guard decide _se_ um processo pode começar, não o que ele faz depois. Um runtime autorizado roda com os direitos da
pessoa, e as ferramentas que a CLI tem dentro do próprio processo nunca passam pelo Atlas. O `SandboxProvider` existia só
como ponto de encaixe (`NoSandbox`). Se o **processo da CLI** inteiro rodar sob um perfil do sistema operacional, o sistema
confina também as ferramentas internas e tudo o que elas iniciam, sem que o Atlas precise interceptá-las.

## Decisões

### 1. O que é confinado e o que não é

O processo de um runtime (`ProcessContext::Runtime`) e, no futuro, um comando pedido por um agente
(`AgentRequested`) passam pelo provedor. Sondas (`--version`), as ferramentas do próprio Atlas (`openspec`) e a UI não.
No macOS, o programa vira `sandbox-exec -D … -f <perfil> <caminho absoluto da CLI> <args originais>`. O terminal (PTY)
não muda: o wrapper é o programa.

### 2. Permissivo por padrão, negando o que importa

`(allow default)` + `(deny file-write*)` + uma lista de escrita + negação de **leitura** dos cofres de credenciais. A
autenticação (Keychain), a rede e as ferramentas continuam funcionando, como o spike mediu nas cinco CLIs. Um perfil
"negar tudo" exigiria o log de negações, que não está disponível, e quebraria a cada atualização das CLIs. Nas regras do
perfil a última que casa vence; por isso as negações que precisam vencer uma liberação (`hooks`) vêm depois.

### 3. O que uma execução pode escrever

| Liberado                                                       | Por quê                                                                                                     |
| -------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| o worktree (ou o projeto, se for o próprio diretório)          | só quando a execução é isolada **ou** a política permite escrever; um run de leitura no projeto não escreve |
| uma pasta temporária **da execução** (`TMPDIR` próprio, 0700)  | `/private/tmp` inteiro não é liberado                                                                       |
| as pastas da própria CLI (`sandbox/cli.rs`)                    | `~/.claude`, `~/.codex`, `~/.gemini`, `~/.local/share/opencode`…                                            |
| `.git` estreito, se o diretório é um worktree vinculado        | veja abaixo                                                                                                 |
| caches de ferramentas de desenvolvimento, **se a pessoa quer** | `~/.npm`, `~/.cache`, `~/.cargo/registry`…                                                                  |

**Git estreito.** O `.git` de um worktree vinculado é um arquivo que aponta para `<repo>/.git/worktrees/<id>`, e um commit
escreve no `.git` do repositório principal. Libera-se exatamente: o diretório do worktree, `objects`, `refs/heads/atlas` e
`logs/refs/heads/atlas`. Ficam negados `hooks` (código fora do perfil), `config`, `refs/heads/main` e qualquer outra
branch, e também `stash`, `branch -d`, `gc`, `fetch`/`pull`/`push`. Num projeto que é o próprio diretório de trabalho,
`.git/hooks` e `.git/config` são fechados de novo dentro da pasta liberada. O caminho do `.git` vem do arquivo `.git`
do worktree, nunca de texto de um agente.

**Cofres negados para leitura:** `~/.ssh`, `~/.aws`, `~/.gnupg`, `~/.kube`, `~/.docker`, `~/.netrc`, `~/.config/gh`,
`~/.config/gcloud`, perfis do Chrome e do Safari, Mail e Messages. `~/.npmrc` fica legível de propósito (o `npm` precisa
dele para registros privados): é um risco residual. A autoatualização do binário do `agy` (`~/.local/bin`) fica negada de
propósito: o sandbox impede uma CLI de se substituir.

**Uma CLI com sandbox próprio (Codex).** O Codex aplica o seu seatbelt aos comandos que roda (`--sandbox`), e isso
**falha dentro do nosso perfil**: `sandbox-exec: sandbox_apply: Operation not permitted` (medido sem custo com
`codex sandbox -- <cmd>`, dentro e fora do perfil). Sem tratamento, todo comando de shell do Codex falharia com o sandbox
obrigatório. Dentro do perfil o Atlas troca o valor de `--sandbox` por `danger-full-access` (`cli::adapt_to_confinement`),
que aqui só significa "não adicione um segundo sandbox": o confinamento é o do processo inteiro. Um run que o Codex
manteria somente-leitura (`read-only`) continua assim porque o perfil deixa o worktree **sem escrita**. Só acontece
quando o processo _é_ confinado; sem confinamento o Codex mantém o dele. A auditoria registra
`sandbox:replaces_cli_sandbox:codex`. O mesmo vale, em princípio, para qualquer ferramenta que se confine sozinha.

**Ferramentas por runtime sob o Atlas** (o que o teste real precisa respeitar): o Claude recebe só `Read, Grep, Glob, Edit,
Write` (sem shell), o agy aceita edições e recusa shell em modo headless, o OpenCode e o Codex têm shell.

### 4. Perfil gerado, não escrito à mão

`sandbox/profile.rs` é uma função pura de fatos para texto. **Todo caminho entra como parâmetro `-D`** (`(param "P3")`),
nunca como texto dentro do perfil: um caminho com aspas não altera o que o perfil diz (há teste). O único texto
interpolado, o prefixo de arquivos como `~/.claude.json` (e seus irmãos temporários), é verificado e recusado se não for
simples. O perfil vai para um arquivo `0600` em `<dados do Atlas>/sandbox/profiles/` e é removido depois de um dia.

### 5. Falha visível, nunca silenciosa

Três modos (`SandboxMode`): `off` (padrão), `auto` e `required`.

- **auto**: confina quando o sistema pode; quando não pode (Windows, Linux, sem `sandbox-exec`, runtime desconhecido)
  roda sem confinamento **e diz isso** na trilha de auditoria (`sandbox:unavailable:<motivo>`, mostrada na atividade
  com ⚠ e o motivo).
- **required**: não inicia (`Reason::SandboxUnavailable`, registrado como negado).
- **Um perfil que não pôde ser feito** (pasta ilegível, caminho que não é simples) nunca é pulado, em nenhum dos modos
  (`Reason::SandboxRefused`).
- Um confinamento bem-sucedido aparece como `sandbox:confined` na mesma linha de "permissão concedida: iniciar …".
- O modo `off` não diz nada (nada mudou). O padrão é `off` porque o confinamento muda o comportamento e a primeira
  tarefa real com ferramentas ainda não foi observada.

### 5b. O que se sabe de cada CLI é uma Strategy, não um `match`

`RuntimeConfinement` (porta, em `sandbox/cli.rs`) tem uma implementação por runtime em `runtimes/confinement.rs` (**Strategy**),
listadas numa fábrica (`default_confinements`, **Factory**) e achadas pelo nome do programa num `ConfinementRegistry`
(**Registry**). Cada uma diz o que a CLI escreve (`needs`) e, se for preciso, como seus argumentos mudam dentro do
confinamento (`adapt`; só o Codex muda). O provedor do macOS não conhece nenhum runtime pelo nome. Um runtime novo é uma
`impl` e uma linha na fábrica, e um teste falha enquanto um programa de `RUNTIME_PROGRAMS` não tiver estratégia. O mesmo molde
já era usado para o MCP (`mcp_adapter.rs`).

### 6. Onde fica na arquitetura

`GuardedProcessRunner` chama o provedor com a política, o escopo da execução e o caminho do programa (`locate`). O
provedor decide, e o guard registra a nota no evento "permitido" antes de iniciar. A configuração
(`AppSettings.sandbox`) é lida a cada início, então uma mudança vale para a próxima execução. As listas por CLI vêm do
spike (`sandbox/cli.rs`) e um teste garante que nenhuma toca cofres nem o binário da CLI.

### 7. Interface

Configurações → "Sandbox dos runtimes": o modo, a escolha dos caches de ferramentas e o que este sistema consegue (com o
que cada modo significa onde não há sandbox). A atividade da execução mostra "confinado pelo sistema" ou o aviso.

## O que isto não faz

- **Rede por domínio**: o SBPL só filtra por endereço e porta. A saída por HTTPS continua aberta: o perfil protege a
  leitura de segredos e a escrita, não o que sai pela conexão.
- **Escopo do processo**: ele é confinado ao que a execução precisa, não à tarefa: o worktree inteiro é gravável.
- **Windows e Linux**: sem confinamento (`auto` diz isso, `required` recusa). AppContainer, namespaces ou um contêiner
  seriam outros provedores atrás da mesma interface.
- **Sandbox aninhado**: uma ferramenta que aplica seatbelt a si mesma (o sandbox opcional do Claude Code) falha dentro
  do perfil.
- A pasta temporária de cada execução não é apagada ao fim; é varrida depois de um dia.

## Verificação com modelo real (2026-10-15)

Quatro CLIs reais, pelo caminho inteiro do Atlas (guard, `MacosSandbox` em modo `required`, worktree isolado, CLI real), em
`executions/sandbox_real_tests.rs` (8 testes `#[ignore]`, rodados pelo nome completo exato). Consumo reportado: opencode
(grátis) ~60 mil tokens; claude haiku ~35 mil (US$ 0,03); codex ~145 mil (uma execução repetida); agy ~55 mil.

| CLI                        | Trabalho dentro do perfil                                                                                                 | Tentativa de fuga                               |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------- |
| opencode                   | **ok**: criou o arquivo, `ls` e `git status` no shell, o Atlas fez o merge                                                | **não exercitada**: o modelo recusou duas vezes |
| claude (haiku)             | **ok**: criou o arquivo, Atlas fez o merge                                                                                | **não exercitada**: o modelo recusou            |
| codex (gpt-5.6-luna)       | **ok**: `file_change` e dois comandos de shell rodaram, com `sandbox:replaces_cli_sandbox:codex`; o arquivo chegou ao Git | **não exercitada**: o modelo recusou            |
| agy (gemini-3.8-flash-low) | **ok**: criou o arquivo, Atlas fez o merge                                                                                | **não exercitada**: o modelo recusou            |

O que isto prova: nas quatro CLIs o confinamento **não impede o trabalho normal** (estado da CLI, rede, Keychain,
shell, `git status`, escrita no worktree) e o Codex roda comandos de shell dentro do perfil depois da troca de
`--sandbox`. O que **não** prova: que a CLI real é impedida de sair do worktree. Os modelos seguem a regra do próprio
Atlas ("não toque em nada fora do worktree") e se recusam a tentar, mesmo com a tentativa descrita como teste autorizado
pelo dono da máquina; não se insistiu além disso, e os testes de fuga agora dizem `NOT EXERCISED` em vez de passar sem
dizer nada. A recusa é, ela mesma, o comportamento esperado do prompt. A confinação em si é provada pelos testes com
`sandbox-exec` real (`macos.rs`), e um teste do pipeline inteiro com uma CLI de mentira que de fato tenta as escritas
(`HOME`, `/private/tmp`, árvore do projeto, `hooks`) sob `required`: nenhum arquivo apareceu.

Um bug do teste, não do sandbox: o prompt do teste de trabalho dizia "sandbox ok." e o Codex escreveu o ponto; o prompt
e a verificação foram corrigidos (o teste corrigido não foi repetido para não gastar mais tokens).

## Não verificado

- A CLI real tentando de fato escrever fora do perfil (ver acima): só uma execução em que o modelo aceite tentar, ou um
  teste que invoque a ferramenta de arquivo da CLI sem passar pelo modelo, fecharia isso.
- `npm`/`cargo` de verdade (caches de ferramentas) e o modo terminal (PTY) sob o perfil.
- Quais escritas cada CLI _tenta e é negada_ sem quebrar: o `Claude` engole falhas; as outras, até agora, não falharam.

## Plano para ligar por padrão

1. ~~Rodar uma tarefa real por CLI com `required`~~ feito: o trabalho normal funciona nas quatro.
2. Decidir o padrão `auto` (a pessoa liga primeiro nas configurações e usa por um tempo).
3. Fechar a lacuna da fuga com a CLI real, se for importante (veja acima).
