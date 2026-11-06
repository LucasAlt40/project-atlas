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

test('priority defaults to normal and is stored on the todo', () => {
  const s = createStore();
  assert.strictEqual(s.add('a').priority, 'normal');
  assert.strictEqual(s.add('b', {}).priority, 'normal');
  assert.strictEqual(s.add('c', { priority: 'low' }).priority, 'low');
  assert.strictEqual(s.add('d', { priority: 'high' }).priority, 'high');
});

test('invalid priority on add throws RangeError and leaves the store unchanged', () => {
  const s = createStore();
  s.add('a');
  for (const priority of ['urgent', '', null, 'HIGH', 'constructor']) {
    assert.throws(() => s.add('b', { priority }), RangeError);
  }
  assert.deepStrictEqual(s.list().map((t) => t.title), ['a']);
});

test('empty title is still rejected before priority is checked', () => {
  const s = createStore();
  assert.throws(() => s.add(' ', { priority: 'urgent' }), TypeError);
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
  assert.deepStrictEqual(
    s.list().map((t) => t.title),
    ['h1', 'h2', 'n1', 'n2', 'l1', 'l2'],
  );
});

test('list does not reorder the internal store', () => {
  const s = createStore();
  s.add('l', { priority: 'low' });
  s.add('h', { priority: 'high' });
  s.list();
  assert.deepStrictEqual(s.list().map((t) => t.title), ['h', 'l']);
  s.add('n');
  assert.deepStrictEqual(s.list().map((t) => t.title), ['h', 'n', 'l']);
});

test('list returns copies, so mutating a result does not affect the store', () => {
  const s = createStore();
  s.add('a');
  s.list()[0].title = 'changed';
  assert.strictEqual(s.list()[0].title, 'a');
});

test('list filters by priority', () => {
  const s = createStore();
  s.add('l', { priority: 'low' });
  s.add('n', { priority: 'normal' });
  s.add('h', { priority: 'high' });
  s.add('h2', { priority: 'high' });
  assert.deepStrictEqual(s.list({ priority: 'high' }).map((t) => t.title), ['h', 'h2']);
  assert.deepStrictEqual(s.list({ priority: 'normal' }).map((t) => t.title), ['n']);
  assert.deepStrictEqual(s.list({ priority: 'low' }).map((t) => t.title), ['l']);
});

test('list with an unknown filter priority throws RangeError', () => {
  const s = createStore();
  s.add('a');
  assert.throws(() => s.list({ priority: 'urgent' }), RangeError);
});

test('format marks high-priority todos with a !', () => {
  assert.strictEqual(
    formatList([
      { title: 'Pay rent', done: false, priority: 'high' },
      { title: 'Tidy', done: true, priority: 'low' },
      { title: 'Read', done: false, priority: 'normal' },
    ]),
    '[ ] !Pay rent\n[x] Tidy\n[ ] Read',
  );
});
