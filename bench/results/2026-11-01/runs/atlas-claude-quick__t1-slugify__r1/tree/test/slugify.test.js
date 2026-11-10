const test = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('lowercases and joins words with hyphens', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Hello World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Über café'), 'uber-cafe');
  assert.strictEqual(slugify('São Paulo'), 'sao-paulo');
});

test('collapses runs of other characters into a single hyphen', () => {
  assert.strictEqual(slugify('a   b---c!!!d'), 'a-b-c-d');
  assert.strictEqual(slugify('foo_bar.baz/qux'), 'foo-bar-baz-qux');
});

test('never starts or ends with a hyphen', () => {
  assert.strictEqual(slugify('  --hello--  '), 'hello');
  assert.strictEqual(slugify('!!!'), '');
  assert.strictEqual(slugify(''), '');
});

test('keeps digits', () => {
  assert.strictEqual(slugify('Version 2.0 beta'), 'version-2-0-beta');
});

test('maxLength cuts at the last hyphen that fits', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
  assert.strictEqual(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
});

test('maxLength cuts hard when the first word does not fit', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
  assert.strictEqual(slugify('supercalifragilistic big', { maxLength: 5 }), 'super');
});

test('maxLength result never exceeds the limit or ends with a hyphen', () => {
  const result = slugify('one two three four five', { maxLength: 10 });
  assert.ok(result.length <= 10);
  assert.ok(!result.endsWith('-'));
  assert.strictEqual(result, 'one-two');
});

test('maxLength leaves short slugs unchanged', () => {
  assert.strictEqual(slugify('ola mundo', { maxLength: 50 }), 'ola-mundo');
});

test('throws a TypeError when text is not a string', () => {
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
  assert.throws(() => slugify(['a']), TypeError);
});
