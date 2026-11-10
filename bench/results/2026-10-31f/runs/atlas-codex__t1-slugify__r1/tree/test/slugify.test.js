const assert = require('node:assert/strict');
const test = require('node:test');

const { slugify } = require('../src/slugify');

test('lowercases text and removes accents and diacritics', () => {
  assert.equal(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.equal(slugify('Ação & Reação'), 'acao-reacao');
  assert.equal(slugify('Über façade déjà vu'), 'uber-facade-deja-vu');
});

test('turns every run of non-alphanumeric characters into one separator', () => {
  assert.equal(slugify('one---two __ three'), 'one-two-three');
  assert.equal(slugify('***hello***'), 'hello');
});

test('truncates at the last separator that fits', () => {
  assert.equal(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.equal(slugify('hello big world', { maxLength: 8 }), 'hello');
});

test('hard-cuts a first word that exceeds maxLength', () => {
  assert.equal(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('does not truncate when maxLength is not exceeded', () => {
  assert.equal(slugify('hello world', { maxLength: 11 }), 'hello-world');
  assert.equal(slugify('hello world'), 'hello-world');
});

test('throws TypeError for non-string text', () => {
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(42), TypeError);
});
