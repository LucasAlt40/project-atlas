# Levantamento — Stitch × app (Agentes e Personalidades)

Fonte visual: HTML e PNG exportados do projeto Stitch `526454930665111857` em [`../stitch/`](../stitch/).
Legenda: **existe** = o app tem o dado ou a ação; **não existe** = o Stitch mostra, o app não tem (não
implementado); **adaptado** = o app tem algo equivalente com outro nome, texto ou lugar.

## Moldura (não alterada)

| Elemento do Stitch                                                                 | App                                   |
| ---------------------------------------------------------------------------------- | ------------------------------------- |
| "Atlas Core · v2.4.0 · Cluster" no topo da barra lateral e "v0.1.0" no rodapé (A1) | não existe                            |
| "Configurações" separado dos demais itens (A2)                                     | adaptado: o app as tem na mesma lista |
| Ponto verde ao lado do workspace (P1)                                              | não existe                            |
| Breadcrumb com o nome inteiro da tela ("Agentes — Lista de agentes")               | adaptado: o app mostra "Agentes"      |

## A1 e A2 — Agentes: lista e vazio

| Stitch                                                                          | App                             |                                                                        |
| ------------------------------------------------------------------------------- | ------------------------------- | ---------------------------------------------------------------------- |
| Título + chip "3 no total", subtítulo, busca, Importar, + Criar agente          | existe                          | existe                                                                 |
| Resumo em uma linha ("3 agentes · 2/2 runtimes prontos · …")                    | quatro caixas (`agents.stat.*`) | adaptado: os quatro números continuam, em uma linha                    |
| Abas Todos / Prontos / Indisponíveis com contagem                               | filtro                          | existe                                                                 |
| Cartão: ícone, nome, selo da personalidade, runtime, selo PRONTO / INDISPONÍVEL | existe                          | existe                                                                 |
| Quatro dados em pares, sem caixa interna                                        | existe (caixa interna)          | adaptado                                                               |
| Ícone por personalidade (compasso, selo, código)                                | ícone único de agente           | não existe                                                             |
| "Editar" + ⋯ com Exportar e Excluir                                             | três ícones sem legenda         | adaptado (ações com texto; confirmação de exclusão continua no cartão) |
| Estado vazio com texto e "Criar agente" (também no cabeçalho)                   | só o texto                      | adaptado                                                               |

## A3 — Agentes: criar e editar

| Stitch                                                                                              | App                   |                                                       |
| --------------------------------------------------------------------------------------------------- | --------------------- | ----------------------------------------------------- |
| "← Agentes" + título                                                                                | crumb sem ação        | adaptado (agora volta à lista)                        |
| Subtítulo "Configure a identidade, runtime e políticas de isolamento."                              | —                     | não existe                                            |
| Cinco seções (Identidade & papel, Provedor & runtime, Instruções adicionais, Resultado, Permissões) | existem               | existe                                                |
| Marcador (ponto) no título de cada seção                                                            | barra de acento       | não implementado: o prompt pedia títulos sem marcador |
| "Detectar runtimes novamente" no cabeçalho da seção                                                 | botão no fim da seção | adaptado                                              |
| Placeholder e ajuda de "Instruções adicionais", "Markdown / Plaintext"                              | textos do app         | não existe                                            |
| Texto do isolamento ("Cria uma árvore de trabalho temporária…")                                     | texto do app          | adaptado (vale o do app)                              |
| Rodapé fixo Cancelar / Criar agente                                                                 | existe                | existe                                                |

## P1 — Personalidades: biblioteca

| Stitch                                                                                       | App                    |                                                     |
| -------------------------------------------------------------------------------------------- | ---------------------- | --------------------------------------------------- |
| Título "Personalidades — Biblioteca"                                                         | "Personalidades"       | adaptado (nome mantido)                             |
| Chips, Importar, Restaurar (ghost), Nova personalidade                                       | existem                | existe                                              |
| Aviso "Injeção global do protocolo Atlas" (veio aberto)                                      | sempre aberto          | adaptado (recolhível, fechado, como o prompt pedia) |
| Abas com contagem e busca                                                                    | existem                | existe                                              |
| Cartão compacto + um aberto por vez, Usar esta personalidade + ⋯ (Editar, Exportar, Excluir) | cartão sempre completo | adaptado                                            |
| Tags e descrições de exemplo                                                                 | dados reais do app     | adaptado                                            |

## P2 — Personalidades: nova e editar

| Stitch                                          | App                      |                                        |
| ----------------------------------------------- | ------------------------ | -------------------------------------- |
| Painel lateral de 440 px sobre a biblioteca     | terceira coluna da grade | adaptado (painel sobreposto de 32 rem) |
| Título + selo, quatro campos, Cancelar / Salvar | existe                   | existe                                 |
| Placeholder longo de "Instruções" (C#, MediatR) | placeholder do app       | não existe                             |
| Nota de personalidade nativa não editável       | existe                   | existe (mantida)                       |
