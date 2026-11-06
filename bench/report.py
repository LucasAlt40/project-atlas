#!/usr/bin/env python3
"""Builds the benchmark spreadsheet from a graded results folder.

    python3 bench/report.py bench/results/2026-10-31 [--baseline bench/results/2026-10-30]

Reads all.json (grade.mjs), rubric.json (manual quality review, optional) and findings.json
(improvements, optional). Every aggregate is a formula over the "Execuções" sheet, so editing a
run's numbers there updates the summary.
"""
import json
import sys
from pathlib import Path

from openpyxl import Workbook
from openpyxl.comments import Comment
from openpyxl.formatting.rule import ColorScaleRule
from openpyxl.styles import Alignment, Border, Font, PatternFill, Side
from openpyxl.utils import get_column_letter

results = Path(sys.argv[1])
baseline = Path(sys.argv[sys.argv.index("--baseline") + 1]) if "--baseline" in sys.argv else None
rows = json.loads((results / "all.json").read_text())
config = json.loads((results / "config.json").read_text())
rubric = json.loads((results / "rubric.json").read_text()) if (results / "rubric.json").exists() else {}
findings = json.loads((results / "findings.json").read_text()) if (results / "findings.json").exists() else []

FONT = "Arial"
HEAD = PatternFill("solid", start_color="1F3A5F")
SUB = PatternFill("solid", start_color="E8EEF6")
ATLAS = PatternFill("solid", start_color="EAF5EA")
thin = Side(style="thin", color="C9D3DF")
BOX = Border(left=thin, right=thin, top=thin, bottom=thin)
ARMS = ["claude-pure", "atlas-claude", "atlas-claude-quick", "codex-pure", "atlas-codex"]
ARM_LABEL = {
    "claude-pure": "Claude puro",
    "atlas-claude": "Atlas + Claude",
    "atlas-claude-quick": "Atlas + Claude (Quick)",
    "codex-pure": "Codex puro",
    "atlas-codex": "Atlas + Codex",
}
TASK_LABEL = {
    "t1-slugify": "T1 Feature nova (slugify)",
    "t2-cart-bug": "T2 Bug por sintoma (carrinho)",
    "t3-todo-priority": "T3 Feature multi-arquivo (todo)",
    "t4-review": "T4 Revisão (3 defeitos)",
    "t5-rate-limiter": "T5 Limitador de taxa",
    "t6-csv-bug": "T6 Bug CSV entrelaçado",
    "t7-order-cancel": "T7 Cancelar pedido (3 arquivos)",
    "t8-oficina-recorrente": "T8 Bug real: doação recorrente (Oficina)",
}

present_arms = [a for a in ARMS if any(r["arm"] == a for r in rows)]
present_tasks = sorted({r["task"] for r in rows})
ran_codex = any(r["runtime"] == "codex" for r in rows)
atlas_v = config.get("atlas") or {}
atlas_desc = (
    f"Atlas {atlas_v.get('commit')} ({atlas_v.get('branch')}{', com mudanças sem commit' if atlas_v.get('dirty') else ''})"
    if atlas_v.get("commit") else "versão do Atlas não registrada"
)
# Runs lost to the account (quota, login): not results, but the report says how many.
account_failures = sorted(p.parent.name for p in (results / "runs").glob("*/account-failure.json") if not (p.parent / "metrics.json").exists())


def mean(xs):
    xs = [x for x in xs if x is not None]
    return sum(xs) / len(xs) if xs else None


def measured(rows):
    """Plain conclusions computed from the runs, so a fresh round has them without findings.json."""
    out = []
    by = {a: [r for r in rows if r["arm"] == a] for a in present_arms}
    def avg(arm, f):
        return mean([f(r) for r in by[arm]])
    total = len(rows)
    perfect = sum(1 for r in rows if r["grade"].get("hiddenScore", 0) >= 1)
    out.append(f"Testes ocultos: {perfect} de {total} execuções passaram 100% (os testes que o agente nunca viu).")
    for arm in present_arms:
        runs = by[arm]
        failed = [r["id"] for r in runs if not r["ok"]]
        broken = sum(1 for r in runs if (r["grade"].get("ownTests") or {}).get("fail"))
        score = avg(arm, lambda r: r["grade"].get("hiddenScore", 0))
        cost = avg(arm, lambda r: r.get("costUsd"))
        secs = avg(arm, lambda r: r["elapsedMs"] / 1000)
        out.append(
            f"{ARM_LABEL[arm]}: {len(runs)} execuções, nota oculta média {score:.0%}, "
            f"tempo médio {secs:.0f} s" + (f", custo médio US$ {cost:.4f}" if cost is not None else "")
            + f", teste próprio falhando em {broken}, não concluiu em {len(failed)}"
            + (f" ({', '.join(failed[:3])})" if failed else "") + "."
        )
    for runtime, label in (("claude", "Claude"), ("codex", "Codex")):
        pure = f"{runtime}-pure"
        for atlas_arm in (f"atlas-{runtime}", f"atlas-{runtime}-quick"):
            if pure not in by or atlas_arm not in by:
                continue
            # Only the tasks both arms ran: the Quick arm runs the feature tasks alone.
            common = {r["task"] for r in by[atlas_arm]} & {r["task"] for r in by[pure]}
            if not common:
                continue
            a_runs = [r for r in by[atlas_arm] if r["task"] in common]
            p_runs = [r for r in by[pure] if r["task"] in common]
            def ratio(f):
                a, p = mean([f(r) for r in a_runs]), mean([f(r) for r in p_runs])
                return a / p if a is not None and p else None
            c = ratio(lambda r: r.get("costUsd"))
            t = ratio(lambda r: r["elapsedMs"] / 1000)
            d = mean([r["grade"].get("hiddenScore", 0) for r in a_runs]) - mean([r["grade"].get("hiddenScore", 0) for r in p_runs])
            scope = "nas mesmas tarefas" if len(common) == len(present_tasks) else f"nas {len(common)} tarefas que os dois rodaram"
            out.append(
                f"{ARM_LABEL[atlas_arm]} comparado ao {label} puro ({scope}): nota oculta {d:+.0%}"
                + (f", custo {c:.2f}×" if c else "") + (f", tempo {t:.2f}×" if t else "") + "."
            )
    if account_failures:
        out.append(f"{len(account_failures)} execução(ões) não contaram por falha da conta (cota ou login): {', '.join(account_failures[:4])}.")
    return out


wb = Workbook()


def style_header(ws, row, ncols):
    for c in range(1, ncols + 1):
        cell = ws.cell(row=row, column=c)
        cell.font = Font(name=FONT, bold=True, color="FFFFFF")
        cell.fill = HEAD
        cell.alignment = Alignment(horizontal="center", vertical="center", wrap_text=True)
        cell.border = BOX
    ws.row_dimensions[row].height = 32


def body(ws, first, last, ncols):
    for r in range(first, last + 1):
        for c in range(1, ncols + 1):
            cell = ws.cell(row=r, column=c)
            cell.font = Font(name=FONT, bold=cell.font.bold, color=cell.font.color)
            cell.border = BOX
            if cell.alignment.wrap_text is None:
                cell.alignment = Alignment(vertical="top")


def widths(ws, ws_widths):
    for i, w in enumerate(ws_widths, start=1):
        ws.column_dimensions[get_column_letter(i)].width = w


def title(ws, text, note):
    ws["A1"] = text
    ws["A1"].font = Font(name=FONT, bold=True, size=14, color="1F3A5F")
    ws["A2"] = note
    ws["A2"].font = Font(name=FONT, italic=True, size=9, color="555555")


# ---------------------------------------------------------------- Execuções (raw, one per run)
ex = wb.active
ex.title = "Execuções"
title(ex, "Execuções (uma linha por execução)", "Números crus de cada execução. As abas de resumo calculam tudo a partir daqui.")
cols = [
    ("ID", 34), ("Braço", 15), ("Runtime", 9), ("Tarefa", 30), ("Tipo", 15), ("Rodada", 7),
    ("Concluiu (1/0)", 9), ("Nota testes ocultos", 10), ("Tokens entrada", 12), ("Tokens cache", 12),
    ("Entrada sem cache", 12), ("Tokens saída", 11), ("Custo US$ (informado)", 11), ("Tempo (s)", 9),
    ("Agentes rodados", 9), ("Arquivos alterados", 9), ("Fora do escopo", 9), ("Linhas +", 8),
    ("Linhas −", 8), ("Testes próprios (passam)", 10), ("Testes próprios (falham)", 10),
    ("Nota rubrica (0–10)", 10), ("Observação", 60),
]
HR = 4
for i, (name, _) in enumerate(cols, start=1):
    ex.cell(row=HR, column=i, value=name)
style_header(ex, HR, len(cols))
widths(ex, [w for _, w in cols])
order = {a: i for i, a in enumerate(ARMS)}
rows.sort(key=lambda r: (r["task"], order.get(r["arm"], 9), r["rep"]))
r0 = HR + 1
for n, run in enumerate(rows):
    r = r0 + n
    g = run["grade"]
    note = []
    if run["arm"].startswith("atlas"):
        note.append(f"status={run.get('status')}; passos={' > '.join(run.get('steps', []))}")
        if run.get("failure"):
            note.append(str(run["failure"])[:160])
    if g.get("outOfScope"):
        note.append("fora do escopo: " + ", ".join(g["outOfScope"]))
    if g.get("hidden", {}).get("failed"):
        note.append("ocultos falharam: " + "; ".join(g["hidden"]["failed"][:4]))
    if g.get("defects"):
        note.append("defeitos: " + ", ".join(f"{d['id']}={'sim' if d['found'] else 'não'}" for d in g["defects"]))
    if run.get("arm") == "codex-pure" or run.get("runtime") == "codex":
        note.append("Codex não informa custo (assinatura ChatGPT)")
    values = [
        run["id"], ARM_LABEL[run["arm"]], run["runtime"], TASK_LABEL.get(run["task"], run["task"]), g["kind"],
        run["rep"], 1 if run["ok"] else 0, round(g.get("hiddenScore", 0), 4), run.get("inputTokens"),
        run.get("cachedInputTokens"), f"=IF(I{r}=\"\",\"\",I{r}-N(J{r}))", run.get("outputTokens"),
        run.get("costUsd"), round(run["elapsedMs"] / 1000, 1), run.get("agentRuns"), len(g["changed"]),
        len(g["outOfScope"]), g["added"], g["removed"], g.get("ownTests", {}).get("pass"),
        g.get("ownTests", {}).get("fail"), rubric.get(run["id"], {}).get("score"),
        " | ".join(note + ([rubric[run["id"]]["note"]] if rubric.get(run["id"], {}).get("note") else [])),
    ]
    for c, v in enumerate(values, start=1):
        ex.cell(row=r, column=c, value=v)
    if run["arm"].startswith("atlas"):
        for c in range(1, len(cols) + 1):
            ex.cell(row=r, column=c).fill = ATLAS
last = r0 + len(rows) - 1
body(ex, r0, last, len(cols))
for r in range(r0, last + 1):
    ex.cell(row=r, column=8).number_format = "0%"
    for c in (9, 10, 11, 12):
        ex.cell(row=r, column=c).number_format = "#,##0"
    ex.cell(row=r, column=13).number_format = "$0.0000"
    ex.cell(row=r, column=23).alignment = Alignment(wrap_text=True, vertical="top")
ex.freeze_panes = "B5"
ex.auto_filter.ref = f"A{HR}:{get_column_letter(len(cols))}{last}"
ex.cell(row=HR, column=13).comment = Comment("total_cost_usd informado pela CLI do Claude (equivalente em API; no plano Pro sai da cota, não da fatura).", "bench")
ex.cell(row=HR, column=9).comment = Comment("Claude: input + cache_creation + cache_read (mesma conta do runtime do Atlas). Codex: input_tokens somado dos turn.completed (já inclui cache).", "bench")
ex.cell(row=HR, column=8).comment = Comment("Fração dos testes ocultos que passam (o agente nunca viu esses testes). Em T4: fração dos 3 defeitos plantados citados na revisão.", "bench")

R = f"'Execuções'!$B${r0}:$B${last}"   # arm label
T = f"'Execuções'!$D${r0}:$D${last}"   # task label


def rng(col):
    return f"'Execuções'!${col}${r0}:${col}${last}"


# ---------------------------------------------------------------- Resumo (per arm)
rs = wb.create_sheet("Resumo", 0)
title(
    rs,
    "Benchmark Atlas × Claude/Codex puros",
    f"{len(rows)} execuções · Claude {config['models']['claude']}" + (f" · Codex {config['models']['codex']}" if ran_codex else "") + f" · {config['reps']} rodadas · "
    f"{config['versions']['claude']}" + (f" · {config['versions']['codex']}" if ran_codex else "") + f" · {atlas_desc} · iniciado {config['startedAt'][:16]}",
)
heads = [
    ("Braço", 18), ("Execuções", 10), ("Concluiu", 10), ("Nota testes ocultos (média)", 13),
    ("Tokens entrada (média)", 13), ("Tokens cache (média)", 13), ("Entrada sem cache (média)", 13),
    ("Tokens saída (média)", 12), ("Tempo médio (s)", 11), ("Custo médio US$", 11),
    ("Agentes por tarefa", 10), ("Execuções fora do escopo", 11), ("Nota rubrica (média)", 11),
    ("Nota por 100 mil tokens de entrada", 13),
]
for i, (h, _) in enumerate(heads, start=1):
    rs.cell(row=4, column=i, value=h)
style_header(rs, 4, len(heads))
widths(rs, [w for _, w in heads])
for i, arm in enumerate(ARMS):
    r = 5 + i
    a = f"$A{r}"
    rs.cell(row=r, column=1, value=ARM_LABEL[arm]).font = Font(name=FONT, bold=True)
    f = {
        2: f"=COUNTIF({R},{a})",
        3: f"=IFERROR(AVERAGEIFS({rng('G')},{R},{a}),\"\")",
        4: f"=IFERROR(AVERAGEIFS({rng('H')},{R},{a}),\"\")",
        5: f"=IFERROR(AVERAGEIFS({rng('I')},{R},{a}),\"\")",
        6: f"=IFERROR(AVERAGEIFS({rng('J')},{R},{a}),\"\")",
        7: f"=IFERROR(E{r}-N(F{r}),\"\")",
        8: f"=IFERROR(AVERAGEIFS({rng('L')},{R},{a}),\"\")",
        9: f"=IFERROR(AVERAGEIFS({rng('N')},{R},{a}),\"\")",
        10: f"=IFERROR(AVERAGEIFS({rng('M')},{R},{a}),\"n/d\")",
        11: f"=IFERROR(AVERAGEIFS({rng('O')},{R},{a}),\"\")",
        12: f"=COUNTIFS({R},{a},{rng('Q')},\">0\")",
        13: f"=IFERROR(AVERAGEIFS({rng('V')},{R},{a}),\"\")",
        14: f"=IFERROR(D{r}/(E{r}/100000),\"\")",
    }
    for c, v in f.items():
        rs.cell(row=r, column=c, value=v)
    if arm.startswith("atlas"):
        for c in range(1, len(heads) + 1):
            rs.cell(row=r, column=c).fill = ATLAS
body(rs, 5, 4 + len(ARMS), len(heads))
for r in range(5, 5 + len(ARMS)):
    for c in (3, 4):
        rs.cell(row=r, column=c).number_format = "0%"
    for c in (5, 6, 7, 8):
        rs.cell(row=r, column=c).number_format = "#,##0"
    rs.cell(row=r, column=9).number_format = "0"
    rs.cell(row=r, column=10).number_format = "$0.0000"
    rs.cell(row=r, column=11).number_format = "0.0"
    rs.cell(row=r, column=13).number_format = "0.0"
    rs.cell(row=r, column=14).number_format = "0.00"

# Atlas vs pure, per runtime
rs["A11"] = "Atlas comparado ao runtime puro (Atlas ÷ puro; 1,00 = igual)"
rs["A11"].font = Font(name=FONT, bold=True, size=12, color="1F3A5F")
vs = ["Runtime", "Nota testes ocultos (Δ pontos)", "Tokens entrada (×)", "Entrada sem cache (×)", "Tokens saída (×)", "Tempo (×)", "Custo (×)", "Rubrica (Δ pontos)"]
for i, h in enumerate(vs, start=1):
    rs.cell(row=12, column=i, value=h)
style_header(rs, 12, len(vs))
row_of = {arm: 5 + i for i, arm in enumerate(ARMS)}
pairs = [
    ("Claude", row_of["claude-pure"], row_of["atlas-claude"]),
    ("Claude (Quick)", row_of["claude-pure"], row_of["atlas-claude-quick"]),
    ("Codex", row_of["codex-pure"], row_of["atlas-codex"]),
]
for i, (label, pure, at) in enumerate(pairs):
    r = 13 + i
    rs.cell(row=r, column=1, value=label).font = Font(name=FONT, bold=True)
    rs.cell(row=r, column=2, value=f"=IFERROR(D{at}-D{pure},\"\")").number_format = "+0%;-0%;0%"
    for c, col in ((3, "E"), (4, "G"), (5, "H"), (6, "I"), (7, "J")):
        rs.cell(row=r, column=c, value=f"=IFERROR({col}{at}/{col}{pure},\"n/d\")").number_format = "0.00\"×\""
    rs.cell(row=r, column=8, value=f"=IFERROR(M{at}-M{pure},\"\")").number_format = "+0.0;-0.0;0.0"
body(rs, 13, 12 + len(pairs), len(vs))

rs["A17"] = "Como ler"
rs["A17"].font = Font(name=FONT, bold=True, size=12, color="1F3A5F")
legend = [
    "Nota testes ocultos: fração dos testes que o agente nunca viu e que passam na árvore final (T4: defeitos plantados citados). É a medida principal de qualidade.",
    "Tokens entrada inclui o que veio do cache do provedor; 'sem cache' é o que foi processado de novo. Cache é bem mais barato, então os dois importam.",
    "Custo: só o Claude informa (total_cost_usd, preço de API). No plano Pro isso sai da cota, não da fatura. Codex na assinatura ChatGPT não informa custo.",
    "Fora do escopo: execuções que alteraram arquivos que a tarefa não permitia (ex.: revisão que editou código).",
    "Rubrica: nota manual 0–10 (corretude, legibilidade, escopo, testes) dada lendo o código final; detalhes na aba Qualidade.",
    "Linhas em verde são execuções pelo Atlas.",
]
for i, t in enumerate(legend):
    c = rs.cell(row=18 + i, column=1, value="• " + t)
    c.font = Font(name=FONT, size=10)
    rs.merge_cells(start_row=18 + i, start_column=1, end_row=18 + i, end_column=len(heads))
    c.alignment = Alignment(wrap_text=True, vertical="top")
    rs.row_dimensions[18 + i].height = 28
rs.freeze_panes = "B5"

# ---------------------------------------------------------------- Por tarefa (arm × task)
pt = wb.create_sheet("Por tarefa", 1)
title(pt, "Resultado por tarefa e braço (médias das rodadas)", "Cada célula é uma média sobre a aba Execuções.")
blocks = [("Nota testes ocultos", "H", "0%"), ("Tokens entrada", "I", "#,##0"), ("Tokens saída", "L", "#,##0"), ("Tempo (s)", "N", "0")]
row = 4
for name, col, fmt in blocks:
    pt.cell(row=row, column=1, value=name).font = Font(name=FONT, bold=True, size=12, color="1F3A5F")
    row += 1
    pt.cell(row=row, column=1, value="Tarefa")
    for j, arm in enumerate(ARMS, start=2):
        pt.cell(row=row, column=j, value=ARM_LABEL[arm])
    style_header(pt, row, len(ARMS) + 1)
    hr = row
    for task in TASK_LABEL.values():
        row += 1
        pt.cell(row=row, column=1, value=task)
        for j in range(2, len(ARMS) + 2):
            colL = get_column_letter(j)
            cell = pt.cell(row=row, column=j, value=f"=IFERROR(AVERAGEIFS({rng(col)},{R},{colL}${hr},{T},$A{row}),\"\")")
            cell.number_format = fmt
    body(pt, hr + 1, row, len(ARMS) + 1)
    if col == "H":
        pt.conditional_formatting.add(f"B{hr+1}:{get_column_letter(len(ARMS)+1)}{row}", ColorScaleRule(start_type="num", start_value=0, start_color="F8D7DA", end_type="num", end_value=1, end_color="C8E6C9"))
    row += 2
widths(pt, [34] + [16] * len(ARMS))

# ---------------------------------------------------------------- Passos do Atlas
ps = wb.create_sheet("Passos Atlas")
title(ps, "Onde o Atlas gasta: cada agente de cada execução", "Do ledger de uso do Atlas (o que o runtime informou por execução de agente).")
ph = [("Execução", 34), ("Papel", 22), ("Concluiu", 9), ("Tempo (s)", 10), ("Tokens entrada", 13), ("Tokens cache", 13), ("Tokens saída", 12), ("Custo US$", 11)]
for i, (h, _) in enumerate(ph, start=1):
    ps.cell(row=4, column=i, value=h)
style_header(ps, 4, len(ph))
widths(ps, [w for _, w in ph])
r = 5
for run in rows:
    for step in run.get("perStep", []) or []:
        vals = [run["id"], step.get("role"), 1 if step.get("ok") else 0, round((step.get("ms") or 0) / 1000, 1),
                step.get("inputTokens"), step.get("cachedInputTokens"), step.get("outputTokens"), step.get("cost")]
        for c, v in enumerate(vals, start=1):
            ps.cell(row=r, column=c, value=v)
        for c in (5, 6, 7):
            ps.cell(row=r, column=c).number_format = "#,##0"
        ps.cell(row=r, column=8).number_format = "$0.0000"
        r += 1
plast = r - 1
body(ps, 5, plast, len(ph))
ps.auto_filter.ref = f"A4:H{plast}"
ps.freeze_panes = "A5"
# Per-role averages
ps.cell(row=4, column=10, value="Papel")
ps.cell(row=4, column=11, value="Passos")
ps.cell(row=4, column=12, value="Tempo médio (s)")
ps.cell(row=4, column=13, value="Entrada média")
ps.cell(row=4, column=14, value="Saída média")
for c in range(10, 15):
    cell = ps.cell(row=4, column=c)
    cell.font = Font(name=FONT, bold=True, color="FFFFFF")
    cell.fill = HEAD
    cell.alignment = Alignment(horizontal="center", wrap_text=True)
roles = sorted({s.get("role") or "?" for run in rows for s in (run.get("perStep") or [])})
for i, role in enumerate(roles):
    rr = 5 + i
    ps.cell(row=rr, column=10, value=role)
    ps.cell(row=rr, column=11, value=f"=COUNTIF($B$5:$B${plast},J{rr})")
    ps.cell(row=rr, column=12, value=f"=IFERROR(AVERAGEIFS($D$5:$D${plast},$B$5:$B${plast},J{rr}),\"\")").number_format = "0"
    ps.cell(row=rr, column=13, value=f"=IFERROR(AVERAGEIFS($E$5:$E${plast},$B$5:$B${plast},J{rr}),\"\")").number_format = "#,##0"
    ps.cell(row=rr, column=14, value=f"=IFERROR(AVERAGEIFS($G$5:$G${plast},$B$5:$B${plast},J{rr}),\"\")").number_format = "#,##0"
    for c in range(10, 15):
        ps.cell(row=rr, column=c).font = Font(name=FONT)
        ps.cell(row=rr, column=c).border = BOX
for c, w in zip("JKLMN", (22, 8, 12, 13, 12)):
    ps.column_dimensions[c].width = w

# ---------------------------------------------------------------- Qualidade (rubrica)
qs = wb.create_sheet("Qualidade")
title(qs, "Rubrica de qualidade (leitura manual do código final)", "Corretude 0–4 · Legibilidade 0–2 · Escopo/minimalismo 0–2 · Testes escritos 0–2 = nota 0–10.")
qh = [("Execução", 34), ("Corretude (0–4)", 11), ("Legibilidade (0–2)", 11), ("Escopo (0–2)", 10), ("Testes (0–2)", 10), ("Nota (0–10)", 10), ("Comentário", 90)]
for i, (h, _) in enumerate(qh, start=1):
    qs.cell(row=4, column=i, value=h)
style_header(qs, 4, len(qh))
widths(qs, [w for _, w in qh])
r = 5
for run in rows:
    q = rubric.get(run["id"])
    if not q:
        continue
    qs.cell(row=r, column=1, value=run["id"])
    for c, k in ((2, "correct"), (3, "readable"), (4, "scope"), (5, "tests")):
        qs.cell(row=r, column=c, value=q.get(k))
    qs.cell(row=r, column=6, value=f"=SUM(B{r}:E{r})")
    qs.cell(row=r, column=7, value=q.get("note")).alignment = Alignment(wrap_text=True, vertical="top")
    r += 1
body(qs, 5, r - 1, len(qh))
qs.freeze_panes = "B5"

# ---------------------------------------------------------------- Melhorias
ms = wb.create_sheet("Melhorias", 2)
title(ms, "O que melhorar no Atlas (saído dos números)", "Prioridade: Alta = muda o resultado do benchmark; Média = custo/tempo; Baixa = acabamento.")
mh = [("#", 4), ("Achado (evidência)", 60), ("Melhoria proposta", 60), ("Métrica que deve mexer", 24), ("Prioridade", 10), ("Esforço", 9)]
for i, (h, _) in enumerate(mh, start=1):
    ms.cell(row=4, column=i, value=h)
style_header(ms, 4, len(mh))
widths(ms, [w for _, w in mh])
for i, f_ in enumerate(findings):
    r = 5 + i
    for c, v in enumerate([i + 1, f_["finding"], f_["fix"], f_["metric"], f_["priority"], f_["effort"]], start=1):
        cell = ms.cell(row=r, column=c, value=v)
        cell.alignment = Alignment(wrap_text=True, vertical="top")
body(ms, 5, 4 + len(findings), len(mh))

# ---------------------------------------------------------------- Metodologia
md = wb.create_sheet("Metodologia")
title(md, "Plano e metodologia", "Para reproduzir: node bench/run.mjs → node bench/grade.mjs → python3 bench/report.py (ver bench/README.md).")
lines = [
    ("Objetivo", "Medir se rodar uma tarefa pelo Atlas (workflow de agentes) entrega código melhor que usar o Claude Code ou o Codex direto, e a que custo em tokens e tempo."),
    ("Braços", "Claude puro: claude -p <tarefa> --model haiku --output-format json --permission-mode acceptEdits, numa cópia do projeto. Codex puro: codex exec --json --sandbox workspace-write --model gpt-5.6-luna <tarefa>. Atlas + Claude / Atlas + Codex: o mesmo texto da tarefa num template de produto do Atlas (backend real, CLI real, worktree isolado), todos os agentes no mesmo modelo."),
    ("Templates do Atlas", "Features (T1, T3, T5, T7): 'Implement' (software_feature); o braço Quick usa 'Quick change' (feature_basic). Bugs (T2, T6, T8): 'Debug & fix' (bug_fix). Revisão (T4): 'Review & audit' (code_review). Planos aprovados automaticamente."),
    ("Tarefas", "T1 slugify com 6 regras e bordas (14 testes ocultos). T2 bug descrito só por sintomas num carrinho com 3 defeitos (11 testes ocultos). T3 prioridades num store de todo em 2 arquivos, sem quebrar o existente (12 testes ocultos, 4 deles de regressão). T4 revisão de um módulo com 3 defeitos plantados (paginação off-by-one, idade sem aniversário, escapeHtml que só troca a 1ª ocorrência). T5 limitador de taxa com relógio injetável (12 testes ocultos). T6 parser de CSV com 5 defeitos entrelaçados (12). T7 cancelamento de pedido em 3 arquivos (10). T8 bug REAL do backend Java/Spring da Oficina das Meninas: a cobrança mensal de uma doação recorrente sobrescreve o pagamento do mês anterior (7 testes ocultos JUnit/Mockito sobre o webhook do PagBank; 259 arquivos do repositório como fixture)."),
    ("Testes ocultos", "Ficam fora do projeto que o agente vê e só são copiados para a árvore final na avaliação. Foram validados contra uma solução de referência (T1 14/14, T2 11/11, T3 12/12, T5 12/12, T6 12/12, T7 10/10, T8 7/7) e falham no código original (T8: 3/7). No T8 rodam com Maven offline (mvn -o test), com um serviço de pagamentos falso que responde pelo nome dos métodos, para julgar o comportamento mesmo se a correção mudar a interface."),
    ("Tokens", "Claude: input + cache_creation + cache_read e output do evento result (a mesma conta que o runtime do Atlas faz). Codex: soma dos turn.completed. Atlas: soma do ledger de uso de todos os agentes da execução."),
    ("Tempo", "Relógio de parede da execução inteira (Atlas: do início do workflow ao fim, incluindo a montagem do worktree)."),
    ("Escopo", "Arquivos alterados comparados aos permitidos pela tarefa (meta.json → allowed)."),
    ("Qualidade", "Principal: testes ocultos. Complementar: testes que o agente escreveu (quantos passam), tamanho do diff, escopo e rubrica manual 0–10."),
    ("Ambiente", f"{config['versions']['claude']} · {config['versions']['codex']} · Node {config['versions']['node']} · macOS · projeto de teste fora do repositório (nenhum CLAUDE.md/AGENTS.md de projeto). O Claude puro carrega a configuração global do usuário (como no uso real); o Atlas isola o Claude (--strict-mcp-config, --disable-slash-commands) e lhe dá Bash só para os comandos concedidos (testes e git de leitura), no sandbox da CLI. Java 21 e Maven 3.9 offline (MAVEN_ARGS=-o) no PATH de todos os braços. Código medido: " + atlas_desc + "." + ("" if ran_codex else " O Codex não rodou nesta rodada.") + (f" {len(account_failures)} execução(ões) perdida(s) por falha da conta (cota ou login) ficaram de fora: {', '.join(account_failures)}." if account_failures else "")),
    ("Limites", f"Amostra pequena ({config['reps']} rodadas × {len(present_tasks)} tarefas): diferenças pequenas podem ser ruído. Tarefas pequenas favorecem quem tem menos overhead; o ganho de um workflow tende a aparecer em tarefas maiores. Modelos baratos (haiku, luna). Até a ADR 0044 (rodada de 2026-10-30) os agentes do Atlas no Claude não tinham Bash; desde ela (2026-10-31), os passos que editam rodam só os testes do projeto no sandbox da CLI, e os que julgam continuam só lendo. O Claude puro em -p não pode rodar Bash sem permissão; o Codex roda comandos no sandbox nos dois braços."),
    ("Nota sobre os arquivos", "Na rodada de 2026-10-30, depois da avaliação, um 'prettier --write' reformatou por engano as árvores salvas. As dos braços puros foram restauradas byte a byte (com verificação); nas do Atlas, os arquivos que o agente não tocou foram restaurados e os que ele escreveu ficaram só reformatados (espaçamento). Uma reavaliação deu nota oculta, escopo e testes idênticos nas 32 execuções; os números desta planilha são os de antes do acidente. bench/results agora fica fora do prettier."),
]
for i, (k, v) in enumerate(lines):
    r = 4 + i
    md.cell(row=r, column=1, value=k).font = Font(name=FONT, bold=True)
    c = md.cell(row=r, column=2, value=v)
    c.font = Font(name=FONT)
    c.alignment = Alignment(wrap_text=True, vertical="top")
    md.row_dimensions[r].height = max(30, 15 * (len(v) // 110 + 1))
md.column_dimensions["A"].width = 18
md.column_dimensions["B"].width = 120

# ---------------------------------------------------------------- Antes × Depois (Atlas arms)
if baseline is not None:
    before_rows = json.loads((baseline / "all.json").read_text())
    before_rubric = json.loads((baseline / "rubric.json").read_text()) if (baseline / "rubric.json").exists() else {}
    eb = wb.create_sheet("Execuções antes")
    title(eb, f"Execuções da rodada anterior ({baseline.name})", "Base da aba Antes × Depois. Mesmas colunas da aba Execuções, só as que a comparação usa.")
    bcols = [("ID", 34), ("Braço", 15), ("Tarefa", 30), ("Concluiu (1/0)", 9), ("Nota testes ocultos", 10), ("Tokens entrada", 12), ("Tokens cache", 12), ("Tokens saída", 11), ("Custo US$", 11), ("Tempo (s)", 9), ("Agentes rodados", 9), ("Fora do escopo", 9), ("Nota rubrica", 9)]
    for i, (h, _) in enumerate(bcols, start=1):
        eb.cell(row=4, column=i, value=h)
    style_header(eb, 4, len(bcols))
    widths(eb, [w for _, w in bcols])
    before_rows.sort(key=lambda r: (r["task"], order.get(r["arm"], 9), r["rep"]))
    for n, run in enumerate(before_rows):
        g = run["grade"]
        vals = [run["id"], ARM_LABEL[run["arm"]], TASK_LABEL.get(run["task"], run["task"]), 1 if run["ok"] else 0, round(g.get("hiddenScore", 0), 4), run.get("inputTokens"), run.get("cachedInputTokens"), run.get("outputTokens"), run.get("costUsd"), round(run["elapsedMs"] / 1000, 1), run.get("agentRuns"), len(g["outOfScope"]), before_rubric.get(run["id"], {}).get("score")]
        for c, v in enumerate(vals, start=1):
            eb.cell(row=5 + n, column=c, value=v)
    blast = 4 + len(before_rows)
    body(eb, 5, blast, len(bcols))
    for r in range(5, blast + 1):
        eb.cell(row=r, column=5).number_format = "0%"
        for c in (6, 7, 8):
            eb.cell(row=r, column=c).number_format = "#,##0"
        eb.cell(row=r, column=9).number_format = "$0.0000"
    eb.freeze_panes = "B5"

    ad = wb.create_sheet("Antes × Depois", 1)
    title(ad, "Efeito das melhorias no Atlas (antes × depois)", f"Antes: {baseline.name}. Depois: {results.name}. Compara a média de cada braço do Atlas nas duas rodadas; confira na Metodologia de cada uma se as tarefas e modelos foram os mesmos.")
    def b(col):
        return f"'Execuções antes'!${col}$5:${col}${blast}"
    metrics = [
        ("Concluiu", "G", "D", "0%"),
        ("Nota testes ocultos", "H", "E", "0%"),
        ("Tokens entrada (média)", "I", "F", "#,##0"),
        ("Tokens saída (média)", "L", "H", "#,##0"),
        ("Tempo médio (s)", "N", "J", "0"),
        ("Custo médio US$", "M", "I", "$0.0000"),
        ("Agentes por tarefa", "O", "K", "0.0"),
        ("Nota rubrica (média)", "V", "M", "0.0"),
    ]
    row = 4
    for arm in ("atlas-claude", "atlas-codex"):
        label = ARM_LABEL[arm]
        ad.cell(row=row, column=1, value=label).font = Font(name=FONT, bold=True, size=12, color="1F3A5F")
        row += 1
        for i, h in enumerate(["Métrica", "Antes", "Depois", "Diferença", "Puro (referência)"], start=1):
            ad.cell(row=row, column=i, value=h)
        style_header(ad, row, 5)
        pure = ARM_LABEL[arm.replace("atlas-", "") + "-pure"]
        for name, now_col, before_col, fmt in metrics:
            row += 1
            ad.cell(row=row, column=1, value=name)
            ad.cell(row=row, column=2, value=f'=IFERROR(AVERAGEIFS({b(before_col)},{b("B")},"{label}"),"n/d")').number_format = fmt
            ad.cell(row=row, column=3, value=f'=IFERROR(AVERAGEIFS({rng(now_col)},{R},"{label}"),"n/d")').number_format = fmt
            ad.cell(row=row, column=4, value=f'=IFERROR(C{row}-B{row},"")').number_format = ("+0%;-0%;0%" if fmt == "0%" else "+#,##0.0;-#,##0.0;0")
            ad.cell(row=row, column=5, value=f'=IFERROR(AVERAGEIFS({rng(now_col)},{R},"{pure}"),"n/d")').number_format = fmt
        body(ad, row - len(metrics) + 1, row, 5)
        row += 1
        ad.cell(row=row, column=1, value="Execuções fora do escopo")
        ad.cell(row=row, column=2, value=f'=COUNTIFS({b("B")},"{label}",{b("L")},">0")')
        ad.cell(row=row, column=3, value=f'=COUNTIFS({R},"{label}",{rng("Q")},">0")')
        ad.cell(row=row, column=4, value=f"=C{row}-B{row}")
        ad.cell(row=row, column=5, value=f'=COUNTIFS({R},"{pure}",{rng("Q")},">0")')
        body(ad, row, row, 5)
        row += 3
    widths(ad, [30, 14, 14, 14, 16])

# ---------------------------------------------------------------- Apresentação (first sheet)
ap = wb.create_sheet("Apresentação", 0)
title(ap, "Atlas × " + ("Claude e Codex puros" if ran_codex else "Claude puro") + ": o que os números mostram", f"{len(rows)} execuções · {config['models']['claude']}" + (f" · {config['models']['codex']}" if ran_codex else "") + f" · {config['startedAt'][:10]} · {atlas_desc} · cada número é uma fórmula sobre a aba Execuções.")
ah = ["Braço", "Execuções", "Concluiu", "Testes ocultos", "Tempo médio (s)", "Custo médio US$", "Tokens saída", "Fora do escopo", "Rubrica (0–10)"]
for i, h in enumerate(ah, start=1):
    ap.cell(row=4, column=i, value=h)
style_header(ap, 4, len(ah))
present = present_arms
for i, arm in enumerate(present):
    r = 5 + i
    a = f'"{ARM_LABEL[arm]}"'
    ap.cell(row=r, column=1, value=ARM_LABEL[arm]).font = Font(name=FONT, bold=True)
    for c, f, fmt in [
        (2, f"=COUNTIF({R},{a})", "0"),
        (3, f"=IFERROR(AVERAGEIFS({rng('G')},{R},{a}),\"\")", "0%"),
        (4, f"=IFERROR(AVERAGEIFS({rng('H')},{R},{a}),\"\")", "0%"),
        (5, f"=IFERROR(AVERAGEIFS({rng('N')},{R},{a}),\"\")", "0"),
        (6, f"=IFERROR(AVERAGEIFS({rng('M')},{R},{a}),\"n/d\")", "$0.0000"),
        (7, f"=IFERROR(AVERAGEIFS({rng('L')},{R},{a}),\"\")", "#,##0"),
        (8, f"=COUNTIFS({R},{a},{rng('Q')},\">0\")", "0"),
        (9, f"=IFERROR(AVERAGEIFS({rng('V')},{R},{a}),\"\")", "0.0"),
    ]:
        ap.cell(row=r, column=c, value=f).number_format = fmt
    if arm.startswith("atlas"):
        for c in range(1, len(ah) + 1):
            ap.cell(row=r, column=c).fill = ATLAS
body(ap, 5, 4 + len(present), len(ah))
row = 6 + len(present)


def paragraphs(heading, texts):
    global row
    ap.cell(row=row, column=1, value=heading).font = Font(name=FONT, bold=True, size=12, color="1F3A5F")
    for n, text in enumerate(texts, start=1):
        row += 1
        c = ap.cell(row=row, column=1, value=f"{n}. {text}")
        c.font = Font(name=FONT, size=10)
        c.alignment = Alignment(wrap_text=True, vertical="top")
        ap.merge_cells(start_row=row, start_column=1, end_row=row, end_column=len(ah))
        ap.row_dimensions[row].height = max(30, 14 * (len(text) // 120 + 1))
    row += 2


# Computed every time, so a new round is readable before anyone writes findings.json.
paragraphs("Números desta rodada (calculados)", measured(rows))
if findings:
    paragraphs("Conclusões (análise escrita, findings.json)", [f["finding"] for f in ([f for f in findings if f.get("headline")] or findings[:5])])
widths(ap, [26, 10, 10, 12, 12, 13, 12, 12, 12])

out = results / "benchmark-atlas.xlsx"
# openpyxl writes no cached values: ask the spreadsheet app to compute every formula on open.
from openpyxl.workbook.properties import CalcProperties

wb.calculation = CalcProperties(fullCalcOnLoad=True)
wb.save(out)
print(out)
