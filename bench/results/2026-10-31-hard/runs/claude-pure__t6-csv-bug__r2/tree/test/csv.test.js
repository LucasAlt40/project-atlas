const test = require('node:test');
const assert = require('node:assert');
const { parseCsv } = require('../src/csv.js');

test('simple rows', () => {
  assert.deepStrictEqual(parseCsv('a,b\nc,d'), [['a', 'b'], ['c', 'd']]);
});

test('quoted field without specials', () => {
  assert.deepStrictEqual(parseCsv('"a",b'), [['a', 'b']]);
});

test('comma inside quoted field', () => {
  assert.deepStrictEqual(parseCsv('"Lisbon, Portugal",2'), [['Lisbon, Portugal', '2']]);
});

test('doubled quote inside quoted field', () => {
  assert.deepStrictEqual(parseCsv('"say ""hi"""'), [['say "hi"']]);
});

test('line break inside quoted field', () => {
  assert.deepStrictEqual(parseCsv('"a\nb",c\nd,e'), [['a\nb', 'c'], ['d', 'e']]);
});

test('trailing newline does not add an empty row', () => {
  assert.deepStrictEqual(parseCsv('a,b\n'), [['a', 'b']]);
});

test('CRLF line endings leave no carriage return', () => {
  assert.deepStrictEqual(parseCsv('a,b\r\nc,d\r\n'), [['a', 'b'], ['c', 'd']]);
});

test('empty fields are kept as empty strings', () => {
  assert.deepStrictEqual(parseCsv('a,,b'), [['a', '', 'b']]);
  assert.deepStrictEqual(parseCsv('a,b,'), [['a', 'b', '']]);
});

test('empty input gives no rows', () => {
  assert.deepStrictEqual(parseCsv(''), []);
});

test('unterminated quoted field throws SyntaxError', () => {
  assert.throws(() => parseCsv('"abc,def'), SyntaxError);
});
