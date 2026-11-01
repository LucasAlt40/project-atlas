// Parses CSV text into rows of fields, following RFC 4180.
function parseCsv(text) {
  const rows = [];
  let row = [];
  let field = '';
  let inQuotes = false;
  // True once the current row has any content, so a trailing newline does not add an empty row.
  let pending = false;
  for (let i = 0; i < text.length; i++) {
    const char = text[i];
    pending = true;
    if (inQuotes) {
      if (char !== '"') {
        field += char;
      } else if (text[i + 1] === '"') {
        // A doubled quote inside a quoted field is one literal quote.
        field += '"';
        i++;
      } else {
        inQuotes = false;
      }
    } else if (char === '"' && field === '') {
      inQuotes = true;
    } else if (char === ',') {
      row.push(field);
      field = '';
    } else if (char === '\r' && text[i + 1] === '\n') {
      // The \n that follows ends the row, so the \r of a CRLF is dropped.
    } else if (char === '\n') {
      row.push(field);
      rows.push(row);
      row = [];
      field = '';
      pending = false;
    } else {
      field += char;
    }
  }
  if (inQuotes) {
    throw new SyntaxError('Unterminated quoted field');
  }
  if (pending) {
    row.push(field);
    rows.push(row);
  }
  return rows;
}

module.exports = { parseCsv };
