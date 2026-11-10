const test = require('node:test');
const assert = require('node:assert');
const { createStore } = require('../src/store.js');
const { formatList } = require('../src/format.js');

test('priority defaults to normal', () => {
  const s = createStore();
  assert.strictEqual(s.add('a').priority, 'normal');
  assert.strictEqual(s.add('b', {}).priority, 'normal');
});

test('explicit priority is stored', () => {
  const s = createStore();
  assert.strictEqual(s.add('a', { priority: 'low' }).priority, 'low');
  assert.strictEqual(s.add('b', { priority: 'high' }).priority, 'high');
});

test('invalid priority throws RangeError and adds nothing', () => {
  const s = createStore();
  assert.throws(() => s.add('a', { priority: 'urgent' }), RangeError);
  assert.throws(() => s.add('a', { priority: null }), RangeError);
  assert.deepStrictEqual(s.list(), []);
});

test('list sorts high, then normal, then low, keeping insertion order within a priority', () => {
  const s = createStore();
  s.add('n1');
  s.add('l1', { priority: 'low' });
  s.add('h1', { priority: 'high' });
  s.add('n2', { priority: 'normal' });
  s.add('h2', { priority: 'high' });
  s.add('l2', { priority: 'low' });
  assert.deepStrictEqual(s.list().map((t) => t.title), ['h1', 'h2', 'n1', 'n2', 'l1', 'l2']);
});

test('list with filter.priority returns only that priority', () => {
  const s = createStore();
  s.add('a', { priority: 'low' });
  s.add('b', { priority: 'high' });
  s.add('c', { priority: 'low' });
  assert.deepStrictEqual(s.list({ priority: 'low' }).map((t) => t.title), ['a', 'c']);
  assert.deepStrictEqual(s.list({ priority: 'high' }).map((t) => t.title), ['b']);
  assert.deepStrictEqual(s.list({ priority: 'normal' }), []);
});

test('list returns copies, so callers cannot change the store', () => {
  const s = createStore();
  s.add('a');
  s.list()[0].priority = 'high';
  assert.strictEqual(s.list()[0].priority, 'normal');
});

test('format marks only high-priority todos with !', () => {
  const s = createStore();
  s.add('Pay rent', { priority: 'high' });
  s.add('Buy milk');
  s.add('Someday', { priority: 'low' });
  assert.strictEqual(formatList(s.list()), '[ ] !Pay rent\n[ ] Buy milk\n[ ] Someday');
});

test('format puts ! after the checkbox for done high-priority todos', () => {
  assert.strictEqual(formatList([{ title: 'a', done: true, priority: 'high' }]), '[x] !a');
});
