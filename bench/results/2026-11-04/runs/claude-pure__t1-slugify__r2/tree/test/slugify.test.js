const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('lowercases and joins words with a single dash', () => {
  assert.strictEqual(slugify('Hello World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify('é'), 'e');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Müller Ção'), 'muller-cao');
});

test('collapses runs of non a-z0-9 characters into one dash', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('a---b   c!!!d'), 'a-b-c-d');
});

test('never starts or ends with a dash', () => {
  assert.strictEqual(slugify('  --hello--  '), 'hello');
  assert.strictEqual(slugify('!!!'), '');
  assert.strictEqual(slugify(''), '');
});

test('keeps digits', () => {
  assert.strictEqual(slugify('Version 2.0 Final'), 'version-2-0-final');
});

test('throws TypeError when text is not a string', () => {
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
  assert.throws(() => slugify(['a']), TypeError);
});

test('maxLength cuts at the last dash that fits', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
});

test('maxLength keeps a word ending exactly at the limit', () => {
  assert.strictEqual(slugify('hello big', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big', { maxLength: 5 }), 'hello');
});

test('maxLength cuts hard when the first word does not fit', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('maxLength result never ends with a dash', () => {
  const result = slugify('a bb ccc dddd', { maxLength: 7 });
  assert.strictEqual(result, 'a-bb');
  assert.ok(result.length <= 7);
  assert.ok(!result.endsWith('-'));
});

test('maxLength leaves short results unchanged', () => {
  assert.strictEqual(slugify('ola', { maxLength: 10 }), 'ola');
  assert.strictEqual(slugify('ola', { maxLength: 3 }), 'ola');
});

test('throws RangeError for an invalid maxLength', () => {
  assert.throws(() => slugify('hello', { maxLength: 0 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: -1 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: 2.5 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: '5' }), RangeError);
});
