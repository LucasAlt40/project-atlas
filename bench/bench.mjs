#!/usr/bin/env node
// The whole benchmark in one command: checks the machine, compiles Atlas once, runs every arm,
// grades the trees and builds the spreadsheet (compared with the previous round).
//
//   node bench/bench.mjs --check          # free: fake model, one task, checks the plumbing
//   node bench/bench.mjs                  # real round: every arm, every task, 2 reps (spends tokens)
//
// Options (all optional): --out <dir> --arms a,b --tasks t1,t8 --reps 2 --claude-model haiku
// --codex-model gpt-5.6-luna --baseline <dir>|none --skip-existing (resume a stopped round).
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, '..');
const ALL_ARMS = ['claude-pure', 'atlas-claude', 'atlas-claude-quick', 'codex-pure', 'atlas-codex'];
const FAKE_MODEL = 'not-a-real-model-xyz';

const argv = process.argv.slice(2);
const flag = (k) => argv.includes(`--${k}`);
const opt = (k, d) => {
  const i = argv.indexOf(`--${k}`);
  return i >= 0 && argv[i + 1] && !argv[i + 1].startsWith('--') ? argv[i + 1] : d;
};

const check = flag('check');
const today = new Date().toISOString().slice(0, 10);
const out = path.resolve(
  opt(
    'out',
    check
      ? path.join(os.tmpdir(), `atlas-bench-check-${Date.now()}`)
      : path.join(here, 'results', today),
  ),
);
const arms = opt('arms', check ? 'claude-pure,atlas-claude' : ALL_ARMS.join(','))
  .split(',')
  .filter(Boolean);
const tasks = opt('tasks', check ? 't1' : '');
const reps = opt('reps', check ? '1' : '2');
const models = {
  claude: check ? FAKE_MODEL : opt('claude-model', 'haiku'),
  codex: check ? FAKE_MODEL : opt('codex-model', 'gpt-5.6-luna'),
};

const say = (s) => console.log(`\n== ${s}`);
const die = (s) => {
  console.error(`\nERRO: ${s}`);
  process.exit(1);
};
const sh = (cmd, a, o = {}) => spawnSync(cmd, a, { encoding: 'utf8', ...o });
const has = (cmd) => sh('sh', ['-c', `command -v ${cmd}`]).stdout.trim().length > 0;

for (const a of arms) if (!ALL_ARMS.includes(a)) die(`braço desconhecido: ${a}`);
const usesClaude = arms.some((a) => a.includes('claude'));
const usesCodex = arms.some((a) => a.includes('codex'));
const usesAtlas = arms.some((a) => a.startsWith('atlas-'));
const taskList = fs
  .readdirSync(path.join(here, 'tasks'))
  .sort()
  .filter((t) => !tasks || tasks.split(',').some((p) => t.startsWith(p)));
if (!taskList.length) die(`nenhuma tarefa casa com --tasks ${tasks}`);

// ---------------------------------------------------------------- 1. the machine
say('1/5 Conferindo a máquina');
const problems = [];
if (!has('python3') || sh('python3', ['-c', 'import openpyxl']).status !== 0)
  problems.push('python3 com openpyxl (pip install openpyxl)');
if (usesAtlas && !has('cargo')) problems.push('cargo (Rust) para os braços do Atlas');
if (usesClaude) {
  if (!has('claude')) problems.push('a CLI claude');
  else if (!check) {
    const s = sh('claude', ['auth', 'status']).stdout;
    if (!/"loggedIn":\s*true/.test(s))
      problems.push('o Claude não está logado (rode: claude /login)');
  }
}
if (usesCodex) {
  if (!has('codex')) problems.push('a CLI codex');
  else if (!check) {
    const s = sh('codex', ['login', 'status']);
    if (!/logged in/i.test(`${s.stdout}${s.stderr}`))
      problems.push('o Codex não está logado (rode: codex login)');
  }
}
const javaTasks = taskList.filter((t) => {
  const m = JSON.parse(fs.readFileSync(path.join(here, 'tasks', t, 'meta.json'), 'utf8'));
  return m.runner === 'maven';
});
if (javaTasks.length) {
  const { mavenBin } = await import('./maven.mjs');
  const mvn = mavenBin();
  if (mvn === 'mvn' && !has('mvn'))
    problems.push(
      `Maven para ${javaTasks.join(', ')} (nenhum mvn nem download do wrapper em ~/.m2)`,
    );
  if (!fs.existsSync(path.join(os.homedir(), '.m2', 'repository')))
    problems.push('~/.m2/repository (as dependências Java rodam offline; o SSD está montado?)');
  if (!has('java')) problems.push('java (JDK 21) para a tarefa Java');
}
if (problems.length) die(`falta:\n  - ${problems.join('\n  - ')}`);
const featureTasks = taskList.filter(
  (t) =>
    JSON.parse(fs.readFileSync(path.join(here, 'tasks', t, 'meta.json'), 'utf8')).template ===
    'software_feature',
).length;
const runs =
  arms.reduce((n, a) => n + (a.endsWith('-quick') ? featureTasks : taskList.length), 0) *
  Number(reps);
console.log(
  `ok · ${runs} execuções (${arms.join(', ')} × ${taskList.length} tarefas × ${reps}) · modelos ${models.claude}${usesCodex ? ` / ${models.codex}` : ''}`,
);
console.log(`resultados em ${out}`);
if (check)
  console.log(
    'modo --check: modelo falso, nada é cobrado; as execuções devem falhar na primeira chamada.',
  );
else
  console.log(
    'ATENÇÃO: chama modelos reais na sua conta. Ctrl+C para parar; --skip-existing retoma depois.',
  );

// ---------------------------------------------------------------- 2. compile Atlas once
if (usesAtlas) {
  say('2/5 Compilando o Atlas (uma vez, fora do tempo das execuções)');
  const c = sh('cargo', ['test', '--lib', '--no-run', '-q'], {
    cwd: path.join(repo, 'src-tauri'),
    stdio: 'inherit',
  });
  if (c.status !== 0) die('o Atlas não compilou');
} else say('2/5 Sem braços do Atlas: nada a compilar');

// ---------------------------------------------------------------- 3. run
say('3/5 Rodando');
const runArgs = [
  path.join(here, 'run.mjs'),
  '--out',
  out,
  '--arms',
  arms.join(','),
  '--reps',
  reps,
  '--claude-model',
  models.claude,
  '--codex-model',
  models.codex,
  ...(tasks ? ['--tasks', tasks] : []),
  ...(flag('skip-existing') ? ['--skip-existing'] : []),
];
if (sh('node', runArgs, { stdio: 'inherit' }).status !== 0) die('run.mjs parou com erro');

// ---------------------------------------------------------------- 4. grade
say('4/5 Avaliando (testes ocultos, escopo, testes próprios)');
if (sh('node', [path.join(here, 'grade.mjs'), out], { stdio: 'inherit' }).status !== 0)
  die('grade.mjs parou com erro');

// ---------------------------------------------------------------- 5. report
say('5/5 Gerando a planilha');
let baseline = opt('baseline', 'auto');
if (baseline === 'auto') {
  // The latest earlier round in bench/results (folder names are dates, so they sort in time).
  const resultsDir = path.join(here, 'results');
  baseline =
    fs
      .readdirSync(resultsDir)
      .filter((d) => d < path.basename(out) && fs.existsSync(path.join(resultsDir, d, 'all.json')))
      .sort()
      .map((d) => path.join(resultsDir, d))
      .pop() ?? 'none';
  if (check) baseline = 'none';
}
const reportArgs = [
  path.join(here, 'report.py'),
  out,
  ...(baseline !== 'none' ? ['--baseline', baseline] : []),
];
const rep = sh('python3', reportArgs, { stdio: 'inherit' });
if (rep.status !== 0) die('report.py parou com erro');

// ---------------------------------------------------------------- summary
const all = JSON.parse(fs.readFileSync(path.join(out, 'all.json'), 'utf8'));
const lost = fs
  .readdirSync(path.join(out, 'runs'))
  .filter(
    (d) =>
      fs.existsSync(path.join(out, 'runs', d, 'account-failure.json')) &&
      !fs.existsSync(path.join(out, 'runs', d, 'metrics.json')),
  );
say('Pronto');
for (const a of arms) {
  const r = all.filter((x) => x.arm === a);
  if (!r.length) continue;
  const avg = (f) => r.reduce((s, x) => s + (f(x) ?? 0), 0) / r.length;
  console.log(
    `${a.padEnd(20)} ${String(r.length).padStart(3)} exec · concluiu ${r.filter((x) => x.ok).length} · ocultos ${(avg((x) => x.grade.hiddenScore) * 100).toFixed(0)}% · ${avg((x) => x.elapsedMs / 1000).toFixed(0)} s · US$ ${avg((x) => x.costUsd).toFixed(4)}`,
  );
}
if (lost.length)
  console.log(
    `\n${lost.length} execução(ões) não contaram por falha da conta (cota/login): rode de novo com --skip-existing quando a conta voltar.`,
  );
console.log(
  `\nPlanilha: ${path.join(out, 'benchmark-atlas.xlsx')}${baseline !== 'none' ? `\nComparada com: ${path.basename(baseline)}` : ''}`,
);
if (check)
  console.log('\n--check ok: o encanamento funciona. Para a rodada real: node bench/bench.mjs');
