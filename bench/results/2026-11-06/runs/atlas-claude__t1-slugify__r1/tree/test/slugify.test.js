const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify.js');

test('lowercases and removes accents and diacritics', () => {
  assert.strictEqual(slugify('ÉCOLE Çà Ãü'), 'ecole-ca-au');
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
});

test('handles uppercase dotted İ without leaving a combining mark', () => {
  assert.strictEqual(slugify('İstanbul'), 'istanbul');
});

test('collapses runs of other characters into one dash and trims the ends', () => {
  assert.strictEqual(slugify('a   b---c__d'), 'a-b-c-d');
  assert.strictEqual(slugify('--Hello, World!!--'), 'hello-world');
});

test('returns an empty string when nothing is left', () => {
  assert.strictEqual(slugify(''), '');
  assert.strictEqual(slugify('!!! ???'), '');
});

test('leaves a result untouched when maxLength is undefined or fits', () => {
  assert.strictEqual(slugify('hello big world'), 'hello-big-world');
  assert.strictEqual(slugify('hello big world', { maxLength: undefined }), 'hello-big-world');
  assert.strictEqual(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
});

test('cuts at the last dash that fits within maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
  assert.strictEqual(slugify('hello big world', { maxLength: 14 }), 'hello-big');
});

test('cuts the first word hard when it does not fit', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('never returns a result ending with a dash', () => {
  const result = slugify('ab cd ef', { maxLength: 3 });
  assert.strictEqual(result, 'ab');
  assert.ok(!result.endsWith('-'));
});

test('throws TypeError when text is not a string', () => {
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
});

test('throws RangeError when maxLength is not a positive integer', () => {
  assert.throws(() => slugify('hello', { maxLength: 0 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: -1 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: 2.5 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: NaN }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: '5' }), RangeError);
});
