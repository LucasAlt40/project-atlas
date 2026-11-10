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
  assert.strictEqual(s.add('b', {}).priority, 'normal');
  assert.strictEqual(s.add('c', { priority: 'low' }).priority, 'low');
  assert.strictEqual(s.list()[2].priority, 'low');
});

test('invalid priority rejected', () => {
  const s = createStore();
  for (const priority of ['urgent', 'High', '', null, 1]) {
    assert.throws(() => s.add('a', { priority }), RangeError);
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

test('list keeps insertion order in the store after sorting', () => {
  const s = createStore();
  s.add('low', { priority: 'low' });
  s.add('high', { priority: 'high' });
  s.list();
  s.add('normal');
  assert.deepStrictEqual(s.list().map((t) => t.title), ['high', 'normal', 'low']);
  assert.deepStrictEqual(s.list({ priority: 'low' }).map((t) => t.title), ['low']);
});

test('list filters by priority', () => {
  const s = createStore();
  s.add('a', { priority: 'low' });
  s.add('b', { priority: 'high' });
  s.add('c', { priority: 'low' });
  assert.deepStrictEqual(s.list({ priority: 'low' }).map((t) => t.title), ['a', 'c']);
  assert.deepStrictEqual(s.list({ priority: 'high' }).map((t) => t.title), ['b']);
  assert.deepStrictEqual(s.list({ priority: 'normal' }), []);
});

test('list rejects an unknown filter priority', () => {
  const s = createStore();
  s.add('a');
  assert.throws(() => s.list({ priority: 'urgent' }), RangeError);
});

test('list returns copies', () => {
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
