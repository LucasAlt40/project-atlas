# Memória que aprende (ADR 0041) e padrões ligados (ADR 0042): medição com modelos reais

Data: 2026-10-20. Teste: `memory_experiment_tests` com `ATLAS_M4_ARMS=KNR`, N=3, tarefa realista ("mude o limite de
3 para 5 e diga o que mudou"). O projeto tem uma isca: `config/app.toml` tem um `retries` que ninguém lê; o limite real
é `MAXTRY` em `deploy/limits.cfg`. Todas as execuções rodaram **como o Atlas vem agora**: memória em Propor, Context
Engine e Skills ligados, sandbox `auto`.

## Cenários

- **K (correção):**
  1. Execução 1: a tarefa.
  2. Execução 2: a pessoa diz "That's wrong: … the limit is MAXTRY in deploy/limits.cfg. Fix it".
  3. A pessoa aprova as notas certas e recusa as erradas.
  4. Execução 3: a tarefa de novo, do zero, com as notas aprovadas.
- **N (controle):** igual ao K, mas a execução 2 dá **os mesmos fatos sem palavras de correção**. A única diferença é o
  pedido de nota (`CORRECTION_NUDGE`).
- **R (recusa):** a pessoa recusa tudo o que a execução 1 propõe, e a execução 2 faz a mesma tarefa.

A "pessoa" é simulada (`a_person_would_approve`): ela aprova uma nota que diz `MAXTRY` em `deploy/limits.cfg` e que
`config/app.toml` não é lido, e recusa o resto. Na primeira rodada do haiku e na do Codex ela aprovava tudo; essa
rodada do haiku foi descartada (ver Limites).

## Resultados

| Modelo         | Cenário | Isca tocada: execução 1 → execução 3 | Arquivo certo na execução 3 | Propôs nota depois do follow-up |
| -------------- | ------- | ------------------------------------ | --------------------------- | ------------------------------- |
| Claude haiku   | K       | 3/3 → **0/3**                        | 3/3                         | 3/3                             |
| Claude haiku   | N       | 3/3 → 2/3                            | 3/3                         | 3/3                             |
| Codex luna (1) | K       | 3/3 → **0/3**                        | 3/3                         | **3/3**                         |
| Codex luna (1) | N       | 3/3 → 1/3                            | 2/3                         | 1/3                             |
| Codex luna (2) | K       | 3/3 → **0/3**                        | 3/3                         | **3/3**                         |
| Codex luna (2) | N       | 3/3 → 3/3                            | 3/3                         | 0/3                             |

(1) é a primeira rodada do Codex, (2) é a repetição depois das correções descritas abaixo.

1. **Uma correção vira uma nota que resolve.** Nos três lotes, depois da correção e da triagem da pessoa, a execução
   seguinte **não tocou a isca nenhuma vez (0 de 9)**. Com os mesmos fatos ditos sem cara de correção, tocou em 6 de 9.
2. **O pedido de nota faz o Codex registrar a lição:** ele propôs em 6 de 6 depois de uma correção e em 1 de 6 sem
   ela. O haiku propôs nos dois casos, mas as notas depois da correção foram melhores (0/3 contra 2/3 na isca).
3. **O haiku agora propõe notas:** 3 de 3 já na primeira execução. No M4 tinham sido 0 de 18; a diferença é a tarefa
   realista e a regra no fim do prompt. **Metade das notas da primeira execução é falsa** ("o limite é `retries` em
   `config/app.toml`"). Por isso a triagem da pessoa (Propor) é indispensável e o modo automático continua desligado.
4. **Recusa (R):** em 6 repetições, nenhum modelo repropôs a mesma lição recusada. As propostas da segunda execução eram
   lições diferentes, e foram perguntadas, como devem ser. Numa delas era a lição certa, que tinha o **mesmo título** da
   recusada ("Retry limit lives in three places"). A primeira versão da parecença comparava só o título e a teria
   descartado; a regra foi corrigida antes de rodar.
5. **Os padrões novos não quebraram nada:** 90 execuções completas de 90 (42 do haiku, 48 do Codex), com sandbox `auto`, Context Engine e Skills
   ligados.

## Problemas que os testes reais acharam (e foram corrigidos)

| Achado                                                                                                                                                                                         | Correção                                                                                                                                                       |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Com o sandbox `auto`, **os heredocs do zsh falhavam**. O zsh grava em `/tmp/zsh` (`TMPPREFIX`), não em `TMPDIR`. O Codex anotou "o shell não consegue criar arquivos temporários" (4 menções). | `TMPPREFIX` aponta para a pasta temporária da execução. Teste real do `sandbox-exec` (`zsh_heredoc`, `bash_heredoc`). Depois disso: 0 menções.                 |
| **Duplicatas reescritas:** depois de aprovar "MAXTRY em deploy/limits.cfg; app.toml não é usado", a execução seguinte propunha a mesma lição com outras palavras.                              | `submit` usa `Memory::resembles` também contra as notas guardadas. Na repetição, 3 duplicatas foram descartadas sem perguntar.                                 |
| **A parecença por título igual escondia lições contrárias** ("Use pnpm" e "Use npm" com o mesmo título).                                                                                       | A regra passou a ser: ≥ 60% das palavras do conteúdo em comum, ou ≥ 66% de título e conteúdo juntos, com pelo menos 3 palavras. O título sozinho nunca decide. |

## Sobre o "travamento" com OpenCode e Codex

O usuário notou o terminal travando enquanto o OpenCode e o Codex rodavam. O que os logs mostram:

- Todas as execuções terminaram (status `Completed`). Nenhuma estourou o tempo limite, e as lentas fizeram o mesmo
  trabalho que as rápidas (mesmos tokens, mesmas chamadas de ferramenta).
- As lentas (cerca de 240 s em vez de 25 s) coincidem com compilações e com a suíte inteira de testes Rust rodando em
  paralelo, que naquele momento também ficou 8 vezes mais lenta. O Claude, que rodou num intervalo sem compilação,
  ficou em 11–17 s.
- Não sobrou nenhum processo depois de interromper os testes no meio. O Atlas põe cada runtime no próprio grupo de
  processos (`process_group(0)`) e mata o grupo no fim (`kill_group`).

Conclusão provável: a máquina estava sobrecarregada por execução paralela (três CLIs mais compilação), e não há loop no
Atlas. **Não verificado:** uma execução isolada do Codex ou do OpenCode com a máquina ociosa. O usuário pediu para não
usá-los mais nos testes.

## Limites

- N=3 por cenário e uma tarefa só.
- A pessoa é simulada por uma regra de palavras.
- **Na primeira rodada do haiku** a pessoa aprovava tudo, inclusive notas falsas, e a execução 3 tocou a isca 2 de 3
  vezes. Essa rodada foi descartada e refeita com triagem.
- **Nas duas rodadas do Codex** a pessoa também aprovava tudo; ali as notas propostas depois da correção estavam certas.
- **OpenCode:** a rodada foi interrompida e não entra nos números.
- **Não medido com modelo real:** o efeito de "aceitar ou descartar o trabalho" na ordem das notas (`helped`/`misled`).
  Ele depende de decisões de merge que esta bancada não simula.
