'use strict';

const { test } = require('node:test');
const assert = require('node:assert/strict');
const { slugify } = require('../src/slugify');

test('lowercases and joins words with a single dash', () => {
  assert.equal(slugify('Hello World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.equal(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.equal(slugify('Ação & Reação'), 'acao-reacao');
  assert.equal(slugify('Über Café'), 'uber-cafe');
});

test('collapses runs of non a-z0-9 characters into one dash', () => {
  assert.equal(slugify('a   b---c!!!d'), 'a-b-c-d');
  assert.equal(slugify('one_two.three/four'), 'one-two-three-four');
});

test('never starts or ends with a dash', () => {
  assert.equal(slugify('  --hello--  '), 'hello');
  assert.equal(slugify('!!!hi???'), 'hi');
});

test('returns an empty string when nothing is left', () => {
  assert.equal(slugify(''), '');
  assert.equal(slugify('!!! ???'), '');
});

test('characters with no a-z decomposition become dashes', () => {
  assert.equal(slugify('straße'), 'stra-e');
});

test('maxLength cuts at the last dash that fits', () => {
  assert.equal(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.equal(slugify('hello big world', { maxLength: 8 }), 'hello');
  assert.equal(slugify('abc-d', { maxLength: 3 }), 'abc');
});

test('maxLength returns the whole slug when it fits', () => {
  assert.equal(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
  assert.equal(slugify('hello', { maxLength: 5 }), 'hello');
});

test('maxLength hard-cuts when the first word does not fit', () => {
  assert.equal(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('maxLength result never ends with a dash', () => {
  for (let max = 1; max <= 15; max += 1) {
    const result = slugify('hello big world', { maxLength: max });
    assert.ok(result.length <= max, `length ${result.length} exceeds ${max}`);
    assert.ok(!result.endsWith('-'), `"${result}" ends with a dash`);
  }
});

test('throws TypeError when text is not a string', () => {
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
});

test('throws RangeError when maxLength is not a positive integer', () => {
  assert.throws(() => slugify('hello', { maxLength: 0 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: -3 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: 2.5 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: '5' }), RangeError);
});
