# Levantamento — Stitch × app (Configurações)

Fonte visual: HTML e PNG do Stitch em [`../stitch/`](../stitch/). Legenda: **existe**, **não existe**
(não implementado), **adaptado**. As cinco telas do Stitch são seções de uma página só no app.

## Moldura e índice

| Stitch                                                                               | App             |                                                                                                                |
| ------------------------------------------------------------------------------------ | --------------- | -------------------------------------------------------------------------------------------------------------- |
| Índice interno com Geral, Memória, Importar e exportar, Sobre e atualizações         | não havia       | adaptado: novo, 200 px, sem título; rola até a seção e marca a atual (no Stitch cada tela mostra uma seção só) |
| Rótulo "PREFERÊNCIAS" e chevron no item ativo (C1)                                   | —               | não existe                                                                                                     |
| Barra superior por cima da barra lateral (C3), "Configurações" como item avulso (C1) | moldura do app  | ignorado: vale a moldura real                                                                                  |
| Breadcrumb "Configurações — <seção>"                                                 | "Configurações" | adaptado (a seção não é uma rota)                                                                              |

## C1 — Geral

| Stitch                                                                                                       | App                                          |                                                         |
| ------------------------------------------------------------------------------------------------------------ | -------------------------------------------- | ------------------------------------------------------- |
| Idioma, Limite de tokens (+ Salvar), Sandbox dos runtimes, separados por linha fina, controle antes da ajuda | existiam em cartões, ajuda antes do controle | adaptado                                                |
| Selo verde "Este sistema consegue confinar processos."                                                       | texto                                        | adaptado (selo; âmbar quando não consegue)              |
| Caixa dos caches e sua ajuda                                                                                 | existe                                       | existe                                                  |
| "Como funciona" fechado (com "segurança & isolamento")                                                       | parágrafo longo antes do controle            | adaptado (recolhido; sem o rótulo mono, que não existe) |

## C2 — Memória

| Stitch                                                                                                             | App                |                                                                                                                       |
| ------------------------------------------------------------------------------------------------------------------ | ------------------ | --------------------------------------------------------------------------------------------------------------------- |
| Quatro abas com contagem                                                                                           | tudo empilhado     | adaptado: Onde as notas são usadas · Esperando sua decisão (n) · Notas guardadas (n) · Adicionar uma nota             |
| Cartão da nota: título, selo "Não confirmada", meta mono, "Usada 0×", texto em bloco, Editar / Descartar / Aprovar | existe             | existe (Aprovar primeiro, como no app)                                                                                |
| Três linhas de resumo no fim da página (vazaram do prompt)                                                         | —                  | não existe                                                                                                            |
| Texto de exemplo da nota (Jest/Postgres)                                                                           | texto real da nota | adaptado                                                                                                              |
| Abas não desenhadas (uso, guardadas, adicionar)                                                                    | existem            | existe, com o mesmo sistema de abas e selos                                                                           |
| Selos longos de proveniência                                                                                       | texto completo     | adaptado: selo curto ("Não confirmada", "Aguardando validação") com o texto completo em `title` e para leitor de tela |
| Arquivar e Apagar à vista                                                                                          | à vista            | adaptado: no ⋯ (Apagar em vermelho); Editar, Promover e "Usar de novo" ficam à vista                                  |

## C3 — Importar e exportar

| Stitch                                                                                    | App                                  |               |
| ----------------------------------------------------------------------------------------- | ------------------------------------ | ------------- |
| Dois cartões (Importar                                                                    | Exportar) e o passo "O que exportar" | botões soltos | adaptado |
| Descrições dos cartões, chip com os nomes dos agentes, seta em "Continuar", borda de foco | —                                    | não existe    |

## C4 — Revisar importação

| Stitch                                                                                                                               | App             |                                                                                          |
| ------------------------------------------------------------------------------------------------------------------------------------ | --------------- | ---------------------------------------------------------------------------------------- |
| Título + selos de arquivo e versão, aviso fixo                                                                                       | existem         | existe                                                                                   |
| Itens recolhíveis com selo de tipo e de situação                                                                                     | cartões abertos | adaptado: os escolhidos e os com conflito abrem; a dependência sem problema fica fechada |
| Escolha por botões de rádio                                                                                                          | lista de opções | adaptado (mantida a lista; nenhuma opção marcada por padrão)                             |
| "3 definições encontradas", "Mesmo hash SHA-256", "Sem conflitos", "Mapear para agente existente", faixa final "Pronto para aplicar" | —               | não existe                                                                               |
| Texto do aviso de substituir e título "Como deseja resolver o conflito"                                                              | texto do app    | adaptado                                                                                 |

## C5 — Sobre e atualizações

| Stitch                                                       | App          |                      |
| ------------------------------------------------------------ | ------------ | -------------------- |
| Cartão único com Sobre e Atualizações, ponto de estado verde | dois cartões | adaptado             |
| Estados "disponível" e "instalando" (só descritos)           | existem      | existe (ponto âmbar) |
