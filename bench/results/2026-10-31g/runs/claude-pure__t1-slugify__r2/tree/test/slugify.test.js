const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('lowercases and joins words with hyphens', () => {
  assert.strictEqual(slugify('Hello World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('é ç ã ü'), 'e-c-a-u');
});

test('collapses runs of other characters into a single hyphen', () => {
  assert.strictEqual(slugify('a!!!b   c...d'), 'a-b-c-d');
});

test('trims leading and trailing separators', () => {
  assert.strictEqual(slugify('  Olá, Mundo!  '), 'ola-mundo');
});

test('returns an empty string when nothing is left', () => {
  assert.strictEqual(slugify('!!! ???'), '');
  assert.strictEqual(slugify(''), '');
});

test('keeps digits', () => {
  assert.strictEqual(slugify('Version 2.0 Release'), 'version-2-0-release');
});

test('cuts at the last hyphen that fits within maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
});

test('keeps the result unchanged when it already fits maxLength', () => {
  assert.strictEqual(slugify('hello big', { maxLength: 9 }), 'hello-big');
});

test('cuts a first word that does not fit hard at maxLength', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('never ends with a hyphen after truncation', () => {
  const result = slugify('one two three', { maxLength: 4 });
  assert.strictEqual(result, 'one');
  assert.ok(result.length <= 4);
  assert.ok(!result.endsWith('-'));
});

test('throws a TypeError when text is not a string', () => {
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
});
