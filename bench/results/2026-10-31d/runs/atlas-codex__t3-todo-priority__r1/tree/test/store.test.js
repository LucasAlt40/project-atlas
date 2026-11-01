const test = require('node:test');
const assert = require('node:assert');
const { createStore } = require('../src/store.js');
const { formatList } = require('../src/format.js');

test('add and list', () => {
  const s = createStore();
  s.add('a');
  s.add('b');
  assert.deepStrictEqual(s.list().map((t) => t.title), ['a', 'b']);
  assert.deepStrictEqual(s.list().map((t) => t.priority), ['normal', 'normal']);
});

test('priority is stored and unfiltered todos are ordered by priority', () => {
  const s = createStore();
  s.add('normal 1');
  s.add('low', { priority: 'low' });
  s.add('high 1', { priority: 'high' });
  s.add('high 2', { priority: 'high' });
  s.add('normal 2', { priority: 'normal' });

  assert.deepStrictEqual(s.list().map((t) => t.title), ['high 1', 'high 2', 'normal 1', 'normal 2', 'low']);
});

test('list filters by priority', () => {
  const s = createStore();
  s.add('high', { priority: 'high' });
  s.add('normal');
  s.add('another high', { priority: 'high' });

  assert.deepStrictEqual(s.list({ priority: 'high' }).map((t) => t.title), ['high', 'another high']);
});

test('invalid priority rejected', () => {
  const s = createStore();
  assert.throws(() => s.add('a', { priority: 'urgent' }), RangeError);
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
  assert.strictEqual(formatList([
    { title: 'a', done: true },
    { title: 'b', done: false },
    { title: 'Pay rent', done: false, priority: 'high' },
  ]), '[x] a\n[ ] b\n[ ] !Pay rent');
});
