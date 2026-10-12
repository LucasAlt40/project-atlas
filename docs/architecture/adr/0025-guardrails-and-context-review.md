# 0025 — Camada de Otimização, fase 5: Guardrails e Revisão de Contexto

## Contexto

A fase 5 pede guardrails contextuais ("dados o agente, a tarefa, o contexto, o runtime e a política, isto pode
acontecer?") e uma revisão do contexto que um agente está prestes a receber. A regra que governa as duas coisas: **há
uma única autoridade de segurança** (`application::security`), e nada que um agente, uma skill, um handoff ou um arquivo
do projeto diga pode ser essa autoridade.

## Inventário: o que o `security/` já era

| Necessidade             | O que já existia                                                                                                                                         |
| ----------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Respostas               | `PermissionDecision` (`Allowed` / `Denied` / `RequiresApproval`) = ALLOW / DENY / ASK; `Permission` (negado < aprovação necessária < permitido)          |
| Por quê                 | Códigos `Reason` que a UI redige; `Evaluation`                                                                                                           |
| Política                | `SecurityPolicy`, `restrict` (toda camada só pode estreitar), perfis (`read_only`, `developer`), `ToolAccess::ceiling` (capacidade do runtime como teto) |
| Quem pode iniciar o quê | `GuardedProcessRunner` + `evaluator` (caminhos, symlinks, shells, classes de git, rede, ambiente)                                                        |
| Auditoria               | `AuditLog`, `PermissionEvent` (guardados em `Execution::permission_events`, transmitidos à UI)                                                           |
| Aprovação               | `ApprovalBroker` para aprovações de processo; `PendingInteraction` para perguntas que pausam um passo                                                    |
| Segredos                | `harness::secrets::redact_secrets`                                                                                                                       |
| Ponto de confinamento   | `SandboxProvider` (hoje só `NoSandbox`)                                                                                                                  |
| Concessão de edição     | `edit_access`: a política permite escrever + worktree isolado + capacidade do runtime                                                                    |

O que **não** existe: qualquer noção de _autonomia_ (Manual / Guiado / Auto). Não existe tipo, configuração nem caminho
de código para isso. O briefing da fase diz para respeitar os modos existentes; como não há nenhum, nenhum foi
inventado. As decisões abaixo não recebem um modo de autonomia, e um `ASK` é possível exatamente onde uma pessoa pode
responder (um passo de workflow). Introduzir autonomia é uma decisão de produto a ser tomada à parte (veja "Decisões
pendentes").

Também não é verdade sobre a superfície de ferramentas: o Atlas não consegue se interpor ao que um runtime de CLI faz
dentro do seu processo. `Read`, `Grep`, `Edit` e `Write` rodam no loop de ferramentas do próprio Claude; o Atlas vê o
prompt que envia e a resposta que lê (ADR 0023, 0024). Então um guardrail "antes de cada ação" ou "antes de cada
escrita" não existe para essas ferramentas: o que o Atlas controla é o que ele concede (a lista de ferramentas, o
worktree) e o que ele repassa. Os guardrails, portanto, atuam nos dois momentos que o Atlas possui: **antes de o agente
começar** e **antes de o resultado de um passo ser repassado**.

## Decisão

**Uma autoridade, estendida e não duplicada.**

- `ALLOW` / `ASK` / `DENY` são `PermissionDecision` com outro nome (`GuardrailDecision::from`). `TRANSFORM` é a única
  resposta nova.
- Toda avaliação é um `PermissionEvent` na única trilha de auditoria da execução (novas ações `ReviewContext`,
  `EditFiles`, `ShareResult`; novo desfecho `Transformed`; novos códigos `Reason`). O id da regra, as regras casadas e a
  descrição de qualquer transformação viajam como notas. **Nunca conteúdo**: os alvos são nomes (`context`, `skills`), e
  o próprio segredo nunca é citado.
- Um `ASK` é um `PendingInteraction` (tipo `Permission`, botões Allow/Deny fixados pelo Atlas), com um novo
  `DetectionSource::Guardrail`. Nenhum segundo sistema de aprovação.
- Política, permissão, capacidade e o guard permanecem intocados. `restrict` continua só estreitando.

**Decisões** (`security/guardrails.rs`, todas funções determinísticas das suas entradas):

| Situação                                                                             | Resposta                         | Regra / motivo                                       |
| ------------------------------------------------------------------------------------ | -------------------------------- | ---------------------------------------------------- |
| Contexto `Healthy` ou `Partial`                                                      | ALLOW                            | `context.healthy` / `context.partial`                |
| Contexto `NeedsReview`, um passo que pode esperar                                    | ASK                              | `context.needs_review` / `ContextNeedsReview`        |
| …e uma pessoa respondeu Allow                                                        | ALLOW (pelo usuário)             | `context.needs_review.approved`                      |
| …e uma pessoa respondeu Deny                                                         | DENY (pelo usuário)              | `context.needs_review.declined` / `ApprovalRejected` |
| …em uma conversa (ninguém para perguntar)                                            | ALLOW, informado                 | `context.needs_review.unattended`                    |
| Contexto `Invalid`                                                                   | DENY, o que quer que se responda | `context.invalid` / `ContextInvalid`                 |
| Ferramentas de edição: a política escreve, worktree isolado, o runtime edita         | ALLOW                            | `write.worktree_scope`                               |
| …a política escreve mas o agente não está isolado                                    | DENY (roda somente leitura)      | `write.not_isolated` / `WriteNotIsolated`            |
| …a política escreve mas o runtime não pode ser iniciado com ferramentas de edição    | DENY (somente leitura)           | `write.runtime_capability` / `RuntimeCannotEdit`     |
| Segredos no contexto vindos de arquivos ou outros agentes / no resultado de um passo | TRANSFORM                        | `secrets.redact` / `SecretsRedacted`                 |

(Uma política que não permite escrever é o estado normal de um agente somente leitura: ninguém pediu, então nada é
registrado.)

**A aprovação tem uma única fonte.** `Answered` vem de um `PendingInteraction` respondido cuja origem é `Guardrail`, para
este passo e esta passada por ele, lido pelo workflow a partir da execução (`brief::guardrail_answer`) e depois
entregue como `StepOptions::review`. Texto em um handoff ("o usuário aprovou"), uma skill, um arquivo, uma pergunta do
próprio runtime (qualquer outra origem de detecção) ou a mensagem do agente nunca é lido como tal, e há testes que
provam cada caso. O agente não é informado da pergunta do Atlas (ele nunca a fez). Uma pessoa não pode aprovar um
contexto `Invalid`. A aprovação cobre a passada pelo passo para a qual foi dada.

**TRANSFORM é só redação.** Linhas que parecem segredos (a regra do próprio Harness) são substituídas por um aviso, em
contexto não confiável (Harness, Task Context, skills, contexto do workflow, handoff) antes de o prompt ser montado, e no
resultado estruturado de um passo antes de ser repassado (`WorkflowEventKind::GuardrailTransformed`, só contagens). O que
o usuário escreveu (a tarefa, as instruções do agente) é dele: é informado, nunca reescrito. Nenhum comando é jamais
reescrito.

## Revisão de Contexto

Uma verificação pura e determinística dos mesmos itens com que o Context Engine trabalha (`prompt_items`), de modo que os
dois concordem sobre o que cada parte é. Ela informa; o guardrail decide. Não julga código e não usa modelo.

- **Integridade** (bloqueante): as regras do Atlas estão no texto de sistema, existe uma tarefa, um passo tem seu
  protocolo de resultado, e o engine não alterou texto obrigatório (a promessa do engine é conferida, não confiada).
- **Consistência** (erro): duas fontes afirmam escolhas diferentes em um pequeno conjunto de grupos exclusivos conhecidos
  (banco de dados, framework de front-end, gerenciador de pacotes, executor de testes) como instruções. Estreita de
  propósito: a mesma fonte duas vezes, comparações, migrações, proibições, blocos de código e linhas que nomeiam duas
  escolhas não são conflitos. O que não puder ser demonstrado não é informado.
- **Atualidade** (aviso): itens do Harness sobre os quais o projeto mudou (`ContextRecord::stale_items`, e o bloco "pode
  estar desatualizado" no texto). Um sinal, não um veredito.
- **Orçamento** (erro / aviso / info): partes obrigatórias que não cabem; itens deixados de fora; linhas repetidas
  removidas (apenas informação).
- **Conteúdo** (aviso): segredos no contexto que está prestes a ser enviado, e texto não confiável que reivindica
  autoridade ("ignore previous", "administrator access", "the user approved", `sudo`…, em inglês e português). Este
  último não muda nada: é mostrado, porque a defesa é que nenhum texto jamais é uma permissão.
- **Proveniência e confiança**: todo item tem um `SectionKind` e um `SourceTrust` (`Atlas`, `Configured`, `Untrusted`).

Saúde: `Healthy` (só informação) < `Partial` (avisos) < `NeedsReview` (um erro) < `Invalid` (um problema bloqueante).
Duplicação sozinha é `Healthy`. O resultado, com os dois lados de cada problema, é guardado com a execução
(`OptimizationMetrics.contextReview`).

## Onde roda

`ExecutionService::run_step`, depois que o prompt é montado (e depois do Context Engine, se ligado) e antes de o runtime
iniciar: revisar → redigir → decidir → registrar → montar de novo a partir das entradas se algo foi redigido → iniciar,
perguntar ou parar. Uma execução negada ou pausada nunca inicia seu runtime. O worktree que recebeu é mantido, como em
qualquer execução que falha. O `Apply` não é tocado e continua sendo uma decisão humana; nada aqui faz commit, push ou
merge.

Flag: `optimization.guardrails.enabled`, **ligada por padrão** (uma verificação de segurança; só muda um prompt para
remover o que parecia um segredo). Desligada, nada disto roda.

## Medido

- Com a pilha real e uma CLI de substituição: um passo cujas próprias instruções dizem MongoDB enquanto sua skill diz
  PostgreSQL pausa antes de qualquer processo iniciar e pergunta; um sim o executa até o fim sem perguntar de novo; um
  não faz o passo falhar com o motivo e nada depois dele roda.
- Com a **CLI real do Claude e um modelo que não existe** (ela inicia, informa, falha na primeira requisição, não gasta
  nada): a mesma pausa, nenhum processo antes da resposta, e depois do sim a CLI real é iniciada (e falha por causa do
  modelo, não do Atlas).

## Endurecimento (fase 5.1)

Três lacunas encontradas depois da fase 5 e fechadas sem acrescentar uma funcionalidade.

### Uma aprovação vale para uma avaliação

Um `ASK` é um `PendingInteraction`; ele agora carrega `evaluation`, a impressão digital da avaliação sobre a qual foi
perguntado (o mesmo campo em `InteractionDetection`, então nada novo existe ao lado da interação). A impressão digital é
um digest do que a avaliação olhou: a etapa, o id, a origem e o conteúdo de cada item de contexto, os achados da
revisão, o agente, o runtime, o modelo e as capacidades, as políticas **resolvidas** do agente (não os arquivos de onde
vieram), o worktree e o acesso de edição concedido. Exclui o id da execução, porque o passo é executado de novo (uma
nova execução) depois da resposta.

A resposta chega ao guardrail como `ReviewAnswer { answered, evaluation }`. `brief::guardrail_answer` só a devolve para
uma pergunta feita nesta execução de workflow, por este passo, nesta passada, **pela tentativa que está sendo executada
de novo** (a pergunta de uma tentativa mais antiga não é desta) e que carrega uma avaliação (um registro de antes desta
mudança não aprova nada). O guardrail então a descarta a menos que `evaluation` seja igual à impressão digital que acabou
de calcular; uma resposta descartada significa que a pergunta é feita de novo, e o evento diz que a aprovação anterior
não vale mais (`staleApproval`). Uma resposta recusada é vinculada da mesma forma. Um contexto inválido continua sendo
negado qualquer que seja a resposta.

Testes: o mesmo contexto na sua resposta passa (A); um texto de tarefa alterado (B), uma política alterada (C) e um
handoff alterado (D) perguntam de novo; uma resposta de outro agente não é deste agente; uma pergunta do passo A não vale
nada para o passo B (E); de uma passada nada para outra (F); de uma tentativa anterior, de outra execução, ou sem uma
avaliação, nada (G).

Limite: a impressão digital é um digest de dados que o Atlas montou, não uma assinatura. Ela impede uma aprovação
obsoleta; não defende contra quem já consegue reescrever os arquivos de execução do Atlas.

### O ChangeSet é revisado antes do Apply

`security/changeset.rs` revisa o que uma execução colocaria no projeto, arquivo por arquivo, de forma determinística:

| Achado                                                                                                                     | Saúde         | Resposta |
| -------------------------------------------------------------------------------------------------------------------------- | ------------- | -------- |
| `.git` como qualquer componente (também o caminho antigo de um arquivo renomeado), qualquer caixa ou separador             | `Invalid`     | `DENY`   |
| caminho absoluto, letra de unidade, `../` que sai do projeto                                                               | `Invalid`     | `DENY`   |
| um arquivo cuja localização real (seguindo links, também uma pasta ligada acima dele) está fora                            | `Invalid`     | `DENY`   |
| `.atlas/` na raiz do projeto                                                                                               | `NeedsReview` | `ASK`    |
| `.env`, `.env.*` (não `.example`/`.sample`/`.template`/`.dist`), credenciais, chaves privadas, `.ssh/`, `.aws/credentials` | `NeedsReview` | `ASK`    |
| uma linha adicionada que parece um segredo (o scanner do Harness, `redact_secrets`; só linhas adicionadas)                 | `NeedsReview` | `ASK`    |
| um arquivo que não pôde ser lido ou um link que não resolve dentro                                                         | `NeedsReview` | `ASK`    |
| nada                                                                                                                       | `Healthy`     | `ALLOW`  |

Reaproveitado, não reinventado: `security/paths.rs` (`NormalizedPath`, `is_inside_physical`) para escapes,
`harness/secrets.rs` para conteúdo. Não havia política por caminho para a qual adiar, então a regra do `.atlas/` é
fixada aqui; nenhuma foi adicionada. A revisão nomeia um caminho e um código, nunca conteúdo.

Ela roda no `IntegrationService::apply`, pelo mecanismo de bloqueio que já existe. `DENY` → a execução fica `Blocked`
com o novo `BlockReason::ProtectedPaths`, o Apply não é oferecido e tentar de novo não muda nada (Manter isolado e
Descartar continuam funcionando). `ASK` → o primeiro Apply é respondido com `Blocked`/`NeedsReview` e os achados; o
próximo Apply da pessoa **sobre a mesma impressão digital da revisão** (revisões, arquivos, achados) é a decisão dela.
Código alterado é uma revisão diferente e é segurado de novo. `ALLOW` → o Apply segue como antes. Em todo caso o Apply é
um ato de uma pessoa, **nunca faz commit**, e as demais verificações (conflitos, checkout sujo, política) são as
existentes: uma revisão saudável não sobrepõe um conflito, e um ChangeSet sem arquivos não é aplicado (a execução diz que
não há nada para aplicar). A revisão é guardada na execução (`integration.review`). Se as mudanças não puderem ser lidas
para serem revisadas, o Apply fica retido (`Undetermined`, pode ser tentado de novo): a revisão falha de forma fechada.

O que não pôde ser exercitado de ponta a ponta: o Git não informa caminhos `.git/…` nem caminhos absolutos / `../` em um
change set real (ele se recusa a adicioná-los), então esses são provados só pelos testes da própria revisão. Symlinks
(inclusive um para `/etc`), `.env`, `.atlas/`, linhas de segredo, vários arquivos de uma vez, conflitos e o change set
vazio são provados contra worktrees Git reais.

A auditoria desta decisão é o próprio registro da execução (`integration.review`, o evento `IntegrationChanged`); não
está no `AuditLog` por execução, que é indexado por uma execução e uma `PermissionAction` e não tem nenhuma para isto.

### `--restricted` para o Claude: investigado, não adotado

Claude CLI 2.1.285. Seu texto de ajuda: `--restricted` remove as ferramentas embutidas que executam comandos ou código
(Bash, PowerShell, REPL…) e o `WebFetch`, a menos que `--tools` as nomeie, e ignora os arquivos de configuração de
usuário, projeto e locais (as configurações gerenciadas e `--settings` ainda se aplicam); ele também confina as
ferramentas de arquivo aos diretórios de trabalho, protege escritas em arquivos de configuração, do Git e de ferramentas,
e recusa `bypassPermissions` (esses três pontos são o próprio texto de ajuda da CLI; **não observados aqui**, veja
abaixo).

Experimento, sem modelo e sem tokens: a CLI real iniciada em um diretório vazio com `--model not-a-real-model-xyz` e
`< /dev/null`, que imprime seu `system/init` e falha na primeira requisição com uso 0 e custo 0. Cada configuração do
Atlas (ferramentas somente leitura, ferramentas de edição com `--permission-mode acceptEdits`, sem ferramentas) com e sem
`--restricted`, mais a flag sozinha e com as flags de isolamento do Atlas e sem `--tools`:

| Observado no próprio `init` da CLI                                          | Atlas hoje                                            | Atlas + `--restricted`                                                                                                                        |
| --------------------------------------------------------------------------- | ----------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| ferramentas, somente leitura / edição / nenhuma                             | 3 / 5 / 0                                             | 3 / 5 / 0 (idêntico)                                                                                                                          |
| servidores MCP, slash commands, skills                                      | 0, 0, 0                                               | 0, 0, 0                                                                                                                                       |
| modo de permissão                                                           | igual                                                 | igual                                                                                                                                         |
| plugins                                                                     | 4: o `atlassian` e o `figma` do usuário e 2 embutidos | só os 2 embutidos                                                                                                                             |
| caminho da memória automática (o `~/.claude/projects/…/memory/` do usuário) | informado                                             | ausente                                                                                                                                       |
| erro                                                                        | modelo não reconhecido, saída 1                       | o mesmo                                                                                                                                       |
| sem `--tools`: `Bash`, `CronCreate`, `Monitor`, `RemoteTrigger`, `WebFetch` | expostos                                              | removidos                                                                                                                                     |
| a flag sozinha, sem as outras flags do Atlas                                | —                                                     | 32 ferramentas, 6 servidores MCP do claude.ai, 55 slash commands, 19 skills: não substitui `--strict-mcp-config` / `--disable-slash-commands` |

Conclusões: com a lista de permissão explícita `--tools` que o Atlas já passa, o `--restricted` não muda nenhuma
ferramenta, nenhum servidor MCP e nenhum slash command; quem faz esse trabalho é a lista de permissão, o
`--strict-mcp-config` e o `--disable-slash-commands`, e o `--restricted` não os substitui. O que ele muda está fora da
lista de ferramentas: os plugins do usuário não são carregados e as configurações e a memória do usuário não são lidas.
Isso é um ganho real de isolamento (um plugin ou um arquivo de configuração do usuário deixa de alcançar uma execução que
o Atlas inicia) que foi **observado apenas como o estado declarado de inicialização da CLI**.

Não observado, portanto não afirmado: que as ferramentas de arquivo são confinadas ao diretório de trabalho, que
escritas em arquivos do Git ou de configuração são recusadas, como o `--restricted` se comporta em uma execução que de
fato chama ferramentas, se o login por OAuth/keychain funciona sob ele (a requisição que falhou era a verificação do
modelo, antes de a autenticação ser exercitada), e o que os hooks ou regras de `permissions` do próprio usuário, nos
arquivos de configuração ignorados, faziam por ele (por exemplo, um hook de que ele dependa deixaria de se aplicar). Cada
um desses pontos precisa de uma chamada de modelo.

**Decisão: C, investigar depois.** Não adotado. A evidência diz que ele pode acrescentar isolamento dos plugins,
configurações e memória do usuário e nada além, e a parte que mais importaria (o confinamento das ferramentas de arquivo
em uma execução com edição, e o login) não pode ser demonstrada sem gastar tokens. O próximo passo é uma pequena
execução real, com o aval do usuário, em um passo com edição habilitada em um worktree descartável: pedir que escreva
fora da pasta e que toque no `.git`, com e sem a flag, e verificar o login.

## Limitações

- Não existem modos de autonomia; nenhum foi adicionado.
- Nenhum guardrail "antes de cada ação / escrita" para as ferramentas próprias de um runtime de CLI (veja acima). O
  confinamento de escrita para elas é o que o Atlas concede (lista de ferramentas, worktree isolado) mais as regras da
  própria CLI; o `--restricted` foi investigado na fase 5.1 e não adotado.
- A revisão do ChangeSet é baseada em caminhos e padrões: um segredo que não parece um, ou uma mudança nociva em um
  arquivo comum, não é detectado. Ela barra os arquivos do Git, escapes e os arquivos sensíveis óbvios; não julga se a
  mudança é boa.
- Um `ASK` existe só em passos de workflow. Em uma conversa, um contexto que precisa de revisão é informado e a execução
  segue.
- O detector de conflitos conhece quatro grupos; a maioria das contradições reais não é detectável por regra e não é
  informada.
- A detecção de reivindicação de autoridade e de segredos são listas de padrões: deixarão passar paráfrases. São
  sinais; a segurança não se apoia neles.
- Uma aprovação é vinculada à avaliação sobre a qual foi perguntada (fase 5.1), e a uma tentativa de uma passada de um
  passo.
- As métricas e a revisão são guardadas por execução; não há dashboard (fase 7).

## Decisões pendentes

1. Se vamos introduzir autonomia (Manual / Guiado / Auto) e o que cada uma significa para o `ASK`.
2. `--restricted` para o Claude: decisão C, é preciso uma pequena execução real (veja acima).
3. Estender o léxico de conflitos a partir de projetos reais.
