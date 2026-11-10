const test = require('node:test');
const assert = require('node:assert');
const { parseCsv } = require('../src/csv.js');

test('simple rows', () => {
  assert.deepStrictEqual(parseCsv('a,b\nc,d'), [['a', 'b'], ['c', 'd']]);
});

test('quoted field without specials', () => {
  assert.deepStrictEqual(parseCsv('"a",b'), [['a', 'b']]);
});

test('comma inside a quoted field stays in the field', () => {
  assert.deepStrictEqual(parseCsv('"Lisbon, Portugal",x'), [['Lisbon, Portugal', 'x']]);
});

test('doubled quote inside a quoted field becomes one quote', () => {
  assert.deepStrictEqual(parseCsv('"say ""hi"""'), [['say "hi"']]);
});

test('trailing newline does not add an extra row', () => {
  assert.deepStrictEqual(parseCsv('a,b\nc,d\n'), [['a', 'b'], ['c', 'd']]);
});

test('CRLF line endings leave no carriage return in fields', () => {
  assert.deepStrictEqual(parseCsv('a,b\r\nc,d\r\n'), [['a', 'b'], ['c', 'd']]);
});

test('line break inside a quoted field stays in one row', () => {
  assert.deepStrictEqual(parseCsv('"line1\nline2",x\ny,z'), [['line1\nline2', 'x'], ['y', 'z']]);
});

test('empty fields are kept as empty strings', () => {
  assert.deepStrictEqual(parseCsv('a,,b'), [['a', '', 'b']]);
});

test('empty input gives no rows', () => {
  assert.deepStrictEqual(parseCsv(''), []);
});

test('quote that is never closed throws SyntaxError', () => {
  assert.throws(() => parseCsv('a,"abc\nd'), SyntaxError);
});

test('quote inside an unquoted field is kept literally', () => {
  assert.deepStrictEqual(parseCsv('a"b,c'), [['a"b', 'c']]);
});
