'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const { slugify } = require('../src/slugify');

test('normalizes case, accents, and separators', () => {
  assert.equal(slugify('  Olá, Mundo!  '), 'ola-mundo');
  assert.equal(slugify('Ação & Reação'), 'acao-reacao');
  assert.equal(slugify('Über déjà vu'), 'uber-deja-vu');
});

test('removes leading and trailing separators and collapses runs', () => {
  assert.equal(slugify('---hello___world---'), 'hello-world');
  assert.equal(slugify('***'), '');
});

test('limits length at a word boundary', () => {
  assert.equal(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.equal(slugify('hello big world', { maxLength: 8 }), 'hello');
});

test('hard-cuts an overlong first word', () => {
  assert.equal(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('ignores non-positive or non-integer maxLength values', () => {
  assert.equal(slugify('hello world', { maxLength: 0 }), 'hello-world');
  assert.equal(slugify('hello world', { maxLength: 1.5 }), 'hello-world');
});

test('throws TypeError for non-string text', () => {
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(123), TypeError);
});
