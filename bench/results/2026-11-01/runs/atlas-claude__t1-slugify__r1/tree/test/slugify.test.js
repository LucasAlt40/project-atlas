const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('lowercases and joins words with dashes', () => {
  assert.strictEqual(slugify('Hello World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Über façade ãs'), 'uber-facade-as');
});

test('collapses runs of non a-z0-9 characters into one dash', () => {
  assert.strictEqual(slugify('a   b---c!!!d'), 'a-b-c-d');
  assert.strictEqual(slugify('x_y.z/w'), 'x-y-z-w');
});

test('never starts or ends with a dash', () => {
  assert.strictEqual(slugify('  ¡¿hola!?  '), 'hola');
  assert.strictEqual(slugify('---'), '');
  assert.strictEqual(slugify(''), '');
});

test('keeps digits', () => {
  assert.strictEqual(slugify('Version 2.0 Final'), 'version-2-0-final');
});

test('returns whole slug when it fits maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
  assert.strictEqual(slugify('hello big world', { maxLength: 100 }), 'hello-big-world');
});

test('cuts at the last dash that fits maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
});

test('cuts hard when the first word does not fit maxLength', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('never leaves a trailing dash after truncation', () => {
  // The cut lands right after a word, so the character at maxLength is a dash.
  assert.strictEqual(slugify('ab cd', { maxLength: 2 }), 'ab');
  // The cut lands right on a dash inside the head.
  assert.strictEqual(slugify('ab cd', { maxLength: 3 }), 'ab');
  assert.strictEqual(slugify('a-b-c', { maxLength: 4 }), 'a-b');
});

test('throws TypeError when text is not a string', () => {
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
});

test('throws RangeError when maxLength is not a positive integer', () => {
  assert.throws(() => slugify('abc', { maxLength: 0 }), RangeError);
  assert.throws(() => slugify('abc', { maxLength: -1 }), RangeError);
  assert.throws(() => slugify('abc', { maxLength: 1.5 }), RangeError);
  assert.throws(() => slugify('abc', { maxLength: '5' }), RangeError);
});
