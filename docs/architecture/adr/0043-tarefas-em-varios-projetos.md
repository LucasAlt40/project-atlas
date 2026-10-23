# ADR 0043 — Uma tarefa em vários projetos: uma execução por projeto

Status: aceito. Complementa a [ADR 0033](0033-multi-project-workspaces.md) (workspaces com vários projetos) e vale
para o workflow e para a implementação de uma spec ([ADR 0035](0035-spec-driven-development.md)). Não muda nenhuma
invariante da ADR 0033: uma execução continua alterando um único projeto.

## Contexto

Num workspace com frontend e backend, o workflow e o SDD escolhiam **um** projeto de trabalho; os outros eram só
consulta. Uma tarefa fullstack ("adicionar a tela de pedidos") alterava só o backend. A ADR 0033 já dizia que mudar
dois projetos são duas execuções, mas nada as encadeava: o usuário precisava rodar duas vezes e repetir o contexto.

Duas formas foram consideradas:

| Critério                 | A: uma execução por projeto, em sequência                        | B: uma execução que altera todos                            |
| ------------------------ | ---------------------------------------------------------------- | ----------------------------------------------------------- |
| Runtimes                 | todos                                                            | só os que aceitam pastas extras graváveis (Claude, Copilot) |
| Segurança e sandbox      | inalterados (um projeto preso por execução)                      | o guard e o perfil do sandbox passam a ter várias raízes    |
| Revisão e merge          | os de hoje, um por projeto                                       | N branches numa revisão; merge pela metade possível         |
| Coerência entre projetos | a execução seguinte recebe o que a anterior mudou (diff, resumo) | o agente vê os dois lados ao mesmo tempo                    |
| Custo                    | ~N× os passos do workflow                                        | uma execução                                                |

Escolhido: **A**.

## Decisão

- **Plano da execução.** Ao rodar, o usuário escolhe, por projeto: _altera_, _consulta_ ou _fora_, e a ordem dos que
  alteram. Padrão: **só o projeto primário altera** e os outros são consultados. Outro projeto só entra na pipeline se o
  usuário o marcar (na ordem das dependências declaradas, que ele pode mudar). A escolha fica guardada por workspace no
  navegador (uma conveniência).
- **Alerta.** Em todo workspace com mais de um projeto, rodar um workflow ou implementar uma spec abre uma confirmação
  que diz o que vai ser alterado, em quantas execuções e o que só é consultado. Com mais de 3 projetos alterados ela
  mostra a sugestão ("alterar só X") como opção; o usuário decide.
- **Cadeia.** `start_workflow` recebe `nextProjectIds`. A primeira execução trabalha no projeto do escopo e guarda uma
  `ProjectChain` (`WorkflowExecution.chain`: projetos e nomes, posição, leituras, o que as anteriores deixaram). Cada
  execução consulta os outros projetos da cadeia e as leituras escolhidas.
- **As execuções conversam (regra implícita do Atlas).** Toda execução que tem projetos depois dela recebe no prompt a
  exigência de terminar com uma passagem de bastão: no bloco de resultado, um item `nextProjects` para **cada** projeto
  seguinte, com `needed`, `done` (o que fez que importa lá: endpoints, payloads, tipos, nomes) e `todo` (o que ele
  provavelmente precisa fazer). Vale a última palavra de cada passo sobre cada projeto. A execução seguinte começa com um
  _briefing_: primeiro o que as anteriores disseram que o seu projeto precisa, depois o que cada passo delas fez, as
  decisões, os arquivos, o começo do diff e a branch (que pode ainda não estar mesclada).
- **Só roda se precisar.** Quando uma execução da cadeia **conclui**, o orquestrador vai para o próximo projeto marcado;
  se a última palavra sobre ele foi `needed: false`, ele é pulado (evento `workflow:chain_skipped`, registrado em
  `ProjectChain.skipped` com o motivo) e o seguinte é avaliado. Sem nota, o projeto roda: a falta de passagem nunca pula
  nada. A execução do próximo (`WorkflowService::start_next_in_chain`) tem a mesma tarefa, a mesma spec e a definição
  atual do workflow, e é dirigida na mesma thread. Uma execução que falha ou é cancelada para a cadeia; retomada e
  concluída depois, a cadeia continua. O evento `workflow:chain_continued` leva a tela para a execução seguinte, que mostra
  o que recebeu das anteriores.
- **SDD.** A implementação pode começar em qualquer projeto do workspace (antes só no da spec) e encadear os outros;
  todas as execuções levam a mesma spec. Cada uma marca só as tarefas do seu projeto e o progresso da spec junta as
  execuções da cadeia. A escrita da spec, num workspace com vários projetos, pede as tarefas agrupadas por projeto.

## Consequências e limites

- O merge continua por projeto. A execução seguinte não vê os arquivos do worktree da anterior, só o que lhe é escrito
  no prompt; se o diff for grande, só o começo.
- As execuções são sequenciais (uma por vez por workflow, como antes). Não há execuções paralelas da cadeia.
- Se o app fechar entre uma execução que concluiu e o início da próxima, a próxima não é criada sozinha: o usuário
  roda de novo os projetos que faltam.
- O chat de um agente (fora do workflow) continua com um projeto de trabalho e consultas.
