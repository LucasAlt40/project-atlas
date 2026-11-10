const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('lowercases and joins words with a single dash', () => {
  assert.strictEqual(slugify('Hello World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Çava über, mãe'), 'cava-uber-mae');
});

test('collapses runs of non a-z0-9 characters into one dash', () => {
  assert.strictEqual(slugify('a   b---c!!!d'), 'a-b-c-d');
  assert.strictEqual(slugify('x_y.z/0 9'), 'x-y-z-0-9');
});

test('never starts or ends with a dash', () => {
  assert.strictEqual(slugify('  --hello--  '), 'hello');
  assert.strictEqual(slugify('!!!'), '');
});

test('keeps digits', () => {
  assert.strictEqual(slugify('Version 2 of 10'), 'version-2-of-10');
});

test('returns an empty string when nothing is left', () => {
  assert.strictEqual(slugify(''), '');
  assert.strictEqual(slugify('@#$%'), '');
});

test('throws TypeError when text is not a string', () => {
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
  assert.throws(() => slugify({ toString: () => 'x' }), TypeError);
});

test('maxLength cuts at the last word boundary that fits', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
  assert.strictEqual(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
});

test('maxLength hard cuts when the first word does not fit', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('maxLength result never ends with a dash', () => {
  for (let max = 1; max <= 15; max++) {
    const result = slugify('hello big world', { maxLength: max });
    assert.ok(result.length <= max, `length ${result.length} exceeds ${max}`);
    assert.ok(!result.endsWith('-'), `"${result}" ends with a dash`);
  }
});

test('maxLength has no effect when the slug already fits', () => {
  assert.strictEqual(slugify('short', { maxLength: 50 }), 'short');
  assert.strictEqual(slugify('short', { maxLength: 5 }), 'short');
});

test('throws RangeError for an invalid maxLength', () => {
  assert.throws(() => slugify('abc', { maxLength: 0 }), RangeError);
  assert.throws(() => slugify('abc', { maxLength: -3 }), RangeError);
  assert.throws(() => slugify('abc', { maxLength: 2.5 }), RangeError);
  assert.throws(() => slugify('abc', { maxLength: '5' }), RangeError);
});
