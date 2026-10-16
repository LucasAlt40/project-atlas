# M4 da Memória: resultados da etapa 1 (opencode e claude)

Data: 2026-10-15. Plano: [memory-m4-plan.md](memory-m4-plan.md). Três repetições por braço, uma tarefa, um projeto com
isca (`config/app.toml` não é lido; o limite está em `deploy/limits.cfg`). Medianas. **Indício, não prova.**

## opencode (`opencode/big-pickle`, grátis) e claude (`haiku`)

| CLI      | braço        | execuções | acertou o arquivo | mexeu na isca | entrada (mediana) | em cache | saída   | chamadas de ferramenta | notas no prompt (tokens est.) | segundos |
| -------- | ------------ | --------- | ----------------- | ------------- | ----------------- | -------- | ------- | ---------------------- | ----------------------------- | -------- |
| opencode | A controle   | 3         | 3/3               | **3/3**       | 97 151            | 71 808   | 2 794   | n/d                    | 0                             | 33       |
| opencode | B nota       | 3         | 3/3               | **0/3**       | 90 162            | 67 259   | 2 225   | n/d                    | 290                           | 52       |
| opencode | C execução 1 | 3         | 3/3               | 2/3           | 83 465            | 56 384   | 3 301   | n/d                    | 153                           | 30       |
| opencode | C execução 2 | 3         | 3/3               | 3/3           | 100 022           | 83 968   | 3 002   | n/d                    | 153                           | 27       |
| claude   | A controle   | 3         | 2/3               | **3/3**       | 74 130            | 65 643   | 1 444   | 9                      | 0                             | 17       |
| claude   | B nota       | 3         | **3/3**           | **0/3**       | **36 320**        | 29 140   | **532** | **2**                  | 290                           | **9**    |
| claude   | C execução 1 | 3         | 1/3               | 3/3           | 62 346            | 53 769   | 1 438   | 7                      | 153                           | 17       |
| claude   | C execução 2 | 3         | 2/3               | 3/3           | 75 697            | 70 612   | 1 752   | 9                      | 153                           | 18       |

O opencode não registra chamadas de ferramenta no mesmo formato (n/d). Tokens somados: opencode ~1,3 milhão de entrada
(grátis, 20 minutos, com uma execução atípica de 12 minutos); claude ~755 mil de entrada, a maior parte em cache.

## O que os números dizem

1. **Uma nota certa, na hora certa, vale muito no claude.** Com a nota (B) o claude gastou **metade da entrada** (36 mil
   contra 74 mil), **um terço da saída**, 2 chamadas de ferramenta em vez de 9, na metade do tempo, acertou o arquivo
   3 de 3 e **nunca mexeu na isca**. Sem nota ele mexeu na isca em todas as execuções e errou o arquivo certo em uma.
   O custo da nota foi de ~290 tokens: insignificante perto da economia.
2. **No opencode a economia de tokens não aparece** (90 mil contra 97 mil, dentro do ruído: o controle foi de 81 a 141 mil),
   mas o efeito sobre o **acerto** é o mesmo: 0 de 3 mexeram na isca com a nota, 3 de 3 sem ela. O prompt-base do opencode
   (~80 mil) dilui qualquer economia pequena.
3. **O ciclo de aprendizado não começou.** Nas 6 execuções 1 do braço C (3 de cada CLI), em modo `Auto`, **o agente não
   propôs nenhuma nota**, embora a regra estivesse no prompt. Por isso a execução 2 não viu nada (C2 ≈ A). O modo
   `Auto` hoje não funciona de ponta a ponta com estes modelos e esta regra; no braço B só uma de seis execuções propôs algo.
4. **A regra de proposta custa ~153 tokens em todo prompt e, até agora, não rendeu nada.**

## O que isto muda

- A Memória **funciona como uma dica certa que a pessoa escreve ou aprova** (braço B): vale ligar `Propose` para quem tem
  lições conhecidas.
- O `Auto` **não está pronto**: a instrução de proposta precisa de trabalho antes de qualquer padrão `Auto`. É um experimento
  barato de iterar no opencode (grátis): reescrever a regra e repetir só os braços C.
- Enquanto a regra não render propostas, ela é custo sem benefício; deve só entrar quando houver motivo (por exemplo, no
  fim de um passo que falhou e depois deu certo) ou ficar mais curta.

## Limites deste resultado

- **Melhor caso:** a nota é exata e a tarefa foi desenhada para ela. A economia de um uso real será menor.
- **Três execuções por braço, uma tarefa, um projeto.** Sem teste estatístico; a variação do opencode (81 a 141 mil de
  entrada no controle) mostra o tamanho do ruído.
- O braço C mediu o ciclo, mas como nada foi proposto, ele não diz se uma nota gerada pelo agente seria tão boa quanto a
  escrita à mão.
- Codex e antigravity (etapa 2) não foram rodados.

## Rodada 2: regra de proposta reescrita (só os braços C, opencode, grátis)

**Por que a regra antiga falhava.** Ela dizia que a nota era opcional ("você pode…") e a tarefa pedia "responda só DONE": o
modelo obedeceu à pessoa e omitiu o bloco. **A regra nova** (`memory/proposal.rs::RULE`) diz _quando_ uma nota é devida (algo
custou uma busca ou uma tentativa), mostra o formato uma vez, avisa que o bloco é tirado da resposta (então nunca conflita com
um formato de resposta pedido) e só dispensa o bloco quando não há nada que valha guardar. Custa ~203 tokens por prompt
(eram ~153). O exemplo da regra não tem relação com a tarefa do experimento, para o modelo não copiá-lo.

Duas repetições do braço C (N=3 cada, 6 execuções por rodada) no `opencode/big-pickle`:

|                                     | rodada 1 (regra antiga) | rodada 2 (regra nova) | rodada 3 (regra nova, notas impressas) |
| ----------------------------------- | ----------------------- | --------------------- | -------------------------------------- |
| execuções 1 que deixaram uma nota   | 0 de 3                  | 1 de 3                | 1 de 3                                 |
| execuções sem nota que deixaram uma | 0 de 6                  | 3 de 5                | 3 de 5                                 |

(Contando as execuções 1 e as execuções 2 em que a nota ainda não existia. Antes da regra nova: nenhuma nota em 6 execuções,
nas duas CLIs; agora, cerca de 3 em cada 5 execuções sem nota deixam uma.)

**As notas que os agentes escreveram são boas.** Exemplo, escrito por conta própria na execução 1 e aceito pelo modo
automático:

> _Retry limit is not in config/app.toml_: O limite vem da variável MAXTRY (exportada de deploy/limits.cfg) lida por
> src/client.py; o `retries=3` de config/app.toml nunca é lido por src/config_loader.py, que só devolve name e timeout.

**O ciclo fechou uma vez de ponta a ponta** (rodada 3, repetição 1): a execução 1 deixou a nota; a execução 2 a recebeu
(373 tokens de notas no prompt), **não mexeu na isca**, gastou **55 mil tokens de entrada contra 105 mil** da execução 1 e
**1,3 mil de saída contra 4,7 mil**. É um caso, não uma estatística: nas outras repetições a execução 1 não deixou nota e a
execução 2 não teve o que recuperar.

**O que isto muda.** O modo `Auto` agora funciona de ponta a ponta com este modelo; a taxa em que o agente propõe (~60%) e
o tamanho do ganho ainda precisam de mais repetições. A regra de ~203 tokens por prompt se paga se a nota economizar mais
que isso numa execução seguinte, e no caso que fechou o ciclo a economia foi de dezenas de milhares. Falta confirmar no
claude (o modelo em que a nota mais rendeu) e nos outros: `ATLAS_M4_ARMS=C`.

## Rodada 4: a regra nova no claude (haiku), braços C, N=3

| braço        | execuções | acertou | mexeu na isca | entrada (mediana) | saída | chamadas | notas no prompt (tokens est.) | segundos |
| ------------ | --------- | ------- | ------------- | ----------------- | ----- | -------- | ----------------------------- | -------- |
| C execução 1 | 3         | 3/3     | 3/3           | 74 857            | 1 559 | 9        | 203                           | 19       |
| C execução 2 | 3         | 2/3     | 3/3           | 74 627            | 1 480 | 9        | 203                           | 16       |

**O claude (haiku) não propôs nenhuma nota em 6 execuções**, com a mesma regra que fez o opencode propor em cerca de 3 de
cada 5. Sem nota, a execução 2 é igual à execução 1 (nada a recuperar) e a Memória só custou os ~203 tokens da regra. O
`Auto` portanto **não é independente do modelo**: funciona no opencode e, até aqui, não no claude haiku.

Hipóteses (não verificadas, cada execução custa tokens): o modelo segue o "responda só DONE" da tarefa e ignora um bloco
que considera extra; ou a regra, que chega no meio do prompt do sistema do Atlas, tem pouco peso para ele; ou ele não
considera que "algo custou uma busca" (9 chamadas de ferramenta sugerem que sim).

**Diagnóstico (uma execução, resposta final impressa):** a resposta do claude foi exatamente `DONE`. **Ele não escreveu o
bloco**: não é um bloco ilegível que o Atlas perdeu, é a regra ignorada. Com 9 chamadas de ferramenta e uma isca em que
caiu, ele tinha o que registrar e, ainda assim, obedeceu ao formato que a tarefa pedia ("responda só DONE") em vez da regra
do sistema. O custo da execução foi de ~75 mil tokens de entrada (66 mil em cache).

A regra está na parte do sistema do prompt, longe da instrução. Um modelo que dá mais peso ao fim do prompt e à instrução
explícita da pessoa a ignora. Próxima hipótese a medir: pôr a regra **depois da tarefa**, no fim do prompt.

## Rodada 5: a regra no fim do prompt, depois da tarefa (braços C, N=3)

`Prompt::memory_rule` fecha o prompt, depois do `USER INSTRUCTION` (antes estava na parte do sistema).

| CLI            | execuções 1 que deixaram nota     | notas aceitas | execução 2 recebeu notas | entrada 1 → 2 (mediana) | saída 1 → 2   | mexeu na isca 1 → 2 |
| -------------- | --------------------------------- | ------------- | ------------------------ | ----------------------- | ------------- | ------------------- |
| opencode       | **3 de 3** (antes: 1 de 3)        | 4             | 3 de 3                   | 81 926 → 68 474         | 2 092 → 1 318 | 2/3 → 2/3           |
| claude (haiku) | **0 de 3** (6 execuções sem nota) | 0             | 0 de 3                   | 61 613 → 62 277         | 1 162 → 1 457 | 3/3 → 3/3           |

- **O ciclo fechou no opencode nas três repetições**: a execução 1 deixou nota(s) e a execução 2 as recebeu. A entrada caiu
  16% e a saída 37% na mediana, mas a isca continuou tocada em 2 de 3.
- **O claude continuou sem propor nada**, mesmo com a regra colada à tarefa. Mover a regra resolveu para um modelo e não
  para o outro.

### O achado que muda o desenho: uma nota automática pode ensinar o erro

Na repetição 2 do opencode, o agente da execução 1 mexeu na isca e gravou, **aceita sozinha pelo modo automático**:

> _Retry limit is configured in three places_: … `config/app.toml` (retries, but config_loader.load ignores it). **Change all
> three to keep them consistent.**

A execução 2 recebeu essa nota e **mexeu na isca de novo**. A nota nasceu de uma execução que "deu certo" (o arquivo certo foi
mudado), então passou em todas as condições do modo automático, mas o que ela ensina é o erro. As notas precisas (repetição 3:
"`app.toml` é código morto, mude `deploy/limits.cfg`") fizeram o contrário. Ou seja: **"o passo teve sucesso" não mede se a
lição é verdadeira**. Duas outras notas do opencode eram vagas ou duplicadas ("No retry-named string"; três notas quase iguais
sobre o mesmo tema), e a detecção de duplicatas só pega texto idêntico.

### Conclusões desta rodada

1. O lugar da regra importa (opencode: 1 → 3 de 3), mas **não basta** (claude: 0).
2. O `Auto` **não deve ser o padrão**: confirma a decisão da ADR de `Propose` como modo normal, em que a pessoa lê a nota
   antes de ela entrar. O `Auto` fica como escolha explícita, com o aviso de que a nota pode reforçar um erro.
3. A tarefa do experimento pede "responda só DONE", o que briga com qualquer bloco extra: pode ser parte do motivo da
   recusa do claude. Uma tarefa realista ("e diga o que mudou") é a próxima variável a testar.

## Rodada 6: claude haiku com a tarefa realista ("e diga brevemente o que mudou")

Braços C, N=3, regra no fim do prompt.

| braço        | execuções | acertou | mexeu na isca | entrada (mediana) | saída | chamadas | notas deixadas |
| ------------ | --------- | ------- | ------------- | ----------------- | ----- | -------- | -------------- |
| C execução 1 | 3         | 3/3     | 3/3           | 62 389            | 1 638 | 8        | **0**          |
| C execução 2 | 3         | 2/3     | 3/3           | 62 509            | 1 661 | 8        | 0              |

**Não há notas para mostrar:** o claude haiku não escreveu nenhum bloco `atlas-memory` em mais 6 execuções. Somando as rodadas,
são **0 notas em 18 execuções** com três variações da regra (no sistema, no fim do prompt, e com a tarefa pedindo um resumo
em vez de "só DONE"). O motivo, portanto, não é o formato de resposta pedido pela tarefa: o haiku, nesta configuração, não
segue a regra de proposta. Se isso é limite do modelo pequeno ou do jeito que a regra chega a ele, só outra medição diz
(por exemplo, um modelo maior do claude, uma execução).

Resumo do estado da Memória depois do M4:

- **Nota escrita ou aprovada por uma pessoa (`Propose`):** funciona e vale muito no claude (metade da entrada, um terço da saída,
  sem tocar na isca) e no opencode (sem tocar na isca).
- **Nota escrita pelo agente (`Auto`):** funciona no opencode (ciclo fechado 3 de 3, com a regra no fim do prompt), **não funciona
  no claude haiku** (0 em 18) e, quando funciona, a nota pode ensinar o erro.

## Rodada 7: uma execução no claude sonnet (tarefa realista, braço C execução 1, resposta impressa)

| modelo | acertou o arquivo | mexeu na isca | entrada | saída | chamadas | nota deixada            |
| ------ | ----------------- | ------------- | ------- | ----- | -------- | ----------------------- |
| sonnet | **não**           | sim           | 42 418  | 754   | 4        | **sim, aceita sozinha** |

**É limite do haiku (ou de modelo pequeno): o sonnet seguiu a regra e escreveu o bloco.** A resposta que a pessoa vê saiu limpa (o
bloco foi tirado, como a regra promete):

> I changed the retry limit from 3 to 5 in `config/app.toml:3`, so the line now reads `retries = 5`. A search of the project found
> no other retry setting, so this was the only place to change. I didn't run anything, so the change is untested.

**E a nota que ele deixou é falsa**, e o modo automático a aceitou:

> _Retry setting location_ (how-to): The service retry limit lives in config/app.toml as `retries`; no other file defines
> retries.

O agente caiu na isca (o limite real é `MAXTRY` em `deploy/limits.cfg`), declarou isso com confiança e o Atlas gravou como
lição. Todas as condições do `Auto` valeram (passo concluído, sem segredo, sem pretensão de autoridade, sem duplicata,
cabe na cota): **nenhuma delas mede se a lição é verdadeira**. A execução seguinte receberia a nota e tenderia a repetir o
erro. É a mesma falha da rodada 5 (uma nota que ensina o erro), agora num modelo que acerta o formato.

### Consequências para o desenho

1. **`Propose` fica como o modo normal e recomendado**; o `Auto` só como escolha explícita, com o aviso de que a nota pode ser
   falsa. A ADR 0037 já dizia isso; o experimento confirma.
2. Se o `Auto` for continuar, ele precisa de um sinal de que a lição **é verdadeira**, não de que o passo terminou. O candidato
   natural: só aceitar sozinha uma nota de um passo que um **validador independente** aprovou (um desfecho `pass` de outro
   agente do workflow), nunca de um passo que se avaliou sozinho.
3. Uma nota aceita sozinha deve ficar visivelmente "não confirmada" e ser a primeira a sair quando um passo posterior a
   contradiz (por exemplo, quando o agente muda o arquivo que a nota diz ser o errado).

## Consequência implementada: o portão do validador

As duas notas falsas (rodadas 5 e 7) levaram a mudar o modo `Auto` (ADR 0037, decisão 5b): uma nota que o agente propõe fica
**retida** e só entra quando um **validador independente** (outro agente, contrato de validação ou revisão, desfecho `pass`,
mesma execução do workflow) aprova; um `fail` a descarta; e uma nota que entrou assim segue **não confirmada** (marcada no
prompt, na lista e no Inspetor) até uma pessoa decidir. Foi provado sem custo, com testes do serviço, do orquestrador (agente
validador, outro agente que diz "pass" sem ser validação, o próprio autor tentando validar) e da execução. O experimento ganhou
`ATLAS_M4_VALIDATE=1`, que faz um validador simulado passar a execução 1 do braço C, para medir o ciclo quando existe validador;
sem ele as notas ficam retidas e a execução 2 não vê nada.

**O que não está provado:** que um validador real barra uma lição falsa. Um validador que só confere o arquivo pedido aceitaria a
nota do exemplo do sonnet (o limite foi mesmo mudado, no arquivo errado, mas o validador pode não perceber). O portão troca a
confiança no autor pela confiança em um segundo agente; reduz o erro, não o elimina.

## Rodada 8: codex e antigravity (A, B e C; N=3; tarefa realista; `ATLAS_M4_VALIDATE=1`)

### codex (`gpt-5.6-luna`)

| braço        | execuções | completas | acertou | mexeu na isca | entrada (mediana) | saída     | chamadas | segundos |
| ------------ | --------- | --------- | ------- | ------------- | ----------------- | --------- | -------- | -------- |
| A controle   | 3         | 3/3       | 2/3     | **3/3**       | 106 682           | 1 721     | 6        | 26       |
| B nota       | 3         | 3/3       | **3/3** | **0/3**       | **91 239**        | **1 180** | 5        | **18**   |
| C execução 1 | 3         | 3/3       | 3/3     | 3/3           | 125 347           | 2 145     | 7        | 30       |
| C execução 2 | 3         | 3/3       | 3/3     | 3/3           | 142 600           | 2 548     | 8        | 28       |

- **A nota ajuda também no codex**: acertou 3 de 3 e nunca tocou na isca (o controle tocou nas 3), com 14% menos entrada,
  31% menos saída e 30% menos tempo.
- **O codex segue a regra de proposta**, mas só às vezes: com a nota presente (braço B) propôs em 2 de 3 execuções, e as notas
  são boas ("MAXTRY em deploy/limits.cfg; `retries` de `app.toml` não é lido"). No braço C propôs na execução 2 (duas vezes) e
  **não na execução 1** (0 de 3). Com nada para recuperar, o ciclo não fechou (o validador simulado não achou nada para
  confirmar na execução 1).

### antigravity (`gemini-3.8-flash-low`)

| braço        | execuções | **completas** | acertou | mexeu na isca | entrada (mediana) | saída |
| ------------ | --------- | ------------- | ------- | ------------- | ----------------- | ----- |
| A controle   | 3         | **0/3**       | 0/3     | 0/3           | n/d               | n/d   |
| B nota       | 3         | **2/3**       | 2/3     | 0/3           | 158 570           | 1 687 |
| C execução 1 | 3         | 0/3           | 0/3     | 0/3           | n/d               | n/d   |
| C execução 2 | 3         | 0/3           | 0/3     | 0/3           | n/d               | n/d   |

**A maioria das execuções do antigravity não terminou, e o motivo é do runtime, não da Memória nem do sandbox.** Reexecutado com o
motivo impresso (3 execuções do controle, todas falharam), a causa é sempre a mesma:

> _Antigravity stopped after a tool call failed, with the work unfinished: run_command (`python3 -c "…"`): permission check
> failed … user denied permission to run command._

O modelo decide **conferir o trabalho rodando um comando de shell** (`python3 -c`, para contar as tentativas), e o modo
headless do `agy` (`--mode accept-edits`) nega todo comando de shell. O `agy` então encerra, e o Atlas registra a execução
como falha com o trabalho incompleto. É o comportamento já documentado em `antigravity.rs` ("comandos de shell continuam
recusados"), que aqui aparece como falha porque o agente tenta um e a CLI não segue adiante. As execuções de trabalho do teste
do sandbox, que pedem só para criar um arquivo, completam.

**Efeito colateral visível:** com a nota (braço B) o agente não precisou explorar e conferir, e completou 2 de 3 contra 0 de 3
sem ela. Mas isso confunde Memória com a falha do runtime e não é um resultado limpo sobre a Memória. Nenhuma nota foi proposta
em execuções completas do braço C porque nenhuma completou.

### Gemini CLI

A conta do usuário é inelegível para o Gemini Code Assist (falha antes do primeiro pedido, medido de graça), então **o
Gemini CLI não pôde ser provado em nada**: nem Memória, nem sandbox. O que se afirma dele vem só do desenho (a estratégia de
confinamento tem teste de registro) e dos testes sem modelo.

## Matriz final do que foi provado com modelo real

| Runtime       | Nota certa reduz erro e custo                               | Agente propõe nota sozinho                 | Sandbox, trabalho normal             |
| ------------- | ----------------------------------------------------------- | ------------------------------------------ | ------------------------------------ |
| claude haiku  | **sim**: metade da entrada, 0/3 na isca                     | **não** (0 em 18)                          | **sim**                              |
| claude sonnet | não medido em A/B                                           | **sim** (mas escreveu uma nota falsa)      | não medido                           |
| opencode      | **sim** (isca 0/3 contra 3/3; sem economia de tokens)       | **sim**, no fim do prompt (3 de 3)         | **sim**                              |
| codex         | **sim**: isca 0/3 contra 3/3, -14% de entrada               | **às vezes** (2 de 3 com nota, 0 de 3 sem) | **sim** (com a troca do `--sandbox`) |
| antigravity   | confundido pela falha do runtime (2/3 contra 0/3 completas) | não verificável (quase nada completa)      | **sim**                              |
| gemini CLI    | não verificável (conta inelegível)                          | não verificável                            | não verificável                      |

## Rodada 9: GitHub Copilot CLI (`auto`; A, B e C; N=3; tarefa realista; `ATLAS_M4_VALIDATE=1`)

| braço        | execuções | completas | acertou | mexeu na isca | entrada (mediana) | saída | chamadas | segundos |
| ------------ | --------- | --------- | ------- | ------------- | ----------------- | ----- | -------- | -------- |
| A controle   | 3         | 3/3       | 1/3     | **3/3**       | 68 718            | 569   | 5        | 17       |
| B nota       | 3         | 3/3       | **3/3** | **0/3**       | **56 146**        | 501   | **3**    | 15       |
| C execução 1 | 3         | 3/3       | 1/3     | 3/3           | 69 762            | 533   | 5        | 18       |
| C execução 2 | 3         | 3/3       | 0/3     | 3/3           | 69 398            | 516   | 6        | 16       |

- **A nota ajuda no Copilot também**: acerta o arquivo 3 de 3 (controle: 1 de 3), nunca toca na isca (controle: 3 de 3), com 18%
  menos entrada e 40% menos chamadas de ferramenta.
- **O Copilot não propôs nenhuma nota** em 9 execuções sem nota (0 de 9), como o claude haiku: com o modelo `auto` a regra de
  proposta é ignorada. O ciclo do `Auto` não começa nele.
- Todas as execuções completaram (o Copilot nega comandos de shell, e o prompt já diz isso: `runs_commands = false`).
- O sandbox, trabalho normal (`sandbox_real_copilot_does_its_work`), passou com `sandbox:confined` (27 mil tokens de entrada).

### Matriz final (acrescenta o Copilot)

| Runtime          | Nota certa reduz erro e custo                    | Agente propõe nota sozinho | Sandbox, trabalho normal |
| ---------------- | ------------------------------------------------ | -------------------------- | ------------------------ |
| copilot (`auto`) | **sim**: 3/3 certo, 0/3 na isca, -18% de entrada | **não** (0 de 9)           | **sim**                  |

### Antigravity depois da correção

O agente tentava `python3 -c` para conferir o trabalho e o modo headless recusa o comando e encerra o turno. Duas mudanças:
o prompt deixa de dizer que ele pode rodar comandos (`ModelRuntime::runs_commands`, falso no antigravity) e o adaptador
**continua a mesma conversa** (`--conversation <id>`, no máximo duas vezes) dizendo que o comando foi recusado e que ele termine
só com as ferramentas de arquivo. Controle (A, tarefa realista, 3 execuções): **3 de 3 completas e 3 de 3 acertaram o arquivo**
(antes: 0 de 3 completas). Custo: cada execução ficou em 440 a 630 mil tokens de entrada (a conversa continuada soma o contexto),
muito acima das outras CLIs.
