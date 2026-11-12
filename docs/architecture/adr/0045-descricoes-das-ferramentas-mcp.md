# ADR 0045 — Descrições das ferramentas MCP: o Atlas pergunta ao servidor

Status: aceito e implementado (verificado com testes determinísticos; ver "Não verificado"). Emenda a decisão 8 da
[ADR 0028](0028-mcp-foundation.md) (a descoberta é o relatório do runtime, só nomes) e o limite da
[ADR 0030](0030-workflow-templates-and-tools.md) (`read_only` não governava ferramentas MCP). Não muda nenhuma invariante
da [ADR 0027](0027-rules-context-authority-and-system-prompt-channel.md): o que o servidor diz é texto `Untrusted`.

## Contexto

As CLIs dos runtimes só informam o nome de cada ferramenta MCP quando iniciam um servidor. Sem descrições o Atlas não
conseguia mostrar o que uma ferramenta faz, não conseguia orçar o peso das definições na janela do modelo
(`ExecutionBudget.tool_definitions` ficava sempre `Unknown`) e decidia o que é "de leitura" por um palpite a partir do
nome (`get_…`, `list_…`). Um passo `read_only` recebia todas as ferramentas MCP concedidas, inclusive as que escrevem.

O protocolo MCP tem a resposta: `tools/list` devolve nome, descrição, esquema de entrada e anotações
(`readOnlyHint`, `destructiveHint`). Para lê-la, o Atlas precisa falar com o servidor ele mesmo.

## Decisões

1. **Um cliente MCP mínimo, só para `tools/list`** (`application/mcp/describe.rs`): `initialize`,
   `notifications/initialized`, `tools/list` com paginação, e fim. Nunca chama uma ferramenta (`tools/call` não existe no
   código). Responde `ping` do servidor e recusa qualquer outro pedido dele com "não suportado". Não é um Tool Gateway nem
   um proxy: na execução o servidor continua sendo iniciado pela CLI do runtime (estratégia A da ADR 0026).
2. **Só quando a pessoa pede**, num botão do inspetor do hub MCP ("Ler descrições"), numa conexão que ela ligou e cujos
   segredos estão guardados (as mesmas condições da sonda). Nada pergunta sozinho, antes de um passo ou em segundo plano.
   Sem o catálogo, nada num passo muda: o plano, o prompt e o orçamento ficam idênticos aos de antes.
3. **O processo passa pelo guard** como um tipo novo, `ProcessContext::McpServer`, e só como sessão
   (`ProcessRunner::open_session`, linha a linha). A regra (`evaluate_mcp_server`): nada de shell, nenhuma variável que
   mude o que o programa carrega (`PATH`, `NODE_OPTIONS`, `LD_*`…) nem `ATLAS_*`, sem terminal, sem stdin dada de início,
   sem pasta de trabalho. Um launcher (`npx`, `uvx`) é aceito porque é como a maioria dos servidores começa e foi a pessoa
   que o escolheu. Qualquer outro contexto não abre sessão, e um `McpServer` nunca roda até o fim por `run`.
4. **Segredos no ambiente do servidor**, como a pessoa configurou (o nome dela, o valor do cofre), nunca em argumento,
   arquivo, log ou no catálogo.
5. **Tudo limitado:** 60 s para a conversa inteira (um launcher pode baixar o servidor), 20 páginas, 500 ferramentas,
   1000 linhas que não são resposta, nome até 128 caracteres, descrição guardada até 1000 caracteres sem caracteres de
   controle. O tamanho contado para o orçamento é o da definição inteira. Uma lista cortada diz `truncated`. Ao fim, a
   sessão é descartada e o processo termina com o que ele iniciou (grupo no Unix, árvore com `taskkill /T` no Windows).
6. **O catálogo fica na conexão** (`McpConnection.catalog`, separado de `discovery`): `discovery` continua sendo o que o
   runtime vê e governa o que é retido; o catálogo é o que o servidor disse. Mudar o transporte apaga os dois.
7. **O texto do servidor é `Untrusted` e não vai para prompt nenhum.** Aparece só no inspetor, com o aviso de que é do
   servidor e não concede nada. As anotações só servem para **dar menos**:
   - **`read_only` governa ferramentas MCP de um servidor descrito.** Num passo somente leitura, a seleção da concessão
     vira só as ferramentas que leem (`plan.rs::reading_only`), e o resto é retido pelo caminho já existente
     (`held_back`, `--disallowedTools`, lista de permissão). Se nenhuma lê, o servidor não é dado
     (`McpProblem::NoReadingTools`, dito ao agente, na revisão e na UI). Um servidor nunca descrito é dado como antes.
   - **A regra de "leitura" é uma só** (`domain::mcp::reads_tool`), usada pelo plano e pelos templates: `destructiveHint`
     verdadeiro nunca lê; `readOnlyHint` decide quando dado; sem nenhum dos dois, vale o palpite pelo nome.
8. **O orçamento conta as definições** quando sabe: `McpPlan::tool_definitions()` soma `chars / 4` das definições das
   ferramentas dadas, marcado `Estimated`, e `ExecutionBudget::with_tool_definitions` desconta da entrada que o Context
   Engine usa. Se algum servidor dado não foi descrito, se alguma ferramenta dada não está no catálogo, se o catálogo foi
   cortado ou se é a cópia da própria CLI, fica `Unknown`. Sem MCP também fica `Unknown`, como antes.

## Evidência (determinística, sem modelo)

- Cliente: `application::mcp::describe::tests` (handshake na ordem, `ping` respondido, ruído e notificações pulados,
  duas páginas, duplicata descartada, descrição limpa e cortada, erro do servidor, silêncio, ruído demais, páginas sem
  fim).
- Guard: `only_an_mcp_server_is_kept_open_and_only_as_the_person_configured_it` (launcher aceito; shells, variáveis
  perigosas, `ATLAS_*`, stdin e terminal recusados; outros contextos não abrem sessão; `run` recusado; nada recusado
  chega ao executor).
- Sessão real: `a_session_answers_line_by_line_and_ends_the_process_when_dropped` (Unix: um processo de verdade responde
  linha a linha, um silêncio é timeout e não trava, descartar a sessão mata o processo).
- Plano: `a_read_only_step_is_given_only_the_tools_a_described_server_says_only_read`,
  `a_server_atlas_never_asked_is_given_to_a_read_only_step_as_before`,
  `a_read_only_step_narrows_a_named_grant_and_never_widens_it`,
  `a_described_server_with_nothing_that_only_reads_is_not_given_to_a_read_only_step`,
  `the_tool_definitions_a_step_is_given_are_estimated_only_when_all_are_described`.
- Templates: `what_the_server_said_about_a_tool_replaces_the_guess_from_its_name`.
- Orçamento: `known_tool_definitions_come_off_the_input_and_unknown_ones_change_nothing`.
- Serviço: `describing_a_server_keeps_what_it_said_about_its_tools_and_needs_it_ready`,
  `a_service_without_a_runner_describes_nothing`.
- Pela execução real: `a_read_only_step_is_given_only_what_a_described_server_says_reads_and_its_weight_is_budgeted`
  (antes de descrever, nada muda; depois, o passo somente leitura retém `write`, o agente é informado só de `read` e o
  orçamento tem a estimativa; um passo que escreve recebe o servidor inteiro).
- UI: `McpHubPage.test.tsx` (o botão pergunta uma vez e mostra descrição, dica e peso; desligado, o botão fica
  desabilitado).

## Não verificado

- **Um servidor MCP real** (Chrome DevTools pelo `npx`, por exemplo): a conversa só foi exercitada contra um servidor
  roteirizado e contra um processo `sh` de verdade. Fechar: rodar "Ler descrições" no DevTools fixado do catálogo e
  conferir nomes, anotações e o fim do processo. Não gasta tokens.
- **Windows:** o fim da árvore por `taskkill /T` e o `npx.cmd` iniciado direto não rodaram; o teste da sessão real é só
  Unix.
- **Sandbox (ADR 0038):** o servidor que o Atlas inicia para perguntar não passa pelo perfil do sandbox, que hoje se
  aplica à CLI do runtime. É o mesmo programa que a pessoa ligou, por no máximo 60 s, sem modelo; registrar aqui até o
  sandbox decidir se cobre esse caso.

## Consequências

- O Atlas agora inicia por conta própria um programa que a pessoa configurou. É a terceira coisa que ele inicia sem uma
  execução (depois das sondas das CLIs e do OpenSpec) e tem a sua própria linha no guard.
- Um servidor que mente nas anotações pode, no máximo, fazer um passo somente leitura receber uma ferramenta que
  escreve. Isso é o comportamento de antes desta ADR para todo servidor; nunca pode fazer um passo receber mais do que
  a concessão dá.
- Um passo somente leitura de um servidor descrito, num runtime que não consegue reter ferramentas, deixa de receber o
  servidor (`tool_filter_unsupported`): o mesmo que já acontecia com concessões por ferramentas nomeadas.
