# Levantamento — Stitch × app (MCP Hub)

Fonte visual: HTML e PNG do Stitch em [`../stitch/`](../stitch/). Legenda: **existe**, **não existe**
(não implementado), **adaptado**.

## Moldura (não alterada)

O Stitch desenhou a moldura certa nesta rodada (sem busca ⌘K no topo, sem versão inventada); só traz o
nome inteiro da tela no breadcrumb ("MCP Hub — Servidores") e um ícone do hub no logo da H2. O app
mantém a sua moldura.

## H1 — Servidores

| Stitch                                                                                    | App                  |                                                                              |
| ----------------------------------------------------------------------------------------- | -------------------- | ---------------------------------------------------------------------------- |
| Título + subtítulo inteiro; sem mini-breadcrumb nem selo "JSON-RPC 2.0"                   | tinha os dois        | adaptado (removidos, como o plano decidiu)                                   |
| Verificar conexões, Configuração (menu), Adicionar servidor MCP                           | quatro botões        | adaptado: Sincronizar CLIs e atlas-mcp.json foram para o menu "Configuração" |
| Seis métricas em uma linha                                                                | seis caixas com dica | adaptado (dicas viram `title`; só "Conectados" mostra a % ao lado)           |
| Filtro Todos / Locais / Remotos, busca com ⌘K, chave "Apenas conectados"                  | existem              | existe                                                                       |
| Cartão: chave, nome, `stdio`, `obrigatória`, estado, Inspecionar, Testar, comando, resumo | existem              | existe                                                                       |
| Rodapé "ligada/desligada"                                                                 | existia              | não implementado: a chave e o selo dizem o mesmo                             |
| Avisos recolhíveis "N avisos" (vermelho para falha)                                       | avisos âmbar soltos  | adaptado: fechado, com a contagem e o tom do mais grave à vista              |
| Inspetor: abas, ferramentas em chips, concessões, "Conceder"                              | existe               | existe                                                                       |
| "Guardrails do Atlas"                                                                     | sempre aberto        | adaptado: recolhido (o Stitch o desenhou aberto)                             |
| Rodapé do inspetor com "Exportar…" e "Remover" lado a lado                                | idem                 | adaptado: menu ⋯ com Remover em vermelho (confirmação existente)             |
| Texto cortado no seletor "Escolher agente"                                                | —                    | artefato do Stitch, ignorado                                                 |

## H2 — Sem servidores

| Stitch                                                        | App                          |                                         |
| ------------------------------------------------------------- | ---------------------------- | --------------------------------------- |
| Cabeçalho idêntico ao da lista, sem métricas nem filtros      | métricas e filtros apareciam | adaptado (somem quando não há servidor) |
| Cartão central com ícone, texto, Adicionar e Sincronizar CLIs | texto e botões               | existe                                  |

## H3 — Adicionar servidor

| Stitch                                                                                                 | App                             |                          |
| ------------------------------------------------------------------------------------------------------ | ------------------------------- | ------------------------ |
| Modal largo, três abas                                                                                 | existe                          | existe                   |
| Cartões: título, "risco alto", Fonte ↗, descrição, "Executaria:", versão fixada, requisitos, Adicionar | existem                         | existe                   |
| "Catálogo testado e seguro", "Protocolo 2024-11-05", chips "remoto / oauth", ícones                    | —                               | não existe               |
| Botão "Indisponível no Atlas"                                                                          | botão de adicionar desabilitado | adaptado (vale o do app) |
| Linha de descrição de "Registro MCP" e "Manual" no topo                                                | —                               | não existe               |

## H4 — Importar e exportar

| Stitch                                                                                                 | App                      |                               |
| ------------------------------------------------------------------------------------------------------ | ------------------------ | ----------------------------- |
| Duas abas, nota, Formato, Ler o arquivo da CLI, Carregar arquivo…, Configuração, Importar (desligados) | existem                  | existe                        |
| "JSON syntax / raw payload", faixa de descrição da aba Exportar, ícones                                | —                        | não existe                    |
| Motivo "já existe ou protocolo incompatível" em "Deixados de fora"                                     | motivo real por servidor | adaptado (vale o motivo real) |

## H5 — Sincronizar CLIs

| Stitch                                                                      | App            |                                               |
| --------------------------------------------------------------------------- | -------------- | --------------------------------------------- |
| Nota com "Mais detalhes"                                                    | nota longa     | existe                                        |
| Quatro cartões de estado das CLIs                                           | lista          | adaptado                                      |
| Tabela servidor × CLI com selos de texto                                    | símbolos ✓ + · | adaptado (selos de texto, mesmo `aria-label`) |
| Títulos "Estado de detecção…" e "Mapeamento e plano…", chip `stdio`, ícones | —              | não existe                                    |
| Aviso de segredos e botão primário com a contagem                           | existem        | existe                                        |
