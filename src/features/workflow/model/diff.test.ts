import { parseDiff } from './diff';

const DIFF = [
  'diff --git a/src/a.ts b/src/a.ts',
  'index 111..222 100644',
  '--- a/src/a.ts',
  '+++ b/src/a.ts',
  '@@ -1,3 +1,4 @@',
  ' one',
  '-two',
  '+TWO',
  '+two and a half',
  ' three',
  '@@ -10,2 +11,2 @@ function x() {',
  ' ten',
  '-eleven',
  '+ELEVEN',
  '',
].join('\n');

describe('reading a unified diff', () => {
  it('numbers the lines of both sides from the hunk headers', () => {
    const { rows } = parseDiff(DIFF);

    expect(rows.find((r) => r.kind === 'del' && r.text === 'two')).toMatchObject({ oldNo: 2 });
    expect(rows.find((r) => r.kind === 'add' && r.text === 'TWO')).toMatchObject({ newNo: 2 });
    expect(rows.find((r) => r.kind === 'add' && r.text === 'two and a half')).toMatchObject({
      newNo: 3,
    });
    expect(rows.find((r) => r.kind === 'ctx' && r.text === 'three')).toMatchObject({
      oldNo: 3,
      newNo: 4,
    });
    expect(rows.find((r) => r.kind === 'del' && r.text === 'eleven')).toMatchObject({ oldNo: 11 });
    expect(rows.find((r) => r.kind === 'add' && r.text === 'ELEVEN')).toMatchObject({ newNo: 12 });
  });

  it('counts what was added and removed, and finds where each block of changes starts', () => {
    const parsed = parseDiff(DIFF);

    expect([parsed.additions, parsed.deletions]).toEqual([3, 2]);
    expect(parsed.changeStarts).toHaveLength(2);
    expect(parsed.rows[parsed.changeStarts[0] ?? 0]).toMatchObject({ kind: 'del', text: 'two' });
  });

  it('does not take the file header’s --- and +++ for removed and added lines', () => {
    const { rows } = parseDiff(DIFF);

    expect(rows.filter((r) => r.kind === 'meta').map((r) => r.text)).toContain('--- a/src/a.ts');
    expect(rows.some((r) => r.kind === 'add' && r.text.startsWith('++'))).toBe(false);
  });

  it('reads a new file and a deleted file', () => {
    const added = parseDiff(
      'diff --git a/n.ts b/n.ts\nnew file mode 100644\n--- /dev/null\n+++ b/n.ts\n@@ -0,0 +1,2 @@\n+a\n+b\n',
    );
    const removed = parseDiff(
      'diff --git a/o.ts b/o.ts\ndeleted file mode 100644\n--- a/o.ts\n+++ /dev/null\n@@ -1,2 +0,0 @@\n-a\n-b\n',
    );

    expect([added.additions, added.deletions]).toEqual([2, 0]);
    expect([removed.additions, removed.deletions]).toEqual([0, 2]);
  });

  it('an empty diff has no rows', () => {
    expect(parseDiff('')).toEqual({ rows: [], changeStarts: [], additions: 0, deletions: 0 });
  });
});
