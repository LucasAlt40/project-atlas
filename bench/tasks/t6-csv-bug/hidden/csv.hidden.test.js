const test = require('node:test');
const assert = require('node:assert');
const { parseCsv } = require('../src/csv.js');

test('simple rows still work', () => assert.deepStrictEqual(parseCsv('a,b\nc,d'), [['a', 'b'], ['c', 'd']]));
test('comma inside quotes', () => assert.deepStrictEqual(parseCsv('city,"Lisbon, Portugal"'), [['city', 'Lisbon, Portugal']]));
test('doubled quote', () => assert.deepStrictEqual(parseCsv('"say ""hi"""'), [['say "hi"']]));
test('trailing newline adds no row', () => assert.deepStrictEqual(parseCsv('a,b\n'), [['a', 'b']]));
test('CRLF', () => assert.deepStrictEqual(parseCsv('a,b\r\nc,d\r\n'), [['a', 'b'], ['c', 'd']]));
test('newline inside quotes', () => assert.deepStrictEqual(parseCsv('"line1\nline2",x'), [['line1\nline2', 'x']]));
test('CRLF inside quotes is kept', () => assert.deepStrictEqual(parseCsv('"a\r\nb"'), [['a\r\nb']]));
test('empty fields', () => assert.deepStrictEqual(parseCsv('a,,b\n,,'), [['a', '', 'b'], ['', '', '']]));
test('empty quoted field', () => assert.deepStrictEqual(parseCsv('"",x'), [['', 'x']]));
test('empty input', () => assert.deepStrictEqual(parseCsv(''), []));
test('unterminated quote throws', () => assert.throws(() => parseCsv('"abc,d'), SyntaxError));
test('all combined', () => assert.deepStrictEqual(
  parseCsv('name,quote\r\n"Ana, B.","She said ""ok""\nthen left"\r\n'),
  [['name', 'quote'], ['Ana, B.', 'She said "ok"\nthen left']]));
