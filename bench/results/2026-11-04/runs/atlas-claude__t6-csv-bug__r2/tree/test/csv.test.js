const test = require('node:test');
const assert = require('node:assert');
const { parseCsv } = require('../src/csv.js');

test('simple rows', () => {
  assert.deepStrictEqual(parseCsv('a,b\nc,d'), [['a', 'b'], ['c', 'd']]);
});

test('quoted field without specials', () => {
  assert.deepStrictEqual(parseCsv('"a",b'), [['a', 'b']]);
});

test('empty fields stay empty strings', () => {
  assert.deepStrictEqual(parseCsv('a,,b'), [['a', '', 'b']]);
});

test('empty input gives no rows', () => {
  assert.deepStrictEqual(parseCsv(''), []);
});

test('comma inside quoted field is not a separator', () => {
  assert.deepStrictEqual(parseCsv('"Lisbon, Portugal",x'), [['Lisbon, Portugal', 'x']]);
});

test('doubled quote inside quoted field becomes one quote', () => {
  assert.deepStrictEqual(parseCsv('"say ""hi"""'), [['say "hi"']]);
});

test('trailing newline does not add an extra row', () => {
  assert.deepStrictEqual(parseCsv('a,b\nc,d\n'), [['a', 'b'], ['c', 'd']]);
});

test('CRLF line endings leave no carriage return in fields', () => {
  assert.deepStrictEqual(parseCsv('a,b\r\nc,d\r\n'), [['a', 'b'], ['c', 'd']]);
});

test('line break inside quoted field stays in the same row', () => {
  assert.deepStrictEqual(parseCsv('"line one\nline two",x\ny,z'), [
    ['line one\nline two', 'x'],
    ['y', 'z'],
  ]);
});

test('unclosed quote throws SyntaxError', () => {
  assert.throws(() => parseCsv('"abc,def'), SyntaxError);
});

test('quote in the middle of an unquoted field is a literal character', () => {
  assert.deepStrictEqual(parseCsv('5" screen,x'), [['5" screen', 'x']]);
});

test('empty quoted field at the start of a field stays empty', () => {
  assert.deepStrictEqual(parseCsv('"",b'), [['', 'b']]);
});
