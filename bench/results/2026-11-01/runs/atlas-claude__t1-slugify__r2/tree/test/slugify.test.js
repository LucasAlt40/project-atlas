const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify.js');

test('turns accented text into a lowercase, dash-separated slug', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Crème brûlée'), 'creme-brulee');
  assert.strictEqual(slugify('Coração Índio Über'), 'coracao-indio-uber');
});

test('keeps digits and lowercases ASCII letters', () => {
  assert.strictEqual(slugify('Version 2.0 RELEASE'), 'version-2-0-release');
});

test('collapses runs of other characters into a single dash', () => {
  assert.strictEqual(slugify('a!!!---b   c'), 'a-b-c');
});

test('never starts or ends with a dash', () => {
  assert.strictEqual(slugify('---x---'), 'x');
  assert.strictEqual(slugify('  -hello- '), 'hello');
});

test('returns an empty string when nothing is left', () => {
  assert.strictEqual(slugify(''), '');
  assert.strictEqual(slugify('!!! ???'), '');
});

test('turns non-ASCII letters without a decomposition into dashes', () => {
  assert.strictEqual(slugify('Straße'), 'stra-e');
});

test('keeps whole words when the text fits maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
  assert.strictEqual(slugify('hello big', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big', { maxLength: 20 }), 'hello-big');
});

test('cuts a too-long first word hard at maxLength', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('never leaves a trailing dash after a cut', () => {
  assert.strictEqual(slugify('ab cd', { maxLength: 3 }), 'ab');
  assert.strictEqual(slugify('ab cd', { maxLength: 2 }), 'ab');
});

test('throws a RangeError for an invalid maxLength', () => {
  for (const maxLength of [0, -1, 1.5, '5', NaN]) {
    assert.throws(() => slugify('hello', { maxLength }), RangeError);
  }
});

test('throws a TypeError when text is not a string', () => {
  for (const text of [42, null, undefined, ['a'], {}]) {
    assert.throws(() => slugify(text), TypeError);
  }
});
