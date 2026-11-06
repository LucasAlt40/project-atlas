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
  assert.strictEqual(s.list()[0].priority, 'normal');
});

test('add accepts low, normal and high', () => {
  const s = createStore();
  assert.strictEqual(s.add('a', { priority: 'low' }).priority, 'low');
  assert.strictEqual(s.add('b', { priority: 'normal' }).priority, 'normal');
  assert.strictEqual(s.add('c', { priority: 'high' }).priority, 'high');
});

test('add rejects unknown priorities with a RangeError', () => {
  const s = createStore();
  assert.throws(() => s.add('a', { priority: 'urgent' }), RangeError);
  assert.throws(() => s.add('a', { priority: null }), RangeError);
  assert.throws(() => s.add('a', { priority: 'toString' }), RangeError);
  assert.strictEqual(s.list().length, 0);
});

test('list sorts high, then normal, then low, keeping add order within a priority', () => {
  const s = createStore();
  s.add('l1', { priority: 'low' });
  s.add('n1');
  s.add('h1', { priority: 'high' });
  s.add('n2', { priority: 'normal' });
  s.add('h2', { priority: 'high' });
  s.add('l2', { priority: 'low' });
  assert.deepStrictEqual(s.list().map((t) => t.title), ['h1', 'h2', 'n1', 'n2', 'l1', 'l2']);
});

test('list filters by priority', () => {
  const s = createStore();
  s.add('l1', { priority: 'low' });
  s.add('h1', { priority: 'high' });
  s.add('h2', { priority: 'high' });
  assert.deepStrictEqual(s.list({ priority: 'high' }).map((t) => t.title), ['h1', 'h2']);
  assert.deepStrictEqual(s.list({ priority: 'normal' }), []);
});

test('list does not reorder the store', () => {
  const s = createStore();
  s.add('l1', { priority: 'low' });
  s.add('h1', { priority: 'high' });
  s.list();
  assert.deepStrictEqual(s.list({ priority: 'low' }).map((t) => t.title), ['l1']);
  assert.deepStrictEqual(s.list().map((t) => t.title), ['h1', 'l1']);
});

test('format marks high-priority todos with !', () => {
  assert.strictEqual(
    formatList([
      { title: 'Pay rent', done: false, priority: 'high' },
      { title: 'Water plants', done: true, priority: 'normal' },
      { title: 'Read', done: false, priority: 'low' },
    ]),
    '[ ] !Pay rent\n[x] Water plants\n[ ] Read',
  );
});
