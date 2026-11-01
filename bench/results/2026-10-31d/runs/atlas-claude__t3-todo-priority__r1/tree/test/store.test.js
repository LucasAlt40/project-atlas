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
  assert.strictEqual(s.add('b', { priority: 'low' }).priority, 'low');
  assert.strictEqual(s.add('c', { priority: 'high' }).priority, 'high');
  assert.strictEqual(s.list().find((t) => t.title === 'a').priority, 'normal');
});

test('invalid priority rejected with RangeError', () => {
  const s = createStore();
  assert.throws(() => s.add('a', { priority: 'urgent' }), RangeError);
  assert.throws(() => s.add('a', { priority: null }), RangeError);
  assert.throws(() => s.add('a', { priority: 'toString' }), RangeError);
  assert.throws(() => s.list({ priority: 'urgent' }), RangeError);
  assert.strictEqual(s.list().length, 0);
});

test('title is checked before priority', () => {
  const s = createStore();
  assert.throws(() => s.add('  ', { priority: 'urgent' }), TypeError);
});

test('list filters by priority', () => {
  const s = createStore();
  s.add('a', { priority: 'low' });
  s.add('b');
  s.add('c', { priority: 'low' });
  s.add('d', { priority: 'high' });
  assert.deepStrictEqual(s.list({ priority: 'low' }).map((t) => t.title), ['a', 'c']);
  assert.deepStrictEqual(s.list({ priority: 'high' }).map((t) => t.title), ['d']);
});

test('list sorts high, normal, low and keeps insertion order within a priority', () => {
  const s = createStore();
  s.add('a', { priority: 'low' });
  s.add('b');
  s.add('c', { priority: 'high' });
  s.add('d', { priority: 'low' });
  s.add('e', { priority: 'high' });
  s.add('f');
  assert.deepStrictEqual(s.list().map((t) => t.title), ['c', 'e', 'b', 'f', 'a', 'd']);
});

test('list returns copies that do not change the store', () => {
  const s = createStore();
  s.add('a');
  s.list()[0].title = 'changed';
  assert.strictEqual(s.list()[0].title, 'a');
});

test('formatList marks high priority with !', () => {
  assert.strictEqual(
    formatList([
      { title: 'Pay rent', done: false, priority: 'high' },
      { title: 'a', done: true, priority: 'normal' },
      { title: 'b', done: false, priority: 'low' },
    ]),
    '[ ] !Pay rent\n[x] a\n[ ] b',
  );
});
