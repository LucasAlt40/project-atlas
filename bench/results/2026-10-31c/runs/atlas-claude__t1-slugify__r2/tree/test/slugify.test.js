const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify.js');

test('lowercases and joins words with a single dash', () => {
  assert.strictEqual(slugify('Hello World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Über Ñandú'), 'uber-nandu');
});

test('collapses runs of non a-z0-9 characters into one dash', () => {
  assert.strictEqual(slugify('a   b---c!!!d'), 'a-b-c-d');
  assert.strictEqual(slugify('foo_bar.baz/qux'), 'foo-bar-baz-qux');
});

test('never starts or ends with a dash', () => {
  assert.strictEqual(slugify('  --hello--  '), 'hello');
  assert.strictEqual(slugify('!!!'), '');
  assert.strictEqual(slugify(''), '');
});

test('keeps digits', () => {
  assert.strictEqual(slugify('Version 2.0 Final'), 'version-2-0-final');
});

test('returns the full slug when maxLength is not exceeded', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
  assert.strictEqual(slugify('hello big world', { maxLength: 100 }), 'hello-big-world');
});

test('cuts at the last dash that fits', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
  assert.strictEqual(slugify('hello big world', { maxLength: 10 }), 'hello-big');
});

test('hard cuts the first word when it does not fit', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
  assert.strictEqual(slugify('supercalifragilistic big', { maxLength: 5 }), 'super');
});

test('never returns a result ending with a dash after truncation', () => {
  const result = slugify('a bb ccc dddd', { maxLength: 4 });
  assert.strictEqual(result, 'a-bb');
  assert.ok(result.length <= 4);
  assert.ok(!result.endsWith('-'));
});

test('throws TypeError when text is not a string', () => {
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
});

test('throws RangeError for an invalid maxLength', () => {
  assert.throws(() => slugify('hello', { maxLength: 0 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: -3 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: 2.5 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: '5' }), RangeError);
});
