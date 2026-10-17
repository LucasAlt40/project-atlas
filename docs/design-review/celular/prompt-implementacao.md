# Prompt para o chat de implementação — Celular

Colar num chat novo, aberto na raiz do repositório, com o MCP do Stitch conectado. Só usar depois
que os prompts de `prompts.md` foram enviados e os ids das telas estão preenchidos no plano.

```text
Você vai implementar, numa branch separada, a repaginação visual do acesso pelo celular do Atlas: a
página que abre no navegador do celular (src/mobile/, entrada mobile.html) e o diálogo "Acesso pelo
celular" do computador (src/features/remote/). O usuário avalia o resultado antes do módulo seguinte,
então a fidelidade importa.

LEIA PRIMEIRO, nesta ordem:
1. docs/design-review/_comum/principios-implementacao.md (regras, fases, relatório). Siga-o, com as
   adaptações do plano para a página do celular.
2. docs/design-review/celular/plano-implementacao.md (telas, ids do Stitch, mapa de código, decisões e
   cuidados deste módulo).
3. docs/design-review/celular/README.md (as telas atuais, estados, interações e 9 problemas de design) e as
   imagens em atual/ (as do celular têm 780 px de largura: são 390 px a 2×).
4. docs/design-review/celular/prompts.md e docs/design-review/_comum/p0-mobile.md (o que foi pedido ao
   Stitch).
5. docs/design-review/workspace-harness/implementacao/relatorio.md (o que deu certo e o que o Stitch
   inventou no primeiro módulo; repita o método).

BRANCH: `design/celular`, a partir de `design/workspace-harness` (ou de `main` se essa já foi integrada).
Rode `npm ci`, `npm run check` e `npm run build` e registre a linha de base, incluindo o tamanho do
bundle da entrada mobile.

REGRAS QUE NÃO PODEM SER QUEBRADAS
- O Stitch é a referência VISUAL, não a especificação funcional. Se mostrar algo que o app não faz
  (aba, menu, filtro, notificação, gráfico, botão, texto de estado inexistente), NÃO implemente e liste em
  "O Stitch mostra, o app não tem". Se omitiu um dado que o app mostra, MANTENHA e liste em "Stitch omitiu,
  app manteve".
- LIMITES DO PRODUTO (ADR 0034): a página do celular só acompanha e responde (aprovar plano, perguntas,
  permissões), pausa, retoma e cancela. Não inicia execuções, não concede permissões, não aplica código e
  não mostra terminal. A repaginação não introduz nenhuma dessas ações; os avisos "isto só no computador"
  continuam. A lógica de envio das respostas (InteractionCard), de reconexão e de pareamento não muda.
- Máxima fidelidade ao que o Stitch mostra e o app suporta. Funcionalidade intacta (mesmos comandos,
  estados, regras e testes de comportamento).
- Mobile: 320 a 430 px em pé e paisagem de 568 a 844 px; alvos de toque ≥ 44 px; sem rolagem horizontal;
  safe-area e teclado virtual (a barra de ações fixa não pode cobrir o campo em edição). A página é outro
  bundle: não traga componentes pesados do desktop; reutilize só o que for leve e use tokens de
  global.css. Compare o tamanho do bundle mobile antes e depois no relatório.
- No diálogo do computador (D1): responsivo de 720 a 2560 px; "O que saber" recolhido mas com TODOS os
  pontos (são avisos de segurança); o botão "Celular" da barra superior é moldura e não muda.
- Textos em pt-BR.ts E en-US.ts; nada fixo no JSX. A página escolhe o idioma pelo navegador.
- Escopo: só este módulo. Não toque em Rust, no servidor remoto, em DTOs, comandos, nem nos outros
  módulos.

COMO OBTER O VISUAL: get_screen de cada id do plano, baixe HTML e imagem para
docs/design-review/celular/stitch/ (numerados, com DESIGN.md), como foi feito em
workspace-harness/stitch/. Se o download der 401, renderize o HTML; se não der, peça as imagens ao usuário.
Nunca implemente de memória.

COMO VERIFICAR: copie docs/design-review/_comum/captura/* para tmp/design-capture/, rode
`npx vite --port 1420`. Página do celular: http://localhost:1420/tmp/design-capture/mobile.html (?phase=ok|
unpaired|offline, ?runs=0..4, #/run/run-1), com o preset "mobile" do navegador (390×844). Diálogo do
computador: index.html?remote=off|on|error e o botão "Celular". Compare cada tela com o Stitch lado a lado.
Estenda o mock em tmp/; não mexa no código do app para isso.

ENTREGA: as fases do plano (§4) com commits pequenos em português no estilo do histórico, `npm run check`
verde, e o relatório em docs/design-review/celular/implementacao/relatorio.md (formato em
principios-implementacao.md §5, com o tamanho do bundle). NÃO faça push, NÃO abra PR, NÃO mexa na main. Ao
terminar, diga os comandos exatos para o usuário ver a branch (e como abrir a página no celular real, se
possível) e resuma o que precisa da decisão dele.

Se algo no plano conflitar com o código, pare e pergunte antes de decidir.
```
