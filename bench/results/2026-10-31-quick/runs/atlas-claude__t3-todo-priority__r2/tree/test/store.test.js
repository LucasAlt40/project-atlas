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

test('priority defaults to normal and is stored on the todo', () => {
  const s = createStore();
  assert.strictEqual(s.add('a').priority, 'normal');
  assert.strictEqual(s.add('b', { priority: 'low' }).priority, 'low');
  assert.strictEqual(s.add('c', { priority: 'high' }).priority, 'high');
});

test('invalid priority is rejected', () => {
  const s = createStore();
  assert.throws(() => s.add('a', { priority: 'urgent' }), RangeError);
  assert.throws(() => s.add('a', { priority: null }), RangeError);
  assert.throws(() => s.list({ priority: 'urgent' }), RangeError);
  assert.deepStrictEqual(s.list(), []);
});

test('list filters by priority', () => {
  const s = createStore();
  s.add('a', { priority: 'low' });
  s.add('b');
  s.add('c', { priority: 'low' });
  assert.deepStrictEqual(s.list({ priority: 'low' }).map((t) => t.title), ['a', 'c']);
  assert.deepStrictEqual(s.list({ priority: 'high' }), []);
});

test('list sorts high, normal, low and keeps insertion order within a priority', () => {
  const s = createStore();
  s.add('low-1', { priority: 'low' });
  s.add('normal-1');
  s.add('high-1', { priority: 'high' });
  s.add('normal-2');
  s.add('high-2', { priority: 'high' });
  assert.deepStrictEqual(
    s.list().map((t) => t.title),
    ['high-1', 'high-2', 'normal-1', 'normal-2', 'low-1'],
  );
});

test('list returns copies, so callers cannot change the store', () => {
  const s = createStore();
  s.add('a', { priority: 'high' });
  s.list()[0].priority = 'low';
  assert.strictEqual(s.list()[0].priority, 'high');
});

test('format marks high-priority todos with !', () => {
  assert.strictEqual(
    formatList([
      { title: 'Pay rent', done: false, priority: 'high' },
      { title: 'Read', done: true, priority: 'normal' },
      { title: 'Tidy', done: false, priority: 'low' },
    ]),
    '[ ] !Pay rent\n[x] Read\n[ ] Tidy',
  );
});
