'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('lowercases and joins words with dashes', () => {
  assert.strictEqual(slugify('Hello World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Über Müller'), 'uber-muller');
});

test('collapses runs of non a-z0-9 characters into one dash', () => {
  assert.strictEqual(slugify('a   b---c!!!d'), 'a-b-c-d');
  assert.strictEqual(slugify('v1.2 beta'), 'v1-2-beta');
});

test('never starts or ends with a dash', () => {
  assert.strictEqual(slugify('  --hi--  '), 'hi');
  assert.strictEqual(slugify('!!!'), '');
  assert.strictEqual(slugify(''), '');
});

test('replaces letters that do not decompose with a dash', () => {
  assert.strictEqual(slugify('Straße'), 'stra-e');
});

test('returns the full slug when it fits maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
  assert.strictEqual(slugify('hello big world', { maxLength: 100 }), 'hello-big-world');
});

test('cuts at the last word boundary that fits maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
});

test('cuts the first word hard when it does not fit maxLength', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
  assert.strictEqual(slugify('abcdef-gh', { maxLength: 1 }), 'a');
});

test('result never exceeds maxLength and never ends with a dash', () => {
  const samples = ['hello big world', 'Ação & Reação', 'a-b-c-d-e-f', 'supercalifragilistic'];
  for (const text of samples) {
    for (let maxLength = 1; maxLength <= 20; maxLength++) {
      const result = slugify(text, { maxLength });
      assert.ok(result.length <= maxLength, `${text} @${maxLength} -> ${result}`);
      assert.ok(!result.endsWith('-'), `${text} @${maxLength} -> ${result}`);
    }
  }
});

test('throws TypeError when text is not a string', () => {
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
});

test('throws TypeError when maxLength is not a positive integer', () => {
  for (const maxLength of [0, -1, 1.5, '5', null, NaN]) {
    assert.throws(() => slugify('hello world', { maxLength }), TypeError, `maxLength=${maxLength}`);
  }
});
