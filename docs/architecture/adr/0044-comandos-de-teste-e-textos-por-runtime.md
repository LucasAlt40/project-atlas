# ADR 0044 — Comandos de teste para os agentes, textos de passo conforme o runtime e plano fora do diff

Status: aceito e implementado. Saiu do benchmark de 2026-10-30 (`bench/`, planilha em
`bench/results/2026-10-30/benchmark-atlas.xlsx`). Complementa a [ADR 0024](0024-runtime-surface-and-isolation.md)
(superfície e isolamento do Claude), a [ADR 0025](0025-guardrails-and-context-review.md) (revisão de contexto) e a
[ADR 0038](0038-macos-sandbox.md) (sandbox do macOS).

## Contexto

O benchmark comparou o Atlas com o Claude Code e o Codex usados direto, em quatro tarefas com testes ocultos. Três achados
mudavam o resultado:

1. **Os textos dos passos diziam a todo agente "You cannot run commands".** As regras do sistema já distinguiam o
   runtime que trabalha por comandos (`ATLAS_RULES_EDITING_COMMANDS`), mas o texto do passo contradizia. No Codex, cuja
   única forma de ler um arquivo é um comando, isso fez um agente não ler nada e desistir em 15 s, e outro consertar três
   vezes às cegas enquanto o validador rodava `npm test` e reprovava: Atlas+Codex concluiu 6 de 8, o Codex puro 8 de 8.
2. **No Claude nenhum agente rodava os testes** (`Read, Grep, Glob, Edit, Write`): tudo era julgado lendo.
3. **A regra de plano mandava salvar `docs/plans/<nome>.md` no worktree.** O arquivo entrou na mudança em 7 de 12
   execuções de código, inclusive quando a tarefa dizia "não toque em nenhum outro arquivo".

## Decisões

### 1. Um texto de passo nunca afirma o que as ferramentas permitem; ele remete às regras do Atlas

Os passos que mudam código dizem: _se as suas regras do Atlas permitem rodar comandos, rode o comando de teste do projeto
depois da mudança, conserte o que falhar e relate exatamente o que ele imprimiu; se dizem que você não pode, nunca diga
que testes, build ou linter passaram e liste os comandos que deveriam ser rodados_. Os que julgam (QA, verificação,
revisão) confirmam rodando quando podem e marcam cada evidência como lida ou executada. A frase é uma só
(`verify_change!`, `judge_evidence!` em `template_text.rs`), vale para templates salvos pela pessoa e para qualquer
runtime, e quem decide o "pode" é a regra do sistema daquela execução. Um teste proíbe "You cannot run commands"
incondicional em qualquer passo.

### 2. O Claude ganha `Bash` só para os comandos de teste, dentro do sandbox da própria CLI

Rodar um teste executa código do projeto que o próprio agente pode ter editado. Por isso são três camadas, e a política
continua sendo a única autoridade:

- **Quando:** a execução já edita num worktree isolado (`allow_edits`), o runtime declara `runs_granted_commands()` (o
  Claude, só no macOS) e a política do agente tem `processes.mode = Allowed`. Os comandos são os de
  `executions::granted_commands::TEST_COMMANDS` cujo programa está em `processes.allowed_commands` (`npm test` exige
  `npm`). No perfil `developer` isso dá `npm/pnpm/yarn test`, `cargo test`, `dotnet test`, `mvn/gradle test`,
  `python -m pytest`; `node --test`, `pytest` e `go test` ficam de fora até a pessoa liberar o programa. O
  `runtime_access` da execução passa a declarar `process_execution`, e o guard confere contra a política e audita.
- **O quê:** `--tools ...,Bash` e um `--settings` com `permissions.allow` = cada comando, sozinho ou com argumentos.
  Em `-p` qualquer outro comando precisaria de aprovação e é recusado. Vai em `--settings`, valor único, porque
  `--allowedTools` aceita vários valores e engoliria o prompt quando ele é o último argumento (execução em terminal).
- **Onde:** o sandbox da CLI (`sandbox.enabled`) com `autoAllowBashIfSandboxed: false` (estar no sandbox não aprova um
  comando), `allowUnsandboxedCommands: false` (nunca tenta de novo fora), `failIfUnavailable: true` (falha em vez de rodar
  sem confinamento), nenhum domínio de rede e leitura negada dos mesmos cofres de credenciais da ADR 0038.

O prompt troca a regra por `ATLAS_RULES_GRANTED_BEFORE` + a lista + `ATLAS_RULES_GRANTED_AFTER`. São duas partes fixas para
que a revisão de contexto (ADR 0025) continue conferindo o texto obrigatório. Sem isso, toda execução de edição do Claude
seria barrada por `missing_required`, e um teste de ponta a ponta prova que não é.

**Com o sandbox do Atlas ligado** (ADR 0038), um segundo Seatbelt dentro do primeiro é recusado (medido no Codex).
`ClaudeConfinement::adapt` desliga o sandbox da CLI e mantém as regras de permissão. O confinamento passa a ser o perfil
do Atlas, que deixa a rede aberta, como já acontece com o Codex. Auditoria: `sandbox:replaces_cli_sandbox:claude`.

### 3. O plano fica na mensagem, não no worktree

`PLAN_RULE` continua exigindo o plano inteiro em Markdown na mensagem (o Atlas mostra, notifica e guarda com a execução),
mas não pede mais o arquivo `docs/plans/<nome>.md`, a não ser que a tarefa ou as convenções do projeto peçam. O mesmo
texto vale para os acordos de trabalho que o gerador de Harness escreve. Harness já gerados por versões anteriores
continuam com o texto antigo até a pessoa regenerá-los.

### 4. Com aprovação automática de planos, o passo não para para pedir aprovação

O protocolo de cada passo mandava o agente parar a cada plano com um bloco `plan`, mesmo quando a pessoa escolheu aprovar
planos automaticamente (`auto_approve_plans`). O Atlas então respondia "approve" e rodava o passo inteiro de novo, do
zero, porque o runtime não guarda conversa. No benchmark de 2026-10-31 o Arquiteto rodou duas vezes em todas as
execuções do "Implement" no Codex e em metade das do Claude, a ~50 a 100 mil tokens de entrada por volta. Com a
aprovação automática ligada, `interaction_protocol` diz ao agente que o plano já está aprovado: ele o escreve inteiro
na mensagem (a pessoa continua lendo) e segue na mesma execução até o bloco de resultado. Sem a aprovação automática,
nada muda. Se um agente ainda assim pedir aprovação, o caminho antigo continua valendo. O mesmo protocolo deixou de
pedir o plano salvo como arquivo, que tinha ficado para trás na decisão 3.

### 5. Quem julga reprova só pelo que foi pedido

Validador e QA do "Implement", a verificação do "Debug & fix" e o QA dos templates antigos mandam um `fail` para um
Corretor e abrem uma volta de dois passos. Com o teste executado, eles começaram a reprovar por defeitos reais, mas que
nenhum critério da tarefa pedia: `new String('high')` aceito como prioridade, um cupom `toString`, marcas combinantes
fora de U+0300–U+036F. Foram 1 volta no Claude e 3 no Codex em 8 execuções cada (`bench/`, rodada 2026-10-31b). Agora
reprovam só quando o comportamento pedido pela tarefa ou pelo plano está errado ou sem teste, quando algo que funcionava
quebrou, ou quando a mudança introduz uma falha de segurança. O resto vai como achado de baixa severidade com o resultado
`pass`, e a pessoa decide (`fail_only_for!`). A revisão ("Review & audit") não muda: ela não manda nada de volta e relatar
tudo é o objetivo dela.

Medido na rodada 2026-10-31c: as voltas caíram de 1 para 0 no Claude e de 3 para 2 no Codex, e as duas restantes citam
regras da própria tarefa (um comportamento pedido sem teste; "diacríticos são removidos" lido como todas as marcas
combinantes). A nota oculta seguiu em 100% nos dois.

### 6. Quem só lê também roda os testes, sem poder escrever em nada que vê

Validador e QA são passos só de leitura e, no Claude, julgavam só lendo, enquanto no Codex o sandbox somente-leitura já
deixava rodar os testes. Agora uma execução que só lê, de um agente em worktree isolado, recebe os mesmos comandos de
teste (`Read,Grep,Glob,Bash`, sem `acceptEdits`). O sandbox da CLI nega escrita (`denyWrite`) no diretório de trabalho
**e** em cada pasta extra que ela recebeu (`--add-dir`), porque o sandbox libera escrita nas pastas adicionadas. A regra
do prompt é a variante de leitura (`ATLAS_RULES_READ_GRANTED_BEFORE/AFTER`), reconhecida pela revisão de contexto. Com o
sandbox do Atlas ligado, `ClaudeConfinement::adapt` devolve `read_only`, para o perfil do Atlas manter o worktree sem
escrita. Medido (`claude -p` 2.1.308, haiku, uma execução, US$ 0,003, numa pasta fora do temp da CLI): o teste rodou e
passou; escrever na pasta de trabalho deu `EPERM`, assim como em `~`, na rede e em `~/.ssh`. Um teste que precise
escrever no projeto (`cargo test` em `target/`) falha nesse passo, e a regra manda dizer isso em vez de contornar.

### 7. Relatório curto; os detalhes ficam só no bloco de resultado

Medido por passo (o runtime do Claude agora registra `thinkingTokens`, `textChars` e `toolInputChars`; o do Codex já
registrava `reasoningTokens`): a saída de um passo vai ~40% para raciocínio interno, ~25% para o relatório (o texto mais
o bloco `atlas-result`, que repetia os mesmos achados) e o resto para ferramentas. A narração ao vivo é ~1%. O protocolo
de resultado pede agora um texto curto (o que fez ou concluiu, o veredito, o que não pôde verificar, o que a pessoa
precisa decidir) e cada achado, decisão e artefato uma vez só, no bloco, que o inspetor do workflow já mostra. O passo
cujas instruções pedem um relatório final para a pessoa continua escrevendo-o completo. O raciocínio interno não foi
mexido: limitá-lo é uma troca de qualidade que fica para outra decisão.

### 8. Git de leitura junto com os testes; a auditoria não roda comandos

Os comandos concedidos (`executions::granted_commands`) passaram a incluir `git status`, `git diff`, `git log` e
`git show` quando a política também tem `git.read = Allowed` e `git` está entre os programas permitidos: um Validador que
tentou conferir o escopo com `git status` era recusado. Eles rodam no mesmo sandbox (num passo só de leitura nada pode ser
escrito; o `git status` funciona sem conseguir atualizar o índice). O campo `ExecutionPolicy::no_commands` (só restringe,
como `read_only`) tira os comandos de um passo: o "Review & audit" o liga, porque rodar os testes só confirmava o que a
leitura já mostrava e deixou a revisão 40% mais cara. O formato portátil de workflows o carrega
(`noCommands`, escrito só quando ligado, então arquivos antigos leem igual).

Medido (rodada 2026-10-31f): a revisão voltou de 73 mil para 33 mil tokens de entrada. O Git de leitura não teve efeito
nesta máquina, porque o hook global do usuário (RTK) reescrevia `git status` para `rtk git status`, que não casa com a
regra. Por decisão do usuário, uma execução que recebe comandos agora roda com os hooks do usuário desligados
(`disableAllHooks` no `--settings`); as outras continuam com eles. A regra também passou a pedir cada comando sozinho
(sem `cd` antes, sem `&&`, `;` ou pipe), porque um `npm test` encadeado foi recusado.

### 9. O "Quick change" corrige uma vez

Medido nas tarefas de feature (T1, T3, T5, T7, 8 execuções cada): o "Quick change" (Arquiteto → Desenvolvedor → QA)
entregou o mesmo código correto que o "Implement" (100% nos testes ocultos) com −31% de custo e −30% de tempo. Mas em 2
de 8 o QA reprovou por um problema legítimo e, sem Corretor, a execução terminou como falha com o código bom no worktree.
Agora uma reprovação abre **uma** volta: o Desenvolvedor (o mesmo agente, com o texto do Corretor) corrige e o QA roda
de novo; o passo de correção pode começar uma vez só (seu limite de laço), então uma segunda reprovação encerra a
execução como falha (`fix_once_gate`). Um QA que não declara veredito (agente geral) mantém o caminho simples, sem
passo de correção. O template continua precisando só de Arquiteto, Desenvolvedor e QA.

### 10. Um passo calado é visível e deixa rastro

Uma execução ficou parada (CLI sem CPU, conexões com a API fechadas) e foi encerrada à mão sem deixar nada para ler. O
limite de inatividade (`RUN_TIMEOUT`, 15 min desde a última saída) funciona e é coberto por testes; o que faltava era
diagnóstico. Agora o erro de tempo esgotado carrega um `Silence` (há quanto tempo estava calado e as 10 últimas linhas,
até 300 caracteres cada), que vai nos detalhes técnicos da falha, nos caminhos de pipe e de terminal. Na interface, a
barra da execução e a lista de execuções ativas mostram "Sem saída há X" depois de 2 minutos sem nenhum evento; o texto
ao vivo conta como atividade, atualizado no máximo a cada 10 s para não renderizar a cada token. Sem rede, a CLI do
Claude imprime `system/api_retry` e desiste depois de 10 tentativas (~3 min), o que também conta como saída para o
limite do Atlas.

## O que foi medido e o que não foi

- Uma execução real do `claude -p` (2.1.308, haiku, US$ 0,003) com estes `--settings`: `node --test` rodou; dentro dele,
  escrever em `~` deu `EPERM`, a rede falhou e listar `~/.ssh` deu `EPERM`; `ls -la`, fora das regras, foi recusado.
  Um `echo > ../arquivo` gravou na pasta temporária do próprio Claude (`/private/tmp/claude-<uid>`), que o sandbox da CLI
  libera.
- Os hooks do usuário (`~/.claude/settings.json`) continuam valendo nas execuções do Atlas. No ambiente medido, um hook
  reescreveu `ls` para `rtk ls`. O sandbox é o que limita um comando, aprovado por regra ou por hook.
- **Não medido:** o Claude com o sandbox do Atlas ligado (a troca segue a do Codex por analogia) e Linux/Windows (lá o
  Claude continua sem shell).
