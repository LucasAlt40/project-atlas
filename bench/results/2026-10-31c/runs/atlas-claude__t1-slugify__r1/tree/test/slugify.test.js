const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('exports slugify as a named function', () => {
  assert.strictEqual(typeof slugify, 'function');
});

test('lowercases the text', () => {
  assert.strictEqual(slugify('HELLO World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify('Crème Brûlée à São Paulo'), 'creme-brulee-a-sao-paulo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Olá, Mundo!'), 'ola-mundo');
  assert.strictEqual(slugify('Müller Straße'), 'muller-stra-e');
});

test('replaces each run of other characters with a single dash', () => {
  assert.strictEqual(slugify('a   b --- c!!!d'), 'a-b-c-d');
  assert.strictEqual(slugify('a_b.c/d'), 'a-b-c-d');
});

test('keeps digits', () => {
  assert.strictEqual(slugify('Version 2.0 Final'), 'version-2-0-final');
});

test('never starts or ends with a dash', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('  --hi--  '), 'hi');
});

test('returns an empty string when there are no a-z or 0-9 characters', () => {
  assert.strictEqual(slugify(''), '');
  assert.strictEqual(slugify('!!! ???'), '');
});

test('does not truncate when maxLength is not given', () => {
  assert.strictEqual(slugify('hello big world'), 'hello-big-world');
  assert.strictEqual(slugify('hello big world', {}), 'hello-big-world');
});

test('cuts at the last dash that fits within maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
  assert.strictEqual(slugify('hello big', { maxLength: 5 }), 'hello');
});

test('keeps the whole result when it fits exactly', () => {
  assert.strictEqual(slugify('hello big', { maxLength: 9 }), 'hello-big');
});

test('hard cuts a single word that does not fit', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
  assert.strictEqual(slugify('hello big world', { maxLength: 3 }), 'hel');
});

test('result is within maxLength and never ends with a dash for every limit', () => {
  const input = 'a bb ccc dddd eeeee ffffff';
  for (let maxLength = 1; maxLength <= 30; maxLength++) {
    const result = slugify(input, { maxLength });
    assert.ok(result.length <= maxLength, `maxLength ${maxLength}: "${result}"`);
    assert.ok(!result.endsWith('-'), `maxLength ${maxLength}: "${result}"`);
    assert.ok(!result.startsWith('-'), `maxLength ${maxLength}: "${result}"`);
  }
});

test('throws a TypeError when text is not a string', () => {
  for (const value of [undefined, null, 42, ['a'], { toString: () => 'a' }]) {
    assert.throws(() => slugify(value), TypeError);
  }
});

test('throws a TypeError when maxLength is not an integer', () => {
  for (const maxLength of ['5', 1.5, NaN]) {
    assert.throws(() => slugify('hello', { maxLength }), TypeError);
  }
});

test('throws a RangeError when maxLength is below 1', () => {
  for (const maxLength of [0, -3]) {
    assert.throws(() => slugify('hello', { maxLength }), RangeError);
  }
});
