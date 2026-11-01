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

## Rodar

```bash
node bench/run.mjs --out bench/results/<data> --reps 2 --claude-model haiku --codex-model gpt-5.6-luna
node bench/grade.mjs bench/results/<data>
python3 bench/report.py bench/results/<data> [--baseline bench/results/<data anterior>]
```

- `--arms claude-pure,atlas-claude` e `--tasks t1,t3` restringem a matriz; `--skip-existing` retoma uma rodada parada.
- Para checar o encanamento sem gastar nada, passe um modelo que não existe (`--claude-model not-a-real-model-xyz
--codex-model not-a-real-model-xyz`): as CLIs falham na primeira chamada.
- `rubric.json` (nota manual por execução) e `findings.json` (melhorias) na pasta de resultados entram na planilha se
  existirem.

A planilha (`benchmark-atlas.xlsx`) tem Resumo, Por tarefa, Melhorias, Execuções, Passos Atlas, Qualidade e Metodologia.
As médias são fórmulas sobre a aba Execuções.
