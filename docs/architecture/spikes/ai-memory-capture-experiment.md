# Experimento — o ai-memory aprende sozinho uma lição útil?

Data: 2026-10-20. Estágios 0, 1 e 2 executados (resultados abaixo). A análise de custo e benefício está em
[`ai-memory-analysis.md`](ai-memory-analysis.md). Relacionado à [ADR 0040](../adr/0040-memoria-externa-ai-memory.md).

## Pergunta

A ADR 0040 provou o **canal**: uma nota certa, entregue pelo servidor, evita o erro e corta tokens. Ela **não** provou que
o ai-memory, capturando uma sessão por conta própria, produz essa nota. É isso que decide se vale ligar a captura
automática dele ou manter só o que uma pessoa escreve e aprova.

Hipótese a refutar: **a captura do ai-memory transforma uma sessão de agente em uma nota que, entregue a uma sessão
seguinte, tem o mesmo efeito da nota escrita à mão (isca evitada, arquivo certo, menos tokens), sem notas falsas e
sem vazar segredo.**

## Estágio 0 — captura sem LLM (EXECUTADO, sem custo, sem rede)

Eventos de hook de uma sessão do Claude Code (prompt, leitura de `config/app.toml`, busca por `MAXTRY`, edição de
`deploy/limits.cfg`, fim) foram reproduzidos por `ai-memory hook --event … --agent claude-code` num servidor isolado, com
`AI_MEMORY_HOME` numa pasta temporária.

| Medido                                        | Resultado                                                                                                                                                                                                                                                                                        |
| --------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| O que o servidor escreve                      | Uma página `sessions/<id>.md`: o prompt, a contagem de chamadas de ferramenta ("tool file: 2") e a lista de observações. **Nenhuma lição.** "MAXTRY está em deploy/limits.cfg" não aparece em lugar nenhum.                                                                                      |
| O título da página                            | É o próprio prompt. Uma tarefa escrita parecido casaria com ela como se fosse uma nota. Por isso o Atlas passou a **ignorar `sessions/…`** (e páginas `_…`, como o relatório de lint do servidor) na leitura.                                                                                    |
| Escritas fora das pastas isoladas             | Nenhuma (`~/.claude`, `~/.codex`, `~/.config/opencode`, LaunchAgents comparados antes e depois).                                                                                                                                                                                                 |
| Hooks por execução, sem tocar na configuração | Possível: `install-hooks --agent claude-code --hooks-dir <fonte>/hooks --config-file <temp>/settings.json --apply` gera os hooks num arquivo temporário (comandos apontando para a pasta de dados e a porta do servidor de teste) e `claude -p --settings <arquivo>` os usa só naquela execução. |

**Conclusão do estágio 0:** sem um LLM consolidando, a captura do ai-memory é um diário de sessões, não memória de
lições. A pergunta só se responde nos estágios com LLM.

## Estágio 1 — captura e consolidação reais com um LLM (EXECUTADO com o Claude haiku)

1. **Capturar de verdade.** Servidor isolado (porta própria, pasta própria, `AI_MEMORY_BACKFILL_ON_START=false`,
   embeddings desligados). N = 6 execuções reais do braço A da tarefa do M4 (projeto sintético com a isca), cada uma com
   `claude -p --settings <hooks temporários>`. Nada é instalado globalmente.
2. **Consolidar.** `memory_consolidate` / `auto-improve` do ai-memory com um provedor de LLM. Opções, da menos para a mais
   exposta: `opencode` (modelo gratuito, o texto da sessão sintética vai ao provedor dele); `claude` ou `codex` (usam
   a conta logada, custam tokens). O texto enviado é só o do projeto sintético; nenhum dado seu.
3. **Ler.** Uma sessão nova (braço **F**) recupera o que a consolidação escreveu, com as regras do Atlas (título, aviso,
   orçamento), contra A (controle) e D (nota escrita à mão).

### Medidas

| Medida                         | Como                                                                                                                                                                                                   | Aprova se            |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------- |
| Rendimento da lição            | Fração das sessões cujas notas geradas contêm o fato certo (`MAXTRY` e `deploy/limits.cfg`), checado por texto, sem juiz-LLM                                                                           | ≥ 50%                |
| Notas falsas                   | Notas que afirmam o contrário (`retries` em `config/app.toml` como o limite). O M4 já viu um modelo escrever uma assim                                                                                 | ≤ 10% do que é lido  |
| Efeito a jusante               | Braço F contra A e D: arquivo certo, isca tocada, input/output, chamadas de ferramenta                                                                                                                 | F ≈ D e melhor que A |
| Custo líquido                  | Tokens da consolidação por sessão + tokens da nota por prompt, contra a economia medida                                                                                                                | economia > custo     |
| Segredos                       | 10 segredos sintéticos (chaves `sk-…`, JWT, PEM, **e strings aleatórias de alta entropia**, que o sanitizador não pega) nos prompts e nas saídas de ferramenta; busca por eles nas páginas e em `raw/` | 0 vazamentos         |
| Efeitos fora da pasta de dados | Comparar `~/.claude`, `~/.codex`, `~/.config`, LaunchAgents antes e depois                                                                                                                             | 0 arquivos alterados |

### Decisão que o resultado produz

- **Aprova em tudo:** liga-se a leitura de notas geradas, ainda marcadas como "não confirmadas" e **sempre passando pela
  aprovação do Atlas** antes de virarem notas compartilháveis (o gate da ADR 0037 continua valendo).
- **Falha em notas falsas ou segredos:** a captura automática fica desligada e o caminho oficial segue sendo a nota que
  uma pessoa escreve ou aprova (o que já funciona).
- **Falha só em rendimento:** vale testar o provedor de LLM mais forte antes de descartar.

## Estágio 2 — embeddings (EXECUTADO após autorização do download)

Os 40 casos parafraseados do benchmark (`ai-memory-bench/retrieval_bench.py`) acham a nota certa no top 3 em apenas 9
(sem a checagem de título) porque a busca é por palavras. Os embeddings locais do ai-memory (MiniLM) poderiam fechar
essa lacuna.

- **O que seria baixado:** `model.safetensors` e arquivos do `all-MiniLM-L6-v2`, cerca de 87 MB, de `huggingface.co`
  (checksum fixo), para a pasta de dados do servidor de teste. Não é executável; é peso de modelo. Nada é enviado.
- **Medir:** o mesmo benchmark com embeddings ligados: nota certa no top 3, notas de outro assunto por prompt, e se a
  checagem de título (`precise`) ainda é necessária ou passa a atrapalhar.
- **Custo:** zero tokens; o download e um pouco de CPU.

## Segurança do próprio experimento

- Servidor em `127.0.0.1`, porta e pasta próprias, token gerado nela. Sem `setup`, `install-mcp` nem serviço.
- `AI_MEMORY_HOME` numa pasta temporária; o arquivo de hooks é temporário e some com o experimento.
- Projeto, prompts e arquivos sintéticos. Nada do seu trabalho entra.
- Qualquer provedor de LLM do estágio 1 é escolhido por você antes de rodar.

## Estimativa

| Estágio      | Tokens pagos                                   | Rede                                       | Tempo   |
| ------------ | ---------------------------------------------- | ------------------------------------------ | ------- |
| 0 (feito)    | 0                                              | nenhuma                                    | minutos |
| 1 (opencode) | 0 (modelo gratuito)                            | só o provedor do OpenCode, texto sintético | ~1 h    |
| 1 (claude)   | ~6 execuções do haiku + consolidação, centavos | provedor da conta                          | ~1 h    |
| 2            | 0                                              | download de ~87 MB do Hugging Face         | ~30 min |

## Resultados

### Como o Claude foi usado como LLM do ai-memory, sem entregar uma credencial

O provedor `anthropic-oauth` do ai-memory exige um token longo gerado por `claude setup-token`. Em vez disso, o
provedor `openai-compat` aponta para [`ai-memory-bench/claude_llm_shim.py`](ai-memory-bench/claude_llm_shim.py): um
adaptador em `127.0.0.1`, com chave aleatória, que repassa cada pedido ao `claude -p` já logado, com **todas as
ferramentas desligadas** (`--tools ""`, verificado: um pedido para criar um arquivo não cria nada), sem o seu
`CLAUDE.md` nem as suas configurações (`--setting-sources project`, verificado: o modelo deixa de saber o que é RTK),
com teto de 80 chamadas e US$ 1,50. A credencial nunca saiu do CLI. Dois detalhes de configuração: a variável da chave é
`LLM_API_KEY` (sem prefixo) e o agendador de auto-melhoria foi desligado para atribuir cada custo.

### Estágio 1

8 sessões reais do Claude haiku no projeto com a isca (6 sem correção, 2 em que a pessoa corrige: "é MAXTRY em
`deploy/limits.cfg`") e 2 sessões em projetos com segredos-isca. Hooks gerados num arquivo temporário e passados por
`claude --settings`; nenhum arquivo foi alterado em `~/.claude`, `~/.codex`, `~/.config/opencode` nem LaunchAgents.

| Medida                                                                                            | Resultado                                                                                                                                                                                                                                                                           |
| ------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Sessões sem correção                                                                              | 5 de 6 editaram o lugar errado (4 a isca `config/app.toml`, 1 `src/client.py`); só 1 tocou `deploy/limits.cfg`                                                                                                                                                                      |
| Rendimento da lição                                                                               | **2 de 2** sessões com correção viraram nota (`gotchas/…`), corretas, mas **quase duplicadas** entre si. **0 de 6** sessões sem correção geraram lição (esperado: ninguém sabia)                                                                                                    |
| Notas falsas                                                                                      | 0 notas curadas falsas. Mas os **resumos das 6 sessões sem correção descrevem o erro como fato** ("retry limit changed to 5 in config/app.toml"); estão em `sessions/…`, que o Atlas não lê                                                                                         |
| `memory_auto_improve`                                                                             | Com os limites padrão (≥ 8 observações, ≥ 120 s) **pula** as nossas sessões; relaxado, 2 chamadas e "nenhuma proposta válida" com aviso de resumo vazio do LLM. Inconclusivo: pode ser limitação do adaptador com saída estruturada                                                 |
| Custo do LLM do ai-memory                                                                         | **US$ 0,91** em 22 chamadas (média 52 s, 134 mil tokens de entrada, 130 mil de saída), ~US$ 0,10 por sessão consolidada, contra ~US$ 0,012 que a própria sessão custou ao agente (**cerca de 8×**)                                                                                  |
| Segredos no formato `NOME=valor` (5)                                                              | **Protegidos**: o sanitizador trocou o resto do arquivo por `[REDACTED:env_secret]` (e marcou parte de um caminho como `api_key` por engano)                                                                                                                                        |
| Segredos soltos, sem nome de variável (4: senha em prosa, código de uso único, hex, alfanumérico) | **4 de 4 vazaram em texto puro** em `observations` e no índice de busca de observações do banco do servidor. Nenhum chegou às páginas da wiki depois da consolidação (o resumo saiu como "File read request with sensitive data exposure"), mas as observações são o que vai ao LLM |
| Efeito a jusante (Claude haiku, 3 rodadas)                                                        | **F** (notas geradas, com checagem de título): isca 0/3 contra 3/3 do controle, arquivo certo 3/3, input 61 202 → 48 857 (-20%), output 1 531 → 980 (-36%), 8 → 4 chamadas de ferramenta, 172 tokens de notas. **G** (sem a checagem): 1/3 ainda tocou na isca, 206 tokens          |

Defeitos meus que o experimento expôs e foram corrigidos: a checagem de título comparava palavras exatas e
**barraria as duas notas corretas** (a busca do servidor usa radical, `retry` acha "retries"; agora o Atlas também
compara radicais); e os resumos de sessão ocupavam o topo dos resultados e soterravam as notas curadas (agora o Atlas
pede uma janela de 25 a 50 resultados e ignora `sessions/…`).

### Estágio 2 (embeddings, MiniLM de 87 MB baixado de `huggingface.co`)

O servidor carregou o modelo (`embedder enabled provider="local" dim=384`). Mesmo corpus de 240 notas, nota certa no top 3:

| 40 tarefas                              | só palavras (`/admin/search`, o que o Atlas usa) | híbrido com embeddings (`memory_query`) |
| --------------------------------------- | ------------------------------------------------ | --------------------------------------- |
| Diretas                                 | **35**                                           | 26                                      |
| Parafraseadas, frase inteira            | **9**                                            | 5                                       |
| Parafraseadas, só o trecho que descreve | n/d                                              | 15                                      |

Os embeddings **não ajudaram e pioraram o top 3 nas tarefas diretas**. O corpus é de propósito homogêneo (lições
parecidas de configuração), o que prejudica vetores, mas não há aqui argumento para o download. O `/admin/search` que o
Atlas usa nem combina vetores.

### Confronto com a memória nativa do Atlas, no mesmo corpus

| Notas | Nativa: tokens por prompt | Nativa: notas de outro assunto por prompt | Busca do servidor: nota certa no top 3 |
| ----- | ------------------------- | ----------------------------------------- | -------------------------------------- |
| 15    | ~462                      | 8,3                                       | 4/4                                    |
| 46    | ~583                      | 9,9                                       | 8/8                                    |
| 99    | ~589                      | 9,3                                       | 16/16                                  |
| 145   | ~587                      | 9,3                                       | 23/24                                  |
| 240   | ~586                      | 9,2                                       | 35/40                                  |

A nativa **enche o orçamento de 600 tokens em todo prompt já com 15 notas**, porque pontua por palavras em comum sem
peso por raridade ("change", "service" e "limit" empatam tudo). A nota certa está sempre lá, no meio de ruído. Aplicando
à nativa a mesma checagem de título: **0 notas de outro assunto e a nota certa em 40 de 40** (com ~6 notas do mesmo
assunto por prompt). Ou seja, a precisão vem da checagem, e a checagem não depende do servidor.
