const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('lowercases and joins words with a single dash', () => {
  assert.strictEqual(slugify('Hello World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Über Café Ünïcode'), 'uber-cafe-unicode');
});

test('collapses runs of non a-z0-9 characters into one dash', () => {
  assert.strictEqual(slugify('a   b__c...d'), 'a-b-c-d');
  assert.strictEqual(slugify('foo!@#$%bar'), 'foo-bar');
});

test('keeps digits', () => {
  assert.strictEqual(slugify('Release 2.0 Notes'), 'release-2-0-notes');
});

test('never starts or ends with a dash', () => {
  assert.strictEqual(slugify('---hi---'), 'hi');
  assert.strictEqual(slugify('  !!! '), '');
});

test('returns an empty string when nothing is left', () => {
  assert.strictEqual(slugify(''), '');
  assert.strictEqual(slugify('日本語'), '');
});

test('maxLength cuts at the last dash that fits', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
  assert.strictEqual(slugify('hello big world', { maxLength: 10 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
});

test('maxLength returns the slug unchanged when it already fits', () => {
  assert.strictEqual(slugify('hello', { maxLength: 5 }), 'hello');
  assert.strictEqual(slugify('hello', { maxLength: 50 }), 'hello');
});

test('maxLength hard-cuts when the first word does not fit', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('maxLength result never ends with a dash', () => {
  const result = slugify('ab cd ef', { maxLength: 3 });
  assert.strictEqual(result, 'ab');
  assert.ok(result.length <= 3);
  assert.ok(!result.endsWith('-'));
});

test('options are optional', () => {
  assert.strictEqual(slugify('Hello World', undefined), 'hello-world');
  assert.strictEqual(slugify('Hello World', {}), 'hello-world');
});

test('throws a TypeError when text is not a string', () => {
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify({ toString: () => 'x' }), TypeError);
});
