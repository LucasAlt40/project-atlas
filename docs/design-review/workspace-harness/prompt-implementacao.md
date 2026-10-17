# Prompt para o chat de implementação

Cole o bloco abaixo num chat novo, aberto na raiz do repositório (`PROJECT-ATLAS`), com o MCP do
Stitch conectado.

```text
Você vai implementar, numa branch separada, a repaginação visual do módulo Workspace + Harness do
Atlas (app desktop Tauri + React + TypeScript). O resultado será avaliado pelo usuário antes de
repetirmos o processo nos outros módulos, então a fidelidade importa.

LEIA PRIMEIRO, nesta ordem:
1. docs/design-review/workspace-harness/plano-implementacao.md (o plano completo: princípios,
   telas, fases, verificação, relatório). Siga-o.
2. docs/design-review/workspace-harness/README.md (as telas atuais do app, estados, interações e
   12 problemas de design observados) e prompts.md (o que foi pedido ao Stitch para cada tela).
3. As imagens em docs/design-review/workspace-harness/atual/ (o app antes da repaginação).

TAREFA
Crie a branch `design/workspace-harness` a partir de `main`. Rode `npm ci` e `npm run check` e
registre a linha de base. Depois aplique, tela por tela, o design do Stitch (projeto
`526454930665111857`, design system `assets/7aa0774b734b4149b2d38a23136aadfd`) nestas telas, cujos
ids estão no plano: Workspace — Lista de agentes, Workspace — Grade de agentes, Agente — Detalhes e
permissões, Execução — Inspetor, Projetos e Harness, Harness — Analisar projeto, Harness — Revisar
achados, Boas-vindas — Criar primeiro workspace, Seletor de workspace.

REGRAS QUE NÃO PODEM SER QUEBRADAS
- O Stitch é a referência VISUAL, não a especificação funcional. Não viaje: se o Stitch mostrar algo
  que o app não faz (campo, botão, métrica, filtro, atalho, tela, texto de estado inexistente),
  NÃO implemente. Liste no relatório em "O Stitch mostra, o app não tem". Se o Stitch omitiu um dado
  que o app mostra hoje, MANTENHA o dado e liste em "Stitch omitiu, app manteve".
- Máxima fidelidade ao que o Stitch mostra e o app suporta: estrutura, hierarquia, agrupamento,
  espaçamento, tipografia, cor, ordem dos blocos, rótulos e nomes de tela. Divergir só com motivo
  escrito.
- Funcionalidade intacta: mesmos comandos Tauri, estados, regras e testes de comportamento. Passar de
  modal para página (Projetos e Harness e as etapas de análise/revisão) ou de modal estreito para
  painel largo (Inspetor) é só apresentação; o estado e os hooks continuam os mesmos.
- Responsividade para desktop: a janela abre em 1100×720 e tem largura mínima 720
  (src-tauri/tauri.conf.json). Funcionar de 720px a 2560px (alturas 700 a 900): perfeito de 1100px
  para cima, degradando com elegância abaixo (barra lateral recolhida, colunas empilhadas). Sem
  rolagem horizontal da página, sem texto cortado sem reticências, colunas que se reorganizam por
  container. Teste cada tela em 720, 1100, 1280, 1440, 1920 e 2560, nos estados vazio, carregando,
  erro e com conteúdo longo.
- É repaginação, não redesenho: tema escuro único, tokens de src/styles/global.css, Geist e
  JetBrains Mono, borda de 1px em vez de sombra (só overlay tem sombra), raios 4/6/8px, `Button` com
  `variant`. Só crie token novo se o valor se repetir e não existir.
- Todo texto vem de src/i18n/pt-BR.ts e en-US.ts (nos dois). Nenhuma string fixa no JSX.
- Escopo fechado: só o módulo e os compartilhados de que ele depende. Se alterar Button, Modal, Icon
  ou AppShell, confirme que as telas dos outros módulos seguem corretas e liste a mudança.
- Não toque em Rust, em DTOs, em nomes de comandos, nem em Workflow, Agentes, Personalidades, Specs,
  MCP Hub, Configurações ou no app móvel.

COMO OBTER O VISUAL
Leia cada tela com `get_screen` e baixe o HTML (`htmlCode.downloadUrl`) e a imagem
(`screenshot.downloadUrl`). Se o download der 401, renderize o HTML no navegador e, se também não
der, peça as imagens ao usuário. Não implemente de memória nem adivinhando o layout. O HTML do
Stitch é Tailwind: traduza para os tokens do app.

COMO VERIFICAR
Use o harness em docs/design-review/workspace-harness/captura/ (instruções no plano, §5): copie para
tmp/design-capture/, rode `npx vite --port 1420` e abra
http://localhost:1420/tmp/design-capture/index.html (parâmetros ?harness=none|review|ready|stale,
?agents=0..3, ?runs=0|1, ?workspaces=0). Compare cada tela com o Stitch lado a lado. Se faltar um
estado no mock, estenda o mock em tmp/, não o código do app.

ENTREGA
- Fases do plano (§4) com commits pequenos em português no estilo do histórico (feat(workspace): ...,
  refactor(ui): ..., test(...), docs(design): ...). Termine com `npm run check` verde, ajustando
  testes ao novo DOM sem enfraquecer asserções.
- Relatório em docs/design-review/workspace-harness/implementacao/relatorio.md com o conteúdo do
  plano (§7), incluindo as imagens do app ao lado das do Stitch.
- NÃO faça push, NÃO abra PR, NÃO mexa na main. Quando terminar, diga como o usuário roda a branch
  para ver o resultado (comandos exatos) e resuma os pontos que precisam da decisão dele.

Se algo no plano conflitar com o que você encontrar no código, pare e pergunte antes de decidir.
```
