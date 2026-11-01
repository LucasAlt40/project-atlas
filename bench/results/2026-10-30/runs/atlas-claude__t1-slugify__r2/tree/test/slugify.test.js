const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify.js');

test('lowercases and joins words with a single dash', () => {
  assert.strictEqual(slugify('Hello World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify('  Olá, Mundo!  '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Güntér ã'), 'gunter-a');
});

test('collapses runs of non a-z0-9 characters into one dash', () => {
  assert.strictEqual(slugify('a   --  b!!!c'), 'a-b-c');
  assert.strictEqual(slugify('x_y.z/w'), 'x-y-z-w');
});

test('never starts or ends with a dash', () => {
  assert.strictEqual(slugify('---hello---'), 'hello');
  assert.strictEqual(slugify('!!!'), '');
  assert.strictEqual(slugify(''), '');
});

test('keeps digits', () => {
  assert.strictEqual(slugify('Version 2 of 10'), 'version-2-of-10');
});

test('maxLength cuts at the last dash that fits', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
});

test('maxLength keeps the result when the cut lands on a dash', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 5 }), 'hello');
  assert.strictEqual(slugify('ab-cd', { maxLength: 2 }), 'ab');
});

test('maxLength hard-cuts a single word that does not fit', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('maxLength has no effect when the slug already fits', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
  assert.strictEqual(slugify('hello', { maxLength: 100 }), 'hello');
});

test('maxLength result never ends with a dash', () => {
  for (let max = 1; max <= 15; max++) {
    const result = slugify('hello big world', { maxLength: max });
    assert.ok(result.length <= max, `length ${result.length} > ${max}`);
    assert.ok(!result.endsWith('-'), `"${result}" ends with a dash`);
  }
});

test('throws TypeError when text is not a string', () => {
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
});

test('throws RangeError when maxLength is not a positive integer', () => {
  assert.throws(() => slugify('hello', { maxLength: 0 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: -3 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: 2.5 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: '5' }), RangeError);
});
