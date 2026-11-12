# ADR 0046 — Servidores MCP remotos (HTTP), com a camada de destino

Status: aceito e implementado no Claude (verificado com testes determinísticos; ver "Não verificado"). Emenda a
[ADR 0028](0028-mcp-foundation.md), que reservava `McpTransport::Http` e o recusava. Segue a estratégia A da
[ADR 0026](0026-context-and-tooling-platform.md): a CLI do runtime conecta ao servidor ela mesma, e o Atlas não fica no
meio (decisão do usuário em 2026-11-11, entre três caminhos: HTTP nativo com checagem no lançamento, proxy local do
Atlas, ou só a ponte stdio).

## Decisões

1. **O transporte** é `Http { url, headers, allow_private }`. Um cabeçalho é `{ name, value }` com o mesmo
   `McpEnvValue` das variáveis de um servidor STDIO: o valor de um segredo fica no cofre do SO sob o nome do cabeçalho e
   nunca na configuração. Configurações antigas (`Http { url }`) são lidas com cabeçalhos vazios e `allow_private`
   desligado.
2. **O formato é validado sem rede** (`validate::check_http`): `https` (ou `http`, ver 3), um host, nada de usuário ou
   senha na URL, nada de fragmento, até 2048 caracteres, sem espaço nem quebra de linha, sem referência de expansão
   (`${…}`, `$VAR`, `{env:…}`, `{file:…}`, que as CLIs expandem também na URL e nos cabeçalhos), e nenhum parâmetro de
   consulta com cara de credencial (`token`, `api_key`, `key`, `secret`, `password`, `sig`…): a URL acaba em logs, na
   trilha de auditoria e nos arquivos da CLI, e um segredo vai num cabeçalho. Cabeçalhos: até 32, nome em
   `[A-Za-z0-9-]`, sem repetição, nunca `Host`, `Content-Length`, `Transfer-Encoding`, `Connection`, `Upgrade`, `TE`,
   `Trailer`, `Keep-Alive` nem `Proxy-*`, e um valor simples sem caractere de controle (um valor não acrescenta outro
   cabeçalho).
3. **A camada de destino (SSRF)** é `mcp::destination::check`, atrás da porta `HostResolver` (o DNS do sistema em
   `infrastructure/host_resolver.rs`). Todo endereço para o qual o nome resolve conta:
   - público: aceito, por `https`;
   - loopback, rede privada, CGNAT, IPv6 de escopo local único (e um IPv4 mapeado em IPv6 que caia aí): só com
     `allow_private`, a permissão explícita da pessoa para um servidor na própria máquina ou rede;
   - link-local (`169.254.0.0/16`, `fe80::/10`), `fd00:ec2::254` e os nomes de metadados conhecidos
     (`metadata.google.internal`…): **nunca**, nem com `allow_private`;
   - não especificado, multicast, broadcast e faixas de documentação: nunca;
   - `http` em claro só para um destino inteiramente privado com `allow_private`; nunca pela internet.
4. **Quando é checado:** ao salvar (um destino sabidamente fechado é recusado com o motivo, `destination_<código>`; um
   nome que não resolve agora, offline, é guardado) e **de novo logo antes de cada passo** que recebe o servidor, ao
   ler os segredos. Um destino fechado ou que não resolve nessa hora deixa o servidor fora do passo
   (`McpProblem::DestinationBlocked`); se ele é obrigatório, o passo para, pelo caminho já existente. Sem um
   `HostResolver`, nenhum servidor remoto é dado.
5. **Por runtime, tradução ou recusa com motivo** (`McpFeatures.http`):
   - **Claude:** `{"type": "http", "url", "headers"}` no `--mcp-config` de cada execução. Um cabeçalho secreto vai como
     `${ATLAS_MCP_<SERVIDOR>_H_<CABEÇALHO>}` e o valor só no ambiente do processo da CLI, como os segredos de um
     servidor STDIO (o nome do cabeçalho é normalizado porque `-` não cabe numa referência). Com um servidor remoto o
     Atlas fixa `MCP_TOOL_TIMEOUT` (120 s por chamada) e `MAX_MCP_OUTPUT_TOKENS` (25 000), que a CLI aplica.
   - **Codex, Gemini, OpenCode, Copilot e Antigravity:** recusado (`McpProblem::HttpUnsupported`, dito ao agente, na
     revisão e na UI). Não por falta de suporte nas CLIs, mas porque nenhum adaptador para HTTP foi escrito e medido;
     cada um vira uma linha `http: true` quando for.
6. **O que não é feito com um servidor remoto:** a sonda só roda num runtime com `http` (o Claude); o cliente
   `tools/list` do Atlas ([ADR 0045](0045-descricoes-das-ferramentas-mcp.md)) não inicia um servidor remoto; a
   importação e a exportação para os arquivos das CLIs (`config_io`) continuam pulando HTTP.
7. **UI:** o manifesto mostra `type`, `url`, os cabeçalhos (um segredo como "guardado no cofre" ou "não guardado",
   nunca o valor) e `allowPrivate`; os cabeçalhos secretos aparecem onde a pessoa guarda segredos. Uma conexão remota
   é escrita no editor JSON do manifesto; não há formulário próprio.

## Evidência (determinística, sem modelo e sem rede)

- Destino: `application::mcp::destination::tests` (público aceito com IPv4, IPv6 e porta; loopback, privado, CGNAT,
  ULA, IPv4 mapeado e um nome com um endereço privado entre públicos só com permissão; metadados nunca, inclusive
  por nome, com ponto final e maiúsculas, e por um nome que resolve para `169.254.169.254`; `http` em claro só na rede
  da pessoa; URL ilegível, esquema errado, nome que não resolve e faixas reservadas recusados).
- Formato: `a_remote_server_is_a_url_and_headers_whose_secrets_are_named_never_written`,
  `a_url_that_carries_a_credential_or_is_not_one_is_refused`,
  `a_header_cannot_add_another_steer_the_request_or_read_another_secret`.
- Serviço: `a_remote_server_on_a_closed_destination_is_refused_when_it_is_saved`,
  `a_remote_server_is_launched_with_its_header_secret_and_its_destination_checked_again`,
  `a_remote_server_that_cannot_be_looked_up_before_the_step_is_left_out`,
  `a_remote_server_is_not_examined_by_a_runtime_without_http_nor_asked_by_atlas`.
- Plano: `a_remote_server_goes_only_to_a_runtime_whose_adapter_takes_one`.
- Claude: `a_remote_server_is_an_http_entry_whose_secret_header_travels_in_the_environment` (entrada `http`, segredo
  só no ambiente, limites só com servidor remoto).
- DNS do sistema: `localhost_is_looked_up_without_the_network`.
- UI: `hub.test.ts` (manifesto e cabeçalhos de um servidor remoto).

## Não verificado

- **A CLI real do Claude com um servidor remoto:** que ela expande `${…}` nos cabeçalhos, conecta, lista as
  ferramentas e respeita `MCP_TOOL_TIMEOUT` e `MAX_MCP_OUTPUT_TOKENS`. A sonda sem modelo fecha as três primeiras
  sem gastar tokens, contra um servidor HTTP local (com `allow_private`) ou um público de teste.
- Os outros cinco runtimes com HTTP: nenhum adaptador.

## Limites (o que esta camada não faz)

- **Não intercepta chamadas.** A CLI fala com o servidor diretamente; o Atlas decide o que entrega no lançamento e não
  vê as requisições. Limite de tamanho e de tempo são os da CLI, configurados pelo Atlas.
- **DNS rebinding:** um nome que resolve para um endereço público na checagem e para um privado quando a CLI conecta
  não é pego. A janela é curta (a checagem é logo antes do passo), mas existe.
- **Redirecionamentos** que a CLI siga não são vistos.
- Interceptar de verdade exigiria um proxy local do Atlas no caminho de cada chamada: um Tool Gateway, que a ADR 0026
  recusou e que o usuário não escolheu aqui.

## Consequências

- O Figma oficial (`https://mcp.figma.com/mcp`) passa a depender só de OAuth (item M2 do checklist): o transporte e a
  camada de destino existem.
- Servidores remotos que já funcionavam pela ponte stdio (`mcp-remote`, como o Jira) continuam como estão.
