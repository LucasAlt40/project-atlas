const test = require('node:test');
const assert = require('node:assert');
const { createStore } = require('../src/store.js');
const { formatList } = require('../src/format.js');

test('legacy: add and list keeps order for same priority', () => {
  const s = createStore();
  s.add('a'); s.add('b');
  assert.deepStrictEqual(s.list().map((t) => t.title), ['a', 'b']);
});
test('legacy: complete', () => {
  const s = createStore();
  const t = s.add('a');
  assert.strictEqual(s.complete(t.id), true);
  assert.strictEqual(s.complete(999), false);
});
test('legacy: empty title TypeError', () => assert.throws(() => createStore().add(''), TypeError));
test('legacy: list returns copies', () => {
  const s = createStore();
  s.add('a');
  s.list()[0].title = 'changed';
  assert.strictEqual(s.list()[0].title, 'a');
});
test('default priority normal', () => assert.strictEqual(createStore().add('a').priority, 'normal'));
test('explicit priority stored', () => assert.strictEqual(createStore().add('a', { priority: 'high' }).priority, 'high'));
test('invalid priority RangeError', () => {
  const s = createStore();
  assert.throws(() => s.add('a', { priority: 'urgent' }), RangeError);
  assert.strictEqual(s.list().length, 0, 'nothing added on error');
});
test('sorted high, normal, low, stable', () => {
  const s = createStore();
  s.add('n1'); s.add('l1', { priority: 'low' }); s.add('h1', { priority: 'high' });
  s.add('n2', { priority: 'normal' }); s.add('h2', { priority: 'high' }); s.add('l2', { priority: 'low' });
  assert.deepStrictEqual(s.list().map((t) => t.title), ['h1', 'h2', 'n1', 'n2', 'l1', 'l2']);
});
test('filter by priority', () => {
  const s = createStore();
  s.add('n1'); s.add('h1', { priority: 'high' }); s.add('l1', { priority: 'low' }); s.add('h2', { priority: 'high' });
  assert.deepStrictEqual(s.list({ priority: 'high' }).map((t) => t.title), ['h1', 'h2']);
  assert.deepStrictEqual(s.list({ priority: 'low' }).map((t) => t.title), ['l1']);
});
test('ids still increase', () => {
  const s = createStore();
  const a = s.add('a', { priority: 'low' }); const b = s.add('b');
  assert.ok(b.id > a.id);
});
test('format high with bang', () => {
  assert.strictEqual(
    formatList([{ title: 'Pay rent', done: false, priority: 'high' }, { title: 'b', done: true, priority: 'low' }]),
    '[ ] !Pay rent\n[x] b');
});
test('format legacy todos without priority', () => {
  assert.strictEqual(formatList([{ title: 'a', done: true }, { title: 'b', done: false }]), '[x] a\n[ ] b');
});
