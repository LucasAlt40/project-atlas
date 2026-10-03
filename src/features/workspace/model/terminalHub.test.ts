import { terminalChunk, terminalSnapshot } from '@/test/fixtures';
import { TERMINAL_OUTPUT_LIMIT, TerminalHub } from './terminalHub';

describe('TerminalHub', () => {
  it('keeps each execution’s output apart and in order', () => {
    const hub = new TerminalHub();

    hub.push(terminalChunk('e1', 0, 'one '));
    hub.push(terminalChunk('e2', 0, 'other'));
    hub.push(terminalChunk('e1', 1, 'two'));

    expect(hub.read('e1')).toBe('one two');
    expect(hub.read('e2')).toBe('other');
    expect(hub.read('nope')).toBe('');
  });

  it('tells a listener of each new piece, and stops when it unsubscribes', () => {
    const hub = new TerminalHub();
    const seen: string[] = [];
    const stop = hub.subscribe('e1', (data) => seen.push(data));

    hub.push(terminalChunk('e1', 0, 'a'));
    hub.push(terminalChunk('e2', 0, 'not mine'));
    stop();
    hub.push(terminalChunk('e1', 1, 'b'));

    expect(seen).toEqual(['a']);
  });

  it('ignores a chunk it already has', () => {
    const hub = new TerminalHub();
    const seen: string[] = [];
    hub.subscribe('e1', (data) => seen.push(data));

    hub.push(terminalChunk('e1', 0, 'a'));
    hub.push(terminalChunk('e1', 0, 'a'));
    hub.push(terminalChunk('e1', 1, 'b'));

    expect(hub.read('e1')).toBe('ab');
    expect(seen).toEqual(['a', 'b']);
  });

  it('meets a late snapshot without a gap or a repeat', () => {
    const hub = new TerminalHub();
    hub.push(terminalChunk('e1', 0, 'a'));
    // The snapshot was taken after chunk 2: it already holds a, b, c.
    const shown = hub.load(terminalSnapshot('e1', { output: 'abc', nextSeq: 3 }));
    hub.push(terminalChunk('e1', 2, 'c'));
    hub.push(terminalChunk('e1', 3, 'd'));

    expect(shown).toBe('abc');
    expect(hub.read('e1')).toBe('abcd');
  });

  it('trusts the snapshot for what this hub missed, and keeps the chunks that follow it', () => {
    const hub = new TerminalHub();
    // The view was not listening yet when chunks 0 and 1 were written.
    hub.push(terminalChunk('e1', 2, 'c'));
    hub.push(terminalChunk('e1', 3, 'd'));

    expect(hub.load(terminalSnapshot('e1', { output: 'ab', nextSeq: 2 }))).toBe('abcd');
  });

  it('keeps the chunks it received when the snapshot is older than them', () => {
    const hub = new TerminalHub();
    hub.push(terminalChunk('e1', 0, 'a'));
    hub.push(terminalChunk('e1', 1, 'b'));

    expect(hub.load(terminalSnapshot('e1', { output: 'a', nextSeq: 1 }))).toBe('ab');
    hub.push(terminalChunk('e1', 2, 'c'));
    expect(hub.read('e1')).toBe('abc');
  });

  it('forgets cleared text but keeps following the output', () => {
    const hub = new TerminalHub();
    hub.push(terminalChunk('e1', 0, 'old'));

    hub.clear('e1');
    hub.push(terminalChunk('e1', 1, 'new'));

    expect(hub.read('e1')).toBe('new');
  });

  it('is bounded: only the newest output is kept', () => {
    const hub = new TerminalHub();
    const line = 'x'.repeat(1024);

    for (let seq = 0; seq < 700; seq++) hub.push(terminalChunk('e1', seq, line));
    hub.push(terminalChunk('e1', 700, 'END'));

    expect(hub.read('e1').length).toBe(TERMINAL_OUTPUT_LIMIT);
    expect(hub.read('e1').endsWith('END')).toBe(true);
  });

  it('forgets the oldest executions, but never one a view is watching', () => {
    const hub = new TerminalHub();
    hub.subscribe('e0', () => undefined);
    for (let n = 0; n < 12; n++) hub.push(terminalChunk(`e${String(n)}`, 0, `out${String(n)}`));

    expect(hub.read('e0')).toBe('out0');
    expect(hub.read('e1')).toBe('');
    expect(hub.read('e11')).toBe('out11');
  });
});
