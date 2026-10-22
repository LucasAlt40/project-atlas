# Refatoração visual geral — relatório de implementação

Branch: `design/refatoracao-visual` (25 commits sobre a `main`, 184 arquivos). Nenhum Rust, DTO, comando Tauri ou
funcionalidade nova foi tocado. O mobile (`mobile.html`, `src/mobile`) ficou fora.

Capturas em `img/`: `<tela>-antes.png` (main) e `<tela>-depois.png` (branch), 1440×900, mesmos dados de exemplo.
Comparação com o Stitch: `../stitch/<tela>.png`.

## 1. Tela por tela

Legenda: **fiel**, **fiel com ajustes** (estrutura e hierarquia iguais, diferença pontual descrita), **divergente**.

| Tela                                            | Resultado         | Observação                                                                                                                                                                                                           |
| ----------------------------------------------- | ----------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1.1 Workspace — Lista                           | fiel com ajustes  | Cabeçalho, abas com ícone, cartão de agente e compositor compacto. A faixa de uso do dia ficou em mono no cabeçalho.                                                                                                 |
| 1.2 Workspace — Grade                           | fiel com ajustes  | Mesma moldura; cartões na grade seguem o novo cartão.                                                                                                                                                                |
| 1.4 Workspace — Detalhes                        | fiel              | Gaveta com abas Resumo / Permissões / Contexto / Consumo.                                                                                                                                                            |
| 1.5 Workspace — Execuções                       | fiel com ajustes  | Mestre-detalhe dentro do cartão. O inspetor mantém abas que o Stitch não desenhou (Contexto, Chat, Terminal).                                                                                                        |
| 1.6 Boas-vindas                                 | fiel com ajustes  | Mesmo sistema de design; passos e chamadas do app mantidos.                                                                                                                                                          |
| 2.1 Projetos e Harness                          | fiel com ajustes  | Virou página (não modal), com breadcrumb `Projetos / Harness`.                                                                                                                                                       |
| 2.2 / 2.4 Harness (analisar, revisar)           | fiel com ajustes  | Três passos (Analisar, Análise ao vivo, Revisar contexto); revisão agrupada por categoria com achado em uma linha.                                                                                                   |
| 3.1 Workflow — Editar                           | fiel com ajustes  | Canvas principal, barras de uma linha, ferramentas flutuantes, inspetor com seções recolhíveis. Sem estouro em 1440.                                                                                                 |
| 3.3 Novo workflow                               | fiel              | Três passos.                                                                                                                                                                                                         |
| 3.4 / 3.5 / 3.7 Aguardando, executando, revisão | fiel com ajustes  | Painel lateral "Ação necessária"; revisão com cartão de entrega (Manter/Descartar no ⋯). A captura 3.7 deste relatório mostra o editor porque o mock não força o estado "concluído" (a tela foi verificada à parte). |
| 4.1 Agentes — Lista                             | fiel              | Tabela.                                                                                                                                                                                                              |
| 4.2 Criar agente                                | fiel              | Uma coluna de 720px, sugestão da personalidade abaixo da escolha, contrato em cartões, rodapé fixo.                                                                                                                  |
| 4.3 Personalidades                              | fiel com ajustes  | Cartões resumidos em 3 colunas, expansão no próprio cartão. Os cartões não têm a mesma altura: só o aberto cresce.                                                                                                   |
| 4.4 Editar personalidade                        | fiel              | Gaveta de 560px.                                                                                                                                                                                                     |
| 5.1 Specs — Lista                               | fiel              | Linhas.                                                                                                                                                                                                              |
| 5.2 Specs — Documento                           | fiel com ajustes  | Passos numerados, barra "Sua decisão" fixa, validações na lateral. O documento tem a largura do app, não 72 caracteres.                                                                                              |
| 5.3 Specs — Revisão final                       | fiel com ajustes  | Seções em linhas de verificação. "Abrir" por verificação só aparece quando há bloqueio.                                                                                                                              |
| 5.4 Nova especificação                          | fiel com ajustes  | Modal de 640px com coluna única. Sem o campo "Projeto" (ver seção 2).                                                                                                                                                |
| 6.1 MCP Hub                                     | fiel com ajustes  | Cabeçalho, linha de números em mono, filtros segmentados, mestre-detalhe. "Testar" ficou no cartão.                                                                                                                  |
| 6.2 Adicionar servidor                          | fiel com ajustes  | Abas, nota, três cartões, rodapé "Fechar".                                                                                                                                                                           |
| 6.3 Sincronizar CLIs                            | **não conferida** | Só ganhou o modal novo (título e moldura). A captura mostra "Lendo os arquivos das CLIs…" (o mock não devolve o plano).                                                                                              |
| 7.1 Configurações — Geral                       | fiel              | Índice de 200px e coluna de 720px.                                                                                                                                                                                   |
| 7.2 / 7.3 Memória, Importar e exportar          | fiel com ajustes  | Mesma moldura; o conteúdo interno dos painéis de memória e portabilidade só recebeu os tokens novos.                                                                                                                 |
| 7.4 Celular                                     | fiel              | Coluna única, selo de estado, QR e código em cartão.                                                                                                                                                                 |

Telas e estados sem desenho (diálogos de spec, harness, erros, vazios) receberam o mesmo sistema (Badge, Button, Modal,
Callout, Tabs) sem redesenho próprio.

## 2. O Stitch mostra, o app não tem

- **Aprovar / Rejeitar** no painel "Ação necessária" (1.3): o componente `ApprovalRequests` existe, mas nunca é montado. Segui o código e não o liguei. **Decisão pendente.**
- **Campo "Projeto"** na nova especificação (5.4): o app grava a spec no projeto principal do workspace.
- **"Abrir"** em cada linha verde da revisão final (5.3).
- **"Servidor HTTP local ativo"**, **"Rede local"** e **"visto há 2 min"** no acesso pelo celular (7.4): o app não informa esses dados.
- **Placeholders de exemplo** em "Instruções adicionais" (4.2) e nas perguntas de contexto (5.4): invenção do Stitch, não implementada.
- **Chave "Remotos (HTTP/SSE)" com a dica "ainda não suportado"**: já existia (desabilitada quando não há servidor remoto); mantive a regra.

## 3. O Stitch omitiu, o app manteve

- Terminal do agente (movido para o menu ⋯ do cartão).
- Cancelar execução (movido para o ⋯ da barra de execução).
- Manter / Descartar a entrega (no ⋯ do cartão de entrega, escondido enquanto está ocupado).
- Testar servidor MCP no cartão; aviso de entrega por CLI (Antigravity) e "Copiar comando".
- Guardrails do MCP no inspetor; id do agente nos detalhes; contexto do projeto nos detalhes do agente.
- Integrações do MCP na nova spec, orçamento de tokens, Importar specs (agora no ⋯).
- Importar / Restaurar personalidades (no ⋯), exportar por cartão.
- Painel de Sandbox e Presença no Discord em Configurações (desenhados), além do botão "Como funciona" de cada um.
- Faixa de estatísticas e subtítulo da lista de agentes: **removidos**, porque o Stitch não os tem.

## 4. Divergências intencionais

1. **Código ganha do prompt** quando divergem: `ApprovalRequests` (acima); "Adicionar" no Hub abre em "Conhecidos do Atlas" (aba que o prompt pede ativa) — os testes passaram a escolher "Registro MCP" explicitamente.
2. **Seção Memória e Importar/exportar** mostram um painel por vez (índice), não rolagem contínua.
3. **Texto de "não disponível"** do cartão de Jira no Hub repete a frase do Figma (texto do app; não alterei o conteúdo, só a forma). Vale corrigir à parte.
4. **Terminologia**: Agente, Etapa, Passagem (handoff), Achado; restos em inglês do `pt-BR.ts` corrigidos.
5. **Sem caixa alta**: títulos e rótulos sempre em caixa normal.
6. **Botões "Mostrar mais / Ver mais"** nos cartões e no aviso de personalidades.

## 5. Componentes novos e tokens alterados

Novos em `src/components/ui`: `status` (13 estados e o mapa único estado→tom), `Tabs`, `SegmentedControl`, `Drawer`,
`Callout`, `Switch`, `PageHeader` (com `level`), `SectionNav`, `ActionBar`. Reescritos: `Badge`, `Button`, `OverflowMenu`,
`CollapsibleSection` (variantes card / flat / disclosure), `Modal` (tamanhos, título e rodapé), `Steps`, `Icon`, Markdown.
Fora de `ui`: `HeaderChip`, `BreadcrumbContext`, `ExecutionView`, `platform`.

Tokens em `src/styles/global.css`: grade de 4px (`--space-half … --space-6`), raios `--radius-xs … lg`, escala de fontes
(`--font-size-2xs … 2xl`), `--line-height-*`, `--measure-reading` e `--measure-form`, `--accent-active`, `--shadow-overlay`,
`--backdrop*`, `--rail-collapsed-width`, `--control-height(-sm)`. As cores antigas (`--color-danger/success/warning`)
continuam para o mobile.

Limpeza: 61 chaves de texto sem uso removidas nos dois idiomas e 150 regras CSS de classes sem referência.

## 6. Testes e `npm run check`

Testes de comportamento mantidos; seletores ajustados onde a interface mudou (menus ⋯, abas, rádios, tabela, gaveta, índice
de configurações), sem enfraquecer asserções. Foram acrescentados: aplicar a sugestão da personalidade, e a sugestão sem
botão quando já seguida.

Resultado de `npm run check` no fim do trabalho: **verde** — `tsc`, `eslint`, `prettier`, 788 testes do front, `cargo fmt`, `clippy -D warnings` e `cargo test`.

## 7. O que não deu para verificar / decisões pendentes

- Nenhum teste real com modelo pago foi executado (nenhum `--ignored`).
- **Estouro horizontal**: o verificador automático (720, 1024, 1280, 1440, 1920, 2560) não achou estouro nem texto cortado nas 23 telas capturadas. Telas só alcançáveis com dados reais (sincronização de CLIs, harness com projeto real) não foram exercitadas em todas as larguras.
- **6.3 Sincronizar CLIs** e **3.7 revisão** não foram comparadas contra o Stitch com dados.
- **Acessibilidade**: seguiu os papéis e rótulos dos testes; não houve auditoria com leitor de tela.
- **Decisões para você**: (1) ligar Aprovar/Rejeitar no painel "Ação necessária"; (2) campo "Projeto" na nova spec; (3) corrigir o texto "não disponível" do Jira no Hub; (4) "Abrir" por verificação na revisão final.
