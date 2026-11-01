#!/usr/bin/env node
// Grades every run of a results folder from the tree it left, without calling any model:
// hidden tests (the agent never saw them), the agent's own tests, scope (files touched outside
// what the task allowed), diff size and, for the review task, which planted defects were reported.
//
//   node bench/grade.mjs bench/results/2026-10-30
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const results = path.resolve(process.argv[2] ?? '');
const runsDir = path.join(results, 'runs');

function nodeTest(cwd, pattern) {
  const r = spawnSync('node', ['--test', '--test-reporter=tap', '--test-timeout=10000', pattern], {
    cwd,
    encoding: 'utf8',
    timeout: 60000,
  });
  const text = `${r.stdout}${r.stderr}`;
  const n = (k) => Number(text.match(new RegExp(`^# ${k} (\\d+)`, 'm'))?.[1] ?? 0);
  const failed = [...text.matchAll(/^not ok \d+ - (.+)$/gm)].map((m) => m[1]);
  return { pass: n('pass'), fail: n('fail'), failed };
}

function files(root) {
  if (!fs.existsSync(root)) return [];
  return fs
    .readdirSync(root, { recursive: true, withFileTypes: true })
    .filter((e) => e.isFile())
    .map((e) => path.relative(root, path.join(e.parentPath ?? e.path, e.name)))
    .filter(
      (p) => !p.split(path.sep).includes('.git') && !p.split(path.sep).includes('node_modules'),
    );
}

function diff(fixture, tree) {
  const before = new Set(files(fixture));
  const after = new Set(files(tree));
  const changed = [];
  let added = 0;
  let removed = 0;
  for (const p of new Set([...before, ...after])) {
    const a = before.has(p) ? fs.readFileSync(path.join(fixture, p), 'utf8') : null;
    const b = after.has(p) ? fs.readFileSync(path.join(tree, p), 'utf8') : null;
    if (a === b) continue;
    changed.push(p);
    const r = spawnSync(
      'git',
      [
        'diff',
        '--no-index',
        '--numstat',
        a === null ? '/dev/null' : path.join(fixture, p),
        b === null ? '/dev/null' : path.join(tree, p),
      ],
      { encoding: 'utf8' },
    );
    const [plus, minus] = r.stdout.trim().split(/\s+/).map(Number);
    added += plus || 0;
    removed += minus || 0;
  }
  return { changed: changed.sort(), added, removed };
}

const assistantText = (transcript) =>
  transcript
    .split(/^--- /m)
    .filter((b) => b.startsWith('Assistant'))
    .join('\n')
    .toLowerCase();

const rows = [];
for (const id of fs.readdirSync(runsDir).sort()) {
  const dir = path.join(runsDir, id);
  const metricsPath = path.join(dir, 'metrics.json');
  if (!fs.existsSync(metricsPath)) continue;
  const metrics = JSON.parse(fs.readFileSync(metricsPath, 'utf8'));
  const taskDir = path.join(here, 'tasks', metrics.task);
  const meta = JSON.parse(fs.readFileSync(path.join(taskDir, 'meta.json'), 'utf8'));
  const tree = path.join(dir, 'tree');
  const hasTree = fs.existsSync(tree);
  const d = hasTree
    ? diff(path.join(taskDir, 'fixture'), tree)
    : { changed: [], added: 0, removed: 0 };
  const outOfScope = d.changed.filter(
    (p) => !meta.allowed.some((a) => (a.endsWith('/') ? p.startsWith(a) : p === a)),
  );
  const g = { id, task: metrics.task, kind: meta.kind, ...d, outOfScope, hasTree };

  if (meta.kind === 'review') {
    const said = assistantText(
      fs.existsSync(path.join(dir, 'transcript.txt'))
        ? fs.readFileSync(path.join(dir, 'transcript.txt'), 'utf8')
        : '',
    );
    g.defects = meta.defects.map((x) => ({
      id: x.id,
      found: x.any.some((k) => said.includes(k.toLowerCase())),
    }));
    g.defectsFound = g.defects.filter((x) => x.found).length;
    g.defectsTotal = meta.defects.length;
    g.hiddenScore = g.defectsFound / g.defectsTotal;
    g.reviewChars = said.length;
  } else if (hasTree) {
    const work = fs.mkdtempSync(path.join(os.tmpdir(), 'atlas-grade-'));
    fs.cpSync(tree, work, { recursive: true });
    const own = files(work).filter((p) => /\.test\.[cm]?js$/.test(p));
    g.ownTestFiles = own.length;
    g.ownTests = own.length ? nodeTest(work, '**/*.test.js') : { pass: 0, fail: 0, failed: [] };
    fs.mkdirSync(path.join(work, '__hidden__'));
    for (const f of fs.readdirSync(path.join(taskDir, 'hidden')))
      fs.copyFileSync(path.join(taskDir, 'hidden', f), path.join(work, '__hidden__', f));
    g.hidden = nodeTest(work, '__hidden__/*.test.js');
    const total = g.hidden.pass + g.hidden.fail;
    g.hiddenScore = total ? g.hidden.pass / total : 0;
  } else {
    g.hiddenScore = 0;
  }
  fs.writeFileSync(path.join(dir, 'grade.json'), JSON.stringify(g, null, 2));
  rows.push({ ...metrics, grade: g });
  console.log(
    `${id.padEnd(40)} score=${(g.hiddenScore * 100).toFixed(0)}% changed=${d.changed.length} outOfScope=${outOfScope.length}`,
  );
}
fs.writeFileSync(path.join(results, 'all.json'), JSON.stringify(rows, null, 2));
