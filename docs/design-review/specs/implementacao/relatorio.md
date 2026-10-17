# Relatório — repaginação de Specs

Branch `design/workspace-harness` (continuação). Referência: [`../stitch/`](../stitch/). Levantamento: [`levantamento.md`](levantamento.md). Capturas a 1440×900 com `_comum/captura/` (`?specs=some|none`); em S5 acrescentei uma pergunta pendente à spec `s3` do mock.

## 1. Por tela

### S1. Specs — Lista de especificações — fiel com ajustes

Sem trilha nem contadores; abas com contagem e busca na mesma linha; selos pelo `Badge`.

| Antes                           | Stitch                            | Depois                       |
| ------------------------------- | --------------------------------- | ---------------------------- |
| ![Antes](../atual/01-lista.jpg) | ![Stitch](../stitch/01-lista.png) | ![App](img/app-01-lista.png) |

### S2. Specs — Sem especificações — fiel

Só o cartão central tem as ações; o cabeçalho as mostra quando há specs.

| Antes                              | Stitch                                | Depois                           |
| ---------------------------------- | ------------------------------------- | -------------------------------- |
| _não capturado (lacuna do dossiê)_ | ![Stitch](../stitch/02-sem-specs.png) | ![App](img/app-02-sem-specs.png) |

### S3. Spec — Documento em revisão — fiel com ajustes

Cabeçalho com ⋯, "Sua decisão" logo abaixo do stepper, coluna lateral de seções recolhíveis. O documento mantém a renderização do app.

| Antes                                                   | Stitch                                           | Depois                                      |
| ------------------------------------------------------- | ------------------------------------------------ | ------------------------------------------- |
| ![Antes](../atual/02-detalhe-requisitos-em-revisao.jpg) | ![Stitch](../stitch/03-documento-em-revisao.png) | ![App](img/app-03-documento-em-revisao.png) |

### S4. Spec — Agente escrevendo — fiel com ajustes

Faixa de execução e coluna lateral como em S3. O "antes" é a etapa pronta para escrever (não havia captura da etapa escrevendo).

| Antes                                                      | Stitch                                        | Depois                                   |
| ---------------------------------------------------------- | --------------------------------------------- | ---------------------------------------- |
| ![Antes](../atual/04-detalhe-requisitos-para-escrever.jpg) | ![Stitch](../stitch/04-agente-escrevendo.png) | ![App](img/app-04-agente-escrevendo.png) |

### S5. Spec — Perguntas e achados — fiel com ajustes

Cartão da pergunta com barra de acento e a pergunta em bloco. Os achados continuam na revisão final, onde a lista e o painel de leitura já seguiam o Stitch. Capturada com uma pergunta acrescentada ao mock.

| Antes                              | Stitch                                        | Depois                                   |
| ---------------------------------- | --------------------------------------------- | ---------------------------------------- |
| _não capturado (lacuna do dossiê)_ | ![Stitch](../stitch/05-perguntas-achados.png) | ![App](img/app-05-perguntas-achados.png) |

### S6. Spec — Revisão final e publicação — fiel com ajustes

Veredito e "Aprovar a especificação" no topo, sete verificações em linhas, quatro seções recolhíveis com selo de estado.

| Antes                                                  | Stitch                                    | Depois                               |
| ------------------------------------------------------ | ----------------------------------------- | ------------------------------------ |
| ![Antes](../atual/03-detalhe-revisao-final-pronta.jpg) | ![Stitch](../stitch/06-revisao-final.png) | ![App](img/app-06-revisao-final.png) |

### S7. Specs — Nova especificação — fiel com ajustes

Modal largo em duas colunas; corpo rola por dentro só se não couber.

| Antes                                        | Stitch                                | Depois                           |
| -------------------------------------------- | ------------------------------------- | -------------------------------- |
| ![Antes](../atual/05-nova-especificacao.jpg) | ![Stitch](../stitch/07-nova-spec.png) | ![App](img/app-07-nova-spec.png) |

### S8. Specs — Importar do projeto — fiel

O modal já tinha a estrutura do Stitch; não foi alterado.

| Antes                                    | Stitch                               | Depois                          |
| ---------------------------------------- | ------------------------------------ | ------------------------------- |
| ![Antes](../atual/06-importar-specs.jpg) | ![Stitch](../stitch/08-importar.png) | ![App](img/app-08-importar.png) |

## 2. O Stitch mostra, o app não tem (não implementado)

- Lista: rótulo "Aprovação técnica"; pedidos, agentes, ids e etapas de exemplo; "+ +" no botão.
- Documento (S3): o conteúdo em blocos (chip "ADDED", cartão por requisito, cenário com WHEN/THEN); títulos em caixa alta; ícones.
- S4: "transmissão em tempo real", "Markdown", selo "Aguardando", "—" no histórico, percentual na etapa.
- S5: "Requer decisão", "PERGUNTA DO AGENTE", placeholder do campo, "⌘ + Enter", seta no botão.
- S6: "Abrir" em cada verificação já satisfeita (seria um atalho novo).
- S7: "Obrigatório", cadeado e ponto verde no Projeto, selo "Sempre perguntado", placeholders e ícones.
- S8: "O QUE FOI ENCONTRADO", "3 itens selecionados", selo "Completo", pontos verdes.
- Moldura: "Core", "Novo Fluxo", "Configurações" no rodapé (S7, S5, S2), ponto de estado no rodapé.

## 3. Stitch omitiu, app manteve

- Todo o conteúdo dos painéis que ele só desenhou fechados: campos, botões e textos de Revisão pelo agente revisor, Publicar, Implementar e OpenSpec CLI (as regras de habilitação continuam as mesmas).
- Consumo (tokens, custo, limite): o alerta de limite atingido e o "Continuar (+N)" ficam no topo da página; os números ficam na coluna lateral.
- Feedback registrado do documento, consistência entre documentos, tarefas por grupo, requisitos e estados do painel de tarefas.
- A explicação "Nada roda até você pedir" e as razões de uma ação desabilitada ao lado da decisão.

## 4. Divergências intencionais

- **Sem trilha e sem contadores no cabeçalho** (o plano permitia, o Stitch os omitiu): os números estão nas abas.
- **Excluir no ⋯**, e o menu some quando a spec não pode ser excluída (ocupada ou com implementações), em vez de um botão desabilitado.
- **"Sua decisão" no topo**, mas em página estreita (< ~62 rem) ela volta para o rodapé fixo para não sair da vista.
- **Seções da revisão final recolhidas por padrão**, mas abrem sozinhas se houver achado em aberto, revisão rodando ou publicação com falha; o selo de estado fica sempre à vista.
- **Verificações sem "Abrir"** nas já satisfeitas.

## 5. Componentes compartilhados alterados

| Mudança                                                            | Quem mais usa | Verificação |
| ------------------------------------------------------------------ | ------------- | ----------- |
| `StatusBadge` de Specs agora usa o `Badge` (mapa único tom → selo) | só Specs      | testes      |
| `PanelSection` (novo, sobre `CollapsibleSection`)                  | só Specs      | testes      |
| Nenhum componente de `src/components/ui` foi alterado              | —             | —           |

Removidas as chaves `specs.crumb`, `specs.counters` e `specs.counter.*` (sem uso).

## 6. `npm run check` e testes alterados

- Final: ver o fim deste arquivo.

**Testes alterados (mesmas asserções, caminho novo):** em `SpecsPage.test.tsx`, os contadores soltos viraram as contagens das abas (`Waiting for you 1`, `Ready 1`); as seções da revisão final, da publicação, do OpenSpec e "Implementar" são abertas por um auxiliar (`openPanel`) antes de agir.

## 7. Pendências e sugestões

- Estados não capturados: limite de tokens atingido, falha, desatualizado, bloqueado, achados e publicação nos estados publicada/falhou, 30+ specs (cobertos pelos testes e pelas regras de abertura automática).
- Responsividade: `chk.js` limpo em 720, 1100, 1280, 1440, 1920 e 2560 para lista, vazio, S3 a S6 e os dois modais.
- Próximo módulo (Celular): a página do celular não tem a moldura do desktop; usar 390 px.

- Final: **verde** — typecheck, lint, Prettier, 70 arquivos / 777 testes, `cargo fmt`, `clippy` e `cargo test`. Rust, DTOs e comandos intocados.
