# Benchmark: Atlas × Claude/Codex puros

Mede se rodar uma tarefa pelo Atlas (um workflow de agentes) entrega código melhor do que usar o Claude Code ou o Codex
direto, e quanto isso custa em tokens e tempo. Serve para justificar o Atlas com números e para medir o efeito de cada
melhoria (rode de novo e compare as planilhas).

**Todas as execuções chamam modelos reais na conta do usuário.** Avise antes de rodar.

## Braços

| Braço          | Como roda                                                                                                                   |
| -------------- | --------------------------------------------------------------------------------------------------------------------------- |
| Claude puro    | `claude -p <tarefa> --model <m> --output-format json --permission-mode acceptEdits` numa cópia do fixture                   |
| Codex puro     | `codex exec --json --sandbox workspace-write --model <m> <tarefa>` numa cópia do fixture                                    |
| Atlas + Claude | teste `bench_atlas_task` (backend real, CLI real, worktree) com o template do `meta.json`, todos os agentes no mesmo modelo |
| Atlas + Codex  | idem, no Codex                                                                                                              |

## Tarefas (`tasks/<id>/`)

Cada uma tem `task.md` (o texto exato que todos os braços recebem), `fixture/` (o projeto), `hidden/` (testes que o
agente nunca vê, copiados só na avaliação) e `meta.json` (template do Atlas, arquivos permitidos, defeitos da revisão).
Os testes ocultos foram validados contra uma solução de referência e falham no código original. T1–T4 são as tarefas
básicas; T5–T7 (limitador de taxa, CSV com defeitos entrelaçados, cancelamento em 3 arquivos) são mais difíceis, mas
ainda bem especificadas: em 2026-10-31 o Claude puro (haiku) acertou 100% delas também.

Uma tarefa sobre o repositório de outra pessoa não carrega uma cópia dele: a T8 (backend da Oficina das Meninas) aponta
para o repositório público e um commit fixo em `meta.json` (`fixtureRepo`). `bench/fixture.mjs` clona esse commit uma vez em
`bench/.cache/fixtures/` (fora do Git) e as árvores que as execuções dessa tarefa deixam ficam em `bench/.cache/result-trees/`,
nunca em `bench/results`.

## Rodar

Um comando faz tudo: confere a máquina (CLIs logadas, Python com openpyxl, Maven e `~/.m2` para a tarefa Java),
compila o Atlas uma vez, roda os braços, avalia e gera a planilha comparada com a rodada anterior.

```bash
npm run bench:check   # grátis: modelo falso, T1, confere o encanamento de ponta a ponta
npm run bench         # rodada real: 5 braços × 8 tarefas × 2 rodadas em bench/results/<hoje> (gasta tokens)
```

- Opções (depois de `npm run bench --`): `--arms claude-pure,atlas-claude`, `--tasks t1,t8`, `--reps 1`,
  `--claude-model haiku`, `--codex-model gpt-5.6-luna`, `--out <pasta>`, `--baseline <pasta>|none`.
- `--skip-existing` retoma uma rodada parada na mesma pasta; `config.json` guarda cada invocação.
- Falha da conta (cota esgotada, login) não vira resultado: a execução fica em `account-failure.json`, o resto daquele
  runtime é pulado e `--skip-existing` tenta de novo depois.
- `config.json` registra o commit do Atlas medido (e se havia mudanças sem commit em `src`/`src-tauri`).
- Os passos à mão continuam valendo: `node bench/run.mjs …`, `node bench/grade.mjs <pasta>`,
  `python3 bench/report.py <pasta> [--baseline <pasta>]`.
- `rubric.json` (nota manual por execução) e `findings.json` (melhorias) na pasta de resultados entram na planilha se
  existirem.

O relatório para apresentar é `relatorio.html` (gerado por `report_html.py`, uma página sem dependências): veredito,
resumo por braço, gráficos por tarefa, onde o código entregue estava errado, onde o Atlas gasta, a análise escrita
(`findings.json`), a evolução (`bench/history.json`, uma entrada por rodada que mediu uma mudança no Atlas), a
metodologia e todas as execuções. Só `findings.json` e `history.json` são escritos à mão; o resto vem dos dados.

A planilha (`benchmark-atlas.xlsx`) tem Resumo, Por tarefa, Melhorias, Execuções, Passos Atlas, Qualidade e Metodologia.
As médias são fórmulas sobre a aba Execuções.
