# ADR 0040 — Memória externa: ler e compartilhar notas com um servidor `ai-memory` na mesma máquina

Status: POC implementada e medida em 2026-10-20 (desligada por padrão). Estágios de captura (com o Claude) e de
embeddings executados no mesmo dia; **veredito e custos em [`spikes/ai-memory-analysis.md`](../spikes/ai-memory-analysis.md)**. A descoberta mais útil
desta POC foi aplicada à memória nativa na [emenda da ADR 0037](0037-memory.md). Complementa a [ADR 0037](0037-memory.md)
(Memory) e a [ADR 0029](0029-mcp-for-every-runtime.md) (MCP). Medições e auditoria em
[`spikes/ai-memory-poc.md`](../spikes/ai-memory-poc.md).

## Contexto

O [ai-memory](https://github.com/akitaonrails/ai-memory) é um servidor local de memória (wiki em markdown, busca
FTS5, grafo, decaimento por uso) que várias ferramentas já usam. A rota óbvia, o MCP Hub, **não serve**: a ponte expõe
24 ferramentas (~17 mil tokens de schema por prompt; só `memory_query` custa ~2 mil) e exige que o modelo informe
`workspace` e `project` em cada chamada, ou a busca cai em outro projeto e volta vazia.

## Decisão

### Ler

1. **O Atlas faz o recall, não o agente.** Antes de montar o prompt, `application/memory/external.rs` monta a consulta
   com as palavras relevantes da tarefa, na ordem em que a tarefa as diz (mesmas stop words EN/PT/ES da memória nativa,
   unidas por `OR`, no máximo 12), e o escopo (workspace do servidor + nome da pasta do projeto). O agente não recebe
   ferramenta nenhuma.
2. **Escopo que as outras ferramentas enxergam.** O workspace do servidor é uma configuração (`default`, onde as
   ferramentas do próprio ai-memory colocam um projeto), e o projeto é o nome da pasta, como elas o derivam. Assim uma
   nota vista por uma ferramenta é vista pela outra.
3. **Mesma seção, mesma desconfiança.** As notas entram em `NOTES FROM EARLIER WORK`, sob o aviso que agora diz "left
   by the user, by an agent or by another tool on this machine", como `MemoryBlock` com `automatic = true` e id
   `ext:<página>`. Cada nota é limpa (marcação do servidor, caracteres de controle e de direção removidos, uma só linha,
   480 caracteres) e uma que parece segredo é **recusada inteira**. Passam pela revisão de contexto e pelo orçamento
   (3 notas, 400 tokens) como qualquer nota, e aparecem no manifesto da execução.
4. **Sem duplicar e sem vazar entre workspaces.** A página `atlas/<id>.md` de qualquer nota que **esta instalação**
   conhece (de qualquer workspace e em qualquer estado) nunca volta: o Atlas já pesa as suas notas, e o escopo no
   servidor é o nome da pasta do projeto, que dois workspaces podem compartilhar (uma nota de um não pode aparecer no
   prompt do outro). A página de uma nota que esta instalação não conhece (outra máquina, outro Atlas) entra, assim como
   a escrita por uma pessoa ou por outra ferramenta.
5. **Falha aberta.** Servidor desligado, lento, com token recusado ou resposta ilegível = execução sem notas, com o
   prompt **idêntico byte a byte** ao de quem não tem o recurso. Um projeto que o servidor ainda não conhece (`404`)
   é "sem notas", não erro.

6. **Precisão: só nota cujo título combine com a tarefa** (`precise`, ligado por padrão). A busca ordena por todas as
   palavras, então uma tarefa que só divide "change" e "service" com uma nota ainda a encontra. Medido num servidor de 240
   páginas parecidas (`spikes/ai-memory-bench/retrieval_bench.py`, sem modelo): numa tarefa dita com outras palavras que
   a nota, a busca sozinha injetava **87 notas de outro assunto em 40 prompts**; com a checagem (2 termos da tarefa no
   título, ou 1 se a tarefa só tem um), **3 notas, todas certas, 0 erradas**. Numa tarefa dita como a nota nada muda
   (a nota certa no top 3 em 35 de 40, 0 de outro assunto, com e sem a checagem). O custo: uma página com título genérico
   que não cita a tarefa não é trazida; desligue para receber o que a busca achar. A comparação é por radicais
   (`retry` com `retries`, `limit` com `limits`, `change` com `changed`), como a própria busca do servidor: sem isso a
   checagem barrava as duas notas corretas que o estágio 1 gerou. Pede-se ao servidor uma janela de 25 a 50 resultados:
   os resumos de sessão repetem a tarefa e ocupam o topo, e as notas curadas não podem ficar soterradas.
7. **O que não é nota não entra.** Páginas internas do servidor (qualquer segmento de caminho que comece com `_`, como o
   relatório de lint) e capturas brutas de sessão (`sessions/…`) são ignoradas. Reproduzindo uma sessão no servidor
   (`spikes/ai-memory-capture-experiment.md`, estágio 0): sem LLM ele escreve um diário (o prompt e contagens de
   ferramentas), e o título é o próprio prompt, o que passaria por nota em qualquer tarefa parecida.

### Escrever (compartilhar)

8. **Publicar o que uma pessoa aprovou.** Só notas de escopo `workspace`, `Active` e **não automáticas** viram página
   (`atlas/<id>.md`, tipo `gotcha`/`fact`/`rule`/`decision`, tags `atlas` e o tipo da nota) em **cada projeto do
   workspace**. Candidata, automática, de usuário ou de agente nunca saem. Uma nota que parece segredo também não.
9. **Retirar quando sai de uso.** Arquivar, ficar obsoleta ou apagar retira a página. O servidor mantém o histórico
   (git); o Atlas só escreve e retira páginas `atlas/<id>.md` de id curto e simples, verificado no adaptador: nunca uma
   página que uma pessoa escreveu.
10. **Registro do que foi publicado.** `external-memory-published.json` (na pasta de dados do Atlas, fora de `memory/`,
    cujos `*.json` são lidos como notas) guarda cada `(workspace, projeto, página)`. Com ele, a reconciliação (botão
    "Publicar agora" e uma automática ao abrir o app) retira a página de uma nota apagada com o servidor desligado, de um
    projeto que saiu do workspace ou de um workspace do servidor que foi trocado.
11. **Fila, não bloqueio, e sem corrida.** No app, as escritas rodam numa thread própria, em ordem. Salvar uma nota ou
    iniciar uma execução nunca espera a rede. Um cadeado de trabalho é segurado por cada job da fila e por uma
    sincronização inteira, e a sincronização só pede a lista de notas **depois** de pegá-lo (senão uma nota aprovada
    durante a sincronização seria tomada por órfã e retirada). Um job que chega depois de uma versão mais nova da mesma
    nota, ou depois da exclusão dela, é ignorado. Um _panic_ num job é registrado e a fila continua. Em teste, tudo
    roda na hora.
12. **Desligado por padrão e em dois degraus**: `enabled` (ler) e `share` (escrever). Comandos `set_external_memory`,
    `check_external_memory` (testar conexão) e `sync_external_memory` (publicar agora).

### Segurança

13. **Só loopback.** `server_url` só aceita `http://127.0.0.1|localhost|[::1]:porta`, validado ao salvar e de novo no
    cliente, que só usa endereços de loopback e tenta todos os que o nome resolve. O token é lido de
    `<data_dir>/auth-token` a cada chamada (nunca é configuração, nunca é registrado) e só vai no cabeçalho
    `Authorization`.
14. **Sem dependência nova.** O cliente (`infrastructure/ai_memory.rs`) é HTTP/1.0 sobre `std::net::TcpStream`, com
    **prazo total de 3 s** por requisição (um servidor que "pinga" um byte por vez não segura o chamador), resposta
    limitada a 256 KiB e ids de página recusados se não forem simples e curtos.
15. **Sem hooks do `ai-memory`.** Eles capturam prompts e entradas de ferramentas sem criptografia em repouso, e o
    `backfill_on_start` (ligado por padrão) importa transcrições de `~/.claude/projects` e de outros agentes. O Atlas não
    instala hooks nem executa `setup`/`install-*`.

## O que foi medido (CLI e modelo reais, tarefa do M4 com isca, formulação realista)

Controle = sem memória. Com nota = a mesma nota do braço B, entregue pelo `ai-memory` real (braço D) ou, no braço E,
aprovada num Atlas, publicada e lida por **um segundo Atlas com memória vazia**, e depois retirada.

| Runtime / modelo      | Amostra (controle / com nota) | Arquivo certo (controle → com nota) | Tocou a isca (controle → com nota) | Input (mediana)                                                    | Output                                                    | Tempo                                            |
| --------------------- | ----------------------------- | ----------------------------------- | ---------------------------------- | ------------------------------------------------------------------ | --------------------------------------------------------- | ------------------------------------------------ |
| Claude · haiku        | 12 / 3 (D) + 9 (E)            | 9/12 → **12/12**                    | 12/12 → **0/12**                   | D: 87 924 → 48 171 (-45%) · E: -16%, -21% e 86 454 → 35 981 (-58%) | D: 1 966 → 754 · E: 1 499 → 789, 1 560 → 773, 1 852 → 651 | 20 s → 12 s (D) · E: 18 → 14, 16 → 11, 21 → 10 s |
| Codex · gpt-5.6-luna  | 3 / 3                         | 2/3 → **3/3**                       | 3/3 → **0/3**                      | 91 384 → 74 180 (-19%)                                             | 1 817 → 1 007 (-45%)                                      | 37 s → 30 s                                      |
| OpenCode · big-pickle | 8 / 12                        | 8/8 → 10/12                         | 7/8 → **0/12**                     | sem ganho consistente                                              | 2 129 → 1 011 no lote de 5                                | sem ganho consistente                            |
| **Total**             | **23 / 27**                   | **19/23 → 25/27**                   | **22/23 → 0/27**                   |                                                                    |                                                           |                                                  |

- A nota custou **138 a 147 tokens estimados** por prompt (a regra de propostas da memória nativa custa ~200).
- **O ganho firme é de correção:** com a nota, nenhum run (0/27) alterou o arquivo-isca, contra 22 de 23 sem ela. O
  corte de input é grande no Claude e no Codex e varia entre os lotes do mesmo modelo (-16% a -58% no Claude, em quatro lotes), então a
  faixa é a medida honesta, não um número só. No OpenCode não há ganho de tokens, como já era no M4 nativo.
- **O ciclo de escrita está provado com modelo real (braço E, Claude):** a cada rodada o teste afirma que a nota foi
  publicada (1 página no servidor), que o segundo Atlas, sem notas próprias, a leu (`prompt_has_notes`, manifesto
  `notes_sent: 1`) e que apagar a nota no primeiro retirou a página (0 páginas).
- Dois runs do OpenCode com a nota não alteraram nenhum arquivo (10/12); ficaram só no primeiro lote e não se repetiram
  em 9 rodadas seguintes. Causa não investigada; tratados como variação do modelo gratuito, sem prova.
- Ida e volta contra o servidor real, sem modelo: publicar → achar → retirar → sumir → retirar de novo sem erro
  (`real_server_publish_find_and_retire`).

## Revisão independente e o que ela mudou

Um agente revisou o código só para defeitos reais. Não achou caminho para o token ou o conteúdo chegar a um host que
não seja loopback. Achou, e foi corrigido: rede síncrona no caminho do usuário (agora fila + prazo total); páginas
órfãs (registro + reconciliação); ordem das chamadas (fila em ordem); `localhost` resolvendo `::1` primeiro (tenta todos
os endereços); consulta com as 12 primeiras palavras em ordem **alfabética** (agora na ordem da tarefa); supressão que
escondia notas que o Atlas não tinha recuperado (agora só as já no prompt ou fora de uso); `publish` sem guarda de
caminho; ids e texto do servidor sem limite; aviso do prompt que não citava "outra ferramenta"; limites de `maxHits` e
`maxTokens` no comando.

## Limites e o que não foi feito

**Corrigidos depois da primeira versão desta ADR:**

- _"Não mede ruído"_: agora mede (decisão 6). O ruído existia só quando a tarefa não usa as palavras da nota, e a
  checagem por título o elimina nesse benchmark. Ela foi desenhada **contra** esse benchmark, que é sintético (títulos
  que ecoam o assunto); em notas reais com títulos fracos o ganho de precisão vem com perda de cobertura, por isso a opção.
- _Motivos de falha em inglês_: a tela agora recebe um código estável (`not_running`, `refused`, `timeout`…) e o traduz;
  o inglês fica só no log. Um endereço fora do loopback é recusado com mensagem própria (`error.external_address_invalid`).
- _O Atlas não sobe o servidor_: `scripts/ai-memory-local.sh` sobe o ai-memory com o ambiente que o Atlas espera (só
  `127.0.0.1`, sem embeddings, sem importar histórico, sem instalar nada), recusa porta ocupada e pasta relativa, e
  imprime o que colocar na tela. O Atlas continua **não** supervisionando o processo.
- _Páginas internas e capturas brutas do servidor podiam virar nota_: ignoradas (decisão 7).

**Que continuam:**

- **A nota medida é escrita à mão**, a mesma do braço B. Mede o _canal_ e o ciclo de compartilhamento, não a qualidade
  do que o `ai-memory` captura sozinho. O estágio 0 (sem LLM) mostrou que ele escreve só um diário; saber se a
  consolidação com LLM produz uma lição útil, correta e sem vazar segredo é o experimento proposto em
  [`spikes/ai-memory-capture-experiment.md`](../spikes/ai-memory-capture-experiment.md), que precisa da sua escolha de
  provedor de LLM (estágio 1) e do seu OK para baixar o modelo de embeddings (estágio 2).
- **Paráfrase.** A busca é por palavras: uma tarefa dita com outras palavras que a nota (40 casos sintéticos) acha a nota
  certa no top 3 em 9 de 40 sem a checagem e em 3 de 40 com ela (a checagem tira o ruído, não recupera o que a
  busca não vê). Só embeddings fecham isso, e dependem do estágio 2.
- Só notas de **workspace** são compartilhadas. As de usuário e de agente (privadas por decisão da ADR 0037) não saem do
  Atlas.
- Nomes de pasta que diferem só por maiúsculas ou pontuação ("My App" e "my-app") viram o mesmo projeto no servidor.
  Não há conserto sem quebrar o nome que as outras ferramentas dão ao projeto. A fila é por processo; entre dois
  processos do app a reconciliação ao abrir restabelece o estado, mas não a ordem.
- **Quem ocupa a porta configurada antes do servidor subir recebe o token.** Isto não é uma fronteira de segurança contra
  código do mesmo usuário (que já pode ler o arquivo do token); protege de um servidor no lugar errado, e o script
  recusa subir sobre uma porta ocupada.
- Só macOS foi exercitado. O cliente é `std::net` puro e deve funcionar em Windows e Linux, mas o script é `bash`.
- Com o sandbox ligado nada precisa ser liberado: quem fala com o servidor é o Atlas, fora do confinamento.
- Não foi criado um preset no MCP Hub (ver Contexto). Quem quiser a ponte pode registrá-la à mão, liberando só
  `memory_query`, ciente do custo de ~2 mil tokens e de que o modelo precisa informar o escopo.

## Como reproduzir

```bash
# servidor isolado (sem rede externa, sem importar histórico, só 127.0.0.1)
scripts/ai-memory-local.sh --data-dir <pasta> --port 49391 --bin <ai-memory>

# qualidade da busca contra um servidor cheio de páginas parecidas (sem modelo, sem custo)
python3 -I docs/architecture/spikes/ai-memory-bench/retrieval_bench.py http://127.0.0.1:49391 <pasta> src-tauri/src/domain/memory.rs

# ida e volta contra o servidor real (sem modelo)
ATLAS_AIMEM_DATA=<pasta> cargo test infrastructure::ai_memory::tests::real_server_publish_find_and_retire \
  -- --ignored --exact

# experimento com modelo real (custa tokens; nome exato). Braços: A controle, D servidor, E ciclo compartilhado
ATLAS_M4_ARMS=AE ATLAS_M4_TASK=realistic ATLAS_M4_AIMEM_BIN=<ai-memory> ATLAS_M4_AIMEM_DATA=<pasta> \
  cargo test application::executions::memory_experiment_tests::memory_experiment_claude -- --ignored --exact --nocapture
```
