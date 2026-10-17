# Relatório — repaginação do módulo Workspace + Harness

Branch `design/workspace-harness`, a partir de `main`. Referência visual: [`../stitch/`](../stitch/) (HTML e PNG exportados do projeto Stitch `526454930665111857`). Levantamento detalhado: [`levantamento.md`](levantamento.md).

Os PNGs do app foram gerados a 1440×900 com o harness de `captura/` (dados de exemplo). Os do Stitch têm 2560 px de largura, por isso a escala é diferente.

## 1. Por tela

### 1. Workspace — Lista de agentes — fiel com ajustes

Cabeçalho, faixa de metadados, abas sublinhadas com contagem, compositor em uma linha, ⋯ com Detalhes e Remover. Faltam os chips "1 agente em execução" e "backend · main" e os ícones nas abas (não existem no app / não criados).

| Stitch                                    | App                                  |
| ----------------------------------------- | ------------------------------------ |
| ![Stitch](../stitch/01-lista-agentes.png) | ![App](img/app-01-lista-agentes.png) |

### 2. Workspace — Grade de agentes — fiel com ajustes

Mesmos cartões em duas colunas; abaixo de ~52rem de largura da página a grade vira uma coluna. A célula vazia convida a adicionar um agente; os presets rápidos não existem no app.

| Stitch                                    | App                                  |
| ----------------------------------------- | ------------------------------------ |
| ![Stitch](../stitch/02-grade-agentes.png) | ![App](img/app-02-grade-agentes.png) |

### 3. Agente — Detalhes e permissões — fiel com ajustes

Gaveta de 32rem, seções recolhíveis, resumo "Consumo hoje", cartões de permissão com tag. Sem "Nível 2", "Perfil atual" no resumo, nem o rodapé "Configurar regras / Salvar alterações".

| Stitch                                      | App                                    |
| ------------------------------------------- | -------------------------------------- |
| ![Stitch](../stitch/03-agente-detalhes.png) | ![App](img/app-03-agente-detalhes.png) |

### 4. Execução — Inspetor — fiel com ajustes

Painel largo, selo, tarefa, status/duração/custo, fatos em grade e linha do tempo ao lado. Sem os botões Exportar log, Reexecutar, Ver diff e Abrir no Workspace.

| Stitch                                        | App                                      |
| --------------------------------------------- | ---------------------------------------- |
| ![Stitch](../stitch/04-execucao-inspetor.png) | ![App](img/app-04-execucao-inspetor.png) |

### 5. Projetos e Harness — fiel com ajustes

Página no lugar do modal; linhas com selos unificados, barra de achados e ⋯ (Resetar, Excluir); três cartões embaixo. Sem o selo de cluster, o filtro e o rodapé do motor.

| Stitch                                       | App                                     |
| -------------------------------------------- | --------------------------------------- |
| ![Stitch](../stitch/05-projetos-harness.png) | ![App](img/app-05-projetos-harness.png) |

### 6. Harness — Analisar projeto — fiel com ajustes

Etapa 1 na página, com o indicador de passos e as duas opções lado a lado (Análise com IA | Revisar sem IA). As ações ficam no rodapé fixo, sem duplicar "Analisar com IA" dentro do cartão.

| Stitch                                       | App                                     |
| -------------------------------------------- | --------------------------------------- |
| ![Stitch](../stitch/06-harness-analisar.png) | ![App](img/app-06-harness-analisar.png) |

### 7. Harness — Revisar achados — fiel com ajustes (lista de achados não redesenhada)

Etapa 2 na página, com métricas, abas, inspetor de evidência e rodapé. Os cartões de achado (`FindingRow`) continuam como eram: o Stitch os desenha de outro jeito e isso não foi refeito.

| Stitch                                      | App                                    |
| ------------------------------------------- | -------------------------------------- |
| ![Stitch](../stitch/07-harness-revisar.png) | ![App](img/app-07-harness-revisar.png) |

### 8a. Boas-vindas — Criar primeiro workspace — fiel com ajustes

Cartão com título, frase e três passos numerados. Sem o rótulo de status, os rodapés dos passos e o atalho do terminal.

| Stitch                                   | App                                 |
| ---------------------------------------- | ----------------------------------- |
| ![Stitch](../stitch/08a-boas-vindas.png) | ![App](img/app-08a-boas-vindas.png) |

### 8b. Seletor de workspace — fiel com ajustes

Menu com cabeçalho, ícone, selo "Ativo", visto, "Novo workspace" e "Gerenciar workspaces →". Sem contagem de livres, busca, atalho nem fundo desfocado.

| Stitch                                         | App                                       |
| ---------------------------------------------- | ----------------------------------------- |
| ![Stitch](../stitch/08b-seletor-workspace.png) | ![App](img/app-08b-seletor-workspace.png) |

## 2. O Stitch mostra, o app não tem (não implementado)

- **Moldura:** busca ⌘K, botão Atlas CLI, ícones de terminal, notificações e ajustes, avatar, "Sync OK", botão "Novo Fluxo", telemetria da barra lateral, versão "v2.4.0" e a barra de status inferior.
- **Workspace:** chips "1 agente em execução" e "backend · main"; cartões de métricas na aba Atividade; "Streaming tokens"; presets rápidos na célula vazia.
- **Detalhes do agente:** "Nível 2"; "Perfil atual" no resumo; "Escopo principal / Arquivos mapeados / Diretrizes"; rodapé "Configurar regras" e "Salvar alterações" (o perfil grava ao ser trocado); ⋮ do cabeçalho.
- **Inspetor:** Exportar log, Reexecutar tarefa, Ver diff no Git, Abrir no Workspace, ⋯; "Sessão", "Latência média de tool", "Resumo da execução & validação".
- **Projetos:** selo "Cluster 01-PROD", filtro "Todos os módulos", "Harness Sync: 100%", copiar caminho, `.atlas/harness.lock`, "Injetado no prompt", rodapé do motor.
- **Analisar:** "HARNESS SETUP", "ID: HRN-0924", tempos "(0.04s)", atalho "↵ Enter", mensagem "Nenhuma alteração foi salva no disco".
- **Revisar:** "Salvar rascunho", "Filtrar manifestos…", "Marcar todos", "Sync OK", "AST Parser", "RAM cognitiva".
- **Boas-vindas:** rótulo "Pronto para orquestrar", rodapés dos passos, "ou abra pelo terminal com atlas init".
- **Seletor:** "2 de 5 livres" (o app não limita workspaces), busca, atalho ⌘N.

## 3. Stitch omitiu, app manteve

- Rótulo "Este workspace" do chip de uso (fica só para leitores de tela).
- Estado do Harness **e** saúde do Harness na linha do projeto (o Stitch mostra um só selo).
- Tecnologias, dependências ("Usa:"), contagem de achados desatualizados, aviso de pasta ausente e as confirmações inline de resetar e excluir.
- Seletor de escopo no compositor, controles de execução, pré-visualização do contexto da tarefa, terminal e as abas Contexto e Git do inspetor.
- Consumo, Cota e dados do agente na gaveta de detalhes.
- Notas de privacidade, conflitos, diff e alertas da análise e da revisão.

## 4. Divergências intencionais

- **Nomes de tela:** "Revisar contexto do projeto" virou "Harness — Revisar achados" e "Analisar projeto" virou "Harness — Analisar projeto" (nomes do Stitch). Os subtítulos continuam os do app, porque descrevem o que ele faz.
- **Textos do Stitch que descreviam o que o app não faz** foram reescritos com o que ele faz (passos das boas-vindas, cartão "Revisar sem IA" com dois itens verdadeiros, sem tempos).
- **Ações do cartão de agente:** "Remover" e "Detalhes e permissões" ficam no ⋯ (como no Stitch), o que custa um clique a mais para remover.
- **Botões duplicados do Stitch** (Analisar com IA no cartão e no rodapé; Voltar/Inicializar no cabeçalho e no rodapé da revisão) não foram duplicados: ficou o do app.
- **Barra lateral:** abaixo de 900 px de janela ela começa recolhida (se o usuário nunca escolheu); a escolha dele continua valendo.
- **Grade:** o número de colunas passou a vir de uma variável CSS, e a grade vira coluna única quando a página é estreita.
- **Ícones nas abas e no avatar do agente:** o Stitch usa um por aba/personalidade; o app tem um ícone único de agente e abas só com texto.

## 5. Componentes compartilhados alterados

| Mudança                                                                                 | Quem mais usa                                     | Verificação                                                  |
| --------------------------------------------------------------------------------------- | ------------------------------------------------- | ------------------------------------------------------------ |
| Novos: `Badge`, `OverflowMenu`, `CollapsibleSection`, `Steps` (em `src/components/ui/`) | só o módulo                                       | testes do módulo                                             |
| `Modal`: tamanhos `drawer` e `inspector` (os existentes não mudaram)                    | todos os modais                                   | suíte inteira verde                                          |
| `Icon`: cinco ícones novos (`more`, `externalLink`, `chevronUp`, `arrowRight`, `play`)  | todos                                             | só acrescenta                                                |
| `AppShell`: recolhe sozinho abaixo de 900 px quando não há escolha salva                | todas as telas                                    | testes do `App`; **não conferi as outras telas visualmente** |
| `Terminal.module.css`: abas sublinhadas e com rolagem horizontal                        | cartão de agente e inspetor                       | testes                                                       |
| `ExecutionInspector` (painel largo com linha do tempo)                                  | também a página **Workflow** (`WorkflowPage`)     | testes passam; **não conferi visualmente na tela Workflow**  |
| `PolicySummary` / `Security.module.css` (cartões com tag)                               | gaveta do agente e selo de segurança do workspace | testes                                                       |
| `HealthBadge`, `HarnessStats` (barra)                                                   | só Workspace/Harness                              | testes                                                       |

Infra: `.prettierignore` ignora `docs/design-review/workspace-harness/stitch` e `tmp`; `eslint.config.js` ignora `tmp`; formatei com o Prettier cinco documentos que já estavam fora do padrão na `main`.

## 6. Resultado de `npm run check`

- Linha de base na `main` (com a pasta `tmp/` fora do caminho): lint verde, mas `format:check` falhava em documentos já commitados; corrigido no primeiro commit da branch. 67 arquivos de teste, 771 testes, Rust verde.
- Final: **verde** — typecheck, lint, Prettier, 67 arquivos / 771 testes (mesmos números), `cargo fmt`, `clippy` e `cargo test`. Não toquei em Rust, DTOs nem comandos.

**Testes alterados (DOM novo, mesmas asserções de comportamento):**

- `WorkspacePage.test.tsx`: metadados do agente agora lidos como "Personality: … Runtime: … Provider: …"; remover agente passa pelo menu ⋯; texto das boas-vindas; "Projects of Atlas" é uma região, não um diálogo.
- `ExecutionHistory.test.tsx`: "Git isolation: OFF" lido no texto do cartão.
- `WorkspaceSwitcher.test.tsx`: opção ativa inclui o selo "Active"; botão "New Workspace" sem o "+" no texto (agora é ícone).
- `WorkspaceProjects.test.tsx`, `ProjectHarness.test.tsx`: a página e as etapas são regiões (`region`), não diálogos; resetar e excluir passam pelo menu ⋯; títulos novos das etapas.
- `InitializeProjectModal.test.tsx`: títulos novos das etapas.

## 7. Pendências e sugestões

- **Não verificado:** estados do app que o mock não cobre (agente executando, aprovação pendente, falha, terminal ao vivo, análise com IA em andamento, modais de atualizar Harness e de contexto de tarefa), e a tela Workflow com o inspetor largo.
- **Responsividade:** verificada com o harness em 720, 1100 e 2560 px (lista, detalhes, inspetor, projetos, análise, revisão, boas-vindas e seletor): sem rolagem horizontal da página e sem texto cortado sem reticências, depois de dois ajustes (cabeçalho do cartão e métricas da revisão). 1280, 1440 e 1920 só foram vistos em 1440. Conteúdo longo (nomes e caminhos enormes, muitos agentes) **não foi testado de forma sistemática**.
- A lista de achados da revisão (`FindingRow`) não foi redesenhada.
- Para os próximos módulos: reaproveitar `Badge`, `OverflowMenu`, `CollapsibleSection`, `Steps` e `HarnessFrame`; o Workflow já usa o inspetor novo.
