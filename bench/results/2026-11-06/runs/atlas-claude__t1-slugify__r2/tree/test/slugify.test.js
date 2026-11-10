const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('lowercases, strips accents and joins words with a dash', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Güssen Ñandú'), 'gussen-nandu');
  assert.strictEqual(slugify('Çà ü, ã é'), 'ca-u-a-e');
});

test('collapses runs of non a-z0-9 characters into a single dash', () => {
  assert.strictEqual(slugify('a   b---c!!!d'), 'a-b-c-d');
});

test('never starts or ends with a dash', () => {
  assert.strictEqual(slugify('---hello---'), 'hello');
  assert.strictEqual(slugify('  !!hello world??  '), 'hello-world');
});

test('returns an empty string when nothing alphanumeric remains', () => {
  assert.strictEqual(slugify(''), '');
  assert.strictEqual(slugify('!!! ???'), '');
});

test('accepts null or missing options', () => {
  assert.strictEqual(slugify('hello world', null), 'hello-world');
  assert.strictEqual(slugify('hello world', {}), 'hello-world');
});

test('leaves the slug alone when it already fits maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
  assert.strictEqual(slugify('hello big world', { maxLength: 100 }), 'hello-big-world');
});

test('cuts at the last dash that fits so no word is split', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
  assert.strictEqual(slugify('ab cd', { maxLength: 3 }), 'ab');
});

test('cuts hard when the first word does not fit', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
  assert.strictEqual(slugify('hello big world', { maxLength: 1 }), 'h');
});

test('never returns a result ending with a dash after truncation', () => {
  for (let max = 1; max <= 15; max++) {
    const result = slugify('hello big world', { maxLength: max });
    assert.ok(result.length <= max, `length ${result.length} exceeds ${max}`);
    assert.ok(!result.endsWith('-'), `"${result}" ends with a dash`);
  }
});

test('throws a TypeError when text is not a string', () => {
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
});

test('throws a RangeError when maxLength is not a positive integer', () => {
  for (const maxLength of [0, -1, 1.5, '5', NaN, null]) {
    assert.throws(() => slugify('hello world', { maxLength }), RangeError, `maxLength ${maxLength}`);
  }
});
