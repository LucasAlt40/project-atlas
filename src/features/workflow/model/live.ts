import type {
  FileChangeDto,
  LivePhaseDto,
  LiveWorkspaceStateDto,
  LiveWorkspaceUpdateDto,
} from '@/lib/tauri/commands';
import type { TranslationKey } from '@/i18n';
import type { WorkflowRun } from '../types';

/**
 * What the screen knows of a run's worktree: the backend's snapshot, kept up to date by its
 * updates. Nothing here reads a file or runs Git; it only applies what the backend says.
 */
export interface LiveModel {
  state: LiveWorkspaceStateDto;
  /**
   * The revision at which each file was last announced as changed. A file whose number moved is
   * a file whose text may have, even when its counts did not: it is what tells the viewer to read
   * it again, and the only thing that does.
   */
  touched: Readonly<Record<string, number>>;
}

export function modelOf(state: LiveWorkspaceStateDto): LiveModel {
  return {
    state,
    touched: Object.fromEntries(state.files.map((f) => [f.path, state.revision])),
  };
}

export type Applied =
  /** The update followed the state: here is the next one. */
  | { kind: 'applied'; model: LiveModel }
  /** Older than (or the same as) what is shown, or about another worktree: ignored. */
  | { kind: 'stale' }
  /** Revisions were skipped: what the screen has cannot be trusted, read the snapshot again. */
  | { kind: 'gap' };

const byPath = (a: FileChangeDto, b: FileChangeDto) =>
  a.path < b.path ? -1 : a.path > b.path ? 1 : 0;

/**
 * Applies one update. Only the very next revision is accepted: an older one never overwrites a
 * newer state, and a gap means an update was missed (or the state is not what the backend thinks).
 */
export function applyUpdate(model: LiveModel, update: LiveWorkspaceUpdateDto): Applied {
  const { state } = model;
  if (update.worktreeExecutionId !== state.worktreeExecutionId) return { kind: 'stale' };
  if (update.revision <= state.revision) return { kind: 'stale' };
  if (update.revision !== state.revision + 1) return { kind: 'gap' };

  const files = new Map<string, FileChangeDto>(
    update.full ? [] : state.files.map((f) => [f.path, f]),
  );
  for (const path of update.removed) files.delete(path);
  for (const change of update.changed) files.set(change.path, change);

  const touched: Record<string, number> = {};
  for (const [path] of files) touched[path] = model.touched[path] ?? update.revision;
  // A full read names every file: any of them may have changed, so each is read again.
  for (const change of update.changed) touched[change.path] = update.revision;

  return {
    kind: 'applied',
    model: {
      touched,
      state: {
        ...state,
        files: [...files.values()].sort(byPath),
        filesChanged: update.filesChanged,
        additions: update.additions,
        deletions: update.deletions,
        currentRevision: update.currentRevision,
        availability: update.availability,
        phase: update.phase,
        observation: update.observation,
        revision: update.revision,
        updatedAt: update.updatedAt,
      },
    },
  };
}

// ---- activity ---------------------------------------------------------------------------------

/** Something that really happened to the worktree, as an update reported it. */
export type ActivityEntry =
  | { id: string; at: number; kind: 'file'; path: string; status: FileChangeDto['status'] }
  | { id: string; at: number; kind: 'reverted'; path: string }
  | { id: string; at: number; kind: 'phase'; phase: LivePhaseDto };

export const MAX_ACTIVITY = 200;

/**
 * What an update says happened. For a full read, only what differs from before is listed (the
 * rest is not news); a new phase is news too. Nothing is made up: no entry exists without a file
 * or a phase the backend reported.
 */
export function activityOf(
  before: LiveModel,
  update: LiveWorkspaceUpdateDto,
  after: LiveModel,
): ActivityEntry[] {
  const entries: ActivityEntry[] = [];
  const at = update.updatedAt;
  const id = (suffix: string) => `${String(update.revision)}:${suffix}`;
  if (update.phase !== before.state.phase) {
    entries.push({ id: id('phase'), at, kind: 'phase', phase: update.phase });
  }
  const previous = new Map(before.state.files.map((f) => [f.path, f]));
  const touched = update.full
    ? after.state.files.filter((f) => !sameChange(previous.get(f.path), f))
    : update.changed;
  for (const file of touched) {
    entries.push({
      id: id(`f:${file.path}`),
      at,
      kind: 'file',
      path: file.path,
      status: file.status,
    });
  }
  const gone = update.full
    ? before.state.files.filter((f) => !after.state.files.some((n) => n.path === f.path))
    : update.removed.map((path) => ({ path }));
  for (const file of gone) {
    entries.push({ id: id(`r:${file.path}`), at, kind: 'reverted', path: file.path });
  }
  return entries;
}

function sameChange(a: FileChangeDto | undefined, b: FileChangeDto): boolean {
  if (a === undefined) return false;
  return (
    a.status === b.status &&
    a.additions === b.additions &&
    a.deletions === b.deletions &&
    a.oldPath === b.oldPath
  );
}

/** Newest first, bounded. */
export function pushActivity(list: ActivityEntry[], entries: ActivityEntry[]): ActivityEntry[] {
  return [...entries.slice().reverse(), ...list].slice(0, MAX_ACTIVITY);
}

// ---- what to say ------------------------------------------------------------------------------

export type LiveTone = 'running' | 'waiting' | 'idle' | 'done' | 'stopped' | 'unavailable';

export interface Headline {
  /** LIVE while the run may still change the worktree; REVIEW once it is over (or gone). */
  mode: 'live' | 'review';
  tone: LiveTone;
  label: TranslationKey;
}

/**
 * The one line that says what is going on. It is the backend's phase and availability, said in
 * words: no percentage, no "editing X", nothing the backend did not report.
 */
export function headlineOf(state: LiveWorkspaceStateDto, run: WorkflowRun): Headline {
  if (state.availability === 'invalid') {
    return { mode: 'review', tone: 'unavailable', label: 'live.state.invalid' };
  }
  if (state.availability === 'missing') {
    return { mode: 'review', tone: 'unavailable', label: 'live.state.missing' };
  }
  switch (state.phase) {
    case 'running':
      return { mode: 'live', tone: 'running', label: 'live.state.running' };
    case 'waiting_for_input':
      return { mode: 'live', tone: 'waiting', label: 'live.state.waiting' };
    case 'cancelled':
      return { mode: 'live', tone: 'stopped', label: 'live.state.cancelled' };
    case 'ended':
      return { mode: 'review', tone: 'done', label: 'live.state.ended' };
    case 'stopped':
      return { mode: 'review', tone: 'stopped', label: 'live.state.stopped' };
    case 'idle':
      return run.status === 'running' || run.status === 'paused'
        ? { mode: 'live', tone: 'idle', label: 'live.state.idle' }
        : { mode: 'review', tone: 'idle', label: 'live.state.idleEnded' };
  }
}

// ---- the file tree ----------------------------------------------------------------------------

export interface TreeRow {
  kind: 'dir' | 'file';
  /** Full path of the folder or file. */
  path: string;
  name: string;
  depth: number;
  change?: FileChangeDto;
  /** Files below a folder. */
  count?: number;
}

/**
 * The changed files as folders and files, in path order, with the folders in `collapsed` closed.
 * Folders with a single folder inside are shown as one (`src/app`), as editors do.
 */
export function treeRows(
  files: readonly FileChangeDto[],
  collapsed: ReadonlySet<string>,
): TreeRow[] {
  interface Dir {
    name: string;
    path: string;
    dirs: Map<string, Dir>;
    files: FileChangeDto[];
  }
  const root: Dir = { name: '', path: '', dirs: new Map(), files: [] };
  for (const file of files) {
    const parts = file.path.split('/');
    let dir = root;
    for (const part of parts.slice(0, -1)) {
      const path = dir.path === '' ? part : `${dir.path}/${part}`;
      let next = dir.dirs.get(part);
      if (!next) {
        next = { name: part, path, dirs: new Map(), files: [] };
        dir.dirs.set(part, next);
      }
      dir = next;
    }
    dir.files.push(file);
  }
  const countOf = (dir: Dir): number =>
    dir.files.length + [...dir.dirs.values()].reduce((n, d) => n + countOf(d), 0);
  const rows: TreeRow[] = [];
  const walk = (dir: Dir, depth: number) => {
    const dirs = [...dir.dirs.values()].sort((a, b) => a.name.localeCompare(b.name));
    for (const child of dirs) {
      let shown = child;
      let name = child.name;
      while (shown.files.length === 0 && shown.dirs.size === 1) {
        const [only] = [...shown.dirs.values()];
        if (!only) break;
        name = `${name}/${only.name}`;
        shown = only;
      }
      rows.push({ kind: 'dir', path: shown.path, name, depth, count: countOf(shown) });
      if (!collapsed.has(shown.path)) walk(shown, depth + 1);
    }
    for (const file of dir.files.slice().sort(byPath)) {
      rows.push({
        kind: 'file',
        path: file.path,
        name: file.path.split('/').at(-1) ?? file.path,
        depth,
        change: file,
      });
    }
  };
  walk(root, 0);
  return rows;
}

export const STATUS_LETTER: Record<FileChangeDto['status'], string> = {
  added: 'A',
  modified: 'M',
  deleted: 'D',
  renamed: 'R',
};

export const STATUS_LABEL: Record<FileChangeDto['status'], TranslationKey> = {
  added: 'integration.file.added',
  modified: 'integration.file.modified',
  deleted: 'integration.file.deleted',
  renamed: 'integration.file.renamed',
};
