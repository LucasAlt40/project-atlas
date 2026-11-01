Bug reports about parseCsv in src/csv.js, which must follow RFC 4180:
- "Lisbon, Portugal" (a quoted field with a comma inside) is split into two fields.
- A quoted field with a doubled quote, such as "say ""hi""", should become: say "hi" — it does not.
- A file that ends with a newline gets an extra row containing one empty field.
- Files saved on Windows (CRLF line endings) leave a "\r" at the end of the last field of each row.
- A quoted field may contain a line break; such a field is cut into two rows.

parseCsv(text) returns an array of rows, each an array of strings. Empty fields stay as "" (a,,b gives ["a", "", "b"]), and an empty input gives []. A quote that opens a field and is never closed throws a SyntaxError. Find the root causes and fix them in src/csv.js, keeping module.exports = { parseCsv }. The existing tests in test/csv.test.js must keep passing; add regression tests there.
