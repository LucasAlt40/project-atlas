# ADR 0034 — Acesso móvel: acompanhar e responder execuções a partir de um celular na rede local

Status: aceito. Baseia-se na [ADR 0013](0013-workflow-engine-and-orchestrator.md) (execuções, eventos), na
[ADR 0016](0016-human-in-the-loop.md) e na [ADR 0031](0031-plan-approval.md) (o que espera por uma pessoa).

## Contexto

Uma execução de workflow para quando um agente precisa de uma pessoa (um plano para aprovar, uma pergunta) e a pessoa
pode estar longe do computador. A execução pertence ao Atlas Desktop; nada fora do app consegue vê-la ou respondê-la.
Queremos que um **navegador de celular** acompanhe execuções e responda o que está esperando, sem nada na nuvem, sem app
nativo e sem um segundo engine de workflow.

## O que o código mostra

| Pergunta                                 | Resposta                                                                                                                                              |
| ---------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| Fonte da verdade de uma execução         | `WorkflowExecution` no `config.json`, gravada pelo `Orchestrator` a cada transição (`WorkflowService::execution/executions`).                         |
| Como uma pessoa responde                 | `Orchestrator::answer_interaction(run, interaction, answer, observer)`; o engine revalida dentro do loop da execução (`NotPending` em uma corrida).   |
| Como funcionam os controles              | `Orchestrator::control(run, Pause/Resume/Cancel, observer)`.                                                                                          |
| O que a UI ouve                          | `WorkflowObserver` (`workflow:*`), `ExecutionObserver` (`execution:progress`), adaptadores em `commands/events.rs`. Eventos carregam ids, não estado. |
| Onde os serviços são ligados             | `build_state` (Rust puro, sem Tauri); os adaptadores específicos do Tauri são criados em commands/`setup`.                                            |
| Aprovações de comando (`ApprovalBroker`) | Só pela UI, por desenho (ADR 0003/0007): autorizam um comando de processo.                                                                            |
| Frontend                                 | React 19 + Vite, tokens de design em `styles/global.css`, `Markdown`, i18n tipado (pt-BR/en-US). Uma entrada (`index.html`).                          |

## Decisões

### 1. O servidor é um adaptador sobre serviços existentes

`application/remote/` guarda as regras (pareamento, sessões, níveis de autorização, o modelo de leitura, o hub de
eventos); `infrastructure/remote_server/` é um adaptador HTTP; `commands/remote.rs` é a forma de a UI do desktop
ligá-lo. O gateway **chama** `WorkflowService` / `Orchestrator`; não contém orquestração. Uma resposta remota segue
exatamente o caminho de uma resposta do desktop, então o desktop a reflete pelo mesmo observador e pelos eventos
`workflow:*`, e uma corrida desktop/celular é decidida pelo engine (`NotPending`), não pelo transporte.

### 2. Pilha HTTP: hyper 1 sobre Tokio, ambos já na árvore de dependências

O Tauri já liga Tokio e hyper. Habilitamos `server`/`http1` do `hyper` e escrevemos um roteador pequeno; nenhum
framework web é adicionado. Os novos crates são pequenos e de propósito único: `if-addrs` (listar interfaces de rede nos
três SOs), `qrcode` (QR como SVG, sem pilha de imagens) e `getrandom` (CSPRNG do SO).

### 3. Tempo real: Server-Sent Events com cursor, não WebSocket

O tráfego é unidirecional (servidor → celular); as escritas são `POST`s autenticados comuns, o que também lhes dá
novas tentativas, códigos de status e idempotência de graça. O SSE reconecta sozinho e envia `Last-Event-ID`. Os eventos
são **dicas, não estado**: `{seq, kind, runId}`. O cliente reage relendo o recurso REST, então a fonte da verdade é
sempre o armazenamento, nunca o fluxo:

- O hub guarda um anel dos últimos 512 eventos com `seq` monotônico. Ao conectar, o `Last-Event-ID` do cliente é
  comparado com ele; se o cursor for mais antigo que o anel (ou de uma inicialização anterior do servidor) o fluxo
  começa com um evento `resync`, e o cliente recarrega tudo.
- Duplicatas e reordenação não podem corromper nada: tratar um evento significa "ler de novo".
- Um heartbeat de 15 s mantém a conexão viva através de proxies e deixa o navegador perceber uma rede morta; revogar um
  dispositivo também acorda o seu fluxo de imediato.
- A página também lê de novo em `visibilitychange`, quando a rede volta e a cada 30 s como rede de segurança, e quando o
  fluxo cai ela pergunta direto ao computador após 2 s: nenhuma resposta é mostrada como "não consigo alcançar o
  computador", não como um "reconectando" sem fim.

### 4. UI móvel: uma segunda entrada do mesmo projeto Vite, servida pelos assets do próprio app

`mobile.html` + `src/mobile/` (pequena, sem roteador, sem biblioteca de grafo) reaproveitam os tokens de design, o
renderizador de `Markdown` e o catálogo de i18n. Os arquivos compilados fazem parte do frontend embutido do Tauri, então
o servidor Rust os serve com `asset_resolver` (nenhum arquivo para distribuir, nada para desempacotar, a mesma versão do
app). Não é a UI do desktop encolhida para uma tela pequena: tem suas próprias telas (Painel, Execução, Plano/Pergunta)
feitas para toque.

### 5. Autenticação: código de pareamento de uso único → cookie de sessão

- Ligar o servidor cria um **código de pareamento**: 80 bits aleatórios (16 caracteres base32), válido por 5 minutos,
  **de uso único**. O QR codifica `http://<ip>:<porta>/#<código>`. O código está no **fragmento** da URL: nunca é
  enviado ao servidor na linha de requisição, nunca no `Referer`, nos logs ou no histórico do servidor; a página o
  remove com `history.replaceState` imediatamente.
- A página faz `POST /api/pair {code, deviceName}`. O servidor consome o código e responde com uma **sessão**: 256 bits
  aleatórios em um cookie `HttpOnly; SameSite=Strict; Path=/`. Só o SHA-256 do token é guardado (em memória).
  Expiração por inatividade de 24 h, absoluta de 7 dias.
- As sessões vivem **só em memória**: parar o servidor (ou fechar o Atlas) encerra todas. Cada ativação é uma nova
  autorização explícita; nenhum segredo é gravado em disco.
- Força bruta: 5 pareamentos falhos invalidam todo código pendente e travam o pareamento por 60 s (dobrando enquanto
  continuar); o espaço de códigos (2^80) torna o palpite inútil mesmo antes disso. Toda conferência de código é uma
  busca por hash, não uma comparação de string com o segredo.
- Revogação: um dispositivo, todos os dispositivos, "novo código" (invalida os anteriores não usados), desligar o
  servidor. Revogar envia aos fluxos abertos do dispositivo um evento `revoked` e os fecha de imediato.
- Tokens nunca aparecem em URLs, logs, erros ou eventos.

### 6. Proteções de rede

Uma LAN privada não é confiável, então toda requisição passa, em ordem, por:

1. **Verificação de Host** contra uma lista de permissão montada a partir do endereço ligado (`<ip>:<porta>`,
   `localhost`): derrota o DNS rebinding.
2. **Nenhum cabeçalho CORS** (somente mesma origem). Para escritas (`POST`) o cabeçalho `Origin`, quando presente, deve
   ser igual à origem permitida; um `Origin` ausente em uma escrita é recusado; e o corpo deve ser `application/json`
   (não um post de formulário).
3. **Autenticação** para tudo, exceto o shell estático e `POST /api/pair`.
4. **Limite de taxa** por sessão nas escritas e por IP de cliente no pareamento; corpo ≤ 16 KB.
5. As respostas levam `Cache-Control: no-store`, `X-Content-Type-Options`, `Referrer-Policy: no-referrer`,
   `X-Frame-Options: DENY` e uma CSP estrita (`default-src 'self'; connect-src 'self'; frame-ancestors 'none'`).

O servidor liga **um** endereço de interface escolhido pelo usuário (nunca `0.0.0.0` e nunca um endereço público por
padrão), somente enquanto habilitado, em uma porta de uma pequena faixa (alternativa se estiver ocupada). O Atlas não faz
redirecionamento de portas, UPnP nem tunelamento.

### 7. HTTP, não HTTPS

Um certificado autoassinado em um IP de LAN não é confiável para celulares; o navegador mostra um aviso de página
inteira que ensina o usuário a clicar para passar, e instalar uma CA privada em um celular não é uma configuração
aceitável. Certificados reais para um IP não são obteníveis sem um nome público e uma nuvem. Servimos, portanto, HTTP
simples e dizemos isso, e compensamos: pareamento de uso único e curta duração, cookie HttpOnly + SameSite, nenhum
segredo em URLs, CSP estrita, modelo de leitura sem segredos e ações em níveis (§8). A limitação é mostrada no diálogo
do desktop: **quem consegue capturar o tráfego na mesma rede (Wi-Fi aberto) pode ler o que o celular mostra e roubar sua
sessão**; use o recurso em uma rede em que confia, ou alcance o computador por uma VPN (WireGuard/Tailscale), que o
criptografa e não exige nada do Atlas. Um contexto seguro também não está disponível por HTTP, então a API de
Notification e service workers não são oferecidos (veja §10).

### 8. O que o celular pode fazer (níveis de autorização)

| Nível | Ação                                                                                                                                                                                                                                           | Remoto                                                                          |
| ----- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| 0     | Ler execuções, passos, eventos, planos, perguntas, resultados (redigidos)                                                                                                                                                                      | sim                                                                             |
| 1     | Responder uma interação de **plano**, **esclarecimento**, **aprovação**                                                                                                                                                                        | sim                                                                             |
| 2     | **Pausar / retomar** uma execução                                                                                                                                                                                                              | sim                                                                             |
| 2     | **Cancelar** uma execução (mantém o que concluiu)                                                                                                                                                                                              | sim, com confirmação explícita na requisição (`confirm: true`)                  |
| 3     | Uma pergunta de **permissão** ou de **confirmação de runtime**                                                                                                                                                                                 | só a resposta _recusando_ (`deny`/`no`): o remoto pode estreitar, nunca ampliar |
| 4     | Perguntas de guardrail (qual contexto vai a um modelo), aprovações de comando (`ApprovalBroker`), aplicar/manter/descartar código, retomar uma execução falha/interrompida, iniciar uma execução, entrada de terminal, arquivos, configurações | só no desktop — mostrado no celular como "precisa do computador"                |

Qualquer coisa que não está nesta tabela não existe como endpoint: não há proxy genérico de comandos do Tauri, nem
parâmetro de caminho, programa ou arquivo em lugar nenhum da API remota.

### 9. Modelo de leitura: uma projeção, não os objetos do domínio

`application/remote/view.rs` constrói DTOs a partir de `WorkflowExecution`: nomes em vez de caminhos de pasta, passos
com nome do agente, status, tentativas e horários, arestas (para mostrar a estrutura paralela/condicional), interações
(documento completo para as pendentes), eventos (mensagens como o engine as escreveu) e o resumo do resultado. Nunca
enviados: caminhos de projeto, caminhos de worktree, ambiente, segredos MCP, saída bruta de terminal, diffs. Todo texto
livre vindo de agentes (documentos de plano, resumos, mensagens de evento) passa pela redação de segredos existente
(`harness::secrets::redact_secrets`) — um plano pode citar um `.env` que leu. O progresso só é mostrado onde o engine o
conhece (passos concluídos de passos no grafo **não** é apresentado como porcentagem).

### 10. Notificações

A v1 entrega alertas **enquanto a página está aberta**: um banner de atenção na página, o título da aba do navegador
(`(2) Atlas`) e um painel com pendências primeiro. Notificações do navegador e push precisam de um contexto seguro e de
um serviço de push; explicitamente não são prometidas. O hub já emite eventos tipados, então outro canal é um novo
assinante, não um redesenho.

### 11. Ciclo de vida

- Desligado a cada inicialização; nunca persistido como "ligado".
- `RemoteAccess` guarda o estado `disabled → starting → active | error`. Um problema de bind é informado com um código
  (`no_network`, `address_unavailable`, `port_unavailable`, `bind_failed`). Uma mudança de rede é percebida pelo diálogo
  do desktop, que lê o status a cada segundo: o status diz `addressLost` quando o endereço ligado deixou de ser um dos do
  computador, e o diálogo pede para desligar e ligar de novo (as sessões terminam então; não há rebind silencioso).
- O servidor roda no runtime assíncrono, à parte das threads do orquestrador: uma queda ou parada dele não afeta
  nenhuma execução. Parar aborta toda conexão e descarta todas as sessões. O Atlas saindo faz o mesmo.
- Suspensão/desligamento: o fluxo do celular quebra, a página mostra "computador inacessível" com a hora da última
  atualização e tenta de novo; não promete monitoramento.

## Não feito na v1

HTTPS/push, início remoto de execuções, aplicar código pelo celular, responder remotamente a concessões de
guardrail/permissão, dispositivos persistentes entre reinicializações, contas multiusuário, terminal remoto.

## Verificação

- Rust: testes unitários de `application/remote/*` (tokens, pareamento, sessões, hub, política, ranking de interfaces) e
  `application/workflow/remote_tests.rs`, que executam o servidor real em um socket de loopback contra um `Orchestrator`
  real conduzindo uma execução roteirizada.
- Frontend: `src/mobile/*.test.*` (pareamento, painel, execução, respostas, conflitos, offline, revogação, alertas) e
  `src/features/remote/*.test.tsx` (o diálogo).
- A página do celular também foi aberta no navegador embutido do app em uma viewport de celular, servida pelo servidor
  real sobre dados reais (pareamento pelo link, painel, plano, aprovação, atualização ao vivo, servidor morto →
  "reconectando"). O caminho de assets empacotados (`asset_resolver`) é coberto por um teste que lê a página do frontend
  embutido.
- **Não verificado aqui:** um celular físico em uma rede Wi-Fi; builds de Windows e Linux (todo crate adicionado é
  multiplataforma, mas só o macOS foi compilado e executado); um app empacotado (`tauri build`) com o servidor ligado
  pelo diálogo.

## Consequências

- Uma nova superfície HTTP no app. Tudo o que ela pode fazer está em `application/remote/policy.rs` e nas rotas de
  `remote_server.rs`; uma nova rota ou ação é uma nova linha ali e um novo teste, não um novo proxy.
- O `orchestrator_tests` expõe alguns auxiliares ao módulo de testes irmão (`pub(super)`); o código de produção não muda,
  exceto pelas derivações de observador em `commands/events.rs`, a entrada `RemoteAccess` do `AppState` e os novos
  comandos.
