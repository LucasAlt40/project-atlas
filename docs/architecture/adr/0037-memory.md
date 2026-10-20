# ADR 0037 — Memory: lições duráveis entre sessões, propostas pelo agente e decididas pela pessoa

Status: proposto; **Fases M1, M2 e M3 implementadas** (M1: domínio, armazenamento, recuperação, prompt, manifest, comandos; M2: protocolo `atlas-memory`, candidatos, aprovação e modo automático; M3: painel nas Configurações e notas no Inspetor, em pt-BR e en-US). M4 pendente. Baseia-se na seção 6 de
[context-and-tooling-platform.md](../context-and-tooling-platform.md), na [ADR 0021](0021-context-engine.md) (Context
Engine), na [ADR 0025](0025-guardrails-and-context-review.md) (guardrails), na [ADR 0026](0026-context-and-tooling-platform.md)
(orçamento e manifest) e na [ADR 0027](0027-rules-context-authority-and-system-prompt-channel.md) (autoridade de contexto).

## Contexto

Um dos sete componentes de um harness é a memória: o que persiste entre sessões. Hoje o Atlas guarda o **conhecimento do
projeto** (Harness, `.atlas/`), o **estado de uma execução** (SharedExecutionState) e o **histórico**. Não guarda nada do
que os agentes _aprenderam fazendo_: "os testes deste repositório rodam com `npm run check`", "o Windows quebra com esse
escape", "esta pessoa quer commits em português". Cada execução redescobre isso gastando tokens (leituras, tentativas,
falhas) e repete os mesmos erros. `.atlas/memory/` é só um marcador que nada lê.

Memória, aqui, **não é** um segundo Harness (fatos do projeto continuam no Harness), nem Rules (instruções que a pessoa
escreve e que obrigam), nem histórico (transcrição). É uma lição curta, recuperável, com dono, origem e validade.

## Decisões

### 1. O que é uma Memory

`Memory { id, scope, kind, title, content, topic?, provenance, confidence, status, validity, uses, created_at,
updated_at, last_used_at, fingerprint }`.

- **Escopos**: `User` (entre projetos), `Workspace` (o que a seção 6 chama de Project, dobrado no Workspace) e `Agent`
  (privada por padrão, nunca compartilhada sozinha). Workflow continua sendo o SharedExecutionState.
- **Tipos** (`kind`): `how_to` (comando, ordem, ferramenta que funciona), `pitfall` (o que falhou e por quê),
  `preference` (como a pessoa quer o resultado), `decision` (uma escolha feita e a razão). Só isso: o tipo decide o
  enquadramento no prompt e a ordem sob orçamento, não permissões.
- **Tamanho**: `content` tem teto (480 caracteres). Uma lição que não cabe é um fato do projeto e vai para o Harness.
- **Proveniência**: `User` (escrita ou aprovada pela pessoa), `Agent` (proposta por um agente, com a execução e o passo
  de origem), `Imported`. Quem escreveu nunca promove a força: **memória não obriga**.

### 2. Memória nunca é instrução

Uma Memory entra no prompt como contexto de **autoridade `Untrusted`**, como a seção 16 já previa para "memórias": o
agente a usa como pista, a revisão a varre e uma memória que se declara regra, permissão ou aprovação é um achado
(`unknown_provenance` / reivindicação de autoridade), nunca uma promoção. Memória não concede nem amplia nada: o guard de
processos não lê prompt. Isto é o que impede um texto envenenado (vindo de uma página ou de um arquivo lido por um agente)
de virar "conhecimento" permanente com poder: ele pode, no máximo, ser uma dica rejeitável, e a pessoa decide antes que
entre (decisão 4), ou fica marcado como `Agent` no modo automático (decisão 5).

### 3. Armazenamento próprio, atrás de uma porta

Memória não vai no `config.json` (reescrito inteiro a cada mudança, já pesado) nem em `.atlas/` (viaja no repositório e
contaria como texto de terceiros). Uma porta `MemoryStore` e um primeiro adaptador de arquivo no diretório de dados do Atlas
(um arquivo por escopo-dono, escrita atômica, como o `spec_store`). Consequências: exportação/importação (ADR 0032)
inclui memórias só quando a pessoa marca, e uma memória do escopo Workspace é apagada com o workspace.

### 4. Como uma memória nasce: propostas com aprovação, e automático por escolha

Três modos, **por escopo e por workspace**, configurados pela pessoa (padrão: `off`, porque muda prompts, como
`contextEnabled` e `skillsEnabled`):

| Modo      | O que acontece                                                                                                                                                        |
| --------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `off`     | Nada é recuperado, nada é proposto, o prompt é idêntico byte a byte ao de antes.                                                                                      |
| `propose` | O agente propõe lições ao fim de um passo; cada uma vira um _candidato_ que a pessoa aprova, edita ou descarta.                                                       |
| `auto`    | Candidatos que passam em todas as verificações da decisão 5 ficam **retidos**; só entram quando um validador independente aprova (decisão 5b); o resto vira proposta. |

**Como o agente propõe, sem custo de um segundo modelo**: um bloco cercado `atlas-memory` na resposta final (mesmo molde
do `atlas-interaction`, ADR 0016), lido pelo adaptador do runtime, no máximo 3 por passo. A instrução que o pede só entra no prompt quando o modo do escopo não é `off`. A primeira versão (duas linhas, "você
pode") não fez nenhum agente propor; a atual diz quando uma nota é devida, mostra o formato e avisa que o bloco sai da
resposta (veja `spikes/memory-m4-results.md`), a ~200 tokens por prompt. Não há chamada extra de modelo para "extrair" memórias.
**Coleta determinística falha→sucesso: não construída.** Sem um modelo, o Atlas só vê _que_ uma tentativa falhou e
qual foi a mensagem de erro, não _o que mudou_ na tentativa seguinte: um candidato gerado dali seria "o passo X falhou com
Y e depois passou", que não ensina nada e enche a fila da pessoa. A lição, quando existe, é do agente, que a propõe no
bloco `atlas-memory`.

### 5. O que o modo automático nunca dispensa

Um candidato só entra sozinho se **todas** as condições valem; qualquer falha o rebaixa a proposta:

1. veio de um passo `completed` com desfecho de sucesso do contrato (nunca de um `fail`, de um passo interrompido ou de
   um `WaitingForInput`);
2. `redact_secrets` não achou nada (se achou, o candidato é descartado e o fato é registrado, não rebaixado);
3. a revisão de contexto não achou reivindicação de autoridade nem de segurança no texto;
4. não duplica nem contradiz uma memória ou rule existente do mesmo tópico (comparação exata de chave e fingerprint,
   sem julgamento semântico, como nas Rules); contradição vira proposta;
5. cabe no teto de tamanho e na cota do escopo (por padrão 40 por workspace, 20 por agente, 20 de usuário); passando da
   cota, a nota **não entra sozinha**: vira proposta. Nada que já está guardado é substituído nem apagado em silêncio
   (isto substitui a ideia inicial de expulsar a de menor valor);
6. tipo `preference` e escopo `User` **nunca** são automáticos: o que fala pela pessoa é aprovado por ela.

Memórias automáticas nascem com `confidence` baixa, validade (padrão 60 dias sem uso) e um selo visível na lista e no
Inspetor. A pessoa pode promover (vira `User`, sem validade), editar ou apagar a qualquer momento, e pode apagar todas as
automáticas de uma vez.

### 6. Recuperação determinística sob o orçamento

Sem vetores e sem RAG, como a seção 6 decidiu. Dado o escopo da execução (usuário, workspace, agente) e a tarefa:

1. filtra por escopo, `status = active` e validade;
2. pontua por: sobreposição de termos entre a tarefa (mais o `context_query`) e `title`/`content`/`topic`; tipo
   (`pitfall` e `how_to` antes de `decision`); proveniência (`User` antes de `Agent`); recência de uso; `uses`;
3. mantém as melhores sob um **teto próprio de tokens** (padrão 600, configurável) dentro do `ExecutionBudget`: se o
   orçamento do passo é apertado, o teto encolhe; nunca estoura o orçamento;
4. cada memória escolhida é um `ContextItem` `SectionKind::Memory`, prioridade `Optional` (a primeira a sair), e a
   resposta do Context Engine decide o que fica. Com zero memórias relevantes **nada** é adicionado, nem um título.

O manifest registra cada memória considerada: entrou, foi cortada (e por qual razão) ou não pontuou, de modo que "por que o
agente soube disso" é uma pergunta com resposta. Isso também alimenta a medida de ganho (decisão 8).

### 7. Validade e higiene

- Uma memória guarda o que a torna verdadeira quando é verificável sem modelo: caminhos citados e comandos de `how_to`
  (`package.json` scripts, `Makefile`). Antes de recuperar, o Atlas confere se o caminho ainda existe e se o script ainda
  está declarado; se não, a memória vai para `stale` e não é enviada (a pessoa vê o motivo e pode renovar ou apagar).
- `uses` e `last_used_at` sobem só quando a memória _entrou no prompt de um passo concluído_; é o sinal que decide poda,
  cota e validade.
- Nada é apagado em silêncio: o que expira ou é podado vai para `archived`, recuperável por 30 dias.

### 8. Medir antes de dizer "otimizado"

Seguindo a ADR 0020: as métricas da execução ganham o que a Memory custou (tokens injetados, itens, instrução de
proposta) e o que se observou depois (passos concluídos com e sem memória, tentativas até o sucesso, tokens por passo
concluído). O Atlas **não** afirma ganho sem esses números; a verificação com modelo real é uma fase própria e pede
autorização (custa tokens do usuário).

### 9. O que fica fora

- Vetores, embeddings, RAG e qualquer chamada de modelo só para extrair, resumir ou ranquear memórias.
- Memória compartilhada entre agentes ou usuários; sincronização entre máquinas.
- Memória do workflow entre execuções (SharedExecutionState continua sendo o único dono; reavaliar depois da M4).
- Gravar em `.atlas/memory/` ou em arquivos de memória das CLIs (`CLAUDE.md`, `memory/` do Claude): o Atlas nunca edita
  `~/.claude` nem arquivos de usuário das ferramentas.

### 5b. Um passo que terminou não é um passo que acertou (o portão do validador)

A medição do M4 (`spikes/memory-m4-results.md`) mostrou o defeito do desenho original, em que uma nota que passava nas
checagens da decisão 5 entrava sozinha: dois modelos diferentes caíram na isca da tarefa, **terminaram o passo** e gravaram
a lição errada ("o limite está em `app.toml`", "mude os três arquivos"), e a execução seguinte a recebeu e repetiu o erro.
Nenhuma checagem da decisão 5 mede se a lição é _verdadeira_. Por isso, em modo `auto`:

1. Uma nota que passa em todas as checagens fica **retida**: `Candidate` com `provenance.automatic`. A pessoa a vê (e pode
   aprovar ou descartar), nenhum modelo a lê, e ela não é recuperada.
2. Ela só **entra** (`Active`) quando um **validador independente** passa a execução: um passo de _outro_ agente, cujo
   contrato de resultado é de validação ou de revisão (`ContractKind::Validation`/`Review`), com desfecho `pass`, na mesma
   execução de workflow. O passo que propôs a nota nunca a valida (checa-se o agente, não só a execução). Quem faz a
   ligação é o orquestrador (`MemoryService::confirm_held`).
3. Um desfecho `fail` desse validador **descarta** as notas retidas dos passos que ele julgou (`discard_held`); notas que uma
   pessoa propôs ou aprovou nunca são tocadas.
4. Uma nota que entrou assim continua **não confirmada** (`is_unconfirmed`): ninguém a decidiu. Ela é recuperada e usada, mas
   aparece no prompt como `[pitfall (unconfirmed)]`, na lista como "Não confirmada (automática)" com a marca de que outro
   agente a validou, e expira (60 dias sem uso). Só uma pessoa (aprovar, editar, "tornar minha") a confirma.
5. A cota das automáticas é conferida de novo na entrada, para uma sequência de aprovações não a estourar.
6. **Fora de um workflow** (conversas, passos de spec) não há validador independente: a nota fica retida até a pessoa
   decidir. Nesse caso `auto` se comporta como `propose`.

O `auto` continua sendo escolha explícita da pessoa, e `propose` continua sendo o modo recomendado.

## Fases

| Fase | Entrega                                                                                                                                                                      | Critério de pronto                                                                                                  |
| ---- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| M1   | Domínio `Memory`, porta e armazenamento, recuperação determinística, `SectionKind::Memory` (Untrusted), manifest, guardrails; comandos Tauri de CRUD; modo `off` por padrão  | `npm run check` verde; com modo `off` o prompt é idêntico byte a byte; testes de escopo, cota, validade e orçamento |
| M2   | Protocolo `atlas-memory`, candidatos, aprovação (aprovar/editar/descartar), coleta determinística de falha→sucesso, modo `auto` com todas as condições da decisão 5          | testes provam que cada condição rebaixa ou descarta; `preference`/`User` nunca automáticos; segredo descartado      |
| M3   | Interface: painel de Memória por escopo, candidatos pendentes inline no passo e no chat, selo de automática, promover/editar/apagar, aba no Inspetor, export/import opcional | revisão visual no app; textos nos três idiomas; acessibilidade do painel                                            |
| M4   | Validação: métricas de custo/benefício; execução real autorizada (tokens) comparando passos com e sem memória                                                                | números registrados; decisão sobre padrões e cotas baseada neles                                                    |

## Decisões pendentes (a pessoa decide antes da M2)

- Padrão do modo por escopo quando a pessoa ligar a Memory pela primeira vez (proposta: `propose` em Workspace e Agent,
  `propose` em User, `auto` só por escolha explícita).
- Cotas e validades padrão (valores acima são o ponto de partida, a calibrar na M4).

## Emenda de 2026-10-20 — poda de inundação na recuperação

Medida em 240 notas parecidas (`docs/architecture/spikes/ai-memory-analysis.md`): a recuperação pontuava por palavras
em comum sem peso por raridade, e como "change", "service" e "limit" aparecem em quase tudo, **a partir de ~15 notas
o orçamento inteiro de 600 tokens era preenchido em todo prompt, com ~9 notas de outro assunto**. A nota certa estava lá,
no meio do ruído.

- A comparação de palavras é por **radical** (`domain::memory::stem`), nas notas e na tarefa; só corta quando sobram 4
  letras e guarda o `e` de palavras curtas (`pipe` não é `pip`).
- **Até 5** notas casando, ou uma memória com **menos de 12** notas (`PRUNE_ABOVE`, `PRUNE_NEEDS_NOTES`): nada muda.
  Caso contrário ficam as que são sobre a tarefa: o título traz duas palavras dela (ou a única que ela tem), **ou** a
  nota compartilha uma palavra rara (1 nota em 20, `RARE_ONE_IN`) ou uma palavra incomum do seu título e pontua pelo
  menos 60% do melhor (`KEEP_PERCENT`); ou compartilha duas palavras incomuns (`TWO_WORDS_MIN`, que não estejam em mais
  da metade das notas) e pontua pelo menos 40% do melhor (`TWO_WORDS_PERCENT`).
- A pontuação pesa cada palavra pela raridade (`ln(1 + N/df)` em milésimos, título em dobro) e **ignora as palavras que
  mais de uma nota em quatro tem** (`GENERIC_ONE_IN`, só com 20 notas ou mais). O que fica é ordenado em décimos do
  melhor, depois por `tie_bonus` (tipo e origem, como no caminho sem poda), depois por uso e recência. O resto é
  `NoMatch` no manifesto.
- Sem configuração nova: os valores são constantes em `recall.rs`, em números inteiros. O contrato desta ADR não muda:
  sem nota relevante o prompt é idêntico byte a byte, a nota continua não confiável e sem poder de instrução.
- Efeito medido contra o código original, no mesmo benchmark com ids embaralhados: 586 → 273 tokens por prompt com 240
  notas, de 374 para 32 notas de outro assunto, a nota certa no prompt em 40 de 40 (antes 39). Em 46 notas reais do
  projeto e 18 tarefas reais: 375 → 191 tokens por tarefa e 5,7 → 2,2 notas a mais por tarefa, mas **88% → 81% das notas
  esperadas encontradas** (a poda custa 2 de 26). Com o Claude haiku e a memória cheia, isca tocada 0/3 contra 3/3 sem
  memória e input -50% (numa execução diferente da do ADR 0040, onde o ganho de um servidor com uma só nota foi de -16% a -58% conforme o lote).

Limites: as constantes foram ajustadas num corpus sintético e, a última regra, no conjunto de 46 notas reais do projeto
(18 tarefas), sem um conjunto separado para validar; o radical é inglês e cru; entre notas do mesmo assunto a
ordem só ganha com uso acumulado; uma tarefa dita com outras palavras que a nota recupera pouco (19 de 40) e traz pouco
ruído; numa memória de 12 a 20 notas concentrada num tema, com uma tarefa que só traz o tema, a poda pode descartar
notas úteis.
