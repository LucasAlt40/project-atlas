const assert = require('node:assert/strict');
const test = require('node:test');

const slugifyModule = require('../src/slugify');
const { slugify } = slugifyModule;

test('exports slugify as a CommonJS named export', () => {
  assert.deepStrictEqual(Object.keys(slugifyModule), ['slugify']);
  assert.strictEqual(typeof slugify, 'function');
});

test('lowercases text and removes accents and diacritics', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Crème brûlée, naïve façade'), 'creme-brulee-naive-facade');
});

test('collapses non-alphanumeric runs and trims separators', () => {
  assert.strictEqual(slugify('  hello___world -- again  '), 'hello-world-again');
  assert.strictEqual(slugify('***'), '');
});

test('truncates at a word boundary when maxLength allows it', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
});

test('hard-cuts an overlong first word', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('does not truncate without a positive integer maxLength', () => {
  assert.strictEqual(slugify('hello big world'), 'hello-big-world');
  assert.strictEqual(slugify('hello big world', { maxLength: 0 }), 'hello-big-world');
  assert.strictEqual(slugify('hello big world', { maxLength: 2.5 }), 'hello-big-world');
});

test('throws TypeError when text is not a string', () => {
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(42), TypeError);
});
