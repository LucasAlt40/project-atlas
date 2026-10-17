# Prompts para o Stitch — Workflow

Ainda **não enviados**. Cada prompt = **P0** de [`../_comum/p0-regras.md`](../_comum/p0-regras.md)
(com `<NOME DA TELA>` trocado) + o bloco **TELA** abaixo. Projeto Stitch `526454930665111857`,
design system `assets/7aa0774b734b4149b2d38a23136aadfd`, `deviceType: DESKTOP`.

Telas novas aparecem ao lado das originais. A chamada dá timeout e a geração termina: não reenviar;
esperar o usuário confirmar e conferir com `list_screens`.

O Workflow é **uma página com três modos** (editor, execução, revisão). Os prompts dividem por modo e
por painel; **o cabeçalho (W1) e o seletor de execução são os mesmos em todas as telas**: peça ao Stitch
para repeti-los iguais.

| Prompt | Nome da tela                                    | Ferramenta                  | Base                               |
| ------ | ----------------------------------------------- | --------------------------- | ---------------------------------- |
| W1     | Workflow — Editor                               | `edit_screens`              | `9a4e01fec642497893da7b95e3f86727` |
| W2     | Workflow — Configurar etapa                     | `edit_screens`              | `3fa99d82145843139c5b78abffa6a4e3` |
| W3     | Workflow — Novo workflow                        | `generate_screen_from_text` | (nova)                             |
| W4     | Workflow — Execução em andamento                | `generate_screen_from_text` | (nova)                             |
| W5     | Workflow — Aguardando sua resposta              | `generate_screen_from_text` | (nova)                             |
| W6     | Workflow — Handoffs e alterações                | `edit_screens`              | `d5bccc8f0e124aaf9fb1d70b6c321bc4` |
| W7     | Workflow — Revisão e entrega                    | `edit_screens`              | `b0ff9a0657d14b59b7953901c33ea7d2` |
| W8     | Workflow — Retomar execução                     | `generate_screen_from_text` | (nova)                             |
| W9     | Workflow — Salvar como template e reparar rotas | `generate_screen_from_text` | (nova)                             |

Ordem: W1 → W2 → W4 → W5 → W7 → W6 → W3 → W8 → W9.

## Cabeçalho comum (descrito em W1; repetir igual em W2 a W8)

Uma só faixa de 56px, em vez de quatro: à esquerda o seletor de workflow ("Recuperação de senha") e o
botão "+ Novo workflow"; ao centro o selo "Automático · versão 1" e, em execução, o selo de estado e o
seletor de execução; à direita a ação principal do modo (no editor: "Salvar" e "Executar"; em execução:
"Pausar" e "Cancelar"; na revisão: "Voltar ao editor") e um menu "⋯" com "Personalizar workflow", "Salvar
como template", "Exportar", "Importar" e, em vermelho e separado, "Excluir workflow".

## W1 — Workflow — Editor

```text
TELA: o editor do workflow "Recuperação de senha", sem execução, com o canvas como protagonista.
Cabeçalho comum (uma faixa só, como descrito acima). Logo abaixo, a barra de tarefa, uma linha: o campo
"O que os agentes devem fazer? Ex.: Implementar recuperação de senha" com o botão primário "Executar" e,
ao lado, três opções discretas num menu "Opções da execução": "Como começar: Vibe coding | A partir de uma
spec aprovada" (a segunda desabilitada, com o link "Escrever uma nova spec"), a chave "Aprovar planos
automaticamente" e o escopo "transport-erp".
O canvas ocupa o resto da altura (pelo menos 60% da tela), com pontos de grade e 5 nós ligados: Arquiteto
(Arquiteto Backend · Arquiteto), Desenvolvedor (Implementador API · Arquiteto), QA (QA Regressão · QA, "passo
1 de 3"), Corretor de bugs e o nó final "Concluído"; as arestas de QA têm os rótulos "fail" (tracejada,
vermelha) e "pass" (verde); "5 nós · 5 rotas" no canto superior esquerdo; um minimapa pequeno no canto
inferior ESQUERDO, fora dos nós. Uma paleta flutuante sobre o canvas, centralizada no topo, reúne as
ferramentas: "+ Agente", "+ Condição", "+ Fim", "Conectar", "Remover", um separador, "Desfazer", "Refazer",
"Redefinir layout", "Ajustar" e o zoom "− 76% +".
À direita, o painel "Visão geral do workflow" de 320px: "Nome" (campo "Recuperação de senha"), "Automático ·
versão 1 · 5 nós" e a seção "Ferramentas (MCP)" com github (ligada · conectada), playwright (ligada ·
falhou) e postgres (desligada · nunca verificada), cada um com "concedida ao workflow ou a um agente, não a
uma etapa" em texto secundário, e o botão "Gerenciar no hub MCP".
USE só esses dados. NÃO mude: as ferramentas, a barra de tarefa, a visão geral.
```

## W2 — Workflow — Configurar etapa

```text
TELA: o mesmo editor (mesmo cabeçalho comum, barra de tarefa e canvas), com a etapa "Arquiteto"
selecionada (borda de acento no nó). O painel da direita vira o painel "Configurar etapa" de 420px, que NÃO
cobre o canvas: o canvas se reduz e continua visível. Cabeçalho do painel: ícone, "Arquiteto", selo
"Agente", "✓ Etapa válida" e o id "NODE_ID: architect", botão fechar.
Conteúdo em seções recolhíveis, a primeira aberta: "Identificação & agente" ("Rótulo" Arquiteto; "Agente"
Arquiteto Backend; quatro dados somente leitura em uma linha de selos: Personalidade Arquiteto, Runtime
Claude CLI, Modelo sonnet, Isolamento Ligado, Contexto Por tarefa), "Contrato de resultado" (Geral),
"Instruções e critério de conclusão" (área de texto e o critério com a dica), "Permissões", "Resiliência"
(tentativas, ao falhar: parar o workflow ou rotear para uma etapa, loop máximo), "Ferramentas (MCP)" desta
etapa (conexão, escopo "todas as ferramentas" ou "somente leitura", "Conceder", "Revogar"), "Dependências e
rotas". Rodapé fixo do painel: o estado "Salvo" (ou "Alterações não salvas") e o botão primário "Salvar
configuração".
USE só esses campos. NÃO mude: as seções, os campos, as opções.
```

## W3 — Workflow — Novo workflow

```text
TELA: o painel "Novo workflow" no lugar do canvas (mesmo cabeçalho comum, com "Importar"). Em duas colunas,
sem rolagem longa. Coluna esquerda, em passos numerados: "1 Como começar" (cartões "Vibe coding — A tarefa
que você escreve é toda a instrução, como sempre foi." e "A partir de uma spec aprovada — Nenhuma spec está
pronta ainda." desabilitado, com o link "Escrever uma nova spec"), "2 Como montar" (segmento "Automático — O
Atlas propõe um workflow recomendado para a sua tarefa." | "Personalizado — Comece de um modelo ou do zero e
monte o grafo você mesmo."), "3 Tarefa" (área de texto, placeholder "Implementar recuperação de senha") e a
faixa "Sugerido para a sua tarefa: Implementar — Constrói uma funcionalidade ou mudança: planeja, implementa,
revisa e confere.". Coluna direita, "Não é isso? Escolha outro workflow": lista de cartões compactos de
template (Implementar [Recomendado], Mudança rápida, Depurar e corrigir, Revisar e auditar — cada um com uma
linha de descrição e "2 etapas · 2 ferramentas sugeridas" ou "Somente leitura") e, abaixo, o detalhe do
template escolhido (Implementar): descrição, "Etapas" como lista (Arquiteto, Desenvolvedor, Validador,
Corretor de bugs, QA, com o papel e o "feito quando"), "Saídas" e "Ferramentas sugeridas" (navegador, issues,
docs, repositório, cada uma com "pronta" ou "faltando"). Rodapé fixo: "Cancelar" e o primário "Criar
workflow". No modo Personalizado: "Começar de" (um template ou "Workflow em branco") e o nome opcional.
USE só esses dados.
```

## W4 — Workflow — Execução em andamento

```text
TELA: o workflow "Recuperação de senha" com uma execução em andamento. Cabeçalho comum em modo
execução: seletor de execução "16/10/2026, 22:07:53 · Em execução", selo "EM EXECUÇÃO" azul, botões
"Pausar" e "Cancelar", a chave "Aprovar planos automaticamente" e "Voltar ao editor". O canvas é o
protagonista (60% da altura): Arquiteto "Concluído" (verde), Desenvolvedor "Executando" (azul, com a
aresta de saída animada), QA, Corretor de bugs e Concluído "Pendente" (cinza); a paleta de edição some
(só "Ajustar" e zoom). À direita, o painel de execução de 360px com três abas "Visão geral | Handoffs (2)
| Alterações". Aba Visão geral: "Status — Em execução", barra "1 / 4 nós concluídos · 25%", a lista de
etapas com estado (Arquiteto — Concluído, Desenvolvedor — Executando, QA — Pendente, Corretor de bugs —
Pendente), "Atual: Desenvolvedor", "Agentes ativos", quatro contadores em linha (Executando 1, Pendentes 2,
Falharam 0, Bloqueados 0), a seção recolhível "Ferramentas (MCP)" e, no rodapé, "1 concluído(s) · 0
falha(s)". Em "Tarefa: Implementar a recuperação de senha por e-mail", texto secundário no topo do painel.
USE só esses dados.
```

## W5 — Workflow — Aguardando sua resposta

```text
TELA: a mesma execução, agora "Aguardando você" (selo âmbar "AGUARDANDO VOCÊ"; o chip do topo "1 agent
aguardando você" é moldura). O canvas continua visível (Desenvolvedor com borda âmbar). A pergunta NÃO
cobre o canvas: aparece num painel lateral de 420px à direita, que substitui o painel de execução e tem a
aba "Responder" em primeiro plano. Conteúdo do painel: o rótulo "Ação necessária", "Desenvolvedor está
aguardando sua resposta", "Motivo da espera: Esclarecimento", a pergunta "Qual estratégia de autenticação
devo usar?" em destaque, a seção recolhível "Plano / mensagem completa" (aberta, markdown: "Plan — 1. Add the
column — 2. Fix the webhook"), a seção "Contexto" (fechada), o campo "Digite sua resposta…" e os botões
"Enviar resposta" (primário) e, separado, "Cancelar execução" (vermelho), com a nota "Sua resposta nunca
aplica mudanças ao projeto: isso é uma revisão separada.". Descreva, sem desenhar, as variantes: plano
("Aprovar", "Pedir ajustes", "Refazer o plano"), aprovação e pedido de permissão.
USE só esses dados.
```

## W6 — Workflow — Handoffs e alterações

```text
TELA: a execução concluída "Recuperação de senha" (mesmo cabeçalho comum e canvas), com o painel direito de
360px nas abas "Handoffs (2)" e "Alterações". Aba Handoffs: uma linha do tempo vertical com um cartão por
passagem entre etapas: "Handoff 1 · success" no topo; "Arquiteto → Desenvolvedor · passo 1" como título;
dois selos "Emissor: Arquiteto Backend (Arquiteto)" e "Receptor: Implementador API (Desenvolvedor)"; o
resumo "Implemented the endpoint."; e, recolhidas e só quando existirem, as seções "Contrato de saída e
resultado" ("status: success · Execução concluída"), "Decisões", "Artefatos", "Arquivos alterados",
"Validação" e "Falha". Aba Alterações: lista de arquivos com marcador A (verde) ou M (âmbar), caminho em mono
("src/auth/password-reset.ts +82 −0", "src/auth/auth.controller.ts +31 −4"), o total "2 arquivo(s) · +113 −4"
e o botão "Revisar diff".
USE só esses dados.
```

## W7 — Workflow — Revisão e entrega

```text
TELA: a execução concluída, no modo revisão. Mesmo cabeçalho comum com o botão "Voltar ao editor". Um
controle segmentado "Canvas | Revisão" no topo da área de conteúdo, com "Revisão" ativo. A revisão ocupa a
largura toda e se organiza assim:
1. Faixa de resumo em uma linha: "Workspace de revisão · Workflow concluído (selo verde) · 3 etapa(s) · 3
   agente(s) · 2 arquivo(s) alterado(s) +113 −4 · 18m 0s · Sem findings".
2. Cartão de entrega "O que entra no projeto", o mais importante da tela: duas linhas de estado ("Revisão —
   Pronto para revisão" e "Integração — Não aplicado"), o texto "Foram produzidas alterações no código, que
   estão num worktree isolado. Ainda não estão no seu projeto.", "2 arquivo(s) alterado(s) · +113 −4" e as
   ações em ordem de importância: "Revisar diff" (primário) à esquerda; "Aplicar alterações" (secundário
   de destaque), "Abrir na IDE" (ghost, desabilitado), e, num menu "⋯",
   "Manter isolado" e, em vermelho e separado, "Descartar". A frase de segurança "Concluir o workflow não
   significa que o código está no seu projeto: você decide o que entra nele. Aplicar só altera seus
   arquivos; nunca faz commit nem push." fica logo abaixo dos botões, em texto secundário.
3. Seções recolhíveis, cada uma com contagem no cabeçalho e fechadas quando a contagem é 0: "Linha do tempo
   dos agentes (3)" (aberta: Arquiteto Concluído · Arquiteto Backend · 30s; Desenvolvedor; QA, com
   tentativas e "Abrir execução"), "Findings (0)", "Handoffs (2)", "Decisões (0)", "Artefatos (0)" e
   "Conjunto de alterações (2)" com "Revisar o diff de todos os arquivos".
Descreva também, sem desenhar, os estados da entrega: "Aplicado na working tree — sem commit" (com
"Desfazer"), "Descartado", "Conflitos" e "Sem código para integrar".
USE só esses dados. NÃO mude: as ações de entrega, as seções.
```

## W8 — Workflow — Retomar execução

```text
TELA: o workflow "Recuperação de senha" com uma execução "Interrompida (recuperável)" (selo âmbar).
Cabeçalho comum em modo execução. Sobre o canvas, um cartão de destaque "Esta execução pode ser
retomada" com: "Último passo concluído: Arquiteto", "Parou em: Desenvolvedor", "Continua a partir de:
Desenvolvedor", "Mantidos sem refazer: Arquiteto", "Retomada 1 vez(es)" e o botão primário "Retomar de onde
parou" e, separado, "Cancelar". No canvas, Arquiteto "Concluído (reutilizado, não rodou de novo)" e
Desenvolvedor "Interrompido". Descreva, sem desenhar, o estado "Esta execução ainda não pode ser
retomada" (cartão cinza com o motivo).
USE só esses dados.
```

## W9 — Workflow — Salvar como template e reparar rotas

```text
TELA: dois modais lado a lado no mesmo quadro, sobre o editor esmaecido. (a) "Salvar como template": campo
"Nome", campo "Descrição", "Escopo" (este workspace ou todos) e os botões "Cancelar" e "Salvar template"
(primário). (b) "Reparar rotas": introdução curta, e uma lista por agente: nome do agente ("QA"), "Contrato
atual: Geral" e "Contrato sugerido: Validação" lado a lado, e para cada rota que falta um seletor "Escolher
rota" com a nota; rodapé "Cancelar" e "Aplicar" (primário), e o link "Histórico".
```

## Registro de envios

| Prompt         | Enviado | Tela gerada no Stitch | Observação |
| -------------- | ------- | --------------------- | ---------- |
| (nenhum ainda) |         |                       |            |
