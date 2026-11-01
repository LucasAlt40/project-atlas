const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('lowercases and joins words with dashes', () => {
  assert.strictEqual(slugify('Hello World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Çüã'), 'cua');
});

test('collapses runs of non a-z0-9 characters into a single dash', () => {
  assert.strictEqual(slugify('a   b---c!!!d'), 'a-b-c-d');
  assert.strictEqual(slugify('x_y.z/1'), 'x-y-z-1');
});

test('never starts or ends with a dash', () => {
  assert.strictEqual(slugify('  --hello--  '), 'hello');
  assert.strictEqual(slugify('!!!'), '');
  assert.strictEqual(slugify(''), '');
});

test('returns no limit when maxLength is undefined', () => {
  const long = 'word '.repeat(50).trim();
  assert.strictEqual(slugify(long), long.replace(/ /g, '-'));
});

test('keeps result unchanged when it already fits maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
  assert.strictEqual(slugify('hello big world', { maxLength: 100 }), 'hello-big-world');
});

test('cuts at the last dash that fits maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
});

test('cuts at a dash that sits exactly at maxLength', () => {
  // 'hello-big' is 9 chars; the dash at index 5 is exactly where the first word ends.
  assert.strictEqual(slugify('hello big world', { maxLength: 5 }), 'hello');
});

test('hard-cuts when the first word does not fit maxLength', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('never returns a result longer than maxLength or ending with a dash', () => {
  for (let max = 1; max <= 20; max++) {
    const result = slugify('a bb ccc dddd eeeee ffffff', { maxLength: max });
    assert.ok(result.length <= max, `length ${result.length} > ${max}`);
    assert.ok(!result.endsWith('-'), `"${result}" ends with a dash`);
  }
});

test('throws TypeError when text is not a string', () => {
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify({}), TypeError);
});

test('throws RangeError when maxLength is not a positive integer', () => {
  assert.throws(() => slugify('hello', { maxLength: 0 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: -3 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: 2.5 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: '5' }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: null }), RangeError);
});
