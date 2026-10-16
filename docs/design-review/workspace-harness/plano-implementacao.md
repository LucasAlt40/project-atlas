# Plano de implementação — Repaginação do módulo Workspace + Harness

Objetivo: aplicar no app a repaginação gerada no Stitch para o módulo Workspace + Harness, numa
**branch separada** (`design/workspace-harness`), para avaliar o resultado antes de repetir o
processo em Workflow e nos demais módulos. Nada vai para `main` nesta etapa.

## 1. Princípios (valem para toda decisão)

1. **O Stitch é a referência visual, não a especificação funcional.** O app é a fonte de verdade
   do que existe: dados, ações, estados, textos de erro, regras. Se o Stitch mostrar algo que
   o app não faz (campo, botão, métrica, filtro, atalho, tela, texto de status que não existe), **não
   implemente**. Registre no relatório final, numa lista "O Stitch mostra, o app não tem".
2. **Máxima fidelidade ao que o Stitch mostra e o app suporta.** Estrutura, hierarquia, agrupamento,
   espaçamento, tipografia, cor, peso visual, ordem dos elementos, rótulos e nomes de tela devem
   bater. Divergir só com motivo escrito (ver §6).
3. **Nada de informação some.** A repaginação separa melhor; não esconde dado. Se o Stitch omitiu
   um dado que o app mostra hoje, **mantenha o dado** no lugar mais próximo do padrão do Stitch e
   anote em "Stitch omitiu, app manteve".
4. **Funcionalidade intacta.** Mesmos comandos, mesmos estados, mesmas regras de negócio,
   mesmos testes de comportamento. Mudança de modal para página (ou de modal estreito para painel
   largo) é apresentação: o estado, os hooks e o fluxo continuam os mesmos.
5. **Desktop com responsividade.** Janela Tauri: abre em 1100×720 e a largura mínima é 720
   (`src-tauri/tauri.conf.json`); uso principal em ≥ 1280px. Tem que funcionar de 720px até 2560px:
   perfeito de 1100px para cima, degradando com elegância abaixo disso (barra lateral recolhida,
   colunas empilhadas). Sem rolagem horizontal da página, sem texto cortado sem reticências, colunas
   que se reorganizam por container, não só por viewport. Alturas pequenas (≥ 700px) não podem
   esconder ações primárias.
6. **Identidade preservada.** Tema escuro único, tokens de `src/styles/global.css`, Geist e
   JetBrains Mono, 1px de borda em vez de sombra (só overlay tem sombra), raios 4/6/8px, `Button`
   com `variant` (primary / secondary / ghost). É repaginação, não redesenho.
7. **Idioma e i18n.** Todo texto sai de `src/i18n/pt-BR.ts` e `en-US.ts`; nenhuma string
   fixa no JSX. Texto novo do Stitch entra nos **dois** arquivos de idioma.
8. **Escopo fechado.** Só os arquivos do módulo e os componentes compartilhados de que ele depende.
   Se mudar um compartilhado (`Button`, `Modal`, `AppShell`, `Icon`), confirme que as telas dos outros
   módulos continuam corretas e liste a mudança no relatório.

## 2. Telas e o que implementar

Ids do projeto Stitch `526454930665111857`. O que o app já tem fica no código; o caminho dos
arquivos é a hipótese de partida e deve ser confirmado lendo o código.

| # | Tela no Stitch (nome = título) | Id da tela nova | Original que ela refaz | Código do app (hipótese) |
|---|---|---|---|---|
| 1 | Workspace — Lista de agentes | `d1b03b16eb254cfe87d5bdd13109c61b` | `ee4c1206…` | `workspace/pages/WorkspacePage.tsx`, `AgentCard`, `AgentHeader`, `AgentMessageList`, `AgentComposer`, `WorkspaceUsageChip`, `Workspace.module.css`, `AgentCard.module.css` |
| 2 | Workspace — Grade de agentes | `aad74b5709b446468c817c8efefdf737` | `6eecbeea…` | `workspace/components/WorkspaceGrid.tsx` + `AgentCard` |
| 3 | Agente — Detalhes e permissões | `967bb5d474564863823bfd2f31ddd137` | `01d7d6b3…` | `AgentDetailsPanel`, `AgentPermissions`, `GitDetails`, `ContextPlatformPanel`, `MemoryPanel`, `McpPanel`, `RulesPanel`, `Details.module.css` |
| 4 | Execução — Inspetor | `5903404c41b6412c8cd0c79cab3ab2d6` | `4c0c75ea…` | `ExecutionInspector`, `ExecutionDetails`, `AgentActivity`, `Inspector.module.css` |
| 5 | Projetos e Harness | `299f698ba6a542aba7ac511ed4b29c64` | `499b4695…` | `WorkspaceProjectsPanel`, `ProjectForm`, `WorkspaceInitializeModal`, `Projects.module.css` |
| 6 | Harness — Analisar projeto | `68ed60d88752430d878d6d4648f5734c` | `cf432e92…` | `harness/components/InitializeProjectModal.tsx` (estados `findings`/`analyzing`), `AgentAnalysisScreen` |
| 7 | Harness — Revisar achados | `af24471b2e3f4ff4983290e1e5cba9f9` | `54508e9f…` | `InitializeProjectModal.tsx` (estado `review`), `FindingRow`, `ConflictsPanel`, `DiffView`, `HealthBadge`, `StaleNotice` |
| 8a | Boas-vindas — Criar primeiro workspace | `c65576984a6a4b6e849b57e0b758a70c` | (nova) | `workspace/components/OnboardingPanel.tsx` |
| 8b | Seletor de workspace | `a1838a92b4de4eab99a02355f07136ce` | (nova) | `workspace/components/WorkspaceSwitcher.tsx` |

Observações sobre o que o Stitch propôs de estrutural (a confirmar na tela gerada):
- **Projetos e Harness** deixa de ser modal e vira página cheia do workspace; **Analisar projeto** e
  **Revisar achados** viram etapas ("1 de 3", "2 de 3") com indicador de passos dentro dessa
  página. Hoje são três níveis de modal. Implementar como etapas na mesma área, mantendo
  `InitializeProjectModal` como fonte do estado (estágios `analyzing`, `findings`, `review`, `done`,
  `failed`); a terceira etapa do indicador ("Inicializar") corresponde ao envio e ao estado `done`.
- **Execução — Inspetor** vira painel largo em vez de modal estreito, com a linha do tempo ao lado.
- A moldura (barra lateral e superior) não muda, salvo o que o Stitch mostrar de diferente no
  seletor de workspace.

## 3. Fonte da verdade visual: como obter

1. Ler cada tela com `mcp__stitch__get_screen` (`projects/526454930665111857/screens/<id>`) e
   baixar o HTML (`htmlCode.downloadUrl`) e a imagem (`screenshot.downloadUrl`). Nesta sessão de
   revisão o download da imagem deu **401** sem credencial; se também der 401 aí, use o HTML
   (renderize no navegador do app) ou peça ao usuário as imagens. Não implemente "de memória".
2. Extrair do HTML os valores reais: cores, tamanhos de fonte, espaçamentos, raios, larguras de
   coluna, ordem dos blocos. O HTML do Stitch é Tailwind: converta para os tokens do app
   (`var(--surface-1)`, `var(--space-3)`...) e só crie token novo se o valor se repetir e não existir.
3. O design system do projeto é `assets/7aa0774b734b4149b2d38a23136aadfd` ("Atlas Engineering
   Core"). `global.css` já espelha seus tokens. Se o Stitch usou valores que divergem dos tokens,
   registre a divergência e prefira o token, salvo se a diferença for visível e intencional.

## 4. Fases

| Fase | O que entrega | Verificação |
|---|---|---|
| 0. Preparação | Branch `design/workspace-harness` a partir de `main`; `npm ci`; `npm run check` verde como linha de base; capturas "antes" de cada tela com a ferramenta de `captura/` | linha de base registrada |
| 1. Levantamento | Para cada tela: `get_screen`, HTML e imagem; quadro "elemento do Stitch → componente/dado do app → existe / não existe"; lista dos itens que **não** serão implementados | `docs/design-review/workspace-harness/implementacao/levantamento.md` |
| 2. Base compartilhada | Ajustes em tokens, `Button`, selos/badges (um sistema único de selos), `Modal`/painel largo, tabela densa, indicador de passos, menu "⋯", seção recolhível | testes dos componentes; telas dos outros módulos intactas |
| 3. Workspace | Telas 1, 2, 8a, 8b | captura lado a lado com o Stitch |
| 4. Agente e execução | Telas 3 e 4 | idem |
| 5. Projetos e Harness | Telas 5, 6, 7 e o fluxo completo (sem análise → analisar → revisar → inicializar; atualizar; recarregar; resetar; excluir) | idem + testes existentes de `InitializeProjectModal`, `ProjectHarness` |
| 6. Responsividade | Cada tela em 720, 1100, 1280, 1440, 1920 e 2560 de largura, alturas 700 e 900; estados vazio, carregando, erro e longos (nomes e caminhos compridos, muitos agentes, muitos achados) | capturas por tamanho |
| 7. Fechamento | Testes ajustados ao novo DOM sem enfraquecer asserções; `npm run check` verde; relatório | relatório final (§7) |

Commits pequenos por fase, em português, no estilo do histórico (`feat(workspace): ...`,
`refactor(ui): ...`, `test(...)`, `docs(design): ...`). Não fazer push nem abrir PR.

## 5. Como verificar visualmente

`docs/design-review/workspace-harness/captura/` tem o harness que roda o app real no navegador com
um Tauri simulado (dados de exemplo, sem backend). Para usar:

```bash
mkdir -p tmp/design-capture
cp docs/design-review/workspace-harness/captura/* tmp/design-capture/
npx vite --port 1420
# abrir http://localhost:1420/tmp/design-capture/index.html
```

Parâmetros: `?harness=none|review|ready|stale`, `?agents=0..3`, `?runs=0|1`, `?workspaces=0`. O
`tmp/` é ignorado pelo git. O harness cobre o essencial; se faltar um estado (agente em
execução, aprovação pendente, terminal, falha), estenda o mock em `tmp/`, não no código do app.

Para cada tela: capturar o app em 1440×900 e comparar com a imagem do Stitch; depois repetir nas
larguras da fase 6. Diferenças visíveis ficam no relatório ou são corrigidas.

## 6. Quando divergir do Stitch (e como registrar)

Pode divergir, anotando o motivo, quando: (a) o Stitch pede dado ou ação que o app não tem;
(b) o texto do Stitch contradiz o comportamento real; (c) o layout do Stitch quebra com conteúdo
real (nomes longos, muitos itens) e precisa de ajuste; (d) acessibilidade (foco visível, contraste,
navegação por teclado, `aria-*` existentes) seria perdida; (e) a largura do Stitch (2560) não se
traduz para a janela real. Em qualquer caso, o ajuste segue o espírito do Stitch.

## 7. Relatório final (entregável)

`docs/design-review/workspace-harness/implementacao/relatorio.md` com:
1. Por tela: fiel / fiel com ajustes / divergente, com a imagem do app ao lado da do Stitch.
2. **O Stitch mostra, o app não tem** (não implementado).
3. **Stitch omitiu, app manteve.**
4. Divergências intencionais e motivo.
5. Mudanças em componentes compartilhados e telas de outros módulos afetadas.
6. Resultado de `npm run check`, testes alterados e por quê.
7. Pendências e sugestões para os próximos módulos (Workflow e demais).

## 8. Fora do escopo

Workflow, Agentes, Personalidades, Specs, MCP Hub, Configurações e o app móvel; código Rust;
qualquer funcionalidade nova; mudança de nomes de comandos Tauri ou de DTOs; internacionalização
de novos idiomas.
