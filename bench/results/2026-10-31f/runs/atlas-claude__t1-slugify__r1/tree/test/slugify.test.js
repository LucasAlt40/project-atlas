'use strict';

const test = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('exports only slugify', () => {
  assert.deepStrictEqual(Object.keys(require('../src/slugify')), ['slugify']);
});

test('converts examples from the spec', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
});

test('lowercases the result', () => {
  assert.strictEqual(slugify('HeLLo World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify('é'), 'e');
  assert.strictEqual(slugify('ç'), 'c');
  assert.strictEqual(slugify('ã'), 'a');
  assert.strictEqual(slugify('ü'), 'u');
  assert.strictEqual(slugify('Crème Brûlée'), 'creme-brulee');
});

test('collapses each run of other characters into one dash', () => {
  assert.strictEqual(slugify('a   b--c_d'), 'a-b-c-d');
  assert.strictEqual(slugify('one, two; three!'), 'one-two-three');
});

test('keeps digits', () => {
  assert.strictEqual(slugify('Version 2 of 10'), 'version-2-of-10');
});

test('never starts or ends with a dash', () => {
  assert.strictEqual(slugify('  --hello--  '), 'hello');
  assert.strictEqual(slugify('!hello!'), 'hello');
});

test('returns an empty string when nothing is left', () => {
  assert.strictEqual(slugify(''), '');
  assert.strictEqual(slugify('!!! ---'), '');
});

test('returns the full slug when it fits maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
  assert.strictEqual(slugify('hello big world', { maxLength: 100 }), 'hello-big-world');
});

test('cuts at the last dash that fits when the cut is inside a word', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
  assert.strictEqual(slugify('hello big world', { maxLength: 6 }), 'hello');
});

test('keeps the whole word when the cut lands on a dash', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 5 }), 'hello');
});

test('cuts the first word hard when it does not fit', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('result never exceeds maxLength and never ends with a dash', () => {
  const result = slugify('Ação & Reação, então vamos lá', { maxLength: 12 });
  assert.ok(result.length <= 12);
  assert.ok(!result.endsWith('-'));
  assert.strictEqual(result, 'acao-reacao');
});

test('ignores maxLength when it is undefined', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: undefined }), 'hello-big-world');
});

test('throws TypeError when text is not a string', () => {
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(['hello']), TypeError);
});

test('throws RangeError when maxLength is not a positive integer', () => {
  assert.throws(() => slugify('hello', { maxLength: 0 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: -1 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: 1.5 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: NaN }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: '5' }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: null }), RangeError);
});
