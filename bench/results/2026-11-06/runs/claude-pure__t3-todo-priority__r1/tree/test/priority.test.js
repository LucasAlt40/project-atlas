const test = require('node:test');
const assert = require('node:assert');
const { createStore } = require('../src/store.js');
const { formatList } = require('../src/format.js');

test('priority defaults to normal', () => {
  const s = createStore();
  assert.strictEqual(s.add('a').priority, 'normal');
  assert.strictEqual(s.add('b', {}).priority, 'normal');
  assert.strictEqual(s.add('c', { priority: undefined }).priority, 'normal');
});

test('priority is stored on the todo', () => {
  const s = createStore();
  assert.strictEqual(s.add('a', { priority: 'low' }).priority, 'low');
  assert.strictEqual(s.add('b', { priority: 'high' }).priority, 'high');
  assert.strictEqual(s.list()[0].priority, 'high');
});

test('invalid priority throws RangeError', () => {
  const s = createStore();
  assert.throws(() => s.add('a', { priority: 'urgent' }), RangeError);
  assert.throws(() => s.add('a', { priority: null }), RangeError);
  assert.throws(() => s.add('a', { priority: 'HIGH' }), RangeError);
  assert.strictEqual(s.list().length, 0);
});

test('list sorts high, then normal, then low, keeping insertion order within a priority', () => {
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

test('list with a priority filter returns only that priority, in insertion order', () => {
  const s = createStore();
  s.add('a', { priority: 'low' });
  s.add('b', { priority: 'high' });
  s.add('c', { priority: 'low' });
  s.add('d', { priority: 'high' });
  assert.deepStrictEqual(s.list({ priority: 'low' }).map((t) => t.title), ['a', 'c']);
  assert.deepStrictEqual(s.list({ priority: 'high' }).map((t) => t.title), ['b', 'd']);
  assert.deepStrictEqual(s.list({ priority: 'normal' }), []);
});

test('list keeps completion state and does not reorder the store', () => {
  const s = createStore();
  const low = s.add('low', { priority: 'low' });
  s.add('high', { priority: 'high' });
  assert.strictEqual(s.complete(low.id), true);
  assert.deepStrictEqual(s.list().map((t) => [t.title, t.done]), [['high', false], ['low', true]]);
});

test('list returns copies that do not affect the store', () => {
  const s = createStore();
  s.add('a', { priority: 'high' });
  s.list()[0].priority = 'low';
  assert.strictEqual(s.list()[0].priority, 'high');
});

test('formatList marks high-priority todos with !', () => {
  assert.strictEqual(
    formatList([
      { title: 'Pay rent', done: false, priority: 'high' },
      { title: 'b', done: true, priority: 'normal' },
      { title: 'c', done: false, priority: 'low' },
    ]),
    '[ ] !Pay rent\n[x] b\n[ ] c',
  );
});

test('formatList leaves todos without a priority unmarked', () => {
  assert.strictEqual(formatList([{ title: 'a', done: false }]), '[ ] a');
});
