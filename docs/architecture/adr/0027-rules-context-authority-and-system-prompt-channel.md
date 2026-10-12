# 0027 — Rules, Autoridade de Contexto e o canal de system prompt (Context & Tooling Platform, Fase C)

## Contexto

A Fase B ([ADR 0026](0026-context-and-tooling-platform.md)) tornou a entrega verificável. A Fase C fecha a hierarquia de
autoridade sobre o que um agente é informado: o que é uma rule, quem pode defini-la, qual vence, o que pode instruir e o
que só pode informar, e como os guardrails enxergam tudo isso. A auditoria antes de construí-la encontrou: nenhum
conceito de rules em lugar algum (o Harness tem `constraints.md`, que é conhecimento, e `.atlas/policies/`, um marcador
não lido); `SourceTrust` com três níveis; uma revisão que já sinalizava reivindicações de autoridade em texto não
confiável e conflitos de escolhas conhecidas entre seções; uma impressão digital de avaliação sobre a origem e o texto de
cada item; `RuntimeCapabilities.system_prompt` como um booleano não usado. O desenho completo está em
[context-and-tooling-platform.md](../context-and-tooling-platform.md), seção 16.

## Decisões

1. **Uma Rule é um conceito de domínio e é apenas orientação.** `Rule { id, scope, owner, title, content, strength,
priority, enabled, topic, provenance, metadata }`; escopos Global, Project, Workspace, Workflow, Agent, Task; forças
   Mandatory, Preference, Informational. Rule, Permission, Policy e Guardrail respondem a quatro perguntas diferentes e
   vivem separadas (o `security/` guarda as três últimas). Uma rule não pode conceder, ampliar nem suspender nada; testes
   provam que uma rule que autoriza deixa `allow_edits` e o acesso a ferramentas como a política os definiu.
2. **Uma fonte por escopo, nenhum segundo armazenamento.** As rules Global/Workspace/Workflow/Agent são
   `UserConfig.rules`; as do projeto são `.atlas/context/rules.md`, um arquivo do usuário sob a convenção existente de
   `.atlas/context/`, lido pelo `HarnessStore::read_context` existente; as de uma tarefa chegam com ela. O
   `constraints.md` do Harness continua sendo conhecimento e não é copiado. Uma rule de projeto ou de tarefa na
   configuração é ignorada.
3. **A especificidade nunca vence um mandato.** A resolução é pura e determinística. Rules em um mesmo `topic` são
   alternativas: a Mandatory mais ampla governa, senão a Preference mais estreita; informação de fundo nunca governa; a
   perdedora não é enviada e um conflito é registrado. Uma rule mais estreita que contradiz uma obrigatória mais ampla é
   um achado que precisa de uma pessoa (ASK em um passo). Sem julgamento semântico: rules sem um tópico compartilhado se
   somam.
4. **A proveniência limita a força.** Somente as origens `User` e `ProjectFile` podem obrigar; `Generated` e `External`
   são mantidas como informação de fundo, seja o que for que reivindiquem, e a revisão diz isso.
5. **Rules são itens de contexto no pipeline existente.** `SectionKind::Rules`; Mandatory é `Required`, Preference
   `Normal`, Informational `Optional`; o Context Engine e o orçamento não mudam, então uma rule obrigatória nunca é cortada
   e, se a parte obrigatória não cabe, aplica-se o caminho `budget_exceeded` → ASK existente. Sem nenhuma rule o prompt é
   idêntico byte a byte ao de antes.
6. **Autoridade de Contexto** (`Authoritative`, `Instructional`, `Informational`, `Untrusted`) diz o que um texto pode
   fazer. É derivada da seção (e, para rules, da força e da origem), carregada por todo item, mostrada na revisão e nas
   métricas, e mantida consistente com `SourceTrust` (que fica, para revisões guardadas) por um teste. Ela não deriva
   nenhuma permissão.
7. **Os guardrails são estendidos, não duplicados.** Novos achados: `rule_conflict`, `unknown_provenance`, reivindicações
   classificadas por tipo (sobrepor, conceder permissão, desabilitar segurança, falsa aprovação, elevação; inglês e
   português). Uma reivindicação em uma rule é um Erro (ASK); em contexto não confiável continua sendo um Aviso. Nada é
   reescrito. A impressão digital da avaliação passa para a versão 2 e cobre a prioridade, a autoridade e a proveniência
   de cada item e a resolução das rules, de modo que o conteúdo, o estado habilitado, a proveniência, a força ou a origem
   de uma rule anule uma aprovação anterior.
8. **`SystemPromptChannel { Unsupported, Native, Appended }`** substitui o booleano não usado como capacidade do runtime;
   todo adaptador declara `Unsupported` e nenhum recebe uma divisão. `delivery()` só divide para um runtime que declara um
   canal (instruções e rules do Atlas ali, o resto no corpo), com hash das duas partes delimitadas pelo comprimento. O
   `--append-system-prompt` do Claude continua não adotado: seu comportamento no Windows e com escapes e seu efeito
   semântico não foram testados.
9. **O manifest é evidência, não uma métrica** (reverte a nota da Fase B da ADR 0026 que o ligava à flag de métricas):
   `Execution.plan` e `Execution.manifest` existem para toda execução que montou um prompt, com métricas ligadas ou não,
   e registram as rules e o que aconteceu com cada uma.

## C.1: endurecimento da rule obrigatória

Decidido depois da Fase C, porque o mecanismo de tópicos deixava uma lacuna: uma rule obrigatória e uma tarefa que a
contradiz, sem `topic` em comum, conviviam e o modelo escolhia.

10. **Uma rule obrigatória é obrigatória.** Um detector simples e determinístico (`review/directives.rs`) lê _faça_ /
    _não faça_ em torno de um pequeno vocabulário fechado (testes, commit, push, merge, comandos destrutivos, e "não
    mude nada fora de `<dir>`"), em inglês e português, contando negações ("não pule os testes" é um _faça_) e tratando
    hesitações ("if needed", "se necessário") como incertas. Um texto que diz claramente o oposto de uma rule obrigatória
    é um conflito **comprovado**.
    - vindo de uma fonte que **pode instruir** (a tarefa, as instruções do agente, outra rule): `rule_conflict`, Erro. Em
      um passo de workflow os guardrails fazem **ASK**; um "não" é um **DENY**; com **ninguém para perguntar** (uma
      conversa) é um **DENY** (`context.mandatory_unresolved`). Nunca um ALLOW não resolvido. Um "sim" de uma pessoa é
      uma aprovação vinculada à impressão digital da avaliação, como antes.
    - vindo de uma fonte que **só pode informar** (Harness, skill, handoff, uma rule de fundo): `rule_conflict`, Aviso.
      Esse texto não pode instruir, então não pode sobrepor a rule; é mostrado, e o passo não é parado por ele.
11. **Possível não é comprovado.** A mesma ação tocada em direções opostas, mas hesitante ou enredada em negações, é
    `possible_rule_conflict`, Aviso: não muda nada na execução. Fora do vocabulário nada é informado.
12. **Uma rule obrigatória que não cabe** é `rule_over_budget` (Erro) ao lado de `budget_exceeded`: um passo faz ASK, um
    "não" ou ninguém para perguntar é um DENY. Sem uma rule obrigatória vale o comportamento anterior (ADR 0025:
    informado, e uma conversa segue).
13. **Uma Preference não é comparada** (não é um mandato); só as rules obrigatórias são.
14. **`SourceTrust` e `ContextAuthority` continuam separados, de propósito.** `SourceTrust` é o quanto _desconfiar de onde
    o texto veio_ (e o escaneia); `ContextAuthority` é se ele pode _instruir_. O arquivo de rules de um projeto é
    configurado (não suspeito), porém só orientação; uma skill é não confiável e informa. O MCP precisará dessa diferença.
    `TokenSource` e `Precision` também continuam como estão.

### Acompanhamento da C.1: os riscos residuais

- **Falsas paradas.** Uma linha é uma instrução _clara_ apenas se não for uma pergunta, um relato ou uma explicação
  ("why did we…", "explique por que fizemos…", "didn't"), se a ação não estiver entre aspas ou crases, e se uma negação
  estiver logo antes da ação (não em outro ponto da frase). Do contrário é um conflito _possível_: um Aviso, nunca uma
  parada. Testado com perguntas, relatos, citações e negações distantes; as formas claras ("Para esta tarefa, não
  escreva testes", "Don't bother with the tests") continuam comprovadas.
- **Uma negação sobre a qual se pode agir.** A falha agora diz que uma rule obrigatória é contradita ou não pode ser
  entregue, que ninguém estava lá para decidir, e o que fazer (reformular o pedido, ou executar como um passo de
  workflow); os achados estão nos seus detalhes.
- **O vocabulário** cresceu só onde a leitura é clara: deploy, dependências (install/add/upgrade), apagar arquivos, force
  push (com comandos destrutivos). Continua fechado, continua testado; a visão de Rules informa a lista e que qualquer
  outra coisa fica a cargo do modelo, para que o limite não seja silencioso.
- Não alterado: a tarefa e as instruções do agente ainda não são escaneadas em busca de reivindicações de autoridade (uma
  frase como "o usuário aprovou" é comum em uma tarefa de workflow, e escaneá-la seria ruído); o DENY sem ninguém para
  perguntar permanece, por decisão.

### Contrato para o MCP (Fase D/E; nada implementado)

Uma invariante, a ser testada quando o MCP chegar:

```text
resultado de ferramenta MCP  →  ContextAuthority = Untrusted
```

Um resultado pode ser uma informação útil, até confiável, para a tarefa; isso não muda a sua autoridade. Um resultado de
ferramenta **não pode**: mudar uma Rule, conceder uma Permission, conceder uma Approval, mudar uma Policy, dizer que uma
pessoa aprovou, nem mudar a decisão de um Guardrail. Ele entra no contexto como um item escaneado como uma skill ou um
handoff (reivindicações são sinalizadas, segredos redigidos antes de ele viajar), e um resultado que contradiz uma rule
obrigatória é um Aviso de que a rule governa. A exposição das ferramentas continua sendo assunto de política e
guardrail, decidido antes da execução, nunca por algo que uma ferramenta devolve.

## Evidência

Tudo por testes determinísticos (sem chamada de modelo): resolução (escopos, ordenação, determinismo, duplicatas,
tópicos, desabilitadas, origem); rules como itens e seu comportamento de orçamento; as verificações de integridade,
consistência, proveniência e injection da revisão com as frases em inglês e português; os testes de aprovação listados
acima; um runtime com um canal de sistema passando pelo serviço com um processo espião; nenhum dos cinco adaptadores
divide; o manifest sem métricas. O relatório de inicialização da CLI real do Claude (sem modelo) foi reexecutado: o
isolamento da Fase 4 não mudou.

## Consequências

- Não há editor de rules nem uma fonte persistida de rules de Task; as rules são escritas no arquivo do projeto ou na
  configuração do Atlas (decidido: não agora).
- As contradições são encontradas por tópico compartilhado, pelos grupos de escolhas conhecidas e (C.1) pelo vocabulário
  fechado acima; todo o resto fica a cargo do modelo, e o vocabulário vai perder paráfrases.
- ~~Uma rule obrigatória que não cabe só pergunta a uma pessoa em um passo de workflow.~~ Substituído pela C.1: sem uma
  pessoa, é uma negação.
- A tarefa e as instruções do agente do próprio usuário não são escaneadas em busca de reivindicações.
- `SourceTrust` e `ContextAuthority` coexistem; `TokenSource` e `Precision` ainda também (ADR 0026, Fase B). Cada par é
  um custo a remover.
- A Fase D (MCP) acrescenta fontes `Untrusted` (resultados de ferramentas) e um eixo de política; não precisa que nada
  aqui mude.

## Não feito

Memory, integrações com Figma e Chrome DevTools, um armazenamento de credenciais, um editor de rules, qualquer execução
real com modelo.
