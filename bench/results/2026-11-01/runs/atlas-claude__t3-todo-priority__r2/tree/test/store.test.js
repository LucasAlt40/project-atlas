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
  assert.strictEqual(s.add('d', { priority: 'high' }).priority, 'high');
});

test('invalid priority rejected', () => {
  const s = createStore();
  for (const priority of [
    'urgent',
    'HIGH',
    '',
    null,
    'toString',
    'constructor',
    ['high'],
    new String('high'),
    { toString: () => 'high' },
  ]) {
    assert.throws(() => s.add('a', { priority }), RangeError, String(priority));
  }
  assert.deepStrictEqual(s.list(), []);
});

test('list rejects non-string priority filter', () => {
  const s = createStore();
  s.add('h1', { priority: 'high' });
  assert.throws(() => s.list({ priority: ['high'] }), RangeError);
  assert.throws(() => s.list({ priority: new String('high') }), RangeError);
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

test('list filters by priority and keeps insertion order', () => {
  const s = createStore();
  s.add('l1', { priority: 'low' });
  s.add('h1', { priority: 'high' });
  s.add('l2', { priority: 'low' });
  assert.deepStrictEqual(s.list({ priority: 'low' }).map((t) => t.title), ['l1', 'l2']);
  assert.deepStrictEqual(s.list({ priority: 'normal' }), []);
});

test('list rejects invalid priority filter', () => {
  const s = createStore();
  assert.throws(() => s.list({ priority: 'urgent' }), RangeError);
});

test('list returns copies', () => {
  const s = createStore();
  s.add('a', { priority: 'high' });
  s.list()[0].title = 'changed';
  assert.strictEqual(s.list()[0].title, 'a');
});

test('format marks high priority with !', () => {
  assert.strictEqual(
    formatList([
      { title: 'Pay rent', done: false, priority: 'high' },
      { title: 'b', done: true, priority: 'normal' },
      { title: 'c', done: false, priority: 'low' },
    ]),
    '[ ] !Pay rent\n[x] b\n[ ] c',
  );
});
