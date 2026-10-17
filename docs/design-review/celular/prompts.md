# Prompts para o Stitch — Celular

Ainda **não enviados**. As telas da **página do celular** (M1 a M5) usam o **P0 móvel**
([`../_comum/p0-mobile.md`](../_comum/p0-mobile.md)) e `deviceType: MOBILE`. A tela do **computador** (D1)
usa o **P0** normal ([`../_comum/p0-regras.md`](../_comum/p0-regras.md)) e `deviceType: DESKTOP`. Projeto
Stitch `526454930665111857`, design system `assets/7aa0774b734b4149b2d38a23136aadfd`.

Nenhuma tela anterior no Stitch: todas usam `generate_screen_from_text`. A chamada dá timeout e a
geração termina: não reenviar; esperar o usuário confirmar e conferir com `list_screens`.

| Prompt | Nome da tela                           | Dispositivo |
| ------ | -------------------------------------- | ----------- |
| M1     | Celular — Painel de execuções          | MOBILE      |
| M2     | Celular — Execução esperando resposta  | MOBILE      |
| M3     | Celular — Execução: etapas e resultado | MOBILE      |
| M4     | Celular — Conectar este celular        | MOBILE      |
| M5a    | Celular — Sem conexão                  | MOBILE      |
| M5b    | Celular — Sem execuções                | MOBILE      |
| D1     | Computador — Acesso pelo celular       | DESKTOP     |

Ordem: M1 → M2 → M3 → M4 → M5a → M5b → D1.

## Acréscimos a todo prompt deste módulo

Colar logo antes do bloco TELA, depois do P0 móvel (M1 a M5b). Em D1, usar o P0 normal com a moldura
reforçada (ver D1).

```text
AJUSTES DESTE MÓDULO: não desenhe barra de status do sistema (hora, bateria, sinal), notch, teclado nem
indicador de "home". Não desenhe rodapés informativos, rótulos técnicos extras, selos decorativos nem ícones
sem função, e não escreva resumos, subtítulos, textos de ajuda nem textos de exemplo que não estejam
descritos. Nenhum texto pode ficar cortado com reticências, salvo onde o prompt manda. Não repita ícone e
símbolo no mesmo botão. Quando o prompt diz "não desenhe" um estado ou mensagem, ele só existe como texto.
```

## M1 — Celular — Painel de execuções

```text
TELA: o painel inicial. Barra de topo com "Atlas" e "iPhone do Lucas". Logo abaixo, uma faixa de alerta
dispensável (só quando há): "Esperando sua resposta" com o botão "Dispensar".
A lista de execuções agrupada por urgência, cada grupo com título e contagem; os grupos que pedem ação
vêm primeiro e em destaque: "Precisa de você (1)" (âmbar), "Em andamento (1)", "Com falha (1)"
(vermelho), "Terminadas recentemente (1)".
Cada execução é um cartão compacto de 3 linhas, e não de 6: (1) nome do workflow em destaque e o selo de
estado à direita ("Esperando você" âmbar, "Executando" azul, "Falhou" vermelho, "Concluída" verde);
(2) o workspace e a tarefa numa única linha ("Transport ERP · Implementar a recuperação de senha por
e-mail", com reticências só nessa linha); (3) a linha do que importa agora — para "Precisa de você": "Uma pergunta espera sua resposta"
(âmbar); para "Com falha": "A execução falhou." (vermelho); para as demais, "Developer · 2 de 4 etapas" e
a duração. O cartão inteiro é tocável. Mostre 4 cartões: Recuperação de senha (Esperando você), Revisão do
faturamento (Executando, 2 de 4 etapas, 32m 01s), Exportar CSV (Falhou) e Modo escuro (Concluída, 3 de 3
etapas, 1h 00m).
No fim da lista: o link "Desconectar este celular" e o aviso "O Atlas roda no seu computador. Esta página só
funciona enquanto ele estiver ligado, acordado e na mesma rede.".
USE só esses dados. NÃO mude: os quatro grupos, o selo de estado, o que cada cartão informa.
```

## M2 — Celular — Execução esperando resposta

```text
TELA: a execução "Recuperação de senha" aberta, esperando uma resposta. Topo: "‹ Voltar", "Atlas" e
"iPhone do Lucas". Cabeçalho da execução: título, selo "Esperando você", a linha "Transport ERP · iniciada
22:10 · 14m 06s" e a tarefa ("Implementar a recuperação de senha por e-mail") numa caixa recolhível.
O foco da tela é a pergunta: um cartão âmbar com o tipo ("Plano para aprovar · Developer · Dev agent"), a
pergunta "Você aprova este plano?" e o documento do plano ("O plano: criar a rota de recuperação; enviar o
e-mail com o link; escrever os testes") com "Mostrar o texto inteiro" / "Mostrar menos".
As ações ficam numa BARRA FIXA no pé da tela, ao alcance do polegar: o campo "Uma nota para o agente
(opcional)" recolhido num botão pequeno, o botão primário "Aprovar" em largura total e, abaixo, dois botões
menores lado a lado: "Pedir ajustes" e "Refazer o plano" (vermelho). A mensagem "Diga o que mudar e
toque em “Pedir ajustes” de novo." só aparece depois de um toque em "Pedir ajustes" sem texto: não desenhe.
A barra fica sempre visível no pé; a pergunta e as seções rolam por trás dela.
Depois da pergunta (rolando): as seções "Controlar a execução" com "Cancelar a execução" (vermelho, separado)
e "Etapas" resumida em uma linha ("1 de 3 etapas concluídas · Developer espera você"). Descreva, sem
desenhar, os outros tipos de pergunta: "Pergunta" (campo "Sua resposta" + "Enviar resposta"), "Aprovação"
(Aprovar / Rejeitar), "Pedido de permissão" (Permitir / Negar), "Confirmação" (Sim / Não), e o resultado do
envio: "Enviado. O computador confirmou.".
USE só esses dados. NÃO mude: as três respostas do plano e a nota opcional.
```

## M3 — Celular — Execução: etapas e resultado

```text
TELA: a mesma execução "Recuperação de senha" (mesmo topo e cabeçalho que "Celular — Execução esperando
resposta"), vista pelas seções, sem a barra de ações. Logo abaixo do cabeçalho, a pergunta aparece reduzida
a uma linha âmbar "Developer espera você" (um toque a abre). Depois, as seções recolhíveis com um selo no
cabeçalho de cada uma: "Etapas" (aberta), "Resultado" ("Até agora"), "Código" ("0 arquivos alterados (+0 −0)") e
"Registro de atividade" (fechadas).
"Etapas": linha do tempo vertical com marcador por etapa — Architect ✓ "Concluída" ("Arch agent · 30s", com
"O que ela relatou" recolhível), Developer ? "Esperando você" ("Dev agent · 21:17", "Depois: QA") e QA ○ "Não
iniciada" ("QA agent"). Menos informação por linha: nome, selo e, na segunda linha, o agente e a duração; o
resto só ao tocar. Em "Código" (fechada): o texto "Revisar e aplicar o código é feito no computador." aparece só ao abrir.
Rodapé da seção de controle: "Pausar" ou "Retomar" e "Cancelar a execução" (vermelho, separado). Só existem
como texto, não desenhe: a frase "2 etapas estão rodando ao mesmo tempo.", "Um comando espera aprovação no
computador." e a confirmação "Confirmar o cancelamento" com "Sim, cancelar a execução" e "Deixar rodando".
USE só esses dados.
```

## M4 — Celular — Conectar este celular

```text
TELA: a primeira tela, antes de conectar. Topo só com "Atlas". Título "Acompanhe seus workflows e responda o
que está esperando por você.". Dois passos numerados, cada um com um número grande à esquerda e uma frase:
"1 No computador, abra o Atlas e escolha “Celular”." e "2 Escaneie o QR Code com a câmera deste celular ou
digite o código abaixo.". Campo "Código de pareamento" (placeholder "ABCD-EFGH-IJKL-MNOP", em mono; sem teclado na tela), botão primário "Conectar" em largura total (estado "Conectando…"), e a nota "O código vale uma vez
e expira em minutos. Nada sai da sua rede.". O erro "Esse código não é válido. Confira ou peça um novo no computador." (faixa vermelha) e o estado
"Conectando…" só existem como texto: não desenhe.
```

## M5a — Celular — Sem conexão

```text
TELA: topo só com "Atlas" (sem o nome do celular). No centro, uma faixa vermelha com o texto "Não consigo
falar com o computador. Ele pode estar desligado, em suspensão ou fora da rede. As execuções não são
afetadas; esta página continua tentando." e, abaixo, o botão primário "Tentar de novo" em largura total.
Nada mais.
```

## M5b — Celular — Sem execuções

```text
TELA: o painel com o topo completo ("Atlas" e "iPhone do Lucas") e, no centro, o texto "Ainda não há
execuções. Inicie um workflow no computador e ele aparece aqui.". No fim da tela, o link "Desconectar este
celular" e o aviso "O Atlas roda no seu computador. Esta página só funciona enquanto ele estiver ligado,
acordado e na mesma rede.". Nada mais.
```

## D1 — Computador — Acesso pelo celular

(Usa o P0 **normal**; `deviceType: DESKTOP`. Trocar no P0 a moldura por: "Barra lateral de 216px, de cima
a baixo da janela: logo Atlas no topo (só a palavra e o ícone, sem "Core", sem quadrado com letra); … A barra
lateral NÃO tem botão "Novo Fluxo" nem nenhum outro botão" e "Barra superior de 48px, só à direita da barra
lateral"; o rodapé mostra "Project Atlas v0.1.5" sem ponto de estado. Colar também o bloco "AJUSTES DESTE
MÓDULO" de Specs, sem as linhas de celular.)

```text
TELA: a tela principal do workspace esmaecida ao fundo e, sobre ela, o modal "Acesso pelo celular" com duas
colunas e sem rolagem interna em 1440×900. Cabeçalho: título, a frase "Acompanhe suas execuções de workflow e
responda o que espera por você pelo navegador do celular, na mesma rede. Nada passa pela internet nem por uma
nuvem.", o estado "● Ligado" com o botão "Desligar" (antes de ligar: "Desligado" + "Ligar"), e a faixa âmbar
"1 pergunta espera uma resposta.".
Coluna esquerda, "Conectar um celular": o QR code grande, "Escaneie com a câmera do celular. O código vale
uma vez e expira em 5 minutos.", o código "K7QD-M2PX-9WTR-4HNB" em mono, "Expira em 4:00", o botão "Novo
código", o endereço "http://192.168.1.20:47831" com "Copiar" e, quando há mais de uma interface, o seletor
(rede local recomendada, VPN).
Coluna direita: "Celulares pareados" com "iPhone do Lucas — de 192.168.1.31 · visto por último 22:23" e o
botão "Desconectar" (vermelho, secundário) e "Desconectar todos"; abaixo, a seção recolhível "O que saber"
(fechada) com a primeira frase visível: "Só funciona enquanto este computador estiver ligado, acordado e com o
Atlas aberto.".
USE só esses dados. NÃO mude: os textos de segurança da seção "O que saber" (ficam dentro da seção).
Só o estado "Ligado" é desenhado; "Desligado" + "Ligar" e o seletor de interface (que só aparece com mais de
uma rede) existem só como texto.
```

## Registro de envios

| Prompt         | Enviado | Tela gerada no Stitch | Observação |
| -------------- | ------- | --------------------- | ---------- |
| M1 | 2026-10-16 | "Celular — Painel de execuções" `353aa8a106af44408c2dc337f053d82f` (canvas 2560×2048, coluna de celular centralizada) | `generate_screen_from_text` com `deviceType: MOBILE` e bloco comum; respondeu sem timeout. |
| M2 | 2026-10-16 | "Celular — Execução esperando resposta" `068a7930a0ec4643ad87d2ea16a2f904` (canvas 2560×2048, coluna de celular) | `generate_screen_from_text` com `deviceType: MOBILE`; respondeu sem timeout. |
| M3 | 2026-10-16 | "Celular — Execução: etapas e resultado" `35e8c067f9364cdfa829f555d3c89832` (canvas de celular, 780×1768 na captura) | `generate_screen_from_text` com `deviceType: MOBILE`; timeout, não reenviado, confirmado por `list_screens`. |
| M4 | 2026-10-16 | "Celular — Conectar este celular" `6c341e698dc048c28b6782f9af6cd74c` (canvas 2560×2048, coluna de celular) | `generate_screen_from_text` com `deviceType: MOBILE`; respondeu sem timeout. |

## Avaliação

Observação geral: com `deviceType: MOBILE` o Stitch devolveu a tela marcada como DESKTOP, num canvas de
2560×2048, com a página do celular como uma coluna estreita (~390px) centralizada sobre fundo preto. Serve
como referência; as medidas valem pela coluna, não pelo canvas.

### M1 — Celular — Painel de execuções

Fiel: topo com "Atlas" e "iPhone do Lucas", faixa âmbar "Esperando sua resposta" com "Dispensar", quatro
grupos com contagem e ponto de cor (o de ação em destaque), cartões de três linhas com selo à direita e a
linha do que importa em cor, e o fim da lista com o aviso e o link. Sem barra de status nem teclado.

O Stitch mostra, o app não tem (não implementar):

- Títulos de grupo em caixa alta ("PRECISA DE VOCÊ (1)"): usar o título normal do app.
- Descrições de tarefa inventadas nos três cartões sem texto no prompt ("Auditoria de conciliação bancária…",
  "Exportação em lote de faturas…", "Suporte completo ao tema escuro…"); usar a tarefa real.
- Cartões com borda colorida por estado (âmbar, vermelha) além do selo: pode ficar só no de "Precisa de
  você", que é o que pede ação.

Desvio de ordem: no fim da lista o aviso "O Atlas roda no seu computador…" veio antes do link "Desconectar
este celular"; o prompt pedia o link primeiro. Manter a ordem do app.

### M2 — Celular — Execução esperando resposta

Muito fiel e a melhor tela do módulo até aqui: topo com "‹ Voltar", cabeçalho com título, selo e metadados
em mono, a tarefa numa caixa recolhível, a pergunta como cartão âmbar com faixa lateral (tipo, pergunta e
plano com "Mostrar o texto inteiro"), seções "Etapas" e "Controlar a execução" recolhidas (o "Cancelar a
execução" fica dentro da segunda, separado), e a barra fixa no pé com a nota recolhida, "Aprovar" em largura
total e "Pedir ajustes" / "Refazer o plano" (vermelho) lado a lado. Sem barra de status nem teclado.

O Stitch mostra, o app não tem (não implementar):

- "Atlas" colado ao "‹ Voltar" no meio da barra de topo (o prompt manda "Atlas" à esquerda e o nome do
  celular à direita; em telas internas o "‹ Voltar" fica à esquerda e "Atlas" logo depois: manter assim só
  se o app já faz).
- A caixa da tarefa mostra "Tarefa" e o texto cortado com reticências; ao abrir deve mostrar o texto inteiro.
- Ícone "+" no botão "Uma nota para o agente (opcional)".

Pontos positivos a manter: a barra de ações fixa, o "Refazer o plano" com contorno vermelho (não preenchido,
para não competir com "Aprovar") e a seção "Controlar a execução" fechada por padrão. "Mostrar menos" e as
mensagens condicionais não foram desenhadas (só descritas).

### M3 — Celular — Execução: etapas e resultado

Muito fiel, e desta vez o canvas veio em formato de celular (diferente de M1 e M2). Mesmo topo e cabeçalho,
a pergunta reduzida a uma linha âmbar "Developer espera você" com seta, "Etapas" aberta com a linha do
tempo (Architect ✓ "Concluída", Developer ? "Esperando você" com "Depois: QA", QA ○ "Não iniciada"; nome,
selo e agente · duração por linha, "O que ela relatou" recolhível) e as quatro seções fechadas com selo
("Até agora", "0 arquivos alterados (+0 −0)") ou sem selo. Sem barra de ações, como pedido.

Sem invenções relevantes. Observações: o ícone "‹" do "Voltar" virou chevron grande e o "Atlas" fica ao lado
(igual a M2); o marcador de cada etapa é um círculo com ícone (✓, ?, ○), o que combina com o pedido. Os
textos condicionais e a confirmação de cancelar não foram desenhados (só descritos).

### M4 — Celular — Conectar este celular

Exatamente o pedido e sem nenhuma invenção: topo só com "Atlas", título, dois passos com número grande à
esquerda, campo "Código de pareamento" em mono com o placeholder, "Conectar" em largura total no pé e a nota
embaixo. Sem QR code desenhado, sem teclado e sem barra de status (o reforço funcionou).

Observação de layout: entre o campo e o botão há um vazio grande, porque o botão ficou ancorado no pé da
tela (alcance do polegar). É intencional e deve ser mantido no app. O erro e o estado "Conectando…" não
foram desenhados (só descritos): implementar a faixa vermelha acima do campo e trocar o rótulo do botão.
