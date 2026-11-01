const { describe, it } = require('node:test');
const assert = require('node:assert');
const { slugify } = require('../src/slugify');

describe('slugify', () => {
  it('lowercases and joins words with dashes', () => {
    assert.strictEqual(slugify('Hello World'), 'hello-world');
  });

  it('removes accents and diacritics', () => {
    assert.strictEqual(slugify('Olá, Mundo!'), 'ola-mundo');
    assert.strictEqual(slugify('Ação & Reação'), 'acao-reacao');
    assert.strictEqual(slugify('Über Café'), 'uber-cafe');
    assert.strictEqual(slugify('ã ü ç é'), 'a-u-c-e');
  });

  it('collapses runs of non-alphanumeric characters into one dash', () => {
    assert.strictEqual(slugify('a   b --- c!!!d'), 'a-b-c-d');
  });

  it('never starts or ends with a dash', () => {
    assert.strictEqual(slugify('  Olá, Mundo!  '), 'ola-mundo');
    assert.strictEqual(slugify('!!!hello!!!'), 'hello');
  });

  it('returns an empty string when nothing alphanumeric remains', () => {
    assert.strictEqual(slugify('!!! ???'), '');
    assert.strictEqual(slugify(''), '');
  });

  it('throws a TypeError when text is not a string', () => {
    assert.throws(() => slugify(42), TypeError);
    assert.throws(() => slugify(null), TypeError);
    assert.throws(() => slugify(undefined), TypeError);
    assert.throws(() => slugify(['hello']), TypeError);
  });

  describe('maxLength', () => {
    it('leaves the slug alone when it already fits', () => {
      assert.strictEqual(slugify('hello big world', { maxLength: 15 }), 'hello-big-world');
      assert.strictEqual(slugify('hello big world', { maxLength: 100 }), 'hello-big-world');
    });

    it('cuts at the last dash that fits', () => {
      assert.strictEqual(slugify('hello big world', { maxLength: 9 }), 'hello-big');
      assert.strictEqual(slugify('hello big world', { maxLength: 8 }), 'hello');
    });

    it('cuts at a dash that lands exactly on maxLength', () => {
      assert.strictEqual(slugify('hello big world', { maxLength: 5 }), 'hello');
      assert.strictEqual(slugify('hello big world', { maxLength: 6 }), 'hello');
    });

    it('never returns a result ending with a dash', () => {
      const result = slugify('ab-cd', { maxLength: 3 });
      assert.strictEqual(result, 'ab');
      assert.ok(!result.endsWith('-'));
    });

    it('cuts a single long word hard at maxLength', () => {
      assert.strictEqual(slugify('supercalifragilistic', { maxLength: 5 }), 'super');
    });

    it('never exceeds maxLength', () => {
      const result = slugify('Ação de graças para todos os amigos', { maxLength: 12 });
      assert.ok(result.length <= 12);
      assert.strictEqual(result, 'acao-de');
    });
  });
});
