'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('lowercases and joins words with hyphens', () => {
  assert.strictEqual(slugify('Hello World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Über café'), 'uber-cafe');
  assert.strictEqual(slugify('São Paulo'), 'sao-paulo');
});

test('collapses runs of non a-z0-9 characters into one hyphen', () => {
  assert.strictEqual(slugify('a   b__c...d'), 'a-b-c-d');
  assert.strictEqual(slugify('x!@#y'), 'x-y');
});

test('never starts or ends with a hyphen', () => {
  assert.strictEqual(slugify('  --hi--  '), 'hi');
  assert.strictEqual(slugify('!!!'), '');
  assert.strictEqual(slugify(''), '');
});

test('keeps digits', () => {
  assert.strictEqual(slugify('Version 2.0 Final'), 'version-2-0-final');
});

test('maxLength cuts at the last hyphen that fits', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
  assert.strictEqual(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
});

test('maxLength cuts hard when the first word does not fit', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('maxLength does not leave a trailing hyphen when the cut lands on one', () => {
  assert.strictEqual(slugify('ab cd', { maxLength: 2 }), 'ab');
  assert.strictEqual(slugify('ab cd', { maxLength: 3 }), 'ab');
});

test('maxLength leaves short results untouched', () => {
  assert.strictEqual(slugify('short', { maxLength: 50 }), 'short');
});

test('maxLength is optional when options are omitted', () => {
  assert.strictEqual(slugify('a b c'), 'a-b-c');
  assert.strictEqual(slugify('a b c', {}), 'a-b-c');
});

test('throws a TypeError when text is not a string', () => {
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
  assert.throws(() => slugify(['a']), TypeError);
});

test('throws a RangeError for an invalid maxLength', () => {
  assert.throws(() => slugify('abc', { maxLength: 0 }), RangeError);
  assert.throws(() => slugify('abc', { maxLength: -1 }), RangeError);
  assert.throws(() => slugify('abc', { maxLength: 2.5 }), RangeError);
});
