import { fileChange, liveState, liveUpdate, run, passwordRecovery } from '@/test/workflowFixtures';
import {
  activityOf,
  applyUpdate,
  headlineOf,
  modelOf,
  pushActivity,
  treeRows,
  MAX_ACTIVITY,
  type LiveModel,
} from './live';

const base = (): LiveModel =>
  modelOf(
    liveState({ files: [fileChange('src/a.ts'), fileChange('src/b.ts', 'added')], revision: 4 }),
  );

const applied = (model: LiveModel, update: ReturnType<typeof liveUpdate>): LiveModel => {
  const result = applyUpdate(model, update);
  if (result.kind !== 'applied') throw new Error(`expected an applied update, got ${result.kind}`);
  return result.model;
};

describe('applying the backend’s updates by revision', () => {
  it('applies the very next revision: a file added, one modified, one removed', () => {
    const next = applied(
      base(),
      liveUpdate(5, {
        changed: [
          fileChange('src/c.ts', 'added'),
          fileChange('src/a.ts', 'modified', { additions: 9 }),
        ],
        removed: ['src/b.ts'],
        filesChanged: 2,
        additions: 12,
        deletions: 1,
        phase: 'waiting_for_input',
      }),
    );

    expect(next.state.revision).toBe(5);
    expect(next.state.files.map((f) => f.path)).toEqual(['src/a.ts', 'src/c.ts']);
    expect(next.state.files[0]?.additions).toBe(9);
    expect(next.state.phase).toBe('waiting_for_input');
    expect(next.state.filesChanged).toBe(2);
    expect(next.state.additions).toBe(12);
  });

  it('a deleted file stays, as a change with the status deleted', () => {
    const next = applied(
      base(),
      liveUpdate(5, { changed: [fileChange('src/a.ts', 'deleted')], filesChanged: 2 }),
    );

    expect(next.state.files.find((f) => f.path === 'src/a.ts')?.status).toBe('deleted');
  });

  it('a rename arrives as one entry that remembers where the file was', () => {
    const next = applied(
      base(),
      liveUpdate(5, {
        full: true,
        changed: [
          fileChange('src/b.ts', 'added'),
          fileChange('lib/a.ts', 'renamed', { oldPath: 'src/a.ts', additions: 0, deletions: 0 }),
        ],
        filesChanged: 2,
      }),
    );

    expect(next.state.files.find((f) => f.path === 'lib/a.ts')).toMatchObject({
      status: 'renamed',
      oldPath: 'src/a.ts',
    });
    expect(next.state.files.some((f) => f.path === 'src/a.ts')).toBe(false);
  });

  it('a full update replaces the files: whatever it does not name is gone', () => {
    const next = applied(
      base(),
      liveUpdate(5, { full: true, changed: [fileChange('only.ts')], filesChanged: 1 }),
    );

    expect(next.state.files.map((f) => f.path)).toEqual(['only.ts']);
  });

  it('an older or repeated revision never overwrites a newer state', () => {
    const model = base();

    expect(applyUpdate(model, liveUpdate(4, { changed: [fileChange('x.ts')] })).kind).toBe('stale');
    expect(applyUpdate(model, liveUpdate(2, { changed: [fileChange('x.ts')] })).kind).toBe('stale');
  });

  it('a skipped revision is a gap: the snapshot has to be read again', () => {
    expect(applyUpdate(base(), liveUpdate(6, { changed: [fileChange('x.ts')] })).kind).toBe('gap');
  });

  it('an update about another worktree is ignored', () => {
    const other = liveUpdate(5, { worktreeExecutionId: 'exec-99' });

    expect(applyUpdate(base(), other).kind).toBe('stale');
  });

  it('remembers which revision each file last changed at, so only that file is read again', () => {
    const first = base();
    const next = applied(
      first,
      liveUpdate(5, { changed: [fileChange('src/a.ts')], filesChanged: 2 }),
    );

    expect(next.touched['src/a.ts']).toBe(5);
    expect(next.touched['src/b.ts']).toBe(first.touched['src/b.ts']);
  });
});

describe('the activity', () => {
  it('lists what the update reported: files with their real status, and a new phase', () => {
    const before = base();
    const update = liveUpdate(5, {
      changed: [fileChange('src/c.ts', 'added')],
      removed: ['src/b.ts'],
      phase: 'waiting_for_input',
      filesChanged: 2,
    });
    const after = applied(before, update);

    const entries = activityOf(before, update, after);

    expect(entries).toEqual([
      expect.objectContaining({ kind: 'phase', phase: 'waiting_for_input' }),
      expect.objectContaining({ kind: 'file', path: 'src/c.ts', status: 'added' }),
      expect.objectContaining({ kind: 'reverted', path: 'src/b.ts' }),
    ]);
  });

  it('for a full read, lists only what differs from before', () => {
    const before = base();
    const update = liveUpdate(5, {
      full: true,
      changed: [
        fileChange('src/a.ts'),
        fileChange('src/b.ts', 'added'),
        fileChange('src/new.ts', 'added'),
      ],
      filesChanged: 3,
    });

    const entries = activityOf(before, update, applied(before, update));

    expect(entries.map((e) => ('path' in e ? e.path : e.kind))).toEqual(['src/new.ts']);
  });

  it('says nothing for an update that changed nothing the screen shows', () => {
    const before = base();
    const update = liveUpdate(5, { phase: before.state.phase });

    expect(activityOf(before, update, applied(before, update))).toEqual([]);
  });

  it('puts the newest first and keeps a bounded list', () => {
    let list = pushActivity(
      [],
      [
        { id: '1', at: 1, kind: 'reverted', path: 'a' },
        { id: '2', at: 2, kind: 'reverted', path: 'b' },
      ],
    );
    expect(list.map((e) => e.id)).toEqual(['2', '1']);

    for (let n = 0; n < MAX_ACTIVITY + 20; n += 1) {
      list = pushActivity(list, [{ id: `n${String(n)}`, at: n, kind: 'reverted', path: 'x' }]);
    }
    expect(list).toHaveLength(MAX_ACTIVITY);
  });
});

describe('what the screen says about the run', () => {
  const w = passwordRecovery();
  const active = run(w, 'running', {});
  const done = run(w, 'completed', {});
  const failed = run(w, 'failed', {});
  const say = (state: Parameters<typeof liveState>[0], r = active) =>
    headlineOf(liveState(state), r);

  it('is LIVE while the agent works, waits for a person, or between steps', () => {
    expect(say({ phase: 'running' })).toMatchObject({ mode: 'live', label: 'live.state.running' });
    expect(say({ phase: 'waiting_for_input' })).toMatchObject({
      mode: 'live',
      tone: 'waiting',
      label: 'live.state.waiting',
    });
    expect(say({ phase: 'idle' })).toMatchObject({ mode: 'live', label: 'live.state.idle' });
    expect(say({ phase: 'cancelled' })).toMatchObject({
      mode: 'live',
      label: 'live.state.cancelled',
    });
  });

  it('is REVIEW once the run is over', () => {
    expect(say({ phase: 'ended' }, done)).toMatchObject({
      mode: 'review',
      tone: 'done',
      label: 'live.state.ended',
    });
    expect(say({ phase: 'stopped' }, failed)).toMatchObject({
      mode: 'review',
      label: 'live.state.stopped',
    });
    expect(say({ phase: 'idle' }, done)).toMatchObject({ mode: 'review' });
  });

  it('says plainly when the worktree is gone or is not what Atlas made, whatever the phase was', () => {
    expect(say({ availability: 'missing', phase: 'running' })).toMatchObject({
      mode: 'review',
      tone: 'unavailable',
      label: 'live.state.missing',
    });
    expect(say({ availability: 'invalid' })).toMatchObject({ label: 'live.state.invalid' });
  });
});

describe('the file tree', () => {
  const files = [
    fileChange('src/auth/login.ts'),
    fileChange('src/auth/token.ts', 'added'),
    fileChange('src/index.ts'),
    fileChange('README.md', 'deleted'),
  ];

  it('groups by folder, folders first, with the count below each', () => {
    const rows = treeRows(files, new Set());

    expect(rows.map((r) => `${r.kind}:${String(r.depth)}:${r.name}`)).toEqual([
      'dir:0:src',
      'dir:1:auth',
      'file:2:login.ts',
      'file:2:token.ts',
      'file:1:index.ts',
      'file:0:README.md',
    ]);
    expect(rows[0]?.count).toBe(3);
  });

  it('shows a folder holding only a folder as one line, and hides the children of a closed one', () => {
    const deep = [fileChange('a/b/c/d.ts')];

    expect(treeRows(deep, new Set()).map((r) => r.name)).toEqual(['a/b/c', 'd.ts']);
    expect(treeRows(files, new Set(['src/auth'])).map((r) => r.name)).not.toContain('login.ts');
  });

  it('carries each file’s own status from the backend', () => {
    const readme = treeRows(files, new Set()).find((r) => r.path === 'README.md');

    expect(readme?.change?.status).toBe('deleted');
  });
});
