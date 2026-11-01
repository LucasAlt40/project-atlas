const assert = require('node:assert/strict');
const test = require('node:test');

const { slugify } = require('../src/slugify');

test('slugifies text with accents and punctuation', () => {
  assert.equal(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.equal(slugify('Ação & Reação'), 'acao-reacao');
});

test('removes combining marks outside the basic diacritic range', () => {
  assert.equal(slugify(`a${String.fromCodePoint(0x1ab0)}b`), 'ab');
});

test('collapses non-alphanumeric runs and trims separators', () => {
  assert.equal(slugify('  hello___world...42  '), 'hello-world-42');
  assert.equal(slugify('---Already--slug---'), 'already-slug');
});

test('truncates at the longest complete word prefix', () => {
  assert.equal(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.equal(slugify('hello big world', { maxLength: 8 }), 'hello');
});

test('hard cuts a first word that exceeds maxLength', () => {
  assert.equal(
    slugify('supercalifragilistic', { maxLength: 5 }),
    'super',
  );
});

test('returns the full slug without a length limit', () => {
  assert.equal(slugify('hello big world'), 'hello-big-world');
});

test('throws TypeError for non-string text', () => {
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(42), TypeError);
});
