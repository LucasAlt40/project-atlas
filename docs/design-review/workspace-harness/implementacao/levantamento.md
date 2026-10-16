# Levantamento — Stitch × app (módulo Workspace + Harness)

Fonte visual: HTML e PNG exportados do projeto Stitch `526454930665111857` (design system
"Atlas Engineering Core"), guardados em [`../stitch/`](../stitch/). Legenda: **existe** = o app tem o
dado ou a ação; **não existe** = o Stitch mostra, o app não tem (não implementado); **adaptado** =
o app tem algo equivalente com outro nome ou lugar.

## Moldura (fora do escopo do módulo, não alterada)

| Elemento do Stitch                                                                              | App                                                  |
| ----------------------------------------------------------------------------------------------- | ---------------------------------------------------- |
| Busca "Buscar agentes e comandos… ⌘K"                                                           | não existe                                           |
| Botões "Atlas CLI", ícones de terminal, notificações e ajustes no topo, avatar "operator@atlas" | não existe                                           |
| Selo "Sync OK / Sync Local"                                                                     | não existe                                           |
| Botão "Novo Fluxo" na barra lateral                                                             | não existe                                           |
| Telemetria na barra lateral (Cluster Online, latência, RPC, RAM, Region) e versão "v2.4.0"      | não existe                                           |
| Barra de status inferior ("MCP Host", "Sincronização de Contexto", documentação)                | não existe                                           |
| Breadcrumb "Atlas / Workspace / Tela"                                                           | adaptado: o app mostra "Workspace / Tela"; não mudou |

## 1 e 2. Workspace — Lista e Grade

| Stitch                                                                     | App                                                                    |                                                       |
| -------------------------------------------------------------------------- | ---------------------------------------------------------------------- | ----------------------------------------------------- |
| Título + chip "Hoje / Semana"                                              | `WorkspaceUsageChip`                                                   | existe                                                |
| "Integrações ↗", "Projetos (2)", "+ Adicionar agente" (ordem e peso)       | mesmos botões                                                          | existe                                                |
| Alternância Lista/Grade, "Recolher todos"                                  | existe                                                                 | existe                                                |
| Chips "1 agente em execução" e "backend · main"                            | —                                                                      | não existe                                            |
| Cartão: avatar, nome, selo, "Editar", ⋯, "Recolher"                        | edição, remoção, detalhes, recolher                                    | existe (⋯ guarda "Detalhes e permissões" e "Remover") |
| Faixa "Personalidade · Runtime · Provedor · Modelo · Isolamento Git"       | existe (era o cabeçalho)                                               | existe                                                |
| Abas com contagem "Execuções (3)"                                          | abas + histórico                                                       | existe                                                |
| Chat, aviso de pergunta pendente, compositor numa linha com escopo         | `AgentMessageList`, `ApprovalRequests`, `AgentComposer`, `ScopePicker` | existe                                                |
| Aba Atividade com cartões de métricas (SonarQube, acoplamento…)            | linha do tempo de eventos                                              | não existe (só a linha do tempo)                      |
| Célula vazia com "Adicionar agente", "Criar novo agente" e presets rápidos | célula "Disponível"                                                    | adaptado (ação sim, presets não existem)              |
| "Streaming tokens (48 t/s)"                                                | —                                                                      | não existe                                            |

## 3. Agente — Detalhes e permissões

| Stitch                                                                               | App                                             |                                       |
| ------------------------------------------------------------------------------------ | ----------------------------------------------- | ------------------------------------- |
| Selo de estado + id do agente, nome, linha mono                                      | `AgentDetailsPanel`                             | existe                                |
| Resumo "Consumo hoje"                                                                | uso do agente hoje                              | existe                                |
| Resumo "Perfil atual"                                                                | está em Permissões (seletor)                    | adaptado (não repetido no resumo)     |
| Seções recolhíveis: Permissões, Contexto do projeto, Última execução                 | existe                                          | existe                                |
| "Nível 2" no cabeçalho de Permissões                                                 | —                                               | não existe                            |
| Seletor de perfil + quatro cartões (arquivos, processos, Git, rede) com tag          | `AgentPermissions` + `PolicySummary`            | existe                                |
| Contexto do projeto: "Sincronizado", escopo principal, arquivos mapeados, diretrizes | estado do Harness e a nota de que é só contexto | adaptado (o resto não existe)         |
| Rodapé "Configurar regras" e "Salvar alterações"                                     | o perfil grava ao ser trocado                   | não existe                            |
| ⋮ no cabeçalho                                                                       | —                                               | não existe                            |
| Consumo, Cota, dados do agente                                                       | existem                                         | existe (mantidos, o Stitch os omitiu) |

## 4. Execução — Inspetor

| Stitch                                                                                             | App                   |                               |
| -------------------------------------------------------------------------------------------------- | --------------------- | ----------------------------- |
| Painel largo, selo "EXECUÇÃO #e1", título + tarefa, status, duração, custo                         | `ExecutionInspector`  | existe                        |
| Abas Detalhes, Contexto, Atividade (6), Chat, Terminal                                             | mesmas abas           | existe                        |
| Fatos em grade de 2 colunas + "Custo consolidado"                                                  | `ExecutionDetails`    | existe (custo é um dos fatos) |
| Linha do tempo à direita com "N eventos"                                                           | `AgentActivity`       | existe                        |
| "Exportar Log", "Reexecutar tarefa", "Ver diff no Git", "Abrir no Workspace", ⋯                    | —                     | não existe                    |
| "Sessão: cl-session-…", "Latência média de tool", "Resumo da Execução & Validação", "14/14 testes" | —                     | não existe                    |
| Rodapé "Cluster · Runtime v2.4.0 · Isolamento Git"                                                 | seção Git do inspetor | adaptado                      |

## 5. Projetos e Harness (página)

| Stitch                                                                                        | App                           |                                                |
| --------------------------------------------------------------------------------------------- | ----------------------------- | ---------------------------------------------- |
| Página com título, texto e "+ Adicionar pasta de projeto"                                     | era modal                     | existe (agora página)                          |
| Selo "Cluster 01-PROD", filtro "Todos os módulos"                                             | —                             | não existe                                     |
| Linha: nome, Principal, papel, estado, caminho, stack                                         | existe                        | existe                                         |
| "Saudável" / "Não analisado"                                                                  | estado do Harness **e** saúde | existe (o Stitch omitiu um dos dois; mantidos) |
| Barra de achados + legenda                                                                    | contagens (`HarnessStats`)    | existe (barra é só desenho das contagens)      |
| "Harness Sync: 100%", botão copiar caminho                                                    | —                             | não existe                                     |
| Última análise, "Usa:"                                                                        | existe                        | existe                                         |
| Atualizar, Recarregar, Editar, ⋯                                                              | + Resetar e Excluir no ⋯      | existe                                         |
| Cartão Harness (+ `.atlas/harness.lock`), Contexto do workspace, Notas ("Injetado no prompt") | existem (sem as tags)         | existe                                         |
| Rodapé "Harness Engine v1.8.2, Daemon, Memory, Sandbox"                                       | —                             | não existe                                     |

## 6. Harness — Analisar projeto

| Stitch                                                                              | App                  |                                          |
| ----------------------------------------------------------------------------------- | -------------------- | ---------------------------------------- |
| Indicador "1. Analisar projeto › 2. Revisar descobertas › 3. Inicializar Harness"   | —                    | adaptado (novo, em cima do mesmo estado) |
| "HARNESS SETUP", "ID: HRN-0924"                                                     | —                    | não existe                               |
| Projeto, caminho, selos ATLAS ENGINE e Local Core, aviso de leitura                 | existe               | existe                                   |
| Cartão "Análise com IA" + campo, contador de tokens, agente executor, modo restrito | `SemanticSection`    | existe                                   |
| Botão "Analisar com IA" dentro do cartão                                            | está no rodapé       | adaptado (não duplicado)                 |
| Cartão "Revisar sem IA" com 4 itens e "(0.04s)"                                     | —                    | adaptado (2 itens factuais, sem tempo)   |
| Diagnóstico inicial em três colunas                                                 | existe               | existe                                   |
| Rodapé "Cancelar, Revisar manualmente, Analisar com IA ↵"                           | existe, sem o atalho | existe                                   |

## 7. Harness — Revisar achados

| Stitch                                                                                                  | App                          |                            |
| ------------------------------------------------------------------------------------------------------- | ---------------------------- | -------------------------- |
| Etapas no topo, título "Harness — Revisar achados", Voltar / Inicializar                                | existe                       | existe                     |
| Quatro métricas, abas por área com contagem                                                             | existe                       | existe                     |
| Cartões de achado com selos e botão Evidência                                                           | `FindingRow` (lista própria) | adaptado (não redesenhada) |
| Inspetor de evidência à direita                                                                         | existe                       | existe                     |
| "Salvar rascunho", "Filtrar manifestos…", "Marcar todos", "Sync OK", "AST Parser v4.1", "RAM cognitiva" | —                            | não existe                 |
| Rodapé: estado do Harness + "Não enviar o .atlas/ ao Git"                                               | existe                       | existe                     |

## 8a. Boas-vindas

| Stitch                                                                                                        | App                      |                                                |
| ------------------------------------------------------------------------------------------------------------- | ------------------------ | ---------------------------------------------- |
| Cartão com título, frase e três passos numerados                                                              | só título, frase e botão | adaptado (passos escritos com o que o app faz) |
| Rótulo "Pronto para orquestrar", rodapés dos passos, "ou abra pelo terminal com atlas init", rodapé da página | —                        | não existe                                     |
| "Criar workspace →"                                                                                           | existe                   | existe                                         |

## 8b. Seletor de workspace

| Stitch                                                | App                                |                                                         |
| ----------------------------------------------------- | ---------------------------------- | ------------------------------------------------------- |
| Lista com ícone, nome, "Ativo", caminho, visto        | existe                             | existe                                                  |
| "2 de 5 livres", busca "Buscar workspace…", atalho ⌘N | sem limite, sem filtro, sem atalho | não existe                                              |
| "Novo workspace", "Gerenciar workspaces →"            | existe                             | existe                                                  |
| Fundo desfocado                                       | —                                  | não implementado (o app já escurece o fundo dos modais) |
