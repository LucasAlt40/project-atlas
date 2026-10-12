# 0028 — Fundação MCP (Context & Tooling Platform, Fase D)

## Contexto

A ADR 0026 escolheu o formato (um domínio agnóstico de runtime, um adaptador do Claude, estratégia A: o suporte MCP do
próprio runtime, configurado pelo Atlas por execução) e mediu a CLI do Claude sem chamada de modelo. A ADR 0027 fixou a
invariante para resultados de MCP: `ContextAuthority::Untrusted`, nunca capaz de mudar uma rule, permissão, aprovação,
política ou decisão de guardrail. A Fase D constrói a fundação, só STDIO, com o Claude como o único runtime que tem um
adaptador. O desenho está em [context-and-tooling-platform.md](../context-and-tooling-platform.md), seção 17.

## Decisões

1. **O domínio não nomeia nenhum runtime** (`domain/mcp.rs`): `McpConnection`, `McpTransport`, `McpGrant`,
   `ToolSelection`, `McpSupport`, `McpProblem`, `McpToolState`, `McpRecord`, `Secret`. O adaptador do Claude
   (`runtimes/claude_mcp.rs`) traduz; nada no domínio é uma flag de CLI.
2. **Uma conexão não é uma autorização.** Uma conexão diz "este workspace tem esta integração" e é guardada na
   configuração (`UserConfig.mcp_connections`) sem nenhum segredo, desligada e concedida a ninguém. Uma concessão
   (`mcp_grants`) diz qual agente, opcionalmente qual workflow e qual passo, pode usá-la e quais ferramentas. O conjunto
   efetivo de ferramentas de um passo é uma função pura (`application/mcp/plan.rs`): habilitada → concedida → a política
   permite → o runtime tem um adaptador → configuração válida → as ferramentas podem ser retidas → segredos presentes. A
   primeira falha é o motivo de não ser exposta.
3. **Seis fatos por ferramenta, nunca uma flag só:** descoberta, habilitada, autorizada, exposta (o que o Atlas
   arranjou), informada (o que o runtime listou: `None` é "não disse"), usada. Estão no manifest (`McpRecord`) e no
   Inspetor.
4. **O eixo de política está no `security/`**, não ao lado dele: `SecurityPolicy.mcp` (`Allowed` / `ApprovalRequired` /
   `Denied`), estreitado como toda outra configuração; o perfil somente leitura o nega, o perfil developer o permite, e
   uma política antiga guardada não o restringe. Nada é exposto sem uma concessão de qualquer jeito, então o padrão é
   negar. `ApprovalRequired` é um achado (`mcp_approval_required`, Erro): um ASK em um passo de workflow, um DENY sem
   um.
5. **Os guardrails enxergam o MCP pelo caminho existente:** `McpFacts` na Revisão de Contexto (`mcp_unavailable`:
   Bloqueante para uma conexão obrigatória que não pode ser dada, Aviso para uma opcional), e `McpPlan::canonical()` na
   impressão digital da avaliação: configuração da conexão e versões das credenciais, descoberta, o que cada concessão
   cobre, o que é retido, a política. Qualquer um deles mudando anula uma aprovação anterior. Nenhum `McpSecurity`.
6. **STDIO é `executável + args[] + env`, nunca um shell.** A validação recusa shells (`sh`, `bash`, `cmd`,
   `powershell`, `env`…, pelo nome em qualquer estilo de caminho), executáveis relativos ou com espaços, variáveis
   perigosas (`PATH`, `NODE_OPTIONS`, `LD_*`…, `ATLAS_*`) e **qualquer `${`**: a CLI expande `${VAR}` a partir do
   ambiente do Atlas no comando, argumentos e valores de um servidor, então uma configuração poderia, do contrário,
   nomear uma variável que guarda o segredo de outra conexão. O Atlas escreve essas referências ele mesmo, para os seus
   próprios segredos, e nada mais pode.
7. **Segredos vão para o armazenamento de credenciais do SO** pela porta `CredentialStore` (`KeyringCredentialStore`, o
   crate `keyring`, usado só em `infrastructure/`). A configuração guarda o nome e quando cada um foi guardado. Um
   segredo chega a um servidor como `${ATLAS_MCP_<SERVIDOR>_<NOME>}` na configuração e como um valor no **ambiente** do
   processo da CLI: nunca em um arquivo, um argumento, o prompt, o manifest, as métricas, um evento ou um log (`Secret`
   não tem `Display`/`Serialize` e tem um `Debug` que não mostra nada). Uma variável não definida **não** é um erro para
   a CLI (o texto fica literal), então o Atlas confere a presença primeiro e só lê o segredo depois que os guardrails
   permitiram o passo.
8. **A descoberta é o relatório do próprio runtime**, não um cliente MCP do Atlas: `ModelRuntime::probe_mcp` inicia o
   servidor pela CLI com um modelo que não existe, que informa o status do servidor e os nomes das suas ferramentas e
   falha na primeira requisição. Exige que a conexão esteja ligada (essa é a decisão do usuário de deixar o Atlas iniciar
   esse processo) e seus segredos guardados. Só nomes: o relatório não tem descrições nem esquemas, e nenhum é inventado
   (o `tool_definitions` do orçamento continua `Unknown`).
9. **Reter ferramentas, e o que isso não consegue fazer.** `--tools` não limita ferramentas MCP; `--disallowedTools
mcp__<servidor>__<ferramenta>` esconde uma descoberta; uma ferramenta que o servidor adiciona depois é listada de
   qualquer forma. Uma concessão ao servidor inteiro é, portanto, a granularidade padrão (o usuário confia no servidor),
   e ferramentas nomeadas precisam de uma descoberta para que o resto possa ser retido. Quando o runtime lista, antes de
   perguntar qualquer coisa a um modelo, uma ferramenta MCP que ninguém autorizou (uma ferramenta retida que voltou, uma
   que um servidor adicionou, um servidor que o Atlas não iniciou, como um conector do usuário), o Atlas para o passo:
   pede à sessão que termine (possível só quando a execução está ligada a um terminal), faz a execução falhar
   (`mcp_tool_not_authorized`) e não repassa a sua resposta. Isso é detecção mais uma parada de melhor esforço, não
   prevenção; uma requisição pode já ter sido enviada.
10. **Falha nunca vira permissão.** Uma conexão que não pode ser iniciada como planejado (o segredo sumiu, o cofre está
    travado) é deixada de fora do passo, e uma **obrigatória** o para (`mcp_connection_unavailable`). Uma execução
    somente texto nunca recebe um servidor (`--tools ""` não desliga o MCP, então a única forma é não dar nenhum). Um
    runtime que ninguém investigou não recebe nada (`McpSupport::NotInvestigated`).
11. **A invariante para resultados vale como na ADR 0027:** o resultado de uma ferramenta MCP é `Untrusted`; para um
    runtime de CLI ele nunca passa pelo Atlas. Um teste mostra que um texto afirmando que uma pessoa aprovou não consegue
    responder à pergunta que a política faz.

## Evidência (sem chamada de modelo)

- Claude Code 2.1.285, CLI real, o servidor inofensivo da Fase A: a sonda vê `connected` e `echo_static`; um segredo
  viaja ambiente → `${ATLAS_MCP_…}` → o servidor, que o devolve como um nome de ferramenta; um servidor que não consegue
  iniciar fica `failed` sem ferramentas; uma ferramenta retida não é listada e uma que o servidor adiciona depois é (e o
  evento `init` real chega ao fluxo do Atlas).
- 20 testes em nível de execução com o serviço real, o serviço de segurança real como resolvedor de políticas, o serviço
  MCP real sobre um armazenamento em memória (concessões por agente, workflow e passo; isolamento de workspace; negação
  somente leitura; revogar e desligar; ferramentas nomeadas; falhas obrigatórias e opcionais; segredos nunca em nada que
  seja guardado; aprovações anuladas por comando, versão da credencial, descoberta, concessão ou política; a parada em
  uma ferramenta não autorizada, conferida por mutação).

## Consequências

- Só o Claude pode receber servidores MCP. Os outros quatro runtimes são `NotInvestigated`: uma conexão concedida aos
  seus agentes simplesmente não é dada, com um achado.
- O ciclo completo do Keychain do macOS foi exercitado pelo teste real ignorado (nome exato). Os armazenamentos do
  Windows e do Linux não: o `keyring` compila para `x86_64-pc-windows-msvc`, o crate inteiro não pôde ser construído
  para ele aqui (sem `llvm-rc`), o alvo Linux não está instalado.
- A UI de conexões chegou na Fase E (doc seção 18): o painel de Integrações e um catálogo com o DevTools (fixado) e o
  Figma (listado, não adicionável).
- A parada em uma ferramenta não autorizada não precisa mais de uma sessão de terminal: `SessionRegistry::stop`
  encerra uma sessão de terminal ou faz o runner por pipe matar o grupo de processos (veja doc seção 17.7). Uma
  requisição pode já ter chegado ao modelo quando o relatório chega.
- Descrições e esquemas das ferramentas são desconhecidos do Atlas, então ele não pode mostrá-los nem orçá-los.
- Transporte HTTP e OAuth (o servidor oficial do Figma) não foram construídos; `McpTransport::Http` existe e é
  recusado.

## Não feito

Integrações com Chrome DevTools e Figma (Fase E; o Figma precisa de HTTP e OAuth), qualquer UI para adicionar ou
conceder uma conexão, um cliente MCP no Atlas, um Tool Gateway ou proxy, HTTP e sua camada de SSRF, interceptação por
chamada, MCP para os outros quatro runtimes, uma execução real com modelo.

## Acompanhamento da Fase F

Uma concessão que nomeia um workflow ou um passo é recusada quando esse workflow ou passo não existe, não é um passo de
agente, ou executa um agente diferente do que a concessão nomeia (doc seção 19.1).
