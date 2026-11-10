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
  assert.strictEqual(s.list()[0].priority, 'normal');
});

test('add accepts valid priorities', () => {
  const s = createStore();
  assert.strictEqual(s.add('low', { priority: 'low' }).priority, 'low');
  assert.strictEqual(s.add('normal', { priority: 'normal' }).priority, 'normal');
  assert.strictEqual(s.add('high', { priority: 'high' }).priority, 'high');
});

test('invalid priority rejected', () => {
  const s = createStore();
  assert.throws(() => s.add('a', { priority: 'urgent' }), RangeError);
  assert.throws(() => s.add('a', { priority: null }), RangeError);
});

test('list sorts by priority while preserving order within a priority', () => {
  const s = createStore();
  s.add('normal 1');
  s.add('low 1', { priority: 'low' });
  s.add('high 1', { priority: 'high' });
  s.add('high 2', { priority: 'high' });
  s.add('normal 2');
  s.add('low 2', { priority: 'low' });

  assert.deepStrictEqual(
    s.list().map((t) => t.title),
    ['high 1', 'high 2', 'normal 1', 'normal 2', 'low 1', 'low 2'],
  );
});

test('list filters by priority', () => {
  const s = createStore();
  s.add('high', { priority: 'high' });
  s.add('normal');
  s.add('low', { priority: 'low' });

  assert.deepStrictEqual(
    s.list({ priority: 'high' }).map((t) => t.title),
    ['high'],
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
      { title: 'b', done: false, priority: 'high' },
      { title: 'c', done: false, priority: 'low' },
    ]),
    '[x] a\n[ ] !b\n[ ] c',
  );
});
