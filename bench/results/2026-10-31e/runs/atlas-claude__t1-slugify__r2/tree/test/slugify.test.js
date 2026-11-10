const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('lowercases and joins words with a single dash', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Hello   World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Über café, señor'), 'uber-cafe-senor');
  assert.strictEqual(slugify('ÇÃÜÉ'), 'caue');
});

test('collapses runs of other characters into one dash', () => {
  assert.strictEqual(slugify('a!!!b???c'), 'a-b-c');
  assert.strictEqual(slugify('a - _ - b'), 'a-b');
});

test('never starts or ends with a dash', () => {
  assert.strictEqual(slugify('---hi---'), 'hi');
  assert.strictEqual(slugify('  !!hi!!  '), 'hi');
});

test('returns an empty string when nothing is left', () => {
  assert.strictEqual(slugify(''), '');
  assert.strictEqual(slugify('!!! ???'), '');
  assert.strictEqual(slugify('日本語'), '');
});

test('keeps digits', () => {
  assert.strictEqual(slugify('Version 2.0 Final'), 'version-2-0-final');
});

test('maxLength cuts at the last word that fits', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
  assert.strictEqual(slugify('hello big world', { maxLength: 10 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 11 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
});

test('maxLength cuts a first word that is too long hard', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
  assert.strictEqual(slugify('supercalifragilistic big', { maxLength: 5 }), 'super');
});

test('maxLength never leaves a trailing dash', () => {
  const result = slugify('one two three', { maxLength: 4 });
  assert.strictEqual(result, 'one');
  assert.ok(!result.endsWith('-'));
});

test('maxLength has no effect when the slug already fits', () => {
  assert.strictEqual(slugify('short', { maxLength: 5 }), 'short');
  assert.strictEqual(slugify('short', { maxLength: 100 }), 'short');
});

test('throws TypeError when text is not a string', () => {
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
});

test('throws RangeError for an invalid maxLength', () => {
  assert.throws(() => slugify('abc', { maxLength: 0 }), RangeError);
  assert.throws(() => slugify('abc', { maxLength: -1 }), RangeError);
  assert.throws(() => slugify('abc', { maxLength: 1.5 }), RangeError);
  assert.throws(() => slugify('abc', { maxLength: '5' }), RangeError);
});
