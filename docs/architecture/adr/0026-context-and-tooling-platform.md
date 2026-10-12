# 0026 — Context & Tooling Platform (Fase A: arquitetura e spike)

## Contexto

O Atlas monta um prompt por execução e o entrega a um runtime de CLI. As fases 0–5 da Camada de Otimização (ADRs
0020–0025) deram a ele medição, um Context Engine, Skills, isolamento de runtime e Guardrails. O que lhe falta para
gerenciar contexto e ferramentas como uma plataforma: um orçamento que pertença a um runtime + modelo, um registro do que
foi realmente entregue, Rules, Memory e MCP (com o qual o Claude é iniciado sem, de propósito, ADR 0024). A auditoria que
precedeu esta ADR e o desenho completo estão em [context-and-tooling-platform.md](../context-and-tooling-platform.md).
Esta ADR registra as decisões; não edita as ADRs 0010–0025.

## Decisões

1. **Uma resolução de execução.** Contexto, Rules, Memory e Tooling/MCP alimentam a mesma resolução por passo, revisada
   pelos guardrails existentes e entregue pelo adaptador do runtime. Não são funcionalidades independentes. Nenhum
   segundo Context Engine, nenhum segundo sistema de permissões, nenhum `McpSecurity`.
2. **`ExecutionBudget`** substitui os dois `ContextBudget`s. É um orçamento de uma execução, com seções (system prompt,
   rules, Harness, skills, memory, histórico, handoffs, definições de ferramentas, resultados de ferramentas) e um
   `available_for_task_context` derivado. Todo número carrega `source` (Reported / Configured / Default / Unknown) **e**
   `precision` (Exact / Estimated / Unknown) **e**, quando estimado, um `method`. Reported ≠ Exact; Configured ≠ Exact.
   `chars/4` continua sendo a única estimativa, rotulada `Estimated`.
3. **`ModelLimits` é indexado por (runtime, modelo)**, nunca só por modelo nem por provedor. Resolução: informado →
   tabela conhecida do Atlas → override do workspace → override do agente/workflow; um override pode estreitar, nunca
   exceder um teto Reported ou Exact, e o ajuste é registrado. Uma janela desconhecida continua desconhecida. Um item
   Required que não cabe nunca é cortado: DENY ou ASK.
4. **Context Plan ≠ Context Manifest.** O Plan é o que o Atlas pretendia montar; o Manifest é o que ele preparou e
   entregou (dezessete perguntas na página de desenho). O Manifest guarda referências, impressões digitais, tamanhos e
   motivos, não segredos nem cópias.
5. **O hash do prompt** é calculado imediatamente antes do `runtime.execute()` sobre o payload que o adaptador vai
   entregar. O adaptador é dono de uma função `delivery()` usada tanto na entrega quanto no hash; um teste com um
   spy-runner afirma `manifest.prompt_hash == hash(payload capturado)`. Onde o payload não pode ser representado, o
   manifest diz isso em vez de calcular o hash de outra coisa.
6. **`RuntimeSurface`.** As fontes do que o agente vê são classificadas `AtlasControlled / RuntimeControlled /
UserControlled / Unknown`, cada uma `Reported / Declared / NotObserved`. Para o Claude, os plugins, o `CLAUDE.md`, os
   hooks, as regras de permissão e a auto-memória são `UserControlled` e continuam assim; o Atlas não afirma controlá-los.
   O isolamento total não faz parte deste trabalho.
7. **Rules são um conceito de domínio** (id, escopo, prioridade, conteúdo, obrigatória, proveniência, fingerprint,
   habilitada, aplicabilidade), resolvidas de forma determinística em `ContextItem`s. Uma Rule não é uma Permission nem o
   System Prompt; testes de prompt injection são obrigatórios. As restrições do usuário no Harness não são copiadas para
   Rules.
8. **Memory é um domínio, não um arquivo JSON**, e só para o que nem o Harness nem o `SharedExecutionState` guardam.
   Escopos mantidos: User, Workspace, Agent. Project (≈ Workspace, coberto pelo Harness) e Workflow (coberto pelo
   `SharedExecutionState`) não são escopos de Memory. Recuperação por filtros; sem vetores, sem RAG. Não está na Fase B.
9. **O domínio MCP é agnóstico de runtime** (`McpConnection`, `McpServer`, `McpTool`, `McpTransport`, `McpCapabilities`,
   `ToolPolicy`, `ToolAccess`); o `ClaudeMcpAdapter` traduz para `--mcp-config --strict-mcp-config`. Outros runtimes:
   Unsupported / Not investigated. Estratégia A (nativa do runtime, configuração gerada por execução). Sem Tool Gateway
   nem proxy universal.
10. **Workspace → Conexão → Política → Agente/Workflow/Passo → Conjunto Efetivo de Ferramentas.** Uma conexão não é uma
    autorização. Seis estados separados: servidor descoberto, ferramenta descoberta, habilitada, autorizada, exposta ao
    runtime, usada.
11. **Os guardrails estendem o `security/`**: um eixo MCP na `SecurityPolicy` (negar por padrão), uma capacidade MCP
    honesta de três estados em `RuntimeCapabilities`, e servidores MCP, o conjunto efetivo de ferramentas e uma
    impressão digital da configuração de ferramentas na impressão digital da avaliação, de modo que uma mudança invalide
    uma aprovação anterior. Resultados de MCP são dados não confiáveis.
12. **STDIO é `executável + args[] + env`**, nunca uma string de shell; o usuário autoriza o processo local
    explicitamente. **HTTP é reservado** em `McpTransport` e não construído; quando for, precisa da sua própria camada
    de destino/DNS/IP/redirect/TLS/autenticação.
13. **`CredentialStore` é uma porta de domínio**; o keyring nativo do SO é uma dependência de infraestrutura. Segredos
    nunca entram em prompt, manifest, métricas, logs ou `UserConfig`.
14. **System prompt inalterado.** `--append-system-prompt` não é adotado; a arquitetura reserva um `SystemPromptChannel`
    por runtime (padrão `Inline`).
15. **O Figma não é uma integração stdio da Fase E**: o servidor oficial é HTTP + OAuth. Ele espera o transporte HTTP e
    uma decisão sobre OAuth. O Chrome DevTools MCP (stdio) pode vir antes, fixado em uma versão explícita, nunca iniciado
    nem autorizado por padrão.

## Evidência (spike, sem chamada de modelo)

Claude Code 2.1.285, argumentos de lançamento do Atlas + `--mcp-config` + `--strict-mcp-config`, modelo que não existe.
Detalhes e tabela: página de desenho, seção 10; reproduzível com `docs/architecture/spikes/mcp-claude/`.

- Só aparece o servidor que o Atlas passa (`mcp_servers`: `connected`, `source: dynamic`); as ferramentas se chamam
  `mcp__<servidor>__<ferramenta>`.
- **`--tools` não limita ferramentas MCP**; `--tools ""` ainda as expõe; `--allowedTools` não as esconde;
  `--disallowedTools mcp__<servidor>__<ferramenta>` remove uma ferramenta da lista.
- Uma ferramenta com esquema inválido faz a CLI descartar **todas** as ferramentas do servidor enquanto ainda informa
  `connected`.
- Um servidor que falha ao iniciar fica `failed` e a execução continua; uma configuração JSON inválida faz a CLI sair.
- Nenhum processo MCP sobrando depois de qualquer execução; `~/.claude/settings.json` e `CLAUDE.md` intocados.

## Consequências

- A autorização por ferramenta para o Claude via `--disallowedTools` é uma lista de negação e falha aberta para
  ferramentas que um servidor adicione depois. A Fase D precisa provar uma mitigação (autorização por servidor, e
  conferir `init.tools` e parar antes da primeira requisição) ou documentar a autorização por ferramenta como de melhor
  esforço.
- Uma execução somente texto não deve receber uma configuração MCP.
- `connected` não é evidência de que uma ferramenta é utilizável; o Atlas compara as ferramentas esperadas e as expostas.
- Sob a estratégia A a CLI inicia e para servidores stdio fora do guard de processos do Atlas, e os resultados das
  ferramentas não passam pelo Atlas. A autoridade do Atlas é o que ele expõe no lançamento e o que repassa depois.
- Contagens reais de tokens de entrada, morte de servidor no meio da execução, comportamento no cancelamento,
  auto-memória/`CLAUDE.md` chegando ao modelo e a semântica do `--append-system-prompt` precisam de uma execução real com
  modelo. Nenhuma foi feita; o Atlas perguntará antes de gastar tokens.

## Fase B (implementada)

As decisões 2–6 estão implementadas como escritas; os detalhes e os refinamentos estão na página de desenho, seção 15.
Em resumo: os `ContextBudget` do engine e do Harness são substituídos (`ExecutionBudget`, `HarnessBudget`); `Precision`
é adicionado ao lado do `TokenSource` da fase 0 em vez de renomeá-lo; `ModelRuntime::delivery()` é a única fonte do
payload e `manifest.promptHash` é o seu SHA-256 (`sha2`, já na árvore), provado com um `ProcessRunner` espião pelo
serviço real e para os cinco adaptadores; `RuntimeSurface` classifica por controle e observação e confirma apenas o
prompt entregue como recebido pelo modelo. Nenhum limite de modelo é conhecido por nenhum runtime, então todo limite é
`Unknown` a menos que o usuário configure um. Não alterados: Apply, worktrees, handoffs, semântica de workflow, os
guardrails, o isolamento do Claude da ADR 0024.

## Não feito

Rules, Memory, MCP, o canal de system prompt, a impressão digital da avaliação do guardrail dentro do manifest, e tudo
o que a página de desenho lista em "Ainda desconhecido ou não feito".
