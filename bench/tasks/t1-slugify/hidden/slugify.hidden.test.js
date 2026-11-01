const test = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify.js');

test('lowercase and spaces', () => assert.strictEqual(slugify('Hello World'), 'hello-world'));
test('trims and punctuation', () => assert.strictEqual(slugify('  Olá, Mundo!  '), 'ola-mundo'));
test('accents', () => assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao'));
test('more diacritics', () => assert.strictEqual(slugify('Über Crème Brûlée'), 'uber-creme-brulee'));
test('runs collapse', () => assert.strictEqual(slugify('a---b___c   d'), 'a-b-c-d'));
test('no leading or trailing dash', () => assert.strictEqual(slugify('--x--'), 'x'));
test('digits kept', () => assert.strictEqual(slugify('Top 10 dicas 2026'), 'top-10-dicas-2026'));
test('empty and only symbols', () => { assert.strictEqual(slugify(''), ''); assert.strictEqual(slugify('!!!'), ''); });
test('maxLength exact fit', () => assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big'));
test('maxLength cut at word boundary', () => assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello'));
test('maxLength hard cut of first word', () => assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super'));
test('maxLength larger than text', () => assert.strictEqual(slugify('a b', { maxLength: 50 }), 'a-b'));
test('maxLength never ends with dash', () => {
  const s = slugify('one two three four', { maxLength: 8 });
  assert.ok(s.length <= 8 && !s.endsWith('-'), s);
  assert.strictEqual(s, 'one-two');
});
test('non-string throws TypeError', () => {
  assert.throws(() => slugify(42), TypeError);
  assert.throws(() => slugify(null), TypeError);
  assert.throws(() => slugify(undefined), TypeError);
});
