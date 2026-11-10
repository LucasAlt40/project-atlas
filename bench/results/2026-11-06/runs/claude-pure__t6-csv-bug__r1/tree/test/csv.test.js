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

test('doubled quote inside quoted field becomes one quote', () => {
  assert.deepStrictEqual(parseCsv('"say ""hi"""'), [['say "hi"']]);
});

test('file ending with newline has no extra empty row', () => {
  assert.deepStrictEqual(parseCsv('a,b\nc,d\n'), [['a', 'b'], ['c', 'd']]);
});

test('CRLF line endings leave no carriage return in fields', () => {
  assert.deepStrictEqual(parseCsv('a,b\r\nc,d\r\n'), [['a', 'b'], ['c', 'd']]);
});

test('line break inside quoted field stays in one row', () => {
  assert.deepStrictEqual(parseCsv('"line1\nline2",b\nc,d'), [['line1\nline2', 'b'], ['c', 'd']]);
});

test('CRLF inside quoted field is kept verbatim', () => {
  assert.deepStrictEqual(parseCsv('"a\r\nb",c\r\n'), [['a\r\nb', 'c']]);
});

test('empty fields are preserved', () => {
  assert.deepStrictEqual(parseCsv('a,,b'), [['a', '', 'b']]);
  assert.deepStrictEqual(parseCsv('a,b,\n,'), [['a', 'b', ''], ['', '']]);
});

test('empty quoted field is a field', () => {
  assert.deepStrictEqual(parseCsv('"",x'), [['', 'x']]);
  assert.deepStrictEqual(parseCsv('x,""'), [['x', '']]);
});

test('empty input gives no rows', () => {
  assert.deepStrictEqual(parseCsv(''), []);
});

test('last row without trailing newline is kept', () => {
  assert.deepStrictEqual(parseCsv('a\nb,"c"'), [['a'], ['b', 'c']]);
});

test('unterminated quoted field throws SyntaxError', () => {
  assert.throws(() => parseCsv('"abc,def\n'), SyntaxError);
  assert.throws(() => parseCsv('a,"b""'), SyntaxError);
});
