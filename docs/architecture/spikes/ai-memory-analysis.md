# Análise — vale a pena trazer o ai-memory para o Atlas?

Data: 2026-10-20. Fecha a POC da [ADR 0040](../adr/0040-memoria-externa-ai-memory.md) depois dos estágios 0, 1 e 2 do
[experimento de captura](ai-memory-capture-experiment.md). Tudo abaixo foi medido nesta máquina, com CLIs e modelos
reais; o que é estimativa está dito.

## Veredito

**Em parte.** O que vale, o que não vale e o que rendeu mais do que a integração em si:

| Peça                                                | Vale?                                                   | Por quê, em uma linha                                                                                                                     |
| --------------------------------------------------- | ------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| Ler notas de um servidor ai-memory (opt-in)         | **Sim**                                                 | Evita o erro e corta tokens com 138 a 172 tokens por prompt e 0,8 ms de latência; é o mesmo ganho de uma nota certa, com busca que escala |
| A checagem de título (`precise`)                    | **Sim, e é a peça mais valiosa**                        | Tirou o ruído (87 notas de outro assunto em 40 prompts → 0) e **não depende do servidor**                                                 |
| Compartilhar notas aprovadas (escrever)             | **Só para quem usa o ai-memory com outras ferramentas** | Funciona e está provado, mas o benefício é entre ferramentas; para o Atlas sozinho, é complexidade sem retorno                            |
| Captura automática por hooks + consolidação por LLM | **Não, não agora**                                      | Rendeu só quando uma pessoa corrigiu; custou ~8× a sessão; deixou segredos soltos em texto puro no banco                                  |
| Embeddings (modelo de 87 MB)                        | **Não**                                                 | Não ajudaram e pioraram o top 3 nas tarefas diretas                                                                                       |
| O preset no MCP Hub que você pediu no começo        | **Não**                                                 | ~17 mil tokens de schema por prompt e o modelo precisa informar o escopo                                                                  |

**O melhor retorno não foi o servidor, foi o que a medição revelou na memória nativa do Atlas, e isso já foi
corrigido** (ver "Correção feita na memória nativa" abaixo): ela enchia o orçamento de 600 tokens em todo prompt a
partir de 15 notas, com cerca de 9 notas de outro assunto por prompt. Agora, com 240 notas, entram ~6 notas (~242
tokens), a nota certa está presente em 40 de 40 e há 0 notas de outro assunto em vez de 374 (medido contra o código
original, com ids embaralhados). É pouco código e nenhuma dependência.

## O que foi ganho, em números

### Efeito em execuções reais (tarefa com isca, 4 lotes no Claude haiku, 1 no Codex, 3 no OpenCode)

|                                                | Sem nota               | Com a nota do servidor            |
| ---------------------------------------------- | ---------------------- | --------------------------------- |
| Tocou o arquivo-isca                           | 22 de 23               | **0 de 27**                       |
| Alterou o arquivo certo                        | 19 de 23               | 25 de 27                          |
| Claude haiku, input                            | 61 mil a 88 mil tokens | 36 mil a 61 mil (-16% a -58%)     |
| Claude haiku, chamadas de ferramenta           | 8 a 10                 | 2 a 5                             |
| Notas geradas pelo próprio ai-memory (braço F) | isca 3/3               | isca 0/3, input -20%, output -36% |

O mecanismo é simples: o agente sem a nota edita a isca (`config/app.toml`) em quase toda tentativa, porque ela
parece o lugar certo. Uma nota que diz "é `MAXTRY` em `deploy/limits.cfg`" evita isso. **A mesma nota escrita à mão e
guardada na memória nativa já produzia esse efeito (braço B do M4).** O servidor não torna a nota mais eficaz; ele
muda onde ela mora e como é achada.

### O que o servidor traz que a memória nativa não traz

- **Busca que escala.** Mesmo corpus de 240 notas parecidas: servidor, nota certa no top 3 em 35 de 40 com 3 notas por
  prompt e 0 de outro assunto; nativa (código original), ~586 tokens por prompt com ~9 notas de outro assunto desde 15 notas; depois da correção, ~252 tokens e 9 notas de outro assunto em 40 prompts.
- **Compartilhamento entre ferramentas e máquinas** (provado no braço E: aprovada num Atlas, lida por outro).
- **Consolidação por LLM de correções em notas**, que funcionou (2 de 2 correções viraram notas corretas).
- Histórico em git das páginas, e uma wiki legível por humanos e por outras ferramentas.

Se você não usa o ai-memory com outras ferramentas, o primeiro item é o único que importa, e ele só pesa quando as notas
passam de algumas dezenas.

## O que custa

| Custo                            | Medido                                                                                                                                                                 |
| -------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Código de produção acrescentado  | ~820 linhas de Rust (leitura/escrita 516, cliente HTTP 266, registro 39) + 253 de TSX + 51 de script; mais ~850 linhas editadas em arquivos já existentes              |
| Testes                           | ~1 400 linhas de teste Rust (2 029 testes passando no projeto, 792 de frontend)                                                                                        |
| Superfície nova a manter         | cliente HTTP à mão, fila em thread, registro de publicados, reconciliação, 3 comandos, uma tela, um script e um servidor externo que o Atlas não supervisiona          |
| Latência por execução            | 0,8 ms (mediana) para ler; escrita fora do caminho (fila)                                                                                                              |
| Dinheiro, captura e consolidação | **US$ 0,91** para consolidar 9 sessões (22 chamadas, 52 s cada, 134 mil tokens de entrada, 130 mil de saída): ~US$ 0,10 por sessão contra ~US$ 0,012 da própria sessão |
| Dependência                      | um binário de ~40 MB compilado do fonte (4 minutos) que você precisa subir e manter                                                                                    |

**Ponto de equilíbrio da consolidação (estimativa).** Uma nota consolidada custa ~US$ 0,10. Num modelo barato como o
haiku, uma nota certa economiza ~US$ 0,004 a 0,01 por execução em tokens (a economia maior é a mudança errada que não
acontece, que não tem preço em dólares aqui), então se paga em 10 a 25 execuções que a usem. Em modelos 5 a 15 vezes
mais caros, em 1 a 3. Isso só vale se a lição for realmente reutilizada.

## O que deu errado ou preocupa

1. **A captura sem correção aprende o erro.** Das 6 sessões sem correção, 5 editaram o lugar errado e os resumos
   gerados descrevem isso como fato. Só as 2 sessões em que a pessoa disse "está errado, é MAXTRY" renderam lição. A
   lição vem da correção humana, não da sessão. (Por isso o Atlas ignora `sessions/…` na leitura.)
2. **Segredos soltos vazam para o banco.** Os 5 segredos no formato `NOME=valor` foram redigidos, os 4 sem nome de
   variável (senha em prosa, código de uso único, hex e alfanumérico soltos) ficaram em texto puro em `observations` e
   no índice de busca de observações, e são o que a consolidação envia ao LLM. Não chegaram às páginas, mas ficam no
   disco sem criptografia. Quem liga os hooks do ai-memory aceita isso.
3. **A consolidação é cara e lenta para o que entrega**: ~8× o custo da sessão, 80 a 160 s por sessão, e duas notas
   quase iguais para uma mesma lição.
4. **O `auto_improve` não rendeu nada** aqui (pula sessões curtas; relaxado, "nenhuma proposta"). Pode ser o meu
   adaptador; não consigo separar.
5. **Os embeddings não ajudaram**, e o `/admin/search` que o Atlas usa nem os usa.

## Limites desta evidência (o que não dá para concluir)

- **Uma tarefa, um projeto sintético.** Todos os resultados de execução vêm da tarefa do M4 (uma isca, uma nota exata).
- **Poucas rodadas por lote** (3 a 5). Os efeitos de isca são fortes (0 de 27 contra 22 de 23), os de tokens variam
  (-16% a -58% no mesmo modelo).
- **O corpus de 240 notas é sintético e homogêneo.** Ele é duro para a nativa (palavras comuns em tudo) e para os
  embeddings; notas reais, mais variadas, podem beneficiar a nativa e os vetores. O benchmark prova a _forma_ do
  problema (sem peso por raridade, a nativa degrada com o número de notas), não os números absolutos que você veria.
- **Capturei com o haiku.** A qualidade da consolidação com outro modelo, e a captura em sessões longas e reais, não
  foram medidas.
- **Um usuário, uma máquina, macOS.**

## Recomendação

1. **Feito:** a poda de inundação na memória nativa (radicais, título, raridade da palavra). Veja abaixo.
2. **Mantenha a leitura do servidor e a escrita como recursos opt-in e desligados**, para quem já roda o ai-memory. Não
   invista mais nelas até haver um segundo usuário com outras ferramentas.
3. **Não ligue a captura automática (hooks).** Se quiser captura, o que funcionou foi a **correção**: um botão no Atlas
   "transformar esta correção em nota", com a aprovação que já existe (ADR 0037), custa uma chamada curta em vez de ~8
   vezes a sessão, e nunca guarda segredo solto porque nunca guarda a sessão inteira.
4. **Não baixe os embeddings para o Atlas.** O modelo de 87 MB foi baixado só para este teste e fica na pasta temporária.
5. **Se quiser mais evidência antes de decidir o item 2:** rodar o benchmark e o braço E com notas reais suas (sem o
   corpus sintético) e com um modelo mais caro; é barato e responde se o servidor se paga no seu uso.

## Correção feita na memória nativa

`application/memory/recall.rs` e `domain/memory.rs`, emenda à [ADR 0037](../adr/0037-memory.md). A regra, sem
configuração nova:

- As palavras passam a ser comparadas por **radical** (`retry` com `retries`, `log` com `logging`), nas notas e na
  tarefa. O radical só corta quando sobram 4 letras ou mais e guarda o `e` final de palavras curtas, para `pipe` não
  virar `pip` nem `rate` virar `rat`.
- Se **até 5** notas compartilham uma palavra com a tarefa, ou a memória tem **menos de 12** notas, nada muda: todas
  ficam, como sempre foi. (Uma memória pequena e concentrada num tema, 6 de 8 notas sobre "build", não é inundação.)
- Se **mais de 5** compartilham, numa memória de 12 notas ou mais, é uma inundação e só ficam as que são **sobre** a
  tarefa: o título traz duas palavras dela (ou a única que ela tem), **ou** a nota compartilha uma palavra rara (no
  máximo 1 nota em 20) ou uma palavra incomum do próprio título, **e** pontua pelo menos 60% do melhor; **ou** a nota
  compartilha **duas** palavras incomuns com a tarefa (no título ou no conteúdo) e pontua pelo menos 40% do melhor.
  Essa última regra veio das notas reais (abaixo): sem ela a poda perdia notas cujo título não repete a tarefa.
- A pontuação pesa cada palavra pela raridade e **não conta as palavras que mais de um quarto das notas têm** (em uma
  memória de 20 notas ou mais): "change" e "service" não distinguem a nota certa das outras. É feita em números
  inteiros (milésimos), para dar a mesma resposta em qualquer plataforma.
- O que fica é ordenado em décimos do melhor (diferença menor que isso é ruído), depois pelo tipo (armadilha antes de
  how-to antes de decisão) e pela origem (escrita pela pessoa antes de proposta), e então pelo **uso** e pela recência.
- O que fica de fora conta como "sem relação" no manifesto. Uma tarefa que nada casa de verdade não recebe nenhuma nota.

### Dois erros de medição meus, e como ficou o resultado honesto

1. **Ids que flatearam o benchmark.** A primeira medição parecia ótima (a nota certa entre as 3 primeiras em 39 de 40
   casos). Era artefato: as notas certas tinham id `gold-…` e as de enchimento `noise-…`, e o último critério de
   desempate do ranking é o id, em ordem alfabética. O benchmark agora embaralha os ids, e o código original (o commit
   `HEAD`, num diretório separado) foi medido no mesmo benchmark.
2. **Uma soma errada** num resumo anterior (controle em "arquivo certo": eram 19 de 23, não 14 de 20).

A revisão independente apontou, além disso, que as constantes (60%, 25%, 5%, o mínimo de 12 e de 20 notas) foram
**ajustadas no mesmo corpus em que são medidas**, um corpus sintético gerado por modelos de frase, onde todas as notas
são do mesmo tipo e nenhuma tem uso. Por isso há agora também uma memória de 24 notas **escrita à mão** com 6 tarefas
(`a_hand_written_memory_finds_what_each_task_needs_and_little_else`), que não foi usada para ajustar nada, mais testes
de borda (5 contra 6 casamentos, 11 contra 12 notas, tipos misturados, uma nota com uma só palavra no título). Isso
reduz o risco de sobreajuste, mas não o elimina: **não há ainda um conjunto de notas reais de usuários.**

### Antes e depois (`native_bench_tests`, 240 notas, orçamento de 600 tokens, ids embaralhados, sem modelo)

|                                                         | Antes (código original) | Depois (regra final) |
| ------------------------------------------------------- | ----------------------- | -------------------- |
| Tokens por prompt, tarefas diretas                      | 586                     | **273**              |
| Nota certa no prompt, tarefas diretas                   | 39/40                   | **40/40**            |
| Nota certa entre as 3 primeiras, diretas                | 3/40                    | **13/40**            |
| Notas de outro assunto, diretas (40 prompts)            | 374                     | **32**               |
| Nota certa no prompt, parafraseadas                     | 4/40                    | **19/40**            |
| Notas de outro assunto, parafraseadas                   | 561                     | **128**              |
| Tokens por prompt, parafraseadas                        | 585                     | **224**              |
| Orçamento de 150 tokens, nota certa no prompt (diretas) | 3/40                    | **14/40**            |

| Por tamanho da memória (tarefas diretas) | Tokens: antes → depois | Nota certa no prompt | Outro assunto |
| ---------------------------------------- | ---------------------- | -------------------- | ------------- |
| 15 notas                                 | 462 → 217              | 4/4 → 4/4            | 33 → 7        |
| 46 notas                                 | 590 → 222              | 8/8 → 8/8            | 81 → 0        |
| 99 notas                                 | 587 → 265              | 16/16 → 16/16        | 149 → 8       |
| 145 notas                                | 587 → 273              | 24/24 → 24/24        | 227 → 19      |
| 240 notas                                | 586 → 273              | 39/40 → 40/40        | 374 → 32      |

A regra das "duas palavras incomuns" custou, no corpus sintético, mais notas de outro assunto do que a versão anterior
(diretas: 9 → 32; parafraseadas: 80 → 128) em troca de recuperar notas reais (próxima seção). É um trade-off, não um
ganho livre.

**Sobre a ordem.** Entre notas do mesmo assunto não há sinal de palavras que aponte a nota com a resposta. A ordem
passou de "contra a resposta" (3/40) para "perto do acaso" (13/40). O que o Atlas tem é o **uso**, e agora ele decide os
quase-empates; o benchmark não tem uso, então não mostra esse ganho, mas há testes unitários que mostram.

## Com notas reais do projeto (não geradas)

A memória nativa do Atlas **nunca foi usada no app de verdade**: a pasta `memory/` nem existe na pasta de dados. Então
não havia notas do usuário no Atlas para medir. Em vez disso, montei o conjunto a partir do que o projeto já registrou:
**46 notas** que condensam fielmente as lições dos arquivos de memória do projeto e dos ADRs (36 em inglês e 8 em
português, como nas fontes), e **18 tarefas** com o texto de commits reais e pedidos do dia a dia. Para cada tarefa, as
notas que um desenvolvedor gostaria de ver foram anotadas **antes** de rodar (26 no total; 3 tarefas sem nenhuma nota
útil, para medir o que entra à toa). As anotações são julgamento meu; o conjunto está em
[`ai-memory-bench/real_notes_corpus.json`](ai-memory-bench/real_notes_corpus.json) e o teste é
`real_notes_tests` (`ATLAS_REAL_CORPUS=… cargo test application::memory::real_notes_tests -- --ignored --nocapture`).

| 46 notas, 18 tarefas                     | Código original | Primeira regra | **Regra final** |
| ---------------------------------------- | --------------- | -------------- | --------------- |
| Notas esperadas encontradas              | **23/26 (88%)** | 18/26 (69%)    | 21/26 (81%)     |
| Notas a mais por tarefa com resposta     | 5,7             | **1,7**        | 2,2             |
| Tokens de notas por tarefa               | 375             | **155**        | 191             |
| Notas trazidas em 3 tarefas sem resposta | 10              | 8              | 8               |

**O que isso mostra.**

- A primeira regra, ajustada no corpus sintético, **perdia 5 das 26 notas** que um desenvolvedor queria. No sintético
  isso não aparecia (100% das notas certas) porque os títulos das notas geradas repetem o assunto; nas notas reais o
  título quase nunca repete as palavras da tarefa.
- A regra final recupera 3 delas (81%) por mais 0,5 nota a mais por tarefa, e ainda traz **metade dos tokens** e **39%
  das notas extras** do código original. Perde, em relação ao original, 2 notas de 26 (88% → 81%).
- Escolhi entre **três variantes** da regra olhando para este mesmo conjunto (sem corte: 22/26 com 3,2 extras por tarefa;
  corte de 60%: 18/26; corte de 40%: 21/26 com 2,2). Isso é ajuste no conjunto de teste, com apenas 18 tarefas; o número
  não é uma previsão para o uso real.
- Cinco notas continuam perdidas: duas da tarefa do celular (nenhuma palavra em comum de verdade), uma de exportação
  (o conteúdo diz "export" e a tarefa diz "import"), uma que só compartilha "sandbox" e uma de "timing tests" (a menos
  relacionada). Algumas das minhas anotações são generosas.
- Em português a poda ajuda menos: o radical é inglês, e as 8 notas em português dependem de palavras exatas.

### Com modelo real (Claude haiku, código final, 3 rodadas por braço)

|                                                                  | Sem memória | Memória nativa com 240 notas | Servidor ai-memory (ciclo compartilhado) |
| ---------------------------------------------------------------- | ----------- | ---------------------------- | ---------------------------------------- |
| Tocou a isca                                                     | 3/3         | **0/3**                      | **0/3**                                  |
| Arquivo certo                                                    | 2/3         | 3/3                          | 3/3                                      |
| Input (mediana)                                                  | 73 037      | **50 750 (-31%)**            | **48 508 (-34%)**                        |
| Output                                                           | 1 486       | **781 (-47%)**               | **749 (-50%)**                           |
| Chamadas de ferramenta                                           | 8           | **3**                        | **4**                                    |
| Bloco de notas no prompt (tokens estimados, com o aviso de ~300) | 0           | 753 (10 notas, ~443 delas)   | 147 (1 nota)                             |

Repetição depois da última regra (as duas palavras incomuns), mesma tarefa, 3 rodadas por braço: sem memória, isca
tocada 3/3 e input 74 558; memória nativa com as 240 notas, isca **0/3**, input **37 995 (-49%)**, output 1 830 → 733
(-60%) e 2 chamadas de ferramenta contra 9.

A tarefa deste teste é o pior caso do corpus para a memória nativa: "retry" é palavra de **três** assuntos (limite,
atraso de retentativa, tentativas de webhook), então entram 10 notas em vez das ~6 da média (252 tokens). Mesmo assim o
resultado foi o esperado: a isca não foi tocada em nenhuma das 6 execuções com memória.

### O que a correção não resolve

- A ordem entre notas do mesmo assunto só melhora com uso acumulado; sem ele é perto do acaso.
- Paráfrase: 19 de 40 e ~2 notas de ruído por prompt (antes 4 de 40 com 14 de ruído). Só embeddings melhorariam, e não
  ajudaram aqui.
- Numa memória de 12 a 20 notas concentrada num tema e com uma tarefa que só traz esse tema e um verbo, a poda pode
  tirar notas que serviriam. Abaixo de 12 notas isso não acontece.
- O radical é inglês e cru, e o benchmark é em inglês. Os limiares foram ajustados num corpus sintético (ver acima).
- A comparação no limite entre duas faixas de décimos é um degrau: 0,949 do melhor cai na faixa 9 e 0,95 na 10.
