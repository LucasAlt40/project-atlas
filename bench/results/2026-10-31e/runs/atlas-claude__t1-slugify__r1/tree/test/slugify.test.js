const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('lowercases and strips accents and diacritics', () => {
  assert.strictEqual(slugify('Olá'), 'ola');
  assert.strictEqual(slugify('Ação'), 'acao');
  assert.strictEqual(slugify('Pingüino'), 'pinguino');
  assert.strictEqual(slugify('ÀÉÎÕÜ'), 'aeiou');
});

test('replaces each run of non a-z0-9 characters with a single dash', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('a   --  b'), 'a-b');
  assert.strictEqual(slugify('a_b.c/d'), 'a-b-c-d');
});

test('keeps digits', () => {
  assert.strictEqual(slugify('Version 2 of 10'), 'version-2-of-10');
});

test('never starts or ends with a dash', () => {
  assert.strictEqual(slugify('---hello---'), 'hello');
  assert.strictEqual(slugify('  !!hi!!  '), 'hi');
});

test('returns an empty string when nothing is left', () => {
  assert.strictEqual(slugify(''), '');
  assert.strictEqual(slugify('!!! ???'), '');
});

test('letters without a decomposition become dashes', () => {
  assert.strictEqual(slugify('Straße'), 'stra-e');
  assert.strictEqual(slugify('Ørsted'), 'rsted');
});

test('maxLength cuts at the last dash that fits', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
  assert.strictEqual(slugify('hello big world', { maxLength: 11 }), 'hello-big');
});

test('maxLength keeps the full slug when it fits exactly or with room to spare', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
  assert.strictEqual(slugify('hello big world', { maxLength: 100 }), 'hello-big-world');
});

test('maxLength cuts hard when the first word does not fit', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('maxLength cut never ends with a dash', () => {
  assert.strictEqual(slugify('ab-cd', { maxLength: 2 }), 'ab');
  assert.strictEqual(slugify('ab-cd', { maxLength: 3 }), 'ab');
  assert.strictEqual(slugify('hello-world', { maxLength: 6 }), 'hello');
});

test('maxLength result never exceeds the limit', () => {
  const text = 'the quick brown fox jumps over the lazy dog';
  for (let max = 1; max <= text.length; max++) {
    const result = slugify(text, { maxLength: max });
    assert.ok(result.length <= max, `"${result}" longer than ${max}`);
    assert.ok(!result.endsWith('-'), `"${result}" ends with a dash`);
  }
});

test('throws TypeError when text is not a string', () => {
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify({}), TypeError);
});

test('throws TypeError when maxLength is not an integer', () => {
  assert.throws(() => slugify('hello', { maxLength: 2.5 }), TypeError);
  assert.throws(() => slugify('hello', { maxLength: '5' }), TypeError);
  assert.throws(() => slugify('hello', { maxLength: NaN }), TypeError);
});

test('throws RangeError when maxLength is below 1', () => {
  assert.throws(() => slugify('hello', { maxLength: 0 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: -3 }), RangeError);
});
