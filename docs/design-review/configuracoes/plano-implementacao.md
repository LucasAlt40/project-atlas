# Plano de implementação — Repaginação de Configurações

Branch `design/configuracoes`, criada a partir de `design/workspace-harness` (ou de `main`, se essa
já tiver sido integrada; a base compartilhada `Badge`, `CollapsibleSection`, `OverflowMenu`, `Steps`
precisa existir). Princípios, fases, fonte do visual, relatório e regras de divergência:
[`../_comum/principios-implementacao.md`](../_comum/principios-implementacao.md). Dossiê:
[`README.md`](README.md).

## 1. Telas e código

Ids das telas geradas no Stitch (registro e avaliação em `prompts.md`). As cinco telas são seções da
**mesma página** (`SettingsPage`); o app continua tendo uma tela só, com um índice interno.

| #   | Tela (nome no Stitch)                | Id da tela nova | Código do app (hipótese a confirmar)                                                                    |
| --- | ------------------------------------ | --------------- | ------------------------------------------------------------------------------------------------------- |
| C1  | Configurações — Geral                | `67ed2d6872a94adc9838d074b6fab48e`   | `settings/pages/SettingsPage.tsx`, `settings/components/SandboxSettings.tsx`, `SettingsPage.module.css` |
| C2  | Configurações — Memória              | `531ba78a90f042819c5788c9f9a07fac`   | `memory/components/MemorySettings.tsx`, `Memory.module.css`, `memory/hooks/useMemories`                 |
| C3  | Configurações — Importar e exportar  | `f51323b94a09433fb8f5a56733ff2fee`   | `portability/components/PortabilitySettings.tsx`, `PortabilityActions.tsx`, `ExportDialog.tsx`          |
| C4  | Configurações — Revisar importação   | `34439c29d5234a84aa9bea57546f8c17`   | `portability/components/ImportDialog.tsx`, `ImportItemCard.tsx`, `ImportResult.tsx`, `ModelPanel.tsx`   |
| C5  | Configurações — Sobre e atualizações | `7c2b416caa5d408c974bc972850cc9c3`   | `home/components/AppInfoPanel.tsx`, `settings/components/UpdateChecker.tsx`                             |

Compartilhados com outros módulos: `PortabilityActions` / `ImportButton` / o diálogo de exportar são
usados também em **Agentes** e **Personalidades** (botão `Importar`, `Exportar` no cartão) e no
Workflow. `LanguageSwitch` da barra superior e `SettingsProvider` não mudam. Textos em `settings.*`,
`memory.*`, `portability.*` de `pt-BR.ts` e `en-US.ts`.

## 2. Decisões já tomadas para este módulo

- **Uma página, índice interno**: o índice lateral (Geral, Memória, Importar e exportar, Sobre e
  atualizações) rola até a seção e marca a atual; não é roteamento novo nem nova tela do `SCREENS`. Em
  janela estreita o índice vira uma faixa de âncoras no topo. O conteúdo tem largura máxima
  (~880 px) e se centraliza ou alinha como o Stitch mostrar.
- **Controle primeiro, ajuda depois**: onde o app hoje põe o parágrafo antes do controle, o controle vem
  primeiro e a ajuda embaixo. **Nenhum texto de ajuda é removido**; os longos (sandbox, modos de
  memória) vão para uma `CollapsibleSection` ("Como funciona") fechada.
- **Memória separada em configuração e conteúdo**: modos e limite de um lado, propostas, guardadas e
  formulário de outro (abas ou seções, como o Stitch desenhar). Todas as ações continuam: aprovar,
  descartar, editar, arquivar, apagar, apagar as automáticas, adicionar.
- **Selos de proveniência**: um `Badge` curto ("Não confirmada") com o detalhe completo acessível
  (`title` e texto para leitor de tela). O texto atual não pode ser perdido.
- **Ações por nota**: `Aprovar` e `Descartar` à vista nas propostas; nas guardadas, `Editar` à vista e
  `Arquivar`/`Apagar` no `OverflowMenu` (`Apagar` com `danger`). A confirmação atual de apagar, se
  existir, continua.
- **Importar/exportar**: continua o mesmo comportamento (exportar vários expande na página e abre o
  diálogo de exportar; importar abre o modal). Só a aparência e o agrupamento mudam. **Não unificar o
  fluxo** nesta rodada.
- **Salvo**: onde o app grava ao mudar (idioma, modos), pode haver um sinal discreto de "salvo" **se o
  Stitch o mostrar e o app tiver o estado**; sem estado, não inventar.

## 3. Cuidados específicos

- Os testes `SettingsPage.test.tsx`, `SandboxSettings.test.tsx`, `MemorySettings.test.tsx`,
  `ExportDialog.test.tsx`, `ImportDialog.test.tsx` procuram por papéis e nomes acessíveis: mantê-los;
  se um controle mudar de lugar (aba da Memória), ajustar o caminho, não a asserção, e justificar.
- O formulário do limite de tokens é um `<form>` com `onSubmit`; manter (Enter envia).
- O limite de 480 caracteres e o contador da nota continuam.
- `ImportDialog` tem estados `pick`, `review` (com escolhas por item) e `done`; o `ModelPanel` escolhe
  runtime e modelo. Não mexer na lógica de decisões.
- Dados longos: nome de nota de 80 caracteres, 50 notas, 20 itens importados, motivos de falha longos.

## 4. Fases

| Fase | Entrega                                                                                                                                                                                                        |
| ---- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 0    | Preparação (ver `_comum`), capturas "antes" com `?memory=some` e `?memory=none`                                                                                                                                |
| 1    | `implementacao/levantamento.md` + exportação do Stitch para `stitch/`                                                                                                                                          |
| 2    | Base compartilhada: layout de página com índice interno, grupo de controle com ajuda, seção recolhível de ajuda                                                                                                |
| 3    | C1 (Geral) e C5 (Sobre e atualizações)                                                                                                                                                                         |
| 4    | C2 (Memória)                                                                                                                                                                                                   |
| 5    | C3 e C4 (Importar e exportar; revisar importação) — conferir o uso em Agentes, Personalidades e Workflow                                                                                                       |
| 6    | Responsividade 720–2560 (índice como faixa abaixo de ~900 px) e estados (memória vazia, sandbox indisponível, limite inválido, atualização disponível e instalando, importação com conflitos e itens externos) |
| 7    | Testes, `npm run check`, relatório                                                                                                                                                                             |

## 5. Captura

`_comum/captura/` com `?memory=none|some`. Para sandbox indisponível, atualização disponível e a revisão
da importação, estender o mock em `tmp/` (`get_sandbox_status` com `available:false`, `check_for_update`
com `{version, notes}`, `pick_config_file`, `preview_config_import`, `import_configs`).

## 6. Fora do escopo

Lógica de memória, sandbox, importação e atualização; Rust, DTOs, comandos; a troca de idioma da barra
superior; o chip de uso do Workspace; os outros módulos.
