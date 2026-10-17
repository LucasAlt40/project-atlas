# Revisão de design — Celular (acesso pelo celular)

Módulo 6 da repaginação (processo em [`../_comum/README.md`](../_comum/README.md)). Reúne duas coisas
ligadas pelo ADR 0034:

- **A página do celular** (`mobile.html`, `src/mobile/`): serve o app do próprio computador para o
  navegador de um celular na mesma rede. Acompanha as execuções de workflow e responde o que espera
  por você (aprovar plano, perguntas, permissões), pausa, retoma ou cancela. Não inicia execuções,
  não concede permissões, não aplica código.
- **O diálogo "Acesso pelo celular" do computador**: liga e desliga o servidor, mostra o QR e o código
  de pareamento e quem está conectado.

Capturas em [`atual/`](atual/). A página do celular foi capturada em 390×844 (as imagens têm o dobro de
pixels: 780 de largura). Harness: `_comum/captura/mobile.html` +
`mobile-capture.tsx` (simula `fetch` e `EventSource` com os dados de exemplo do próprio app):
`?phase=ok|unpaired|offline`, `?runs=0..4`, `#/run/run-1` abre uma execução (a 1 espera um plano;
a 3 falhou; a 4 terminou). O diálogo do computador usa `?remote=off|on|error` no harness desktop e o
botão "Celular" da barra superior.

## 1. Moldura

A página do celular **não tem** a moldura do desktop. Barra de topo própria: "Atlas" + nome do celular
(e `‹ Voltar` nas telas internas). O diálogo do computador é um modal dentro da moldura do desktop.

## 2. Telas, estados e interações

### 2.1 Celular — painel **[img 01, 02]**

- **Topo**: "Atlas" e o nome do celular ("iPhone do Lucas"); faixas de alerta (`Banners`): "Esperando sua
  resposta", "Precisa de você no computador", "Uma execução falhou", "Uma execução terminou", cada uma com
  `Dispensar`; faixa de conexão ("Não consigo falar com o computador…", "Reconectando…", "Última atualização
  …").
- **Grupos**, cada um com título e contagem: `PRECISA DE VOCÊ` (âmbar), `EM ANDAMENTO`, `COM FALHA`
  (vermelho), `TERMINADAS RECENTEMENTE`.
- **Cartão de execução**: nome do workflow, selo (`? Esperando você`, `● Executando`, `✕ Falhou`, `✓
Concluída`, `Pausada`, `Cancelada`, `Interrompida`), workspace, a tarefa, bloco de aviso (âmbar: "Uma
  pergunta espera sua resposta" / "N perguntas esperam sua resposta" / "Precisa de você no computador";
  vermelho: o motivo da falha), etapa atual ("Developer · Dev agent"), "1 de 3 etapas concluídas" e a
  duração ou data.
- **Rodapé**: `Desconectar este celular` (link) e o aviso "O Atlas roda no seu computador. Esta página só
  funciona enquanto ele estiver ligado, acordado e na mesma rede."
- Estados: carregando, vazio ("Ainda não há execuções. Inicie um workflow no computador e ele aparece
  aqui."), sem conexão, reconectando. O título da aba mostra "(1) Atlas" quando há pergunta esperando.

### 2.2 Celular — execução **[img 03, 04]**

Abre ao tocar um cartão (`#/run/<id>`). Topo com `‹ Voltar`. Título do workflow + selo; linha "Transport ERP ·
iniciada 22:10 · 14m 06s"; a tarefa numa caixa; "1 de 3 etapas concluídas".

- **Pergunta esperando** (`InteractionCard`): cartão âmbar com o tipo ("PLANO PARA APROVAR · DEVELOPER · DEV
  AGENT"), a pergunta ("Você aprova este plano?"), o documento (markdown, `Mostrar menos` / `Mostrar o texto
inteiro`), "Uma nota para o agente (opcional)" e três botões empilhados: `Aprovar` (primário), `Pedir
ajustes`, `Refazer o plano` (vermelho). Outros tipos: Pergunta (campo "Sua resposta" + `Enviar resposta`),
  Aprovação (Aprovar / Rejeitar), Pedido de permissão (Permitir / Negar), Confirmação (Sim / Não). Avisos:
  "Esta pergunta é respondida no computador", e o resultado do envio ("Enviado. O computador confirmou.").
  Respondidas antes: seção "Respondidas antes".
- **Controlar a execução**: `Pausar` / `Retomar` e `Cancelar a execução` (vermelho, com confirmação "Confirmar o
  cancelamento" → `Sim, cancelar a execução` / `Deixar rodando`).
- **Etapas** (`Timeline`): lista vertical com marcador por etapa (✓, ?, ○), nome, agente, selo ("Concluída",
  "Esperando você", "Não iniciada"), horário e duração, "Depois: QA", "O que ela relatou" (recolhível),
  "N tentativas", "passada N", "N etapas estão rodando ao mesmo tempo".
- **Resultado** ("Até agora" / "Resultado"), **Código** ("N arquivos alterados (+A −R)"; "Revisar e aplicar o
  código é feito no computador."), **Registro de atividade** (recolhido), avisos "Um comando espera
  aprovação no computador." e "A recuperação é feita no computador."

### 2.3 Celular — parear **[img 05]**

Primeira vez ou sessão perdida: "Atlas", a frase "Acompanhe seus workflows e responda o que está esperando
por você.", dois passos numerados (abra o Atlas e escolha "Celular"; escaneie o QR Code ou digite o código),
campo "Código de pareamento" (placeholder `ABCD-EFGH-IJKL-MNOP`), `Conectar`, nota "O código vale uma vez e
expira em minutos. Nada sai da sua rede." Erros: código inválido, bloqueado, tentativas demais, limite de
celulares, sem conexão, sessão revogada ou expirada.

### 2.4 Celular — sem conexão e vazio **[img 06, 07]**

Sem conexão: "Não consigo falar com o computador. Ele pode estar desligado, em suspensão ou fora da rede. As
execuções não são afetadas; esta página continua tentando." + `Tentar de novo`. Vazio: ver 2.1.

### 2.5 Computador — Acesso pelo celular (modal) **[img 08]**

Botão "Celular" na barra superior (com bolinha âmbar e contagem quando há pergunta esperando). O modal:
título, introdução ("Acompanhe suas execuções de workflow e responda o que espera por você pelo navegador do
celular, na mesma rede. Nada passa pela internet nem por uma nuvem."), estado (`Ligado` + `Desligar`; antes:
`Desligado` + `Ligar`; `Iniciando…`; erro com motivo), aviso "1 pergunta espera uma resposta.", **QR code** +
"Escaneie com a câmera do celular. O código vale uma vez e expira em 5 minutos." + código de pareamento em
mono + "Ou abra http://192.168.1.20:47831 e digite o código." + "Expira em 4:00", `Novo código`, o endereço
(com seletor quando há mais de uma interface: rede local, VPN) e `Copiar`; **Celulares pareados** (nome,
"de 192.168.1.31 · visto por último 22:23", `Desconectar`; `Desconectar todos`); **O que saber** (lista longa
do que funciona e do que não: só enquanto o computador estiver ligado e acordado; HTTP simples em rede local;
o que o celular pode e não pode fazer; avisos de VPN e de endereço perdido).

## 3. Problemas de design observados

1. **Cartão de execução com cinco camadas** (título + selo, workspace, tarefa, aviso, etapa atual, progresso
   e hora): o que pede ação (a pergunta) é só mais um bloco no meio.
2. **Texto repetido**: "Developer · Dev agent" em todo cartão, mesmo quando a etapa é irrelevante ao que
   a pessoa precisa decidir; "1 de 3 etapas concluídas" e a duração no mesmo rodapé fino.
3. **Selos de formatos diferentes** (`? Esperando você`, `● Executando`, `✕ Falhou`, `✓ Concluída`) e blocos de
   aviso de bordas diferentes (âmbar, vermelho).
4. **Execução com tudo empilhado**: pergunta, controle, etapas, resultado, código e registro, em sequência,
   com a pergunta (o motivo da visita) já ocupando a primeira tela inteira e os botões saindo da área visível.
5. **Botões de resposta empilhados no meio da rolagem**: `Aprovar`, `Pedir ajustes`, `Refazer o plano` ficam
   no fim do cartão, não ao alcance do polegar; a nota opcional fica antes dos botões.
6. **`Cancelar a execução` no fluxo normal** logo abaixo da pergunta, com o mesmo formato de botão da
   resposta.
7. **Etapas com muita informação** (agente, data, duração, depois, relato) em cada linha.
8. **Pareamento e erros só em texto**: sem hierarquia entre o passo a passo, o campo e a nota.
9. **Modal do computador muito longo**: QR, código, endereço, lista de pareados e "O que saber" de 10 pontos
   numa rolagem só.

## 4. Telas do Stitch que já existem

Nenhuma. Todas as telas deste módulo são novas (a página do celular nunca foi desenhada no Stitch).

## 5. Lacunas

Não capturados: execução em falha e terminada, pergunta de texto, aprovação, permissão, confirmação,
resultado e código, erros de parear, sessão expirada, reconectando, alerta de pergunta nova, desktop com o
servidor desligado, com erro e sem celulares pareados.
