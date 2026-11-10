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
  assert.deepStrictEqual(s.list({ priority: 'normal' }).map((t) => t.priority), ['normal', 'normal']);
});

test('invalid priority rejected on add', () => {
  const s = createStore();
  assert.throws(() => s.add('a', { priority: 'urgent' }), RangeError);
  assert.throws(() => s.add('a', { priority: 'HIGH' }), RangeError);
  assert.throws(() => s.add('a', { priority: null }), RangeError);
  assert.deepStrictEqual(s.list(), []);
});

test('list filters by priority', () => {
  const s = createStore();
  s.add('a', { priority: 'low' });
  s.add('b', { priority: 'high' });
  s.add('c');
  s.add('d', { priority: 'high' });
  assert.deepStrictEqual(s.list({ priority: 'high' }).map((t) => t.title), ['b', 'd']);
  assert.deepStrictEqual(s.list({ priority: 'low' }).map((t) => t.title), ['a']);
  assert.deepStrictEqual(s.list({ priority: 'normal' }).map((t) => t.title), ['c']);
});

test('invalid priority filter rejected', () => {
  const s = createStore();
  assert.throws(() => s.list({ priority: 'urgent' }), RangeError);
});

test('list sorts high, normal, low and keeps insertion order within a priority', () => {
  const s = createStore();
  s.add('low-1', { priority: 'low' });
  s.add('normal-1');
  s.add('high-1', { priority: 'high' });
  s.add('low-2', { priority: 'low' });
  s.add('high-2', { priority: 'high' });
  s.add('normal-2', { priority: 'normal' });
  assert.deepStrictEqual(
    s.list().map((t) => t.title),
    ['high-1', 'high-2', 'normal-1', 'normal-2', 'low-1', 'low-2'],
  );
});

test('list returns copies, so changing a result does not change the store', () => {
  const s = createStore();
  s.add('a', { priority: 'low' });
  s.list()[0].title = 'changed';
  s.list()[0].priority = 'high';
  assert.deepStrictEqual(s.list().map((t) => [t.title, t.priority]), [['a', 'low']]);
});

test('add with null or undefined options defaults to normal', () => {
  const s = createStore();
  assert.strictEqual(s.add('a', null).priority, 'normal');
  assert.strictEqual(s.add('b', undefined).priority, 'normal');
});

test('list with null filter returns all todos', () => {
  const s = createStore();
  s.add('a', { priority: 'low' });
  s.add('b', { priority: 'high' });
  assert.deepStrictEqual(s.list(null).map((t) => t.title), ['b', 'a']);
});

test('invalid priority filter rejected when null', () => {
  const s = createStore();
  assert.throws(() => s.list({ priority: null }), RangeError);
});

test('format prefixes a done high-priority title with !', () => {
  assert.strictEqual(
    formatList([{ title: 'Pay rent', done: true, priority: 'high' }]),
    '[x] !Pay rent',
  );
});

test('format prefixes high-priority titles with !', () => {
  assert.strictEqual(
    formatList([
      { title: 'Pay rent', done: false, priority: 'high' },
      { title: 'a', done: true, priority: 'normal' },
      { title: 'b', done: false, priority: 'low' },
    ]),
    '[ ] !Pay rent\n[x] a\n[ ] b',
  );
});
