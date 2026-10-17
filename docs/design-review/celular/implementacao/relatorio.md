# Relatório — repaginação do Celular

Branch `design/workspace-harness` (continuação). Referência: [`../stitch/`](../stitch/). Levantamento: [`levantamento.md`](levantamento.md). Capturas da página do celular a 390 px (em iframe de 390 px sobre `_comum/captura/mobile.html`, `?phase=ok|unpaired|offline&runs=0..4`, `#/run/run-1`) e do diálogo a 1440×1000 (`?s=remote`). O "antes" são as capturas do dossiê em [`../atual/`](../atual/).

## 1. Por tela

### M1. Painel — fiel com ajustes

Cartão com duas linhas de contexto (workspace e tarefa) e rodapé com avanço e tempo; seções e faixa de alerta como estavam.

| Antes                                    | Stitch                             | Depois                        |
| ---------------------------------------- | ---------------------------------- | ----------------------------- |
| ![Antes](../atual/01-celular-painel.jpg) | ![Stitch](../stitch/01-painel.png) | ![App](img/depois-painel.png) |

A 360 px: ![360](img/depois-painel-360.png)

### M2. Execução esperando resposta — fiel com ajustes

"Tarefa" recolhível, barra de ações fixa no pé, nota opcional recolhida (só no plano), Etapas e Controlar recolhidos.

| Antes                                            | Stitch                                         | Depois                          |
| ------------------------------------------------ | ---------------------------------------------- | ------------------------------- |
| ![Antes](../atual/03-celular-execucao-plano.jpg) | ![Stitch](../stitch/02-esperando-resposta.png) | ![App](img/depois-execucao.png) |

A 430 px: ![430](img/depois-execucao-430.png)

### M3. Etapas e resultado — fiel com ajustes

Resultado, Código e Registro viraram painéis recolhíveis (`Fold`); o Código leva o resumo na linha. Sem a faixa "espera você" (o cartão da pergunta já diz isso). Capturado com a execução aberta a partir do painel; as etapas aparecem ao abrir o painel.

| Antes                                             | Stitch                                       | Depois                          |
| ------------------------------------------------- | -------------------------------------------- | ------------------------------- |
| ![Antes](../atual/04-celular-execucao-etapas.jpg) | ![Stitch](../stitch/03-etapas-resultado.png) | ![App](img/depois-execucao.png) |

### M4. Conectar — fiel com ajustes

Passos numerados em quadros mono, erro em faixa e botão de largura total fixo no pé (acompanha o fim do formulário quando a tela é alta).

| Antes                                    | Stitch                               | Depois                          |
| ---------------------------------------- | ------------------------------------ | ------------------------------- |
| ![Antes](../atual/05-celular-parear.jpg) | ![Stitch](../stitch/04-conectar.png) | ![App](img/depois-conectar.png) |

### M5a. Sem conexão — fiel

| Antes                                         | Stitch                                   | Depois                         |
| --------------------------------------------- | ---------------------------------------- | ------------------------------ |
| ![Antes](../atual/06-celular-sem-conexao.jpg) | ![Stitch](../stitch/05a-sem-conexao.png) | ![App](img/depois-offline.png) |

### M5b. Sem execuções — fiel

| Antes                                           | Stitch                                     | Depois                       |
| ----------------------------------------------- | ------------------------------------------ | ---------------------------- |
| ![Antes](../atual/07-celular-sem-execucoes.jpg) | ![Stitch](../stitch/05b-sem-execucoes.png) | ![App](img/depois-vazio.png) |

### D1. Acesso pelo celular (desktop) — fiel com ajustes

Modal largo em duas colunas (conectar / celulares pareados) que empilham em largura menor; painéis com borda; erro em faixa.

| Antes                                                 | Stitch                                     | Depois                                |
| ----------------------------------------------------- | ------------------------------------------ | ------------------------------------- |
| ![Antes](../atual/08-desktop-acesso-pelo-celular.jpg) | ![Stitch](../stitch/06-desktop-acesso.png) | ![App](img/depois-desktop-acesso.png) |

A 720 px: ![720](img/depois-desktop-acesso-720.png) · desligado: ![desligado](img/depois-desktop-desligado.png)

## 2. O Stitch mostra, o app não tem (não implementado)

- M3: faixa "Developer espera você" com seta.
- D1: "Local Token Auth", "256-bit", "Sub-rede", "Permissão: Responder perguntas e monitoramento", "Agente: Invoice OCR Engine", contagem e ponto no aparelho, botão "Concluído".
- Cartões de exemplo ("Auditoria de conciliação…", "1h 00m"), ícones e ilustrações dos estados vazios.

## 3. Stitch omitiu, app manteve

- Tudo o que o ADR 0034 permite ao celular: responder pergunta de texto e de permissão, pausar, retomar e cancelar, resultado e código em leitura. Nada foi acrescentado (não inicia execução, não concede permissão do computador, não aplica código, não mostra terminal).
- Alertas de outras execuções, banner de conexão perdida e o relógio ajustado pelo servidor.
- "O que saber" aberto no diálogo do desktop, com o que o celular pode e não pode fazer.

## 4. Divergências intencionais

- **Etapas abertas só quando não há pergunta pendente**: com pergunta, o cartão dela tem o foco e a barra de ações fica fixa.
- **Nota opcional recolhida só no plano**: nas perguntas de texto o campo é a própria resposta e fica aberto.
- **"Conectar" fixo no pé** com `position: sticky`, não preso à tela (o teclado do celular o empurraria).
- **"O que saber" não recolhe** no diálogo do desktop: é o aviso de segurança.

## 5. Componentes compartilhados alterados

| Mudança                                                               | Quem mais usa | Verificação |
| --------------------------------------------------------------------- | ------------- | ----------- |
| `src/mobile/Fold.tsx` novo (`<details>`; sem React Router nem `Icon`) | só o celular  | testes      |
| `Remote.module.css` e `MobileAccessDialog` (`size="wide"`, colunas)   | só o diálogo  | testes      |
| Nenhum componente de `src/components/ui` foi alterado                 | —             | —           |

Chave nova: `mobile.run.task` (pt-BR e en-US). O celular continua sem importar `Icon`, `Badge` nem o CSS do desktop, para manter o pacote pequeno.

**Tamanho do pacote do celular** (`npm run build`): JS 30,71 → 31,93 kB (gzip 8,96 → 9,38); CSS 10,72 → 14,67 kB (gzip 2,57 → 3,22).

## 6. `npm run check` e testes alterados

- Final: ver o fim deste arquivo.

**Testes alterados:** nenhum para o celular nem para o diálogo; as asserções passaram sem mudança (40 testes em `src/mobile`, 11 em `src/features/remote`).

## 7. Pendências e sugestões

- Responsividade: página do celular sem rolagem horizontal em 360, 390 e 430 px nas quatro fases e na execução; diálogo limpo em 720, 1100, 1440 e 2560 px.
- Não capturado: pergunta de permissão e de texto livre, execução concluída com resultado, execução com falha e código alterado (cobertos pelos testes).
- Não verificado em aparelho real: `safe-area-inset` e a barra de ações fixa com o teclado aberto (só no emulador).
- Próximo: Workflow fica fora desta etapa.

- Final: **verde** — typecheck, lint, Prettier, 70 arquivos / 777 testes, `cargo fmt`, `clippy` e `cargo test`. Rust, DTOs e comandos intocados. Numa das execuções completas, `MobileApp.test.tsx › tells the person when nothing is running yet` falhou com `EventSource is not defined` (efeito tardio sob carga); isolado passa 6 de 6, com e sem as mudanças deste módulo, e a execução seguinte do `check` ficou verde. Vale investigar à parte.
