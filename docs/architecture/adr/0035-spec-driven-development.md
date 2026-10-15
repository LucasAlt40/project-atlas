# ADR 0035 — Specs: desenvolvimento orientado a especificação, com OpenSpec como primeiro formato

Status: aceito. Baseia-se na [ADR 0013](0013-workflow-engine-and-orchestrator.md) (workflow), na
[ADR 0016](0016-human-in-the-loop.md) e na [ADR 0031](0031-plan-approval.md) (a pessoa decide), na
[ADR 0022](0022-skills.md) e na [ADR 0024](0024-runtime-surface-and-isolation.md) (o que um agente alcança) e na
[ADR 0003](0003-command-permissions.md) e [ADR 0007](0007-live-shell-and-process-control.md) (o guard de processos).

## Contexto

Hoje um workflow começa de uma tarefa escrita à mão ("vibe coding"). Queremos um segundo caminho: antes do código,
agentes escrevem uma **especificação** (proposta, requisitos, design, tarefas) que a pessoa lê, corrige e aprova; um
revisor aponta lacunas; só então um workflow implementa **a partir dela**. O formato de referência é o
[OpenSpec](https://github.com/Fission-AI/OpenSpec) (MIT). Nada disso pode exigir Node, Python ou a CLI do OpenSpec, nem
dar a um agente mais alcance do que ele já tem.

## O que o código mostra (e o que o prompt original supunha)

| Pergunta                                         | Resposta                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Uma execução de agente fora de um workflow       | `ChatService` → `ExecutionService`, a mesma de um passo de workflow (política do agente, Task Context do Harness, runtime, guard, uso). Um passo de workflow só difere por ter `read_only`, `context_query` e `detect_interaction`.                                                                                                                                                                                                                                                     |
| `atlas-interaction` só vale em workflow?         | Sim: `detect_interaction = workflow.is_some()`. Foi preciso um caminho próprio (`send_spec_step`) que liga a detecção sem fingir um `WorkflowLink`.                                                                                                                                                                                                                                                                                                                                     |
| A ADR 0031 serve para aprovar um documento?      | O **vocabulário** sim (`approve` / `revise` com nota obrigatória / `redo`). O **armazenamento** não: o `PendingInteraction` mora em `WorkflowExecution.interactions`, preso a um run. Usá-lo exigiria runs de workflow sintéticos. Reutilizamos os tipos e as regras, não o repositório.                                                                                                                                                                                                |
| O `config.json` aguenta documentos?              | Não: é reescrito inteiro a cada mudança e já carrega mensagens e execuções. Specs ganham um armazenamento próprio.                                                                                                                                                                                                                                                                                                                                                                      |
| O agente lê a pasta do rascunho?                 | Não: o runtime só alcança a pasta do projeto (ADR 0024). Os documentos aprovados vão **no prompt**, com orçamento e aviso de corte; o brief de 4 500 caracteres do workflow não se aplica a esse prompt.                                                                                                                                                                                                                                                                                |
| Um agente em worktree isolado vê arquivos novos? | Só o que está no commit base. Uma spec publicada e ainda não commitada **não** aparece no worktree. Por isso o run recebe no brief o conteúdo da spec (requisitos, decisões do design e **todas** as tarefas, até 24 000 caracteres), e o agente é avisado de que os arquivos não estão no worktree. Quando não cabe, cada parte perde um pouco e diz quanto (`[cut: …]`), em vez de perder o fim da lista de tarefas em silêncio (ADR 0036). Os caminhos publicados são só referência. |
| O design depende só do `proposal` no OpenSpec?   | Sim (`schemas/spec-driven/schema.yaml`): `design` exige `proposal`; `tasks` exige `specs` e `design`. O Atlas serializa `design` depois de `specs` (decisão 3).                                                                                                                                                                                                                                                                                                                         |

## Decisões

### 1. Motor interno; a CLI é um extra

`application/specs` escreve, valida, publica e importa sem processo externo. A CLI do OpenSpec só entra se a pessoa pedir
(decisão 9). A ausência de Node, Python, `uv` ou da CLI não impede nenhuma parte do fluxo, e há testes para isso.

### 2. `SpecFramework` e `OpenSpecAdapter`

Um trait Rust simples (sem plugins dinâmicos) concentra tudo o que é do formato: o grafo de artefatos, os templates e as
instruções, a leitura da saída do agente, a validação, os arquivos a publicar, a importação, as tarefas e o digest. O
resto do Atlas (serviços, persistência, telas) pergunta ao framework; nenhum `if framework == OpenSpec` fora do adaptador.
As instruções vêm do schema oficial `spec-driven` (MIT), sem os comandos de terminal (`openspec list`…) que um agente do
Atlas não pode rodar. Spec Kit não faz parte desta entrega.

### 3. Grafo explícito, com uma divergência declarada

`proposal → specs → design → tasks`, cada artefato com suas dependências (`depends_on`). Divergimos do schema em um ponto:
`design` também espera `specs` aprovados, para citar requisitos já aceitos. Os arquivos publicados são os mesmos.
`specs` é **um** artefato que guarda um arquivo por capacidade (`specs/<capacidade>/spec.md`).

### 4. Três eixos que nunca se misturam

- **Execução** (`Generation`): ociosa, rodando (com `run_id`), esperando resposta, falha (`interrupted` quando o app fechou).
- **Revisão humana** (`Review`): sem revisão, aguardando, alterações pedidas, aprovada. Vale para a **última** revisão
  do documento; `approved_revision` guarda a última aprovada.
- **Validação** (`ArtifactValidation`): presa ao número da revisão que leu.

O status do spec (`draft`, `generating`, `ready_for_implementation`… 13 valores) é **derivado** (`rules::status`), nunca
gravado: nada define um status que os documentos não sustentem. "Pronto" é o que sobra quando todas as condições valem
(`rules::readiness`); nenhum agente o declara.

### 5. Concorrência e retomada

Um spec tem uma execução por vez (`RunInProgress`). O resultado de uma execução só é usado se ela ainda é a que o artefato
espera (`run_id`): um resultado tardio ou repetido é descartado e a execução marcada como `Superseded`. Toda mudança é feita
numa cópia, gravada e só então mantida; se a gravação falha o spec fica como estava. Uma aprovação nomeia a revisão
(`expected_revision`); outra revisão é recusada (`SpecRevisionOutdated`). Ao reabrir o app, execuções que ninguém viu terminar
viram `interrupted`; nada é retomado sozinho. Perguntas pendentes sobrevivem a reinício.

### 6. Rascunho fora do projeto; publicação segura

Rascunhos ficam em `<dados do app>/specs/<id>/spec.json` (escrita atômica, `FsSpecStore`). O projeto só recebe arquivos na
publicação, em `openspec/changes/<id>/`, depois da aprovação final: nada é sobrescrito (`create_new`); uma pasta que já
existe e não é deste spec devolve `SpecChangeExists` e a pessoa escolhe outro nome; uma publicação que parou no meio é
registrada (`InProgress`/`Failed` com os arquivos já escritos) e a próxima continua de lá; só é "publicada" depois de reler
cada arquivo; republicar um spec alterado troca apenas os próprios arquivos (via arquivo temporário), nunca apaga. Caminhos
são relativos, sem `..`, e nada passa por symlink (`FsProjectFiles`).

### 7. Duas camadas de validação

**Determinística** (`openspec/validate.rs`): as regras do OpenSpec (`## Why` ≥ 50 caracteres, `SHALL`/`MUST`, cenários
`####` com `WHEN`/`THEN`, deltas `ADDED/MODIFIED/REMOVED/RENAMED`, checkboxes numerados, `MODIFIED` só sobre requisito que
existe no projeto…) mais as do Atlas (capacidade citada tem arquivo; tarefa que cita requisito inexistente é erro; requisito
sem tarefa é aviso). Erro bloqueia; aviso e sugestão não. `TODO`/`TBD`/`[NEEDS CLARIFICATION]` é aviso no rascunho e erro na
validação final. Aprovar um documento exige a validação **da própria revisão** sem erros.
**Por IA**: um agente revisor, somente leitura, devolve um bloco `atlas-review` (JSON, campo a campo validado e cortado).
Cada achado tem id estável (mesmo problema = mesmo achado), categoria, severidade, evidência, pergunta e sugestão, e é uma
**opinião**: bloqueia se for grave ou trouxer uma pergunta, até a pessoa decidir (resolver, descartar, aceitar o risco **por
escrito**, ou responder, o que devolve o documento ao agente). Dispensar a revisão exige texto.

### 8. A spec no workflow

`WorkflowExecution.spec: Option<SpecLink>` (campo novo, `serde(default)`, ausente nas execuções de antes): spec, revisões
dadas, digest, tarefas. O brief ganha o bloco `SPECIFICATION` (resumo limitado, regras: não editar `openspec/`, sinalizar
divergência com a categoria `spec-divergence`, reportar `completedTasks`). Só uma spec **pronta** do workspace do workflow
inicia um run; o run trabalha no projeto da spec. O fluxo de vibe coding não muda: sem `specId`, nada é diferente em
nenhum comando, evento ou formato gravado.
**Tarefas**: `completedTasks` é o que o agente **relata**; o Atlas **confirma** só quando o run termina com sucesso. Não há
teste por tarefa; a tela diz qual dos dois está mostrando. Uma alteração posterior na spec aparece como `specChanged`.

### 9. CLI opcional, pelo guard

`ProcessContext::Tool` (novo): um programa conhecido (`openspec`) por nome puro, sem shell, sem variável de ambiente
perigosa, `cwd` absoluto existente e um subcomando da lista (`--version`, `validate`, `status`, `archive`). Rodado pelo
Atlas, nunca por um agente, com `OPENSPEC_TELEMETRY=0` e `DO_NOT_TRACK=1`. Se o binário achado estiver dentro do projeto, não
roda. Versão major ≥ 1. `validate` é uma segunda opinião (não decide prontidão); `archive` só para um spec pronto,
publicado como está, sem execução de workflow em curso, depois de uma confirmação que diz o que vai acontecer, e confirma a
pasta no destino antes de registrar.

### 10. Sem feature flag

O módulo está sempre ligado: não há configuração para ligá-lo. O que mantém o Vibe Coding intacto é a escolha feita
na hora de executar (padrão: Vibe coding, sem spec), e não uma chave global. Uma versão anterior deste trabalho teve
`specs_enabled`; foi removida. Configs antigas com esse campo continuam lidas (o campo é ignorado).

### 11. Integrações (MCP) para testar e revisar

Ao criar a spec (e depois, em "Agentes e ferramentas") a pessoa escolhe, entre as conexões MCP ligadas do workspace, as que
servem para testar e revisar aquela spec (`Spec.integrations`, ids de conexão). **Escolher não autoriza nada**: quem concede
ferramentas a uma etapa continua sendo o hub MCP (grants). O que a escolha faz é informar: os agentes que escrevem
especificações, design e tarefas recebem o nome e as ferramentas descobertas de cada integração escolhida e ligada, com a
instrução de nomear uma integração na linha de Verificação de uma tarefa só quando ela puder fazer a checagem, de nunca
inventar ferramenta e de escrever a verificação de modo que ainda diga o que checar à mão quando a ferramenta não estiver lá.
O revisor recebe a lista como o que pode usar. Na execução a partir da spec, o brief de cada passo lista as integrações e
manda deixar fora de `completedTasks` o que não pôde ser verificado por não ter a ferramenta. Uma conexão desligada fica
escolhida, aparece como desligada e não é citada a nenhum agente. A proposta não recebe a lista: é sobre por quê e o quê.

## O que não foi feito (e por quê)

- **Spec Kit**: fora do escopo desta entrega.
- **`skip_specs`** (mudança sem comportamento): o fluxo exige ao menos uma capacidade.
- **Mesclar deltas em `openspec/specs/` sem a CLI**: reimplementar o archive do OpenSpec é arriscado; sem a CLI, "Encerrar esta spec" só fecha o registro do Atlas e os arquivos do projeto ficam onde estão.
- **Acompanhar/aprovar pelo celular** (ADR 0034): nada impede, nada foi feito.
- **Diff entre revisões**, edição manual do documento e sincronização de arquivos alterados à mão no projeto.
- **Garantia de que um agente isolado leia os arquivos publicados**: só o digest é garantido (ver a tabela).

## Riscos conhecidos

- A qualidade do documento depende do agente seguir o formato; o formato malformado falha a execução e preserva o que
  havia, mas custa uma chamada.
- O custo de tokens cresce com o tamanho dos documentos (todos vão no prompt da revisão). O orçamento (40 000 caracteres)
  corta com aviso.
- As regras do OpenSpec mudam entre versões; o validador é versionado (`atlas-openspec/1`) e uma validação de outra versão
  é refeita. A CLI foi exercitada de verdade na 1.3.1; o contrato documentado vai até a 1.14.
