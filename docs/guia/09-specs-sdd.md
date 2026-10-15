# 9. Specs: desenvolvimento orientado a especificação

> Parte do [guia do Atlas](README.md). ADR: [0035](../architecture/adr/0035-spec-driven-development.md). Sempre disponível: a
> escolha entre Vibe coding e spec é feita na hora de executar.

Antes do código, o Atlas escreve a **especificação**: uma proposta, os requisitos, um design e uma lista de tarefas, no
formato do [OpenSpec](https://github.com/Fission-AI/OpenSpec). Agentes escrevem; **você aprova cada documento**; um revisor
aponta lacunas; só então um workflow implementa a partir dela.

```text
pedido ─▶ proposta ─▶ requisitos ─▶ design ─▶ tarefas ─▶ revisão final ─▶ pronta ─▶ (publicar) ─▶ workflow
            │            │            │          │            │
        aprovar      aprovar      aprovar    aprovar   validações + revisor + sua aprovação
```

## Como usar e testar

1. O módulo está sempre disponível, sem configuração: o menu **Specs** existe e, ao iniciar um workflow, você escolhe na hora entre **Vibe coding** (o padrão, como sempre foi) e **A partir de uma spec aprovada**.
2. Menu **Specs → Nova especificação**: título, o que construir, agentes que escrevem e revisam. Criar **só grava** o pedido.
3. **Iniciar especificação**: o agente escreve a proposta. Você lê, **Aprovar e continuar**, **Solicitar alterações**
   (texto obrigatório) ou **Escrever de novo**. Cada decisão nomeia a revisão que você viu.
4. Depois dos quatro documentos aprovados, **Revisão final**: rode o revisor, decida os achados, aprove a especificação e
   publique em `openspec/changes/<id>/`.
5. **Workflow → Como começar?**: escolha **Vibe coding** (como sempre) ou **A partir de uma spec aprovada**.

Nada exige a CLI do OpenSpec, Node ou Python. Testes: `npm run check` (usa agentes simulados; nenhuma chamada a modelo).
O teste que usa a CLI real é opcional e não faz parte da suíte:
`cargo test --lib application::specs::real_cli_tests::real_openspec_cli_accepts_what_atlas_publishes -- --ignored --exact`.

## Modelo

| Peça              | Onde                                         | O que é                                                                                   |
| ----------------- | -------------------------------------------- | ----------------------------------------------------------------------------------------- |
| `Spec`, artefatos | `domain/spec/model.rs`                       | O registro: revisões, execução, revisão humana, validação, achados, perguntas, publicação |
| Regras            | `domain/spec/rules.rs`                       | Transições, prontidão, status derivado. Puras e testadas                                  |
| `SpecFramework`   | `application/specs/framework.rs`             | O contrato do formato                                                                     |
| `OpenSpecAdapter` | `application/specs/openspec/`                | Grafo, instruções, leitura da saída, validação, publicação, importação                    |
| `SpecService`     | `application/specs/service.rs`               | Persistência, decisões, publicação, importação, contexto de implementação                 |
| `SpecRunner`      | `application/specs/runner.rs`                | Monta o prompt e roda o turno do agente via `ChatService` (somente leitura)               |
| Armazenamento     | `infrastructure/spec_store.rs`               | `<dados do app>/specs/<id>/spec.json`, escrita atômica                                    |
| Projeto           | `infrastructure/project_files.rs`            | `openspec/` do projeto: nunca sobrescreve, nunca atravessa symlink                        |
| Workflow          | `application/specs/implementation.rs`        | Inicia o run a partir de uma spec pronta; progresso das tarefas                           |
| CLI opcional      | `application/specs/cli.rs`, `cli_service.rs` | `validate` e `archive` pelo guard (`ProcessContext::Tool`)                                |
| Interface         | `src/features/specs`                         | Visão geral, detalhe com etapas, documento, inspetor, dock de decisão, revisão final      |

## Estados

- **Execução** de um documento: `idle` · `running` · `waiting_for_input` · `failed`.
- **Revisão humana**: `none` · `awaiting_review` · `changes_requested` · `approved`.
- **Fase** (para a tela): `blocked` · `ready` · `generating` · `waiting_for_input` · `awaiting_review` · `changes_requested` ·
  `approved` · `outdated` · `failed`.
- **Status do spec** (derivado): `draft` · `awaiting_generation` · `generating` · `waiting_for_input` · `awaiting_review` ·
  `changes_requested` · `awaiting_validation` · `blocked` · `approved` · `ready_for_implementation` · `published` ·
  `archived` · `failed`.

Alterar um documento já aprovado marca os que dependem dele como **desatualizados** (não os aprova nem os refaz): você
mantém como está (volta para revisão) ou manda o agente atualizar.

## O que torna uma spec "pronta"

Tudo ao mesmo tempo: os quatro documentos aprovados e atuais; estrutura sem erros em cada revisão; validação entre os
documentos sem erros; nenhuma pergunta aberta; revisão por IA feita **sobre estas revisões** (ou dispensada por escrito);
nenhum achado grave sem decisão; **sua aprovação final**; e, se houver publicação, ela coerente. Qualquer mudança em um
documento anula a aprovação final, a revisão e as validações.

## Perguntas dos agentes

Um agente que não pode seguir sem uma informação termina com um bloco `atlas-interaction` (`clarification`). A pergunta
fica gravada no spec (sobrevive a reinício); sua resposta volta ao agente num bloco `HUMAN INPUT` e a execução continua.

## Como o agente é chamado

Uma execução comum (`ChatService`), **somente leitura** (`allow_edits` é sempre falso), com o Task Context do Harness e o
projeto da spec. Não há `/opsx:*` nem `/speckit-*` (os runtimes rodam com `--disable-slash-commands`) e nenhum agente roda
a CLI. A resposta é o documento, entre marcadores (`<<<ATLAS-DOCUMENT path="…">>>`); o Atlas valida o caminho, o tamanho e o
formato antes de gravar. Os documentos já aprovados vão no prompt (até 40 000 caracteres, com aviso de corte), porque os
rascunhos ficam fora do projeto que o agente alcança.

## Importar

**Importar specs** lista as mudanças de `openspec/changes/` do projeto (sem `archive`), diz o que falta (`missing:design`…),
lê os arquivos **sem alterá-los** e cria o spec com os documentos como revisão 1, **nada aprovado**. Os documentos que
faltam podem ser escritos a partir dos que existem.

## Implementar

Só uma spec pronta do workspace inicia um run. O run guarda a spec, as revisões, o resumo e as tarefas. Os agentes seguem
`tasks.md`, não editam `openspec/` e sinalizam divergências (`spec-divergence`). Na tela do workflow aparece o painel da
spec: tarefas **relatadas** pelos passos e **confirmadas** (o run terminou com sucesso). Se a spec mudar depois, o painel diz.

## Ferramentas para testar e revisar

Ao criar a spec, o formulário lista as conexões MCP do workspace (Jira, Figma, DevTools…). As que estão ligadas já vêm
marcadas; as desligadas aparecem sem poder ser escolhidas. Depois dá para mudar em **Agentes e ferramentas**. O que você
escolhe é citado aos agentes que escrevem os documentos (as verificações das tarefas passam a poder usar a ferramenta) e
aos que implementam. **Escolher não concede acesso**: a etapa só recebe as ferramentas que o hub MCP conceder a ela.

## Consumo e limite de tokens

Abaixo das etapas, a barra **Consumo** mostra o que as execuções da spec usaram, como o runtime informou (entrada, saída,
cache, custo), e quantas execuções informaram números. Aberta, lista execução por execução. Um runtime que não informa uso
aparece como "não informado", nunca como zero.

Em **Configurações → Limite de tokens por especificação** (ou `optimization.specsMaxTokens` no `config.json`) você define
o quanto uma spec pode usar. Ao chegar nele nenhuma execução nova começa e a barra oferece **Continuar**, que concede o
limite mais uma vez. Vazio: sem limite.

Para gastar menos: pedidos de alteração em documentos longos voltam só as seções que mudam, e a segunda revisão sabe o
que mudou desde a primeira (ADR 0036).

## Limitações

Ver [ADR 0035](../architecture/adr/0035-spec-driven-development.md#o-que-não-foi-feito-e-por-quê). As principais:
um worktree isolado não enxerga os arquivos da spec (o conteúdo vai no brief, até 24 000 caracteres, e o que não couber é marcado como cortado); arquivar com
mesclagem de deltas exige a CLI; sem `skip_specs`; sem diff entre revisões.
