# Prompt para o chat de implementação — refatoração visual geral (todas as telas)

Colar num chat novo, aberto na raiz do repositório. Implementa as 32 telas da refatoração geral **de uma vez**,
sem fases com parada para aprovação, numa branch própria.

Antes de colar: a pasta `docs/design-review/refatoracao-geral/` (prompts, registro, `stitch/` com HTML e PNG)
precisa estar commitada na `main`, para a branch nova nascer com ela.

```text
Você vai implementar, no app Atlas (Tauri 2 + React + TypeScript), a REFATORAÇÃO VISUAL GERAL desenhada no
Stitch: um design system novo e 32 telas que cobrem o app inteiro. Implemente TODAS as telas numa única
entrega, sem parar entre módulos para pedir aprovação, com a máxima fidelidade visual ao Stitch. A
funcionalidade do app não muda: muda a apresentação.

1. BRANCH (antes de tudo)
- `git checkout main && git pull` e confira que existe docs/design-review/refatoracao-geral/stitch/ com 33 PNG,
  33 HTML e o DESIGN.md. Se não existir, PARE e me avise.
- Crie a branch `design/refatoracao-visual` a partir da main. Todo o trabalho fica nela.
- `npm ci` e `npm run check`; registre a linha de base (deve estar verde).
- Commits em português, no estilo do histórico (`refactor(ui): …`, `feat(workflow): …`, `test(…): …`,
  `docs(design): …`), feitos com a identidade do git configurada, SEM linha Co-Authored-By e sem mencionar
  IA. Pode commitar por área para facilitar a revisão, mas não pare entre elas. NÃO faça push, NÃO abra PR,
  NÃO mexa na main.

2. O QUE LER (nesta ordem)
1. docs/design-review/refatoracao-geral/stitch/DESIGN.md — tokens do design system novo.
2. docs/design-review/refatoracao-geral/prompts-por-modulo.md — a BASE (regras comuns) e, por tela, o que ela
   CONTÉM e NÃO CONTÉM; no fim, o REGISTRO com o id de cada tela e o que o Stitch inventou nela.
3. docs/design-review/refatoracao-geral/prompt-stitch.md — o pedido original completo: inventário de
   funcionalidades (parte E), glossário e estados (parte C) e responsividade (parte F).
4. docs/design-review/_comum/principios-implementacao.md — regras de implementação que valem aqui
   (seções 1, 5 e 6). Onde ele fala em "módulo", leia "a refatoração inteira"; as fases dele NÃO se aplicam.
5. Os dossiês antigos de cada módulo (docs/design-review/<módulo>/README.md) quando precisar saber o que o app
   faz hoje numa tela.

3. O VISUAL
Os arquivos estão em docs/design-review/refatoracao-geral/stitch/, numerados como nos prompts:

| Arquivo (.html/.png)                      | Tela                                   | Código atual (ponto de partida)                                   |
| ----------------------------------------- | -------------------------------------- | ----------------------------------------------------------------- |
| 00-design-system                          | Design system                          | src/styles/global.css, src/components/ui/*                        |
| 1.1-workspace-lista                       | Workspace — Lista                      | features/workspace/pages/WorkspacePage, AgentCard, AgentHeader, AgentMessageList, AgentComposer, WorkspaceUsageChip |
| 1.2-workspace-grade                       | Workspace — Grade                      | WorkspaceGrid, AgentCard, AddAgentPanel                           |
| 1.3-workspace-agente-executando           | Agente executando + permissão          | AgentActivity, ApprovalRequests, ActiveExecutions, ActionRequired |
| 1.4-workspace-detalhes-do-agente          | Detalhes do agente (gaveta com abas)   | AgentDetailsPanel, AgentPermissions, ContextPlatformPanel, ContextReviewPanel, MemoryPanel, McpPanel, RulesPanel, GitDetails, usage/TotalsView |
| 1.5-workspace-execucoes-do-agente         | Execuções (mestre-detalhe no cartão)   | ExecutionHistory, ExecutionInspector, ExecutionDetails, TerminalPanel |
| 1.6-workspace-boas-vindas                 | Boas-vindas (sem workspace)            | OnboardingPanel, WorkspaceForm, WorkspaceSwitcher                 |
| 2.1-projetos-e-harness                    | Projetos e Harness (página)            | WorkspaceProjectsPanel, ProjectForm, WorkspaceInitializeModal     |
| 2.2-harness-analisar-projeto              | Harness — Analisar (página com passos) | harness/InitializeProjectModal, HarnessFrame                      |
| 2.3-harness-analise-ao-vivo               | Harness — Análise ao vivo              | harness/AgentAnalysisScreen                                       |
| 2.4-harness-revisar-contexto              | Harness — Revisar contexto             | InitializeProjectModal (revisão), FindingRow, ConflictsPanel, StaleNotice, HarnessStats |
| 3.1-workflow-editar                       | Workflow — Editar                      | workflow/pages/WorkflowPage, WorkflowToolbar, WorkflowCanvas, WorkflowNodes, WorkflowOverview, WorkflowTools, AutoPlansToggle, StartChoice (specs) |
| 3.2-workflow-etapa-selecionada            | Inspetor da etapa                      | WorkflowInspector, StepTools                                      |
| 3.3-workflow-novo-workflow                | Novo workflow (passos)                 | NewWorkflowPanel, TemplatePreview                                 |
| 3.4-workflow-execucao-aguardando-voce     | Ação necessária                        | InteractionPanel, ActionRequired                                  |
| 3.5-workflow-execucao-em-andamento        | Execução em andamento                  | WorkflowOverview (execução), HandoffView, LiveWorkspacePanel, LiveFiles, LiveCode, PlansPanel, SharedStatePanel |
| 3.6-workflow-recuperar-execucao           | Recuperar execução                     | RecoveryPanel                                                     |
| 3.7-workflow-revisao-e-entrega            | Revisão e entrega do código            | ReviewWorkspace, DeliveryPanel, ChangesModal                      |
| 4.1-agentes-lista                         | Agentes — Lista (tabela)               | app/AgentsScreen, agents/pages/AgentsPage, AgentList              |
| 4.2-agentes-criar-agente                  | Criar agente (uma coluna)              | AgentForm, RuntimePicker, ResultContractEditor, EditAgentPanel    |
| 4.3-personalidades-biblioteca             | Personalidades — Biblioteca            | app/PersonalitiesScreen, PersonalitiesPage, PersonalityBrowser, PersonalityCard |
| 4.4-personalidades-editar                 | Editar personalidade (gaveta)          | PersonalityForm                                                   |
| 5.1-specs-lista                           | Specs — Lista (linhas)                 | specs/pages/SpecsPage, SpecCard, StatusBadge                      |
| 5.2-specs-documento-em-revisao            | Documento em revisão                   | SpecDetail, Stepper, ArtifactDocument, Inspector, OpenQuestions, UsageBar, Dock |
| 5.3-specs-revisao-final                   | Revisão final                          | FinalReview, ReviewPanel, Findings, PublishPanel, CliPanel, SpecRunPanel |
| 5.4-specs-nova-especificacao              | Nova especificação (modal)             | NewSpecDialog                                                     |
| 6.1-mcp-hub-servidores                    | MCP Hub — Servidores                   | workspace/pages/McpHubPage, hub/ServerCard, hub/Inspector, hub/GrantAgents |
| 6.2-mcp-hub-adicionar-servidor            | Adicionar servidor (modal)             | hub/AddServer                                                     |
| 6.3-mcp-hub-sincronizar-clis              | Sincronizar CLIs (modal)               | hub/SyncRuntimes, hub/ConfigIo                                    |
| 7.1-configuracoes-geral                   | Configurações — Geral                  | settings/pages/SettingsPage, SandboxSettings, PresenceSettings, LanguageSwitch |
| 7.2-configuracoes-memoria                 | Configurações — Memória                | memory/components/MemorySettings                                  |
| 7.3-configuracoes-importar-e-exportar     | Importar e exportar                    | portability/PortabilitySettings, ExportDialog, ImportDialog, ImportItemCard, ImportResult |
| 7.4-acesso-pelo-celular                   | Acesso pelo celular (modal)            | remote/MobileAccessButton, MobileAccessDialog                     |

Compare cada tela com o PNG e com o HTML (o HTML é Tailwind: converta para os tokens e CSS Modules do app;
não traga Tailwind para o projeto). Nunca implemente de memória. O Stitch está no projeto
526454930665111857 se precisar de algo que não esteja na pasta.

4. O QUE "TOTAL FIDELIDADE" QUER DIZER AQUI
Fiel ao Stitch em: estrutura de cada tela, hierarquia, agrupamento, ordem, tipografia, espaçamento, cores,
raios, bordas, selos, botões, rótulos e textos que o prompt pediu, e o padrão de cada superfície (página,
gaveta, modal, mestre-detalhe, passos). Mudanças de estrutura que o Stitch fez e são PARA IMPLEMENTAR:
- Detalhes do agente vira gaveta à direita com abas (Resumo, Permissões, Contexto, Consumo).
- Execuções do agente viram mestre-detalhe DENTRO do cartão (sem modal).
- Projetos e Harness vira página própria; o Harness vira página com 3 passos (Analisar, Análise ao vivo,
  Revisar contexto), sem modal sobre modal.
- Workflow: canvas como figura principal, barra do workflow + barra de execução em uma linha cada, ferramentas
  flutuantes no canvas, inspetor da etapa com seções recolhíveis, "Ação necessária" como painel ao lado do
  canvas, revisão com o cartão de entrega em destaque. Novo workflow em passos.
- Agentes: lista em tabela; criar/editar agente em UMA coluna com rodapé fixo e a sugestão da personalidade
  logo abaixo da escolha.
- Personalidades: cartões resumidos que expandem; editar em gaveta.
- Specs: lista em linhas; detalhe com passos em cartões e barra "Sua decisão" fixa.
- Configurações: índice de seções à esquerda (200px) e coluna de 720px.
- Moldura: logo fixo (quadrado índigo com "A"), barra superior só com os indicadores, "Celular" e PT | EN.

5. O QUE NÃO SE IMPLEMENTA (mesmo estando no Stitch)
- Tudo que o REGISTRO de prompts-por-modulo.md marca como inventado: barras de status no rodapé ("Daemon
  local…", "Motor orquestrador: ativo", "4 especificações indexadas"), rótulos técnicos ("LATÊNCIA",
  "Stream stdout", "TOKEN BATCH", "taxa de geração", "Conexão local ativa", "3 operações estruturais
  planejadas", "Servidor HTTP local ativo", "5 assinaturas stdio", POST/GET nas ferramentas), ids
  ("SEC-AUTH-091", "TEMPLATE ID", "SPEC-001"), "Foco atual", "Topologia de agentes", "ESC para fechar",
  barra de título do macOS, subtítulos nos nós do canvas, placeholders inventados, requisitos do Figma, o
  rótulo "JetBrains Mono", "#" nas tags, agentes e textos inventados nos fundos borrados.
- Os dados de exemplo do Stitch (Transport ERP, Arquiteto, gpt-5-codex, US$ 1,24…): use os dados reais.
- Qualquer função que o app não tem (a lista completa está na parte G de prompt-stitch.md). O Stitch é a
  referência VISUAL; o app é a fonte da verdade do que existe.
- Liste tudo isso no relatório em "O Stitch mostra, o app não tem".

6. O QUE O STITCH NÃO DESENHOU E VOCÊ MANTÉM
- Toda informação e ação que o app tem e a tela não mostrou continua acessível, no lugar mais próximo do
  padrão desenhado (ex.: abas Permissões/Contexto/Consumo da gaveta do agente; Chat/Terminal do inspetor de
  execução; abas Variáveis/Manifesto/Atividade do MCP Hub; Registro MCP e Manual em Adicionar servidor;
  "Sobre e atualizações" nas Configurações; Passagens, Alterações e Ao vivo da execução; reparo de rotas,
  salvar como template, importar/exportar; estados de erro, carregando e vazio).
- Telas e estados sem desenho (diálogos de portabilidade, reparo de rotas, perguntas da spec, importar specs,
  formulário de workspace, gerenciar workspaces, modais de atualizar Harness e contexto de tarefa, estados
  vazio/carregando/erro de cada página): aplique o mesmo design system e os padrões das telas desenhadas.
- Liste no relatório em "Stitch omitiu, app manteve".

7. DECISÕES JÁ TOMADAS (não pergunte)
- Tipografia: adote a escala do DESIGN.md (corpo 14px etc.) em src/styles/global.css; ajuste os tokens, não
  cada componente.
- Selos: o formato que o Stitch desenhou em todas as telas (retangular, raio 4px, fundo a 14%, borda a 40%,
  ponto de 6px nos de estado), no componente Badge. Um único mapa estado → tom, usado em toda tela (lista de
  estados e cores na parte C de prompt-stitch.md).
- Títulos e rótulos sempre em caixa normal, mesmo onde o Stitch pôs caixa alta (ele foi inconsistente).
- Nas telas que saíram em dobro, a versão certa é a do arquivo numerado na pasta stitch/.
- "Workflow — Editar" estourou 1440px no Stitch: implemente o mesmo desenho sem estourar (quebre a barra em
  duas linhas ordenadas ou mande ações secundárias para o "⋯").
- Terminologia (parte C de prompt-stitch.md): "Agente" (nunca "agent"), "Etapa" (nunca "step"), "Passagem
  (handoff)", "Achado". Corrija pt-BR.ts onde hoje há inglês misturado (ex.: 'interaction.indicatorOne' =
  "1 agent aguardando você", textos de handoff com "step"). en-US.ts acompanha com termos equivalentes.
- Responsividade: o Stitch só desenhou 1440. Aplique a parte F de prompt-stitch.md (720–1023, 1024–1439,
  1440–1919, ≥1920) com container queries/media queries: barra lateral vira trilho de ícones, colunas
  laterais viram gaveta, grades mudam de colunas, conteúdo de leitura com largura máxima.

8. REGRAS QUE NÃO PODEM SER QUEBRADAS
- Sem Rust, sem DTOs, sem nomes de comandos Tauri, sem funcionalidade nova. Mesmos hooks, estados e regras.
- Testes de comportamento continuam valendo: ajuste seletores e textos ao DOM novo sem enfraquecer asserções.
- Todo texto em pt-BR.ts E en-US.ts (o compilador exige as mesmas chaves). Nada fixo no JSX. Remova chaves
  que ficarem sem uso.
- Reaproveite Badge, Button (variant), CollapsibleSection, OverflowMenu, Steps e Modal; crie componente novo
  só se faltar (ex.: Drawer, SegmentedControl, SettingsNav) e use-o em todo lugar que couber.
- A página do celular (mobile.html, src/mobile) NÃO faz parte deste trabalho.
- Se algo nos prompts conflitar com o código real, siga o código (funcionalidade) e registre; pergunte só se
  não houver saída sem perder função.

9. COMO VERIFICAR
- `npm run check` verde no fim (tipos, lint, formatação, testes de frontend e Rust). Não rode testes
  `--ignored` amplos: chamam modelos pagos.
- Copie docs/design-review/_comum/captura/* para tmp/design-capture/ (ignorado pelo git), rode
  `npx vite --port 1420` e abra http://localhost:1420/tmp/design-capture/index.html (parâmetros no main.tsx).
  Estenda o mock em tmp/ para os estados que faltarem; não mexa no código do app para isso.
- Capture cada tela em 1440×900 e compare lado a lado com o PNG do Stitch. Rode o chk.js em 720, 1024, 1280,
  1440, 1920 e 2560: sem rolagem horizontal, sem texto cortado sem reticências, ação principal sempre visível.

10. ENTREGA
- Relatório em docs/design-review/refatoracao-geral/implementacao/relatorio.md, com capturas em
  implementacao/img/ (antes e depois): (1) por tela: fiel / fiel com ajustes / divergente, com o motivo;
  (2) "O Stitch mostra, o app não tem"; (3) "Stitch omitiu, app manteve"; (4) divergências intencionais;
  (5) componentes novos e tokens alterados; (6) testes alterados e resultado do `npm run check`;
  (7) o que não deu para verificar e o que depende de decisão minha.
- No fim, diga em poucas linhas o que ficou pronto, o comando para eu ver a branch e as decisões pendentes.
```
