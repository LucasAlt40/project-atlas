const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('lowercases and joins words with a hyphen', () => {
  assert.strictEqual(slugify('Hello World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Çã Ãü'), 'ca-au');
});

test('collapses runs of other characters into a single hyphen', () => {
  assert.strictEqual(slugify('a   b---c!!!d'), 'a-b-c-d');
  assert.strictEqual(slugify('x_y.z/42'), 'x-y-z-42');
});

test('never starts or ends with a hyphen', () => {
  assert.strictEqual(slugify('---hi---'), 'hi');
  assert.strictEqual(slugify('  !!hi!!  '), 'hi');
});

test('returns an empty string when nothing is left', () => {
  assert.strictEqual(slugify('!!! ???'), '');
  assert.strictEqual(slugify(''), '');
});

test('leaves the result alone when maxLength is omitted', () => {
  assert.strictEqual(slugify('hello big world'), 'hello-big-world');
  assert.strictEqual(slugify('hello big world', {}), 'hello-big-world');
});

test('cuts at the last hyphen that fits within maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
});

test('keeps the result unchanged when it already fits', () => {
  assert.strictEqual(slugify('hello big', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big', { maxLength: 50 }), 'hello-big');
});

test('cuts a single long word hard at maxLength', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('never ends with a hyphen after truncation', () => {
  const result = slugify('ab cd', { maxLength: 3 });
  assert.strictEqual(result, 'ab');
  assert.ok(result.length <= 3);
  assert.ok(!result.endsWith('-'));
});

test('throws a TypeError when text is not a string', () => {
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
});

test('throws a RangeError when maxLength is not a positive integer', () => {
  assert.throws(() => slugify('hello', { maxLength: 0 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: -3 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: 2.5 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: '5' }), RangeError);
});
