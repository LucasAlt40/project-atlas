# ADR 0029 — MCP para todo runtime que consegue suportá-lo

Status: aceito. Baseia-se na [ADR 0028](0028-mcp-foundation.md); não muda nenhuma das suas invariantes.

## Contexto

A Fase D deu servidores MCP somente ao Claude, porque só o Claude havia sido medido. Uma pessoa que concedia uma
integração a um agente em outro runtime não via nada acontecer: o manifest dizia `runtime_unsupported`. O pedido: todo
runtime que o Atlas suporta deve poder usar MCP.

## O que foi medido (CLIs reais, sem chamada de modelo)

| Runtime     | Configuração por execução (nada do usuário é escrito)                      | Segredo                                        | Filtro de ferramentas                                           | Olhada sem modelo                                        | Só os servidores do Atlas? |
| ----------- | -------------------------------------------------------------------------- | ---------------------------------------------- | --------------------------------------------------------------- | -------------------------------------------------------- | -------------------------- |
| Claude      | `--mcp-config <arquivo>` + `--strict-mcp-config`                           | `${VAR}` do ambiente da CLI                    | lista de negação (`--disallowedTools`), precisa de descoberta   | relatório `init`: status **e nomes das ferramentas**     | sim                        |
| Gemini      | `GEMINI_CLI_SYSTEM_SETTINGS_PATH=<arquivo>` com `mcpServers`               | `$VAR` nos valores de `env`                    | lista de permissão (`includeTools`), sem descoberta             | `gemini mcp list`: só status (stderr, sem terminal)      | não                        |
| OpenCode    | `OPENCODE_CONFIG_CONTENT=<json>` com `mcp`                                 | `{env:VAR}` nos valores de `environment`       | nenhum verificado: concessões de ferramentas nomeadas recusadas | `opencode mcp list`: só status                           | não                        |
| Codex       | `-c mcp_servers.<nome>.<chave>=<toml>`                                     | `env_vars=["NOME"]` repassa `NOME` do ambiente | lista de permissão (`enabled_tools`), sem descoberta            | nenhuma (`codex exec` não informa nada sobre servidores) | não                        |
| Antigravity | nenhuma encontrada: `agy mcp add` escreve a configuração global do usuário | —                                              | —                                                               | —                                                        | —                          |

Cada linha de "segredo" foi demonstrada por um servidor que só inicia se o segredo chegou até ele (spike
`MODE=requireenv`): `connected` com o valor certo, `failed`/`Disconnected` com um errado. Os argumentos `-c` do Codex
foram conferidos com `codex mcp get --json`.

## Decisões

1. **Capacidades, não um booleano.** `RuntimeCapabilities.mcp_features` diz como um runtime filtra ferramentas
   (`unsupported`, `deny_list`, `allow_list`), o que uma sonda mostra (`none`, `status_only`, `tools`) e se ele carrega
   apenas os servidores do Atlas (`strict`). O plano as usa: uma concessão que nomeia ferramentas precisa de descoberta
   em um runtime de lista de negação, nenhuma em um de lista de permissão, e **não é honrada** (o servidor não é dado,
   `tool_filter_unsupported`) onde nada retém o resto (nenhum runtime está nesse estado hoje).
2. **O Antigravity é `CliConfigured`** (emendado). Dar a ele um servidor por execução significaria alterar a configuração
   global do usuário, o que o Atlas não faz, então não é possível _entregar-lhe_ um. Mas ele carrega os servidores da sua
   própria configuração e, sem ser avisado, não os usa (um agente de QA com `chrome-devtools` concedido nunca abriu o
   navegador: as duas execuções registraram `runtime_unsupported` e o prompt não dizia nada). Então o plano pergunta à
   CLI o que ela tem (`agy mcp list`, entradas habilitadas), casa uma conexão com uma delas pelo nome (`chrome-devtools`
   é `chrome-devtools-mcp`) ou pelo pacote que ela inicia e, quando há uma, a trata como exposta _pela CLI_: o prompt a
   nomeia pelo nome da própria CLI, o manifest registra `cliName`, nenhum segredo sai do armazenamento de credenciais e
   nada é iniciado. Quando a CLI não a tem, o problema é `not_in_cli_config` (o hub oferece o comando `agy mcp add` para
   copiar). Uma concessão que nomeia ferramentas é recusada (`tool_filter_unsupported`): nada consegue reter as outras
   ferramentas da CLI, e ela carrega também os outros servidores (`strict: false`).
3. **Não estrito onde não foi medido.** Gemini, OpenCode e Codex mesclam os servidores do Atlas com os do próprio
   usuário. O Atlas diz isso (entrada de superfície "controlado pelo usuário, não observado"; uma nota onde uma
   concessão é feita) em vez de afirmar tê-los excluído. O `--allowed-mcp-server-names` do Gemini não foi passado: seu
   efeito não foi verificado.
4. **As chamadas precisam ser permitidas.** Uma execução não interativa não pode perguntar a uma pessoa, então o Claude
   recebe `--allowedTools` para o que foi concedido e o Gemini `trust` nos servidores que dá: a concessão da pessoa (e a
   política e os guardrails antes do passo) é a decisão. Lido nas referências das CLIs, **não executado** (uma execução
   custa tokens).
5. **Nomes que não se distinguem não podem coexistir.** `web-2` e `web_2` mapeiam para uma só variável de ambiente; são
   recusados juntos, um nome não pode conter `__`, um segundo servidor com uma chave já usada fica de fora de um
   lançamento. As configurações não podem conter nenhuma das sintaxes de expansão das CLIs (`${…}`, `$VAR`, `{env:…}`,
   `{file:…}`).
6. **Os segredos do Codex viajam sob o nome de variável do próprio servidor** (o que `env_vars` repassa); dois
   servidores que usam o mesmo nome não podem ser lançados juntos.

## Estrutura: Strategy e Factory

`runtimes/mcp_adapter.rs` define `McpAdapter` (programa, recursos, `prepare`, `probe_plan`, `classify`; `probe` é
fornecido a partir do plano) com uma implementação por CLI, e `McpAdapterFactory::create(McpDialect)`. Um runtime guarda
um `Arc<dyn McpAdapter>`: seu `execute` chama `prepare_for(adapter, request)` e acrescenta os argumentos e o ambiente que
recebe; `probe_mcp` e `mcp_tools_in` delegam; suas capacidades declaradas vêm de `McpAdapterFactory::declared`. O mesmo
formato monta o catálogo (`McpPreset`, `McpPresetFactory`). O que isso removeu: quatro cópias da sequência "escrever um
arquivo, montar o ambiente, rodar a sonda, ler a resposta", e os literais de capacidade que diziam a mesma coisa em cada
runtime. O comportamento medido não mudou (os testes com modelo real passam antes e depois).

## Verificado com modelos reais (o usuário autorizou o gasto; poucos tokens cada)

Por todo o Atlas (guard real, CLI real, serviço MCP real, um passo pelo `ExecutionService`), contra um servidor
inofensivo que devolve um nonce que nenhum modelo adivinha (`executions/real_runtime_tests.rs`, executado por nome
exato):

| Runtime  | Ferramentas nomeadas (só `echo_static`, `env_zzz` de fora)           | Servidor inteiro | Entrada Chrome DevTools do catálogo (fixada, headless) |
| -------- | -------------------------------------------------------------------- | ---------------- | ------------------------------------------------------ |
| Claude   | chamada; a outra ferramenta: "NOT AVAILABLE"; registrada como `used` | as duas chamadas | abriu `https://example.com`, listou as páginas         |
| Codex    | chamada; a outra ferramenta: "NOT AVAILABLE"; registrada como `used` | as duas chamadas | abriu `https://example.com`, listou as páginas         |
| OpenCode | chamada; a outra ferramenta: "NOT AVAILABLE"; registrada como `used` | as duas chamadas | abriu `https://example.com`, listou as páginas         |

O que as execuções reais mostraram, e foi corrigido:

- **O Claude recusava toda chamada sem `--allowedTools`** (uma negação de permissão em uma execução não interativa):
  demonstrado diretamente, depois corrigido.
- **O Codex recusava com "MCP tool call requires approval, but approval policy is never"** até que
  `default_tools_approval_mode="approve"` fosse definido.
- **O OpenCode** não precisou de nada para aprovação; sua lista de permissão é o mapa `tools` (`<servidor>_*: false`,
  `<servidor>_<ferramenta>: true`). Uma regra `permission` não deu um resultado claro e não é usada.
- **Uma chamada que o OpenCode só informa quando termina** não era contada: a sonda agora conta também as chamadas
  concluídas (por ferramenta, o maior entre iniciadas e concluídas), de modo que `tool_calls` e `used` fiquem certos
  para ele.
- **O Antigravity** roda headless com os servidores MCP da configuração do próprio usuário (`call_mcp_tool`, com o
  servidor e a ferramenta como parâmetros) e alcançou o `chrome-devtools-mcp` do usuário quando a tarefa o nomeou; o
  Atlas mostra a chamada como `mcp__<servidor>__<ferramenta>`. Ele não usa um servidor a menos que a tarefa peça: **a
  instrução de um agente no Antigravity precisa dizer isso**.

## O agente é informado de quais integrações recebeu (ligado por padrão)

Um modelo que não é informado tende a não recorrer a uma ferramenta que ninguém mencionou. Medido com modelos reais em
uma tarefa que nunca nomeia uma integração ("veja o que há em https://example.com carregando-o de fato em um
navegador", com a entrada do DevTools concedida): **com** a nota, o OpenCode e o Codex carregaram a página; **sem** ela,
o Codex respondeu de memória e nunca tocou no navegador (o OpenCode o usou nos dois casos).

- A parte de contexto do projeto do prompt ganha duas linhas: `Integrations (MCP servers) given to you for this task:
<nomes>.` e um pedido para usar as ferramentas delas quando a tarefa precisar do que oferecem, e não de outro modo. Ela
  nomeia apenas os servidores que o plano dá ao agente, e nada é dito quando nenhum é dado (então um passo que não recebe
  nada tem o prompt que sempre teve).
- É uma configuração do **passo do workflow** (`executionPolicy.announceIntegrations`, ligada por padrão, mostrada no
  inspetor do nó); uma conversa com um agente sempre a tem ligada. Desligá-la não remove a integração. Workflows antigos
  guardados são lidos como ligados.
- Ela muda o prompt, então o hash do manifest difere de uma execução sem ela; esse é o objetivo do hash.

## Catálogo: Jira e Confluence

`jira` é uma entrada do catálogo: `npx -y mcp-remote@0.1.37 https://mcp.atlassian.com/v1/sse` (o servidor remoto da
Atlassian pelo proxy `mcp-remote`, fixado na versão sob a qual o cache de login existente de quem já o usa é mantido,
para que não precisem entrar de novo; o repositório mudou de `geelen` para `punkpeye`, mesmo projeto). O login é OAuth
em um navegador, feito uma vez pela pessoa; o Atlas mostra se existe um arquivo de tokens em
`~/.mcp-auth/mcp-remote-0.1.37` (apenas o nome do arquivo, nunca o seu conteúdo) e diz como entrar quando não existe.
Verificado com um modelo real: o Claude, com o login salvo, chamou `atlassianUserInfo` pelo Atlas e respondeu `OK`
(nenhum dado da conta foi impresso). Como toda entrada, é adicionada desligada e concedida a ninguém.

## Não verificado

- **Gemini CLI**: a conta é recusada pela CLI ("no longer supported for Gemini Code Assist for individuals"), então
  nenhuma chamada de modelo foi possível. Seu arquivo de configuração, o segredo e a sonda foram verificados sem modelo;
  o `trust` deixando uma chamada passar não foi.
- Nomes de ferramentas que uma execução do Gemini informa (então seu `used` continua desconhecido); o Codex informa
  itens `mcp_tool_call`, mostrados como `mcp__servidor__ferramenta`, e o OpenCode `<servidor>_<ferramenta>`, ambos
  registrados como `used`.
- Um arquivo gerenciado de configurações de sistema do Gemini seria substituído, durante a execução, pelo do Atlas.
