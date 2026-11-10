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

test('priority defaults to normal', () => {
  const s = createStore();
  assert.strictEqual(s.add('a').priority, 'normal');
  assert.strictEqual(s.add('b', {}).priority, 'normal');
  assert.strictEqual(s.add('c', { priority: undefined }).priority, 'normal');
});

test('add stores the given priority', () => {
  const s = createStore();
  assert.strictEqual(s.add('a', { priority: 'low' }).priority, 'low');
  assert.strictEqual(s.add('b', { priority: 'high' }).priority, 'high');
});

test('invalid priority rejected on add', () => {
  const s = createStore();
  for (const priority of ['urgent', 'HIGH', '', null, 0, 'toString', '__proto__']) {
    assert.throws(() => s.add('a', { priority }), RangeError, `priority ${String(priority)}`);
  }
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

test('list filters by priority in insertion order', () => {
  const s = createStore();
  s.add('l1', { priority: 'low' });
  s.add('h1', { priority: 'high' });
  s.add('l2', { priority: 'low' });
  assert.deepStrictEqual(s.list({ priority: 'low' }).map((t) => t.title), ['l1', 'l2']);
  assert.deepStrictEqual(s.list({ priority: 'high' }).map((t) => t.title), ['h1']);
  assert.deepStrictEqual(s.list({ priority: 'normal' }), []);
});

test('invalid priority rejected on list filter', () => {
  const s = createStore();
  s.add('a');
  assert.throws(() => s.list({ priority: 'urgent' }), RangeError);
  assert.throws(() => s.list({ priority: null }), RangeError);
});

test('list returns copies', () => {
  const s = createStore();
  s.add('a', { priority: 'high' });
  s.list()[0].title = 'changed';
  assert.strictEqual(s.list()[0].title, 'a');
});

test('format marks high priority with !', () => {
  const todos = [
    { title: 'a', done: false, priority: 'high' },
    { title: 'b', done: true, priority: 'normal' },
    { title: 'c', done: false, priority: 'low' },
  ];
  assert.strictEqual(formatList(todos), '[ ] !a\n[x] b\n[ ] c');
});
