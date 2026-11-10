'use strict';

const test = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('normalizes case, accents, and separators', () => {
  assert.strictEqual(slugify('  Olá, Mundo!  '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
});

test('truncates at the last fitting word boundary', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
});

test('hard-cuts an overlong first word', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('preserves an exact-fit slug', () => {
  assert.strictEqual(slugify('hello-big', { maxLength: 9 }), 'hello-big');
});

test('rejects non-string text', () => {
  assert.throws(() => slugify(null), TypeError);
});
