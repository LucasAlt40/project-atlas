const test = require('node:test');
const assert = require('node:assert');
const { parseCsv } = require('../src/csv.js');

test('simple rows', () => {
  assert.deepStrictEqual(parseCsv('a,b\nc,d'), [['a', 'b'], ['c', 'd']]);
});

test('quoted field without specials', () => {
  assert.deepStrictEqual(parseCsv('"a",b'), [['a', 'b']]);
});

test('comma inside quoted field is not a separator', () => {
  assert.deepStrictEqual(parseCsv('"Lisbon, Portugal",2'), [['Lisbon, Portugal', '2']]);
});

test('doubled quote inside quoted field becomes one quote', () => {
  assert.deepStrictEqual(parseCsv('"say ""hi"""'), [['say "hi"']]);
});

test('trailing newline does not add an empty row', () => {
  assert.deepStrictEqual(parseCsv('a,b\nc,d\n'), [['a', 'b'], ['c', 'd']]);
});

test('CRLF line endings leave no carriage return in fields', () => {
  assert.deepStrictEqual(parseCsv('a,"b"\r\nc,d\r\n'), [['a', 'b'], ['c', 'd']]);
});

test('line break inside quoted field stays in one row', () => {
  assert.deepStrictEqual(parseCsv('"line1\nline2",x\ny,z'), [['line1\nline2', 'x'], ['y', 'z']]);
});

test('empty fields are kept', () => {
  assert.deepStrictEqual(parseCsv('a,,b'), [['a', '', 'b']]);
  assert.deepStrictEqual(parseCsv('"",b'), [['', 'b']]);
});

test('empty input gives no rows', () => {
  assert.deepStrictEqual(parseCsv(''), []);
});

test('quote in the middle of an unquoted field is kept literally', () => {
  assert.deepStrictEqual(parseCsv('5" screen,2'), [['5" screen', '2']]);
  assert.deepStrictEqual(parseCsv('a"b,c"d'), [['a"b', 'c"d']]);
});

test('unterminated quoted field throws SyntaxError', () => {
  assert.throws(() => parseCsv('"abc,def'), SyntaxError);
  assert.throws(() => parseCsv('a,"b\nc'), SyntaxError);
});
