# 2. Agentes, runtimes e como o prompt é montado

> Parte do [guia do Atlas](README.md). ADRs: [0004](../architecture/adr/0004-runtimes.md),
> [0007](../architecture/adr/0007-live-shell-and-process-control.md),
> [0014](../architecture/adr/0014-handoff-shared-worktree-and-code-integration.md),
> [0015](../architecture/adr/0015-result-contracts-outcomes-and-worktree-lock.md).

## Agente = personalidade + runtime + modelo + instruções (+ política)

Um `Agent` guarda: `personalityId`, `runtimeId`, `modelId`, `instructions` (instruções permanentes do agente),
`permissionProfileId`, `worktreeIsolation` (padrão **ligado**) e `resultContract`. Um agente é configuração **global**;
é colocado em workspaces pelo layout.

### Personalidades embutidas

As cinco que acompanham o Atlas (`application/personalities/builtin.rs`). O usuário pode editar (guarda-se uma cópia em
`builtinOverrides`), esconder (`hiddenBuiltins`), restaurar, e criar personalidades próprias.

| Id                       | Pergunta que responde                                | Perfil de permissão sugerido |
| ------------------------ | ---------------------------------------------------- | ---------------------------- |
| `architect`              | O que deve ser construído e como?                    | `developer`                  |
| `developer`              | Como implemento? (código + testes)                   | `developer`                  |
| `qa`                     | Funciona de verdade? (comportamento, bordas, testes) | `developer`                  |
| `architecture-validator` | Foi construído **corretamente**? **Somente leitura** | `read_only`                  |
| `bug-fixer`              | Por que falhou e como corrijo? (causa raiz, mínimo)  | `developer`                  |

Uma personalidade pode carregar um `suggestedContract`; ele só é aplicado pelo editor quando o agente é criado, nunca
sozinho. Agentes antigos são `general` (sem outcome obrigatório). Agentes novos nascem com o perfil de permissão mais
restritivo (`read_only`) até o usuário escolher outro.

## Runtimes

Um runtime é o adaptador de uma CLI (`trait ModelRuntime`: `info()`, `detect()`, `list_models()`, `execute()`).
Adicionar um runtime = um arquivo em `application/runtimes/` + uma linha em `RuntimeRegistry::with_default_runtimes`;
nada em agentes, prompt, execuções, comandos ou UI muda. **Nenhum código ramifica pelo nome do runtime.**

| Runtime     | Programa   | Como executa (leitura)                                                                                                | Como habilita edição (`allow_edits`)                 | Modelos                                             |
| ----------- | ---------- | --------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- | --------------------------------------------------- |
| Claude CLI  | `claude`   | `-p --output-format stream-json --verbose --include-partial-messages --tools Read,Grep,Glob --no-session-persistence` | `--tools …,Edit,Write --permission-mode acceptEdits` | sem listagem; usuário informa (alias como `sonnet`) |
| OpenCode    | `opencode` | `opencode run --agent plan -m <modelo> --format json`                                                                 | `--agent build`                                      | `opencode models`                                   |
| Gemini CLI  | `gemini`   | `--output-format stream-json --approval-mode default --skip-trust --prompt=…`                                         | `--approval-mode auto_edit`                          | sem listagem                                        |
| Antigravity | `agy`      | `--output-format stream-json --model <id> --prompt=…`                                                                 | `--mode accept-edits`                                | `agy models`                                        |
| Codex       | `codex`    | **só detecção** (`DetectOnlyRuntime`): não é possível criar agentes para ele                                          | —                                                    | —                                                   |

Regras comuns:

- **Nunca** `--dangerously-skip-permissions`, `--yolo` ou equivalentes.
- Programa nomeado "nu" + argumentos fixos; id de modelo é validado para não virar opção; **nenhum shell** é iniciado.
- Em PTY, o prompt vai como **último argumento após `--`** (as CLIs ignoram stdin de terminal). Em pipes, vai por stdin.
  Em Windows `.cmd` com prompt multilinha cai para pipes (sem terminal ao vivo).
- Claude: sessão vem de `claude auth status`. Gemini encerrou a CLI para contas individuais (`IneligibleTierError`) →
  reportado como indisponível apontando ao Antigravity. Antigravity não tem flag para restringir ferramentas
  (`text_only` desligado).
- Erros viram `RuntimeError` normalizado (não instalado, indisponível, autenticação, modelo, timeout, execução falhou,
  pedido inválido, resposta inesperada) → uma execução _failed_ com mensagem ao usuário e detalhe técnico opcional.
- `RuntimeCapabilities` descreve o que o runtime pode (`fileEdit`, `interactiveTerminal`, `interrupt`, `terminalInput`,
  `usageMetrics`, `costMetrics`, `quotaMetrics`, `nonInteractiveExecution`…). A UI mostra só o que o runtime oferece.

### Edição de arquivos: três condições juntas

Um agente **só escreve** se, ao mesmo tempo (`ExecutionService::edit_access`):

1. o **runtime** tem a capacidade `fileEdit`;
2. a **política** do agente permite escrita de arquivos;
3. a execução está em **worktree isolado** (uma edição nunca alcança o checkout do projeto).

Os motivos são distintos (`RuntimeCannotEdit` / `PolicyDenied` / `NotIsolated`) e a UI os diferencia. Desconhecido
significa "não". Mesmo com edição, **o agente não roda shell, testes nem rede** (marco futuro, decisão separada).

## Anatomia do prompt

`PromptBuilder::assemble` é o **único** lugar onde um prompt é montado. O texto final (`Prompt::combined()`) tem seções
rotuladas, nesta ordem:

```text
SYSTEM / PERSONALITY
  <instruções da personalidade>
  <ATLAS_RULES  (somente-leitura)  |  ATLAS_RULES_EDITING (worktree isolado, pode editar arquivos)>
  <LIVE_NARRATION>
  <PLAN_RULE>
[ TASK CONTEXT  |  PROJECT HARNESS ]     ← contexto do Harness (opcional; um ou outro, nunca os dois)
PROJECT CONTEXT
  Project: <nome> / Path: <caminho do worktree ou do projeto> / Detected technologies: …
USER INSTRUCTION
  Agent instructions: <instruções permanentes do agente>      (omitido se vazio)
  Task: <tarefa — ou, num passo de workflow, o brief completo>
```

Regras fixas aplicadas a **todo** agente, qualquer personalidade:

| Bloco                 | Conteúdo / intenção                                                                                                                                                                                                           |
| --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ATLAS_RULES`         | "Modo somente-leitura: não modifique arquivos nem rode comandos que mudem algo. Responda só com texto."                                                                                                                       |
| `ATLAS_RULES_EDITING` | Está em worktree Git isolado; pode criar/editar arquivos; **sem shell, sem rede**, nada fora do diretório; o que escreve é salvo no worktree, passado aos próximos passos e revisado pelo usuário antes de chegar ao projeto. |
| `LIVE_NARRATION`      | Narrar cada passo em uma frase curta; ao terminar, escrever a resposta final **autossuficiente** (a resposta guardada é a mensagem de conclusão). Citar caminhos com aspas.                                                   |
| `PLAN_RULE`           | **Planos são documentos obrigatórios**: planos/designs/análises devem ser escritos por inteiro em Markdown na mensagem e, se puder criar arquivos, salvos em `docs/plans/<nome>.md`.                                          |

O prompt **diz só a verdade**: o texto de regra reflete exatamente o que as ferramentas lançadas permitem. O prompt
cita o caminho do **worktree** como "path do projeto", nunca o do checkout.

O texto exato enviado fica guardado em `Execution.prompt` (e a UI o mostra no inspetor).

## Pipeline de uma execução (`ExecutionService::run_step`)

```text
1. valida tarefa; resolve agente, personalidade, runtime, workspace/projeto
2. emite "started"
3. carrega contexto do Harness NA RAIZ DO PROJETO (Task Context selecionado para a tarefa; fallback = Harness inteiro)
4. se worktreeIsolation: prepara o worktree (ou usa o do run); falha se não for repositório Git — nunca cai para o checkout
5. calcula edit_access (runtime ∩ política ∩ isolado) → can_edit
6. PromptBuilder::assemble → prompt combinado (+ PromptLayout p/ medição)
7. monta ExecutionScope (workspace, agente, execução, isolado, o que as ferramentas conseguem fazer)
8. runtime.execute(...)  → ProcessRunner (GUARD → sistema) → PTY → CLI
9. se for passo de workflow: detecta "esperando pessoa" (ver doc 04)
10. conclui: Completed / Failed / Cancelled / WaitingForInput; usage, custo, metadados
11. worktree: finalize (execução avulsa) ou finish_step (worktree compartilhado do run)
12. grava a auditoria de permissões e o histórico
```

Pontos de design importantes:

- **O Harness é lido uma vez, na raiz do projeto, e chega ao agente só como texto do prompt.** Nem o runtime nem o
  worktree enxergam `.atlas/`.
- **O runtime só recebe um diretório de trabalho** — não existe código de worktree dentro de nenhum runtime.
- O ambiente do processo define `PWD` junto com o cwd (o OpenCode confiava no `PWD` herdado).
- Um agente ocupado não pode rodar outra coisa no mesmo workspace; um passo de workflow cujo agente está ocupado
  **espera** (não falha).

## Formato das respostas e resultados de execução

- A resposta guardada de um agente é a **mensagem de conclusão** (o texto final). A resposta transmitida (tokens/ferramentas)
  é só para a UI ao vivo.
- Um `Execution` registra: `runtimeId`, `modelId`, prompt exato, resultado ou falha, `metadata` do runtime (duração,
  custo…), `context` (modo task-aware/fallback, tamanhos, itens omitidos), `optimization` (ver [06](06-persistencia-eventos-e-observabilidade.md)).
- Estados: `Running → Completed | Failed | Cancelled | WaitingForInput`. `Cancelled` só quando o usuário agiu no processo
  e a execução não completou. `app_closed` marca execuções cortadas por fechamento do app.
- Eventos de progresso (`execution:progress`): `started`, `starting_runtime`, `sending_prompt`, `waiting_for_model`,
  `output_chunk`, `tool_started`, `tool_completed`, `completed`, `failed`. Cada runtime emite só o que a CLI realmente
  transmite. A UI redige o texto a partir de `kind` + `metadata`; a mensagem em inglês do core é para logs.
