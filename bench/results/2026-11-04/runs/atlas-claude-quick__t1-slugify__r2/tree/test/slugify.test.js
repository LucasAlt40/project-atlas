const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('lowercases and joins words with a dash', () => {
  assert.strictEqual(slugify('Hello World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Güemes ãé'), 'guemes-ae');
});

test('collapses runs of non a-z0-9 characters into a single dash', () => {
  assert.strictEqual(slugify('a   b---c!!!d'), 'a-b-c-d');
  assert.strictEqual(slugify('x_y.z/1 2'), 'x-y-z-1-2');
});

test('never starts or ends with a dash', () => {
  assert.strictEqual(slugify('  --hello--  '), 'hello');
  assert.strictEqual(slugify('!!!'), '');
});

test('returns an empty string for empty input', () => {
  assert.strictEqual(slugify(''), '');
});

test('keeps the result when it fits maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
  assert.strictEqual(slugify('hello big world', { maxLength: 100 }), 'hello-big-world');
});

test('cuts at the last dash that fits within maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
});

test('cuts at a dash that ends exactly at maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 5 }), 'hello');
});

test('cuts a single long word hard at maxLength', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('never returns a result ending with a dash when truncating', () => {
  const result = slugify('a-bb-ccc-dddd', { maxLength: 4 });
  assert.strictEqual(result, 'a-bb');
  assert.ok(!result.endsWith('-'));
});

test('throws TypeError when text is not a string', () => {
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
});

test('throws TypeError when maxLength is not a positive integer', () => {
  assert.throws(() => slugify('hello', { maxLength: 0 }), TypeError);
  assert.throws(() => slugify('hello', { maxLength: -3 }), TypeError);
  assert.throws(() => slugify('hello', { maxLength: 2.5 }), TypeError);
  assert.throws(() => slugify('hello', { maxLength: '5' }), TypeError);
});
