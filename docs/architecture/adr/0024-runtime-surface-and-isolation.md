# 0024 — Camada de Otimização, fase 4: o que um runtime consegue alcançar (superfície e isolamento)

## Contexto

A fase 4 da Camada de Otimização foi escrita em torno da seleção de ferramentas e de MCP: um catálogo, um seletor,
carregamento adiado. Antes de construir qualquer coisa disso, a pergunta foi o que um agente do Atlas de fato recebe. O
Atlas não escolhe ferramentas por um catálogo: cada adaptador de runtime passa flags fixas (`claude -p --tools
Read,Grep,Glob`), e o que a CLI faz em volta dessas flags é da CLI.

Isso foi medido, não presumido. O Claude Code informa o que carregou na mensagem `init` da sua saída `stream-json`.
Iniciando a CLI real (2.1.285) com **exatamente os argumentos que o Atlas usava** e um modelo que não existe (falha na
primeira requisição, então nenhum modelo é chamado e nada é gasto) obteve-se, em uma máquina de desenvolvedor com os
conectores e plugins de costume:

| Informado pela CLI ao iniciar        | Lançamento anterior do Atlas                                                                                                                      | Com `--strict-mcp-config --disable-slash-commands` |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------- |
| Ferramentas que o modelo pode chamar | **11**: `Read`, `Grep`, `Glob` e oito `mcp__claude_ai_Claude_Docs__*` (`batch`, `create`, `delete`, `export`, `guide`, `query`, `read`, `update`) | **3**: `Glob`, `Grep`, `Read`                      |
| Servidores MCP carregados            | 8 (Docs, Atlassian, Figma, Google Drive, Calendar, Gmail, Notion…; um conectado, o resto aguardando login)                                        | 0                                                  |
| Skills / slash commands              | 48 / 84                                                                                                                                           | 0 / 0                                              |

Ou seja, um agente que o Atlas descrevia como somente leitura (`tool_access: none`: "sem edição, sem shell, sem
ferramenta web") poderia, em princípio, chamar as ferramentas com poder de escrita dos conectores do usuário. Se uma
execução headless deixa essa chamada passar depende das configurações de permissão do usuário, que o Atlas nem lê nem
controla; a ferramenta estava nas mãos do modelo de qualquer forma. A mesma listagem também custa contexto em toda
execução: descrições de skills e ferramentas de que o agente nunca precisou.

## Decisão

**Isolar o runtime do Claude das extensões do próprio usuário, incondicionalmente** (`launch_args` em
`runtimes/claude.rs`):

- `--strict-mcp-config`, sem `--mcp-config`: nenhum servidor MCP é carregado.
- `--disable-slash-commands`: nenhuma skill. O Atlas tem a sua própria camada de skills (ADR 0022) e decide o que um
  agente recebe.

Isto é mais um endurecimento do que uma otimização (a superfície de ferramentas declarada passa a ser verdadeira), então
não tem chave de liga/desliga. Aplica-se a toda forma como o Atlas inicia o Claude (somente leitura, com edições,
somente texto). Não toca na autenticação: a CLI continua usando o login do próprio usuário (`--bare` não usaria, por
isso não é usado).

Não feito, de propósito: `--bare` ou `--restricted` (mudam a autenticação ou o tratamento de configurações, um passo
maior) e `--setting-sources` (os hooks, regras de permissão e arquivos `CLAUDE.md` do usuário ainda carregam; não estão
na superfície medida acima e precisam da sua própria medição).

**Registrar o que o runtime diz ter carregado** (observabilidade, por execução). O adaptador do Claude lê a mensagem
`init` para os fatos que já informa (`toolsExposed`, `mcpServers`, `skillsLoaded`, `slashCommands`, `pluginsLoaded`); as
métricas os guardam como `tools.exposed` e `extensions` (servidores MCP, skills, slash commands, plugins), e
`tools.used` conta as ferramentas que o modelo chamou pelo nome a partir dos eventos de ferramenta do runtime. Um
runtime que não informa os deixa `None`: nada é inferido.

Um teste opt-in (`cargo test real_claude_surface -- --ignored`) repete a medição contra a CLI real sem chamada de modelo.

## Não construído (e por quê)

- **ToolCatalog / ToolSelector / registro de MCP / descoberta adiada.** O Atlas dá a um agente uma lista fixa de
  ferramentas por modo (somente leitura, edição, nenhuma) e, agora, nenhum MCP. Não há escolha para um seletor fazer e
  nenhum servidor MCP que o próprio Atlas ofereça. No momento em que um agente precisar de um servidor MCP, o caminho é
  gerar um `--mcp-config` explícito por execução só com esse servidor; a busca de ferramentas e o carregamento adiado
  do próprio runtime são então a capacidade nativa a usar.
- **OpenCode, Gemini e Antigravity.** A superfície deles não foi medida: o OpenCode lê sua própria configuração e pode
  carregar os servidores MCP do usuário; Gemini e Antigravity negam shell e escrita de arquivos no modo headless, e o
  carregamento de MCP e extensões deles não é conhecido. Até serem medidos, nada é afirmado sobre eles e seus fatos no
  estilo `init` estão ausentes.
- **Descartar o `CLAUDE.md`, os hooks e as regras de permissão do usuário** nas execuções do Atlas. Eles ainda se
  aplicam. Fazer isso direito exige antes entender as flags seguras para a autenticação acima, e medir quanto contexto
  eles acrescentam.

## Consequências

- Um agente Claude no Atlas pode chamar exatamente `Read`, `Grep` e `Glob` (mais `Edit` e `Write` quando edições foram
  concedidas). Qualquer coisa além é uma decisão a tomar explicitamente, não uma herança da máquina de quem roda.
- O prompt que a CLI monta em volta do prompt do Atlas fica menor pela listagem de 48 skills, 84 slash commands e as
  definições das ferramentas MCP (ainda não medido em tokens: é preciso uma execução real para a entrada informada pelo
  runtime, que a fase 0 registra).
- As execuções do Atlas ficam mais reproduzíveis: deixam de depender de quais conectores o usuário tem logados.
