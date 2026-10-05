/** A unified diff, as rows a screen can draw with line numbers. */

export type DiffRowKind = 'file' | 'meta' | 'hunk' | 'add' | 'del' | 'ctx';

export interface DiffRow {
  kind: DiffRowKind;
  text: string;
  /** Line number in the old text (not for added lines). */
  oldNo?: number;
  /** Line number in the new text (not for removed lines). */
  newNo?: number;
}

export interface ParsedDiff {
  rows: DiffRow[];
  /** Where each block of changed lines starts (a row index): what "next change" jumps between. */
  changeStarts: number[];
  additions: number;
  deletions: number;
}

const HUNK = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/;

export function parseDiff(text: string): ParsedDiff {
  const rows: DiffRow[] = [];
  const changeStarts: number[] = [];
  let additions = 0;
  let deletions = 0;
  let oldNo = 0;
  let newNo = 0;
  let inHunk = false;
  let inChange = false;
  const lines = text.split('\n');
  if (lines.at(-1) === '') lines.pop();
  for (const line of lines) {
    if (line.startsWith('diff ')) {
      rows.push({ kind: 'file', text: line });
      inHunk = false;
      inChange = false;
      continue;
    }
    const hunk = HUNK.exec(line);
    if (hunk) {
      oldNo = Number(hunk[1]);
      newNo = Number(hunk[2]);
      rows.push({ kind: 'hunk', text: line });
      inHunk = true;
      inChange = false;
      continue;
    }
    if (!inHunk) {
      rows.push({ kind: 'meta', text: line });
      continue;
    }
    if (line.startsWith('+')) {
      if (!inChange) changeStarts.push(rows.length);
      inChange = true;
      additions += 1;
      rows.push({ kind: 'add', text: line.slice(1), newNo });
      newNo += 1;
    } else if (line.startsWith('-')) {
      if (!inChange) changeStarts.push(rows.length);
      inChange = true;
      deletions += 1;
      rows.push({ kind: 'del', text: line.slice(1), oldNo });
      oldNo += 1;
    } else if (line.startsWith('\\')) {
      // "\ No newline at end of file"
      rows.push({ kind: 'meta', text: line });
    } else {
      inChange = false;
      rows.push({ kind: 'ctx', text: line.slice(1), oldNo, newNo });
      oldNo += 1;
      newNo += 1;
    }
  }
  return { rows, changeStarts, additions, deletions };
}
