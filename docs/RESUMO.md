# Atlas em uma página

Resumo do [guia técnico](guia/README.md). Cada linha aponta para o documento que detalha.

## A ideia

O Atlas é um app desktop (Tauri 2 + Rust + React) que **controla agentes de IA** que trabalham em projetos locais.
**A IA sugere e faz o trabalho; o Atlas controla o processo.** Texto de agente é _dado_, nunca _comando_: nada que um
agente escreva muda permissões, rotas, aprovações ou política.

## Quem usa o quê

| Peça                  | Usa / é                                                                                                                             | Detalhe                                    |
| --------------------- | ----------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------ |
| Agente                | personalidade + runtime + modelo + instruções + perfil de permissão + isolamento (ligado) + contrato de resultado                   | [02](guia/02-agentes-runtimes-e-prompt.md) |
| Modelo de IA          | Acessado por **CLI local** já logada pelo usuário: `claude`, `opencode`, `gemini`, `agy` (Codex só detecção). **Sem chave de API.** | [02](guia/02-agentes-runtimes-e-prompt.md) |
| Frontend              | React/TS; só chama **comandos Tauri** com allow-list; nunca toca disco/processo/Git/rede                                            | [01](guia/01-visao-geral-e-arquitetura.md) |
| Core                  | Rust em camadas `commands → application → domain`; adaptadores nas bordas; wiring só em `lib.rs`                                    | [01](guia/01-visao-geral-e-arquitetura.md) |
| Personalidades        | Architect, Developer, QA, Architecture & Code Validator (somente leitura), Bug Fixer — editáveis; dá para criar outras              | [02](guia/02-agentes-runtimes-e-prompt.md) |
| Código escrito por IA | Só em **worktree Git isolado**, nunca no checkout; o usuário decide se entra no projeto                                             | [05](guia/05-seguranca-e-isolamento.md)    |

## Como o prompt é montado (sempre pelo `PromptBuilder`)

`SYSTEM/PERSONALITY` (personalidade + regras do Atlas + narração ao vivo + regra de planos) → `TASK CONTEXT` _ou_
`PROJECT HARNESS` → `PROJECT CONTEXT` (nome, caminho do **worktree**, tecnologias) → `USER INSTRUCTION`
(instruções do agente + `Task:`). Passo de workflow: a tarefa vira um _brief_ (contexto do workflow + handoff +
protocolos). O texto exato fica guardado em cada execução. → [02](guia/02-agentes-runtimes-e-prompt.md)

## Padrões de resposta (o que o Atlas espera ler)

| Situação                                            | Formato                                                                                                                                                | Regra de ouro                                                                                                        |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------- |
| Fim de um passo de workflow                         | Bloco ` ```atlas-result ` com JSON: `status`, `outcome`, `summary`, `artifacts`, `decisions`, `findings`, `touchedFiles`, `touchedAreas`, `nextAction` | `outcome` só vale se **declarado no contrato** do agente; prosa nunca vira "pass/fail"; sem bloco = status `unknown` |
| Agente precisa de uma pessoa                        | Bloco ` ```atlas-interaction ` com JSON: `type` (`clarification`/`approval`/`permission`), `question`, `context`, `options`                            | Execução fica **esperando** (não "concluída"); botões de aprovar são do Atlas                                        |
| Plano / design / análise                            | Markdown **completo** na mensagem + `docs/plans/<nome>.md` se puder criar arquivos                                                                     | Plano nunca é só uma linha                                                                                           |
| Narração                                            | Uma frase curta antes de cada passo; resposta final **autossuficiente**                                                                                | A resposta guardada é a mensagem de conclusão                                                                        |
| Contrato `validation` / `review` / `implementation` | `pass`·`fail` / `approved`·`changes_requested` / `implemented`·`partial`·`blocked`                                                                     | Status de execução ≠ outcome                                                                                         |
| Análise semântica do projeto                        | **JSON em esquema fixo**, citando só arquivos que foram enviados                                                                                       | Campo desconhecido ou prosa = rejeitado                                                                              |
| Condições de rota                                   | Só `result.status/outcome/summary/next_action/findings/matched`, `validation.status` + `equals/not_equals/exists/not_exists`                           | Sem expressões, sem código                                                                                           |

→ [04](guia/04-workflow.md)

## Como o contexto do projeto é gerado e usado (Harness)

1. **Escanear** (só leitura; nada executado; `.env` nunca lido) → 2. **Analisar** por regras (fatos com evidência) →
2. _(opcional)_ **análise semântica** por modelo **sem ferramentas** → 4. **checar contradições** →
3. **revisão humana obrigatória** → 6. gravar em **`.atlas/`** dentro do projeto (backup antes de substituir).

- Cada afirmação: `fact` / `inference` / `user_*`, com **evidência**, confiança e verificação. Modelo nunca passa de
  confiança média. **Desconhecido > inventado.**
- Arquivos: `project.yaml`, `knowledge/findings.yaml` (gerado), `context/*.md` (gerados: arquitetura, stack,
  convenções, testes; **do usuário**: negócio, restrições, decisões — nunca sobrescritos).
- **Frescor**: fingerprint do projeto → achados cuja evidência mudou ficam `stale`. Refresh mostra diff e só escreve
  após confirmação.
- **Uso**: para cada tarefa, o **Task Context** seleciona por regras determinísticas (tags, áreas, pesos; ~6 000
  chars) só o que a tarefa precisa; **restrições e decisões sempre entram**; o que ficou de fora é dito no texto;
  sem seleção possível → Harness inteiro (motivo registrado). É **contexto, nunca permissão**.

→ [03](guia/03-harness-e-contexto.md)

## Como o workflow funciona

```text
Tarefa ─▶ Workflow (grafo) ─▶ Engine (máquina de estados pura) ─▶ Orquestrador ─▶ ExecutionService ─▶ Runtime/CLI
```

- Nós: **agente**, **condição**, **fim**. Arestas com condição sobre um conjunto fechado de fatos. **Loops sempre limitados.**
- Resultado sem rota ⇒ o run **falha** (`no_route_matched`); nunca é adivinhado.
- Passos **não conversam**: trocam **handoff** (artefatos + decisões + findings + arquivos medidos pelo Git), citado e
  sem autoridade. Até 4 passos em paralelo; um agente nunca roda dois.
- **Um worktree por run**, compartilhado; quem **escreve** o tem em exclusivo (lock), leitores compartilham.
- Templates determinísticos (feature, bug fix, refactor, code review…); **sem planejador por LLM**; nunca cria agente.
- Validação **antes** de rodar (outcomes declarados × rotas × alcançar um End); reparo de rota só com confirmação.
- Fechou o app no meio? Run fica `interrupted`; o usuário escolhe resume / restart / cancel.

→ [04](guia/04-workflow.md)

## Entrega do código

`completed` **≠** código no projeto. O código fica no worktree até o **usuário** escolher: **Apply** (copia para a
árvore de trabalho, **sem commit/merge/push**; conflito ⇒ nada é escrito), **Keep isolated**, **Discard** (branch
mantida), **Review** do diff real ou **abrir no editor**. O Live Workspace mostra o código sendo escrito (Git contra o
commit-base, nunca o que o agente "diz"). → [04](guia/04-workflow.md)

## Segurança em 8 linhas

1. Permissões em camadas que **só restringem**: máximo global ∩ workspace ∩ perfil do agente ∩ ferramentas do runtime.
2. Perfis: `read_only` (padrão) e `developer`; rede e Git destrutivo **nunca** automáticos.
3. Todo processo passa pelo **guard**: sem shell, sem launchers, sem env perigosa, cwd dentro do projeto/worktree.
4. Aprovação só pelo **usuário** na UI (timeout 5 min = rejeitado).
5. Editar arquivos exige **runtime capaz + política permite + worktree isolado** — os três.
6. Agentes **não rodam shell, testes nem rede** (ainda).
7. Terminal ao vivo (PTY) é controle (Ctrl+C / terminar), não sandbox; **não há sandbox de SO**.
8. Nenhum comando Tauri aceita programa, caminho ou aprovação do webview.

→ [05](guia/05-seguranca-e-isolamento.md)

## O que é gravado

| O quê                                            | Onde                                                  | Limite / regra                                                 |
| ------------------------------------------------ | ----------------------------------------------------- | -------------------------------------------------------------- |
| Estado, agentes, histórico, workflows, runs, uso | `config.json` (dir. de dados do app), escrita atômica | 200 msgs · 100 execuções · 300 eventos/execução · uso 100 dias |
| Conhecimento do projeto                          | `<projeto>/.atlas/`                                   | Versionável; arquivos do usuário nunca sobrescritos            |
| Worktrees                                        | `<app data>/worktrees/<workspace>/exec-NNNNNN`        | Fora do projeto; branch `atlas/exec-NNNNNN`                    |
| Saída de terminal                                | só memória (512 KiB/sessão)                           | Nunca em disco                                                 |
| Segredos                                         | em lugar nenhum                                       | Auth = sessão da CLI                                           |

Uso/custo **nunca inventados** (`null` ≠ `0`; fonte sempre marcada). Tokens que o Atlas calcula são **estimativas**
(`chars/4`). Métricas de prompt/latência (fase 0) são **só observação** e não alteram o prompt. → [06](guia/06-persistencia-eventos-e-observabilidade.md)

## Princípios que se repetem

- **Determinístico > modelo** onde der: seleção de contexto, templates, roteamento, detecção — regras testáveis.
- **Evidência > inferência > palpite; desconhecido > inventado.**
- **Git é a verdade** sobre o que mudou, não o que o agente diz.
- **Nada destrutivo, nada silencioso**: backups, diffs antes de escrever, falha explícita em vez de fallback perigoso.
- **O usuário decide** o que entra no projeto, o que é aprovado e o que é aplicado.
