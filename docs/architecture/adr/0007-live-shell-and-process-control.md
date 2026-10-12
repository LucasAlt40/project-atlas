# ADR 0007 — Shell ao vivo e controle de processos

**Status:** aceito

## O que é

Uma execução que roda um runtime de CLI (Claude, OpenCode) agora roda ligada a um **pseudo-terminal real**. Enquanto o
processo vive, ele tem uma **sessão de processo** (`processSessionId`); a UI pode mostrar a saída real do processo
(xterm.js), interrompê-lo (Ctrl+C), terminá-lo e — só se o runtime lê entrada — digitar nele. O chat é o que o agente
_diz_; o terminal é o que o seu processo _faz_; os dois nunca se misturam.

```
Workspace → Agente → Execução → ProcessSession
ExecutionService → RuntimeRegistry → ModelRuntime → ProcessRunner (guard → sistema) → PTY → claude | opencode
```

É **observabilidade e controle de processo, não um sandbox.** A segurança continua vindo da `PermissionPolicy`, das
restrições do próprio runtime e, mais tarde, de um sandbox de SO (ADR 0006). O shell ao vivo controla um processo que o
Atlas já autorizou; não pode iniciar um.

## Peças

| Peça                                                            | Responsabilidade                                                                                                                 |
| --------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `application/process.rs` — `ProcessSpec.terminal`               | Um runtime pede para rodar em um terminal. Nada vindo da webview chega até aqui.                                                 |
| `infrastructure/pty.rs`                                         | Inicia o processo em um PTY (`portable-pty`), lê sua saída, o encerra, implementa `ProcessSession`.                              |
| `application/sessions.rs` — `ProcessSession`, `SessionRegistry` | Identidade, saída efêmera limitada, estado, quem o parou e o **único** caminho de controle.                                      |
| `ExecutionService`                                              | Pergunta ao registro se o usuário parou uma execução que terminou mal → `Cancelled` em vez de `Failed`.                          |
| `commands/terminal.rs`                                          | `get_execution_terminal`, `execution_interrupt`, `execution_terminate`, `execution_terminal_input`, `execution_terminal_resize`. |
| `commands/events.rs`                                            | `execution:output` (trechos), `execution:status` (estado) e atividade no `execution:progress` existente.                         |
| `commands/exit.rs`                                              | O que acontece com os agentes em execução quando o Atlas fecha.                                                                  |

O `ProcessRunner` continua pequeno (`locate`, `run`). Os controles de um processo em execução são um objeto separado,
`ProcessSession` (`write`, `interrupt`, `terminate`, `resize`), alcançado pelo registro — um runner inicia, uma sessão
controla. O comportamento específico de cada runtime fica no runtime: `cli::deliver_prompt` é o único auxiliar
compartilhado, e nenhum código se ramifica conforme o runtime.

## Por que um PTY, e não pipes

Com pipes o filho vê que a stdout não é um terminal; as CLIs mudam de comportamento (buffering, cores, prompts, saída de
progresso) e algumas se recusam a ser interativas. Um PTY dá ao filho um terminal: saída ANSI, um tamanho que ele pode
consultar e que pode mudar (`resize`), e um driver de terminal que transforma **Ctrl+C em uma interrupção para o grupo
de processos em primeiro plano**.

Biblioteca: **`portable-pty`** (a camada de PTY do WezTerm). Suporta PTYs Unix e ConPTY do Windows atrás de uma única
API, não exige runtime assíncrono e é mantida por um projeto que depende dela para o próprio terminal. Alternativas
consideradas: `pty-process` e `nix::pty` (só Unix, então o Windows exigiria uma segunda implementação), `conpty` (só
Windows) e `openpty` feito à mão (exige `unsafe`, que este crate proíbe). Novas dependências: `portable-pty` e, só no
Unix, `nix` (já dependência transitiva do `portable-pty`, mesma versão; usada para o `killpg` seguro). O crate mantém
`unsafe_code = "forbid"`.

Custo de um PTY, verificado contra as CLIs reais:

- **Um único fluxo de saída.** stdout e stderr se juntam. Os runtimes leem seus diagnósticos das linhas não JSON da saída
  combinada (`cli::diagnostics`); o campo `stream` de `execution:output` faz parte do contrato mas é sempre `stdout`.
- **Sem canal de entrada para um prompt.** `claude -p` e `opencode run` **ignoram a stdin de um terminal** (informam
  "input must be provided…"). Em um terminal, portanto, o prompt é o último argumento, depois de `--`, para que texto
  começando com `-` nunca seja uma opção. O caminho por pipe (prompt na stdin) permanece para o único caso que um
  terminal não consegue atender (abaixo).
- **Sequências de controle na saída.** O resultado estruturado (`stream-json` / `--format json`) ainda passa pelo
  terminal, mas pode vir envolto em sequências de cursor/cor e CRLF. O runner as remove (um removedor com estado, que
  sobrevive a sequências divididas entre leituras) antes de os runtimes interpretarem; os bytes brutos vão para a visão
  do terminal. Um teste executa o runtime real do Claude contra uma CLI de substituição que imprime sequências em torno
  do seu JSON.
- **Shims `.cmd` do Windows** não aceitam argumentos de várias linhas, então um prompt com quebras de linha em um shim
  `.cmd`/`.bat` recorre a pipes (aquela execução não tem terminal ao vivo). Instalações `.exe` nativas usam o terminal.

`--output-format stream-json` é compatível com um terminal: as linhas JSON são a saída real do processo, o
`ExecutionResult` do runtime não muda, e nada foi removido para o terminal funcionar.

## Identidade

`executionId` (do próprio Atlas, já existia) e `processSessionId` (novo, por processo ao vivo). O registro é indexado
por execução. Todo pedido de controle nomeia **os três: workspace, agente e execução**; uma sessão só responde quando os
três são iguais aos com que foi aberta (tirados do `ExecutionScope` aprovado pelo guard, nunca da webview). Qualquer
outra coisa — desconhecido, outro workspace, outro agente — é o mesmo `ExecutionNotFound`, então uma sondagem não
descobre nada. Não existe "processo atual" nem singleton: os Agentes A, B, C e D têm cada um sua execução e sessão, e
interromper o A não alcança o B. O frontend conhece ids de execução, eventos e capacidades; nunca vê um PID, um sinal ou
um grupo de processos.

## Ctrl+C, terminar e árvores de processos

**Interromper** é o byte `0x03` escrito no PTY — um mecanismo em todo SO:

- **macOS / Linux:** o driver do terminal envia `SIGINT` ao grupo de processos em primeiro plano do PTY. A CLI o recebe
  como receberia de uma pessoa, e também tudo o que ela iniciou em primeiro plano (`node → npm → test`). O filho é líder
  de sessão (`setsid`, feito pelo `portable-pty`), então seu grupo de processos é só dele.
- **Windows:** o ConPTY traduz a entrada em um evento de controle de console para os processos ligados ao
  pseudo-console.

Deliberadamente não é `kill(pid)`: isso atingiria só a raiz, pularia o encerramento gracioso da CLI e não conseguiria
distinguir "parar" de "morrer". A CLI pode tratar a interrupção ou ignorá-la; o Atlas informa o estado (`interrupting`)
e deixa o próximo passo para o usuário. Uma segunda interrupção é só um segundo Ctrl+C e não pode causar pânico.

**Terminar** é o segundo passo, sempre disponível e visualmente secundário:

- **Unix:** `SIGTERM` para o grupo de processos inteiro (`killpg`) e, se o grupo ainda existir após 2 s, `SIGKILL` nele.
- **Windows:** `taskkill /PID <pid> /T /F` (mata a árvore; não existe sinal gracioso de grupo).

Os sinais são enviados segurando um lock que também é mantido quando o filho é recolhido, de modo que um sinal nunca
possa atingir um PID que o SO já reutilizou. Quando um processo termina, seu terminal é fechado: o kernel desliga
(`SIGHUP`) tudo o que ainda estiver ligado. Um descendente que se desprende de propósito (nova sessão, ignora `SIGHUP`)
está fora do alcance do Atlas.

Estado: `Running → Interrupting | Terminating → Exited` (ao vivo, por processo, `execution:status`). O status
persistido da execução ganhou **`Cancelled`**; `Interrupting`/`Terminating` são estados de sessão ao vivo, não
guardados. `Timeout` continua sendo um tipo de falha de uma execução `Failed` (compatível com o que existe); `Queued` e
`Starting` não foram adicionados: nada entra em fila, e iniciar já é `starting_runtime`. Uma execução termina como
`Cancelled` quando não completou **e** o usuário agiu sobre o seu processo; uma execução que terminou antes de a
interrupção chegar continua `Completed`.

## Entrada manual

Investigado, não presumido: as duas CLIs rodam de forma não interativa (`claude -p`, `opencode run`) e **ignoram a stdin
de um terminal**. Portanto as duas informam `terminalInput = false` e seu terminal é somente leitura. O Ctrl+C funciona
de qualquer jeito. O caminho existe para um futuro runtime interativo (`RuntimeCapabilities.terminal_input`, depois
`ProcessSpec.terminal.input`) e é imposto no núcleo, não na UI: a entrada é recusada a menos que o runtime a habilite;
os bytes são repassados **intocados** (Ctrl+D, Ctrl+Z, sequências de escape e texto colado de várias linhas são apenas
entrada para o terminal do próprio processo e nunca são interpretados como comandos do Atlas); uma carga acima de 64 KiB
é recusada; e `0x03` é a única exceção — sozinho é encaminhado pela _interrupção_ (para que o estado e o cancelamento
sejam rastreados), misturado com outros bytes é recusado.

## Capacidades

`RuntimeCapabilities` ganhou `interactiveTerminal`, `interrupt`, `terminalInput`, `terminalResize`.

| Runtime      | interactiveTerminal | interrupt | terminalInput | terminalResize |
| ------------ | :-----------------: | :-------: | :-----------: | :------------: |
| Claude CLI   |         sim         |    sim    |      não      |      sim       |
| OpenCode CLI |         sim         |    sim    |      não      |      sim       |
| Codex CLI    |         sim         |    sim    |      não      |      sim       |

A UI mostra só o que o runtime pode fazer ("não oferece um terminal", sem botão de Interromper, selo de somente
leitura).

## Segurança: relação com a `PermissionPolicy`

- Uma sessão de processo só existe para um processo que o **guard deixou iniciar** (o `GuardedProcessRunner` avalia toda
  especificação antes de chegar ao runner do sistema; um programa negado ou desconhecido nunca recebe uma sessão). O
  runner nunca aceita um programa vindo dos comandos, e **nenhum comando inicia um processo** — não há shell, nem "abrir
  terminal", nada que receba um programa ou uma linha de comando da webview.
- A webview não tem a confiança para autorizar: o núcleo reconfere workspace/agente/execução em todo pedido.
- A saída do terminal é dado para exibição. Nunca é interpretada como instrução nem convertida em ações do Atlas.
- O guard ainda registra `LaunchRuntime` e as notas `runtime_exceeds_policy`; o shell ao vivo acrescenta atividade
  (usuário vs. agente): `terminal_connected`, `user_interrupted`, `user_terminated`, `process_exited`, `cancelled`.
- Limites: o shell ao vivo não é um sandbox. Ele não pode impedir o que as ferramentas do próprio runtime fazem dentro
  do processo (a ferramenta de shell do OpenCode roda dentro do grupo de processos do terminal, por exemplo); o Ctrl+C
  as alcança, nada aqui as restringe.

## Saída, persistência e desempenho

A saída chega à UI como trechos incrementais `execution:output` (`seq` por sessão), remontados em um `TerminalHub` fora
do estado do React. O núcleo guarda os últimos **512 KiB** por sessão e as últimas **8** sessões terminadas, para que um
terminal possa ser redesenhado depois de um recarregamento ou troca de workspace (`get_execution_terminal` devolve um
snapshot com `nextSeq`, de modo que o snapshot e os trechos ao vivo se encontrem sem lacuna nem repetição). Nada é
persistido: o `config.json` nunca recebe saída de terminal; o registro da execução ainda guarda prompt, resposta,
status, uso, horários e falha. Os trechos são como lidos do PTY (≤ 8 KiB); nenhum agrupamento ou contrapressão foi
adicionado — as CLIs produzem linhas na velocidade dos tokens e isso não mostrou necessidade. Rever se um runtime
transmitir megabytes por segundo.

## Quando o Atlas fecha

- Janela fechada / saída com execuções em andamento: o Atlas pergunta ("Agentes ainda estão em execução": **Cancelar
  execuções e sair** / **Manter o Atlas aberto**). O diálogo é nativo e redigido pelo núcleo no idioma do usuário (a
  webview está indo embora).
- De qualquer forma que a aplicação termine, os processos dos seus agentes terminam com ela (`RunEvent::Exit` → terminar
  todos, esperando até 4 s). Se o Atlas for morto de vez (`SIGKILL`, queda, falta de energia) o master do PTY fecha e o
  kernel desliga os filhos, o que faz as CLIs usuais saírem; uma que ignore `SIGHUP` poderia sobreviver. O Windows fecha
  o pseudo-console com o mesmo efeito, não verificado.
- Trocar de workspace nunca toca nos processos. Suspender/acordar: os processos simplesmente continuam; nada é feito.

## Validação e limitações conhecidas

- **macOS:** validado, inclusive com as CLIs reais do Claude e do OpenCode (testes opt-in: `cargo test real_ --
--ignored --nocapture --test-threads=1` — uma tarefa longa real, saída observada, Ctrl+C, processo sumido, execução
  `Cancelled`).
- **Linux:** os caminhos de código Unix são os mesmos e usam só recursos POSIX; não foram executados em uma máquina
  Linux neste marco.
- **Windows:** o código tem ramos `cfg(windows)` mas **não foi executado nem compilado** aqui (o script de build do Tauri
  precisa do compilador de recursos do Windows, ausente nesta máquina). O tratamento do `0x03` pelo ConPTY como evento
  de controle de console, o comportamento do `taskkill` e os shims `.cmd` precisam de uma máquina Windows; até lá trate
  como não validado.
- stdout/stderr combinados; prompt visível na lista de argumentos do processo (`ps`) enquanto uma execução em terminal
  está em andamento (antes ia na stdin); ambos são consequências do PTY.
- Interromper/terminar são de melhor esforço: o processo pode ignorar o Ctrl+C (é para isso que serve Terminar) e um
  neto desprendido de propósito está fora de alcance.
- A saída do terminal não é pesquisável depois que o Atlas reinicia (efêmera por projeto); um log com limite explícito
  seria uma decisão posterior e separada.
