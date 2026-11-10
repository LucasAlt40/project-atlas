'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('lowercases and joins words with a single dash', () => {
  assert.strictEqual(slugify('Hello World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Über Façade'), 'uber-facade');
});

test('collapses runs of non a-z0-9 characters into one dash', () => {
  assert.strictEqual(slugify('a  --  b...c!!!d'), 'a-b-c-d');
  assert.strictEqual(slugify('x_y/z'), 'x-y-z');
});

test('never starts or ends with a dash', () => {
  assert.strictEqual(slugify('  --hi there--  '), 'hi-there');
  assert.strictEqual(slugify('!!!'), '');
  assert.strictEqual(slugify(''), '');
});

test('treats non-decomposing letters as separators', () => {
  assert.strictEqual(slugify('straße'), 'stra-e');
  assert.strictEqual(slugify('Ørsted'), 'rsted');
});

test('leaves text unchanged when it fits within maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 20 }), 'hello-big-world');
  assert.strictEqual(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
});

test('cuts at the last word boundary that fits', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
});

test('does not end with a dash when cut lands on a boundary', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 10 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 6 }), 'hello');
});

test('hard-cuts the first word when it alone does not fit', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
  assert.strictEqual(slugify('abcdefgh-ij', { maxLength: 4 }), 'abcd');
});

test('result never exceeds maxLength', () => {
  for (let max = 1; max <= 25; max++) {
    const result = slugify('The quick brown fox jumps over', { maxLength: max });
    assert.ok(result.length <= max, `length ${result.length} exceeds ${max}`);
    assert.ok(!result.endsWith('-'), `"${result}" ends with a dash`);
  }
});

test('undefined maxLength means no limit', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: undefined }), 'hello-big-world');
  assert.strictEqual(slugify('hello big world'), 'hello-big-world');
});

test('throws TypeError when text is not a string', () => {
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
});

test('throws TypeError when maxLength is not a positive integer', () => {
  for (const maxLength of [0, -1, 2.5, '5', NaN, null]) {
    assert.throws(() => slugify('hello', { maxLength }), TypeError, `maxLength ${maxLength}`);
  }
});
