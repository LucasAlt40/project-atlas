const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('lowercases and joins words with a single dash', () => {
  assert.strictEqual(slugify('Hello World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('Güemes, São Paulo'), 'guemes-sao-paulo');
});

test('collapses runs of non a-z0-9 characters into one dash', () => {
  assert.strictEqual(slugify('a   b---c!!!d'), 'a-b-c-d');
  assert.strictEqual(slugify('x_y.z/w'), 'x-y-z-w');
});

test('never starts or ends with a dash', () => {
  assert.strictEqual(slugify('  --hello--  '), 'hello');
  assert.strictEqual(slugify('!!!'), '');
  assert.strictEqual(slugify(''), '');
});

test('returns the slug unchanged when it fits maxLength', () => {
  assert.strictEqual(slugify('hello big', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello', { maxLength: 5 }), 'hello');
});

test('cuts at the last dash that fits maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
  assert.strictEqual(slugify('hello big world', { maxLength: 10 }), 'hello-big');
});

test('treats a dash exactly at maxLength as a word boundary', () => {
  assert.strictEqual(slugify('hello world', { maxLength: 5 }), 'hello');
});

test('cuts the first word hard when it does not fit maxLength', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
  assert.strictEqual(slugify('supercal ifragilistic', { maxLength: 5 }), 'super');
});

test('result never exceeds maxLength and never ends with a dash', () => {
  const result = slugify('one two three four five', { maxLength: 11 });
  assert.ok(result.length <= 11);
  assert.ok(!result.endsWith('-'));
  assert.strictEqual(result, 'one-two');
});

test('throws a TypeError when text is not a string', () => {
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
});
