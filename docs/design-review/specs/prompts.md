# Prompts para o Stitch — Specs

Ainda **não enviados**. Cada prompt = **P0** de [`../_comum/p0-regras.md`](../_comum/p0-regras.md)
(com `<NOME DA TELA>` trocado) + o bloco **TELA** abaixo. Projeto Stitch `526454930665111857`,
design system `assets/7aa0774b734b4149b2d38a23136aadfd`, `deviceType: DESKTOP`.

Telas novas aparecem ao lado das originais. A chamada dá timeout e a geração termina: não reenviar;
esperar o usuário confirmar e conferir com `list_screens`.

Este módulo tem duas telas grandes (a lista e o detalhe). O detalhe é dividido por **estado da spec**,
para o Stitch desenhar cada situação sem inventar:

| Prompt | Nome da tela                      | Ferramenta                  | Base                               |
| ------ | --------------------------------- | --------------------------- | ---------------------------------- |
| S1     | Specs — Lista de especificações   | `edit_screens`              | `a60e9fdaf74f40f2b736fceb1ddbf345` |
| S2     | Specs — Sem especificações        | `generate_screen_from_text` | (nova)                             |
| S3     | Spec — Documento em revisão       | `generate_screen_from_text` | (nova)                             |
| S4     | Spec — Agente escrevendo          | `generate_screen_from_text` | (nova)                             |
| S5     | Spec — Perguntas e achados        | `generate_screen_from_text` | (nova)                             |
| S6     | Spec — Revisão final e publicação | `edit_screens`              | `5126e494822042009e170b618aaaa661` |
| S7     | Specs — Nova especificação        | `generate_screen_from_text` | (nova)                             |
| S8     | Specs — Importar do projeto       | `generate_screen_from_text` | (nova)                             |

Ordem: S1 → S3 → S6 → S4 → S5 → S7 → S8 → S2. Em S3 a S6 o **cabeçalho e o stepper são idênticos**:
mantenha-os iguais nas quatro telas.

## Acréscimos a todo prompt deste módulo

Colar logo antes do bloco TELA, depois do P0. Também vale trocar, na moldura do P0, "Barra lateral de
216px" por "Barra lateral de 216px, de cima a baixo da janela" e "Barra superior de 48px" por "Barra
superior de 48px, só à direita da barra lateral":

```text
AJUSTES DESTE MÓDULO: na moldura, o rodapé da barra lateral mostra "Project Atlas v0.1.5". Não desenhe
rodapés informativos, rótulos técnicos extras, selos decorativos nem ícones sem função, e não escreva
resumos, descrições de cartões, listas de nomes nem textos de exemplo que não estejam descritos. Quando o
prompt dá um dado "de exemplo", use exatamente esse.
```

Para S1 e S6 (`edit_screens`), acrescentar também: "Ignore a moldura e os textos da tela base que não
estejam descritos aqui; vale a moldura do P0."

## S1 — Specs — Lista de especificações

```text
TELA: página "Specs". Cabeçalho: título "Specs" e o subtítulo "Defina, valide e aprove uma
especificação antes de os agentes escreverem código." (sem a trilha em mono "SDD · Desenvolvimento
orientado a especificação / OpenSpec"). À direita, "Importar specs" (secundário) e "Nova
especificação" (primário).
Filtros: os grupos como abas com contagem — "Todas (5) | Em andamento (2) | Aguardando você (2) |
Bloqueadas (0) | Prontas (1) | Arquivadas (0)" — e a busca "Buscar por título, id ou pedido". Os
contadores soltos acima ("3 em andamento"…) saem: as abas já os dão.
Lista: cartões em grade de até 3 colunas; cada um com uma barra fina colorida à esquerda conforme o
estado. No cartão: id da mudança em mono ("dark-mode"), selo de estado à direita ("Aguardando sua
revisão" azul, "Agente escrevendo" azul, "Alterações solicitadas" âmbar, "Pronta para implementação"
verde, "Rascunho" cinza), título ("Modo escuro"), uma linha com o pedido, a barra de avanço
"1/4 aprovados", e no pé o agente que escreve ("Arquiteto Backend"), "Agora: Requisitos" e "há 32
segundos". Quando houver, uma linha de alerta âmbar: "2 pergunta(s) para você" e "1 achado(s) em
aberto". Mostre 5 cartões: Modo escuro (Aguardando sua revisão, 1/4), Exportar faturas em CSV (Agente
escrevendo, 0/4), Recuperação de senha (Alterações solicitadas, 2/4, os dois alertas), Refatorar o
faturamento (Pronta para implementação, 4/4, "Todos os documentos aprovados") e Painel de métricas
(Rascunho, 0/4).
USE só esses dados. NÃO mude: os grupos de filtro, a busca, o que cada cartão informa.
```

## S2 — Specs — Sem especificações

```text
TELA: a página "Specs" sem nenhuma spec. Cabeçalho igual ao da lista (título "Specs", subtítulo "Defina,
valide e aprove uma especificação antes de os agentes escreverem código." e os botões "Importar specs" e
"Nova especificação"). No centro, um estado vazio simples com o título "Nenhuma especificação ainda", o
texto "Descreva o que você quer construir. Os agentes escrevem a proposta, os requisitos, o design e as
tarefas, um de cada vez, e você aprova cada um antes do próximo. Nada roda até você confirmar." e os
botões "Nova especificação" (primário) e "Importar specs" (secundário). Nada mais.
```

## S3 — Spec — Documento em revisão

```text
TELA: detalhe de uma spec, na etapa "Requisitos" com o documento escrito e aguardando revisão.
Cabeçalho: seta "← Todas as specs", título "Modo escuro", selo "Aguardando sua revisão", à direita os
botões "Agentes" (secundário) e um menu "⋯" com "Excluir" em vermelho. Linha de metadados em mono
discreta: "dark-mode · Formato spec-driven/1 · transport-erp · Escreve: Arquiteto Backend · Revisa: QA
Regressão".
Stepper horizontal de 5 etapas (o nome da etapa em cima e o estado embaixo, em texto pequeno; abaixo de
900px de largura vira uma lista vertical), um selo por etapa: "Proposta — Aprovado" (✓ verde), "Requisitos —
Aguardando revisão" (atual, destacada), "Design — Aguardando documentos anteriores" (cinza),
"Tarefas — Aguardando documentos anteriores" (cinza), "Revisão final — A fazer" (cinza).
Logo abaixo do stepper, um cartão de destaque "Sua decisão": "Leia o documento Requisitos. Aprovar
libera a escrita de Design; pedir alterações envia suas palavras ao agente." com as ações "Escrever de
novo" (ghost), "Solicitar alterações" (secundário) e "Aprovar e continuar" (primário).
Conteúdo em duas colunas. O conteúdo do documento fica em inglês, exatamente como escrito (só a interface
é em português). Coluna principal: o documento — nome do arquivo "specs/dark-mode/spec.md",
"Revisão 1 · escrito pelo agente · há 1 minuto", alternância "Requisitos | Markdown" e o texto:
"ADDED Requirements", "Requirement: User can switch the theme", "The system SHALL let the user pick
light or dark.", "Scenario: Switch to dark — WHEN the user picks dark — THEN the interface uses the dark
theme". Coluna lateral com três seções recolhíveis: "Validações automáticas" (selo "Aprovado", "Nenhum
problema encontrado na estrutura.", link "Rodar validações novamente"), "Histórico" ("Revisão 1 · escrito
pelo agente · há 1 minuto") e "Consumo" (fechada).
USE só esses dados. NÃO mude: as etapas, as ações da decisão, o que as seções informam.
```

## S4 — Spec — Agente escrevendo

```text
TELA: o mesmo detalhe de spec (mesmo cabeçalho e stepper que "Spec — Documento em revisão", com o stepper de nome em cima e estado embaixo), agora com
a etapa "Proposta" sendo escrita. Stepper: "Proposta — Escrevendo" (atual, com indicador de
progresso azul) e as demais em cinza. Em vez do cartão "Sua decisão" aparece a faixa de execução:
"Arquiteto Backend está escrevendo: Proposta" com o botão "Cancelar execução" (secundário) e um link
"Detalhes". A coluna principal mostra o texto parcial do documento "proposal.md" ("Proposal — Why — Night
users find the bright interface tiring…") esmaecido; a coluna lateral mostra "Validações automáticas"
("Nada escrito ainda.") e "Histórico" sem nenhum texto dentro. Sem outras ações.
```

## S5 — Spec — Perguntas e achados

```text
TELA: o mesmo detalhe de spec (mesmo cabeçalho e stepper), de "Recuperação de senha", selo "Alterações
solicitadas", na etapa "Design" com duas partes visíveis.
1. Cartão de destaque "O agente precisa da sua resposta" com a pergunta do agente (dado de exemplo: "Devo
   manter a sessão ativa ao redefinir a senha?"), o campo "Sua resposta"
   e o botão "Enviar resposta e continuar"; embaixo, "Este documento deixa 2 pergunta(s) em aberto" com o
   botão "Responder" que abre o diálogo de respostas.
2. Seção "Achados da revisão" com o resumo "1 em aberto · 1 pergunta algo a você" e a lista de achados;
   cada achado é uma linha com o selo da categoria (Ambiguidade, Contradição, Risco, Dependência,
   Cobertura, Decisão ou Outro), a severidade (Baixa, Média ou Alta), o estado (Em aberto, Resolvido ou
   Descartado), o título e, ao expandir, "Evidência e raciocínio", "Sugestão:", a nota "Opinião do agente
   revisor, não uma validação que falhou. Você decide." e as ações "Ver no documento", "Marcar como
   resolvido", "Não é problema", "Aceitar o risco…" (pede o motivo "Por que é aceitável? (fica registrado
   na spec)") e "Reabrir". Mostre dois achados: um de Risco e severidade Alta, em aberto, expandido, com o título (dado de exemplo)
   "Link de redefinição sem expiração"; e um de Ambiguidade, severidade Baixa, resolvido, com o título
   (dado de exemplo) "Prazo do link não definido". Abaixo, "Achados que você já decidiu (1)" recolhido.
USE só esses dados.
```

## S6 — Spec — Revisão final e publicação

```text
TELA: o mesmo detalhe de spec (mesmo cabeçalho e stepper), de "Refatorar o faturamento", selo "Pronta
para implementação", na etapa "Revisão final" (as cinco etapas do stepper aprovadas, a última atual).
Bloco principal "O que falta para esta spec ficar pronta", com o selo grande "Pronta" e o botão primário
"Aprovar a especificação" no próprio bloco, e abaixo a lista de sete verificações em LINHAS (não em
cartões), cada uma com ✓, o nome, a frase de resultado e o link "Abrir": "Os quatro documentos aprovados —
Todos os documentos estão aprovados e atualizados.", "Estrutura validada — Nenhum erro em nenhum documento.",
"Documentos coerentes entre si — Nada se contradiz entre proposta, requisitos, design e tarefas.", "Nenhuma
pergunta em aberto — Os agentes não têm pergunta esperando resposta.", "Revisada pelo agente revisor — A
revisão leu exatamente estas revisões.", "Achados decididos — Nenhum achado sério aguarda a sua decisão.",
"Sua aprovação final — Dada sobre estas mesmas revisões.". Ao lado do título, o link "Rodar validações
novamente".
Abaixo, quatro seções recolhíveis, cada uma com selo de estado no cabeçalho: "Revisão pelo agente revisor"
(selo "Não executada"; texto "QA Regressão lê os documentos aprovados procurando lacunas, ambiguidades e
contradições. Só lê: não altera nada."; "Rodar a revisão" e "Seguir sem a revisão…"), "Publicar no
projeto" (selo "Não publicada"; "Nome da pasta da mudança" com "billing-refactor", dica "Letras
minúsculas, dígitos e hífens", botão "Publicar"), "Implementar a partir desta spec" (botão "Iniciar um
workflow a partir desta spec") e "Arquivar com o OpenSpec CLI". Ações irreversíveis ("Publicar", "Arquivar")
em destaque cuidadoso, nunca como o primário da página.
USE só esses dados. NÃO mude: as sete verificações, as ações de cada seção.
```

## S7 — Specs — Nova especificação

```text
TELA: a lista "Specs — Lista de especificações" esmaecida ao fundo e, sobre ela, um modal largo "Nova
especificação" em duas colunas, sem rolagem interna em 1440×900 (abaixo de 900px de largura as colunas
empilham e o modal pode rolar). Coluna esquerda: "Título" (placeholder "Modo escuro
na tela de configurações"), "Projeto" ("transport-erp"), "O que você quer construir?" (área de texto com a
ajuda "Regras de negócio que você conhece entram aqui. O que ficar de fora, os agentes perguntam em vez de
inventar."), "Mais alguma coisa que o agente deve saber" ("Texto livre: links, exemplos, restrições, o que
já foi tentado."). Coluna direita: o cartão "Ajude o agente a não adivinhar" com a introdução ("Quanto mais
você disser agora, menos a especificação inventa…") e as perguntas de contexto que aparecem conforme a
descrição, cada uma com o selo do motivo ("Sempre perguntado", "Sua descrição fala de telas"), por
exemplo "O que deve ser verdade quando isso estiver pronto? Dê um exemplo concreto." e "O que NÃO deve
mudar ou fica de fora do escopo?", cada uma com "Sua resposta (opcional)"; abaixo, "Agente que escreve"
("Arquiteto Backend") e "Agente que revisa" ("O mesmo que escreve"). Rodapé: a nota "Criar apenas guarda o
pedido. Nenhum agente roda até você iniciar.", "Cancelar" e "Criar" (primário).
```

## S8 — Specs — Importar do projeto

```text
TELA: a lista "Specs — Lista de especificações" esmaecida ao fundo e, sobre ela, um modal "Importar
specs". Texto: "Mudanças OpenSpec que já estão na pasta openspec/changes deste projeto. São apenas lidas:
nada no projeto muda e nada é aprovado por você.". Dois seletores lado a lado: "Projeto" ("transport-erp")
e "Agente que escreve" ("Arquiteto Backend"). Lista do que foi encontrado, com uma linha por mudança
(nome da pasta em mono, os documentos presentes como selos e, quando houver, o problema) e uma caixa de
seleção por linha. Três linhas, dados de exemplo: "add-search" (Proposta, Requisitos, Design e Tarefas
presentes, sem problema), "fix-login" (problema "Sem o documento Design") e "notes-cleanup" (problema
"Arquivo que o Atlas não lê: notes.txt"). O problema "Já é uma spec aqui" aparece só quando a pasta já foi
importada (não desenhe).
Estado alternativo: "Nenhuma mudança OpenSpec encontrada neste projeto.". Aviso: "Você revisa e aprova os
documentos importados; os que faltam podem ser escritos a partir dos demais.". Rodapé: "Cancelar" e
"Importar" (primário).
```

## Registro de envios

| Prompt         | Enviado | Tela gerada no Stitch | Observação |
| -------------- | ------- | --------------------- | ---------- |
| S1 | 2026-10-16 | "Specs — Lista de especificações" `a7eb1bf978364f498e6908e8e85b6ed4` (2560×2048) | `edit_screens` a partir de `a60e9fda…`, com bloco comum; timeout, não reenviado, confirmado por `list_screens`. A original segue intacta. |
| S3 | 2026-10-16 | "Spec — Documento em revisão" `d439e1fbb75c4b7f9d684b0b3c46a543` (2560×2048) | `generate_screen_from_text` com bloco comum; timeout, não reenviado, confirmado por `list_screens`. |
| S6 | 2026-10-16 | "Spec — Revisão final e publicação" `fb6be8f6a81149e3873c33d6d98f31f0` (2560×2048) | `edit_screens` a partir de `5126e494…`; timeout, não reenviado, confirmado por `list_screens`. A original segue intacta. |

## Avaliação

### S1 — Specs — Lista de especificações

Fiel: cabeçalho sem a trilha "SDD…", abas com contagem e busca na mesma linha, cartões em grade de 3 colunas
com barra colorida e selo de estado, barra de avanço, alerta âmbar no cartão de senha e "Todos os documentos
aprovados" no cartão pronto. Moldura correta, v0.1.5.

O Stitch mostra, o app não tem (não implementar):

- Rótulo "Aprovação técnica" acima da barra de avanço (o app mostra só "1/4 aprovados").
- Textos de pedido inventados em todos os cartões ("Suporte completo a tema escuro…"); usar o pedido real.
- Agentes inventados ("Implementador API", "Arquiteto .NET Sênior") e etapas "Design Técnico" e "Validação
  Final"; usar os nomes reais das etapas (Proposta, Requisitos, Design, Tarefas, Revisão final).
- Ids novos além do pedido ("export-invoices-csv", "password-recovery", "metrics-dashboard"); só o dado real.

Problemas de layout para o app não repetir: o pé do cartão ("Agora: …  há …") e o alerta âmbar ficam
cortados (o texto precisa quebrar ou ir para duas linhas); o id "billing-refactor" e o selo "Pronta para
implementação" quebram em duas linhas; o botão primário saiu com "+ +" (ícone e texto repetidos).

### S3 — Spec — Documento em revisão

Muito fiel: cabeçalho com seta, título, selo e metadados em mono; stepper em cinco cartões (nome em cima,
estado embaixo, atual destacada com borda índigo); "Sua decisão" com as três ações na ordem certa;
documento com a alternância Requisitos | Markdown; coluna lateral com Validações, Histórico e Consumo
(fechada). Sem texto cortado. É a tela de referência do cabeçalho e do stepper para S4, S5 e S6.

O Stitch mostra, o app não tem (não implementar):

- Títulos "SUA DECISÃO" em caixa alta com espaçamento (usar o título normal do app).
- O documento renderizado em blocos: chip "+ ADDED Requirements", cartão por requisito e "SCENARIO: SWITCH
  TO DARK" com WHEN/THEN coloridos em caixas separadas. A aba "Requisitos" do app já faz uma renderização
  própria: manter o que ela faz hoje e usar o Stitch só como direção de espaçamento e hierarquia.
- Ícones decorativos nos títulos das seções laterais e o menu "⋯" sem o menu aberto (só o botão).

Desvio: o stepper ocupa um cartão grande com cada etapa em sua própria caixa; no app pode ser mais enxuto
(uma linha de passos) desde que mantenha nome, estado e destaque da etapa atual.

### S6 — Spec — Revisão final e publicação

Muito fiel e consistente com S3: mesmo cabeçalho, metadados e stepper (quatro etapas "Aprovado" e a última
destacada). Bloco "O que falta para esta spec ficar pronta" com selo "Pronta", "Rodar validações
novamente" e o primário "Aprovar a especificação" no próprio bloco; sete verificações em linhas com ✓,
nome, resultado e "Abrir"; quatro seções recolhíveis fechadas, com selos "Não executada" e "Não publicada".
Sem texto cortado. As ações irreversíveis ficaram dentro das seções fechadas, fora do primário da página.

Pequenos desvios (não implementar): chevron duplicado (à esquerda e à direita) nas seções "Implementar a
partir desta spec" e "Arquivar com o OpenSpec CLI"; ponto azul solto no canto da etapa "Revisão final";
selo "Pronta" repetido (no stepper como texto e no bloco como selo; manter só o do bloco). Os conteúdos
das quatro seções (campos, botões, textos) não foram desenhados: implementar pelo prompt.
