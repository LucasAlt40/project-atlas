# Prompt geral para o chat de implementação — todos os módulos desenhados

Colar num chat novo (ou no chat que implementou o módulo 1), aberto na raiz do repositório e com o MCP do
Stitch conectado, se possível. Cobre tudo o que já foi desenhado no Stitch **menos o Workflow**, que ainda
não foi enviado ao Stitch e fica para depois.

```text
Você vai implementar, no app Atlas (Tauri + React + TypeScript), a repaginação visual de TODOS os módulos que
já foram desenhados no Stitch, exceto o Workflow. É uma REPAGINAÇÃO: a identidade, os dados e a
funcionalidade do app continuam; muda a forma como a informação é separada, agrupada e lida.

PRIMEIRO, A BRANCH (faça nesta ordem, antes de qualquer outra coisa)
1. Volte para a sua branch: `git checkout design/workspace-harness`. É a branch onde o módulo 1 (Workspace +
   Harness) foi implementado e onde moram os componentes compartilhados (Badge, CollapsibleSection,
   OverflowMenu, Steps).
2. Traga as atualizações da main: `git fetch` e `git merge main`. A main recebeu, depois da sua última
   mudança, só documentação (a pasta docs/design-review inteira: dossiês, prompts, planos e os ids de todas
   as telas do Stitch). Resolva qualquer conflito mantendo o código da sua branch e o texto novo da main em
   docs/. Rode `npm ci` e `npm run check` e registre a linha de base.
3. A árvore de trabalho da main tem alterações NÃO commitadas de outra frente (arquivos de personalidades,
   Specs e i18n). Elas não fazem parte deste trabalho: não as commite, não as descarte e não as use como base.
   Se `git merge` reclamar delas, pare e me avise.
4. Continue nesta mesma branch, com commits pequenos e separados por módulo (prefixo claro, em português, no
   estilo do histórico). NÃO crie outras branches, NÃO faça push, NÃO abra PR e NÃO mexa na main.

O CONTEXTO ESTÁ NA PASTA docs/design-review/ (estou enviando o projeto inteiro para você consultar)
Leia, nesta ordem:
1. docs/design-review/_comum/README.md (processo, tabela de módulos e lições)
2. docs/design-review/_comum/principios-implementacao.md (as regras, a base compartilhada, as fases e o
   formato do relatório). Siga à risca.
3. docs/design-review/_comum/p0-regras.md e p0-mobile.md (o que foi pedido ao Stitch; ajuda a entender o que
   ele inventou e por quê)
4. Para cada módulo: README.md (dossiê: telas atuais, estados, problemas), plano-implementacao.md (telas, ids
   do Stitch, mapa de código, decisões), prompts.md (o que foi pedido ao Stitch para cada tela, o registro
   com os ids e, ao final, a AVALIAÇÃO tela a tela) e a pasta atual/ (imagens de como o app é hoje).
   O prompts.md de cada módulo traz, por tela, as listas "O Stitch mostra, o app não tem" (NÃO implementar)
   e os desvios: leia antes de olhar o Stitch.
5. O relatório do módulo 1 (docs/design-review/workspace-harness/implementacao/relatorio.md), para repetir o
   método e reaproveitar a base compartilhada.

MÓDULOS A IMPLEMENTAR, NESTA ORDEM (cada um com seu prompt-implementacao.md como detalhe; este prompt manda
quando houver conflito)
2. Agentes e Personalidades: docs/design-review/agentes-personalidades/
3. MCP Hub: docs/design-review/mcp-hub/ (5 telas)
4. Configurações: docs/design-review/configuracoes/ (5 telas; o índice lateral interno é novo, de 200px, sem
   título, e vira abas abaixo de 900px)
5. Specs: docs/design-review/specs/ (8 telas; o cabeçalho e o stepper de S3 a S6 são o mesmo componente)
6. Celular: docs/design-review/celular/ (5 telas da página do celular + 1 diálogo do desktop). A página do
   celular abre no navegador do telefone e NÃO tem a moldura do desktop; confira em 390 px.
NÃO implemente o Workflow (docs/design-review/workflow/): os prompts dele ainda não foram enviados ao Stitch.

ONDE ESTÁ O VISUAL (HTML e PNG do Stitch)
- Os ids das telas estão em cada plano-implementacao.md. Projeto Stitch 526454930665111857, design system
  assets/7aa0774b734b4149b2d38a23136aadfd ("Atlas Engineering Core").
- Se as pastas docs/design-review/<módulo>/stitch/ já tiverem os HTML e PNG, use-as. Se não tiverem, baixe
  com `mcp__stitch__get_screen` em projects/526454930665111857/screens/<id>: o HTML em htmlCode.downloadUrl e a
  imagem em screenshot.downloadUrl. As imagens em lh3.googleusercontent.com baixam sem login e, com `=w1800`
  no fim da URL, saem em alta resolução; as URLs de api.stitch dão 401. Salve em
  docs/design-review/<módulo>/stitch/ com nomes numerados, como em workspace-harness/stitch/.
- Atenção a duas armadilhas do Stitch: (a) a imagem de "Spec — Perguntas e achados" termina no meio do achado
  (altura fixa); o HTML tem a tela inteira. (b) As telas M1, M2, M4, M5a e M5b do Celular vieram num canvas
  de desktop de 2560 px com a página do celular como uma coluna estreita no meio: vale a coluna de ~390 px.
- Nunca implemente de memória: compare cada tela com o Stitch lado a lado.

REGRAS QUE NÃO PODEM SER QUEBRADAS (detalhe em principios-implementacao.md)
- O Stitch é a referência VISUAL, não a especificação funcional. O que ele mostra e o app não faz (campo,
  botão, rótulo, métrica, selo, atalho, texto de ajuda, ícone decorativo) NÃO se implementa: está listado por
  tela em prompts.md. O que o Stitch omitiu e o app mostra, MANTENHA. Quando o Stitch trouxe dados de exemplo
  (nomes, textos, números), use os dados reais do app.
- Moldura: ignore a moldura que o Stitch desenhou (barra lateral, barra superior, logo, rodapé); vale a do app.
  Em algumas telas ele inventou "Core", "Novo Fluxo", ponto de estado e versão errada.
- Máxima fidelidade ao que o Stitch mostra e o app suporta. Funcionalidade intacta: mesmos comandos, estados,
  regras e testes de comportamento; não toque em Rust, DTOs nem nomes de comandos.
- Desktop responsivo de 720 a 2560 px (a janela abre em 1100×720, mínimo 720), sem rolagem horizontal da
  página e sem texto cortado sem reticências. Verifique com o chk.js em 720, 1100, 1280, 1440, 1920 e 2560.
  A página do celular: 390 px (e confira 360 e 430).
- Repaginação, não redesenho: tokens de global.css, Geist e JetBrains Mono, borda de 1px sem sombra (só
  overlay), Button com variant. Reaproveite Badge, CollapsibleSection, OverflowMenu e Steps antes de criar
  componente novo; um único mapa de estado → Badge por módulo.
- Textos em pt-BR.ts E en-US.ts; nada fixo no JSX.
- Se algo em um plano conflitar com o código real, pare e me pergunte antes de decidir.

COMO VERIFICAR
Copie docs/design-review/_comum/captura/* para tmp/design-capture/ (a pasta é ignorada pelo git), rode
`npx vite --port 1420` e abra http://localhost:1420/tmp/design-capture/index.html (parâmetros no main.tsx:
harness, agents, runs, workspaces, custom, runtimes, mcp, memory, specs, remote, wf). A página do celular
tem mobile.html com mocks de fetch e EventSource; use o preset de navegador 390×844. Estenda o mock em tmp/
para os estados que faltarem; não mexa no código do app para isso. Compare cada tela com o Stitch.

ENTREGA
- Por módulo: as fases do plano com commits pequenos, `npm run check` verde, capturas antes e depois e o
  relatório em docs/design-review/<módulo>/implementacao/relatorio.md (formato em principios-implementacao.md
  seção 5, com as listas "O Stitch mostra, o app não tem", "Stitch omitiu, app manteve" e o que depende de
  decisão minha).
- Ao fim de cada módulo, pare e me diga em duas linhas o que ficou pronto e o que precisa da minha decisão;
  só siga para o próximo módulo depois que eu disser (ou se eu tiver dito para seguir direto).
- Ao terminar tudo, diga os comandos exatos para eu ver a branch e resuma as decisões pendentes.
```
