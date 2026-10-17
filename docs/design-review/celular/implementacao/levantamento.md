# Levantamento — Stitch × app (Celular)

Fonte visual: HTML e PNG do Stitch em [`../stitch/`](../stitch/). Legenda: **existe**, **não existe**
(não implementado), **adaptado**. As telas do celular vieram do Stitch numa tela de desktop com a
página como coluna estreita no meio: vale a coluna de ~390 px. A moldura do desktop (barra lateral,
"Core") não existe no celular e foi ignorada; só a tela D1 (diálogo no desktop) tem moldura, e é a do app.

## M1 — Painel

| Stitch                                                                                      | App                                 |                                                               |
| ------------------------------------------------------------------------------------------- | ----------------------------------- | ------------------------------------------------------------- |
| Cabeçalho "Atlas" + nome do aparelho                                                        | existe                              | existe                                                        |
| Faixa "Esperando sua resposta" com "Dispensar"                                              | faixa de alerta com Abrir/Dispensar | existe (não alterada)                                         |
| Seções com ponto colorido, título em caixa alta e contagem                                  | existe                              | existe                                                        |
| Cartão: título, selo de estado, "workspace · tarefa" truncado, aviso, agente, etapas, tempo | existe, em linhas soltas            | adaptado: duas linhas de contexto + rodapé com avanço e tempo |
| Cartão com borda de acento (espera / falha)                                                 | existe                              | existe                                                        |
| Rodapé com texto sobre o computador e "Desconectar este celular"                            | existe                              | existe                                                        |
| Cartões de exemplo ("Auditoria de conciliação…", "1h 00m")                                  | dados reais                         | adaptado: dados reais                                         |

## M2 — Execução esperando resposta

| Stitch                                                                                               | App                                        |                                                  |
| ---------------------------------------------------------------------------------------------------- | ------------------------------------------ | ------------------------------------------------ |
| Voltar, título + selo, linha mono "workspace · iniciada · tempo"                                     | existe                                     | existe                                           |
| "Tarefa" recolhida numa linha com chevron                                                            | caixa de texto inteira                     | adaptado: `TaskBox` recolhível                   |
| Cartão da pergunta com barra de acento, tipo · agente, pergunta, plano com "Mostrar o texto inteiro" | existe                                     | existe                                           |
| Barra de ações fixa no pé: nota opcional recolhida, Aprovar, Pedir ajustes, Refazer                  | campo de nota sempre aberto, botões soltos | adaptado: barra fixa; nota recolhida só no plano |
| Etapas e "Controlar a execução" recolhidos                                                           | seções soltas                              | adaptado: `Fold` (`<details>`)                   |
| Etapas com linha-resumo ("1 de 3 etapas concluídas · Developer espera você")                         | contagem em outro lugar                    | adaptado: linha-resumo no `Fold`                 |

## M3 — Etapas e resultado

| Stitch                                                                     | App    |                                                              |
| -------------------------------------------------------------------------- | ------ | ------------------------------------------------------------ |
| Faixa "Developer espera você" com seta                                     | —      | não existe (o cartão da pergunta já faz esse papel)          |
| Etapas com linha do tempo, selos e "O que ela relatou"                     | existe | existe                                                       |
| Resultado com selo, Código com "N arquivos alterados", Registro, Controlar | existe | adaptado: cada um é um `Fold`; Código leva o resumo na linha |
| Ícone de chevron e cantos arredondados                                     | —      | adaptado: chevron em CSS, raio 8                             |

## M4 — Conectar

| Stitch                                                             | App                   |                                                     |
| ------------------------------------------------------------------ | --------------------- | --------------------------------------------------- |
| "Atlas", frase de apoio, dois passos numerados, campo e "Conectar" | existe, lista simples | adaptado: passos com número em quadro mono          |
| Botão "Conectar" de largura total ancorado no pé                   | botão solto           | adaptado: `position: sticky` no pé, com `safe-area` |
| Faixa de erro acima do campo                                       | texto vermelho        | adaptado: faixa com borda, `role="alert"`           |
| Nota sobre o código e a rede                                       | existe                | existe                                              |

## M5a / M5b — Sem conexão e sem execuções

| Stitch                                            | App                    |                            |
| ------------------------------------------------- | ---------------------- | -------------------------- |
| Faixa de erro com a explicação + "Tentar de novo" | texto vermelho + botão | adaptado: faixa com borda  |
| Vazio: cartão tracejado com texto                 | parágrafo solto        | adaptado: cartão tracejado |
| Ilustrações e ícones grandes                      | —                      | não existe                 |

## D1 — Acesso pelo celular (desktop)

| Stitch                                                                                                                    | App                                |                                                                    |
| ------------------------------------------------------------------------------------------------------------------------- | ---------------------------------- | ------------------------------------------------------------------ |
| Modal largo; título + estado + Desligar; aviso de pergunta; duas colunas (conectar / pareados)                            | uma coluna estreita                | adaptado: modal `wide`, duas colunas que empilham em largura menor |
| QR, código, validade, "Novo código", endereço + Copiar                                                                    | existe                             | existe, em painel com borda                                        |
| "Celulares pareados" com contagem, ponto verde, "Desconectar todos"                                                       | lista com Desconectar por aparelho | existe (contagem e ponto não)                                      |
| "Permissão: Responder perguntas e monitoramento", "Local Token Auth", "256-bit", "Sub-rede", "Agente: Invoice OCR Engine" | —                                  | não existe                                                         |
| "O que saber" recolhido                                                                                                   | lista aberta                       | mantido aberto (é o aviso de segurança do ADR 0034)                |
| Botão "Concluído" no rodapé                                                                                               | fechar pelo ×                      | não existe                                                         |
