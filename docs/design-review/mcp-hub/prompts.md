# Prompts para o Stitch — MCP Hub

Ainda **não enviados**. Cada prompt = **P0** de [`../_comum/p0-regras.md`](../_comum/p0-regras.md)
(com `<NOME DA TELA>` trocado) + o bloco **TELA** abaixo. Projeto Stitch `526454930665111857`,
design system `assets/7aa0774b734b4149b2d38a23136aadfd`, `deviceType: DESKTOP`.

Telas novas aparecem ao lado das originais. A chamada dá timeout e a geração termina: não reenviar;
esperar o usuário confirmar e conferir com `list_screens`.

| Prompt | Nome da tela                               | Ferramenta                  | Base                               |
| ------ | ------------------------------------------ | --------------------------- | ---------------------------------- |
| H1     | MCP Hub — Servidores                       | `edit_screens`              | `2aa1bde31de946909b9583abdf233f9f` |
| H2     | MCP Hub — Sem servidores                   | `generate_screen_from_text` | (nova)                             |
| H3     | MCP Hub — Adicionar servidor               | `generate_screen_from_text` | (nova)                             |
| H4     | MCP Hub — Importar e exportar configuração | `generate_screen_from_text` | (nova)                             |
| H5     | MCP Hub — Sincronizar CLIs                 | `generate_screen_from_text` | (nova)                             |

Ordem: H1 → H3 → H4 → H5 → H2.

## H1 — MCP Hub — Servidores

```text
TELA: página "MCP Hub" com a lista de servidores e o inspetor do servidor escolhido.
Cabeçalho: título "MCP Hub e protocolos de contexto" e o subtítulo "Gerencie os servidores MCP locais
que dão ferramentas aos seus agentes. Nada aqui é ligado, iniciado ou concedido até você fazer isso,
e o que uma ferramenta devolve nunca é tratado como instrução." (em até duas linhas, texto
secundário). Sem mini-breadcrumb e sem selo "JSON-RPC 2.0". Ações à direita: botão primário
"+ Adicionar servidor MCP", botão secundário "Verificar conexões" e um menu "⋯ Configuração" com
"Sincronizar CLIs" e "Importar e exportar (atlas-mcp.json)".
Resumo: UMA linha de métricas compactas, cada uma com número e rótulo (sem caixas grandes): "3
Servidores · 1 Conectado (50% dos ligados) · 1 Falha / login · 1 Desligado · 8 Ferramentas listadas
· 3 Concessões".
Filtros numa única linha: segmento "Todos (3) | Locais (stdio) (3) | Remotos (HTTP/SSE) (0)" (o
último desabilitado), campo de busca "Buscar servidor, ferramenta, comando… (⌘K)" e a chave
"Apenas conectados".
Conteúdo em duas colunas: lista de servidores à esquerda (largura maior) e Inspetor MCP à direita.
Cada servidor é uma linha de lista compacta, não um cartão cheio: chave liga/desliga, nome, chip
"stdio", chip "obrigatória" quando for, selo de estado ("Conectado" verde, "Falhou" vermelho,
"Desligado" cinza, "Não examinado" cinza) e, abaixo, o comando em mono truncado
("npx -y @modelcontextprotocol/server-github@1.4.0"), uma linha de resumo "5 ferramentas · 2 agentes ·
examinado em 16/10/2026, 22:03 via claude" e, quando houver atenção, um indicador recolhido "2
avisos" que ao abrir mostra as mensagens (ex.: "Arquiteto Backend roda no Claude CLI, que não recebe
servidores MCP: não vai receber este.", "O último exame não conseguiu iniciar este servidor.",
"Segredo faltando: DATABASE_URL"). Ações da linha: "Inspecionar" e "Testar" (ghost). Mostre 3
servidores: github (Conectado, selecionado), playwright (obrigatória, Falhou) e postgres (Desligado,
sem ferramentas examinadas, "Concedido a ninguém").
Inspetor MCP (cabeçalho com o nome "github" e o selo "Conectado"), abas "Ferramentas (5) | Variáveis
e env | Manifesto JSON | Atividade". Na aba Ferramentas: seção "O que oferece" com as ferramentas como
chips mono (create_issue, list_pull_requests, get_file_contents, search_code, create_branch) e a
nota "O Atlas lista as ferramentas de um servidor pelo nome e não as chama manualmente: uma ferramenta
só roda quando um agente que a recebeu a usa em uma etapa."; seção "Quem pode usar" com a lista
"Arquiteto Backend — todas as ferramentas" e "Implementador API — somente create_issue, search_code",
cada uma com "Revogar", e o formulário "Conceder". O bloco "Guardrails do Atlas" (selo "políticas do
kernel", três regras) fica numa seção recolhida. No rodapé do inspetor: menu "⋯" com "Exportar…" e,
em vermelho e separado, "Remover".
USE só esses dados. NÃO mude: as abas, as métricas, os filtros, o que cada linha informa.
```

## H2 — MCP Hub — Sem servidores

```text
TELA: a página "MCP Hub" sem nenhum servidor. Cabeçalho igual ao da lista (título, subtítulo e
ações), sem métricas, sem filtros e sem inspetor. No centro, um estado vazio com o texto "Nenhum
servidor neste workspace ainda. Adicione um pelo Registro MCP, ou traga o que suas CLIs já têm." e dois
botões: "Adicionar servidor MCP" (primário) e "Sincronizar CLIs" (secundário).
```

## H3 — MCP Hub — Adicionar servidor

```text
TELA: a página "MCP Hub — Servidores" esmaecida ao fundo e, sobre ela, um modal largo "Adicionar um
servidor MCP" com três abas: "Registro MCP", "Conhecidos do Atlas" (aberta) e "Manual".
Na aba "Conhecidos do Atlas", uma lista de cartões simples com: nome ("Chrome DevTools"), selo
"risco alto", uma frase de descrição ("Permite que um agente controle e inspecione um navegador
Chrome: páginas, rede, console, capturas de tela, traces de desempenho."), uma linha mono "Executaria:
npx -y chrome-devtools-mcp@0.2.1", a nota "Fixado na versão 0.2.1: roda a versão que foi revisada,
não a mais recente.", os requisitos como selos pequenos ("✓ npx — encontrado", "✓ Google Chrome —
encontrado") e, no fim, "Adicionar (fica desligada)" (primário) e o link "Fonte". Mais dois cartões:
"Figma" e "Jira e Confluence (Atlassian)", ambos com um aviso âmbar "Ainda não disponível: o servidor
oficial é remoto (HTTP) e exige login OAuth, que o Atlas não suporta." e o botão desabilitado. Se um
cartão já foi adicionado, o botão mostra "Adicionada".
Descreva também, em uma linha cada, o conteúdo das outras abas (sem desenhá-las): "Registro MCP": nota
"Busque no Registro MCP oficial…", campo "Buscar servidores (Jira, GitHub, Postgres…)" e "Buscar";
"Manual": nome, programa, argumentos e variáveis.
```

## H4 — MCP Hub — Importar e exportar configuração

```text
TELA: a página "MCP Hub — Servidores" esmaecida ao fundo e, sobre ela, um modal "Importar e exportar
configuração (atlas-mcp.json)" com duas abas, "Importar" (aberta) e "Exportar".
Aba Importar: a nota "Cole uma configuração, carregue um arquivo ou leia o arquivo de uma CLI. Os
servidores entram desligados e sem concessão; uma credencial encontrada no texto vai para o cofre de
credenciais do sistema, nunca para a configuração."; seletor "Formato" (Detectar; opções: mcpServers
genérico, Claude Code, Gemini CLI, OpenCode, Codex, GitHub Copilot CLI, Antigravity); dois botões
secundários "Ler o arquivo da CLI" e "Carregar arquivo…"; uma área de texto "Configuração"; e o botão
"Importar (desligados)". Depois da importação, um resumo em três linhas: "2 servidor(es) adicionado(s).",
"Deixados de fora: 1" com o motivo, e "credencial guardada: GITHUB_TOKEN".
Descreva em uma linha a aba Exportar (sem desenhá-la): formato, "Só o servidor selecionado", prévia
"Configuração gerada", "Copiar", "Salvar como arquivo" e "Adicionar ao arquivo da CLI…".
```

## H5 — MCP Hub — Sincronizar CLIs

```text
TELA: a página "MCP Hub — Servidores" esmaecida ao fundo e, sobre ela, um modal largo "Sincronizar
servidores MCP entre suas CLIs". Nota curta no topo ("O Atlas lê a configuração do Claude Code, Gemini
CLI, OpenCode e Codex, traz para o workspace (desligado) o que qualquer uma delas tem, e adiciona ao
arquivo de cada CLI os servidores que faltam. Nada é sobrescrito, cada arquivo é copiado antes de
mudar.", o resto da nota recolhido). Linha de estado por CLI com selos: "Claude Code — arquivo
encontrado", "Gemini CLI — sem arquivo", "OpenCode — arquivo encontrado", "Codex — ilegível (não
mexemos)". Tabela com uma linha por servidor (github, playwright, postgres) e uma coluna por CLI; cada
célula é um selo pequeno: "tem", "será adicionado", "será trazido para o Atlas", "indisponível" ou "sem
arquivo". Abaixo, o aviso "Estes precisam de valor (abra o servidor, aba “Variáveis e env”), e a CLI
precisa da variável no ambiente: GITHUB_TOKEN". Rodapé: "Cancelar" e o botão primário "Sincronizar:
adicionar 2 a 1 arquivo(s), trazer 1 para o Atlas". Estado alternativo: o texto "Tudo está em dia".
```

## Registro de envios

| Prompt         | Enviado | Tela gerada no Stitch | Observação |
| -------------- | ------- | --------------------- | ---------- |
| H1 | 2026-10-16 | "MCP Hub — Servidores" `fa3cca4a27654726aacc04dca4b578b4` (2560×2048) | `edit_screens` criou tela nova; a original `2aa1bde3…` segue intacta. Avaliação abaixo. |
| H3 | 2026-10-16 | "MCP Hub — Adicionar servidor" `53d6a8c1b2b64645a9cd1c5ef1f6f302` (2560×2048) | `generate_screen_from_text` com o design system, sem timeout. |

## Avaliação

### H1 — MCP Hub — Servidores

Fiel ao prompt: cabeçalho com subtítulo em duas a três linhas, métricas em uma linha, filtros em uma
linha, lista compacta de três servidores (github selecionado, playwright com "obrigatória" e "Falhou",
postgres desligado e esmaecido), avisos recolhíveis com as mensagens, inspetor com as quatro abas, chips
mono das ferramentas, nota, concessões com "Revogar" e formulário "Conceder". Moldura correta (sem busca
⌘K na barra superior, sem versão inventada; o ⌘K aparece só no campo de busca, que foi pedido).

Desvios para a implementação tratar:

- "Configuração" saiu como botão com "⋯" à esquerda, não como menu; implementar como `OverflowMenu`.
- "Guardrails do Atlas" saiu **aberto**; o prompt pedia recolhido (usar `CollapsibleSection` recolhida).
- O rodapé do inspetor mostra "Exportar…" com "⋯" e "Remover" como botões lado a lado, em vez de menu
  com "Remover" separado em vermelho; seguir o prompt (menu "⋯", "Remover" em vermelho separado).
- O seletor "Escolher agente" aparece com o texto cortado na vertical (artefato do Stitch).
- O aviso do playwright mostra só duas mensagens (as do prompt eram exemplos); manter as mensagens reais.
- A linha de métricas usa mono em tudo; rótulos ficam em fonte de texto, só os números em mono.

### H3 — MCP Hub — Adicionar servidor

Fiel: modal largo sobre a página esmaecida, três abas com "Conhecidos do Atlas" aberta, cartão do Chrome
DevTools completo (selo "risco alto", comando mono, nota de versão, requisitos verdes, "Adicionar (fica
desligada)" e "Fonte"), Figma e Jira com aviso âmbar e botão desabilitado. Moldura correta.

O Stitch mostra, o app não tem (não implementar):

- Texto "Catálogo testado e seguro" ao lado das notas das abas e rodapé "Atlas MCP Hub • Protocolo 2024-11-05".
- Chip "stdio" dentro do bloco do comando e chips "remoto / oauth" nos cartões Figma e Jira.
- Ícones decorativos em cada cartão e na aba ativa; ponto índigo antes do título.
- Botão desabilitado com o texto "Indisponível no Atlas" (o app mostra o botão de adicionar desabilitado).

Desvios de conteúdo: a nota diz "versão 0.21" por erro de renderização (o texto real é 0.2.1); as
descrições de "Registro MCP" e "Manual" apareceram como linha de texto no topo, o que o app não tem (cada
aba mostra só o próprio conteúdo). Descrição da aba "Registro MCP" foi cortada: "Busque no Registro MCP
oficial…" é só a nota inicial da aba.
