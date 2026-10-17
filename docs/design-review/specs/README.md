# Revisão de design — Specs

Módulo 5 da repaginação (processo em [`../_comum/README.md`](../_comum/README.md)). Specs é o
desenvolvimento orientado a especificação (SDD, formato OpenSpec, ADR 0035): um pedido vira proposta,
requisitos, design e tarefas que agentes escrevem e uma pessoa aprova um a um; depois vem a revisão
final, a publicação no projeto e a implementação por um workflow.

Capturas em 1440×900 com dados de exemplo em [`atual/`](atual/). Capturar: `?specs=none|some`
(padrão `some`: cinco specs — `s1` Modo escuro com Requisitos em revisão, `s2` agente escrevendo, `s3`
alterações solicitadas, `s4` pronta para implementar, `s5` rascunho). Os dados vêm de
`src/test/specFixtures.ts` (texto dos documentos em inglês: é conteúdo do usuário, não da interface).
Não há dados de perguntas e achados no mock; para capturá-los, estender o mock em `tmp/` (`questions`,
`findings`, `aiReview` em `SpecDto`).

## 1. Moldura

Igual à dos outros módulos. A tela tem um breadcrumb próprio ("SDD · Desenvolvimento orientado a
especificação / OpenSpec") acima do título, que não é o da barra superior.

## 2. Telas, estados e interações

### 2.1 Specs — lista **[img 01]**

- **Cabeçalho**: trilha "SDD · Desenvolvimento orientado a especificação / OpenSpec", título "Specs",
  subtítulo; três contadores "3 em andamento · 1 aguardando você · 1 prontas para implementar" e os
  botões `Importar specs` (secundário) e `Nova especificação` (primário).
- **Filtros** em linha: busca "Buscar por título, id ou pedido" e grupos `Todas · Em andamento · Aguardando
você · Bloqueadas · Prontas · Arquivadas`, cada um com contagem.
- **Cartão de spec** (grade de até 3 colunas, uma barra colorida à esquerda): id da mudança em mono
  (`dark-mode`), selo de estado (`Aguardando sua revisão`, `Agente escrevendo`, `Alterações solicitadas`,
  `Pronta para implementação`, `Rascunho`, `Bloqueada`, `Publicada`, `Arquivada`, `Falha na execução`),
  título, o pedido em uma linha, barra "1/4 aprovados", o agente que escreve, "Agora: Requisitos", e
  avisos "2 pergunta(s) para você" / "1 achado(s) em aberto", "Implementada em N execução(ões)", "há 32
  segundos".
- **Estados**: sem workspace ("Abra um workspace para trabalhar com specs."), carregando, erro, nenhuma
  spec ("Nenhuma especificação ainda" + texto), nenhuma corresponde ao filtro.

### 2.2 Spec — detalhe **[img 02, 03, 04]**

Substitui a lista. Cabeçalho: `← Todas as specs`, título, selo de estado, `Agentes` (diálogo), `Excluir`
(com confirmação "Excluir esta especificação?"); linha de metadados em mono (id, "Formato
spec-driven/1", projeto, "Escreve: …", "Revisa: …", "Ferramentas" quando há integrações MCP).

- **Stepper de 5 etapas**: Proposta, Requisitos, Design, Tarefas, Revisão final, cada uma com número ou
  ✓ e a fase ("Aprovado", "Aguardando revisão", "Pronto para escrever", "Aguardando documentos
  anteriores", "Escrevendo", "Desatualizado", "Falhou").
- **Etapa de documento** (Proposta a Tarefas): painel do documento (nome do arquivo, "Revisão 1 · escrito
  pelo agente · há 1 minuto", seletor de revisão, alternância Requisitos renderizados / Markdown, lista de
  arquivos), coluna **Validações automáticas** (selo "Aprovado", "Nenhum problema encontrado na
  estrutura.", erros/avisos/sugestões, `Rodar validações novamente`) e **Histórico** (revisões, origem
  gerada / revisada / regerada / importada); **Consumo** (`UsageBar`: tokens de entrada e saída,
  limite e `Continuar (+N)` quando o limite é atingido).
- **Barra "Sua decisão"** (`Dock`, fixa no fundo): texto do que fazer e as ações da fase: `Escrever este
documento` (pronto), `Cancelar execução` (escrevendo), `Escrever de novo`, `Solicitar alterações`,
  `Aprovar e continuar`, e o aviso de que nada roda até o usuário pedir.
- **Escrevendo**: faixa "<agente> está escrevendo: Proposta" com `Cancelar execução` e detalhes.
- **Perguntas do agente** (**[código]**): "O agente precisa da sua resposta", campo "Sua resposta",
  `Enviar resposta e continuar`; um documento pode deixar "N pergunta(s) em aberto" com
  `AnswerQuestionsDialog` e `SendAnswersDialog` ("Enviar suas respostas", "Enviar N resposta(s) e
  revisar").
- **Solicitar alterações** (**[código]**): diálogo "O que deve mudar?" com o texto livre.
- **Revisão final** **[img 03]**: "O que falta para esta spec ficar pronta" com selo `Pronta` e sete verificações
  em cartões (Os quatro documentos aprovados, Estrutura validada, Documentos coerentes entre si, Nenhuma
  pergunta em aberto, Revisada pelo agente revisor, Achados decididos, Sua aprovação final), cada uma com
  `Abrir`; `Aprovar a especificação`, `Rodar validações novamente`; **Revisão pelo agente revisor** (selo
  `Não executada`, `Rodar a revisão`, `Seguir sem a revisão…` com motivo registrado, achados com
  categoria — Ambiguidade, Contradição, Risco, Dependência, Cobertura, Decisão, Outro —, severidade Baixa /
  Média / Alta, estado Em aberto / Resolvido / Descartado e as ações `Ver no documento`, `Marcar como
resolvido`, `Não é problema`, `Aceitar o risco…`, `Reabrir`); **Publicar no projeto** (nome da pasta da
  mudança, `Publicar`, `Publicar de novo`, `Arquivar`, estados Publicada / Falhou / Desatualizada, arquivos
  gravados); **Implementar a partir desta spec** (`Iniciar um workflow a partir desta spec`, leva ao
  Workflow); **OpenSpec CLI** (`CliPanel`: versão, "segunda opinião", `Arquivar` com confirmação — mescla e
  move); **Painel de tarefas** (`SpecRunPanel`: tarefas por grupo, requisitos, "depois de…", estados
  pendente / reportada / confirmada).
- **Agentes e ferramentas** (diálogo): quem escreve e quem revisa, e as ferramentas MCP para testar e
  revisar (`IntegrationsPicker`).

### 2.3 Nova especificação (modal) **[img 05]**

Título, projeto, "O que você quer construir?" (com ajuda), caixa "Ajude o agente a não adivinhar" com
perguntas de contexto que aparecem conforme a descrição (dados, integrações, quem pode fazer, telas,
dados pessoais, volume, regras; cada uma com "porque…"), "Mais alguma coisa que o agente deve saber",
"Agente que escreve", "Agente que revisa" (o mesmo ou outro), e a nota "Criar apenas guarda o pedido.
Nenhum agente roda até você iniciar." Botões `Cancelar` e `Criar`. É um modal alto que rola por dentro.

### 2.4 Importar specs (modal) **[img 06]**

Texto "Mudanças OpenSpec que já estão na pasta openspec/changes deste projeto. São apenas lidas: nada no
projeto muda e nada é aprovado por você.", projeto, agente que escreve, lista do que foi encontrado (com
problemas: "Sem o documento X", "Arquivo que o Atlas não lê", "Usa o esquema … que o Atlas não lê"),
"Nenhuma mudança OpenSpec encontrada neste projeto.", `Importar`.

### 2.5 Início de uma spec sem pedido iniciado (rascunho) (**[código]**)

`StartChoice`/"O pedido": explicação e `Iniciar especificação` (o rascunho só guarda o pedido).

## 3. Problemas de design observados

1. **Jargão no topo**: a trilha "SDD · Desenvolvimento orientado a especificação / OpenSpec" em mono
   minúsculo e o subtítulo explicam o método em vez de ajudar a achar uma spec.
2. **Contadores como botões**: "3 em andamento", "1 aguardando você" parecem botões mas são texto; os
   filtros de verdade estão uma linha abaixo, com os mesmos números repetidos.
3. **Cartões repetem o pedido**: nos cinco cartões o texto secundário é o mesmo parágrafo; o que difere
   (estado, avanço, quem espera) é pequeno e fica no pé.
4. **O detalhe é uma pilha**: stepper, painel de documento, validações, histórico, consumo, decisão,
   perguntas, achados, revisão, publicação, implementação, CLI, tarefas. Na Revisão final são 10 blocos
   seguidos, todos com o mesmo peso.
5. **A decisão está longe**: a barra "Sua decisão" é o que a pessoa precisa fazer agora, mas fica no fundo
   da página e compete com tudo.
6. **Sete verificações em cartões** na revisão final ocupam uma faixa inteira; o resultado ("Pronta") é um
   selo pequeno no canto.
7. **Duas formas de dizer estado**: selo de estado do cabeçalho, fase do stepper, selo da validação, selo
   da revisão, selo da publicação: cinco selos com formatos diferentes.
8. **Modal de nova spec muito longo**, com perguntas de contexto que aparecem no meio e empurram os
   campos; rola por dentro.
9. **Ações destrutivas e irreversíveis** (`Excluir`, `Publicar`, `Arquivar`) com o mesmo peso de ações
   seguras.
10. **Estados vazios e erro** são frases soltas.

## 4. Telas do Stitch que já existem (versões anteriores)

| Tela do app             | Tela anterior no Stitch (id)                                                                                                |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| Specs — lista e detalhe | Atlas - Specs Workspace (Spec-Driven Development) (`6230e9f584164a16bcea2935050d22e5` e `a60e9fdaf74f40f2b736fceb1ddbf345`) |
| Specs — revisão final   | Atlas - Specs: Revisão Final & Validação de Prontidão (`5126e494822042009e170b618aaaa661`)                                  |
| Handoff para o workflow | Atlas - Handoffs & Transferência entre Agentes (`d5bccc8f0e124aaf9fb1d70b6c321bc4`)                                         |

Os modais não têm tela no Stitch.

## 5. Lacunas

Não capturados: lista vazia, filtros com resultado vazio, spec em "agente escrevendo" com a faixa, perguntas
do agente, achados da revisão, publicação (publicada, falhou, desatualizada), OpenSpec CLI, painel de
tarefas, diálogos Agentes e excluir, estados de erro, limite de tokens atingido.
