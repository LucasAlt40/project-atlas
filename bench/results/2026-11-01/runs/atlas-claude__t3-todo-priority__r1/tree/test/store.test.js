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

test('priority defaults to normal and is stored', () => {
  const s = createStore();
  assert.strictEqual(s.add('a').priority, 'normal');
  assert.strictEqual(s.add('b', {}).priority, 'normal');
  assert.strictEqual(s.add('c', { priority: 'low' }).priority, 'low');
  assert.strictEqual(s.list()[2].priority, 'low');
});

test('invalid priority rejected by add', () => {
  const s = createStore();
  assert.throws(() => s.add('a', { priority: 'urgent' }), RangeError);
  assert.throws(() => s.add('a', { priority: null }), RangeError);
  assert.strictEqual(s.list().length, 0);
});

test('list filters by priority', () => {
  const s = createStore();
  s.add('a', { priority: 'low' });
  s.add('b', { priority: 'high' });
  s.add('c');
  s.add('d', { priority: 'low' });
  assert.deepStrictEqual(s.list({ priority: 'low' }).map((t) => t.title), ['a', 'd']);
  assert.deepStrictEqual(s.list({ priority: 'high' }).map((t) => t.title), ['b']);
  assert.deepStrictEqual(s.list({ priority: 'normal' }).map((t) => t.title), ['c']);
});

test('invalid priority rejected by list filter', () => {
  const s = createStore();
  assert.throws(() => s.list({ priority: 'urgent' }), RangeError);
});

test('list sorts high, normal, low and keeps insertion order within a priority', () => {
  const s = createStore();
  s.add('low1', { priority: 'low' });
  s.add('normal1');
  s.add('high1', { priority: 'high' });
  s.add('low2', { priority: 'low' });
  s.add('high2', { priority: 'high' });
  s.add('normal2', { priority: 'normal' });
  const titles = s.list().map((t) => t.title);
  assert.deepStrictEqual(titles, ['high1', 'high2', 'normal1', 'normal2', 'low1', 'low2']);
  // The sort must not reorder the store itself: a repeated call gives the same result.
  assert.deepStrictEqual(s.list().map((t) => t.title), titles);
});

test('format marks high-priority todos with !', () => {
  assert.strictEqual(
    formatList([
      { title: 'Pay rent', done: false, priority: 'high' },
      { title: 'a', done: true, priority: 'normal' },
      { title: 'b', done: false, priority: 'low' },
    ]),
    '[ ] !Pay rent\n[x] a\n[ ] b'
  );
});
