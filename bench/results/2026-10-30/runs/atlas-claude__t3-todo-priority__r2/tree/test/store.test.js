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
    ]),
    '[x] a\n[ ] b',
  );
});

test('add defaults priority to normal', () => {
  const s = createStore();
  assert.strictEqual(s.add('a').priority, 'normal');
  assert.strictEqual(s.add('b', {}).priority, 'normal');
  assert.strictEqual(s.list()[0].priority, 'normal');
});

test('add stores each priority', () => {
  const s = createStore();
  assert.strictEqual(s.add('l', { priority: 'low' }).priority, 'low');
  assert.strictEqual(s.add('n', { priority: 'normal' }).priority, 'normal');
  assert.strictEqual(s.add('h', { priority: 'high' }).priority, 'high');
});

test('add rejects an invalid priority and leaves the store unchanged', () => {
  const s = createStore();
  s.add('a');
  for (const priority of ['urgent', 'HIGH', null]) {
    assert.throws(() => s.add('b', { priority }), RangeError);
  }
  assert.deepStrictEqual(
    s.list().map((t) => t.title),
    ['a'],
  );
});

test('list sorts high, normal, low; insertion order within a priority', () => {
  const s = createStore();
  s.add('n1');
  s.add('h1', { priority: 'high' });
  s.add('l1', { priority: 'low' });
  s.add('h2', { priority: 'high' });
  s.add('n2');
  assert.deepStrictEqual(
    s.list().map((t) => t.title),
    ['h1', 'h2', 'n1', 'n2', 'l1'],
  );
});

test('list filters by priority', () => {
  const s = createStore();
  s.add('l1', { priority: 'low' });
  s.add('n1');
  s.add('l2', { priority: 'low' });
  assert.deepStrictEqual(
    s.list({ priority: 'low' }).map((t) => t.title),
    ['l1', 'l2'],
  );
});

test('list rejects an invalid priority filter', () => {
  const s = createStore();
  assert.throws(() => s.list({ priority: 'bogus' }), RangeError);
});

test('list() and list({}) match', () => {
  const s = createStore();
  s.add('n1');
  s.add('h1', { priority: 'high' });
  assert.deepStrictEqual(s.list(), s.list({}));
});

test('format marks high priority with !', () => {
  assert.strictEqual(
    formatList([{ title: 'Pay rent', done: false, priority: 'high' }]),
    '[ ] !Pay rent',
  );
  assert.strictEqual(
    formatList([{ title: 'Pay rent', done: true, priority: 'high' }]),
    '[x] !Pay rent',
  );
  assert.strictEqual(
    formatList([
      { title: 'a', done: false, priority: 'normal' },
      { title: 'b', done: false, priority: 'low' },
    ]),
    '[ ] a\n[ ] b',
  );
});
