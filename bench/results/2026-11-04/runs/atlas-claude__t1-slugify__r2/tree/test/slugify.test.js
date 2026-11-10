const { test } = require('node:test');
const assert = require('node:assert/strict');
const { slugify } = require('../src/slugify');

test('lowercases and joins words with a single dash', () => {
  assert.equal(slugify('Hello World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.equal(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.equal(slugify('Ação & Reação'), 'acao-reacao');
  assert.equal(slugify('Über Café'), 'uber-cafe');
});

test('collapses runs of non a-z0-9 characters into one dash', () => {
  assert.equal(slugify('a -- b___c!!!d'), 'a-b-c-d');
});

test('never starts or ends with a dash', () => {
  assert.equal(slugify('  --hello--  '), 'hello');
  assert.equal(slugify('!!!'), '');
});

test('returns an empty string for empty input', () => {
  assert.equal(slugify(''), '');
});

test('returns text unchanged when it is within maxLength', () => {
  assert.equal(slugify('hello big', { maxLength: 9 }), 'hello-big');
  assert.equal(slugify('hello big', { maxLength: 20 }), 'hello-big');
});

test('cuts at the last dash that fits', () => {
  assert.equal(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.equal(slugify('hello big world', { maxLength: 8 }), 'hello');
});

test('cuts hard when the first word does not fit', () => {
  assert.equal(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('never returns a result ending with a dash', () => {
  for (let max = 1; max <= 20; max += 1) {
    const result = slugify('a bb ccc dddd eeeee', { maxLength: max });
    assert.ok(result.length <= max, `length ${result.length} > ${max}`);
    assert.ok(!result.endsWith('-'), `"${result}" ends with a dash`);
  }
});

test('accepts null or undefined options', () => {
  assert.equal(slugify('Hello World', null), 'hello-world');
  assert.equal(slugify('Hello World', undefined), 'hello-world');
});

test('throws TypeError when text is not a string', () => {
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
});

test('throws RangeError when maxLength is not a positive integer', () => {
  assert.throws(() => slugify('hello', { maxLength: 0 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: -3 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: 2.5 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: '5' }), RangeError);
});
