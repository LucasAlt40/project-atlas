# ADR 0041 — Memória ligada por padrão e que aprende com as decisões da pessoa

Status: implementada em 2026-10-20, provada com testes sobre o runtime simulado (sem modelo real). Resolve a decisão
pendente "padrão do modo por escopo" da [ADR 0037](0037-memory.md) e acrescenta o ciclo de aprendizado.

## Contexto

A revisão da POC (ADR 0040 e a emenda da ADR 0037) mostrou um problema que nenhum número cobria: **a memória vinha
desligada e quem nunca a ligou não ganhava nada**. Na instalação do próprio autor não havia uma nota sequer. O que já
estava medido:

- **Uma nota aprovada por uma pessoa funciona.** Em quatro runtimes (claude haiku, opencode, codex, copilot), a isca foi
  tocada 0 de 3 vezes com a nota e 3 de 3 sem ela. No claude a entrada caiu à metade (M4).
- **Sem nota que combine com a tarefa, o custo é só a regra de proposta** (~203 tokens no fim do prompt). Desde a poda,
  uma memória cheia também não inunda o prompt.
- **O modo automático ensina erros.** Notas falsas passaram em todas as checagens (M4, rodadas 5 e 7).
- **Lições boas nascem de correções.** Na captura do ai-memory, saiu uma lição certa nas duas sessões em que a pessoa
  disse "está errado" e nenhuma nas outras seis.
- **A recusa se perdia.** Recusar uma proposta a apagava, e o agente podia propor a mesma coisa na execução seguinte.
- **Aceitar ou descartar o trabalho não chegava à memória.** O contador `uses` só registra que a nota foi para o prompt
  de um passo que terminou, e não diz se ajudou.

## Decisão

### 1. Padrão: "Propor, eu aprovo" em todos os escopos

`MemorySettings::recommended()` coloca User, Workspace e Agent em `Propose`, com teto de 600 tokens. `Auto` nunca é
padrão. As regras de migração:

- O campo `chosen` registra que a pessoa salvou os modos (`set_memory_settings` sempre o grava como `true`).
- Ao carregar a configuração (`adopt_recommended_memory`), quem nunca escolheu e tem tudo `off` passa ao recomendado,
  mantendo o teto de tokens.
- Uma escolha salva nunca é trocada, nem um modo diferente de `off`, mesmo que gravado antes de `chosen` existir.
- Limite: quem desligou tudo de propósito antes desta versão não se distingue de quem nunca mexeu, e passa a ver
  "Propor". Desligar de novo fica gravado como escolha.

### 2. Recusar é diferente de apagar

- Uma candidata recusada (`reject_memory`) vira `Rejected`. Ela fica guardada, nunca é recuperada, e o Atlas mantém no
  máximo 100 por dono (as mais antigas saem primeiro).
- `submit` descarta (`Discarded::Rejected`) uma proposta parecida com uma recusa do mesmo dono, sem perguntar de novo.
  "Parecida" (`Memory::resembles`) quer dizer, por radical: pelo menos 60% das palavras do conteúdo em comum, ou 66% de
  título e conteúdo juntos, com no mínimo 3 palavras. O título sozinho nunca decide, porque "Use pnpm" e "Use npm" sob o
  mesmo título são lições opostas. Uma lição e a sua negação nunca são parecidas ("é lido" e "não é lido" têm as mesmas
  palavras, porque "não" e "not" ficam de fora da comparação). Por isso o Atlas checa as negações à parte, em PT, EN e
  ES (`NEGATIONS`). A mesma regra descarta como **duplicata** a lição já guardada dita com outras palavras. A checagem
  é por palavras, não por significado.
- A pessoa pode mudar de ideia: **Aprovar mesmo assim**, ou **Esquecer** (apaga a recusa, e a lição pode voltar a ser
  proposta).

### 3. O trabalho que a pessoa aceita ou descarta conta para as notas que ele levou

- **Aceitar:** `merge_execution` com resultado `Merged`/`Applied`, ou `apply_workflow_changes` com o fluxo
  `Integrated`, soma `helped` em cada nota que foi **enviada** no prompt daquelas execuções. A lista vem do manifesto.
  Notas `ext:` ficam de fora, e uma nota presente em vários passos conta uma vez.
- **Descartar:** `discard_workflow_changes` soma `misled`.
- **Na ordem:** `trust = helped − 2 × misled` desempata depois da relevância e antes de uso e recência (errar pesa mais
  que acertar). A relevância continua mandando.
- **Fora de uso:** uma nota **de agente** com `misled ≥ 2` e `misled > helped` vira `Stale`, e a interface diz por quê
  ("estava em N execuções que você descartou"). Quando a pessoa a coloca em uso de novo, o `misled` volta a zero. Uma
  nota escrita pela pessoa só cai na ordem, nunca sai de uso por isso.
- **Não contam:** o merge automático (ninguém decidiu), o "manter isolado" depois de aplicar (desfazer não diz que a
  nota errou) e as execuções sem worktree, que não têm o que aceitar.
- **Limite conhecido:** aplicar um fluxo, desfazer ("manter isolado") e aplicar de novo soma `helped` duas vezes. A
  ordem só melhora um pouco a mais para essas notas; nada sai de uso por isso.

### 4. A correção da pessoa pede a nota

Quando as propostas estão ligadas e a mensagem da pessoa parece uma correção, a regra de proposta ganha mais uma
frase (`CORRECTION_NUDGE`, ~45 tokens): o que ela corrigiu é o que a próxima execução precisa saber. "Parece uma
correção" quer dizer que a mensagem contém uma expressão inteira como "está errado", "na verdade", "não funcionou",
"wrong", "actually" ou "en realidad". Se a regra errar, o custo é a frase a mais; nenhuma nota entra sem a pessoa.

## Consequências

- Quem abre o Atlas pela primeira vez já recebe as notas que escreveu ou aprovou, e os agentes que seguem a regra
  propõem notas novas. O custo, com ou sem nota, é a regra de proposta em todo prompt (~203 tokens; ~250 numa correção).
- O workspace aprende sem modelo extra. Ele para de perguntar o que você já negou, põe na frente as notas que estavam
  no trabalho que você aceitou e tira de uso as notas de agente que estavam no trabalho que você descartou. Tudo é
  determinístico, guardado em `memory/*.json` e visível na lista.
- **Medido com modelo real** ([resultados](../spikes/memory-learning-results.md), Claude haiku e Codex luna, N=3):
  - depois de uma correção e da triagem da pessoa, a execução seguinte tocou a isca **0 de 9** vezes; com os mesmos
    fatos ditos sem cara de correção, 6 de 9;
  - com o pedido de nota, o Codex propôs uma nota em 6 de 6; sem ele, em 1 de 6;
  - os testes também acharam e corrigiram as duplicatas reescritas (agora barradas por parecença) e lições contrárias
    escondidas pelo título igual.
- **Não medido com modelo real:** o efeito de aceitar ou descartar o trabalho na ordem das notas. O sinal "descartou o
  trabalho" é fraco, porque o descarte pode ter outro motivo; por isso só tira de uso uma nota de agente, e só depois
  de duas vezes.

## Testes

`domain::memory` (parecença, `trust`, `misleads`, ordem), `application::memory::service` (recusa, limite de 100,
descarte de proposta parecida, veredito, só notas enviadas contam), `application::memory::proposal` (correções em
PT/EN/ES e frases que não são correção), `application::config` (migração), `application::settings` (`chosen`),
`executions::memory_tests` (o ciclo de ponta a ponta no runtime simulado) e `MemorySettings.test.tsx` (Recusar, aba
de recusadas, motivo de "fora de uso").
