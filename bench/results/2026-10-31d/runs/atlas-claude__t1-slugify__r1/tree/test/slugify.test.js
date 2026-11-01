const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('lowercases and joins words with a single dash', () => {
  assert.strictEqual(slugify('Hello World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify('Olá, Mundo!'), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Über Café'), 'uber-cafe');
  assert.strictEqual(slugify('São Tomé'), 'sao-tome');
});

test('letters without a diacritic decomposition become dashes', () => {
  assert.strictEqual(slugify('straße'), 'stra-e');
});

test('collapses runs of other characters into one dash', () => {
  assert.strictEqual(slugify('a  --  b!!!c'), 'a-b-c');
  assert.strictEqual(slugify('x_y.z/0 9'), 'x-y-z-0-9');
});

test('never starts or ends with a dash', () => {
  assert.strictEqual(slugify('  --hello--  '), 'hello');
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
});

test('returns an empty string when nothing is left', () => {
  assert.strictEqual(slugify('!!! ???'), '');
  assert.strictEqual(slugify(''), '');
});

test('keeps the result when it already fits maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
  assert.strictEqual(slugify('hello big world', { maxLength: 100 }), 'hello-big-world');
});

test('cuts at the last dash that fits within maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
  assert.strictEqual(slugify('hello big world', { maxLength: 6 }), 'hello');
});

test('cuts cleanly when maxLength lands on a dash', () => {
  assert.strictEqual(slugify('abc-de', { maxLength: 3 }), 'abc');
});

test('never returns a result ending with a dash', () => {
  assert.strictEqual(slugify('abc-de', { maxLength: 4 }), 'abc');
  assert.strictEqual(slugify('ab-cd-ef', { maxLength: 5 }), 'ab-cd');
});

test('cuts the first word hard when it does not fit', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('throws a TypeError when text is not a string', () => {
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
});

test('throws a TypeError when maxLength is not a number', () => {
  assert.throws(() => slugify('hello', { maxLength: '5' }), TypeError);
});

test('throws a RangeError when maxLength is not a positive integer', () => {
  assert.throws(() => slugify('hello', { maxLength: 0 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: -1 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: 2.5 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: NaN }), RangeError);
});
