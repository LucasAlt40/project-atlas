# Prompt para o chat de implementação — Workflow

Colar num chat novo, aberto na raiz do repositório, com o MCP do Stitch conectado. Só usar depois
que os prompts de `prompts.md` foram enviados e os ids das telas estão preenchidos no plano. É o último
módulo: de preferência a partir da `main` com os módulos anteriores já integrados.

```text
Você vai implementar, numa branch separada, a repaginação visual do módulo Workflow do Atlas (app desktop
Tauri + React + TypeScript + React Flow). É o módulo maior e mais arriscado; o usuário avalia o resultado,
então a fidelidade importa e o trabalho deve ser feito por modo (editor, execução, revisão), com commits
pequenos e verificação visual a cada passo.

LEIA PRIMEIRO, nesta ordem:
1. docs/design-review/_comum/principios-implementacao.md (regras, base compartilhada, fases,
   relatório). Siga-o à risca.
2. docs/design-review/workflow/plano-implementacao.md (telas, ids do Stitch, mapa de código, decisões e
   cuidados deste módulo).
3. docs/design-review/workflow/README.md (as telas atuais, estados, interações e 11 problemas de design)
   e as imagens em atual/.
4. docs/design-review/workflow/prompts.md (o que foi pedido ao Stitch para cada tela).
5. Os relatórios dos módulos anteriores (docs/design-review/*/implementacao/relatorio.md) para repetir o
   método e reaproveitar a base compartilhada.

BRANCH: `design/workflow`, a partir da `main` atualizada (ou de `design/workspace-harness` se os módulos
anteriores ainda não foram integrados). Rode `npm ci` e `npm run check` e registre a linha de base.

REGRAS QUE NÃO PODEM SER QUEBRADAS (detalhe em principios-implementacao.md)
- O Stitch é a referência VISUAL, não a especificação funcional. Se mostrar algo que o app não faz
  (campo, botão, métrica, filtro, atalho, aba, texto de estado inexistente), NÃO implemente e liste em "O
  Stitch mostra, o app não tem". Se omitiu um dado que o app mostra, MANTENHA e liste em "Stitch omitiu, app
  manteve".
- Máxima fidelidade ao que o Stitch mostra e o app suporta. Funcionalidade intacta (mesmos comandos,
  estados, regras e testes de comportamento). O núcleo decide como o workflow executa: a página só mostra e
  edita. A repaginação não pode iniciar, pausar, retomar, cancelar, responder, aplicar ou descartar nada
  fora dos fluxos e confirmações atuais. Os textos de segurança da entrega (o código não está no projeto
  até você aplicar; aplicar nunca faz commit nem push; sua resposta nunca aplica mudanças) continuam.
- Todas as ações do cabeçalho (inclusive as que vão para o menu "⋯") continuam, com as mesmas condições de
  habilitação. As opções da barra de tarefa (como começar, aprovar planos automaticamente, escopo) mudam de
  lugar e continuam funcionando.
- O painel lateral NÃO cobre o canvas (exceto como gaveta abaixo de ~1000 px). Confira fitView, minimapa,
  zoom e arrasto de nós e arestas do React Flow depois de cada mudança de layout.
- Responsivo de 720 a 2560 px (janela real: abre em 1100×720, mínimo 720), sem rolagem horizontal da
  página nem texto cortado sem reticências; verifique com chk.js em 720, 1100, 1280, 1440, 1920 e 2560.
- Repaginação, não redesenho: tokens de global.css, Geist e JetBrains Mono, borda de 1px sem sombra (só
  overlay), Button com variant. Reutilize Badge, CollapsibleSection, OverflowMenu e Steps antes de criar
  componente novo. Um único mapa de estado → Badge para todo o módulo.
- Textos em pt-BR.ts E en-US.ts; nada fixo no JSX.
- Escopo: só este módulo e os compartilhados de que ele depende. ExecutionInspector, ScopePicker (Workspace),
  StartChoice e SpecRunPanel (Specs), ImportButton/ExportButton (Portabilidade) são compartilhados: confira
  as telas deles. Não toque em Rust, DTOs, comandos, nem na barra superior (chip "aguardando você").

COMO OBTER O VISUAL: get_screen de cada id do plano, baixe HTML e imagem para
docs/design-review/workflow/stitch/ (numerados, com DESIGN.md), como foi feito em
workspace-harness/stitch/. Se o download der 401, renderize o HTML; se não der, peça as imagens ao usuário.
Nunca implemente de memória. As telas W1 a W8 compartilham o mesmo cabeçalho: implemente-o uma vez.

COMO VERIFICAR: copie docs/design-review/_comum/captura/* para tmp/design-capture/, rode
`npx vite --port 1420` e abra http://localhost:1420/tmp/design-capture/index.html
(?wf=none|idle|running|waiting|done|failed; abra Workflow e escolha a execução no seletor). Estenda o mock
em tmp/ para recuperação, reparo de rotas, workspace ao vivo e todos os estados de entrega (use
src/test/workflowFixtures.ts); não mexa no código do app para isso. Compare cada tela com o Stitch lado a
lado.

ENTREGA: as fases do plano (§4) com commits pequenos em português no estilo do histórico, `npm run check`
verde, e o relatório em docs/design-review/workflow/implementacao/relatorio.md (formato em
principios-implementacao.md §5). NÃO faça push, NÃO abra PR, NÃO mexa na main. Ao terminar, diga os
comandos exatos para o usuário ver a branch e resuma o que precisa da decisão dele.

Se algo no plano conflitar com o código, pare e pergunte antes de decidir.
```
