# Revisão de design — MCP Hub

Módulo 3 da repaginação (processo em [`../_comum/README.md`](../_comum/README.md)). O MCP Hub reúne
os servidores MCP do workspace ativo: o que cada um é, como foi examinado por último, quem pode
usá-lo, como trazer mais servidores e como manter a configuração de cada CLI em dia. Nada nele liga,
inicia ou concede por conta própria.

Capturas em 1440×900 com dados de exemplo em [`atual/`](atual/). Capturar:
`?mcp=none|some` (padrão `some`: três servidores — `github` conectado, `playwright` com falha e
obrigatório, `postgres` desligado e sem segredo — e três concessões). O catálogo usa ids reais
(`chrome-devtools`, `figma`, `jira`). A tela carrega tarde (`lazy`): esperar 2–3 s depois de abrir.
O diálogo de sincronizar fica em "Lendo os arquivos das CLIs…" porque o mock não responde a esses
comandos; estender o mock em `tmp/` se precisar do resto.

## 1. Moldura

Igual à dos outros módulos. A tela tem um breadcrumb próprio ("Transport ERP / mcp" + selo
"JSON-RPC 2.0") acima do título, que repete o da barra superior.

## 2. Telas, estados e interações

### 2.1 MCP Hub — lista de servidores **[img 01, 02]**

- **Cabeçalho**: título "MCP Hub e protocolos de contexto" e subtítulo longo ("Gerencie os servidores
  MCP locais… o que uma ferramenta devolve nunca é tratado como instrução."). Quatro ações no topo:
  `Verificar conexões` (examina todos os ligados, um após o outro: "Verificando 2/3…"), `Sincronizar
CLIs`, `atlas-mcp.json` (importar e exportar) e `+ Adicionar servidor MCP` (primário).
- **Seis métricas**: Servidores, Conectados (com % dos ligados), Falha / login, Desligados,
  Ferramentas listadas, Concessões — cada uma com uma dica pequena.
- **Filtros**: segmento `Todos (3) · Locais (stdio) (3) · Remotos (HTTP/SSE) (0)` (remotos desabilitado:
  ainda não suportado), busca "Buscar servidor, ferramenta, comando… (⌘K)" (o atalho existe) e a
  chave `Apenas conectados`.
- **Mestre e detalhe**: à esquerda a lista de cartões de servidor; à direita o Inspetor do selecionado.
- **Cartão de servidor**: ícone, nome, chips `stdio` e `obrigatória`, selo de estado (`Conectado`,
  `Falhou`, `Desligado`, `Não examinado`, `Pendente`, `Desconhecido`), chave liga/desliga; o comando em
  mono (`npx -y @modelcontextprotocol/server-github@1.4.0`); chips "5 ferramentas", "Agentes: A, B",
  "Examinado em 16/10/2026, 22:03:31 via claude" (ou "Ferramentas desconhecidas até examinar",
  "Concedido a ninguém"); **avisos âmbar**, um por agente que roda numa CLI que não recebe servidores
  MCP ("Arquiteto Backend roda no Claude CLI, que não recebe servidores MCP: não vai receber este."),
  o aviso de falha do último exame, e "Segredo faltando: DATABASE_URL"; rodapé "ligada/desligada",
  `Inspecionar` e `Testar`.
- **Ligar** pede confirmação ("Ligar…" → texto → `Ligar`); **Testar** examina esse servidor.
- **Estados**: sem workspace ("Abra um workspace para gerenciar seus servidores MCP."), carregando, erro,
  nenhum servidor (texto + `Adicionar servidor MCP` + `Sincronizar CLIs`), nenhum corresponde à busca,
  erro de ação em linha, verificação em andamento.

### 2.2 Inspetor MCP (coluna da direita) **[img 02]**

Cabeçalho "Inspetor MCP" + nome + selo de estado. Quatro abas:

- **Ferramentas (5)**: "O que oferece" (texto sobre examinar, escolha do runtime para examinar,
  botão `Examinar`; as ferramentas listadas como chips mono) e "Quem pode usar" (lista de concessões com
  `Revogar`, formulário `Conceder` com agente, todas as ferramentas ou só as escolhidas, `Marcar todos`,
  `Limpar`, `Aplicar (n)`, concessão avançada por workflow ou etapa); aviso por agente que não vai receber.
- **Variáveis e env**: variáveis do servidor; segredos com "guardado / não guardado", campo de valor,
  `Guardar`, `Esquecer`.
- **Manifesto JSON**: como o Atlas inicia o servidor (segredo só pelo nome), `Editar` / `Salvar` (salvar
  desliga o servidor e esquece o que se aprendeu).
- **Atividade**: Último estado, Examinado via, Quando, Ferramentas listadas; nota de que não há logs do
  servidor; bloco "Guardrails do Atlas" com selo "políticas do kernel" e três regras.
- Rodapé: `Remover` (com confirmação "Remover com suas concessões e segredos") e `Exportar…`.

### 2.3 Adicionar um servidor MCP (modal, 3 abas) **[img 03, 04]**

- **Registro MCP**: nota, busca "Buscar servidores (Jira, GitHub, Postgres…)" + `Buscar`, resultados
  com o que o Atlas iniciaria, argumentos, segredos, "só remoto" / "não roda aqui", "Adicionar também ao
  arquivo próprio da CLI", `Carregar mais`.
- **Conhecidos do Atlas** (catálogo): cartões com nome, "risco alto", descrição, "Executaria: …", versão
  fixada, requisitos ("✓ npx — encontrado", "Google Chrome — encontrado"), `Adicionar (fica desligada)`
  ou "Ainda não disponível…" (HTTP/OAuth), link `Fonte`. Itens já adicionados: `Adicionada`.
- **Manual** (**[código]**): nome (minúsculas, dígitos, `-` e `_`), programa, argumentos, variáveis (valor ou
  segredo), "obrigatória/opcional".

### 2.4 atlas-mcp.json: importar e exportar (modal) **[img 05]**

- **Importar**: nota, `Formato` (Detectar, mcpServers genérico, Claude Code, Gemini CLI, OpenCode, Codex,
  GitHub Copilot CLI, Antigravity), `Ler o arquivo da CLI`, `Carregar arquivo…`, área "Configuração",
  `Importar (desligados)`; resultado com adicionados, deixados de fora (com motivo), credenciais guardadas.
- **Exportar** (**[código]**): formato, "Só o servidor selecionado", prévia "Configuração gerada",
  `Copiar`, `Salvar como arquivo`, `Adicionar ao arquivo da CLI…` (com cópia de segurança).

### 2.5 Sincronizar servidores MCP entre suas CLIs (modal) **[img 06, código]**

Nota longa, tabela Servidor × CLI (Claude Code, Gemini CLI, OpenCode, Codex…) com células "tem / será
adicionado / será trazido para o Atlas / indisponível / sem arquivo", estado por arquivo ("arquivo
encontrado", "sem arquivo", "ilegível (não mexemos)"), aviso de segredos, ação "Sincronizar: adicionar N a
M arquivo(s), trazer K para o Atlas", "Tudo está em dia".

### 2.6 Atalho vindo do Workspace

O botão `Integrações` do cabeçalho do Workspace (e `McpPanel` no agente) leva a esta tela.

## 3. Problemas de design observados

1. **Quatro botões de peso quase igual no topo**; `atlas-mcp.json` é nome de arquivo, não de ação; o
   mini-breadcrumb "Transport ERP / mcp" + "JSON-RPC 2.0" repete a barra superior e acrescenta jargão.
2. **Três faixas de controle antes do primeiro servidor**: seis métricas com dicas minúsculas, o
   segmento de filtro, a busca e a chave. A lista começa abaixo da dobra.
3. **Cartão de servidor sobrecarregado**: ícone, nome, dois chips, selo, chave, comando, três chips de
   informação, vários avisos âmbar (um por agente) e um rodapé. Os avisos dominam o cartão e se repetem
   igual entre cartões.
4. **Estado repetido**: selo `Conectado`, chave ligada e a palavra "ligada" no rodapé dizem a mesma coisa.
5. **Mestre-detalhe estreito**: a lista e o inspetor dividem 1440 px e os dois apertam; o inspetor repete
   estado e ferramentas que o cartão já mostrou.
6. **Inspetor longo e sem hierarquia**: parágrafos de explicação no mesmo peso dos dados ("O que
   oferece", "Quem pode usar"), e o bloco de guardrails ocupa espaço o tempo todo.
7. **Modais com muito texto**: o de adicionar tem três abas, cada uma com cartões longos; o de importar
   começa com um parágrafo denso; o de sincronizar é uma tabela de células com texto.
8. **Vocabulário de estado inconsistente**: `Falhou` / `Falha / login` / `Desconhecido` / `Não
examinado` aparecem em lugares com formatos diferentes.
9. **Ações destrutivas e de risco** (`Remover`, `Salvar` do manifesto que desliga o servidor) com o mesmo
   peso visual das seguras.

## 4. Tela do Stitch que já existe (versão anterior)

| Tela do app                | Tela anterior no Stitch (id)                                                                 |
| -------------------------- | -------------------------------------------------------------------------------------------- |
| MCP Hub — lista e inspetor | Atlas - MCP Hub & Gerenciador de Protocolos de Contexto (`2aa1bde31de946909b9583abdf233f9f`) |

Os modais não têm tela no Stitch.

## 5. Lacunas

Não capturados: sem servidores (`?mcp=none`), erro, verificação em andamento, abas Variáveis e env e
Manifesto JSON, aba Manual, exportar, sincronizar com dados reais, confirmação de ligar e de remover.
