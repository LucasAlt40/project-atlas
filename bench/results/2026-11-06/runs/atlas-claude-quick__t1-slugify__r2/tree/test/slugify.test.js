const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('matches the spec examples', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('lowercases the result', () => {
  assert.strictEqual(slugify('HeLLo WORLD'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify('café'), 'cafe');
  assert.strictEqual(slugify('ç ã ü'), 'c-a-u');
});

test('collapses runs of other characters into a single dash', () => {
  assert.strictEqual(slugify('a   b --- c!!!d'), 'a-b-c-d');
});

test('never starts or ends with a dash', () => {
  assert.strictEqual(slugify('  ...hello...  '), 'hello');
});

test('returns an empty string when no a-z or 0-9 characters remain', () => {
  assert.strictEqual(slugify(''), '');
  assert.strictEqual(slugify(' !!! '), '');
});

test('keeps digits', () => {
  assert.strictEqual(slugify('Version 2 of 10'), 'version-2-of-10');
});

test('returns the full slug when it fits maxLength exactly', () => {
  assert.strictEqual(slugify('hello big', { maxLength: 9 }), 'hello-big');
});

test('without maxLength there is no limit', () => {
  const long = 'word '.repeat(50).trim();
  assert.strictEqual(slugify(long), long.replace(/ /g, '-'));
});

test('cuts at the last dash that fits within maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 11 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
});

test('does not end with a dash when maxLength lands on a boundary', () => {
  // 'hello-big-world': index 5 is a dash, so maxLength 5 must not keep it.
  assert.strictEqual(slugify('hello big world', { maxLength: 6 }), 'hello');
});

test('cuts a single long word hard at maxLength', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('throws TypeError when text is not a string', () => {
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
});

test('throws TypeError when maxLength is not a number', () => {
  assert.throws(() => slugify('hello', { maxLength: '5' }), TypeError);
});

test('throws RangeError when maxLength is not a positive integer', () => {
  assert.throws(() => slugify('hello', { maxLength: 0 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: -1 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: 1.5 }), RangeError);
});
