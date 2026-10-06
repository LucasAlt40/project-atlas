# Guia técnico do Atlas

Documentação detalhada de **como o Atlas funciona hoje**: arquitetura, decisões que envolvem a IA, como as peças se
ligam, como o contexto é gravado, como o workflow roda e como o Harness é gerado.

> Quer só o resumo? Leia [**RESUMO.md**](../RESUMO.md) (uma página, formato "o agente usa X, a resposta vem no padrão Y").
> Quer o histórico de cada decisão? Veja os [ADRs](../architecture/README.md#adrs).

## Índice

| #   | Documento                                                                               | Responde a                                                                  |
| --- | --------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| 1   | [Visão geral e arquitetura](01-visao-geral-e-arquitetura.md)                            | O que é o Atlas, camadas, módulos Rust, conceitos do domínio, frontend      |
| 2   | [Agentes, runtimes e prompt](02-agentes-runtimes-e-prompt.md)                           | Como um agente é definido, como cada CLI é chamada, como o prompt é montado |
| 3   | [Harness e contexto](03-harness-e-contexto.md)                                          | Como o `.atlas/` é gerado, o modelo de conhecimento, Task Context           |
| 4   | [Workflow](04-workflow.md)                                                              | Engine, orquestrador, handoff, contratos, humano no loop, entrega do código |
| 5   | [Segurança e isolamento](05-seguranca-e-isolamento.md)                                  | Política de permissões, guard, worktrees, PTY e controle de processos       |
| 6   | [Persistência, eventos e observabilidade](06-persistencia-eventos-e-observabilidade.md) | O que é gravado e onde, uso/custo, eventos, métricas de otimização          |

## Ordem de leitura sugerida

1. **Quem chega agora**: [RESUMO](../RESUMO.md) → 1 → 2.
2. **Quem vai mexer em IA/prompt/contexto**: 2 → 3 → 6.
3. **Quem vai mexer em workflows**: 4 → 5.
4. **Quem vai revisar segurança**: 5 → 4 (seção "O que um workflow não pode fazer").

## Convenções desta documentação

- Descreve o **estado atual do código**, não intenções. Onde algo é planejado ou não validado, está dito explicitamente.
- Nomes de tipos/arquivos/comandos ficam em `código`; caminhos são relativos à raiz do repositório
  (`src-tauri/src/...`, `src/...`).
- Quando o código e esta documentação divergirem, **o código vence** — e a documentação deve ser corrigida.
- Os ADRs em `docs/architecture/adr/` (em inglês) continuam sendo o registro das decisões; este guia os consolida e
  referencia, não os substitui.
