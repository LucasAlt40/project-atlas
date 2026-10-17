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
| M5     | Celular — Sem conexão e sem execuções  | MOBILE      |
| D1     | Computador — Acesso pelo celular       | DESKTOP     |

Ordem: M1 → M2 → M3 → M4 → M5 → D1.

## M1 — Celular — Painel de execuções

```text
TELA: o painel inicial. Barra de topo com "Atlas" e "iPhone do Lucas". Logo abaixo, uma faixa de alerta
dispensável (só quando há): "Esperando sua resposta" com o botão "Dispensar".
A lista de execuções agrupada por urgência, cada grupo com título e contagem; os grupos que pedem ação
vêm primeiro e em destaque: "Precisa de você (1)" (âmbar), "Em andamento (1)", "Com falha (1)"
(vermelho), "Terminadas recentemente (1)".
Cada execução é um cartão compacto de 3 linhas, e não de 6: (1) nome do workflow em destaque e o selo de
estado à direita ("Esperando você" âmbar, "Executando" azul, "Falhou" vermelho, "Concluída" verde);
(2) o workspace e a tarefa numa linha ("Transport ERP · Implementar a recuperação de senha por e-mail",
truncada); (3) a linha do que importa agora — para "Precisa de você": "Uma pergunta espera sua resposta"
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
menores lado a lado: "Pedir ajustes" e "Refazer o plano" (vermelho). Antes do primeiro toque em "Pedir
ajustes" sem texto, a mensagem "Diga o que mudar e toque em “Pedir ajustes” de novo.".
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
resposta"), rolada até as seções abaixo da pergunta, agora como seções recolhíveis com um selo no cabeçalho
de cada uma: "Etapas" (aberta), "Resultado" ("Até agora"), "Código" ("0 arquivos alterados (+0 −0)") e
"Registro de atividade" (fechadas).
"Etapas": linha do tempo vertical com marcador por etapa — Architect ✓ "Concluída" ("Arch agent · 30s", com
"O que ela relatou" recolhível), Developer ? "Esperando você" ("Dev agent · 21:17", "Depois: QA") e QA ○ "Não
iniciada" ("QA agent"). Menos informação por linha: nome, selo e, na segunda linha, o agente e a duração; o
resto só ao tocar. Quando várias etapas rodam juntas, a frase "2 etapas estão rodando ao mesmo tempo.". Em
"Código": "Revisar e aplicar o código é feito no computador.". Se um comando espera aprovação: "Um comando
espera aprovação no computador.". Rodapé da seção de controle: "Pausar" ou "Retomar" e "Cancelar a execução"
(vermelho; ao tocar pede "Confirmar o cancelamento" com "Sim, cancelar a execução" e "Deixar rodando").
USE só esses dados.
```

## M4 — Celular — Conectar este celular

```text
TELA: a primeira tela, antes de conectar. Topo só com "Atlas". Título "Acompanhe seus workflows e responda o
que está esperando por você.". Dois passos numerados, cada um com um número grande à esquerda e uma frase:
"1 No computador, abra o Atlas e escolha “Celular”." e "2 Escaneie o QR Code com a câmera deste celular ou
digite o código abaixo.". Campo "Código de pareamento" (placeholder "ABCD-EFGH-IJKL-MNOP", em mono, teclado
de letras), botão primário "Conectar" em largura total (estado "Conectando…"), e a nota "O código vale uma vez
e expira em minutos. Nada sai da sua rede.". Mostre também, num estado alternativo, o erro em uma faixa
vermelha: "Esse código não é válido. Confira ou peça um novo no computador.".
```

## M5 — Celular — Sem conexão e sem execuções

```text
TELA: duas telas lado a lado no mesmo quadro. (a) Sem conexão: topo só com "Atlas"; faixa vermelha "Não
consigo falar com o computador. Ele pode estar desligado, em suspensão ou fora da rede. As execuções não
são afetadas; esta página continua tentando." e o botão primário "Tentar de novo". (b) Sem execuções: topo
completo, e no centro "Ainda não há execuções. Inicie um workflow no computador e ele aparece aqui.", com o
link "Desconectar este celular" e o aviso do rodapé.
```

## D1 — Computador — Acesso pelo celular

(Usa o P0 **normal**; `deviceType: DESKTOP`.)

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
```

## Registro de envios

| Prompt         | Enviado | Tela gerada no Stitch | Observação |
| -------------- | ------- | --------------------- | ---------- |
| (nenhum ainda) |         |                       |            |
