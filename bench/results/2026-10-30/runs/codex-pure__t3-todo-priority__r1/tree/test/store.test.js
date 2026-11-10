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

test('priorities default and validate', () => {
  const s = createStore();
  assert.strictEqual(s.add('default').priority, 'normal');
  assert.strictEqual(s.add('low', { priority: 'low' }).priority, 'low');
  assert.strictEqual(s.add('high', { priority: 'high' }).priority, 'high');
  assert.throws(() => s.add('invalid', { priority: 'urgent' }), RangeError);
});

test('list filters and sorts by priority while preserving insertion order', () => {
  const s = createStore();
  s.add('normal 1');
  s.add('low', { priority: 'low' });
  s.add('high 1', { priority: 'high' });
  s.add('high 2', { priority: 'high' });
  s.add('normal 2');

  assert.deepStrictEqual(s.list().map((t) => t.title), ['high 1', 'high 2', 'normal 1', 'normal 2', 'low']);
  assert.deepStrictEqual(s.list({ priority: 'high' }).map((t) => t.title), ['high 1', 'high 2']);
});

test('format', () => {
  assert.strictEqual(formatList([{ title: 'a', done: true }, { title: 'b', done: false }]), '[x] a\n[ ] b');
});

test('format marks high-priority todos', () => {
  assert.strictEqual(formatList([{ title: 'Pay rent', done: false, priority: 'high' }]), '[ ] ! Pay rent');
});
