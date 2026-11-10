const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify.js');

test('lowercases and joins words with a hyphen', () => {
  assert.strictEqual(slugify('Hello World'), 'hello-world');
});

test('removes accents and diacritics', () => {
  assert.strictEqual(slugify('é'), 'e');
  assert.strictEqual(slugify('Ação'), 'acao');
  assert.strictEqual(slugify('Müller Ünterwegs'), 'muller-unterwegs');
  assert.strictEqual(slugify(' Olá, Mundo! '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
});

test('collapses every run of non a-z0-9 characters into one hyphen', () => {
  assert.strictEqual(slugify('a   b__c...d'), 'a-b-c-d');
  assert.strictEqual(slugify('foo!@#$%^&*()bar'), 'foo-bar');
});

test('never starts or ends with a hyphen', () => {
  assert.strictEqual(slugify('---hello---'), 'hello');
  assert.strictEqual(slugify('  !hello?  '), 'hello');
});

test('keeps digits', () => {
  assert.strictEqual(slugify('Version 2.0 Final'), 'version-2-0-final');
});

test('returns an empty string when nothing is left', () => {
  assert.strictEqual(slugify(''), '');
  assert.strictEqual(slugify('!!! ???'), '');
});

test('throws a TypeError when text is not a string', () => {
  assert.throws(() => slugify(undefined), TypeError);
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(['hello']), TypeError);
});

test('leaves the result alone when it already fits maxLength', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
  assert.strictEqual(slugify('hello big world', { maxLength: 100 }), 'hello-big-world');
});

test('cuts at the last hyphen that fits', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
  assert.strictEqual(slugify('hello big world', { maxLength: 10 }), 'hello-big');
});

test('cuts hard when the first word does not fit', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('never ends with a hyphen after truncation', () => {
  for (let maxLength = 1; maxLength <= 20; maxLength++) {
    const result = slugify('one two three four five', { maxLength });
    assert.ok(result.length <= maxLength, `length ${result.length} > ${maxLength}`);
    assert.ok(!result.endsWith('-'), `"${result}" ends with a hyphen`);
  }
});

test('throws a RangeError when maxLength is not a positive integer', () => {
  assert.throws(() => slugify('hello', { maxLength: 0 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: -3 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: 2.5 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: '5' }), RangeError);
});
