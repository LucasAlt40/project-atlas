# Camada de Otimização — status e decisões

O objetivo: menos tokens, menos latência e menos ruído por execução **sem** perder qualidade nem segurança. A regra: medir
antes de otimizar, nunca chamar algo de otimizado sem números, e nunca deixar uma otimização remover contexto
obrigatório ou de segurança. Esta página é o mapa; cada fase tem a sua ADR.

> A próxima evolução (orçamento de execução, manifest de contexto, rules, memory, MCP) está desenhada em
> [context-and-tooling-platform.md](context-and-tooling-platform.md) e na ADR 0026. Ela não muda as fases abaixo. A sua
> Fase B é construída sobre esta camada: o `OptimizationMetrics` agora também carrega o `budget`, o `plan` e o
> `manifest` da execução (e `latency.deliveryMs`); o Context Engine se ajusta a `ExecutionBudget::available_for_context()`
> em vez do seu próprio `ContextBudget`, com comportamento inalterado. A Fase C acrescenta rules e autoridade de contexto
> ao mesmo pipeline (`OptimizationMetrics.rules` e `.authority`) e move o plano e o manifest para a própria execução. A
> Fase D acrescenta `OptimizationMetrics.mcp` e dá ao Claude, e só ao Claude, servidores MCP que o Atlas escolheu (o
> isolamento da ADR 0024 continua valendo: `--strict-mcp-config` com somente esses).

## Em que ponto cada fase está

| Fase | O quê                                                                                                                                                                                                                                       | Estado                                                                                                           | Padrão                                                       | ADR                                               |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------ | ------------------------------------------------- |
| 0    | Observabilidade: seções do prompt, tokens estimados, tokens informados pelo runtime, latência, eventos, A/B e cargas de referência                                                                                                          | feita                                                                                                            | ligada (`optimization.metrics.enabled`)                      | [0020](adr/0020-optimization-observability.md)    |
| 1    | Context Engine: prioridades, deduplicação por linha, compressão de espaços, orçamento de tokens que informa em vez de cortar                                                                                                                | feita                                                                                                            | **desligada** (`optimization.context.enabled`, `.maxTokens`) | [0021](adr/0021-context-engine.md)                |
| 2    | Skills (formato Agent Skills): descoberta, ativação, recursos, verificações de qualidade, cache, seleção determinística                                                                                                                     | feita                                                                                                            | **desligada** (`optimization.skills.enabled`)                | [0022](adr/0022-skills.md)                        |
| 3    | RTK (filtragem de saída)                                                                                                                                                                                                                    | construída e depois **removida**: nenhum agente do Atlas executa comandos pelo Atlas, então não economizava nada | —                                                            | [0023](adr/0023-rtk-evaluated-and-removed.md)     |
| 4    | Superfície do runtime: o que um runtime consegue alcançar; Claude isolado dos servidores MCP e skills do usuário; registro por execução das ferramentas expostas, usadas e das extensões carregadas                                         | feita para o Claude                                                                                              | sempre ligada (endurecimento)                                | [0024](adr/0024-runtime-surface-and-isolation.md) |
| 5    | Guardrails (uma autoridade, `security/` estendido) e Revisão de Contexto: ALLOW / ASK / DENY / TRANSFORM antes de um agente começar e antes de um resultado ser repassado; revisão determinística do contexto; ASK por `PendingInteraction` | feita, endurecida na 5.1 (aprovações vinculadas à avaliação; revisão do ChangeSet antes do Apply)                | ligada (`optimization.guardrails.enabled`)                   | [0025](adr/0025-guardrails-and-context-review.md) |
| 6    | Hooks                                                                                                                                                                                                                                       | não iniciada                                                                                                     | —                                                            | —                                                 |
| 7    | Dashboard                                                                                                                                                                                                                                   | não iniciada                                                                                                     | —                                                            | —                                                 |

As fases 1 e 2 mudam o prompt, então ficam desligadas até alguém ligá-las deliberadamente. Com elas desligadas, o prompt
é idêntico byte a byte ao que o `PromptBuilder` monta (um teste prova isso, e as cargas de referência comparam os dois).

## O que foi medido

Todos os tamanhos são estimativas (`chars / 4`, sempre rotuladas `estimated`) a menos que o runtime os tenha informado
(`exact`).

- **Composição do prompt** (cargas de referência): uma tarefa simples tem cerca de 1,7 KB (418 tokens), em sua maioria
  texto fixo obrigatório (regras, narração, regra de plano, personalidade); um passo de workflow com um brief tem de 6 a
  8 KB, dos quais os protocolos são cerca de 1,8 KB por passo.
- **Deduplicação (fase 1)**: 4,5% de um brief real de workflow (o mesmo artefato e a mesma decisão aparecem no contexto
  do workflow e no handoff); de 9% a 22% em briefs sintéticos com muitos itens repetidos; nada em tarefas sem repetição.
  Nenhuma palavra do prompt original se perde.
- **Skills (fase 2)**: com dez skills de cerca de 800 tokens e uma relevante, a divulgação progressiva envia 808 tokens em
  vez de 8.160, e nenhum metadado do catálogo.
- **Guardrails (fase 5)**: um passo cujas instruções e skill discordam sobre o banco de dados pausa antes de qualquer
  processo iniciar, pergunta, e só executa depois do sim de uma pessoa (ou falha com segurança depois de um não);
  conferido com a pilha real e com a CLI real do Claude em um modelo que não existe (sem chamada de modelo). Um segredo
  no resultado de outro agente ou em contexto não confiável é retirado antes de viajar.
- **Superfície do runtime (fase 4)**: o lançamento do Claude pelo Atlas expunha 11 ferramentas, 8 servidores MCP, 48
  skills e 84 slash commands (medido com a CLI real, sem chamada de modelo); agora expõe 3 ferramentas, 0, 0 e 0.

## Decisões fáceis de esquecer

- O Atlas não é dono do loop de ferramentas dos runtimes de CLI. A saída das ferramentas nunca passa pelo Atlas, então a
  otimização de saída não tem sobre o que agir até que agentes executem comandos pelo Atlas (a ADR 0023 diz o que isso
  exigiria).
- O texto obrigatório (as regras do Atlas, os protocolos, os avisos de "não concede permissões") não é encurtado como
  otimização: cada aviso protege uma fonte diferente de texto não confiável. `LIVE_NARRATION` e `PLAN_RULE` são
  comportamento de produto.
- Um orçamento nunca trunca: ele deixa de fora itens inteiros abaixo da prioridade `High`, diz isso no prompt e, do
  contrário, informa o estouro.
- Modos de autonomia (Manual / Guiado / Auto) não existem no código; a fase 5 não os inventou. O Atlas não consegue se
  interpor às ferramentas que um runtime de CLI executa dentro do seu processo, então os guardrails atuam antes de o
  agente começar e antes de um resultado ser repassado.
- Nada aqui instala software nem edita a configuração do usuário (`~/.claude`, configurações de agentes).
- O Apply é sempre um ato de uma pessoa e nunca faz commit, digam o que disserem os guardrails: a revisão do ChangeSet só
  pode segurar um Apply (e uma pessoa pode aplicar de novo depois de ver um `NeedsReview`), nunca realizar um.
- Uma execução real com modelo não foi feita para nada disto; todo número acima vem de testes determinísticos ou da CLI
  informando o seu próprio estado de inicialização sem chamada de modelo.

## O que está pendente

1. **Uma execução real com medição**: tokens de entrada informados pelo runtime contra a estimativa do Atlas (calibra o
   `chars / 4`), tokens em cache, e quanto da entrada real o `CLAUDE.md`, os hooks e os plugins do usuário acrescentam.
   Custa tokens na conta do usuário, então espera o aval dele.
2. **Superfícies de OpenCode, Gemini e Antigravity** (que MCP e extensões eles carregam em execuções headless).
3. As fases 6 e 7, e as decisões listadas na ADR 0025 (autonomia; `--restricted` para o Claude: investigado, não adotado,
   uma pequena execução real resolveria).
4. Uma decisão sobre agentes executarem comandos pelo Atlas (segurança e produto), que é o que faria a filtragem de
   saída importar.
