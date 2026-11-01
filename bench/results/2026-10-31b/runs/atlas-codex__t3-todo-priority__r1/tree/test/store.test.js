const test = require('node:test');
const assert = require('node:assert');
const { createStore } = require('../src/store.js');
const { formatList } = require('../src/format.js');

test('add and list', () => {
  const s = createStore();
  s.add('a');
  s.add('b');
  assert.deepStrictEqual(s.list().map((t) => t.title), ['a', 'b']);
  assert.strictEqual(s.list()[0].priority, 'normal');
});

test('priority is accepted and invalid priorities are rejected', () => {
  const s = createStore();
  assert.strictEqual(s.add('low', { priority: 'low' }).priority, 'low');
  assert.strictEqual(s.add('high', { priority: 'high' }).priority, 'high');
  assert.throws(() => s.add('invalid', { priority: 'urgent' }), RangeError);
  assert.throws(() => s.add('boxed', { priority: new String('high') }), RangeError);
});

test('list orders priorities while preserving insertion order', () => {
  const s = createStore();
  s.add('normal one');
  s.add('low one', { priority: 'low' });
  s.add('high one', { priority: 'high' });
  s.add('high two', { priority: 'high' });
  s.add('normal two');

  assert.deepStrictEqual(s.list().map((t) => t.title), [
    'high one',
    'high two',
    'normal one',
    'normal two',
    'low one',
  ]);
});

test('list filters by priority', () => {
  const s = createStore();
  s.add('high', { priority: 'high' });
  s.add('normal');
  s.add('another high', { priority: 'high' });

  assert.deepStrictEqual(s.list({ priority: 'high' }).map((t) => t.title), ['high', 'another high']);
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
      { title: 'a', done: true, priority: 'high' },
      { title: 'b', done: false, priority: 'low' },
      { title: 'c', done: false },
    ]),
    '[x] !a\n[ ] b\n[ ] c',
  );
});
