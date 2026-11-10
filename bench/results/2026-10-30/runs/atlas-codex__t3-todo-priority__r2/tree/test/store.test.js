const test = require('node:test');
const assert = require('node:assert');
const { createStore } = require('../src/store.js');
const { formatList } = require('../src/format.js');

test('add and list', () => {
  const s = createStore();
  s.add('a');
  s.add('b');
  assert.deepStrictEqual(
    s.list().map((t) => t.title),
    ['a', 'b'],
  );
  assert.deepStrictEqual(
    s.list().map((t) => t.priority),
    ['normal', 'normal'],
  );
});

test('priority is accepted and preserved', () => {
  const s = createStore();
  assert.strictEqual(s.add('low', { priority: 'low' }).priority, 'low');
  assert.strictEqual(s.add('normal', { priority: 'normal' }).priority, 'normal');
  assert.strictEqual(s.add('high', { priority: 'high' }).priority, 'high');
  assert.deepStrictEqual(
    s.list().map((t) => t.priority),
    ['high', 'normal', 'low'],
  );
});

test('invalid priority is rejected without adding a todo', () => {
  const s = createStore();
  assert.throws(() => s.add('a', { priority: 'urgent' }), RangeError);
  assert.throws(() => s.add('a', { priority: null }), RangeError);
  assert.deepStrictEqual(s.list(), []);
});

test('list filters by priority and preserves order within a priority', () => {
  const s = createStore();
  s.add('normal one');
  s.add('high one', { priority: 'high' });
  s.add('low one', { priority: 'low' });
  s.add('high two', { priority: 'high' });
  s.add('normal two');

  assert.deepStrictEqual(
    s.list().map((t) => t.title),
    ['high one', 'high two', 'normal one', 'normal two', 'low one'],
  );
  assert.deepStrictEqual(
    s.list({ priority: 'high' }).map((t) => t.title),
    ['high one', 'high two'],
  );
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
  assert.strictEqual(
    formatList([
      { title: 'a', done: true },
      { title: 'b', done: false },
      { title: 'c', done: false, priority: 'high' },
    ]),
    '[x] a\n[ ] b\n[ ] !c',
  );
});
