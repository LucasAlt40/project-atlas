# Princípios de implementação (valem para todo módulo)

Cada `plano-implementacao.md` de módulo parte destes princípios e só acrescenta o que é específico
do módulo (telas, código, fases, riscos).

## 1. Regras

1. **O Stitch é a referência visual, não a especificação funcional.** O app é a fonte da verdade do
   que existe (dados, ações, estados, regras, textos de erro). Se o Stitch mostrar algo que o app
   não faz (campo, botão, métrica, filtro, atalho, tela, texto de estado inexistente), **não
   implementar**; listar em "O Stitch mostra, o app não tem".
2. **Máxima fidelidade ao que o Stitch mostra e o app suporta**: estrutura, hierarquia, agrupamento,
   espaçamento, tipografia, cor, ordem, rótulos e nomes de tela. Divergir só com motivo escrito.
3. **Nada de informação some.** Se o Stitch omitiu um dado que o app mostra, manter o dado no lugar
   mais próximo do padrão do Stitch e listar em "Stitch omitiu, app manteve".
4. **Funcionalidade intacta**: mesmos comandos Tauri, estados, regras e testes de comportamento.
   Trocar modal por página ou por painel largo é apresentação; estado e hooks continuam.
5. **Responsividade desktop de 720 a 2560 px** (a janela abre em 1100×720; mínimo 720): perfeito de
   1100 para cima, degradando com elegância abaixo (barra lateral recolhida, colunas empilhadas).
   Sem rolagem horizontal da página, sem texto cortado sem reticências, colunas que respondem ao
   container. Alturas ≥ 700 não escondem ação primária. Verificar com `chk.js` em 720, 1100, 1280,
   1440, 1920 e 2560.
6. **Identidade preservada**: tema escuro único, tokens de `src/styles/global.css`, Geist e
   JetBrains Mono, 1px de borda em vez de sombra (só overlay tem sombra), raios 4/6/8, `Button` com
   `variant`. Token novo só se o valor se repetir e não existir.
7. **i18n**: todo texto em `src/i18n/pt-BR.ts` **e** `en-US.ts`; nenhuma string fixa no JSX.
8. **Escopo fechado**: só o módulo e os compartilhados de que ele depende; mudou um compartilhado,
   confirmar as outras telas e listar no relatório. Sem Rust, sem DTOs, sem nomes de comandos.

## 2. Base compartilhada que já existe (branch `design/workspace-harness`)

Reaproveitar, não recriar. Antes de criar um componente novo, procurar aqui:

| Componente                                                                  | Para quê                                                      |
| --------------------------------------------------------------------------- | ------------------------------------------------------------- |
| `Badge` (`tone`: neutral/accent/success/warning/danger/info; `mono`; `dot`) | o selo único do app: papéis, estados, saúde, tipos            |
| `CollapsibleSection` (`title`, `aside`, `defaultOpen`)                      | blocos longos que dobram até o cabeçalho                      |
| `OverflowMenu` (`items` com `danger`)                                       | o "⋯" de uma linha ou cartão: ações secundárias e destrutivas |
| `Steps` (`done`/`current`/`todo`)                                           | fluxo numerado numa página só                                 |
| `Modal` com tamanho largo                                                   | quando um modal continua sendo o certo                        |
| `Button` `variant` primary/secondary/ghost                                  | sempre                                                        |

**Ponto de partida do código**: criar a branch `design/<módulo>` a partir de `design/workspace-harness`
(ou de `main`, se essa branch já tiver sido integrada). Sem essa base os componentes acima não existem.

## 3. Como obter o visual do Stitch

1. `mcp__stitch__get_screen` em `projects/526454930665111857/screens/<id>` e baixar o HTML
   (`htmlCode.downloadUrl`) e a imagem (`screenshot.downloadUrl`). Se o download der 401, renderizar o
   HTML no navegador e, se também não der, pedir as imagens ao usuário. Nunca implementar de memória.
2. Exportar para `docs/design-review/<módulo>/stitch/` com nomes numerados (`01-nome.html`,
   `01-nome.png`) e o `DESIGN.md` do design system, como em `workspace-harness/stitch/`.
3. O HTML do Stitch é Tailwind: converter para os tokens do app. O design system é
   `assets/7aa0774b734b4149b2d38a23136aadfd` ("Atlas Engineering Core"); `global.css` já o espelha.

## 4. Fases de cada módulo

| Fase | Entrega                                                                                                                              | Verificação                                     |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------- |
| 0    | Branch, `npm ci`, `npm run check` verde (linha de base), capturas "antes"                                                            | linha de base registrada                        |
| 1    | `implementacao/levantamento.md`: por tela, elemento do Stitch → dado/ação do app → existe / não existe / adaptado; exportar o Stitch | levantamento revisável                          |
| 2    | Base compartilhada: só o que faltar (outros componentes entram aqui)                                                                 | testes dos componentes; outros módulos intactos |
| 3..n | Telas do módulo, uma fase por grupo de telas                                                                                         | captura lado a lado com o Stitch                |
| n+1  | Responsividade 720–2560 e estados (vazio, carregando, erro, conteúdo longo)                                                          | `chk.js` limpo                                  |
| n+2  | Testes ajustados ao DOM novo sem enfraquecer asserções; `npm run check` verde; relatório                                             | `implementacao/relatorio.md`                    |

Commits pequenos em português no estilo do histórico (`feat(<módulo>): ...`, `refactor(ui): ...`,
`test(...)`, `docs(design): ...`) com o `Co-Authored-By` da sessão. Sem push, sem PR, sem mexer na
`main`.

## 5. Relatório final (todo módulo)

`implementacao/relatorio.md`: (1) por tela fiel / fiel com ajustes / divergente, com a imagem do
app ao lado da do Stitch; (2) "O Stitch mostra, o app não tem"; (3) "Stitch omitiu, app manteve";
(4) divergências intencionais; (5) mudanças em compartilhados e telas de outros módulos afetadas;
(6) `npm run check` e testes alterados; (7) pendências e sugestões para o módulo seguinte.

## 6. Quando divergir do Stitch

(a) pede dado ou ação que o app não tem; (b) o texto contradiz o comportamento real; (c) o layout
quebra com conteúdo real (nomes longos, muitos itens); (d) perderia acessibilidade (foco, contraste,
teclado, `aria-*`); (e) a largura de 2560 não se traduz para a janela real. O ajuste segue o espírito
do Stitch e fica registrado.

## 7. Fora do escopo de qualquer módulo

Código Rust, DTOs, nomes de comandos Tauri, funcionalidade nova, novos idiomas, módulos que não são o
do plano.
