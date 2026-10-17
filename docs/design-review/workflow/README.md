# Revisão de design — Workflow

Módulo 7 e último da repaginação (processo em [`../_comum/README.md`](../_comum/README.md)), por ser o
maior. O Workflow transforma uma tarefa num grafo de agentes (escolhido para você no modo Automático,
montado por você no Personalizado), que o núcleo executa; a tela mostra e edita a definição e acompanha a
execução, sem decidir nada sobre como ela roda. É também onde a pessoa **responde o que um agente
perguntou**, **recupera** uma execução, **revisa** o que foi feito e **decide o que entra no projeto**.

Capturas em 1440×900 com dados de exemplo em [`atual/`](atual/). Capturar: `?wf=none|idle|running|waiting|done|failed`
(padrão `idle`: um workflow sem execução). Os dados vêm de `src/test/workflowFixtures.ts` (agentes
`a1`, `a2`, `a3`). Para as execuções, abrir **Workflow** e escolher a execução no seletor "Editor (sem
execução)". Não capturados com o mock atual: edição de rota, reparo de rotas, recuperação, painel
"workspace ao vivo" (arquivos e código durante a execução), templates salvos, diálogos de salvar como
template, importar e exportar.

## 1. Moldura

Igual à dos outros módulos. O chip "1 agent aguardando você" e o botão "Celular" ficam na barra
superior (moldura, fora do escopo).

## 2. Telas, estados e interações

A página é **uma só**, com três modos: _editor_ (sem execução), _execução_ (acompanhar) e _revisão_
(o que ela produziu). Tudo fica empilhado na mesma coluna.

### 2.1 Workflow — editor (sem execução) **[img 01, 02]**

Quatro faixas de controle antes do canvas:

1. **Seletor de workflow** ("Recuperação de senha"), `+ Novo workflow`, selo "Automático · versão 1" e, à
   direita, as ferramentas do canvas: `+ Agente`, `+ Condição`, `+ Fim`, `Conectar`, `Remover`.
2. **Ações do workflow**: `Personalizar workflow` (passa do automático ao personalizado), `Salvar`, `Salvar
como template`, `Exportar`, `Importar`, `Excluir workflow`.
3. **Edição do canvas**: `Desfazer`, `Refazer`, `Redefinir layout`, `Ajustar`, zoom (− 76% +).
4. **Como você quer começar** (`StartChoice`): `Vibe coding` (a tarefa que você escreve é toda a instrução) ou
   `A partir de uma spec aprovada` (desabilitada sem specs prontas; `Escrever uma nova spec` leva a Specs) e a
   **barra de tarefa**: "O que os agentes devem fazer? Ex.: Implementar recuperação de senha", a chave
   `Aprovar planos automaticamente`, o escopo (`transport-erp`, `ScopePicker`) e `Executar`.

- **Canvas** (React Flow): nós de agente (nome, agente · personalidade, estado), de condição e de fim; arestas
  com rótulo (`pass`, `fail`); "5 nós · 5 rotas"; minimapa; zoom.
- **Visão geral** (coluna à direita): "Nome" editável, "Automático · versão 1 · 5 nós", bloco **Ferramentas
  (MCP)** (cada servidor com estado e a que está concedido) e `Gerenciar no hub MCP`.
- **Inspetor da etapa** (ao tocar um nó; **[img 02]**): "Arquiteto · Agente", "✓ Etapa válida", `NODE_ID`, seções
  **Identificação & agente** (Rótulo, Agente, quadro Personalidade / Runtime / Modelo / Isolamento / Contexto
  "Por tarefa"), **Contrato de resultado**, **Instruções / critério de conclusão**, **Permissões**,
  **Resiliência** (tentativas, ao falhar: parar ou rotear, loop máximo), **Ferramentas (MCP)** da etapa
  (escopo todas ou só leitura, `Conceder`, `Revogar`), dependências e rotas, "Execução N" e snapshot na
  execução; rodapé "Salvo" / "Alterações não salvas" e `Salvar configuração`. Aresta: "quando" e rótulo.
- **Estados**: sem workspace/agentes, workflow inválido (nós marcados; `Reparar rotas` → diálogo de reparo),
  salvando, erro.

### 2.2 Novo workflow **[img 03]**

Painel que substitui a página: título, `Importar`; "Como você quer começar" (vibe / spec); **Como montar**
(`Automático` — "O Atlas propõe um workflow recomendado para a sua tarefa." / `Personalizado` — "Comece de um
modelo ou do zero e monte o grafo você mesmo."); campo **Tarefa**; faixa "Sugerido para a sua tarefa:
Implementar — …"; "Não é isso? Escolha outro workflow" com **cartões de template** (Implementar
[Recomendado], Mudança rápida, Depurar e corrigir, Revisar e auditar…, cada um com descrição e "N etapas · N
ferramentas sugeridas · Somente leitura"); **detalhe do template escolhido** (descrição, Etapas com papel,
loop, instruções e "feito quando", Saídas, Ferramentas sugeridas — navegador, issues, docs, repositório — com
"pronta / faltando", `Aplicar ferramentas`, escopo, `Excluir` para os seus); no modo Personalizado: "Começar
de" (template ou "Workflow em branco") e nome opcional; `Criar workflow`.

### 2.3 Execução — acompanhar **[img 04, 05]**

Ao escolher uma execução no seletor ("16/10/2026, 22:07:53 · Em execução"): o seletor vira o da execução,
com `Voltar ao editor`, `Pausar` / `Retomar`, `Cancelar`; os selos "EM EXECUÇÃO" / "AGUARDANDO VOCÊ" /
"CONCLUÍDO" / "FALHOU" / "PAUSADO" / "INTERROMPIDO (RECUPERÁVEL)"; a chave `Aprovar planos
automaticamente` por execução. O canvas mostra o estado de cada nó (Concluído, Executando, Pendente, passo
N de M). Visão geral da execução com **três abas**: `Visão geral`, `Handoffs (2)`, `Alterações`.

- **Visão geral**: Status (barra "1 / 4 nós concluídos · 25%"), lista de etapas com estado e duração, "Atual",
  "Agentes ativos", contadores (Executando, Pendentes, Falharam, Bloqueados), Ferramentas (MCP), rodapé "1
  concluído(s) · 0 falha(s)", "Tarefa: …".
- **Handoffs** **[img 09]**: um cartão por passagem entre etapas — "HANDOFF #1 · há … · success", "Arquiteto →
  Desenvolvedor passo 1", agente emissor / receptor, "01 · Contrato de saída & resultado", resumo, decisões,
  artefatos, arquivos alterados, validação, falha.
- **Alterações** **[img 10]**: lista de arquivos (A / M, caminho, +adições −remoções) e `Revisar diff`.
- **Workspace ao vivo** (**[código]**): arquivos e código da execução em curso (`LiveWorkspacePanel`,
  `LiveFiles`, `LiveCode`).

### 2.4 Execução — Ação necessária **[img 04]**

Painel de destaque âmbar sobre o canvas: "AÇÃO NECESSÁRIA", "Desenvolvedor está aguardando sua resposta",
"Motivo da espera: Esclarecimento", a pergunta, "Plano / mensagem completa" (markdown), "Contexto" (recolhido),
campo "Digite sua resposta…", `Enviar resposta`, `Cancelar execução`, e a frase "Sua resposta nunca aplica
mudanças ao projeto: isso é uma revisão separada." Tipos: plano (`Aprovar`, `Pedir ajustes`, `Refazer`),
pergunta, aprovação, permissão. O canvas encolhe para ~150 px de altura.

### 2.5 Execução — recuperar (**[código]**)

`RecoveryPanel`: "Esta execução pode ser retomada", "Último passo concluído: X", "Parou em: Y", "Continua a
partir de: …", "Repete o passo que falhou: …", "Mantidos sem refazer: …", `Retomar de onde parou`, "Retomada N
vez(es)", "Esta execução ainda não pode ser retomada".

### 2.6 Revisão da execução **[img 06, 07, 08]**

Abaixo do canvas, "REVISÃO · Workspace de revisão · Workflow concluído / falhou / cancelado" e uma linha de
estatística em mono ("3 etapa(s) · 3 agente(s) · 2 arquivo(s) alterado(s) +113 −4 · 18m 0s · Sem
findings"). Quando há código: **cartão de entrega** (`DeliveryPanel`) "✓ Workflow concluído": Revisão
"Pronto para revisão", Integração "Não aplicado", o texto "Foram produzidas alterações no código, que estão
num worktree isolado. Ainda não estão no seu projeto.", "2 arquivo(s) alterado(s) · +113 −4", a frase "Concluir
o workflow não significa que o código está no seu projeto: você decide o que entra nele. Aplicar só altera
seus arquivos; nunca faz commit nem push." e as ações `Revisar diff`, `Abrir na IDE`, `Aplicar alterações`
(primário), `Manter isolado`, `Descartar` (com confirmações; estados Aplicado na working tree — sem commit,
Descartado, Conflitos, Desfazer). Depois, seções recolhíveis com contagem: **Linha do tempo dos agentes**
(por etapa: agente, estado, duração, tentativas, "Abrir execução"), **Findings** (erro / aviso / info,
"Encontrado por…", "Recomendação", "Validação posterior passou" / "Sem confirmação de correção", "Do finding à
correção"), **Handoffs**, **Decisões**, **Artefatos**, **Conjunto de alterações** (arquivos, `Revisar o diff de
todos os arquivos`).

- `ChangesModal` / revisão de diff: lista de arquivos e diff por arquivo.
- Estado compartilhado (`SharedStatePanel`) e planos (`PlansPanel`) (**[código]**).

### 2.7 Diálogos (**[código]**)

`Salvar como template` (nome, descrição, escopo), `Reparar rotas` (`RouteRepairDialog`: por agente, contrato
atual × sugerido, "escolher rota", nota, `Aplicar`), importar e exportar (portabilidade), excluir workflow e
excluir template (confirmação), `NewWorkflowPanel` já descrito.

## 3. Problemas de design observados

1. **Quatro faixas de controle antes do canvas** (seletor + ferramentas; ações do workflow; edição; "como
   começar" + tarefa): cerca de 230 px de 900 antes de qualquer nó, e o canvas fica com 280 px de altura. Em
   execução, a faixa "Ação necessária" tira mais 240 px e o canvas cai para ~150.
2. **Duas telas numa só**: editar o grafo, acompanhar uma execução e revisar o resultado compartilham a mesma
   coluna rolável. O canvas, que é a "figura" do workflow, vira uma faixa estreita entre dois blocos de texto.
3. **Controles de modos diferentes juntos**: `+ Agente`, `Conectar`, `Remover` (editar) ficam ao lado de
   `Executar` e `Salvar como template`; em execução ficam desabilitados mas visíveis.
4. **Inspetor longo e denso**: a etapa tem 71 textos; seções com cabeçalho mono minúsculo, campos de tamanhos
   diferentes, quadro de fatos misturado a campos editáveis; o inspetor aberto tapa a metade direita do canvas.
5. **Barra de tarefa com seis controles** numa linha (campo, chave de planos, escopo, `Executar`) e o seletor
   "como começar" acima, que quase nunca muda.
6. **Estado repetido em cinco lugares**: selo do cabeçalho, texto do seletor de execução, "Status" da visão
   geral, selos dos nós e título da revisão ("Workflow concluído") com formatos diferentes.
7. **Revisão como lista de acordeões idênticos**, sem prioridade: Linha do tempo, Findings, Handoffs, Decisões,
   Artefatos e Conjunto de alterações com o mesmo peso, quase todos vazios no exemplo.
8. **Cartão de entrega com quatro botões do mesmo tamanho** (`Revisar diff`, `Abrir na IDE`, `Aplicar
alterações`, `Manter isolado`, `Descartar`) e um parágrafo de segurança longo antes; a decisão mais séria do
   fluxo (o que entra no projeto) não se destaca.
9. **Handoffs em texto técnico** ("01 · CONTRATO DE SAÍDA & RESULTADO", "Resultado: success · Execução:
   concluída") sem hierarquia.
10. **"Novo workflow"**: página longa — escolha de início, modo, tarefa, sugestão, cartões, detalhe do template —
    antes do botão `Criar workflow`.
11. **Minimapa por cima dos nós** no canto do canvas e rótulos de rota pequenos.

## 4. Telas do Stitch que já existem (versões anteriores)

| Tela do app          | Tela anterior no Stitch (id)                                                                    |
| -------------------- | ----------------------------------------------------------------------------------------------- |
| Editor e canvas      | Atlas - Visualizador e Orquestrador de Workflow (`9a4e01fec642497893da7b95e3f86727`)            |
| Configurar etapa     | Atlas - Configuração de Etapa do Workflow (Nó Selecionado) (`3fa99d82145843139c5b78abffa6a4e3`) |
| Handoffs             | Atlas - Handoffs & Transferência entre Agentes (`d5bccc8f0e124aaf9fb1d70b6c321bc4`)             |
| Revisão / resultados | Atlas - Resultados de Execução & Artefatos Gerados (`b0ff9a0657d14b59b7953901c33ea7d2`)         |

## 5. Lacunas

Não capturados: reparo de rotas, recuperação, workspace ao vivo, templates salvos e seu detalhe com
ferramentas, "salvar como template", "importar/exportar", planos, estado compartilhado, aresta selecionada,
workflow inválido, execução pausada / interrompida / cancelada, entrega em outros estados (aplicado, conflito,
descartado), revisão de diff, findings reais.
