// Parses CSV text into rows of fields, following RFC 4180.
function parseCsv(text) {
  const rows = [];
  let row = [];
  let field = '';
  let inQuotes = false;
  let i = 0;

  while (i < text.length) {
    const char = text[i];
    if (inQuotes) {
      if (char === '"' && text[i + 1] === '"') {
        field += '"';
        i += 1;
      } else if (char === '"') {
        inQuotes = false;
      } else {
        field += char;
      }
    } else if (char === '"' && field === '') {
      inQuotes = true;
    } else if (char === ',') {
      row.push(field);
      field = '';
    } else if (char === '\n') {
      row.push(field);
      rows.push(row);
      row = [];
      field = '';
    } else if (char !== '\r' || text[i + 1] !== '\n') {
      // A \r directly before \n is part of the CRLF line ending, so it is dropped.
      field += char;
    }
    i += 1;
  }

  if (inQuotes) {
    throw new SyntaxError('Unterminated quoted field');
  }
  // Text that ends with a newline has already closed its last row.
  if (text !== '' && !text.endsWith('\n')) {
    row.push(field);
    rows.push(row);
  }
  return rows;
}

module.exports = { parseCsv };
