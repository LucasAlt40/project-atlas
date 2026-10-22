# Design system da refatoração geral

Design system `assets/7ac197f798c544509257ebc1201068fc` do projeto Stitch `526454930665111857`, gerado pelo
prompt completo ([../prompt-stitch.md](../prompt-stitch.md)) e usado em todas as 32 telas desta pasta. É uma
evolução do "Atlas Engineering Core" anterior (`assets/7aa0774b734b4149b2d38a23136aadfd`), que é o que
`src/styles/global.css` espelha hoje. A diferença principal é a **escala de tipografia maior** (corpo de 14px
em vez de 13px).

O texto completo (`designMd`) sai de `mcp__stitch__list_design_systems` com o id do projeto. Abaixo, o que
importa para implementar.

## Cores (iguais às de hoje)

| Token          | Valor                    | Uso                                              |
| -------------- | ------------------------ | ------------------------------------------------ |
| Void           | `#0b0d10`                | fundo mais profundo, canvas, terminal            |
| Base           | `#101317`                | fundo da aplicação                               |
| Nível 1        | `#15191e`                | painéis, barra lateral, campos                   |
| Nível 2        | `#1c2128`                | cartões selecionáveis, abas ativas, botão secundário |
| Nível 3        | `#242c37`                | sobreposições, menus, dicas                      |
| Borda sutil    | `#242c37`                | divisões estáticas                               |
| Borda forte    | `#2d3748`                | controles, hover                                 |
| Borda de foco  | `#3b4554`                | foco de teclado                                  |
| Texto          | `#f1f5f9` / `#e2e8f0` / `#94a3b8` / `#64748b` | alto / padrão / secundário / apagado |
| Acento         | `#6366f1`, hover `#4f46e5`, ativo `#4338ca`, suave `#818cf8`, tinta 14% | ação principal, seleção |
| Semânticas     | sucesso `#10b981`, aviso `#f59e0b`, erro `#ef4444`, executando `#60a5fa`, info `#38bdf8`, política `#ec4899`, ocioso `#64748b` | estados |

## Tipografia (muda)

| Papel         | Fonte          | Tamanho / peso       | Antes (global.css)       |
| ------------- | -------------- | -------------------- | ------------------------ |
| headline-xl   | Geist          | 2rem / 600           | —                        |
| headline-lg   | Geist          | 1.5rem / 600         | `--font-size-xl` 1.375rem |
| headline-md   | Geist          | 1.25rem / 500        | —                        |
| headline-sm   | Geist          | 1.125rem / 500       | `--font-size-lg` 1.125rem |
| body-lg       | Geist          | 1rem / 400           | —                        |
| **body-md**   | Geist          | **0.875rem** / 400   | `--font-size-base` 0.8125rem |
| body-sm       | Geist          | 0.75rem / 400        | `--font-size-sm` 0.75rem |
| label-md      | Geist          | 0.8125rem / 500      | —                        |
| label-sm      | Geist          | 0.6875rem / 600      | —                        |
| code-lg / md / sm | JetBrains Mono | 0.875 / 0.8125 / 0.6875rem | —                |

Títulos negativos (`letter-spacing` -0.01 a -0.025em); números em mono com `tabular-nums`.

## Espaço, forma e profundidade

- Espaços: 0.125, 0.25, 0.5, 0.75, 1, 1.5, 2rem. Calha 0.5rem, margem 0.75rem.
- Raios: 0.125 (caixa de seleção), 0.25 (controles, selos, campos), 0.375 (contêineres), 0.5rem (modais e gavetas).
- Superfícies separadas por tom e borda de 1px. Sombra só em sobreposições:
  `0 12px 32px -4px rgb(0 0 0 / 0.65), 0 0 0 1px #2d3748`. Fundo de modal escurecido a 40% com desfoque de 12px.
- Seleção ativa: borda `#6366f1` (ou `#3b4554`) + tinta índigo a 14%, sem sombra.

## Componentes

- **Botões**: primário índigo; secundário Nível 2 + borda `#2d3748`; fantasma transparente, texto `#94a3b8`,
  hover com tinta índigo; destrutivo Nível 1 + borda e texto vermelhos.
- **Selos (chips)**: raio 0.25rem, fundo da cor a 14%, borda da cor a 40%, texto na cor; ponto de 6px nos
  selos de estado. É o formato que o Stitch desenhou em todas as telas (retangular, não pílula).
- **Campos**: Nível 1, borda `#242c37`, foco com borda `#6366f1` sem brilho; prefixos mono em `#64748b`.
- **Caixa de seleção e rádio**: 14px; marcados em `#6366f1`.
- **Cartões**: Nível 1 ou 2, borda sutil.
