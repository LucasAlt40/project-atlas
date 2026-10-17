# Plano de implementação — Repaginação do Workflow

Branch `design/workflow`, criada a partir de `design/workspace-harness` (ou de `main`, se essa já tiver
sido integrada; a base compartilhada `Badge`, `CollapsibleSection`, `OverflowMenu`, `Steps` precisa
existir). **Faça este módulo por último** e, se os módulos anteriores já tiverem sido integrados, a partir
da `main` atualizada: o Workflow usa componentes de Workspace (`ExecutionInspector`, `ScopePicker`), de
Specs (`StartChoice`, `SpecRunPanel`), de Portabilidade (importar/exportar) e do MCP Hub.
Princípios, fases, fonte do visual, relatório e regras de divergência:
[`../_comum/principios-implementacao.md`](../_comum/principios-implementacao.md). Dossiê:
[`README.md`](README.md).

É o módulo **maior e mais arriscado** (≈ 7,4 mil linhas de componentes só no módulo, mais um canvas React
Flow). Trabalhe por **modo** (editor, execução, revisão), com commits pequenos e o harness a cada passo.

## 1. Telas e código

Ids a preencher depois do envio dos prompts (registro em `prompts.md`).

| #   | Tela (nome no Stitch)                           | Id da tela nova | Código do app (hipótese a confirmar)                                                                                                                                                                                                                                                             |
| --- | ----------------------------------------------- | --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| W1  | Workflow — Editor                               | _a preencher_   | `workflow/pages/WorkflowPage.tsx` (cabeçalho e faixas), `WorkflowToolbar.tsx` (`CanvasTools`, `NodeTools`), `WorkflowCanvas.tsx`, `WorkflowNodes.tsx`, `WorkflowOverview.tsx` (`EditorOverview`), `AutoPlansToggle.tsx`, `StartChoice` (Specs), `ScopePicker` (Workspace), `Workflow.module.css` |
| W2  | Workflow — Configurar etapa                     | _a preencher_   | `WorkflowInspector.tsx` (`NodeInspector`, `EdgeInspector`, `LinkInspector`), `StepTools.tsx`, `WorkflowTools.tsx`                                                                                                                                                                                |
| W3  | Workflow — Novo workflow                        | _a preencher_   | `NewWorkflowPanel.tsx`, `TemplatePreview.tsx`                                                                                                                                                                                                                                                    |
| W4  | Workflow — Execução em andamento                | _a preencher_   | `WorkflowOverview.tsx` (`RunOverview`), `LiveWorkspacePanel.tsx`, `LiveFiles.tsx`, `LiveCode.tsx`, `SharedStatePanel.tsx`, `PlansPanel.tsx`                                                                                                                                                      |
| W5  | Workflow — Aguardando sua resposta              | _a preencher_   | `InteractionPanel.tsx`, `ActionRequired.tsx` (chip da barra superior: moldura, não muda)                                                                                                                                                                                                         |
| W6  | Workflow — Handoffs e alterações                | _a preencher_   | `HandoffView.tsx`, `ChangesModal.tsx`, a aba Alterações de `RunOverview`                                                                                                                                                                                                                         |
| W7  | Workflow — Revisão e entrega                    | _a preencher_   | `ReviewWorkspace.tsx`, `DeliveryPanel.tsx`, `workflow/model/review.ts`                                                                                                                                                                                                                           |
| W8  | Workflow — Retomar execução                     | _a preencher_   | `RecoveryPanel.tsx`                                                                                                                                                                                                                                                                              |
| W9  | Workflow — Salvar como template e reparar rotas | _a preencher_   | `SaveTemplate.tsx`, `RouteRepairDialog.tsx`                                                                                                                                                                                                                                                      |

Textos em `workflow.*`, `review.*`, `live.*` de `pt-BR.ts` e `en-US.ts` (≈ 400 chaves). A página recebe
intenções de navegação (`open-workflow`, `workflow-from-spec`) por `NavigationContext`.

## 2. Decisões já tomadas para este módulo

- **Um cabeçalho de uma faixa** em vez de quatro, igual nos três modos: seletor de workflow e `+ Novo
workflow`; selo de modo e de estado; ação principal do modo; `OverflowMenu` com `Personalizar workflow`,
  `Salvar como template`, `Exportar`, `Importar` e `Excluir workflow` (`danger`). **Todas as ações continuam**,
  com as mesmas condições de habilitação.
- **Paleta flutuante** sobre o canvas com as ferramentas de edição (`+ Agente`, `+ Condição`, `+ Fim`, `Conectar`,
  `Remover`, `Desfazer`, `Refazer`, `Redefinir layout`, `Ajustar`, zoom). Em execução ela só tem `Ajustar` e o zoom
  (as de edição já estão desabilitadas hoje: aqui somem). A lógica de `model/edit.ts` não muda.
- **Barra de tarefa**: o campo e `Executar` à vista; "como começar" (vibe/spec), `Aprovar planos
automaticamente` e o escopo vão para "Opções da execução" (`CollapsibleSection` ou menu). **Nada é
  removido**, e o valor atual de cada opção aparece como resumo ao lado do botão quando não é o padrão.
- **Canvas protagonista** (≥ 60% da altura) em todos os modos. Um painel lateral à direita muda de conteúdo
  por modo: visão geral do workflow (editor), configuração da etapa (etapa selecionada), execução com abas
  `Visão geral | Handoffs | Alterações` (execução), resposta à pergunta (aguardando você). **O painel não
  cobre o canvas.**
- **"Aguardando sua resposta"** vira painel lateral (hoje é uma faixa sobre o canvas): os mesmos campos,
  tipos de pergunta (plano, pergunta, aprovação, permissão) e a regra "sua resposta nunca aplica mudanças".
  `InteractionPanel` mantém a lógica de envio.
- **Revisão**: um controle segmentado `Canvas | Revisão` alterna a área principal; a revisão deixa de
  ficar empilhada abaixo do canvas. Quando a execução termina, o app abre na revisão (como hoje a revisão
  aparece), com o canvas a um toque. **A revisão mantém todas as seções** (Linha do tempo, Findings,
  Handoffs, Decisões, Artefatos, Conjunto de alterações), com `CollapsibleSection`s e contagem; as vazias
  fecham.
- **Cartão de entrega**: o que entra no projeto é o centro da revisão. Ordem visual: `Revisar diff`, `Aplicar
alterações`, `Abrir na IDE`, e `Manter isolado` / `Descartar` no `OverflowMenu` (`Descartar` com `danger`).
  **As condições, as confirmações e os textos de segurança (`review.apply.*`) não mudam**; o texto "Concluir o
  workflow não significa…" fica logo abaixo dos botões.
- **Estado em um só vocabulário**: um `Badge` por estado de execução (mapa único para `workflow.run.*`,
  `workflow.status.*`, `review.run.*`), usado no cabeçalho, na visão geral, nos nós e na revisão.
- **Handoffs**: cartões com emissor e receptor em destaque; as seções (decisões, artefatos, arquivos,
  validação, falha) recolhem quando vazias.
- **Novo workflow**: duas colunas e passos numerados; o botão `Criar workflow` fixo no rodapé; templates e
  detalhe lado a lado.
- **Inspetor da etapa**: seções recolhíveis, a primeira aberta, rodapé fixo com o estado salvo e `Salvar
configuração`.

## 3. Cuidados específicos

- **O núcleo decide como executa**: a página só mostra e edita; a repaginação não pode iniciar, retomar,
  cancelar, aplicar nem descartar nada fora dos fluxos e confirmações atuais.
- **React Flow**: o canvas mede o container (o setup de teste simula `ResizeObserver` e `DOMMatrixReadOnly`).
  Ao mudar layout (painel lateral, paleta), confira `fitView`, o minimapa, o zoom e o arrasto de nós e
  arestas; em janela de 720 px o painel vira gaveta sobre o canvas (só nesse caso cobre).
- **Muito texto e muitos estados**: o módulo tem dezenas de estados (`WorkflowExecutionStatusDto`, estados de
  nó, de tentativa, de entrega, de revisão). Cada um precisa de tom no `Badge`; `failed`, `blocked`,
  `interrupted` e `cancelled` não podem ficar parecidos.
- **Testes** (`WorkflowPage.test.tsx`, `WorkflowCode`, `WorkflowInteraction`, `WorkflowSpec`,
  `ReviewWorkspace`, `WorkflowInspector`, `StepTools`, `WorkflowTools`, `SaveTemplate`, `NewWorkflowPanel`,
  `LiveWorkspacePanel`, `useAttentionNotifications`): cobrem fluxos inteiros por papel e nome acessível.
  Mover um botão para o menu muda o caminho do teste: ajuste o caminho, nunca a asserção, e justifique.
- **Intenções de navegação**: `open-workflow` (vindo de uma execução ou do chip "aguardando você") e
  `workflow-from-spec` (vindo de Specs) continuam funcionando e abrem o modo certo.
- **Portabilidade**: `ImportButton` e `ExportButton` são compartilhados (Configurações, Agentes): conferir.
- **Dados extremos**: workflow de 30 nós, nomes de 60 caracteres, 200 arquivos alterados, 50 findings,
  execução de 12 etapas em paralelo, diff grande.
- Estender o mock (`tmp/`) para recuperação, reparo de rotas, workspace ao vivo, entrega em todos os estados
  e findings reais (ver `workflowFixtures.ts`: `handoff`, `changeSet`, `withCode`, `liveState`, `liveUpdate`).

## 4. Fases

| Fase | Entrega                                                                                                                                                                            |
| ---- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 0    | Preparação (ver `_comum`), capturas "antes" com `?wf=idle                                                                                                                          | running | waiting | done | failed` |
| 1    | `implementacao/levantamento.md` + exportação do Stitch para `stitch/`                                                                                                              |
| 2    | Base compartilhada: cabeçalho de uma faixa, paleta flutuante, painel lateral por modo, mapa único de `Badge` de estado                                                             |
| 3    | W1 (editor: cabeçalho, barra de tarefa, canvas, paleta, visão geral)                                                                                                               |
| 4    | W2 (configurar etapa)                                                                                                                                                              |
| 5    | W4 (execução em andamento) e W8 (retomar)                                                                                                                                          |
| 6    | W5 (aguardando sua resposta)                                                                                                                                                       |
| 7    | W7 e W6 (revisão, entrega, handoffs, alterações)                                                                                                                                   |
| 8    | W3 (novo workflow) e W9 (diálogos)                                                                                                                                                 |
| 9    | Responsividade 720–2560 (painel como gaveta abaixo de ~1000 px) e estados (workflow vazio, 30 nós, execução pausada / interrompida / cancelada, entrega em todos os estados, erro) |
| 10   | Testes, `npm run check`, relatório                                                                                                                                                 |

## 5. Captura

`_comum/captura/` com `?wf=none|idle|running|waiting|done|failed`. Para as execuções, abrir Workflow e
escolher a execução no seletor. As fixtures de `src/test/workflowFixtures.ts` já dão handoffs e conjunto de
alterações (modo `done`).

## 6. Fora do escopo

Motor de workflows, templates, recuperação, entrega e revisão no núcleo (Rust); DTOs e comandos; a barra
superior (chip "aguardando você"); os outros módulos (só garantir que os compartilhados continuam corretos).
