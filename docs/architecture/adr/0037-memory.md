# ADR 0037 — Memory: lições duráveis entre sessões, propostas pelo agente e decididas pela pessoa

Status: aceito e implementado, **desligada por padrão**. Medida com modelo real (M4, 2026-10-15, sete rodadas: [spikes/memory-m4-results.md](../spikes/memory-m4-results.md)). Baseia-se na seção 6 de
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
5. cabe no teto de tamanho e na cota do escopo (40 automáticas por workspace e 20 por agente; o escopo User não tem automáticas); passando da
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
- Nada é apagado em silêncio: o que expira ou é podado vai para `archived` e fica guardado (não há expurgo automático).

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

## Medição (M4) e o que ela decidiu

Medido em duas CLIs (`opencode/big-pickle` e `claude` haiku, e uma execução no sonnet), três execuções por braço: A sem
nota, B com a nota certa escrita por uma pessoa, C com o ciclo do agente propondo e a execução seguinte recebendo.

| Achado                                            | Número (medianas)                                                                 |
| ------------------------------------------------- | --------------------------------------------------------------------------------- |
| Nota escrita por uma pessoa, no claude            | entrada 74 mil → 36 mil, saída 1,4 mil → 0,5 mil, 9 → 2 chamadas, isca 3/3 → 0/3  |
| Nota escrita por uma pessoa, no opencode          | entrada dentro do ruído (97 mil → 90 mil), isca 3/3 → 0/3                         |
| Custo de uma nota no prompt                       | ~150–370 tokens estimados                                                         |
| Agente propondo, regra no fim do prompt           | opencode 3 de 3 execuções deixaram nota; claude haiku 0 em 18; sonnet 1 de 1      |
| Nota automática aceita só porque o passo terminou | ensinou o erro duas vezes (opencode e sonnet): origem do portão do validador (5b) |

Decisões tiradas desses números (2026-11-11, item MEM2 do checklist):

- **Modo por escopo:** a pessoa escolhe ao ligar; nenhum escopo liga sozinho. `propose` é o recomendado (é onde a nota
  rendeu), `auto` existe só em Workspace e Agent, por escolha explícita e com o portão do validador; o escopo User não
  tem `auto`. É o que o painel já oferece.
- **Cotas e validades:** mantidas como o código as tem (`memory/service.rs`): até 40 notas automáticas por workspace e
  20 por agente (o escopo User não tem automáticas), até 20 propostas esperando por dono, 60 dias sem uso para uma
  automática ser arquivada, e um teto de 600 tokens por prompt. Uma nota arquivada fica guardada; não há expurgo
  automático. O teto cabe de uma a três notas do
  tamanho medido; nenhuma rodada encheu uma cota, então os números não pedem mudança. Rever quando houver uso real que
  bata nelas.

O que a medição não cobriu: uma tarefa real além da tarefa com isca, Codex e Antigravity, e se a proposta do agente
funciona em modelos pequenos (o haiku não seguiu a regra).

A ADR 0041, se for adotada (está na branch de POC da memória externa), muda o padrão
desta ADR; enquanto ela não estiver nesta branch, vale o que está escrito aqui: desligada por padrão.
