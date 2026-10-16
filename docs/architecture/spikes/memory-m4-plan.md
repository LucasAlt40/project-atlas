# M4 da Memória: o plano da medição com modelo real

Data: 2026-10-15. Status: **preparado, ainda não executado** (custa tokens). Baseia-se na [ADR 0037](../adr/0037-memory.md),
fase M4.

## Pergunta

A Memória faz uma execução real gastar menos tokens e errar menos? E o ciclo inteiro funciona: um agente deixa uma nota,
a próxima execução a usa? Hoje tudo isto está construído, desligado por padrão e sem nenhum número. Sem o número, não se
diz que otimiza.

## Experimento

Um projeto pequeno, de propósito com uma **isca**: `config/app.toml` tem `retries = 3`, que nada lê; o limite de verdade é
`MAXTRY` em `deploy/limits.cfg` (lido do ambiente por `src/client.py`). A tarefa é a mesma em todos os braços: _"Mude o
limite de tentativas deste serviço de 3 para 5, depois responda só DONE."_ Cada execução começa de um mundo novo: repositório
Git temporário, worktree isolado, guard e CLI reais, sandbox desligado (já foi verificado à parte).

| Braço           | O que é                                                                                                                                                          | O que mede                                      |
| --------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------- |
| **A, controle** | Memória desligada                                                                                                                                                | o custo e a taxa de erro de base                |
| **B, nota**     | Memória ligada no workspace, com **uma** nota que a pessoa escreveu: "o limite é `MAXTRY` em `deploy/limits.cfg`; `retries` do `app.toml` não é lido"            | o melhor caso: a nota certa, na hora certa      |
| **C, o ciclo**  | Memória em `Auto`. A execução 1 não sabe nada e pode deixar uma nota (`atlas-memory`); o projeto volta ao estado inicial; a execução 2 recupera o que a 1 deixou | o produto de verdade: o agente aprende sozinho? |

As execuções de A e B são **intercaladas** (A1, B1, A2, B2…), para que uma hora lenta ou uma atualização do modelo não
caia num braço só. `ATLAS_M4_N` (padrão 3) é o número de repetições por braço.

### O que se mede, por execução

- tokens de entrada, saída e em cache (os que a CLI reporta), e o custo quando ela reporta;
- chamadas de ferramenta (linhas "Using …" do registro), e segundos;
- **acertou**: `MAXTRY=5` em `deploy/limits.cfg` do projeto depois do merge; **tocou na isca**: `app.toml` mudou;
- quanto a Memória acrescentou ao prompt (tokens estimados da seção de notas: a nota e a regra de proposta);
- quantas notas o agente propôs, quantas entraram sozinhas e quantas a execução 2 viu.

A saída é uma tabela de **medianas** por braço (três execuções são poucas demais para uma média significar algo), impressa
e, com `ATLAS_M4_OUT=<arquivo>`, anexada em Markdown.

### O que o teste afirma (e o que não)

Afirma só os invariantes: o controle não carrega notas; o braço B carrega a nota; se uma nota entrou sozinha na execução 1,
a 2 a recupera. **Não afirma que a Memória ajuda**: é uma medição, o resultado é o número.

## O que já foi provado sem custo

O mesmo teste rodou com uma CLI de mentira (esperta quando a nota está no prompt, ingênua quando não, e que deixa uma nota
quando a regra de proposta está presente): o encanamento inteiro funciona, a tabela sai e o ciclo fecha (a execução 1 deixa
uma nota, a 2 a recebe). Isso achou um erro do próprio teste (a sonda de arquivos era falsa e marcava a nota como
desatualizada).

## Custo estimado

Por execução, do que as CLIs reportaram nas verificações do sandbox: opencode 12–48 mil tokens de entrada (modelo gratuito);
claude haiku 12–23 mil (US$ 0,016 por execução); codex 14–66 mil; antigravity 18–37 mil. Com `N=3` são **12 execuções por CLI**
(A×3, B×3, C×3 de 2 execuções).

| CLI                     | Execuções | Entrada estimada          | Dinheiro                                         |
| ----------------------- | --------- | ------------------------- | ------------------------------------------------ |
| opencode (big-pickle)   | 12        | ~300 mil                  | grátis                                           |
| claude (haiku)          | 12        | ~220 mil, ~5 mil de saída | ~US$ 0,20                                        |
| codex (gpt-5.6-luna)    | 12        | ~420 mil                  | consumo do plano ChatGPT, não medido em dinheiro |
| antigravity (flash low) | 12        | ~300 mil                  | consumo da conta Google, idem                    |

Em **etapas**: a etapa 1 é opencode e claude (~520 mil tokens, ~US$ 0,20). Só se ela mostrar algo vale a etapa 2, com codex e
antigravity (~720 mil). Total das duas etapas: ~1,2 milhão de tokens de entrada, em boa parte em cache.

## Como rodar (um de cada vez, pelo nome exato; nunca com `--ignored` amplo)

```bash
cd src-tauri
ATLAS_M4_N=3 ATLAS_M4_OUT=/tmp/m4.md cargo test --lib application::executions::memory_experiment_tests::memory_experiment_opencode -- --ignored --exact --nocapture
ATLAS_M4_N=3 ATLAS_M4_OUT=/tmp/m4.md cargo test --lib application::executions::memory_experiment_tests::memory_experiment_claude -- --ignored --exact --nocapture
```

(`memory_experiment_codex` e `memory_experiment_antigravity` para a etapa 2.)

## Limites (para não tirar conclusão demais)

- **Uma tarefa, um projeto, três repetições.** É um indício, não uma prova; serve para decidir os padrões, não para prometer
  uma economia.
- **A tarefa foi desenhada para a Memória ajudar** (isca e dica certa). O braço B é o melhor caso; se nem ele ajudar, a
  Memória não ajuda aqui, e se ajudar, isso é o teto, não a média de um uso real.
- **Custo fixo da Memória:** a regra de proposta (~150 tokens) vai em **todo** prompt em que as propostas estão ligadas,
  mesmo sem nota. O experimento mede isso; se a economia por execução for menor que esse custo, a regra precisa encolher
  ou só entrar quando houver motivo.
- O modelo pode não propor nenhuma nota no braço C; isso também é um resultado (taxa de proposta).
- A isca depende do modelo ler os arquivos certos. Um modelo que acerta de primeira sem a nota (A já ótimo) mostra que a
  Memória não tinha o que ganhar nesta tarefa.

## O que decidir com o resultado

- **A e B empatam em acertos e tokens** → a Memória não paga o próprio custo nesta tarefa; manter desligada por padrão e
  reduzir a regra de proposta.
- **B bem melhor que A** → vale ligar `Propose`; o tamanho da cota e da validade se calibra pelo uso real depois.
- **C2 perto de B** → o ciclo funciona; o modo `Auto` é defensável. **C2 perto de A com notas propostas** → o ruído da
  nota gerada pelo modelo atrapalha; manter `Propose`.
- **C1 sem propostas** → o agente não aprende sozinho com esta regra; reescrever a instrução antes de falar em `Auto`.
