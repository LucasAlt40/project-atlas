# Repaginação visual do Atlas — processo e base comum

Cada módulo do app passa pelo mesmo ciclo. Esta pasta guarda o que é comum a todos; cada módulo tem
a sua pasta ao lado (`docs/design-review/<módulo>/`).

## Ciclo de um módulo

1. **Dossiê** (`README.md` do módulo): telas, estados, interações, problemas de design, capturas do
   app em `atual/`.
2. **Prompts do Stitch** (`prompts.md`): um por tela, começando pelo P0 de [`p0-regras.md`](p0-regras.md).
   Revisados pelo usuário antes de enviar. Registro dos envios no fim do arquivo.
3. **Envio ao Stitch** (`edit_screens` ou `generate_screen_from_text`). A chamada dá timeout, mas a
   geração termina: não reenviar, esperar o usuário confirmar a tela e conferir com `list_screens`.
4. **Plano de implementação** (`plano-implementacao.md`) e **prompt para o chat de implementação**
   (`prompt-implementacao.md`), sempre a partir de [`principios-implementacao.md`](principios-implementacao.md).
5. **Implementação** numa branch `design/<módulo>`, com levantamento, capturas e relatório.
6. **Avaliação do usuário** e só então o módulo seguinte.

## Ordem dos módulos

| #   | Módulo                           | Pasta                     | Estado                                       |
| --- | -------------------------------- | ------------------------- | -------------------------------------------- |
| 1   | Workspace + Harness              | `workspace-harness/`      | feito (branch `design/workspace-harness`)    |
| 2   | Agentes e Personalidades         | `agentes-personalidades/` | 5 telas geradas no Stitch; falta implementar |
| 3   | MCP Hub                          | `mcp-hub/`                | 5 telas geradas; falta implementar           |
| 4   | Configurações                    | `configuracoes/`          | 5 telas geradas; falta implementar           |
| 5   | Specs                            | `specs/`                  | 8 telas geradas; falta implementar           |
| 6   | App móvel (celular)              | `celular/`                | 6 telas geradas; falta implementar           |
| 7   | Workflow (por último, é o maior) | `workflow/`               | plano pronto, prompts por enviar             |
| —   | Refatoração geral (app inteiro)  | `refatoracao-geral/`      | prompt único pronto, por enviar              |

## Layout de cada pasta de módulo

```
<módulo>/
  README.md                 dossiê: telas, estados, problemas
  atual/                    capturas do app antes da repaginação
  prompts.md                prompts do Stitch + registro dos envios
  plano-implementacao.md    o que implementar, fases, mapa tela → código
  prompt-implementacao.md   o prompt para colar no chat de implementação
  stitch/                   (criado na implementação) HTML e PNG exportados + DESIGN.md
  implementacao/            (criado na implementação) levantamento.md e relatorio.md
```

## Ferramenta de captura

[`captura/`](captura/) roda o app real no navegador com um Tauri simulado (sem backend), com
dados de exemplo. `chk.js` mede rolagem horizontal e texto cortado por tela. Uso:

```bash
mkdir -p tmp/design-capture
cp docs/design-review/_comum/captura/* tmp/design-capture/
npx vite --port 1420
# http://localhost:1420/tmp/design-capture/index.html
```

`tmp/` é ignorado pelo git. Cada módulo acrescenta, no `main.tsx` da captura, os comandos que as
suas telas precisam (a lista de parâmetros fica no `README.md` do módulo).

## O que o primeiro módulo ensinou

- O Stitch **inventa moldura** (busca ⌘K, botão Atlas CLI, notificações, avatar, "Sync OK", telemetria
  na barra lateral, barra de status). O P0 agora proíbe isso explicitamente.
- O Stitch **inventa métricas e ações** (Exportar log, Reexecutar, presets, contadores, "Nível 2").
  Tudo isso foi descartado na implementação e listado no relatório. O prompt de cada tela deve dizer
  "use só os dados descritos".
- O Stitch **duplica botões** (no cartão e no rodapé; no cabeçalho e no rodapé). Pedir uma só vez.
- Sub-componentes repetidos (por exemplo os cartões de achado) só são refeitos se o prompt os
  descrever. Se importam, descrevê-los num prompt próprio.
- **Base compartilhada** criada na branch `design/workspace-harness`: `Badge` (um sistema de selos com
  tons), `CollapsibleSection`, `OverflowMenu` (o "⋯"), `Steps` (passos numerados), `Modal` com
  tamanho largo, `HarnessFrame`. Os módulos seguintes **reaproveitam** esses componentes.
- A janela abre em 1100×720, largura mínima 720; a barra lateral começa recolhida abaixo de 900.
- O Stitch devolve tela nova em vez de editar a original; as originais ficam ao lado.
