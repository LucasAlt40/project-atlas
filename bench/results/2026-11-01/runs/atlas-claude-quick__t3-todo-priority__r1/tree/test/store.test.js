const test = require('node:test');
const assert = require('node:assert');
const { createStore } = require('../src/store.js');
const { formatList } = require('../src/format.js');

test('add and list', () => {
  const s = createStore();
  s.add('a');
  s.add('b');
  assert.deepStrictEqual(s.list().map((t) => t.title), ['a', 'b']);
});

test('complete', () => {
  const s = createStore();
  const t = s.add('a');
  assert.strictEqual(s.complete(t.id), true);
  assert.strictEqual(s.complete(999), false);
  assert.strictEqual(s.list()[0].done, true);
});

test('empty title rejected', () => {
  const s = createStore();
  assert.throws(() => s.add('  '), TypeError);
});

test('format', () => {
  assert.strictEqual(formatList([{ title: 'a', done: true }, { title: 'b', done: false }]), '[x] a\n[ ] b');
});

test('add defaults priority to normal', () => {
  const s = createStore();
  assert.strictEqual(s.add('a').priority, 'normal');
  assert.strictEqual(s.list()[0].priority, 'normal');
});

test('add stores the given priority', () => {
  const s = createStore();
  assert.strictEqual(s.add('a', { priority: 'low' }).priority, 'low');
  assert.strictEqual(s.add('b', { priority: 'high' }).priority, 'high');
});

test('add rejects an unknown priority', () => {
  const s = createStore();
  for (const priority of ['urgent', '', null, 'HIGH', 'toString']) {
    assert.throws(() => s.add('a', { priority }), RangeError, `priority ${String(priority)}`);
  }
  assert.deepStrictEqual(s.list(), []);
});

test('list sorts high, normal, low and keeps insertion order within a priority', () => {
  const s = createStore();
  s.add('low1', { priority: 'low' });
  s.add('normal1');
  s.add('high1', { priority: 'high' });
  s.add('low2', { priority: 'low' });
  s.add('high2', { priority: 'high' });
  s.add('normal2', { priority: 'normal' });
  assert.deepStrictEqual(
    s.list().map((t) => t.title),
    ['high1', 'high2', 'normal1', 'normal2', 'low1', 'low2'],
  );
});

test('list filters by priority', () => {
  const s = createStore();
  s.add('low1', { priority: 'low' });
  s.add('normal1');
  s.add('low2', { priority: 'low' });
  assert.deepStrictEqual(s.list({ priority: 'low' }).map((t) => t.title), ['low1', 'low2']);
  assert.deepStrictEqual(s.list({ priority: 'high' }), []);
});

test('list rejects an unknown filter priority', () => {
  const s = createStore();
  s.add('a');
  assert.throws(() => s.list({ priority: 'urgent' }), RangeError);
});

test('format marks high-priority todos with !', () => {
  assert.strictEqual(
    formatList([
      { title: 'Pay rent', done: false, priority: 'high' },
      { title: 'a', done: true, priority: 'normal' },
      { title: 'b', done: false, priority: 'low' },
    ]),
    '[ ] !Pay rent\n[x] a\n[ ] b',
  );
});
