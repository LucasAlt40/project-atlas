# ADR 0030 — Templates de workflow: propósito, ferramentas e cópias salvas

Status: aceito. Baseia-se na [ADR 0013](0013-workflow-engine-and-orchestrator.md), na
[ADR 0015](0015-result-contracts-outcomes-and-worktree-lock.md) e nas [ADR 0028](0028-mcp-foundation.md) /
[0029](0029-mcp-for-every-runtime.md); não muda nenhuma das suas invariantes.

## Contexto (auditado no código, não presumido)

- Um template eram oito trechos de código que construíam cada um um grafo com instruções de uma ou duas frases. Sem
  objetivo, sem "pronto quando", sem ferramentas, sem saídas, nada para ler antes de começar, e nenhum template próprio
  da pessoa: um workflow personalizado não podia ser guardado como ponto de partida.
- **Nenhum agente do Atlas executa comandos**: o prompt de edição diz "Você não pode executar comandos de shell" e o
  Claude é iniciado com `Read,Grep,Glob,Edit,Write`. As instruções antigas ("execute ou revise os testes") convidavam um
  agente a relatar uma execução de testes que nunca aconteceu.
- O MCP estava completo no núcleo (conexões, concessões por agente / workflow / passo, um plano por runtime) e ausente
  dos workflows: o inspetor do nó tinha uma caixa de seleção, as concessões só eram alcançáveis pelo hub, e o prompt
  nomeava servidores mas não as suas ferramentas.
- Um nó não podia estreitar as permissões do seu agente, então um template "somente leitura" dependia de o perfil do
  agente ser somente leitura.

## Decisões

1. **Os três templates em destaque reaproveitam os ids existentes** (`software_feature` → Implementer, `bug_fix` →
   Debugger, `code_review` → Reviewer), de modo que workflows guardados e o modo Automático continuem funcionando. Os
   outros cinco continuam como variações. Suas instruções vivem em `workflow/template_text.rs`.
2. **Um passo diz quando está pronto** (`AgentNode.completion_criteria`, dito ao agente depois das suas instruções com
   "diga o que falta em vez de relatar como feito") e **pode ser restrito à leitura** (`ExecutionPolicy.read_only`).
   Ambos têm como padrão o comportamento antigo. `read_only` só estreita: faz a execução seguir o mesmo caminho de uma
   política que nega escrita (nenhuma ferramenta de edição chega ao runtime, o prompt diz somente leitura) e deixa um
   passo compartilhar o worktree com outros. Ele **não** limita ferramentas MCP: essas são para o que as concessões
   servem (a UI diz isso).
3. **Instruções honestas.** Todo passo que poderia ser tentado diz que não pode executar comandos, pede os comandos que
   alguém deveria executar, e que separe o que foi lido do que foi executado. As ferramentas são mencionadas só
   condicionalmente ("se uma integração lhe foi dada") e uma ausente deve ser relatada, não contornada. A
   proporcionalidade está no texto (o Architect abre com `Size: trivial|moderate|complex`, os passos seguintes se ajustam
   a ele) e no modo Automático, que propõe o rápido `feature_basic` para uma tarefa curta que se declara pequena.
4. **O Debugger é um passo corretor com duas fases** (evidência e causa raiz com uma confiança declarada, depois a menor
   correção com um teste de regressão), uma verificação somente leitura que escreve o relatório final, e uma saída: um
   corretor que declara `blocked` encerra a execução em um nó End **Inconclusive** (`EndOutcome::Failed`) em vez de
   adivinhar. A rota só existe quando o contrato do corretor declara `fixed`/`blocked` (do contrário o passo segue para
   a verificação), porque um nó que nada alcança é um workflow inválido. Um passo "investigar" separado foi rejeitado: o
   contrato do agente é dele, e uma investigação não pode terminar honestamente com `fixed`.
5. **O Reviewer** é um passo somente leitura; o template não consegue fazer um arquivo mudar, seja qual for o perfil do
   agente. Uma correção posterior é uma execução separada que a pessoa inicia.
6. **Ferramentas são sugestões até a pessoa aplicá-las.** Um template nomeia um _tipo_ de ferramenta por passo
   (`ToolCapability`: browser, issues, docs, repo), casado com as conexões do workspace pelo nome (um palpite, então só
   sugere). `template_tools::resolve` diz, por sugestão, qual conexão serviria com quais ferramentas, ou por que nenhuma
   pode (sem conexão, desligada, segredo ausente, não conseguiu conectar, ferramentas nunca listadas, sem ferramentas de
   leitura). `create_workflow_from_template(applyTools: true)` transforma as utilizáveis em concessões comuns para aquele
   agente, workflow e passo. Um passo que só _consulta_ recebe apenas as ferramentas da conexão cuja primeira palavra é
   um verbo de leitura, e só quando o servidor foi descoberto: sem descoberta as ferramentas de escrita não podem ser
   retidas, então nada é concedido e a prévia diz por quê. Um passo de navegador recebe o servidor inteiro (suas
   ferramentas agem sobre uma página). Nada é concedido sem a opção da pessoa, e só conexões que ela já ligou.
7. **O agente é informado a verdade sobre as suas ferramentas.** A nota de integração lista as ferramentas que uma
   concessão cobre (`jira (tools: getJiraIssue)`) e nomeia o que foi concedido e não pôde ser dado (`context7 (its
credential is missing)`), com um pedido para dizer isso no resultado em vez de adivinhar.
8. **Templates salvos são cópias.** `CustomTemplate` (campo de configuração `workflow_templates`, vazio por padrão)
   guarda os passos, rotas e as ferramentas concedidas aos passos de um workflow (por nome da conexão e seleção de
   ferramentas). Templates embutidos são código: não podem ser editados nem excluídos (`workflow_template_builtin`).
   Excluir um workflow agora também remove as concessões feitas aos seus passos.
9. **Três camadas, ditas na UI:** global (agentes, conexões MCP), template (passos, instruções, ferramentas sugeridas),
   execução (a tarefa, e edições no workflow). A prévia lista passos (quem, somente leitura, loop limitado, pronto
   quando, instruções), ferramentas com o seu estado neste workspace e as saídas antes de qualquer coisa ser criada. O
   inspetor edita os critérios e a flag de somente leitura e gerencia as concessões do passo pelos comandos de concessão
   existentes.

## Não feito / limites

- O casamento das sugestões é pelo nome da conexão; uma conexão com outro nome é concedida à mão no inspetor.
- `read_only` não governa ferramentas MCP; o filtro de verbos de leitura é um palpite a partir de nomes (o Atlas não
  conhece as descrições das ferramentas).
- Concessões de ferramentas nomeadas são recusadas onde o runtime não consegue reter ferramentas (OpenCode hoje): o
  servidor então não é dado e o agente é informado.
- Nenhum modelo real foi chamado: o comportamento é coberto pelos serviços reais sobre runtimes falsos e pela pilha de
  CLI de substituição. Quão bem cada instrução conduz um modelo real não foi medido.
- `LIVE_NARRATION` ainda diz para citar caminhos "em comandos de shell", que nenhum agente pode executar. É texto de
  produto e foi deixado como está.
