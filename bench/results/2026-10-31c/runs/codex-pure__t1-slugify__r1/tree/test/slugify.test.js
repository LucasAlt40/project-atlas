const test = require('node:test');
const assert = require('node:assert');

const { slugify } = require('../src/slugify');

test('lowercases text, removes diacritics, and normalizes separators', () => {
  assert.strictEqual(slugify('  Olá, Mundo!  '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Über déjà vu'), 'uber-deja-vu');
});

test('removes leading and trailing separators and collapses runs', () => {
  assert.strictEqual(slugify('---hello___world---'), 'hello-world');
  assert.strictEqual(slugify('***'), '');
});

test('truncates at a word boundary when maxLength allows it', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
});

test('hard-cuts an oversized first word', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('throws TypeError for non-string input', () => {
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(42), TypeError);
});
