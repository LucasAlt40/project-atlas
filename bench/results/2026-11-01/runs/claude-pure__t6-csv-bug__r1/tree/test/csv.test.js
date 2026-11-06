const test = require('node:test');
const assert = require('node:assert');
const { parseCsv } = require('../src/csv.js');

test('simple rows', () => {
  assert.deepStrictEqual(parseCsv('a,b\nc,d'), [['a', 'b'], ['c', 'd']]);
});

test('quoted field without specials', () => {
  assert.deepStrictEqual(parseCsv('"a",b'), [['a', 'b']]);
});

test('comma inside a quoted field', () => {
  assert.deepStrictEqual(parseCsv('"Lisbon, Portugal",2'), [['Lisbon, Portugal', '2']]);
});

test('doubled quote inside a quoted field', () => {
  assert.deepStrictEqual(parseCsv('"say ""hi"""'), [['say "hi"']]);
});

test('line break inside a quoted field', () => {
  assert.deepStrictEqual(parseCsv('"a\nb",c\nd,e'), [['a\nb', 'c'], ['d', 'e']]);
});

test('trailing newline does not add a row', () => {
  assert.deepStrictEqual(parseCsv('a,b\nc,d\n'), [['a', 'b'], ['c', 'd']]);
});

test('CRLF line endings leave no carriage return', () => {
  assert.deepStrictEqual(parseCsv('a,b\r\nc,"d"\r\n'), [['a', 'b'], ['c', 'd']]);
});

test('empty fields are kept as empty strings', () => {
  assert.deepStrictEqual(parseCsv('a,,b\n,\n""'), [['a', '', 'b'], ['', ''], ['']]);
});

test('empty input gives no rows', () => {
  assert.deepStrictEqual(parseCsv(''), []);
});

test('unclosed quote throws a SyntaxError', () => {
  assert.throws(() => parseCsv('"abc,def'), SyntaxError);
});
