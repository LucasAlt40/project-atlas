const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('exports only slugify', () => {
  assert.deepStrictEqual(Object.keys(require('../src/slugify')), ['slugify']);
});

test('lowercases the text', () => {
  assert.strictEqual(slugify('HeLLo World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify('Olá, Mundo!'), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Müller São Tomé'), 'muller-sao-tome');
});

test('collapses runs of non a-z0-9 characters into a single dash', () => {
  assert.strictEqual(slugify('a---b   c!!!d'), 'a-b-c-d');
  assert.strictEqual(slugify('Version 2.0'), 'version-2-0');
});

test('never starts or ends with a dash', () => {
  assert.strictEqual(slugify('  --hello--  '), 'hello');
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
});

test('returns an empty string for empty or punctuation-only input', () => {
  assert.strictEqual(slugify(''), '');
  assert.strictEqual(slugify(' !!! '), '');
});

test('characters without a decomposition become separators', () => {
  assert.strictEqual(slugify('straße'), 'stra-e');
});

test('leaves the slug unchanged when no maxLength is given', () => {
  assert.strictEqual(slugify('hello big world', {}), 'hello-big-world');
  assert.strictEqual(slugify('hello big world', { maxLength: undefined }), 'hello-big-world');
});

test('cuts at the dash that falls exactly on maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
});

test('cuts at the last dash that fits within maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
  assert.strictEqual(slugify('ab cd', { maxLength: 3 }), 'ab');
});

test('cuts hard at maxLength when the first word does not fit', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('returns the whole slug when it is exactly maxLength or shorter', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
  assert.strictEqual(slugify('hello', { maxLength: 5 }), 'hello');
  assert.strictEqual(slugify('hello', { maxLength: 50 }), 'hello');
});

test('never ends with a dash after cutting', () => {
  for (let maxLength = 1; maxLength <= 15; maxLength++) {
    const result = slugify('hello big world', { maxLength });
    assert.ok(result.length <= maxLength, `length for ${maxLength}`);
    assert.ok(!result.endsWith('-'), `trailing dash for ${maxLength}`);
  }
});

test('throws TypeError when text is not a string', () => {
  for (const value of [undefined, null, 42, {}, ['hello']]) {
    assert.throws(() => slugify(value), TypeError);
  }
});

test('throws RangeError when maxLength is not a positive integer', () => {
  for (const maxLength of [0, -1, 1.5, NaN, '5', null]) {
    assert.throws(() => slugify('hello', { maxLength }), RangeError);
  }
});
