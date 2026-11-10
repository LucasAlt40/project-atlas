const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('lowercases and joins words with a single dash', () => {
  assert.strictEqual(slugify('Hello World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Über Café ãõ'), 'uber-cafe-ao');
});

test('collapses runs of non a-z0-9 characters into one dash', () => {
  assert.strictEqual(slugify('a   b---c!!!d'), 'a-b-c-d');
  assert.strictEqual(slugify('a_b.c/d'), 'a-b-c-d');
});

test('never starts or ends with a dash', () => {
  assert.strictEqual(slugify('  --hi--  '), 'hi');
  assert.strictEqual(slugify('!!!'), '');
  assert.strictEqual(slugify(''), '');
});

test('keeps digits', () => {
  assert.strictEqual(slugify('Version 2.0 Release'), 'version-2-0-release');
});

test('maxLength cuts at the last dash that fits', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
  assert.strictEqual(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
  assert.strictEqual(slugify('hello big world', { maxLength: 100 }), 'hello-big-world');
});

test('maxLength cuts a single long word hard', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('maxLength result never ends with a dash', () => {
  const result = slugify('ab cd', { maxLength: 3 });
  assert.strictEqual(result, 'ab');
  assert.ok(result.length <= 3);
  assert.ok(!result.endsWith('-'));
});

test('null or missing options mean no length limit', () => {
  assert.strictEqual(slugify('hello big world', null), 'hello-big-world');
  assert.strictEqual(slugify('hello big world', {}), 'hello-big-world');
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
