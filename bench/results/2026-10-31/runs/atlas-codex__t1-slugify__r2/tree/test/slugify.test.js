const assert = require('node:assert/strict');
const test = require('node:test');

const { slugify } = require('../src/slugify');

test('lowercases text and removes accents and diacritics', () => {
  assert.equal(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.equal(slugify('Ação & Reação'), 'acao-reacao');
  assert.equal(slugify('über déjà vu'), 'uber-deja-vu');
  assert.equal(slugify('a\u1AB0b'), 'ab');
  assert.equal(slugify('a\u1DC0b'), 'ab');
  assert.equal(slugify('a\u20D0b'), 'ab');
});

test('collapses non-alphanumeric runs and trims hyphens', () => {
  assert.equal(slugify('---hello___world---'), 'hello-world');
  assert.equal(slugify('hello\n\tworld'), 'hello-world');
  assert.equal(slugify('***'), '');
});

test('limits length at a word boundary when possible', () => {
  assert.equal(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.equal(slugify('hello big world', { maxLength: 8 }), 'hello');
});

test('hard-cuts a first word that exceeds maxLength', () => {
  assert.equal(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('ignores non-positive and non-integer maxLength values', () => {
  assert.equal(slugify('hello world', { maxLength: 0 }), 'hello-world');
  assert.equal(slugify('hello world', { maxLength: -1 }), 'hello-world');
  assert.equal(slugify('hello world', { maxLength: 2.5 }), 'hello-world');
});

test('throws TypeError when text is not a string', () => {
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(42), TypeError);
});
