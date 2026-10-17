# Plano de implementação — Repaginação do MCP Hub

Branch `design/mcp-hub`, criada a partir de `design/workspace-harness` (ou de `main`, se essa já tiver
sido integrada; a base compartilhada `Badge`, `CollapsibleSection`, `OverflowMenu`, `Steps` precisa
existir). Princípios, fases, fonte do visual, relatório e regras de divergência:
[`../_comum/principios-implementacao.md`](../_comum/principios-implementacao.md). Dossiê:
[`README.md`](README.md).

## 1. Telas e código

Ids a preencher depois do envio dos prompts (registro em `prompts.md`).

| #   | Tela (nome no Stitch)                      | Id da tela nova | Código do app (hipótese a confirmar)                                                                                                                                       |
| --- | ------------------------------------------ | --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| H1  | MCP Hub — Servidores                       | _a preencher_   | `workspace/pages/McpHubPage.tsx`, `workspace/components/hub/ServerCard.tsx`, `hub/Inspector.tsx`, `hub/GrantAgents.tsx`, `hub/McpHub.module.css`, `workspace/model/hub.ts` |
| H2  | MCP Hub — Sem servidores                   | _a preencher_   | `McpHubPage.tsx` (ramo vazio)                                                                                                                                              |
| H3  | MCP Hub — Adicionar servidor               | _a preencher_   | `hub/AddServer.tsx` (abas Registro MCP, Conhecidos do Atlas, Manual)                                                                                                       |
| H4  | MCP Hub — Importar e exportar configuração | _a preencher_   | `hub/ConfigIo.tsx`                                                                                                                                                         |
| H5  | MCP Hub — Sincronizar CLIs                 | _a preencher_   | `hub/SyncRuntimes.tsx`                                                                                                                                                     |

Compartilhado com outras telas: `workspace/components/connectionParts` e `McpPanel.tsx` (painel MCP
na gaveta de detalhes do agente, módulo 1). Textos em `hub.*` e `integrations.*` de `pt-BR.ts` e
`en-US.ts`.

## 2. Decisões já tomadas para este módulo

- **Cabeçalho**: ação primária `Adicionar servidor MCP`; `Verificar conexões` secundária;
  `Sincronizar CLIs` e `atlas-mcp.json` (importar e exportar) num `OverflowMenu` "Configuração". Sem o
  mini-breadcrumb "…/ mcp" e sem o selo "JSON-RPC 2.0" (jargão repetido). O subtítulo continua inteiro.
  O atalho ⌘K da busca continua.
- **Métricas**: as seis continuam existindo (`hub.metric.*`, com dica), numa linha compacta.
- **Linha de servidor**: chave liga/desliga, nome, `Badge` de transporte, `Badge` "obrigatória", `Badge`
  de estado, comando em mono truncado com `title` com o texto completo, linha de resumo, ações. O rodapé
  "ligada/desligada" some **porque a chave e o selo já dizem** (informação não se perde; o `aria-label`
  da chave continua).
- **Avisos**: agrupar os avisos de cada servidor num indicador recolhível "N avisos" com o tom do mais
  grave. **Nada pode ficar escondido sem sinal**: um servidor com falha ou segredo faltando mostra o
  tom e a contagem mesmo fechado. Os textos (`hub.card.*`, `hub.grant.*`) não mudam.
- **Inspetor**: coluna fixa; abas iguais; explicações viram texto secundário; "Guardrails do Atlas" numa
  `CollapsibleSection` fechada; `Remover` (confirmação existente) e `Exportar…` num `OverflowMenu`,
  `Remover` com `danger`.
- **Modais**: continuam modais (`Modal` largo); só a hierarquia muda. A regra "entra desligado e sem
  concessão" e os textos de segurança continuam.
- **Ligar**: a confirmação "Ligar…" continua (é regra de segurança do produto).

## 3. Cuidados específicos

- Nada nesta tela liga, inicia ou concede sozinho: a repaginação não pode introduzir atalho que ligue ou
  conceda sem a confirmação atual.
- `McpHubPage.test.tsx` (16 KB) cobre comportamento. Se uma ação mudar de lugar (menu "Configuração"),
  ajustar o **caminho** do teste, nunca a asserção; justificar no relatório.
- `connectionParts` e `McpPanel` são usados pela gaveta do agente: conferir que continuam corretos.
- A lista deve aguentar 30+ servidores e nomes/comandos longos sem quebrar a linha.
- Estados `failed`, `needs_auth`, `pending`, `unknown`, `not_examined` e `off` precisam do mesmo
  sistema de `Badge`.

## 4. Fases

| Fase | Entrega                                                                                                                                                                                        |
| ---- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 0    | Preparação (ver `_comum`), capturas "antes" com `?mcp=some` e `?mcp=none` (esperar 2–3 s)                                                                                                      |
| 1    | `implementacao/levantamento.md` + exportação do Stitch para `stitch/`                                                                                                                          |
| 2    | Base compartilhada: só o que faltar                                                                                                                                                            |
| 3    | H1 e H2 (lista, linha, inspetor, vazio)                                                                                                                                                        |
| 4    | H3 (adicionar servidor, as três abas)                                                                                                                                                          |
| 5    | H4 e H5 (importar/exportar, sincronizar)                                                                                                                                                       |
| 6    | Responsividade 720–2560 (o mestre-detalhe vira lista + painel sobreposto abaixo de ~1000 px) e estados (0, 1, 30 servidores; falha; segredo faltando; verificação em andamento; comando longo) |
| 7    | Testes, `npm run check`, relatório                                                                                                                                                             |

## 5. Captura

`_comum/captura/` com `?mcp=none|some`. Os diálogos de sincronizar e exportar precisam de comandos que o
mock não responde; estender o mock em `tmp/` (por exemplo `plan_mcp_sync`, `read_runtime_mcp_config`,
`export_mcp_config`) para capturar os estados completos.

## 6. Fora do escopo

Lógica do núcleo MCP, registro MCP, cofre de credenciais, Rust, DTOs, comandos; o painel do agente
além de garantir que não quebra; os outros módulos.
