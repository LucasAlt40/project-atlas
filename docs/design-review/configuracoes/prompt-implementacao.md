# Prompt para o chat de implementação — Configurações

Colar num chat novo, aberto na raiz do repositório, com o MCP do Stitch conectado. Só usar depois
que os prompts de `prompts.md` foram enviados e os ids das telas estão preenchidos no plano.

```text
Você vai implementar, numa branch separada, a repaginação visual da tela Configurações do Atlas (app
desktop Tauri + React + TypeScript). O usuário avalia o resultado antes do módulo seguinte, então a
fidelidade importa.

LEIA PRIMEIRO, nesta ordem:
1. docs/design-review/_comum/principios-implementacao.md (regras, base compartilhada, fases,
   relatório). Siga-o à risca.
2. docs/design-review/configuracoes/plano-implementacao.md (telas, ids do Stitch, mapa de código,
   decisões e cuidados deste módulo).
3. docs/design-review/configuracoes/README.md (a tela atual, interações e 10 problemas de design) e as
   imagens em atual/.
4. docs/design-review/configuracoes/prompts.md (o que foi pedido ao Stitch para cada tela).
5. docs/design-review/workspace-harness/implementacao/relatorio.md (o que deu certo e o que o Stitch
   inventou no primeiro módulo; repita o método).

BRANCH: `design/configuracoes`, a partir de `design/workspace-harness` (ou de `main` se essa já foi
integrada). Rode `npm ci` e `npm run check` e registre a linha de base.

REGRAS QUE NÃO PODEM SER QUEBRADAS (detalhe em principios-implementacao.md)
- O Stitch é a referência VISUAL, não a especificação funcional. Se mostrar algo que o app não faz
  (campo, botão, métrica, filtro, atalho, indicador de "salvo" sem estado correspondente, texto de
  estado inexistente), NÃO implemente e liste em "O Stitch mostra, o app não tem". Se omitiu um dado
  que o app mostra, MANTENHA e liste em "Stitch omitiu, app manteve". Nenhum texto de ajuda é removido:
  o longo vai para uma seção recolhida.
- As cinco telas do Stitch são seções da MESMA página: o app continua com uma tela Configurações, com
  índice interno (âncoras), sem novo item de navegação nem roteamento.
- Máxima fidelidade ao que o Stitch mostra e o app suporta. Funcionalidade intacta (mesmos comandos,
  estados, regras e testes de comportamento). Não unifique o fluxo de importar e exportar; só a
  aparência e o agrupamento mudam.
- Responsivo de 720 a 2560 px (janela real: abre em 1100×720, mínimo 720), sem rolagem horizontal da
  página nem texto cortado sem reticências; verifique com chk.js em 720, 1100, 1280, 1440, 1920 e
  2560. Abaixo de ~900 px o índice vira uma faixa de âncoras no topo.
- Repaginação, não redesenho: tokens de global.css, Geist e JetBrains Mono, borda de 1px sem sombra
  (só overlay), Button com variant. Reutilize Badge, CollapsibleSection, OverflowMenu e Steps antes de
  criar componente novo.
- Textos em pt-BR.ts E en-US.ts; nada fixo no JSX.
- Escopo: só este módulo e os compartilhados de que ele depende. PortabilityActions/ImportButton e o
  diálogo de exportar também são usados em Agentes, Personalidades e Workflow: confira todos. Não
  toque em Rust, DTOs, comandos, nem nos módulos Workflow, Specs, MCP Hub, Agentes ou no app móvel.

COMO OBTER O VISUAL: get_screen de cada id do plano, baixe HTML e imagem para
docs/design-review/configuracoes/stitch/ (numerados, com DESIGN.md), como foi feito em
workspace-harness/stitch/. Se o download der 401, renderize o HTML; se não der, peça as imagens ao
usuário. Nunca implemente de memória.

COMO VERIFICAR: copie docs/design-review/_comum/captura/* para tmp/design-capture/, rode
`npx vite --port 1420` e abra http://localhost:1420/tmp/design-capture/index.html (?memory=none|some).
Compare cada tela com o Stitch lado a lado. Estenda o mock em tmp/ para os estados que faltam
(sandbox indisponível, atualização disponível, revisão da importação); não mexa no código do app para
isso.

ENTREGA: as fases do plano (§4) com commits pequenos em português no estilo do histórico, `npm run
check` verde, e o relatório em docs/design-review/configuracoes/implementacao/relatorio.md (formato
em principios-implementacao.md §5). NÃO faça push, NÃO abra PR, NÃO mexa na main. Ao terminar, diga os
comandos exatos para o usuário ver a branch e resuma o que precisa da decisão dele.

Se algo no plano conflitar com o código, pare e pergunte antes de decidir.
```
