# ADR 0033 — Workspaces com vários projetos

Status: aceito. Baseia-se na [ADR 0009](0009-git-worktree-isolation.md) (worktrees), na
[ADR 0010](0010-project-harness.md) e na [ADR 0012](0012-task-aware-context.md) (Harness, contexto ciente da tarefa), e
nas [ADR 0013](0013-workflow-engine-and-orchestrator.md) e [ADR 0014](0014-handoff-shared-worktree-and-code-integration.md)
(workflows, worktree compartilhado); não muda nenhuma das suas invariantes.

## Contexto

Um workspace era uma pasta (`projectPath`). Um sistema com um frontend, um backend e vários serviços precisava de um
workspace por repositório, e nenhum agente conseguia entender o todo. Onde o código presumia uma única pasta:
`Workspace.project_path` (o único caminho guardado), `HarnessService` (`project_path(workspace_id)`),
`ExecutionService::run_step` (diretório de trabalho, Harness, skills, rules, worktree), o resolvedor de segurança
(`project_root` = esse caminho), o `open_workspace` do workflow (o worktree compartilhado da execução), os arquivos de
runtime do MCP e a UI (`projectPath`).

Dois fatos do código existente moldaram a decisão. O Harness já vive no `.atlas/` de cada projeto e toda função do
Harness recebe uma pasta, então já é por projeto. E só um agente **isolado em um worktree** recebe ferramentas de edição
(`edit_access`), um worktree por execução, criado a partir da pasta que lhe for entregue (ele já lida com um projeto
dentro de um repositório maior).

## Decisão: híbrido (A + camada global), não um Harness unificado (B)

| Critério                              | A: Harness local por projeto + contexto do workspace                                           | B: um Harness unificado                                                      |
| ------------------------------------- | ---------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| Precisão, isolamento de regras locais | as regras, achados e evidências de cada projeto ficam como foram analisados                    | todo item precisa de uma tag de projeto; regras locais e globais se misturam |
| Tokens / limites                      | uma tarefa recebe os fragmentos dos projetos do seu escopo, dentro da única alocação existente | um grande armazenamento para filtrar, ou tudo                                |
| Atualização incremental               | adicionar ou mudar um projeto analisa só ele                                                   | qualquer mudança invalida uma análise                                        |
| Custo para começar                    | por projeto, a falha parcial é por projeto                                                     | uma longa execução de tudo ou nada                                           |
| Compatibilidade                       | o código de hoje e os arquivos `.atlas/` inalterados                                           | novo formato, migração de todo Harness                                       |
| Worktrees                             | um projeto é o seu próprio repositório                                                         | sem relação                                                                  |

Escolhido: **A mais um contexto do workspace** que relaciona os projetos. Não é uma cópia dos Harnesses: guarda nomes,
papéis, pastas, stack e estado do Harness, alguns fatos de integração/dados com evidência e a sua fonte, e as
dependências que o **usuário declarou** (o Atlas não as adivinha), mais as próprias anotações do usuário. Guardado na
pasta de dados do Atlas (`workspace-context/<id do workspace>/context.md` gerado, `notes.md` do usuário), nunca dentro de
um repositório: pertence ao workspace, um projeto pode estar em vários workspaces, e nada é commitado. Regenerar nunca
toca nas anotações. Carrega uma impressão digital do que foi usado para fazê-lo, de modo que a tela possa dizer que está
desatualizado.

## Modelo e migração

`Workspace.projects: Vec<WorkspaceProject { id, name, path, role?, depends_on }>` (novo rótulo `ProjectRole`, nada se
ramifica por ele). `projectPath` permanece e é sempre a pasta do projeto primário (o primeiro), de modo que tudo o que
precisa de uma pasta continue funcionando. `Workspace::normalize` deriva o único projeto de um workspace antigo (id
`<id do workspace>-main`, nome a partir da pasta), mantém `project_path` em sincronia e descarta dependências de
projetos ausentes; é idempotente, roda quando a configuração carrega (e é gravada então) e de novo sempre que os
workspaces são listados. Mais nada é tocado (ids, layout, segurança, conversas, execuções, `.atlas/`). Duplicatas são
comparadas pela pasta como ela realmente é (`ProjectInspector::canonical`; sem diferenciar maiúsculas no Windows e
macOS). Um workspace sempre mantém um projeto; remover um projeto só esquece o registro.

## Escopo de execução

`ScopeRequest { working_project_id?, read_project_ids }` → `ProjectScope { working, others }`, resolvido pelo
`WorkspaceService::resolve_scope`. Nenhum pedido é só o projeto primário, byte a byte o mesmo prompt e lançamento de
antes. Viaja em `SendMessageRequest`, `RunAgentRequest` e em uma execução de workflow (`WorkflowExecution.scope`, igual
para todo passo, validado quando a execução inicia).

- **Projeto de trabalho**: onde o processo roda, de cujo repositório o worktree é feito, a cuja pasta a camada de
  segurança prende o processo (`ExecutionScope.project_id`, um id procurado na configuração guardada, nunca um caminho).
- **Projetos consultados** são somente leitura. O prompt os lista com os limites; um fragmento ciente da tarefa do
  Harness de cada um é informado, rotulado com o projeto e sua pasta, dentro da alocação já existente da seção do
  Harness (contexto do workspace ≤ 1.200 caracteres, projeto de trabalho ≥ 60% do resto, os outros dividem o restante;
  os que não cabem são apenas nomeados). `HarnessContextBuilder::build_for_task_within` faz a seleção.
- **Pastas entregues ao runtime** (`RuntimeRequest.extra_dirs`, `ModelRuntime::accepts_extra_dirs`, implementado para o
  Claude como um `--add-dir` por pasta): somente para uma execução **sem ferramentas de edição**. A CLI do Claude não
  consegue dar uma pasta somente leitura a uma execução que edita, então um agente que edita conhece as outras pelo
  Harness delas, não pelos arquivos; o prompt e o log dizem isso. Os outros runtimes não recebem pastas extras até um
  adaptador ser verificado.

## Worktrees: um projeto gravável por execução

Escrever continua sendo o que a ADR 0009 fez: um worktree por execução, do projeto de trabalho. Repositórios
independentes, um projeto dentro de um monorepo e pastas sem Git se comportam como antes (um projeto sem Git não pode ser
isolado, então seus agentes só leem). Deliberadamente **não existe uma execução que escreva em dois projetos**: isso
precisaria de vários worktrees por execução, com sua própria revisão, validação e merge, e uma mudança entre
repositórios aplicada pela metade é exatamente o que os mecanismos de revisão existem para evitar. Uma tarefa que muda o
frontend e o backend são duas execuções (dois passos, ou duas execuções), cada uma revisada e mesclada por conta
própria. Uma execução de workflow tem um worktree compartilhado, portanto um projeto de trabalho; o escopo é o da
execução.

## Iniciar o Harness

`initialize_workspace`: a análise determinística e a criação, por projeto, até três por vez, cada falha isolada; um
projeto com um Harness válido é pulado sem ser lido de novo, então executar de novo depois de uma falha ou depois de
adicionar um projeto faz só o que falta. O progresso por projeto é o evento `harness:workspace-progress`; o relatório
só é `complete` se nenhum projeto pedido falhou. O contexto do workspace é reconstruído depois, dizendo quais projetos
não têm nenhum. A análise e a revisão assistidas por modelo continuam por projeto (os modais existentes, agora
endereçados por `projectId`).

## Consequências e limites

- O cenário "escreve em vários projetos em uma tarefa" não é uma execução (acima).
- A verificação de obsolescência de um projeto escaneia o projeto; a visão geral escaneia todo projeto a cada leitura.
- Os projetos consultados são informados como fragmentos de Harness; seus arquivos brutos só são legíveis por execuções
  do Claude sem ferramentas de edição.
- O escopo do workflow é por execução, não por passo.
