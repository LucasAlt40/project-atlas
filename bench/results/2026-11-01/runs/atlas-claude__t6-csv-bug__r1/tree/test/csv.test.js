const test = require('node:test');
const assert = require('node:assert');
const { parseCsv } = require('../src/csv.js');

test('simple rows', () => {
  assert.deepStrictEqual(parseCsv('a,b\nc,d'), [['a', 'b'], ['c', 'd']]);
});

test('quoted field without specials', () => {
  assert.deepStrictEqual(parseCsv('"a",b'), [['a', 'b']]);
});

test('comma inside quoted field stays in one field', () => {
  assert.deepStrictEqual(parseCsv('"Lisbon, Portugal",PT'), [['Lisbon, Portugal', 'PT']]);
});

test('doubled quote inside quoted field becomes one quote', () => {
  assert.deepStrictEqual(parseCsv('"say ""hi"""'), [['say "hi"']]);
});

test('file ending with newline has no extra row', () => {
  assert.deepStrictEqual(parseCsv('a,b\nc,d\n'), [['a', 'b'], ['c', 'd']]);
});

test('CRLF line endings leave no carriage return in fields', () => {
  assert.deepStrictEqual(parseCsv('a,"b"\r\nc,d\r\n'), [['a', 'b'], ['c', 'd']]);
});

test('quoted field may contain a line break', () => {
  assert.deepStrictEqual(parseCsv('"line1\nline2",x\nz,w'), [['line1\nline2', 'x'], ['z', 'w']]);
});

test('quoted field may contain a CRLF line break unchanged', () => {
  assert.deepStrictEqual(parseCsv('"a\r\nb",c'), [['a\r\nb', 'c']]);
});

test('empty fields are kept as empty strings', () => {
  assert.deepStrictEqual(parseCsv('a,,b'), [['a', '', 'b']]);
});

test('empty input gives no rows', () => {
  assert.deepStrictEqual(parseCsv(''), []);
});

test('unclosed quote throws SyntaxError', () => {
  assert.throws(() => parseCsv('"abc,def'), SyntaxError);
});
