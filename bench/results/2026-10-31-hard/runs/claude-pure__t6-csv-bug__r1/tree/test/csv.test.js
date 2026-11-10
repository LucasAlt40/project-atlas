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
  assert.deepStrictEqual(parseCsv('"Lisbon, Portugal",x'), [['Lisbon, Portugal', 'x']]);
});

test('doubled quote inside quoted field', () => {
  assert.deepStrictEqual(parseCsv('"say ""hi"""'), [['say "hi"']]);
});

test('trailing newline adds no extra row', () => {
  assert.deepStrictEqual(parseCsv('a,b\nc,d\n'), [['a', 'b'], ['c', 'd']]);
});

test('CRLF line endings leave no carriage return', () => {
  assert.deepStrictEqual(parseCsv('a,b\r\nc,d\r\n'), [['a', 'b'], ['c', 'd']]);
});

test('line break inside quoted field stays in one row', () => {
  assert.deepStrictEqual(parseCsv('"line1\nline2",x\ny,z'), [['line1\nline2', 'x'], ['y', 'z']]);
});

test('empty fields are kept', () => {
  assert.deepStrictEqual(parseCsv('a,,b'), [['a', '', 'b']]);
  assert.deepStrictEqual(parseCsv('a,'), [['a', '']]);
});

test('empty input gives no rows', () => {
  assert.deepStrictEqual(parseCsv(''), []);
});

test('quoted empty field', () => {
  assert.deepStrictEqual(parseCsv('"",b'), [['', 'b']]);
});

test('unterminated quote throws SyntaxError', () => {
  assert.throws(() => parseCsv('"abc,def'), SyntaxError);
});
