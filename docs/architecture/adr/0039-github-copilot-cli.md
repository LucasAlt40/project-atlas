# ADR 0039 — GitHub Copilot CLI como runtime

Status: aceito e implementado. Medido com o CLI real (1.0.97) e modelo real em 2026-10-15. Segue a
[ADR 0004](0004-runtimes.md) (runtimes), a [ADR 0029](0029-mcp-for-every-runtime.md) (MCP para todo runtime) e a
[ADR 0038](0038-macos-sandbox.md) (sandbox).

## Decisão

O Copilot entra como mais um adaptador de `ModelRuntime` (`runtimes/copilot.rs`) com seu adaptador MCP
(`runtimes/copilot_mcp.rs`), seu dialeto de arquivo no hub (`ConfigDialect::Copilot`, `~/.copilot/mcp-config.json`,
respeitando `COPILOT_HOME`) e sua estratégia de confinamento. Nada em agentes, prompts ou execuções mudou.

## O que foi medido

- **Execução:** `copilot --prompt=<texto> --output-format json --no-ask-user --no-auto-update --disable-builtin-mcps
--disallow-temp-dir --model <id>`. A saída é JSONL: `assistant.message`, `tool.execution_start/complete`,
  `session.usage_checkpoint` (tokens e lista de ferramentas enviadas ao modelo) e `result` (código de saída).
- **Permissões:** sem ninguém para perguntar, o CLI **nega** o que não foi permitido (comandos, escrita, chamadas MCP).
  O Atlas só passa `--allow-tool=write` quando a execução pode editar e `--allow-tool=<servidor>` para cada servidor
  concedido. Nunca `--allow-all` nem `--yolo`. Comandos de shell ficam sempre negados (`runs_commands = false`).
- **Só-texto:** `--available-tools` com um nome que não existe deixa o pedido com **zero ferramentas**
  (`text_only = true`). Confirmado com os servidores do próprio usuário configurados.
- **Pastas extras:** `--add-dir=<pasta>` libera a leitura; sem ele ela é negada (`accepts_extra_dirs = true`).
- **MCP:** `--additional-mcp-config @<arquivo>` (privado, apagado ao fim). `${VAR}` é expandido do ambiente do CLI, então
  o segredo viaja pelo ambiente e nunca pelo arquivo. `tools` é lista de permissão (`AllowList`). O nome para o modelo
  é `<servidor>-<ferramenta>`; o Atlas mostra `mcp__<servidor>__<ferramenta>` como nos outros runtimes.
- **Modelos:** `copilot --acp` responde `session/new` com os modelos que **a conta** pode escolher, sem chamar modelo
  (cerca de 2 s; o processo encerra quando a entrada acaba). `copilot help config` lista muito mais (28) e foi medido
  oferecendo modelos que a conta não tem (`gpt-5-mini`, `claude-haiku-5.5`, `gpt-5.4`, `gpt-6-luna`, `claude-sonnet-5.5`:
  "não disponível"), por isso não é usado. Nesta conta só `auto` é aceito explicitamente; ele escolhe o modelo sozinho.

## Limites conhecidos (não escondidos)

- **Login desconhecido:** não há comando de status; a ausência de login aparece como `AuthenticationRequired` na execução.
- **MCP não estrito e sem sonda:** os servidores do usuário (e de `.mcp.json` do projeto) carregam junto, e não há como
  olhar um servidor sem chamar um modelo. `copilot mcp list` só lê configuração.
- **Servidores remotos (HTTP):** o hub os ignora com o motivo, como nos outros dialetos.
- **Pergunta ao usuário:** a ferramenta de perguntar não é chamada em modo headless; vale o detector genérico.
- **Sandbox:** o trabalho normal funciona dentro do perfil (`sandbox:confined`, edição no worktree e mesclagem). A
  tentativa de fuga **não foi exercitada**: em 4 execuções com `auto` o modelo se recusou a tentar ("fora do diretório de
  trabalho") e nada apareceu fora do projeto, o que só mostra que não tentou. Outros modelos não estão disponíveis nesta
  conta para forçar a tentativa. O CLI também confere caminhos por conta própria antes do sistema operacional.
- **Memória (ADR 0037):** a nota entra no prompt e muda o resultado (acertou o arquivo, sem tocar o isca, 3 chamadas contra
  5, 56 mil contra 69 mil tokens de entrada; 1 execução por braço). O agente **propor** notas deu 0 em 1 execução:
  inconclusivo, sem amostra para dizer que é limite do Copilot.
- Não verificado: edição de arquivos fora do sandbox por um fluxo longo, e um modelo específico que não seja `auto`.

## Como verificar

Sempre por nome, nunca um `--ignored` amplo (chama modelos pagos); cada um gasta cerca de 1 premium request, salvo os de
workflow e memória. Passaram em 2026-10-15/16 com `auto`:
`real_copilot_answers_through_the_system_runner`, `real_copilot_calls_a_granted_mcp_server_and_the_secret_reached_it`,
`real_copilot_text_only_is_sent_no_tool_even_with_the_users_own_servers`, `real_model_copilot_*` (MCP filtrado, inteiro,
DevTools), `real_copilot_can_be_interrupted`, `real_workflow_*_copilot_cli` (3 fluxos), `sandbox_real_copilot_does_its_work`
e `memory_experiment_copilot` (com `ATLAS_M4_N=1`). `sandbox_real_copilot_cannot_escape` roda, mas não exercita (acima).
