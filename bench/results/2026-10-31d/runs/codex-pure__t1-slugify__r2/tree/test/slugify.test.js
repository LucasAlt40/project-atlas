const test = require('node:test');
const assert = require('node:assert');

const { slugify } = require('../src/slugify');

test('lowercases text, removes accents, and normalizes separators', () => {
  assert.strictEqual(slugify('  Olá, Mundo!  '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('ÜBER déjà vu'), 'uber-deja-vu');
});

test('collapses non-alphanumeric runs and trims hyphens', () => {
  assert.strictEqual(slugify('---hello___world---'), 'hello-world');
  assert.strictEqual(slugify('one\u0000two'), 'one-two');
  assert.strictEqual(slugify('***'), '');
});

test('limits length at a word boundary when possible', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
  assert.strictEqual(slugify('hello big', { maxLength: 5 }), 'hello');
  assert.strictEqual(slugify('hello big', { maxLength: 1 }), 'h');
});

test('does not apply a non-positive or non-integer maxLength', () => {
  assert.strictEqual(slugify('hello world', { maxLength: 0 }), 'hello-world');
  assert.strictEqual(slugify('hello world', { maxLength: 2.5 }), 'hello-world');
});

test('throws TypeError when text is not a string', () => {
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(123), TypeError);
});
