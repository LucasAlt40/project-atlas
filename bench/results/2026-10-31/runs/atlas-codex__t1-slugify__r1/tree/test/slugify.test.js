const test = require('node:test');
const assert = require('node:assert');

const { slugify } = require('../src/slugify');

test('normalizes case and removes accents and diacritics', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Über déjà vu'), 'uber-deja-vu');
  assert.strictEqual(slugify('a\u1AB0b'), 'ab');
});

test('converts every run of non-alphanumeric characters to one separator', () => {
  assert.strictEqual(slugify('hello---world__again'), 'hello-world-again');
  assert.strictEqual(slugify('  hello, world!  '), 'hello-world');
  assert.strictEqual(slugify('***'), '');
});

test('limits length at a complete word when possible', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
  assert.strictEqual(slugify('hello big world', { maxLength: 20 }), 'hello-big-world');
});

test('hard-cuts an oversized first word', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('throws TypeError when text is not a string', () => {
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(42), TypeError);
});
