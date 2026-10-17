# Plano de implementação — Repaginação de Specs

Branch `design/specs`, criada a partir de `design/workspace-harness` (ou de `main`, se essa já tiver
sido integrada; a base compartilhada `Badge`, `CollapsibleSection`, `OverflowMenu`, `Steps` precisa
existir). Princípios, fases, fonte do visual, relatório e regras de divergência:
[`../_comum/principios-implementacao.md`](../_comum/principios-implementacao.md). Dossiê:
[`README.md`](README.md).

Este é o módulo mais **denso depois do Workflow**: o detalhe de uma spec tem muitos painéis. Trabalhe por
estado da spec e confira cada um com o harness antes de passar ao próximo.

## 1. Telas e código

Ids das telas geradas no Stitch (registro e avaliação em `prompts.md`).

| #   | Tela (nome no Stitch)             | Id da tela nova                    | Código do app (hipótese a confirmar)                                                                                             |
| --- | --------------------------------- | ---------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| S1  | Specs — Lista de especificações   | `a7eb1bf978364f498e6908e8e85b6ed4` | `specs/pages/SpecsPage.tsx`, `components/Overview.tsx`, `SpecCard.tsx`, `StatusBadge.tsx`, `model/status.ts`, `Specs.module.css` |
| S2  | Specs — Sem especificações        | `0de3cab5da26442f9bf7cf0ca0d08ba0` | `Overview.tsx` (ramo vazio)                                                                                                      |
| S3  | Spec — Documento em revisão       | `d439e1fbb75c4b7f9d684b0b3c46a543` | `SpecDetail.tsx`, `Stepper.tsx`, `Dock.tsx`, `ArtifactDocument.tsx`, `Inspector.tsx`, `UsageBar.tsx`, `RequestChangesDialog.tsx` |
| S4  | Spec — Agente escrevendo          | `4981422ada754146b0cb711a0df3e897` | `RunBanner.tsx`, `SpecDetail.tsx`                                                                                                |
| S5  | Spec — Perguntas e achados        | `6eadf408b4f543b6b75a02cab206660e` | `OpenQuestions.tsx`, `QuestionCard.tsx`, `AnswerQuestionsDialog.tsx`, `SendAnswersDialog.tsx`, `Findings.tsx`                    |
| S6  | Spec — Revisão final e publicação | `fb6be8f6a81149e3873c33d6d98f31f0` | `FinalReview.tsx`, `ReviewPanel.tsx`, `PublishPanel.tsx`, `CliPanel.tsx`, `SpecRunPanel.tsx`                                     |
| S7  | Specs — Nova especificação        | `3e2672ca7f07412bbe13d373685356fb` | `NewSpecDialog.tsx`, `IntegrationsPicker.tsx`, `AgentsDialog.tsx`                                                                |
| S8  | Specs — Importar do projeto       | `00d70d7a6e5e4f649ceff03b2273646d` | `components/ImportDialog.tsx` (o de Specs, não o de portabilidade)                                                               |

Entradas e ligações: `Home` em `SpecsPage.tsx` recebe `open-spec` e `new-spec` por intenção de
navegação (`NavigationContext`); `onImplement` navega para o Workflow com `workflow-from-spec`.
Textos em `specs.*` de `pt-BR.ts` e `en-US.ts`.

## 2. Decisões já tomadas para este módulo

- **Contadores e filtros**: os contadores soltos (`specs.counter.*`) deixam de ser uma faixa própria; as
  abas de grupo (`specs.group.*`) mostram os mesmos números. Nenhum número se perde: se um contador não tiver
  aba equivalente, manter como texto da aba. A trilha mono `specs.crumb` pode virar um texto discreto ou sair
  da tela **se o Stitch a omitir**; registrar em "Stitch omitiu, app manteve" se ficar.
- **Cartão de spec**: o pedido continua (uma linha); estado, avanço, agente e alertas continuam. Usar `Badge`
  para o estado; a barra lateral colorida vem do mesmo mapa de estados (`model/status.ts`).
- **Cabeçalho e stepper do detalhe iguais em todas as fases** (S3 a S6): um único componente.
- **"Sua decisão" (`Dock`) sobe para logo abaixo do stepper** como cartão de destaque, com as mesmas
  ações e as mesmas condições de habilitação; a lógica do `Dock` não muda. Em janela estreita ele fica
  fixo no rodapé, como hoje, para não sumir da vista.
- **Painéis laterais** (Validações automáticas, Histórico, Consumo) como `CollapsibleSection`s numa coluna
  lateral; abaixo de ~1000 px viram seções empilhadas abaixo do documento.
- **Revisão final**: as sete verificações viram linhas com `✓`/estado e `Abrir`; o resultado "Pronta" e o
  botão `Aprovar a especificação` ficam no topo do bloco. Revisão pelo agente revisor, Publicar, Implementar
  e Arquivar com o CLI viram seções recolhíveis com `Badge` de estado no cabeçalho. **Todas as regras de
  habilitação continuam** (aprovar só com tudo pronto; publicar só pronta; dispensar a revisão exige motivo).
- **Ações irreversíveis** (`Publicar`, `Arquivar`, `Excluir`): nunca o botão primário da página; a
  confirmação atual de cada uma continua. `Excluir` no `OverflowMenu` com `danger`.
- **Nova especificação**: o modal passa a ter duas colunas (campos / "Ajude o agente a não adivinhar") e
  não rola por dentro em 1100×720 se couber; se não couber, rola o corpo, nunca o cabeçalho e o rodapé.
  As perguntas de contexto continuam aparecendo conforme a descrição (mesma lógica de `specs.ctxq.*`).
- **Importar**: mesma lógica; só layout.

## 3. Cuidados específicos

- **Nunca** disparar um agente ao abrir, repaginar ou mover o `Dock`: "Nada roda até você pedir" é regra do
  produto.
- `SpecsPage.test.tsx` (e os testes de componentes que existirem) cobrem fluxos; manter nomes acessíveis.
  Mover a decisão de lugar não pode mudar a ordem de foco de forma que prejudique o teclado.
- O texto dos documentos (markdown) é do usuário: continua renderizado como hoje (`Markdown`).
- Os estados de `SpecStatusDto` (13) e de fase de artefato precisam do mesmo sistema de `Badge`/tons;
  `failed`, `blocked`, `outdated` e `changes_requested` não podem ficar parecidos.
- Estender o mock para capturar `questions`, `findings`, `aiReview`, publicação, limite de tokens atingido
  e `runs` (ver README §0 e `specFixtures.ts`).
- Cuidado com a página com 30+ specs e títulos/pedidos longos; com um documento de 2000 linhas.

## 4. Fases

| Fase | Entrega                                                                                                                   |
| ---- | ------------------------------------------------------------------------------------------------------------------------- |
| 0    | Preparação (ver `_comum`), capturas "antes" com `?specs=some` e `?specs=none` (e os estados que exigirem estender o mock) |
| 1    | `implementacao/levantamento.md` + exportação do Stitch para `stitch/`                                                     |
| 2    | Base compartilhada: cabeçalho de spec, stepper, cartão de decisão, coluna lateral recolhível                              |
| 3    | S1 e S2 (lista e vazio)                                                                                                   |
| 4    | S3 e S4 (documento em revisão; agente escrevendo)                                                                         |
| 5    | S5 (perguntas e achados)                                                                                                  |
| 6    | S6 (revisão final e publicação; CLI; tarefas)                                                                             |
| 7    | S7 e S8 (nova especificação; importar)                                                                                    |
| 8    | Responsividade 720–2560 e estados (vazio, 30 specs, limite de tokens, falha, desatualizado, bloqueado)                    |
| 9    | Testes, `npm run check`, relatório                                                                                        |

## 5. Captura

`_comum/captura/` com `?specs=none|some`. As specs de exemplo vêm de `src/test/specFixtures.ts`.

## 6. Fora do escopo

Lógica de geração, validação, revisão, publicação e do CLI OpenSpec; Rust, DTOs, comandos; o Workflow
(só garantir que `onImplement` continua levando até ele); os outros módulos.
