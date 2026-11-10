// Parses CSV text into rows of fields, following RFC 4180.
function parseCsv(text) {
  const rows = [];
  let row = [];
  let field = '';
  let quoted = false;
  let inQuotes = false;

  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (inQuotes) {
      if (ch === '"' && text[i + 1] === '"') {
        field += '"';
        i++;
      } else if (ch === '"') {
        inQuotes = false;
      } else {
        field += ch;
      }
    } else if (ch === '"' && field === '' && !quoted) {
      inQuotes = true;
      quoted = true;
    } else if (ch === ',') {
      row.push(field);
      field = '';
      quoted = false;
    } else if (ch === '\n') {
      row.push(field);
      rows.push(row);
      row = [];
      field = '';
      quoted = false;
    } else if (ch === '\r' && text[i + 1] === '\n') {
      // CRLF: the LF that follows ends the row.
      continue;
    } else {
      field += ch;
    }
  }

  if (inQuotes) {
    throw new SyntaxError('Unterminated quoted field');
  }
  // A last line without a trailing newline still forms a row.
  if (row.length > 0 || field !== '' || quoted) {
    row.push(field);
    rows.push(row);
  }
  return rows;
}

module.exports = { parseCsv };
