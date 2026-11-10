const test = require('node:test');
const assert = require('node:assert');
const { parseCsv } = require('../src/csv.js');

test('simple rows', () => {
  assert.deepStrictEqual(parseCsv('a,b\nc,d'), [['a', 'b'], ['c', 'd']]);
});

test('quoted field without specials', () => {
  assert.deepStrictEqual(parseCsv('"a",b'), [['a', 'b']]);
});

test('empty fields stay as empty strings', () => {
  assert.deepStrictEqual(parseCsv('a,,b'), [['a', '', 'b']]);
});

test('empty input gives no rows', () => {
  assert.deepStrictEqual(parseCsv(''), []);
});

test('comma inside a quoted field does not split it', () => {
  assert.deepStrictEqual(parseCsv('"Lisbon, Portugal",2'), [['Lisbon, Portugal', '2']]);
});

test('doubled quote inside a quoted field becomes one quote', () => {
  assert.deepStrictEqual(parseCsv('"say ""hi"""'), [['say "hi"']]);
});

test('trailing newline does not add an extra row', () => {
  assert.deepStrictEqual(parseCsv('a,b\n'), [['a', 'b']]);
});

test('CRLF line endings leave no carriage return in fields', () => {
  assert.deepStrictEqual(parseCsv('a,b\r\nc,d\r\n'), [['a', 'b'], ['c', 'd']]);
});

test('line break inside a quoted field stays in the same row', () => {
  assert.deepStrictEqual(parseCsv('"line1\nline2",x\ny,z'), [['line1\nline2', 'x'], ['y', 'z']]);
});

test('unclosed quote throws SyntaxError', () => {
  assert.throws(() => parseCsv('"abc,def'), SyntaxError);
});
