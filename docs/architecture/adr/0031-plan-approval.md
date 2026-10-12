# ADR 0031 — Planos esperam pela pessoa (ou pela aprovação permanente dela)

Status: aceito. Baseia-se na [ADR 0016](0016-human-in-the-loop.md); não muda nenhuma das suas invariantes.

## Contexto

Todo agente já é instruído de que um plano é um documento a ser mostrado por inteiro (a regra dos planos), e um passo de
workflow já pode parar e perguntar a uma pessoa (ADR 0016, `PendingInteraction` com um `document`). Nada fazia do plano
um _portão_: um agente podia escrever um e seguir executando-o, e a pessoa só tinha botões de sim/não para uma aprovação,
sem como dizer o que mudar.

## Decisões

1. **Um plano é uma interação de tipo próprio** (`InteractionKind::Plan`). O agente termina a mensagem, depois de
   escrever o plano por inteiro, com `{"type":"plan"}` no bloco `atlas-interaction` (não precisa de pergunta). O plano é
   o texto antes do bloco e é guardado como o documento da interação. A instrução faz parte do brief de todo passo de
   workflow, então vale para qualquer agente, embutido ou criado pelo usuário, em qualquer runtime. Conversas não são
   afetadas: nelas a pessoa simplesmente responde.
2. **Três respostas, fixadas pelo Atlas**: `approve`, `revise` (as palavras da pessoa são obrigatórias: "mude" sem dizer
   como é recusado) e `redo` (a nota é opcional). Um agente não pode adicionar um botão. Nem `revise` nem `redo` são uma
   recusa; a próxima passada é informada de qual recebeu e de que deve apresentar o plano de novo por inteiro em vez de
   executar qualquer coisa (bloco `HUMAN INPUT`). A aprovação diz para executar o plano, no worktree da própria
   execução, e não concede nada.
3. **A execução espera** exatamente como descreve a ADR 0016 (`WaitingForInput`, nada novo começa, a pergunta é guardada
   com a execução e sobrevive a uma reinicialização) até que uma das três respostas chegue.
4. **A aprovação automática é uma instrução permanente da pessoa**, nunca de um agente.
   `WorkflowExecution.auto_approve_plans` é definido quando a execução é iniciada (`start_workflow(autoApprovePlans)`) ou
   depois (`set_auto_approve_plans`, que também aprova os planos que já estão esperando). Ligada, o orquestrador responde
   `approve` a um plano por conta própria e marca a interação como `auto`, de modo que o histórico e o painel de planos
   digam "aprovado automaticamente". Ela responde só planos: uma pergunta de esclarecimento, permissão, confirmação de
   runtime ou guardrail ainda espera por uma pessoa. A tela lembra a última escolha nesta máquina e a oferece em cada nova
   execução.
5. **Os planos continuam legíveis**: o dock da execução lista todo plano com o que foi decidido sobre ele (esperando,
   aprovado, aprovado automaticamente, mudanças pedidas, refeito).

## Limites

- A detecção depende da cooperação do agente, como toda interação estruturada: um agente que escreve um plano e não
  termina com o bloco não é parado. A heurística de texto da ADR 0016 não é estendida para adivinhar planos.
- Uma execução iniciada antes desta mudança não tem a configuração: ela é lida como desligada.
- Nenhum modelo real foi solicitado a seguir a nova instrução; o comportamento é coberto com executores roteirizados.
