const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('lowercases and joins words with a single dash', () => {
  assert.strictEqual(slugify('Hello World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Pingüino ñandú'), 'pinguino-nandu');
});

test('collapses runs of other characters into one dash', () => {
  assert.strictEqual(slugify('a   b__c...d!!e'), 'a-b-c-d-e');
});

test('never starts or ends with a dash', () => {
  assert.strictEqual(slugify('--  hi  --'), 'hi');
  assert.strictEqual(slugify('!!!hello!!!'), 'hello');
});

test('returns an empty string when nothing is left', () => {
  assert.strictEqual(slugify(''), '');
  assert.strictEqual(slugify('  !!! '), '');
});

test('does not limit the length when maxLength is missing', () => {
  assert.strictEqual(slugify('hello big world'), 'hello-big-world');
});

test('keeps the result when it already fits maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
  assert.strictEqual(slugify('hello big world', { maxLength: 100 }), 'hello-big-world');
});

test('cuts at the last dash that fits', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 10 }), 'hello-big');
});

test('keeps a word that ends exactly at maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 5 }), 'hello');
  assert.strictEqual(slugify('hello big world', { maxLength: 11 }), 'hello-big');
});

test('cuts at a word boundary, not in the middle of a word', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
});

test('cuts the first word hard when it does not fit', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
  assert.strictEqual(slugify('supercalifragilistic big', { maxLength: 5 }), 'super');
});

test('never returns a result that ends with a dash after cutting', () => {
  assert.strictEqual(slugify('ab-cdefgh', { maxLength: 3 }), 'ab');
});

test('throws a TypeError when text is not a string', () => {
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
  assert.throws(() => slugify(['hello']), TypeError);
});

test('exports slugify as a named property', () => {
  assert.strictEqual(typeof require('../src/slugify').slugify, 'function');
});
