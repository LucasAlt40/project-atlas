# 8. Acesso pelo celular

> Parte do [guia do Atlas](README.md). ADR: [0034](../architecture/adr/0034-mobile-remote-access.md).
> Código: `src-tauri/src/application/remote/`, `src-tauri/src/infrastructure/remote_server.rs`, `src-tauri/src/commands/remote.rs`,
> `src/features/remote/` (diálogo no desktop) e `src/mobile/` (a página do celular, `mobile.html`).

Quando um workflow para esperando uma pessoa (um plano para aprovar, uma pergunta), quem não está diante do computador fica travado. O
**acesso pelo celular** deixa o navegador do celular acompanhar as execuções e responder o que espera, **na mesma rede**, sem nuvem, sem conta e
sem aplicativo. O Atlas Desktop continua dono de tudo: o servidor é só uma porta de entrada para os serviços que já existem.

## Como usar

1. No Atlas, clique em **Celular** no cabeçalho (qualquer tela).
2. Escolha o endereço (o recomendado já vem marcado) e clique em **Ligar**. Nada escuta a rede antes disso, e o acesso **nunca** volta ligado
   sozinho ao abrir o Atlas.
3. Aponte a câmera do celular para o QR Code. Ou abra o endereço mostrado e digite o código de pareamento.
4. O celular mostra o painel. Para desconectar um celular, **Desconectar** na lista; para encerrar tudo, **Desligar**.

O código de pareamento vale **uma vez** e **5 minutos**; **Novo código** invalida o anterior.

## O que o celular mostra e faz

| Tela           | Conteúdo                                                                                                                                                                                 |
| -------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Painel         | **Precisa de você** (perguntas esperando, coisas só do computador), **Em andamento**, **Com falha**, **Terminadas recentemente**; etapa atual, agente, "N de M etapas" (nunca um %)      |
| Execução       | Estado, workspace, início e duração, tarefa, **linha do tempo das etapas** (paralelas, repetidas e ignoradas aparecem como são), plano/pergunta, resultado, falha, registro de atividade |
| Plano/pergunta | Texto do plano em Markdown, contexto, agente e etapa; aprovar, pedir ajustes (exige dizer o quê), refazer; responder em texto; nota opcional                                             |
| Alertas        | Faixa ao abrir a página quando uma pergunta passa a esperar, uma execução falha ou termina; o título da aba mostra `(n) Atlas`                                                           |

**Tabela de ações (ADR 0034 §8).** O celular pode _reduzir_ o que um agente pode fazer e responder o que se pergunta para seguir; não pode
_ampliar_ autoridade:

| Ação                                                                                                                                                                                                       | No celular                             |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------- |
| Ler execuções, planos, perguntas, resultado                                                                                                                                                                | sim                                    |
| Responder plano, pergunta ou aprovação                                                                                                                                                                     | sim                                    |
| Pausar / retomar                                                                                                                                                                                           | sim                                    |
| Cancelar a execução                                                                                                                                                                                        | sim, depois de confirmar               |
| Pedido de permissão / confirmação do runtime                                                                                                                                                               | só **recusar** (negar), nunca permitir |
| Pergunta de _guardrail_ (que contexto vai para um modelo), aprovação de comando, aplicar/descartar código, retomar execução interrompida ou com falha, iniciar execução, terminal, arquivos, configurações | **só no computador** (a página avisa)  |

Não existe endpoint genérico: não há caminho, programa ou arquivo em nenhum parâmetro da API remota.

## O que sai do computador

Nomes (workspace, workflow, agente, etapa), a tarefa, o texto dos planos e perguntas, resumos e mensagens do registro. **Nunca:** pastas do
computador, worktrees, variáveis de ambiente, chaves, saída bruta do terminal, diffs. O texto escrito por agentes passa pela mesma redação de
segredos do Harness (linhas que parecem segredo são removidas) e é limitado em tamanho.

## Segurança

- **Autorização explícita e desligado por padrão.** O servidor escuta **um** endereço que a máquina realmente tem (privado, VPN ou "só este
  computador"), nunca `0.0.0.0`, nunca um endereço público; porta de 47615 a 47634 (a primeira livre).
- **Pareamento:** código de 80 bits, uso único, 5 min, no _fragmento_ da URL do QR (o navegador não o envia ao servidor; a página o tira da
  barra de endereço de imediato). 5 erros seguidos invalidam o código e fecham o pareamento por 60 s (dobrando se continuar).
- **Sessão:** 256 bits aleatórios em cookie `HttpOnly; SameSite=Strict`; só o SHA-256 fica na memória. Expira em 24 h sem uso e em 7 dias no
  máximo. Máximo de 8 celulares. **Tudo em memória:** desligar (ou fechar o Atlas) encerra todas as sessões.
- **Cada requisição:** `Host` conferido (contra DNS rebinding); escritas exigem `Origin` do próprio servidor, `Content-Type: application/json`
  e `Sec-Fetch-Site` same-origin; sem CORS; autenticação em tudo menos a página e o pareamento; limite de escritas por celular e de
  tentativas de pareamento por endereço; corpo ≤ 16 KB; no máximo 12 conexões por endereço; cabeçalhos de segurança e CSP estrita.
- **Respostas verificadas no backend:** a decisão é gravada pelo `Orchestrator`; o celular mostra o que o computador confirmou. Repetir o mesmo
  envio (reply perdida) é idempotente; resposta diferente a pergunta já resolvida é `409`; desktop e celular ao mesmo tempo → uma decisão só.

## Limitações (ditas na interface também)

- **HTTP, não HTTPS.** Um certificado autoassinado em IP de rede local não é confiável para o celular (aviso de tela cheia) e instalar uma CA
  no celular não é um fluxo aceitável. Quem consegue capturar tráfego do mesmo Wi-Fi poderia ler o que o celular mostra e roubar a sessão: use
  uma rede de confiança, ou uma VPN (WireGuard/Tailscale — o endereço `100.64.0.0/10` aparece como "VPN" e a VPN criptografa).
  O cookie não tem `Secure` (o navegador o recusaria em HTTP) e, como cookies não isolam portas, outro serviço do mesmo IP poderia lê-lo.
- **Sem notificações do navegador / push:** exigem contexto seguro (HTTPS) e um serviço de push. A v1 alerta com a página aberta (faixa, título
  da aba, vibração quando o navegador deixa). As pendências ficam no painel de qualquer forma.
- **Depende do computador:** ligado, acordado e com o Atlas aberto. Em suspensão, o celular mostra "Não consigo falar com o computador" e a
  hora da última atualização; **nada é prometido** enquanto isso. As execuções não são afetadas pelo celular (nem por ele sumir).
- **Mudança de rede:** se o IP do computador mudar, o diálogo avisa; desligue e ligue para usar o novo endereço (as sessões acabam e é
  preciso parear de novo).
- **Em desenvolvimento** (`tauri dev`) o frontend não está embutido: use `ATLAS_MOBILE_DIST=<pasta dist>` (só em build de debug) depois de
  `npm run build`.

## Como a página se mantém consistente

A página lê o estado pelo REST (`/api/runs`, `/api/runs/{id}`); o fluxo SSE (`/api/events`) só avisa que **algo mudou** (`{seq, kind, runId}`,
nunca texto) e a página lê de novo. O hub guarda os últimos 512 avisos numerados; um celular que reconecta manda `Last-Event-ID` e recebe só o
que perdeu, ou um `resync` (leia tudo) se o cursor é de outra execução do servidor ou mais antigo que o guardado. Além disso a página relê ao
voltar para o primeiro plano, ao voltar a rede e a cada 30 s.

## Testes

`src-tauri/src/application/workflow/remote_tests.rs` sobe o servidor de verdade num socket local, com um orquestrador de verdade, e usa um
cliente HTTP cru: autorização, Host/Origin/CSRF, pareamento (uso único, expiração, trava), revogação, aprovação duplicada/em conflito/
concorrente com o desktop, política por tipo, cancelar com confirmação, fluxo SSE (resync, retomada sem repetir, revogação), isolamento
(desligar não toca nas execuções), porta ocupada, corpo grande, conexão ociosa e limite por endereço. No frontend: `src/mobile/*.test.*` e
`src/features/remote/*.test.tsx`.
