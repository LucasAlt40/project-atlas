const test = require('node:test');
const assert = require('node:assert');

const { slugify } = require('../src/slugify');

test('lowercases text, removes diacritics, and replaces separators', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('a\u1AB0b'), 'ab');
});

test('does not leave leading or trailing separators', () => {
  assert.strictEqual(slugify('---Hello---world---'), 'hello-world');
});

test('truncates at a word boundary when maxLength allows it', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
});

test('cuts the first word hard when it exceeds maxLength', () => {
  assert.strictEqual(
    slugify('supercalifragilistic', { maxLength: 5 }),
    'super',
  );
});

test('throws a TypeError when text is not a string', () => {
  assert.throws(() => slugify(null), TypeError);
});
