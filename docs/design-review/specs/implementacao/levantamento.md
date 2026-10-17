# Levantamento — Stitch × app (Specs)

Fonte visual: HTML e PNG do Stitch em [`../stitch/`](../stitch/). Legenda: **existe**, **não existe**
(não implementado), **adaptado**. Moldura do Stitch ignorada em todas as telas (em S7 voltou a trazer
"Core", "Novo Fluxo" e "Configurações" no rodapé; em S5 e S2 também separou "Configurações").

## S1 e S2 — Lista e vazio

| Stitch                                                                                          | App                 |                                                                                          |
| ----------------------------------------------------------------------------------------------- | ------------------- | ---------------------------------------------------------------------------------------- |
| Cabeçalho sem a trilha "SDD · Desenvolvimento orientado a especificação / OpenSpec"             | tinha a trilha      | adaptado: removida (jargão que o Stitch omitiu)                                          |
| Sem a faixa de contadores                                                                       | três contadores     | adaptado: os mesmos números estão nas abas "Em andamento", "Aguardando você" e "Prontas" |
| Abas com contagem + busca na mesma linha                                                        | chips + busca       | adaptado                                                                                 |
| Importar specs (secundário) e Nova especificação (primário)                                     | existem             | existe                                                                                   |
| Cartão: barra colorida, id, selo de estado, título, pedido, avanço, agente, fase, avisos, tempo | existe              | existe (selo agora é o `Badge`)                                                          |
| Rótulo "Aprovação técnica" acima da barra                                                       | —                   | não existe                                                                               |
| Pedidos, agentes, ids e etapas de exemplo ("Design Técnico", "Validação Final")                 | dados reais         | adaptado                                                                                 |
| Texto cortado no pé do cartão e "+ +" no botão                                                  | —                   | defeitos do Stitch, não repetidos                                                        |
| S2: cartão central e ações duplicadas no cabeçalho                                              | cartão com as ações | adaptado: no vazio só o cartão tem as ações (o cabeçalho as mostra só com specs)         |

## S3 — Documento em revisão

| Stitch                                                                          | App                          |                                                                                            |
| ------------------------------------------------------------------------------- | ---------------------------- | ------------------------------------------------------------------------------------------ |
| Cabeçalho com seta, título, selo, metadados em mono, "Agentes" e ⋯              | "Agentes" e "Excluir"        | adaptado: Excluir foi para o ⋯ (danger); some quando não pode excluir                      |
| Stepper em cinco cartões                                                        | existe                       | existe                                                                                     |
| "Sua decisão" logo abaixo do stepper                                            | barra no fundo               | adaptado: sobe para o topo; em página estreita volta a ficar fixa no rodapé                |
| Documento renderizado em blocos (ADDED, requisito, cenário WHEN/THEN em caixas) | markdown/renderização do app | não implementado: vale a renderização atual                                                |
| Validações automáticas, Histórico, Consumo recolhíveis na coluna lateral        | painéis soltos               | adaptado: `CollapsibleSection` (Consumo mostra o alerta de limite no topo quando atingido) |
| Títulos em caixa alta, ícones nos títulos                                       | —                            | não existe                                                                                 |

## S4 — Agente escrevendo

| Stitch                                                                                            | App         |            |
| ------------------------------------------------------------------------------------------------- | ----------- | ---------- |
| Faixa de execução no lugar de "Sua decisão", cancelar, detalhes                                   | `RunBanner` | existe     |
| "transmissão em tempo real", "Markdown", selo "Aguardando", "—" no histórico, percentual na etapa | —           | não existe |

## S5 — Perguntas e achados

| Stitch                                                                                  | App                                         |                                                            |
| --------------------------------------------------------------------------------------- | ------------------------------------------- | ---------------------------------------------------------- |
| Cartão "O agente precisa da sua resposta" com pergunta, campo e envio                   | `QuestionCard`                              | existe (barra de acento e pergunta em bloco)               |
| "Requer decisão", "PERGUNTA DO AGENTE", placeholder, "⌘ + Enter", seta no botão, ícones | —                                           | não existe                                                 |
| "Este documento deixa N pergunta(s) em aberto" + Responder                              | `OpenQuestions`                             | existe                                                     |
| Achados da revisão (resumo, achado expandido, ações, "Achados que você já decidiu")     | na revisão final (`ReviewPanel`/`Findings`) | existe (a lista e o painel de leitura já seguiam o Stitch) |
| Texto de evidência e sugestão                                                           | texto do agente revisor                     | adaptado                                                   |

## S6 — Revisão final e publicação

| Stitch                                                                                                                    | App              |                                                                                                                            |
| ------------------------------------------------------------------------------------------------------------------------- | ---------------- | -------------------------------------------------------------------------------------------------------------------------- |
| "O que falta para esta spec ficar pronta" + selo "Pronta" + Rodar validações + primário "Aprovar a especificação" no topo | botões no pé     | adaptado                                                                                                                   |
| Sete verificações em linhas com ✓                                                                                         | sete cartões     | adaptado                                                                                                                   |
| "Abrir" em cada verificação                                                                                               | só nos bloqueios | não implementado: seria um atalho novo para cada verificação satisfeita                                                    |
| Seções recolhíveis: Revisão pelo agente revisor, Publicar no projeto, Implementar, Arquivar com o CLI                     | painéis abertos  | adaptado: recolhidas, com o estado em selo; abrem sozinhas se há achado em aberto, revisão rodando ou publicação com falha |
| Chevron duplicado, ponto solto na etapa, "Pronta" repetido                                                                | —                | defeitos do Stitch, não repetidos                                                                                          |

## S7 — Nova especificação

| Stitch                                                                            | App                            |                                                    |
| --------------------------------------------------------------------------------- | ------------------------------ | -------------------------------------------------- |
| Modal em duas colunas (pedido / "Ajude o agente a não adivinhar" + agentes)       | uma coluna longa               | adaptado (modal largo; o corpo rola se não couber) |
| "Obrigatório", cadeado e ponto verde no Projeto, placeholders e ícones inventados | textos do app                  | não existe                                         |
| Selo "Sempre perguntado"                                                          | perguntas conforme a descrição | adaptado (mesma lógica de `specs.ctxq.*`)          |

## S8 — Importar do projeto

| Stitch                                                                                                  | App    |            |
| ------------------------------------------------------------------------------------------------------- | ------ | ---------- |
| Texto, projeto, agente que escreve, pastas com caixa, problemas, rodapé                                 | existe | existe     |
| "O QUE FOI ENCONTRADO", "3 itens selecionados", selo "Completo", pontos verdes, seleção inicial de tudo | —      | não existe |
