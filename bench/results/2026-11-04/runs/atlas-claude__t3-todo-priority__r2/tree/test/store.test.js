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
});

test('add stores the given priority', () => {
  const s = createStore();
  assert.strictEqual(s.add('a', { priority: 'high' }).priority, 'high');
  assert.strictEqual(s.add('b', { priority: 'low' }).priority, 'low');
});

test('add rejects an unknown priority', () => {
  const s = createStore();
  assert.throws(() => s.add('a', { priority: 'urgent' }), RangeError);
  assert.throws(() => s.add('a', { priority: null }), RangeError);
  assert.strictEqual(s.list().length, 0);
});

test('list sorts high, normal, low and keeps insertion order within a priority', () => {
  const s = createStore();
  s.add('n1');
  s.add('l1', { priority: 'low' });
  s.add('h1', { priority: 'high' });
  s.add('n2', { priority: 'normal' });
  s.add('h2', { priority: 'high' });
  s.add('l2', { priority: 'low' });
  assert.deepStrictEqual(s.list().map((t) => t.title), ['h1', 'h2', 'n1', 'n2', 'l1', 'l2']);
});

test('list filters by priority', () => {
  const s = createStore();
  s.add('a', { priority: 'low' });
  s.add('b', { priority: 'high' });
  s.add('c', { priority: 'low' });
  assert.deepStrictEqual(s.list({ priority: 'low' }).map((t) => t.title), ['a', 'c']);
  assert.deepStrictEqual(s.list({ priority: 'normal' }), []);
});

test('list rejects an unknown filter priority', () => {
  const s = createStore();
  assert.throws(() => s.list({ priority: 'urgent' }), RangeError);
});

test('list returns copies that do not change the store', () => {
  const s = createStore();
  s.add('a');
  s.list()[0].title = 'changed';
  assert.strictEqual(s.list()[0].title, 'a');
});

test('format marks high priority with !', () => {
  assert.strictEqual(
    formatList([
      { title: 'Pay rent', done: false, priority: 'high' },
      { title: 'a', done: true, priority: 'normal' },
      { title: 'b', done: false, priority: 'low' },
    ]),
    '[ ] !Pay rent\n[x] a\n[ ] b',
  );
});
