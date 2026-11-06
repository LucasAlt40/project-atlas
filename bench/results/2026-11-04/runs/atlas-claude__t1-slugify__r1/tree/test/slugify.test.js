const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('lowercases and joins words with dashes', () => {
  assert.strictEqual(slugify('Hello World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Über Café Ãé'), 'uber-cafe-ae');
});

test('collapses runs of other characters into a single dash', () => {
  assert.strictEqual(slugify('a   b---c!!!d'), 'a-b-c-d');
  assert.strictEqual(slugify('x_y.z/1 2'), 'x-y-z-1-2');
});

test('never starts or ends with a dash', () => {
  assert.strictEqual(slugify('  --hello--  '), 'hello');
  assert.strictEqual(slugify('!!!start and end???'), 'start-and-end');
});

test('returns an empty string when there are no a-z or 0-9 characters', () => {
  assert.strictEqual(slugify(''), '');
  assert.strictEqual(slugify('!!! ???'), '');
});

test('non-decomposable letters become dashes', () => {
  assert.strictEqual(slugify('Søren'), 's-ren');
});

test('returns the slug unchanged when it fits maxLength', () => {
  assert.strictEqual(slugify('hello big', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello', { maxLength: 5 }), 'hello');
});

test('cuts at the last dash that fits within maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
});

test('cuts hard when the first word does not fit', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('does not end with a dash after a cut', () => {
  assert.strictEqual(slugify('ab cd', { maxLength: 3 }), 'ab');
  assert.strictEqual(slugify('ab-cd', { maxLength: 2 }), 'ab');
});

test('throws a TypeError when text is not a string', () => {
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
});

test('throws a RangeError when maxLength is not a positive integer', () => {
  for (const maxLength of [0, -1, 1.5, '5', NaN, null]) {
    assert.throws(() => slugify('hello', { maxLength }), RangeError);
  }
});
