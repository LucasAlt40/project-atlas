# ADR 0008 — Histórico de conversas e execuções

**Status:** aceito

## O que é

Conversas e execuções encerradas sobrevivem a uma reinicialização. Junto com a ADR 0007 (processo ao vivo, terminal,
interrupção), isso torna uma execução **observável, controlável, persistente e isolada**:

```
Workspace → Agente → Conversa (mensagens) + Execuções (encerradas)
```

- `domain/conversation::Message` e o novo `domain/execution::StoredExecution` são o que fica guardado.
- `application/history::ConversationHistory` é o caso de uso sobre eles. O `ChatService` escreve por meio dele: a mensagem
  do usuário quando é enviada; a execução (com sua linha do tempo) e a mensagem do assistente quando a execução termina.
- O comando `list_executions` devolve as execuções encerradas, da mais nova para a mais antiga; `list_messages` agora
  devolve o que foi armazenado.
- A interface: o card de um agente tem **Chat / Atividade / Terminal / Detalhes / Execuções**; _Execuções_ lista as
  execuções encerradas do agente (hoje / anteriores) e abre um **Inspetor de Execução** (Detalhes, Atividade, Chat, nota
  do Terminal); _Detalhes_ mostra a execução atual ou a mais recente. O cabeçalho mostra `● N agentes em execução`
  enquanto algo roda em _qualquer_ workspace, e escolher um abre aquele agente.

## O que é guardado e o que não é

| Guardado                                                                                                                          | Não guardado                                     |
| --------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------ |
| mensagens (os dois papéis, `executionId`, hora, tipo de falha)                                                                    | saída bruta do terminal (só ao vivo, ADR 0007)   |
| por execução: status, tarefa, runtime, modelo, horários, falha, uso/custo como informado, metadados, **linha do tempo** (eventos) | texto da resposta em streaming (vira a mensagem) |
|                                                                                                                                   | o prompt montado (contexto do projeto, repetido) |

A resposta vive só na mensagem do assistente; a execução não a repete.

## Armazenamento: o `config.json` ainda basta

`messages` e `executions` são mais dois campos de `UserConfig`, com `#[serde(default)]`, então arquivos existentes
continuam carregando. Nenhum banco de dados foi introduzido: o volume é pequeno e **limitado por conversa** (um agente
em um workspace): as **200 mensagens** mais novas, **100 execuções** e **300 eventos de linha do tempo** por execução. As
gravações continuam atômicas (arquivo temporário + renomear) e uma falha ao salvar nunca quebra o chat na tela (é
registrada em log).

**Custo e quando mudar.** Toda alteração reescreve o arquivo inteiro, então o custo cresce com o que é guardado: no pior
caso alguns MB por agente intenso. Isso é aceitável agora. Migrar para SQLite (uma tabela para mensagens e outra para
execuções, mesma interface `ConversationHistory`) quando o arquivo passar com frequência de alguns MB, quando se quiser
busca no histórico, ou quando a retenção precisar ser por tempo e não por quantidade. Nada fora de
`ConversationHistory` mudaria.

## Isolamento

Tudo é indexado por `(workspaceId, agentId)`: as consultas filtram pelos dois, a retenção é aplicada por par, e excluir
um workspace ou um agente exclui suas conversas e execuções. O frontend filtra de novo por workspace e agente, então um
card nunca mostra a execução de outro workspace. Trocar de workspace mantém as execuções em andamento vivas (ADR 0007);
a lista global chega até elas.

## Ids e execuções interrompidas

- Os ids de execução são `exec-N`. Ao iniciar, o contador continua após o maior id armazenado, então um id nunca é
  reutilizado entre execuções do app.
- Se o Atlas foi fechado (ou travou) durante uma execução, a mensagem do usuário fica sem resposta. Na próxima
  inicialização o histórico a responde uma vez com uma mensagem de assistente **com falha** e uma execução armazenada do
  tipo `app_closed` ("O Atlas foi fechado enquanto isto estava rodando"). Ela não aparece como cancelada: o usuário não a
  cancelou.

## Limitações

- A linha do tempo de uma execução em andamento fica na memória (UI); é armazenada quando a execução termina. Uma queda
  perde a linha do tempo da execução ativa, mas não a pergunta.
- O uso de uma execução aparece no inspetor só depois que ela termina (os runtimes informam no final).
- O histórico é por máquina e por `config.json`; não há exportação nem sincronização.
