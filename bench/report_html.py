#!/usr/bin/env python3
"""Builds the presentable benchmark report (one self-contained HTML page) from a graded round.

    python3 bench/report_html.py bench/results/2026-11-04 [--baseline bench/results/2026-11-01]

Reads all.json (grade.mjs), config.json (run.mjs), findings.json (written analysis, optional),
bench/history.json (what each earlier round changed) and each task's meta.json. Every number on the
page is computed here from those files; only findings.json and history.json are written by hand.
Writes <round>/relatorio.html next to the spreadsheet.
"""
import html
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
results = Path(sys.argv[1]).resolve()
baseline = Path(sys.argv[sys.argv.index("--baseline") + 1]).resolve() if "--baseline" in sys.argv else None


def read(path, default=None):
    return json.loads(path.read_text()) if path.exists() else default


rows = read(results / "all.json")
config = read(results / "config.json", {})
findings = read(results / "findings.json", [])
history = read(HERE / "history.json", {"rounds": []})["rounds"]
before = read(baseline / "all.json", []) if baseline else []

ARMS = ["claude-pure", "atlas-claude", "atlas-claude-quick", "codex-pure", "atlas-codex"]
ARM = {
    "claude-pure": "Claude puro",
    "atlas-claude": "Atlas + Claude",
    "atlas-claude-quick": "Atlas Quick",
    "codex-pure": "Codex puro",
    "atlas-codex": "Atlas + Codex",
}
# Series slot per arm: fixed by entity, so a round without an arm never repaints the others.
SLOT = {"claude-pure": 2, "atlas-claude": 1, "atlas-claude-quick": 3, "codex-pure": 4, "atlas-codex": 5}
TASK = {
    "t1-slugify": ("T1", "Feature nova: slugify"),
    "t2-cart-bug": ("T2", "Bug por sintoma: carrinho"),
    "t3-todo-priority": ("T3", "Feature em 2 arquivos: prioridades"),
    "t4-review": ("T4", "Revisão: 3 defeitos plantados"),
    "t5-rate-limiter": ("T5", "Limitador de taxa"),
    "t6-csv-bug": ("T6", "Bug: CSV com 5 defeitos"),
    "t7-order-cancel": ("T7", "Cancelar pedido em 3 arquivos"),
    "t8-oficina-recorrente": ("T8", "Bug real: doação recorrente (Oficina)"),
}
ROLE = {
    "architect": "Arquiteto",
    "developer": "Desenvolvedor",
    "validator": "Validador",
    "qa": "QA",
    "fixer": "Corretor de bug",
    "reviewer": "Revisor",
    # Step ids of the bug_fix and Quick templates (the run lists steps by id, the usage ledger by role).
    "bug-fixer": "Corretor",
    "bug-fixer-retry": "Corretor (nova volta)",
    "bug-fixer-qa": "Corretor (pedido do QA)",
    "bug-fixer-architecture": "Corretor (arquitetura)",
    "fix": "Correção",
}
present = [a for a in ARMS if any(r["arm"] == a for r in rows)]
tasks = sorted({r["task"] for r in rows})
meta = {t: read(HERE / "tasks" / t / "meta.json", {}) for t in tasks}


# ---------------------------------------------------------------- numbers
def mean(xs):
    xs = [x for x in xs if x is not None]
    return sum(xs) / len(xs) if xs else None


def of(arm, data=rows, only=None):
    return [r for r in data if r["arm"] == arm and (only is None or r["task"] in only)]


def hidden(r):
    return r["grade"].get("hiddenScore", 0)


def own_fail(r):
    return bool((r["grade"].get("ownTests") or {}).get("fail"))


def silent(r):
    """Shipped code the hidden tests reject while its own tests were green: an error nobody saw."""
    g = r["grade"]
    return hidden(r) < 1 and g.get("kind") != "review" and g.get("ownTestFiles") and not own_fail(r)


def stats(runs):
    return {
        "n": len(runs),
        "ok": sum(1 for r in runs if r["ok"]),
        "hidden": mean([hidden(r) for r in runs]),
        "perfect": sum(1 for r in runs if hidden(r) >= 1),
        "secs": mean([r["elapsedMs"] / 1000 for r in runs]),
        "cost": mean([r.get("costUsd") for r in runs]),
        "input": mean([r.get("inputTokens") for r in runs]),
        "output": mean([r.get("outputTokens") for r in runs]),
        "agents": mean([r.get("agentRuns") for r in runs]),
        "own_fail": sum(1 for r in runs if own_fail(r)),
        "silent": sum(1 for r in runs if silent(r)),
        "scope": sum(1 for r in runs if r["grade"]["outOfScope"]),
    }


S = {a: stats(of(a)) for a in present}


def compare(atlas, pure):
    """Atlas ÷ pure on the tasks both ran (the Quick arm runs the feature tasks only)."""
    common = {r["task"] for r in of(atlas)} & {r["task"] for r in of(pure)}
    if not common:
        return None
    a, p = stats(of(atlas, only=common)), stats(of(pure, only=common))
    ratio = lambda k: a[k] / p[k] if a[k] is not None and p[k] else None
    return {"common": common, "a": a, "p": p, "cost": ratio("cost"), "secs": ratio("secs"), "input": ratio("input"),
            "hidden": a["hidden"] - p["hidden"]}


def change(arm):
    """This round against the baseline, for one arm, on the tasks both rounds ran."""
    if not before:
        return None
    common = {r["task"] for r in of(arm)} & {r["task"] for r in of(arm, before)}
    if not common:
        return None
    a, b = stats(of(arm, only=common)), stats(of(arm, before, only=common))
    pct = lambda k: (a[k] / b[k] - 1) if a[k] is not None and b[k] else None
    return {"common": common, "now": a, "before": b, "secs": pct("secs"), "cost": pct("cost"), "input": pct("input")}


# ---------------------------------------------------------------- formatting (pt-BR)
def num(x, d=0):
    if x is None:
        return "n/d"
    s = f"{x:,.{d}f}"
    return s.replace(",", " ").replace(".", ",")


def pct(x, d=0):
    return "n/d" if x is None else num(x * 100, d) + "%"


def usd(x, d=4):
    return "n/d" if x is None else "US$ " + num(x, d)


def times(x):
    return "n/d" if x is None else num(x, 2) + "×"


def signed_pct(x):
    return "n/d" if x is None else ("+" if x > 0 else "−" if x < 0 else "") + num(abs(x) * 100, 0) + "%"


def kilo(x):
    if x is None:
        return "n/d"
    return num(x / 1_000_000, 1) + " mi" if x >= 1_000_000 else num(x / 1000, 0) + " mil"


def e(s):
    return html.escape(str(s))


def task_name(t, long=False):
    code, name = TASK.get(t, (t, t))
    return f"{code} · {name}" if long else code


# ---------------------------------------------------------------- verdict
main = compare("atlas-claude", "claude-pure") if {"atlas-claude", "claude-pure"} <= set(present) else None
quick = compare("atlas-claude-quick", "claude-pure") if {"atlas-claude-quick", "claude-pure"} <= set(present) else None
codex = compare("atlas-codex", "codex-pure") if {"atlas-codex", "codex-pure"} <= set(present) else None
evo = change("atlas-claude")

verdicts = []  # (state, label, title, body)
for label, c, atlas_arm, pure_arm in (("Claude", main, "atlas-claude", "claude-pure"), ("Codex", codex, "atlas-codex", "codex-pure")):
    if not c:
        continue
    a, p = c["a"], c["p"]
    d = c["hidden"]
    state = "good" if d > 0.004 else "bad" if d < -0.004 else "even"
    tag = {"good": "Atlas melhor", "bad": f"{label} puro melhor", "even": "Empate"}[state]
    verdicts.append((state, tag, "Qualidade do código",
                     f"{ARM[atlas_arm]}: {a['perfect']} de {a['n']} execuções passaram todos os testes ocultos "
                     f"(média {pct(a['hidden'])}). {ARM[pure_arm]}: {p['perfect']} de {p['n']} (média {pct(p['hidden'])})."))
    sa, sp = a["silent"] + a["own_fail"], p["silent"] + p["own_fail"]
    state = "good" if sa < sp else "bad" if sa > sp else "even"
    tag = {"good": "Atlas melhor", "bad": f"{label} puro melhor", "even": "Empate"}[state]
    verdicts.append((state, tag, "Erros entregues sem aviso",
                     f"Código errado com os próprios testes verdes, ou teste próprio quebrado: {ARM[pure_arm]} em {sp} de {p['n']}, "
                     f"{ARM[atlas_arm]} em {sa} de {a['n']}. É o que o passo de QA do Atlas existe para pegar."))
    if c["cost"]:
        verdicts.append(("cost", "Custa mais" if c["cost"] > 1.05 else "Custa igual", "Custo por tarefa",
                         f"{ARM[atlas_arm]} {usd(a['cost'])} contra {usd(p['cost'])}: {times(c['cost'])} o custo do {label} puro "
                         f"(preço de API informado pela CLI)."))
    verdicts.append(("cost", "Mais lento" if c["secs"] > 1.05 else "Mesmo tempo", "Tempo por tarefa",
                     f"{num(a['secs'])} s contra {num(p['secs'])} s: {times(c['secs'])}. Cada tarefa passa por vários agentes "
                     f"({num(a['agents'], 1)} em média), um depois do outro."))
if evo:
    good = (evo["secs"] or 0) < 0 and (evo["cost"] or 0) <= 0.02
    verdicts.append(("good" if good else "even", "Melhorou" if good else "Estável", f"Atlas contra a rodada {baseline.name}",
                     f"Nas {len(evo['common'])} tarefas que as duas rodadas têm: tempo {signed_pct(evo['secs'])}, custo {signed_pct(evo['cost'])}, "
                     f"tokens de entrada {signed_pct(evo['input'])}; concluiu {evo['now']['ok']} de {evo['now']['n']} (antes {evo['before']['ok']} de {evo['before']['n']})."))


def headline():
    if not main:
        return "Resultados desta rodada do benchmark."
    a, p = main["a"], main["p"]
    errors_a, errors_p = a["silent"] + a["own_fail"], p["silent"] + p["own_fail"]
    if a["perfect"] == a["n"] and p["perfect"] < p["n"]:
        s = (f"O Atlas passou em todos os testes ocultos nas {a['n']} execuções; o Claude puro errou "
             f"{p['n'] - p['perfect']} de {p['n']}")
        s += " e entregou o erro sem perceber" if errors_p and not errors_a else ""
    elif abs(main["hidden"]) <= 0.004:
        s = "O Atlas e o Claude puro entregaram código igualmente correto"
        if errors_a < errors_p:
            s += f", mas só o Claude puro entregou erros sem perceber ({errors_p})"
    else:
        s = f"O Atlas ficou {'acima' if main['hidden'] > 0 else 'abaixo'} do Claude puro nos testes ocultos ({signed_pct(main['hidden'])})"
    if main["cost"]:
        s += f". Em troca, custa {times(main['cost'])} e leva {times(main['secs'])} o tempo"
    return s + "."


# ---------------------------------------------------------------- charts (inline SVG)
def bar_chart(metric, fmt, title_unit):
    """Grouped horizontal bars: one group per task, one bar per arm, one shared scale."""
    groups = []
    for t in tasks:
        bars = []
        for a in present:
            v = mean([metric(r) for r in of(a, only={t})])
            if v is not None:
                bars.append((a, v))
        groups.append((t, bars))
    vmax = max((v for _, bs in groups for _, v in bs), default=1) or 1
    step = nice_step(vmax)
    top = step * -(-vmax // step)
    left, right, width = 168, 64, 760
    plot = width - left - right
    bar_h, gap, group_gap = 11, 2, 16
    y = 26
    parts = []
    # gridlines + tick labels
    ticks = []
    k = 0
    while k * step <= top + 1e-9:
        ticks.append(k * step)
        k += 1
    body_top = y
    rows_svg = []
    for t, bars in groups:
        gh = len(bars) * (bar_h + gap) - gap
        code, name = TASK.get(t, (t, t))
        rows_svg.append(f'<text class="t-label" x="{left - 12}" y="{y + gh / 2 - 3}" text-anchor="end">{e(code)}</text>'
                        f'<text class="t-sub" x="{left - 12}" y="{y + gh / 2 + 10}" text-anchor="end">{e(short(name))}</text>')
        for a, v in bars:
            w = max(2, plot * v / top)
            tip = f"{ARM[a]} · {code}: {fmt(v)}"
            rows_svg.append(
                f'<g class="mark" data-tip="{e(tip)}"><rect class="hit" x="{left}" y="{y - 1}" width="{plot}" height="{bar_h + 2}"/>'
                f'<path class="bar s{SLOT[a]}" d="{rounded_bar(left, y, w, bar_h)}"/>'
                f'<text class="v-label" x="{left + w + 6}" y="{y + bar_h - 2}">{e(fmt(v))}</text></g>')
            y += bar_h + gap
        y += group_gap - gap
    height = y + 4
    for tv in ticks:
        x = left + plot * tv / top
        parts.append(f'<line class="grid" x1="{x}" x2="{x}" y1="{body_top - 8}" y2="{height - 8}"/>'
                     f'<text class="axis" x="{x}" y="{body_top - 12}" text-anchor="middle">{e(fmt(tv, axis=True))}</text>')
    return (f'<svg viewBox="0 0 {width} {height}" role="img" aria-label="{e(title_unit)} por tarefa e braço">'
            + "".join(parts) + "".join(rows_svg) + "</svg>")


def short(name):
    return name if len(name) <= 26 else name[:25].rstrip() + "…"


def nice_step(vmax):
    import math
    raw = vmax / 4
    mag = 10 ** math.floor(math.log10(raw))
    for m in (1, 2, 2.5, 5, 10):
        if raw <= m * mag:
            return m * mag
    return 10 * mag


def rounded_bar(x, y, w, h, r=4):
    """Square at the baseline, 4px round at the data end."""
    r = min(r, w / 2, h / 2)
    return (f"M{x},{y} H{x + w - r} Q{x + w},{y} {x + w},{y + r} V{y + h - r} "
            f"Q{x + w},{y + h} {x + w - r},{y + h} H{x} Z")


def f_secs(v, axis=False):
    return f"{num(v)} s"


def f_cost(v, axis=False):
    return "US$ " + num(v, 2 if axis else 3)


def f_tokens(v, axis=False):
    return kilo(v)


charts = [
    ("tempo", "Tempo", bar_chart(lambda r: r["elapsedMs"] / 1000, f_secs, "Tempo")),
    ("custo", "Custo", bar_chart(lambda r: r.get("costUsd"), f_cost, "Custo")),
    ("entrada", "Tokens de entrada", bar_chart(lambda r: r.get("inputTokens"), f_tokens, "Tokens de entrada")),
]


# ---------------------------------------------------------------- sections
def legend():
    return "".join(f'<span class="key"><i class="sw s{SLOT[a]}"></i>{e(ARM[a])}</span>' for a in present)


def summary_table():
    head = ("<tr><th scope='col'>Braço</th><th>Execuções</th><th>Concluiu</th><th>Testes ocultos</th><th>Erros sem aviso</th>"
            "<th>Tempo médio</th><th>Custo médio</th><th>Entrada</th><th>Saída</th><th>Agentes</th><th>Fora do escopo</th></tr>")
    body = []
    for a in present:
        s = S[a]
        errs = s["silent"] + s["own_fail"]
        body.append(
            f"<tr class='{'atlas' if a.startswith('atlas') else ''}'><th scope='row'><i class='sw s{SLOT[a]}'></i>{e(ARM[a])}</th>"
            f"<td>{s['n']}</td><td>{s['ok']}/{s['n']}</td>"
            f"<td><span class='pill {'good' if s['perfect'] == s['n'] else 'warn'}'>{pct(s['hidden'])}</span> <small>{s['perfect']}/{s['n']} com 100%</small></td>"
            f"<td><span class='pill {'good' if errs == 0 else 'bad'}'>{errs}</span></td>"
            f"<td>{num(s['secs'])} s</td><td>{usd(s['cost'])}</td><td>{kilo(s['input'])}</td><td>{kilo(s['output'])}</td>"
            f"<td>{num(s['agents'], 1)}</td><td>{s['scope']}</td></tr>")
    return f"<div class='scroll'><table class='data'><thead>{head}</thead><tbody>{''.join(body)}</tbody></table></div>"


def matrix():
    head = "<tr><th scope='col'>Tarefa</th>" + "".join(f"<th><i class='sw s{SLOT[a]}'></i>{e(ARM[a])}</th>" for a in present) + "</tr>"
    body = []
    for t in tasks:
        cells = []
        for a in present:
            runs = of(a, only={t})
            if not runs:
                cells.append("<td class='na'>não roda</td>")
                continue
            v = mean([hidden(r) for r in runs])
            cls = "good" if v >= 1 else "warn" if v >= 0.8 else "bad"
            secs = mean([r["elapsedMs"] / 1000 for r in runs])
            cells.append(f"<td><span class='pill {cls}'>{pct(v)}</span> <small>{num(secs)} s</small></td>")
        body.append(f"<tr><th scope='row'>{e(task_name(t, True))}</th>{''.join(cells)}</tr>")
    return f"<div class='scroll'><table class='data matrix'><thead>{head}</thead><tbody>{''.join(body)}</tbody></table></div>"


def misses():
    """Every run the hidden tests rejected, with what exactly failed, next to how Atlas did there."""
    bad = [r for r in rows if hidden(r) < 1 and r["grade"].get("kind") != "review"]
    if not bad:
        return "<p class='note'>Nenhuma execução falhou em teste oculto nesta rodada.</p>"
    out = []
    for t in sorted({r["task"] for r in bad}):
        labels = meta.get(t, {}).get("hiddenLabels", {})
        items = []
        for r in [r for r in bad if r["task"] == t]:
            g = r["grade"]
            own = g.get("ownTests") or {}
            failed = [labels.get(n, n) for n in (g.get("hidden") or {}).get("failed", [])]
            total = (g.get("hidden") or {}).get("pass", 0) + (g.get("hidden") or {}).get("fail", 0)
            items.append(
                f"<li><b>{e(ARM[r['arm']])}, rodada {r['rep']}</b>: {pct(hidden(r))} dos {total} testes ocultos. "
                f"Os testes que o próprio agente escreveu: {own.get('pass', 0)} passam, {own.get('fail', 0)} falham"
                f"{'; ele entregou achando que estava certo' if silent(r) else ''}."
                + ("<ul class='fails'>" + "".join(f"<li>falhou: {e(x)}</li>" for x in failed) + "</ul>" if failed else "") + "</li>")
        others = []
        for a in present:
            if not a.startswith("atlas"):
                continue
            runs = of(a, only={t})
            if runs:
                done = sum(1 for r in runs if r["ok"])
                steps = "; ".join(" → ".join(ROLE.get(s, s) for s in r.get("steps", [])) for r in runs)
                others.append(f"<li><b>{e(ARM[a])}</b>: {sum(1 for r in runs if hidden(r) >= 1)} de {len(runs)} com 100%, "
                              f"concluiu {done} de {len(runs)}, {num(mean([r['elapsedMs'] / 1000 for r in runs]))} s em média. Passos: {e(steps)}.</li>")
        title = meta.get(t, {}).get("title") or task_name(t, True)
        out.append(f"<article class='case'><h3>{e(title)}</h3><ul>{''.join(items)}</ul>"
                   + (f"<p class='sub'>No mesmo problema:</p><ul>{''.join(others)}</ul>" if others else "") + "</article>")
    return "".join(out)


def roles():
    steps = [s for r in of("atlas-claude") for s in (r.get("perStep") or [])] if "atlas-claude" in present else []
    if not steps:
        return ""
    total = sum(s.get("cost") or 0 for s in steps) or 1
    names = sorted({s["role"] for s in steps}, key=lambda n: -sum(s.get("cost") or 0 for s in steps if s["role"] == n))
    body = []
    for n in names:
        ss = [s for s in steps if s["role"] == n]
        share = sum(s.get("cost") or 0 for s in ss) / total
        body.append(f"<tr><th scope='row'>{e(ROLE.get(n, n))}</th><td>{len(ss)}</td><td>{num(mean([s['ms'] / 1000 for s in ss]))} s</td>"
                    f"<td>{kilo(mean([s.get('inputTokens') for s in ss]))}</td><td>{kilo(mean([s.get('outputTokens') for s in ss]))}</td>"
                    f"<td>{usd(mean([s.get('cost') for s in ss]))}</td>"
                    f"<td class='share'><span class='meter'><span style='width:{share * 100:.1f}%'></span></span>{pct(share)}</td></tr>")
    head = "<tr><th scope='col'>Passo</th><th>Vezes</th><th>Tempo médio</th><th>Entrada média</th><th>Saída média</th><th>Custo médio</th><th>Parte do custo</th></tr>"
    return f"<div class='scroll'><table class='data'><thead>{head}</thead><tbody>{''.join(body)}</tbody></table></div>"


def evolution():
    """history.json, each round with Atlas's numbers on T1–T4 (the tasks every round has)."""
    base = {"t1-slugify", "t2-cart-bug", "t3-todo-priority", "t4-review"}
    body = []
    for h in history:
        data = read(HERE / "results" / h["round"] / "all.json", [])
        cells = []
        for a in ("atlas-claude", "atlas-codex"):
            runs = of(a, data, only=base)
            if runs:
                s = stats(runs)
                cells.append(f"<td>{s['ok']}/{s['n']} · {pct(s['hidden'])}<br><small>{num(s['secs'])} s · {usd(s['cost'], 3) if s['cost'] else 'custo n/d'} · fora do escopo {s['scope']}</small></td>")
            else:
                cells.append("<td class='na'>não rodou</td>")
        cur = " class='current'" if h["round"] == results.name else ""
        body.append(f"<tr{cur}><th scope='row'>{e(h['round'])}</th><td>{e(h['change'])}</td><td>{e(h['effect'])}</td>{''.join(cells)}</tr>")
    head = ("<tr><th scope='col'>Rodada</th><th>O que mudou no Atlas</th><th>Efeito medido</th>"
            "<th>Atlas + Claude em T1–T4<br><small>concluiu · ocultos</small></th><th>Atlas + Codex em T1–T4<br><small>concluiu · ocultos</small></th></tr>")
    return f"<div class='scroll'><table class='data history'><thead>{head}</thead><tbody>{''.join(body)}</tbody></table></div>"


def runs_table():
    order = {a: i for i, a in enumerate(ARMS)}
    body = []
    for r in sorted(rows, key=lambda r: (r["task"], order.get(r["arm"], 9), r["rep"])):
        g = r["grade"]
        note = []
        if not r["ok"]:
            note.append(str(r.get("failure") or r.get("status") or "não concluiu")[:90])
        if silent(r):
            note.append("errado com testes próprios verdes")
        if own_fail(r):
            note.append("teste próprio falhando")
        if g["outOfScope"]:
            note.append("fora do escopo: " + ", ".join(g["outOfScope"])[:80])
        if r["arm"].startswith("atlas"):
            note.append(" → ".join(ROLE.get(s, s) for s in r.get("steps", [])))
        body.append(
            f"<tr><td><i class='sw s{SLOT[r['arm']]}'></i>{e(ARM[r['arm']])}</td><td>{e(task_name(r['task']))}</td><td>{r['rep']}</td>"
            f"<td>{'sim' if r['ok'] else '<b>não</b>'}</td><td><span class='pill {'good' if hidden(r) >= 1 else 'bad'}'>{pct(hidden(r))}</span></td>"
            f"<td>{num(r['elapsedMs'] / 1000)} s</td><td>{usd(r.get('costUsd'))}</td><td>{kilo(r.get('inputTokens'))}</td>"
            f"<td>{kilo(r.get('outputTokens'))}</td><td>{r.get('agentRuns') or ''}</td><td class='note-cell'>{e(' · '.join(note))}</td></tr>")
    head = ("<tr><th scope='col'>Braço</th><th>Tarefa</th><th>Rodada</th><th>Concluiu</th><th>Ocultos</th><th>Tempo</th>"
            "<th>Custo</th><th>Entrada</th><th>Saída</th><th>Agentes</th><th>Observação</th></tr>")
    return f"<div class='scroll'><table class='data runs'><thead>{head}</thead><tbody>{''.join(body)}</tbody></table></div>"


def task_list():
    items = []
    for t in tasks:
        m = meta.get(t, {})
        tmpl = {"software_feature": "Implement", "bug_fix": "Debug & fix", "code_review": "Review & audit"}.get(m.get("template"), m.get("template"))
        runner = "JUnit/Mockito com Maven offline" if m.get("runner") == "maven" else "node --test"
        items.append(f"<li><b>{e(task_name(t, True))}</b>: template {e(tmpl)}; testes ocultos em {runner}.</li>")
    return "<ul class='tasks'>" + "".join(items) + "</ul>"


# ---------------------------------------------------------------- page
models = config.get("models", {})
versions = config.get("versions", {})
atlas_v = config.get("atlas") or {}
ran_codex = any(a.startswith("codex") or a.endswith("codex") for a in present)
atlas_label = (f"commit {atlas_v.get('commit')}" + (" com mudanças sem commit" if atlas_v.get("dirty") else "")) if atlas_v.get("commit") else "versão não registrada"
reps = config.get("reps", 1)
account_lost = sorted(p.parent.name for p in (results / "runs").glob("*/account-failure.json") if not (p.parent / "metrics.json").exists())

verdict_html = "".join(
    f"<article class='verdict {st}'><p class='tag'>{e(tag)}</p><h3>{e(title)}</h3><p>{e(body)}</p></article>"
    for st, tag, title, body in verdicts)
written = [f for f in findings if f.get("headline")] or findings[:5]
findings_html = "".join(
    f"<li><p>{e(f['finding'])}</p>" + (f"<p class='then'><b>O que fazer:</b> {e(f['fix'])}</p>" if f.get("fix") else "") + "</li>"
    for f in written)
tabs = "".join(
    f"<button type='button' class='tab' id='tab-{k}' role='tab' aria-selected='{str(i == 0).lower()}' aria-controls='chart-{k}' data-chart='{k}'>{e(n)}</button>"
    for i, (k, n, _) in enumerate(charts))
panels = "".join(f"<div class='chart' id='chart-{k}' role='tabpanel' aria-labelledby='tab-{k}'{'' if i == 0 else ' hidden'}>{svg}</div>"
                 for i, (k, _, svg) in enumerate(charts))

CSS = """
:root {
  /* Lab report: cool slate neutrals, one blue accent, numbers in mono; verdict first, evidence after. */
  --bg: #f6f7f9; --panel: #ffffff; --ink: #14171c; --ink-2: #4a515c; --ink-3: #6b7380; --line: #dde1e7;
  --accent: #1f5fb8;
  --good: #1a7f37; --good-bg: #e6f4ea; --warn: #9a6700; --warn-bg: #fff4d6; --bad: #c4282d; --bad-bg: #fdecec;
  --s1: #2a78d6; --s2: #eb6834; --s3: #1baf7a; --s4: #eda100; --s5: #e87ba4;
  --display: "IBM Plex Sans Condensed", "Arial Narrow", system-ui, sans-serif;
  --body: "IBM Plex Sans", system-ui, -apple-system, "Segoe UI", sans-serif;
  --mono: "IBM Plex Mono", ui-monospace, "SF Mono", Menlo, monospace;
  color-scheme: light;
}
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    --bg: #0f1216; --panel: #171b21; --ink: #eef1f5; --ink-2: #b4bcc8; --ink-3: #8a93a0; --line: #2a313b;
    --accent: #6ea8ff; --good: #4ac26b; --good-bg: #12301c; --warn: #e3b341; --warn-bg: #33280c; --bad: #ff7b72; --bad-bg: #3a1618;
    --s1: #3987e5; --s2: #d95926; --s3: #199e70; --s4: #c98500; --s5: #d55181; color-scheme: dark;
  }
}
:root[data-theme="dark"] {
  --bg: #0f1216; --panel: #171b21; --ink: #eef1f5; --ink-2: #b4bcc8; --ink-3: #8a93a0; --line: #2a313b;
  --accent: #6ea8ff; --good: #4ac26b; --good-bg: #12301c; --warn: #e3b341; --warn-bg: #33280c; --bad: #ff7b72; --bad-bg: #3a1618;
  --s1: #3987e5; --s2: #d95926; --s3: #199e70; --s4: #c98500; --s5: #d55181; color-scheme: dark;
}
* { box-sizing: border-box; }
body { margin: 0; background: var(--bg); color: var(--ink); font: 16px/1.55 var(--body); }
main { max-width: 1120px; margin: 0 auto; padding: 40px 24px 64px; display: grid; gap: 48px; }
h1, h2, h3 { font-family: var(--display); text-wrap: balance; margin: 0; }
h1 { font-size: clamp(30px, 5vw, 44px); line-height: 1.1; font-weight: 600; letter-spacing: -0.01em; }
h2 { font-size: 24px; font-weight: 600; }
h3 { font-size: 17px; font-weight: 600; }
p { margin: 0; max-width: 72ch; }
small, .sub { color: var(--ink-3); font-size: 13px; }
.eyebrow { font: 500 13px/1 var(--mono); color: var(--accent); letter-spacing: 0.04em; text-transform: uppercase; }
header { display: grid; gap: 14px; }
.lede { font-size: 20px; line-height: 1.45; color: var(--ink); max-width: 64ch; }
.meta { display: flex; flex-wrap: wrap; gap: 8px 18px; font: 13px var(--mono); color: var(--ink-3); }
section { display: grid; gap: 18px; }
section > header { gap: 6px; }
section > header p { color: var(--ink-2); }
.verdicts { display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 12px; }
.verdict { background: var(--panel); border: 1px solid var(--line); border-top: 4px solid var(--ink-3); border-radius: 8px; padding: 16px; display: grid; gap: 8px; align-content: start; }
.verdict p { color: var(--ink-2); font-size: 15px; }
.verdict .tag { font: 600 12px/1 var(--mono); text-transform: uppercase; letter-spacing: 0.05em; color: var(--ink-3); }
.verdict.good { border-top-color: var(--good); } .verdict.good .tag { color: var(--good); }
.verdict.bad { border-top-color: var(--bad); } .verdict.bad .tag { color: var(--bad); }
.verdict.cost { border-top-color: var(--warn); } .verdict.cost .tag { color: var(--warn); }
.scroll { overflow-x: auto; background: var(--panel); border: 1px solid var(--line); border-radius: 8px; }
table.data { border-collapse: collapse; width: 100%; font-size: 14px; font-variant-numeric: tabular-nums; }
table.data th, table.data td { padding: 9px 12px; border-bottom: 1px solid var(--line); text-align: left; vertical-align: top; }
table.data thead th { font: 600 12px/1.3 var(--body); color: var(--ink-3); text-transform: uppercase; letter-spacing: 0.03em; white-space: nowrap; background: var(--bg); }
table.data tbody tr:last-child > * { border-bottom: 0; }
table.data tbody th { font-weight: 600; white-space: nowrap; }
table.data tr.atlas th { color: var(--ink); }
table.history td:nth-child(2), table.history td:nth-child(3) { min-width: 260px; }
table.history tr.current { background: color-mix(in srgb, var(--accent) 7%, transparent); }
table.runs td { white-space: nowrap; } table.runs .note-cell { white-space: normal; min-width: 260px; color: var(--ink-2); }
td.na { color: var(--ink-3); font-style: italic; }
.sw { display: inline-block; width: 10px; height: 10px; border-radius: 2px; margin-right: 8px; vertical-align: -1px; }
.s1 { background: var(--s1); fill: var(--s1); } .s2 { background: var(--s2); fill: var(--s2); } .s3 { background: var(--s3); fill: var(--s3); }
.s4 { background: var(--s4); fill: var(--s4); } .s5 { background: var(--s5); fill: var(--s5); }
.pill { display: inline-block; min-width: 3.2em; text-align: center; padding: 1px 8px; border-radius: 999px; font: 600 13px/1.6 var(--mono); }
.pill.good { background: var(--good-bg); color: var(--good); } .pill.warn { background: var(--warn-bg); color: var(--warn); } .pill.bad { background: var(--bad-bg); color: var(--bad); }
.meter { display: inline-block; width: 90px; height: 8px; background: var(--line); border-radius: 4px; margin-right: 8px; vertical-align: 0; overflow: hidden; }
.meter span { display: block; height: 100%; background: var(--s1); border-radius: 4px; }
.chart-card { background: var(--panel); border: 1px solid var(--line); border-radius: 8px; padding: 16px 16px 8px; display: grid; gap: 12px; }
.chart-top { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 12px; align-items: center; }
.tabs { display: flex; gap: 4px; background: var(--bg); padding: 3px; border-radius: 8px; border: 1px solid var(--line); }
.tab { font: 500 14px var(--body); color: var(--ink-2); background: transparent; border: 0; padding: 6px 12px; border-radius: 6px; cursor: pointer; }
.tab[aria-selected="true"] { background: var(--panel); color: var(--ink); box-shadow: 0 1px 2px rgb(0 0 0 / 0.12); }
.tab:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.legend { display: flex; flex-wrap: wrap; gap: 6px 16px; font-size: 14px; color: var(--ink-2); }
.chart { overflow-x: auto; } .chart svg { width: 100%; min-width: 620px; height: auto; display: block; }
svg text { font-family: var(--body); }
svg .t-label { font: 600 13px var(--mono); fill: var(--ink); } svg .t-sub { font-size: 11px; fill: var(--ink-3); }
svg .v-label { font: 11px var(--mono); fill: var(--ink-2); } svg .axis { font: 11px var(--mono); fill: var(--ink-3); }
svg .grid { stroke: var(--line); stroke-width: 1; }
svg .hit { fill: transparent; } svg .mark:hover .bar { opacity: 0.8; }
.tip { position: fixed; pointer-events: none; background: var(--ink); color: var(--bg); font: 13px var(--mono); padding: 6px 9px; border-radius: 6px; z-index: 10; }
.cases { display: grid; gap: 12px; }
.case { background: var(--panel); border: 1px solid var(--line); border-left: 4px solid var(--bad); border-radius: 8px; padding: 16px 18px; display: grid; gap: 10px; }
.case ul { margin: 0; padding-left: 20px; display: grid; gap: 8px; } .case .fails { margin-top: 6px; gap: 2px; color: var(--bad); }
.findings { margin: 0; padding-left: 22px; display: grid; gap: 14px; }
.findings .then { color: var(--ink-2); margin-top: 4px; }
.method { display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 12px; }
.method article { background: var(--panel); border: 1px solid var(--line); border-radius: 8px; padding: 16px; display: grid; gap: 8px; align-content: start; }
.method p, .method li { color: var(--ink-2); font-size: 15px; }
.method ul { margin: 0; padding-left: 18px; display: grid; gap: 4px; }
code, pre { font-family: var(--mono); font-size: 13px; }
pre { margin: 0; background: var(--bg); border: 1px solid var(--line); border-radius: 6px; padding: 10px 12px; overflow-x: auto; color: var(--ink); }
.note { color: var(--ink-2); }
footer { color: var(--ink-3); font-size: 13px; border-top: 1px solid var(--line); padding-top: 16px; }
@media (max-width: 600px) { main { padding: 28px 16px 48px; gap: 36px; } .lede { font-size: 18px; } }
@media (prefers-reduced-motion: no-preference) { .tab { transition: background 0.15s; } }
"""

JS = """
const tip = document.createElement('div'); tip.className = 'tip'; tip.hidden = true; document.body.append(tip);
document.addEventListener('pointermove', (ev) => {
  const m = ev.target.closest('[data-tip]');
  if (!m) { tip.hidden = true; return; }
  tip.textContent = m.dataset.tip; tip.hidden = false;
  const x = Math.min(ev.clientX + 14, innerWidth - tip.offsetWidth - 8);
  tip.style.left = x + 'px'; tip.style.top = (ev.clientY + 14) + 'px';
});
document.querySelectorAll('.tab').forEach((t) => t.addEventListener('click', () => {
  document.querySelectorAll('.tab').forEach((o) => o.setAttribute('aria-selected', String(o === t)));
  document.querySelectorAll('.chart').forEach((c) => { c.hidden = c.id !== 'chart-' + t.dataset.chart; });
}));
"""

started = (config.get("startedAt") or "")[:10]
scope_note = (f"Comparações do Quick usam só as {len(quick['common'])} tarefas de feature, as únicas que ele roda." if quick else "")
page = f"""<!DOCTYPE html>
<html lang="pt-BR">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Benchmark do Atlas</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=IBM+Plex+Mono:wght@400;500;600&family=IBM+Plex+Sans+Condensed:wght@500;600&family=IBM+Plex+Sans:wght@400;500;600&display=swap" rel="stylesheet">
<style>{CSS}</style>
</head>
<body>
<main>
<header>
  <p class="eyebrow">Benchmark · rodada {e(results.name)}</p>
  <h1>Atlas × {'Claude e Codex puros' if ran_codex else 'Claude puro'}</h1>
  <p class="lede">{e(headline())}</p>
  <p class="meta"><span>{len(rows)} execuções</span><span>{len(tasks)} tarefas × {reps} rodadas</span><span>modelo {e(models.get('claude', '?'))}{(' / ' + e(models.get('codex'))) if ran_codex else ''}</span><span>Atlas {e(atlas_label)}</span><span>{e(versions.get('claude', ''))}</span><span>iniciada {e(started)}</span></p>
</header>

<section aria-labelledby="h-verdict">
  <header><h2 id="h-verdict">Veredito</h2><p>Atlas comparado ao runtime puro, nas tarefas que os dois rodaram. Verde é onde o Atlas ganha; amarelo é o preço que ele cobra.</p></header>
  <div class="verdicts">{verdict_html}</div>
</section>

<section aria-labelledby="h-summary">
  <header><h2 id="h-summary">Resumo por braço</h2><p>Médias por execução. "Erros sem aviso" soma o código que os testes ocultos reprovam com os testes do próprio agente verdes e os testes próprios entregues quebrados.</p></header>
  {summary_table()}
</section>

<section aria-labelledby="h-chart">
  <header><h2 id="h-chart">Quanto custa cada tarefa</h2><p>Média das {reps} rodadas por tarefa. {e(scope_note)} Passe o mouse numa barra para ver o valor.</p></header>
  <div class="chart-card">
    <div class="chart-top"><div class="tabs" role="tablist" aria-label="Métrica">{tabs}</div><div class="legend">{legend()}</div></div>
    {panels}
  </div>
</section>

<section aria-labelledby="h-misses">
  <header><h2 id="h-misses">Onde o código entregue estava errado</h2><p>Cada execução reprovada pelos testes ocultos, com os testes que falharam e como o Atlas se saiu no mesmo problema.</p></header>
  <div class="cases">{misses()}</div>
</section>

<section aria-labelledby="h-matrix">
  <header><h2 id="h-matrix">Testes ocultos por tarefa</h2><p>Fração dos testes que o agente nunca viu e que passam no código final (na T4, defeitos plantados encontrados), com o tempo médio.</p></header>
  {matrix()}
</section>

{'<section aria-labelledby="h-roles"><header><h2 id="h-roles">Onde o Atlas gasta</h2><p>Cada passo do workflow do Atlas + Claude nesta rodada. Um passo aparece mais vezes que as execuções quando roda de novo depois de uma reprovação.</p></header>' + roles() + '</section>' if roles() else ''}

{'<section aria-labelledby="h-findings"><header><h2 id="h-findings">Análise</h2><p>Leitura dos números desta rodada, escrita à mão em findings.json.</p></header><ol class="findings">' + findings_html + '</ol></section>' if written else ''}

<section aria-labelledby="h-evolution">
  <header><h2 id="h-evolution">Como o Atlas evoluiu</h2><p>Cada rodada que mediu uma mudança no Atlas, com os números dele em T1–T4, as tarefas que todas as rodadas têm. O Claude puro não mudou entre elas: é o mesmo CLI e o mesmo modelo.</p></header>
  {evolution()}
</section>

<section aria-labelledby="h-method">
  <header><h2 id="h-method">Como foi medido</h2><p>O mesmo texto de tarefa vai para todos os braços; a avaliação não chama modelo nenhum.</p></header>
  <div class="method">
    <article><h3>Braços</h3><ul>
      <li><b>Claude puro</b>: <code>claude -p</code> com o modelo {e(models.get('claude', '?'))}, numa cópia do projeto, como no uso direto.</li>
      <li><b>Atlas + Claude</b>: o template de produto do Atlas para o tipo de tarefa (backend real, CLI real, worktree isolado), todos os agentes no mesmo modelo; planos aprovados automaticamente.</li>
      <li><b>Atlas Quick</b>: o template Quick change nas tarefas de feature, com um agente a menos.</li>
      {'<li><b>Codex puro / Atlas + Codex</b>: o mesmo, com <code>codex exec</code>.</li>' if ran_codex else ''}
    </ul></article>
    <article><h3>Tarefas</h3>{task_list()}</article>
    <article><h3>Qualidade</h3><ul>
      <li><b>Testes ocultos</b>: ficam fora do projeto que o agente vê e só entram na avaliação. Foram validados contra uma solução de referência e falham no código original.</li>
      <li><b>Testes próprios</b>: os que o agente escreveu, rodados no código final.</li>
      <li><b>Escopo</b>: arquivos alterados fora do que a tarefa permite.</li>
    </ul></article>
    <article><h3>Tokens, custo e tempo</h3><ul>
      <li><b>Entrada</b> inclui o que veio do cache do provedor (mais barato); a planilha separa as duas partes.</li>
      <li><b>Custo</b> é o preço de API que a CLI do Claude informa; no plano de assinatura sai da cota, não da fatura. O Codex não informa custo.</li>
      <li><b>Tempo</b> é o relógio da execução inteira, no Atlas incluindo a montagem do worktree.</li>
    </ul></article>
    <article><h3>Limites</h3><ul>
      <li>Amostra pequena: {reps} rodadas por tarefa. Diferenças de poucos segundos ou centavos podem ser ruído.</li>
      <li>Modelo barato ({e(models.get('claude', '?'))}) e tarefas bem especificadas, o terreno onde um agente sozinho já vai bem.</li>
      {'' if ran_codex else '<li>O Codex não rodou nesta rodada.</li>'}
      {f'<li>{len(account_lost)} execução(ões) perdida(s) por falha da conta (cota ou login) ficaram de fora.</li>' if account_lost else ''}
    </ul></article>
    <article><h3>Reproduzir</h3>
      <p>Grátis, confere o encanamento com um modelo falso:</p><pre>npm run bench:check</pre>
      <p>Rodada real (gasta tokens), com relatório e planilha comparados à rodada anterior:</p><pre>npm run bench</pre>
      <p class="sub">Dados crus em <code>bench/results/{e(results.name)}/</code>; os números por execução também estão na planilha <code>benchmark-atlas.xlsx</code>.</p>
    </article>
  </div>
</section>

<section aria-labelledby="h-runs">
  <header><h2 id="h-runs">Todas as execuções</h2><p>Uma linha por execução, na ordem de tarefa e braço.</p></header>
  {runs_table()}
</section>

<footer>Gerado por <code>bench/report_html.py</code> a partir de <code>bench/results/{e(results.name)}</code>{f' · comparado com {e(baseline.name)}' if baseline else ''}. Cada número desta página é calculado dos arquivos da rodada; só a Análise e o histórico de mudanças são escritos à mão.</footer>
</main>
<script>{JS}</script>
</body>
</html>
"""

out = results / "relatorio.html"
out.write_text(page)
print(out)
