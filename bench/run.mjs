#!/usr/bin/env node
// Runs the benchmark: each task on each arm (pure Claude, pure Codex, Atlas on Claude, Atlas on
// Codex), N repetitions, and keeps what each run consumed, how long it took and the tree it left.
// Grading is a separate step (grade.mjs), so a run is never repeated to re-grade it.
//
//   node bench/run.mjs --out bench/results/2026-10-30 \
//     [--arms claude-pure,codex-pure,atlas-claude,atlas-codex] [--tasks t1,t2] [--reps 1] \
//     [--claude-model haiku] [--codex-model gpt-5.6-luna] [--skip-existing]
//
// Every arm makes real model calls on the user's accounts.
import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, '..');
const TIMEOUT_MS = 20 * 60 * 1000;

const args = Object.fromEntries(
  process.argv
    .slice(2)
    .join(' ')
    .split(/\s*--/)
    .filter(Boolean)
    .map((pair) => {
      const [k, ...v] = pair.split(/\s+/);
      return [k, v.join(' ') || true];
    }),
);
const out = path.resolve(
  args.out ?? path.join(here, 'results', new Date().toISOString().slice(0, 10)),
);
const arms = String(args.arms ?? 'claude-pure,codex-pure,atlas-claude,atlas-codex').split(',');
const reps = Number(args.reps ?? 1);
const models = {
  claude: args['claude-model'] ?? 'haiku',
  codex: args['codex-model'] ?? 'gpt-5.6-luna',
};
const allTasks = fs.readdirSync(path.join(here, 'tasks')).sort();
const tasks = args.tasks
  ? allTasks.filter((t) =>
      String(args.tasks)
        .split(',')
        .some((p) => t.startsWith(p)),
    )
  : allTasks;

fs.mkdirSync(path.join(out, 'runs'), { recursive: true });
fs.writeFileSync(
  path.join(out, 'config.json'),
  JSON.stringify(
    {
      arms,
      tasks,
      reps,
      models,
      startedAt: new Date().toISOString(),
      host: os.hostname(),
      versions: { claude: version('claude'), codex: version('codex'), node: process.version },
    },
    null,
    2,
  ),
);

function version(cmd) {
  const r = spawnSync(cmd, ['--version'], { encoding: 'utf8' });
  return (r.stdout || r.stderr || '').trim();
}

function run(cmd, argv, opts = {}) {
  return new Promise((resolve) => {
    const began = Date.now();
    const child = spawn(cmd, argv, {
      cwd: opts.cwd,
      env: { ...process.env, ...opts.env },
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (d) => (stdout += d));
    child.stderr.on('data', (d) => (stderr += d));
    const timer = setTimeout(() => child.kill('SIGKILL'), TIMEOUT_MS);
    child.on('close', (code, signal) => {
      clearTimeout(timer);
      resolve({ code, signal, stdout, stderr, elapsedMs: Date.now() - began });
    });
  });
}

function git(cwd, ...argv) {
  spawnSync(
    'git',
    [
      '-c',
      'user.name=Bench',
      '-c',
      'user.email=bench@example.com',
      '-c',
      'commit.gpgsign=false',
      ...argv,
    ],
    { cwd },
  );
}

/** A fresh Git project holding the fixture, outside the repository (so no project CLAUDE.md/AGENTS.md leaks in). */
function workdir(task) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), `atlas-bench-${task}-`));
  fs.cpSync(path.join(here, 'tasks', task, 'fixture'), dir, { recursive: true });
  git(dir, 'init', '--quiet', '-b', 'main');
  git(dir, 'add', '--all');
  git(dir, 'commit', '--quiet', '-m', 'fixture');
  return dir;
}

function copyTree(from, to) {
  fs.cpSync(from, to, { recursive: true, filter: (src) => path.basename(src) !== '.git' });
}

const sum = (xs) => (xs.some((x) => x != null) ? xs.reduce((a, x) => a + (x ?? 0), 0) : null);

async function pureClaude(task, prompt, dest) {
  const dir = workdir(task);
  const r = await run(
    'claude',
    [
      '-p',
      prompt,
      '--model',
      models.claude,
      '--output-format',
      'json',
      '--permission-mode',
      'acceptEdits',
    ],
    { cwd: dir },
  );
  fs.writeFileSync(
    path.join(dest, 'raw.json'),
    r.stdout + (r.stderr ? `\n--- stderr\n${r.stderr}` : ''),
  );
  let res = {};
  try {
    res = JSON.parse(r.stdout.trim().split('\n').pop());
  } catch {
    /* reported below */
  }
  const u = res.usage ?? {};
  // Same arithmetic as Atlas's Claude runtime: input = fresh + cache creation + cache read.
  const input = sum([u.input_tokens, u.cache_creation_input_tokens, u.cache_read_input_tokens]);
  copyTree(dir, path.join(dest, 'tree'));
  fs.writeFileSync(path.join(dest, 'transcript.txt'), `--- Assistant\n${res.result ?? ''}\n`);
  return {
    ok: r.code === 0 && res.is_error === false,
    exitCode: r.code,
    elapsedMs: r.elapsedMs,
    inputTokens: input,
    cachedInputTokens: u.cache_read_input_tokens ?? null,
    outputTokens: u.output_tokens ?? null,
    costUsd: res.total_cost_usd ?? null,
    turns: res.num_turns ?? null,
    agentRuns: 1,
    modelUsage: res.modelUsage ?? null,
  };
}

async function pureCodex(task, prompt, dest) {
  const dir = workdir(task);
  const r = await run(
    'codex',
    [
      'exec',
      '--json',
      '--skip-git-repo-check',
      '--sandbox',
      'workspace-write',
      '--model',
      models.codex,
      prompt,
    ],
    { cwd: dir },
  );
  fs.writeFileSync(
    path.join(dest, 'raw.jsonl'),
    r.stdout + (r.stderr ? `\n--- stderr\n${r.stderr}` : ''),
  );
  const events = r.stdout.split('\n').flatMap((l) => {
    try {
      return [JSON.parse(l)];
    } catch {
      return [];
    }
  });
  const turns = events.filter((e) => e.type === 'turn.completed');
  const said = events
    .filter((e) => e.type === 'item.completed' && e.item?.type === 'agent_message')
    .map((e) => e.item.text);
  copyTree(dir, path.join(dest, 'tree'));
  fs.writeFileSync(
    path.join(dest, 'transcript.txt'),
    said.map((t) => `--- Assistant\n${t}\n`).join(''),
  );
  return {
    ok: r.code === 0 && turns.length > 0,
    exitCode: r.code,
    elapsedMs: r.elapsedMs,
    inputTokens: sum(turns.map((t) => t.usage?.input_tokens)),
    cachedInputTokens: sum(turns.map((t) => t.usage?.cached_input_tokens)),
    outputTokens: sum(turns.map((t) => t.usage?.output_tokens)),
    reasoningTokens: sum(turns.map((t) => t.usage?.reasoning_output_tokens)),
    costUsd: null,
    turns: turns.length,
    agentRuns: 1,
  };
}

async function atlas(runtime, task, meta, dest) {
  const r = await run(
    'cargo',
    [
      'test',
      '--lib',
      'application::workflow::real_stack_tests::bench_atlas_task',
      '--',
      '--ignored',
      '--exact',
      '--nocapture',
    ],
    {
      cwd: path.join(repo, 'src-tauri'),
      env: {
        ATLAS_BENCH_RUNTIME: runtime,
        ATLAS_BENCH_MODEL: models[runtime],
        ATLAS_BENCH_TEMPLATE: meta.template,
        ATLAS_BENCH_FIXTURE: path.join(here, 'tasks', task, 'fixture'),
        ATLAS_BENCH_TASK: path.join(here, 'tasks', task, 'task.md'),
        ATLAS_BENCH_OUT: dest,
      },
    },
  );
  fs.writeFileSync(path.join(dest, 'cargo.log'), `${r.stdout}\n--- stderr\n${r.stderr}`);
  let rep = null;
  try {
    rep = JSON.parse(fs.readFileSync(path.join(dest, 'atlas.json'), 'utf8'));
  } catch {
    /* the test panicked first */
  }
  const m = (rep?.usage ?? []).map((x) => x.metrics ?? {});
  return {
    ok: rep?.status === 'Completed',
    exitCode: r.code,
    status: rep?.status ?? 'NoReport',
    failure: rep?.failure ?? null,
    asked: rep?.asked ?? [],
    elapsedMs: rep?.elapsedMs ?? r.elapsedMs,
    inputTokens: sum(m.map((x) => x.inputTokens)),
    cachedInputTokens: sum(m.map((x) => x.cachedInputTokens)),
    outputTokens: sum(m.map((x) => x.outputTokens)),
    costUsd: sum(m.map((x) => x.cost)),
    agentRuns: rep?.usage?.length ?? 0,
    steps: rep?.started ?? [],
    perStep: (rep?.usage ?? []).map((x) => ({
      role: x.role,
      ms: x.completedAt - x.startedAt,
      ok: x.succeeded,
      ...x.metrics,
      thinkingTokens: Number(x.runtimeFacts?.thinkingTokens ?? NaN) || null,
      textChars: Number(x.runtimeFacts?.textChars ?? NaN) || null,
      toolInputChars: Number(x.runtimeFacts?.toolInputChars ?? NaN) || null,
      reasoningTokens: Number(x.runtimeFacts?.reasoningTokens ?? NaN) || null,
    })),
  };
}

for (let rep = 1; rep <= reps; rep++) {
  for (const task of tasks) {
    const meta = JSON.parse(fs.readFileSync(path.join(here, 'tasks', task, 'meta.json'), 'utf8'));
    const prompt = fs.readFileSync(path.join(here, 'tasks', task, 'task.md'), 'utf8').trim();
    for (const arm of arms) {
      const id = `${arm}__${task}__r${rep}`;
      const dest = path.join(out, 'runs', id);
      if (args['skip-existing'] && fs.existsSync(path.join(dest, 'metrics.json'))) continue;
      fs.rmSync(dest, { recursive: true, force: true });
      fs.mkdirSync(dest, { recursive: true });
      process.stdout.write(`${new Date().toLocaleTimeString()} ${id} ... `);
      // `atlas-<runtime>` runs the task's template; `atlas-<runtime>-quick` runs the lighter
      // "Quick change" (feature_basic) instead of "Implement" for the feature tasks.
      const quick = arm.endsWith('-quick');
      const runtime = arm.startsWith('atlas-')
        ? arm.slice(6).replace(/-quick$/, '')
        : arm.replace('-pure', '');
      const template =
        quick && meta.template === 'software_feature' ? 'feature_basic' : meta.template;
      const metrics =
        arm === 'claude-pure'
          ? await pureClaude(task, prompt, dest)
          : arm === 'codex-pure'
            ? await pureCodex(task, prompt, dest)
            : await atlas(runtime, task, { ...meta, template }, dest);
      Object.assign(metrics, {
        id,
        arm,
        task,
        rep,
        runtime,
        model: models[runtime],
        template: arm.startsWith('atlas-') ? template : null,
      });
      fs.writeFileSync(path.join(dest, 'metrics.json'), JSON.stringify(metrics, null, 2));
      console.log(
        `${metrics.ok ? 'ok' : 'FAILED'} ${(metrics.elapsedMs / 1000).toFixed(0)}s in=${metrics.inputTokens} out=${metrics.outputTokens}`,
      );
    }
  }
}
