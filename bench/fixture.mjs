// Where a task's starting project comes from. Most tasks keep it in `tasks/<id>/fixture/`. A task
// built on someone else's repository (T8: the Oficina das Meninas backend) names the public
// repository and a fixed commit in `meta.json` (`fixtureRepo`) instead of carrying a copy: it is
// cloned once into `bench/.cache/fixtures/` (not in Git) and reused, the same bytes every time.
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));

function git(cwd, ...args) {
  const r = spawnSync('git', args, { cwd, encoding: 'utf8' });
  if (r.status !== 0) throw new Error(`git ${args.join(' ')}: ${r.stderr.trim()}`);
  return r.stdout;
}

/** Removes `name` wherever it is under `dir` (for files like `.DS_Store`). */
function removeEverywhere(dir, name) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.name === name) fs.rmSync(full, { recursive: true, force: true });
    else if (entry.isDirectory()) removeEverywhere(full, name);
  }
}

/** The folder holding the task's starting project, fetching it first when it comes from a repository. */
export function fixtureDir(task, meta) {
  const repo = meta.fixtureRepo;
  if (!repo) return path.join(here, 'tasks', task, 'fixture');
  const dir = path.join(here, '.cache', 'fixtures', `${task}-${repo.commit.slice(0, 12)}`);
  const ready = `${dir}.ready`;
  if (fs.existsSync(ready)) return dir;

  fs.rmSync(dir, { recursive: true, force: true });
  fs.mkdirSync(dir, { recursive: true });
  git(dir, 'init', '--quiet');
  git(dir, 'remote', 'add', 'origin', repo.url);
  git(dir, 'fetch', '--quiet', '--depth', '1', 'origin', repo.commit);
  git(dir, '-c', 'advice.detachedHead=false', 'checkout', '--quiet', 'FETCH_HEAD');
  if (git(dir, 'rev-parse', 'HEAD').trim() !== repo.commit) {
    throw new Error(`${task}: fetched commit is not ${repo.commit}`);
  }
  fs.rmSync(path.join(dir, '.git'), { recursive: true, force: true });
  // What the task removes from the repository (editor folders, Finder files).
  for (const name of repo.remove ?? []) {
    if (name.startsWith('**/')) removeEverywhere(dir, name.slice(3));
    else fs.rmSync(path.join(dir, name), { recursive: true, force: true });
  }
  for (const [file, text] of Object.entries(repo.append ?? {})) {
    fs.appendFileSync(path.join(dir, file), text);
  }
  fs.writeFileSync(ready, `${repo.url} ${repo.commit}\n`);
  return dir;
}

/**
 * Where a run keeps the tree it left. A task built on someone else's repository keeps it in the
 * local cache, never in `bench/results` (which is committed): the tree is mostly their code.
 */
export function treeDir(resultsDir, runId, meta) {
  return meta.fixtureRepo
    ? path.join(here, '.cache', 'result-trees', path.basename(resultsDir), 'runs', runId, 'tree')
    : path.join(resultsDir, 'runs', runId, 'tree');
}
