const { test } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

test('task examples', () => {
  assert.strictEqual(slugify('  Olá, Mundo!  '), 'ola-mundo');
  assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
  assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('result is lowercase', () => {
  assert.strictEqual(slugify('HeLLo'), 'hello');
});

test('accents and diacritics are removed', () => {
  assert.strictEqual(slugify('é ç ã ü'), 'e-c-a-u');
  assert.strictEqual(slugify('Müller São José'), 'muller-sao-jose');
});

test('each run of non a-z0-9 characters becomes a single dash', () => {
  assert.strictEqual(slugify('a --- b  c!!d'), 'a-b-c-d');
});

test('result never starts or ends with a dash', () => {
  assert.strictEqual(slugify('!!!x???'), 'x');
  assert.strictEqual(slugify('  --hi--  '), 'hi');
});

test('input with no a-z0-9 characters gives an empty slug', () => {
  assert.strictEqual(slugify(''), '');
  assert.strictEqual(slugify('***'), '');
});

test('without maxLength the slug is not truncated', () => {
  const long = 'one two three four five six seven eight nine ten';
  assert.strictEqual(slugify(long), 'one-two-three-four-five-six-seven-eight-nine-ten');
});

test('maxLength equal to the slug length returns it unchanged', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
});

test('maxLength one short of the slug length cuts at the last word', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 14 }), 'hello-big');
});

test('maxLength cut lands exactly on a dash', () => {
  assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
});

test('first word longer than maxLength is cut hard', () => {
  assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
});

test('result never ends with a dash and fits maxLength for every limit', () => {
  const text = 'hello big world, this is a test';
  const full = slugify(text);
  for (let max = 1; max <= full.length; max++) {
    const result = slugify(text, { maxLength: max });
    assert.ok(result.length <= max, `length ${result.length} exceeds ${max}`);
    assert.ok(!result.startsWith('-'), `leading dash for max ${max}`);
    assert.ok(!result.endsWith('-'), `trailing dash for max ${max}`);
  }
});

test('non-string text throws TypeError', () => {
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
});

test('invalid maxLength throws RangeError', () => {
  assert.throws(() => slugify('hello', { maxLength: 0 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: -1 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: 1.5 }), RangeError);
  assert.throws(() => slugify('hello', { maxLength: '5' }), RangeError);
});
