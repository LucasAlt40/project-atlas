const test = require('node:test');
const assert = require('node:assert');
const { parseCsv } = require('../src/csv.js');

test('simple rows', () => {
  assert.deepStrictEqual(parseCsv('a,b\nc,d'), [['a', 'b'], ['c', 'd']]);
});

test('quoted field without specials', () => {
  assert.deepStrictEqual(parseCsv('"a",b'), [['a', 'b']]);
});

test('comma inside quoted field does not split the field', () => {
  assert.deepStrictEqual(parseCsv('"Lisbon, Portugal",2'), [['Lisbon, Portugal', '2']]);
});

test('doubled quote inside quoted field becomes one quote', () => {
  assert.deepStrictEqual(parseCsv('"say ""hi"""'), [['say "hi"']]);
});

test('empty fields stay empty strings', () => {
  assert.deepStrictEqual(parseCsv('a,,b'), [['a', '', 'b']]);
  assert.deepStrictEqual(parseCsv('a,'), [['a', '']]);
  assert.deepStrictEqual(parseCsv('""'), [['']]);
});

test('empty input gives no rows', () => {
  assert.deepStrictEqual(parseCsv(''), []);
});

test('trailing newline does not add an empty row', () => {
  assert.deepStrictEqual(parseCsv('a,b\nc,d\n'), [['a', 'b'], ['c', 'd']]);
});

test('CRLF line endings leave no carriage return in fields', () => {
  assert.deepStrictEqual(parseCsv('a,b\r\nc,d\r\n'), [['a', 'b'], ['c', 'd']]);
});

test('line break inside quoted field stays in the same row', () => {
  assert.deepStrictEqual(parseCsv('"line1\nline2",x\ny,z'), [['line1\nline2', 'x'], ['y', 'z']]);
  assert.deepStrictEqual(parseCsv('"a\r\nb",c'), [['a\r\nb', 'c']]);
});

test('unterminated quoted field throws SyntaxError', () => {
  assert.throws(() => parseCsv('"abc'), SyntaxError);
  assert.throws(() => parseCsv('a,"b\nc'), SyntaxError);
});
