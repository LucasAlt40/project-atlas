# Plano de implementação — Repaginação de Agentes e Personalidades

Branch `design/agentes-personalidades`, criada a partir de `design/workspace-harness` (ou de `main`,
se essa já tiver sido integrada: a base compartilhada `Badge`, `CollapsibleSection`,
`OverflowMenu` e `Steps` precisa existir). Princípios, fases, fonte do visual, relatório e regras de
divergência: [`../_comum/principios-implementacao.md`](../_comum/principios-implementacao.md).
Este plano só acrescenta o que é específico do módulo. Dossiê: [`README.md`](README.md).

## 1. Telas e código

Ids a preencher depois do envio dos prompts (registro em `prompts.md`).

| #   | Tela (nome no Stitch)                        | Id da tela nova | Código do app (hipótese a confirmar)                                                                         |
| --- | -------------------------------------------- | --------------- | ------------------------------------------------------------------------------------------------------------ |
| A1  | Agentes — Lista de agentes                   | _a preencher_   | `agents/pages/AgentsPage.tsx`, `agents/components/AgentList.tsx`, `agents/pages/Pages.module.css`            |
| A2  | Agentes — Sem agentes                        | _a preencher_   | `AgentsPage.tsx` (ramo sem agentes)                                                                          |
| A3  | Agentes — Criar e editar agente              | _a preencher_   | `AgentForm.tsx`, `RuntimePicker.tsx`, `ResultContractEditor.tsx`, `AgentsPage.tsx` (cabeçalho do formulário) |
| P1  | Personalidades — Biblioteca                  | _a preencher_   | `agents/pages/PersonalitiesPage.tsx`, `PersonalityBrowser.tsx`, `PersonalityCard.tsx`                        |
| P2  | Personalidades — Nova e editar personalidade | _a preencher_   | `PersonalityForm.tsx`, o painel dentro de `PersonalityBrowser.tsx`                                           |

Entradas: `src/app/AgentsScreen.tsx`, `src/app/PersonalitiesScreen.tsx`. Textos em
`agents.*`, `runtime.*`, `personalities.*` de `pt-BR.ts` e `en-US.ts`.

## 2. Decisões já tomadas para este módulo

- **Resumo**: o Stitch deve trazer uma linha só em vez de quatro caixas. Os quatro números
  (`agents.stat.*`) **continuam existindo** e aparecem nessa linha; não remover dado.
- **Ações do cartão de agente**: `Editar` à vista; `Exportar` e `Excluir` no `OverflowMenu`
  (`Excluir` com `danger`). A confirmação inline de exclusão continua no cartão e o erro "agente
  trabalhando" continua visível. Registrar o clique a mais no relatório.
- **Selos**: usar `Badge` (tons) para personalidade, disponibilidade, `Padrão`/`Personalizada`.
- **Formulário**: mesma estrutura de campos e mesmas regras (sugestão de contrato e de perfil pela
  personalidade, descoberta de runtimes, modelo livre ou de lista, validação). O que muda é a
  organização visual; nenhuma regra de `AgentForm` é tocada.
- **Aviso global de personalidades**: vira uma `CollapsibleSection` (ou equivalente) fechada; o texto
  completo continua acessível.
- **Cartão de personalidade**: compacto fechado, completo aberto (um aberto por vez). Todo o conteúdo de
  hoje (descrição, comportamento, instruções, tags, ações) continua acessível ao abrir.
- **Painel de personalidade**: painel lateral sobreposto em vez de coluna da grade (a grade não
  encolhe). A nota de personalidade nativa não editável continua.

## 3. Cuidados específicos

- `AgentForm` é usado também por `workspace/components/EditAgentPanel.tsx`. Depois de mudar o
  formulário, abrir "editar agente" pelo Workspace e conferir que continua correto (e que o
  Workspace, já repaginado no módulo 1, não quebrou).
- `ImportButton` e o diálogo de exportar vêm de `features/portability` (módulo Configurações): não
  repaginar aqui; só garantir que o botão `Importar` continua alinhado ao novo cabeçalho.
- `useCatalog` (agentes, personalidades, runtimes) não muda.
- Os testes `AgentsPage.test.tsx` e `PersonalitiesPage.test.tsx` procuram elementos por nome acessível:
  manter `aria-label`s e papéis; se um rótulo de texto substituir um ícone, o nome acessível deve
  ser o mesmo ou os testes mudam com justificativa.

## 4. Fases

| Fase | Entrega                                                                                                                                                                      |
| ---- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 0    | Preparação (ver `_comum`), capturas "antes" de A1, A3, P1 com `?custom=1&runtimes=down`, e A2 com `?agents=0`                                                                |
| 1    | `implementacao/levantamento.md` + exportação do Stitch para `stitch/`                                                                                                        |
| 2    | Base compartilhada: só o que faltar (por exemplo, painel lateral sobreposto reutilizável se o do módulo 1 não servir)                                                        |
| 3    | A1 e A2 (lista e vazio)                                                                                                                                                      |
| 4    | A3 (formulário)                                                                                                                                                              |
| 5    | P1 e P2 (biblioteca e painel)                                                                                                                                                |
| 6    | Responsividade 720–2560 e estados (lista com nomes longos, 12 agentes, 0 agentes, runtime indisponível, busca sem resultado, erro do núcleo, formulário com todos os avisos) |
| 7    | Testes, `npm run check`, relatório                                                                                                                                           |

## 5. Captura

`_comum/captura/` com `?agents=0..3`, `?custom=1`, `?runtimes=down`. Para estados de erro e de
edição, estender o mock em `tmp/`.

## 6. Fora do escopo deste módulo

Diálogos de importar e exportar (Configurações), painel de editar agente do Workspace além de
garantir que não quebra, Workflow, Specs, MCP Hub, Rust e DTOs.
