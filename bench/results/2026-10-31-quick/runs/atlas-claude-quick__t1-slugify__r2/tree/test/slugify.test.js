const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('lowercases and joins words with a dash', () => {
  assert.strictEqual(slugify('Hello World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Pingüino ãé'), 'pinguino-ae');
});

test('collapses runs of other characters into one dash', () => {
  assert.strictEqual(slugify('a   b---c!!!d'), 'a-b-c-d');
  assert.strictEqual(slugify('x_y.z/0 9'), 'x-y-z-0-9');
});

test('never starts or ends with a dash', () => {
  assert.strictEqual(slugify('  --hi--  '), 'hi');
  assert.strictEqual(slugify('!!!start'), 'start');
  assert.strictEqual(slugify('end!!!'), 'end');
});

test('returns an empty string when nothing usable is left', () => {
  assert.strictEqual(slugify(''), '');
  assert.strictEqual(slugify('***'), '');
});

test('throws TypeError when text is not a string', () => {
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
  assert.throws(() => slugify(['a']), TypeError);
});

test('keeps the result unchanged when it fits maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
  assert.strictEqual(slugify('hello big world', { maxLength: 100 }), 'hello-big-world');
});

test('cuts at the last dash that fits within maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
});

test('cuts at a word boundary when the dash is exactly at maxLength', () => {
  // 'hello-big' is 9 characters; the dash sits at index 9 of the full slug.
  assert.strictEqual(slugify('hello big world', { maxLength: 10 }), 'hello-big');
});

test('cuts the first word hard when it alone does not fit', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('never returns a result ending with a dash after truncation', () => {
  const result = slugify('a bb ccc dddd', { maxLength: 4 });
  assert.strictEqual(result, 'a-bb');
  assert.ok(!result.endsWith('-'));
});
