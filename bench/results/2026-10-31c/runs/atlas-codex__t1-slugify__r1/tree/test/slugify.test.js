const assert = require('node:assert/strict');
const test = require('node:test');

const { slugify } = require('../src/slugify');

test('normalizes accents and separators', () => {
  assert.equal(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.equal(slugify('Ação & Reação'), 'acao-reacao');
  assert.equal(slugify('ÜBER déjà vu'), 'uber-deja-vu');
  assert.equal(slugify('hello___world'), 'hello-world');
  assert.equal(slugify('Room 42'), 'room-42');
});

test('returns an empty slug for text without ASCII letters or digits', () => {
  assert.equal(slugify(' !!! '), '');
});

test('rejects non-string text', () => {
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(42), TypeError);
});

test('limits length at a word boundary', () => {
  assert.equal(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.equal(slugify('hello big world', { maxLength: 8 }), 'hello');
  assert.equal(slugify('hello big world', { maxLength: 5 }), 'hello');
});

test('hard-cuts a first word that exceeds maxLength', () => {
  assert.equal(
    slugify('supercalifragilistic', { maxLength: 5 }),
    'super',
  );
});
