const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('lowercases and joins words with a single dash', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Hello World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Über Café Mãe'), 'uber-cafe-mae');
});

test('collapses runs of non a-z0-9 characters into one dash', () => {
  assert.strictEqual(slugify('a---b   c!!!d'), 'a-b-c-d');
  assert.strictEqual(slugify('x_y.z/w'), 'x-y-z-w');
});

test('never starts or ends with a dash', () => {
  assert.strictEqual(slugify('  --hello--  '), 'hello');
  assert.strictEqual(slugify('!!!'), '');
});

test('keeps digits', () => {
  assert.strictEqual(slugify('Version 2.0 Final'), 'version-2-0-final');
});

test('maxLength cuts at the last dash that fits', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
  assert.strictEqual(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
});

test('maxLength hard-cuts when the first word does not fit', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('maxLength result never ends with a dash', () => {
  assert.strictEqual(slugify('ab cd', { maxLength: 3 }), 'ab');
  assert.strictEqual(slugify('ab cd', { maxLength: 2 }), 'ab');
});

test('returns the whole slug when it is within maxLength', () => {
  assert.strictEqual(slugify('hello world', { maxLength: 11 }), 'hello-world');
  assert.strictEqual(slugify('hello world', { maxLength: 50 }), 'hello-world');
});

test('empty options and empty text', () => {
  assert.strictEqual(slugify(''), '');
  assert.strictEqual(slugify('abc', {}), 'abc');
});

test('throws TypeError when text is not a string', () => {
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
});
