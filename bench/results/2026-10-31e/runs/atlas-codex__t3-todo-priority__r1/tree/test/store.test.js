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

test('priority defaults to normal and validates values', () => {
  const s = createStore();
  assert.strictEqual(s.add('default').priority, 'normal');
  assert.strictEqual(s.add('low', { priority: 'low' }).priority, 'low');
  assert.strictEqual(s.add('high', { priority: 'high' }).priority, 'high');
  assert.throws(() => s.add('invalid', { priority: 'urgent' }), RangeError);
  assert.strictEqual(s.add('after invalid').id, 4);
  assert.deepStrictEqual(s.list().map((t) => t.title), ['high', 'default', 'after invalid', 'low']);
});

test('list filters by priority and preserves insertion order within priorities', () => {
  const s = createStore();
  s.add('first high', { priority: 'high' });
  s.add('first low', { priority: 'low' });
  s.add('second high', { priority: 'high' });
  s.add('normal');

  assert.deepStrictEqual(s.list({ priority: 'high' }).map((t) => t.title), ['first high', 'second high']);
  assert.deepStrictEqual(s.list().map((t) => t.title), ['first high', 'second high', 'normal', 'first low']);
});

test('format', () => {
  assert.strictEqual(
    formatList([{ title: 'a', done: true }, { title: 'b', done: false }, { title: 'c', done: false, priority: 'high' }]),
    '[x] a\n[ ] b\n[ ] !c',
  );
});
